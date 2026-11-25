//! Loading sites, doors, stairs, lamps, the Catalog and death.

use super::actors::{ChestState, NodeState, make_enemy, make_guardian};
use super::events::{GameEvent, Sfx};
use super::physics::{self, Mobility};
use super::player::Body;
use super::profile::{DeathBag, LampRecord, VisitRecord};
use super::vec::{V2, v2};
use super::{Game, tile_center};
use crate::babel::rarity::Rarity;
use crate::data::{Consumable, Hazard, RecipeOutput, Resource};
use crate::progression::{catalog, crafting, items};
use crate::world::grid::{Pos, Tile, entry_pos};
use crate::world::hex::Dir;
use crate::world::site::{self, ChestReward, HOME_LAMP, SiteKey};
use crate::world::validation::nearest_walkable;
use std::collections::BTreeMap;

/// Return lamps that can be lit at once.
pub const MAX_LAMPS: usize = 12;

#[derive(Clone, Copy, Debug)]
pub enum Arrival {
    /// Walking in through a doorway on this side.
    Door(Dir),
    /// Coming down a staircase: a way back up appears.
    FromAbove,
    /// Climbing back up.
    FromBelow,
    Hub,
    Lamp,
    At(V2),
}

impl Game {
    /// Generates and populates a site, applying the player's saved changes.
    pub fn enter(&mut self, key: SiteKey, arrival: Arrival) {
        if key != self.site.key {
            self.site =
                site::generate_with(&self.data, &self.feistel, self.profile.world_seed, &key);
        }
        self.profile.world.current = key.clone();
        match arrival {
            Arrival::Door(d) => self.delta_mut().opened_doors[d.index()] = true,
            Arrival::FromAbove => {
                let hub = self.site.hub;
                self.delta_mut().stairs_up = Some(hub);
            }
            _ => {}
        }
        let delta = self.delta();
        for d in Dir::ALL {
            if delta.opened_doors[d.index()] {
                self.site.grid.open_door(d);
            }
        }

        self.nodes = self
            .site
            .nodes
            .iter()
            .filter(|n| !delta.depleted_nodes.contains(&n.id))
            .map(|n| {
                let hp = self.data.node(n.kind).hits as f32;
                NodeState {
                    id: n.id,
                    kind: n.kind,
                    tile: n.pos,
                    pos: tile_center(n.pos),
                    hp,
                    max_hp: hp,
                    shake: 0.0,
                }
            })
            .collect();
        self.chests = self
            .site
            .chests
            .iter()
            .map(|c| ChestState {
                id: c.id,
                pos: tile_center(c.pos),
                reward: c.reward.clone(),
                open: delta.opened_chests.contains(&c.id),
            })
            .collect();
        self.drops.clear();
        self.claim_forgotten_items();
        self.projectiles.clear();
        self.zones.clear();
        self.darkness = 0.0;
        self.hitstop = 0.0;

        let spawns: Vec<_> = if delta.cleared {
            Vec::new()
        } else {
            self.site
                .enemies
                .iter()
                .filter(|e| !delta.defeated_enemies.contains(&e.id))
                .cloned()
                .collect()
        };
        self.enemies = Vec::with_capacity(spawns.len() + 1);
        for s in &spawns {
            let uid = self.new_uid();
            let enemy = make_enemy(&self.data, uid, self.site.depth, s);
            self.enemies.push(enemy);
        }
        self.conjure_illusions();
        if self.stairs_locked()
            && let Some(g) = self.site.guardian.clone()
        {
            let uid = self.new_uid();
            let boss = make_guardian(&self.data, uid, self.site.depth, &g);
            let intro = self
                .data
                .enemies
                .guardians
                .get(g.index)
                .map(|d| d.intro.clone())
                .unwrap_or_default();
            self.events.push(GameEvent::Banner {
                title: boss.name.clone(),
                subtitle: intro,
                color: [255, 90, 110],
            });
            self.sound(Sfx::BossRoar);
            self.enemies.push(boss);
        }

        let hub = tile_center(self.site.hub);
        let wanted = match arrival {
            Arrival::Door(d) => tile_center(entry_pos(d)),
            Arrival::FromAbove => hub + v2(0.0, 22.0),
            Arrival::FromBelow => self
                .site
                .stairs
                .map_or(hub, |s| tile_center(s) + v2(0.0, 22.0)),
            Arrival::Hub => hub + v2(0.0, 22.0),
            Arrival::Lamp => self.lamp_here().map_or(hub, |l| l + v2(0.0, 18.0)),
            Arrival::At(p) => p,
        };
        let pos = self.safe_position(wanted);
        let status = self.body.status;
        self.body = Body::new(pos);
        self.body.status = status;
        self.body.dash_charges = self.derived.dash_charges;
        if let Arrival::Door(d) = arrival {
            let c = tile_center(entry_pos(d));
            self.body.facing = (hub - c).norm();
        }
        self.profile.world.position = (pos.x, pos.y);
        // Nobody may stand on top of the newcomer.
        for e in &mut self.enemies {
            let d = e.pos - pos;
            if d.len() < 64.0 {
                e.grace = e.grace.max(1.8);
            }
        }
        self.record_visit();
        self.events.push(GameEvent::SiteEntered);
        self.events.push(GameEvent::Autosave);
        self.discover_biome();
    }

    fn claim_forgotten_items(&mut self) {
        let forgotten = std::mem::take(&mut self.profile.world.unclaimed);
        if forgotten.is_empty() {
            return;
        }
        for item in forgotten {
            self.say(format!("Recogiste {} al partir", item.name));
            self.profile.player.receive_item(item);
        }
        self.refresh();
    }

    /// Mirror galleries double their inhabitants with harmless reflections.
    fn conjure_illusions(&mut self) {
        if self.data.biomes[self.site.recipe.biome].hazard != Hazard::Mirrors {
            return;
        }
        let originals: Vec<_> = self.enemies.iter().take(4).cloned().collect();
        for original in originals {
            let mut decoy = original;
            decoy.uid = self.new_uid();
            decoy.spawn = None;
            decoy.summoned = true;
            decoy.illusion = true;
            decoy.elite = None;
            decoy.flags.clear();
            decoy.damage = 0.0;
            decoy.hp = 1.0;
            decoy.max_hp = 1.0;
            decoy.xp = 0;
            // The reflection stands across the hexagon from the original.
            let mirrored = v2(640.0 - decoy.pos.x, decoy.pos.y);
            if !physics::blocked(&self.site.grid, &[], mirrored, 6.0, Mobility::Ground) {
                decoy.pos = mirrored;
            }
            self.enemies.push(decoy);
        }
    }

    /// Nearest spot to `wanted` where the player does not overlap anything.
    fn safe_position(&self, wanted: V2) -> V2 {
        let obstacles = self.obstacles();
        let free = |p: V2| !physics::blocked(&self.site.grid, &obstacles, p, 5.0, Mobility::Ground);
        if free(wanted) {
            return wanted;
        }
        for ring in 1..=10 {
            for k in 0..16 {
                let p = wanted
                    + V2::from_angle(k as f32 * std::f32::consts::TAU / 16.0) * (ring as f32 * 6.0);
                if free(p) {
                    return p;
                }
            }
        }
        tile_center(nearest_walkable(
            &self.site.grid,
            Pos::from_world(wanted.x, wanted.y),
        ))
    }

    fn record_visit(&mut self) {
        let SiteKey::Walk { floor, q, r } = self.site.key else {
            return;
        };
        let delta = self.delta();
        let mut exits = self.site.exits;
        for (i, e) in exits.iter_mut().enumerate() {
            *e |= delta.opened_doors[i];
        }
        self.profile.world.deepest_floor = self.profile.world.deepest_floor.max(floor);
        self.profile.world.visited.insert(
            self.site.key.id(),
            VisitRecord {
                floor,
                q,
                r,
                biome: self.site.recipe.biome,
                stairs: self.site.stairs.is_some(),
                exits,
            },
        );
    }

    /// The first site of each biome always grants the same item.
    fn discover_biome(&mut self) {
        if self.site.key.is_home() || self.site.recipe.biome == 0 {
            return;
        }
        let biome = self.data.biomes[self.site.recipe.biome].clone();
        if !self.profile.world.biomes_found.insert(biome.id.clone()) {
            return;
        }
        let uid = self.profile.player.next_item_uid();
        let item =
            items::milestone_item(&self.data, uid, &biome.milestone_word, biome.milestone_slot);
        self.events.push(GameEvent::Banner {
            title: format!("Descubrimiento: {}", biome.name),
            subtitle: format!("Obtuviste {}", item.name),
            color: Rarity::Rare.color(),
        });
        self.profile.player.receive_item(item);
        self.refresh();
        self.grant_xp(25 + self.site.depth * 3);
    }

    pub(crate) fn open_chest(&mut self, index: usize) {
        let Some(chest) = self.chests.get_mut(index) else {
            return;
        };
        if chest.open {
            return;
        }
        chest.open = true;
        let (id, pos, reward) = (chest.id, chest.pos, chest.reward.clone());
        self.delta_mut().opened_chests.insert(id);
        self.sound(Sfx::ChestOpen);
        let uid = self.profile.player.next_item_uid();
        let item = match reward {
            ChestReward::Canon { canon, rarity } => {
                let text = self
                    .data
                    .canon_texts
                    .get(canon)
                    .cloned()
                    .unwrap_or_default();
                self.events.push(GameEvent::Banner {
                    title: "Frase canónica".into(),
                    subtitle: format!("«{text}»"),
                    color: rarity.color(),
                });
                items::from_canon(&self.data, uid, canon, rarity)
            }
            ChestReward::Compound {
                noun,
                quality,
                rarity,
            } => {
                let n = self.data.lexicon.get(noun).word.clone();
                let q = self.data.lexicon.get(quality).word.clone();
                items::from_words(&self.data, uid, &n, Some(&q), rarity)
            }
        };
        self.spawn_item_drop(pos + v2(0.0, 6.0), item);
        self.grant_xp(20 + self.site.depth * 4);
    }

    pub(crate) fn descend(&mut self) -> bool {
        if self.stairs_locked() {
            self.warn("Vencé al Guardián para bajar");
            return false;
        }
        let SiteKey::Walk { floor, q, r } = self.site.key else {
            return false;
        };
        self.sound(Sfx::Stairs);
        self.mark_tutorial(super::tutorial::Step::Stairs);
        self.enter(SiteKey::walk(floor + 1, q, r), Arrival::FromAbove);
        self.events.push(GameEvent::Banner {
            title: format!("Piso {}", floor + 1),
            subtitle: self.data.biomes[self.site.recipe.biome].name.clone(),
            color: [255, 240, 200],
        });
        true
    }

    pub(crate) fn ascend(&mut self) -> bool {
        let SiteKey::Walk { floor, q, r } = self.site.key else {
            return false;
        };
        if floor <= 1 {
            return false;
        }
        self.sound(Sfx::Stairs);
        self.enter(SiteKey::walk(floor - 1, q, r), Arrival::FromBelow);
        true
    }

    pub fn travel_home(&mut self) {
        self.sound(Sfx::Teleport);
        self.enter(SiteKey::walk(1, 0, 0), Arrival::Lamp);
    }

    /// Fast travel from home to a lit lamp.
    pub fn travel_to_lamp(&mut self, index: usize) -> bool {
        if self.body.dead > 0.0 {
            return false;
        }
        let Some(lamp) = self.profile.world.lamps.get(index).cloned() else {
            return false;
        };
        self.sound(Sfx::Teleport);
        self.enter(lamp.key, Arrival::Lamp);
        true
    }

    /// Oil cost of a return lamp here, after discounts and relics.
    pub fn lamp_cost(&self) -> Vec<(Resource, u32)> {
        if self.profile.player.has_relic("lampara_eterna") {
            return Vec::new();
        }
        self.data
            .recipes
            .iter()
            .find(|r| r.output == RecipeOutput::ReturnLamp)
            .map(|r| crafting::effective_cost(r, &self.derived))
            .unwrap_or_default()
    }

    pub fn light_return_lamp(&mut self) {
        if self.site.key.is_home() || self.site.key.is_remote() {
            self.warn("Aquí no hace falta una lámpara de retorno");
            return;
        }
        if self.delta().return_lamp.is_some() {
            self.warn("Este sitio ya tiene una lámpara encendida");
            return;
        }
        let cost = self.lamp_cost();
        if !self.profile.player.pay(&cost) {
            let need: Vec<String> = cost
                .iter()
                .map(|(r, n)| format!("{n} {}", r.name()))
                .collect();
            self.warn(format!("Para la lámpara necesitás {}", need.join(", ")));
            return;
        }
        let tile = Pos::from_world(self.body.pos.x, self.body.pos.y);
        let tile = if self.site.grid.at(tile) == Tile::Door {
            self.site.hub
        } else {
            tile
        };
        self.delta_mut().return_lamp = Some(tile);
        let label = format!(
            "Piso {} · {}",
            self.site.depth, self.data.biomes[self.site.recipe.biome].name
        );
        let record = LampRecord {
            key: self.site.key.clone(),
            pos: tile,
            label,
        };
        self.profile.world.lamps.push(record.clone());
        if self.profile.world.lamps.len() > MAX_LAMPS {
            let oldest = self.profile.world.lamps.remove(0);
            if let Some(delta) = self.profile.world.deltas.get_mut(&oldest.key.id()) {
                delta.return_lamp = None;
            }
            self.say(format!("Se apagó tu lámpara más antigua: {}", oldest.label));
        }
        self.profile.world.respawn = Some(record);
        self.sound(Sfx::LampLight);
        self.events.push(GameEvent::Particles {
            pos: tile_center(tile),
            z: 12.0,
            color: [160, 240, 255],
            count: 30,
            speed: 70.0,
        });
        self.say("Lámpara de retorno encendida: reaparecerás aquí");
        self.mark_tutorial(super::tutorial::Step::Lamp);
        if let Some(r) = self.data.recipe("return_lamp") {
            let xp = r.xp;
            self.grant_xp(xp);
        }
        self.events.push(GameEvent::Autosave);
    }

    pub fn catalog_unlocked(&self) -> bool {
        self.profile.player.has_relic(catalog::SEAL_RELIC)
    }

    /// Spends Indices and travels to the site inscribed with `phrase`.
    pub fn catalog_travel(&mut self, raw: &str) -> Result<(), String> {
        if self.body.dead > 0.0 {
            return Err("No podés viajar mientras caés".into());
        }
        if !self.catalog_unlocked() {
            return Err("El Catálogo sigue sellado".into());
        }
        if self.enemies.iter().any(|e| e.boss.is_some()) {
            return Err("No podés huir de un Guardián por el Catálogo".into());
        }
        let phrase = catalog::sanitize(raw);
        if phrase.is_empty() {
            return Err("Escribí una frase".into());
        }
        let cost = catalog::index_cost(&self.data, &phrase);
        if self.profile.player.consumable(Consumable::Index) < cost {
            return Err(format!("Necesitás {cost} Índices"));
        }
        let difficulty = self.site.depth + 2;
        let key = site::key_for_phrase(&self.feistel, &phrase, difficulty)
            .ok_or_else(|| "La frase no pertenece a la Biblioteca".to_string())?;
        if !self.profile.player.use_consumable(Consumable::Index, cost) {
            return Err(format!("Necesitás {cost} Índices"));
        }
        if !self.site.key.is_remote() {
            self.profile.world.remote_return = Some(self.site.key.clone());
        }
        self.sound(Sfx::Teleport);
        self.enter(key, Arrival::Hub);
        self.events.push(GameEvent::Banner {
            title: "El Catálogo responde".into(),
            subtitle: format!("«{phrase}»"),
            color: [180, 230, 255],
        });
        Ok(())
    }

    pub(crate) fn return_from_catalog(&mut self) -> bool {
        let key = self
            .profile
            .world
            .remote_return
            .take()
            .unwrap_or_else(|| SiteKey::walk(1, 0, 0));
        self.sound(Sfx::Teleport);
        self.enter(key, Arrival::Hub);
        true
    }

    pub(crate) fn die(&mut self) {
        self.profile.player.hp = 0.0;
        self.body.dead = 1.6;
        self.body.swing = None;
        // A fifth of the carried resources stays behind in a recoverable bag.
        let mut lost: BTreeMap<Resource, u32> = self
            .profile
            .world
            .death_bag
            .take()
            .map(|b| b.resources)
            .unwrap_or_default();
        for (r, n) in &mut self.profile.player.resources {
            let share = *n / 5 + u32::from(*n % 5 >= 3);
            if share > 0 {
                *n -= share;
                *lost.entry(*r).or_insert(0) += share;
            }
        }
        if !lost.is_empty() {
            self.profile.world.death_bag = Some(DeathBag {
                site: self.site.key.clone(),
                x: self.body.pos.x,
                y: self.body.pos.y,
                resources: lost,
            });
        }
        self.sound(Sfx::Death);
        self.events.push(GameEvent::Shake(6.0));
        self.events.push(GameEvent::PlayerDied);
    }

    /// Wakes up at the last lit lamp.
    pub fn respawn(&mut self) {
        self.refresh();
        self.profile.player.hp = self.derived.max_hp;
        self.profile.player.energy = self.derived.max_energy;
        self.body.status.clear();
        let target = self
            .profile
            .world
            .respawn
            .as_ref()
            .map_or_else(|| SiteKey::walk(1, 0, 0), |l| l.key.clone());
        // A remote site is a one-way trip: dying there ends the journey.
        self.profile.world.remote_return = None;
        self.enter(target, Arrival::Lamp);
    }

    pub(crate) fn check_death_bag(&mut self) {
        if self.body.dead > 0.0 {
            return;
        }
        let Some(bag) = &self.profile.world.death_bag else {
            return;
        };
        if bag.site.id() != self.site.key.id() || self.body.pos.dist(v2(bag.x, bag.y)) > 14.0 {
            return;
        }
        if let Some(bag) = self.profile.world.death_bag.take() {
            let total: u32 = bag.resources.values().sum();
            for (r, n) in bag.resources {
                self.profile.player.add_resource(r, n);
            }
            self.say(format!("Recuperaste tu bolsa ({total} materiales)"));
            self.sound(Sfx::ChestOpen);
        }
    }

    /// The home lamp, used by the HUD map.
    pub fn home_lamp_tile(&self) -> Pos {
        HOME_LAMP
    }
}
