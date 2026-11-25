//! Damage, defeat, loot and everything that flies or burns.

use super::Game;
use super::actors::{Drop, DropKind, Enemy, Projectile, ShotKind, Zone, ZoneKind, make_enemy};
use super::ai::{self, AiCtx, PlayerHit};
use super::bosses;
use super::events::{GameEvent, NumberKind, Sfx};
use super::physics;
use super::vec::{V2, v2};
use crate::babel::lexicon::{Category, WordId};
use crate::babel::rarity::Rarity;
use crate::babel::seeds::Dice;
use crate::data::{Consumable, ModFlag, Resource, Stat, Unlock};
use crate::progression::items::{self, Item};
use crate::world::grid::Pos;
use crate::world::site::EnemySpawn;

const MAX_MINIONS: usize = 6;

impl Game {
    pub(crate) fn update_enemies(&mut self, dt: f32) {
        let obstacles = self.obstacles();
        let mut enemies = std::mem::take(&mut self.enemies);
        let mut hits = Vec::new();
        let mut summons: Vec<(u32, V2)> = Vec::new();
        let mut darkness = 0.0f32;
        let mut burned: Vec<(u32, i32, NumberKind)> = Vec::new();
        {
            let data = self.data.clone();
            let mut ctx = AiCtx {
                grid: &self.site.grid,
                obstacles: &obstacles,
                player: self.body.pos,
                player_alive: self.body.dead <= 0.0,
                dt,
                depth: self.site.depth,
                projectiles: &mut self.projectiles,
                zones: &mut self.zones,
                hits: &mut hits,
                events: &mut self.events,
                dice: &mut self.dice,
            };
            for e in &mut enemies {
                let (burn, poison) = e.status.tick(dt);
                if burn > 0 {
                    burned.push((e.uid, burn, NumberKind::Burn));
                }
                if poison > 0 {
                    burned.push((e.uid, poison, NumberKind::Poison));
                }
                if e.has(ModFlag::Regenerates) && e.hp < e.max_hp {
                    e.hp = (e.hp + e.max_hp * 0.025 * dt).min(e.max_hp);
                }
                if e.has(ModFlag::BurnTrail) {
                    e.hurt_timer = e.hurt_timer.max(0.0);
                    e.orbit_trail(&mut ctx);
                }
                if e.boss.is_some() {
                    let out = bosses::update(&data, e, &mut ctx);
                    if out.summon > 0 {
                        summons.push((out.summon, e.pos));
                    }
                    darkness = darkness.max(out.darkness);
                } else {
                    ai::update(e, &mut ctx);
                }
            }
        }
        // Keep bodies from stacking into a single blob.
        for i in 0..enemies.len() {
            for j in i + 1..enemies.len() {
                let d = enemies[j].pos - enemies[i].pos;
                let min = (enemies[i].radius + enemies[j].radius) * 0.7;
                let len = d.len();
                if len < min && enemies[i].boss.is_none() && enemies[j].boss.is_none() {
                    let push = if len < 0.1 {
                        v2(0.5, 0.0)
                    } else {
                        d.norm() * ((min - len) * 0.25)
                    };
                    let mobility = enemies[i].mobility();
                    enemies[i].pos = physics::move_body(
                        &self.site.grid,
                        &obstacles,
                        enemies[i].pos,
                        push * -1.0,
                        4.0,
                        mobility,
                    )
                    .0;
                    let mobility = enemies[j].mobility();
                    enemies[j].pos = physics::move_body(
                        &self.site.grid,
                        &obstacles,
                        enemies[j].pos,
                        push,
                        4.0,
                        mobility,
                    )
                    .0;
                }
            }
        }
        self.enemies = enemies;
        self.darkness = self.darkness.max(darkness);
        for (uid, amount, kind) in burned {
            self.apply_damage_to_enemy(uid, amount as f32, kind);
        }
        for (count, near) in summons {
            self.summon_minions(count, near);
        }
        for hit in hits {
            self.hurt_player(hit);
        }
    }

    fn summon_minions(&mut self, count: u32, near: V2) {
        let Some(minion) = self
            .site
            .guardian
            .as_ref()
            .and_then(|g| self.data.enemies.guardians.get(g.index))
            .map(|g| self.data.archetype_index(&g.minion))
        else {
            return;
        };
        let alive = self.enemies.iter().filter(|e| e.summoned).count();
        for i in 0..(count as usize).min(MAX_MINIONS.saturating_sub(alive)) {
            let angle = self.dice.between(0.0, std::f32::consts::TAU) + i as f32;
            let mut pos = near + V2::from_angle(angle) * 34.0;
            if physics::blocked(&self.site.grid, &[], pos, 6.0, physics::Mobility::Ground) {
                pos = near;
            }
            let uid = self.new_uid();
            let spawn = EnemySpawn {
                id: uid,
                archetype: minion,
                word: None,
                modifiers: Vec::new(),
                elite: None,
                pos: Pos::from_world(pos.x, pos.y),
            };
            let mut e = make_enemy(&self.data, uid, self.site.depth, &spawn);
            e.spawn = None;
            e.summoned = true;
            e.pos = pos;
            e.hp *= 0.6;
            e.max_hp = e.hp;
            e.xp /= 3;
            e.grace = 0.9;
            self.events.push(GameEvent::Particles {
                pos,
                z: 8.0,
                color: [180, 120, 255],
                count: 14,
                speed: 60.0,
            });
            self.enemies.push(e);
        }
    }

    pub(crate) fn new_uid(&mut self) -> u32 {
        self.next_uid += 1;
        self.next_uid
    }

    /// A player attack landing on an enemy. Returns false when nothing was hit.
    pub fn strike_enemy(
        &mut self,
        uid: u32,
        base: f32,
        from: V2,
        knockback: f32,
        melee: bool,
    ) -> bool {
        let dim = self.ambient() < 0.45;
        let echo_chance = self.derived.stat(Stat::EchoChance);
        let dark_bonus = self.derived.stat(Stat::DarkDamage);
        let Some(i) = self.enemies.iter().position(|e| e.uid == uid) else {
            return false;
        };
        if !self.enemies[i].lit {
            // Shadow creatures cannot be hurt outside the light.
            return false;
        }
        let e = &mut self.enemies[i];
        e.hits_taken += 1;
        let pos = e.pos;
        if e.illusion {
            self.enemies.remove(i);
            self.floating(pos, "Era un reflejo", [200, 220, 255]);
            self.sound(Sfx::HarvestGlass);
            self.events.push(GameEvent::Particles {
                pos,
                z: 9.0,
                color: [220, 235, 255],
                count: 18,
                speed: 90.0,
            });
            return true;
        }
        if e.shield {
            e.shield = false;
            e.flash = 0.12;
            self.floating(pos, "¡Escudo!", [255, 250, 220]);
            self.sound(Sfx::HitArmor);
            return true;
        }
        if e.has(ModFlag::Dodges) && e.hits_taken.is_multiple_of(4) {
            self.floating(pos, "Esquivado", [140, 240, 220]);
            return false;
        }
        let armored = e.has(ModFlag::Armored);
        let thorns = e.has(ModFlag::Thorns);
        let contact = e.damage;
        let mut damage = base;
        if armored {
            damage *= 0.65;
        }
        if dim {
            damage *= 1.0 + dark_bonus;
        }
        let resist = if e.boss.is_some() {
            0.0
        } else if e.has(ModFlag::HeavyKnockback) || e.radius > 9.0 {
            0.35
        } else {
            1.0
        };
        e.knock = (e.pos - from).norm() * (knockback * resist);
        // A solid hit interrupts light enemies mid-windup.
        if knockback > 180.0 && resist >= 1.0 && matches!(e.ai, super::actors::Ai::Windup { .. }) {
            e.ai = super::actors::Ai::Recover { t: 0.4 };
        }
        let dps = (self.derived.damage * 0.28).max(2.0);
        let burn = self.derived.stat(Stat::BurnChance);
        let poison = self.derived.stat(Stat::PoisonChance);
        let slow = self.derived.stat(Stat::SlowChance);
        if self.dice.chance(burn) {
            self.enemies[i].status.ignite(dps, 3.0);
        }
        if self.dice.chance(poison) {
            self.enemies[i].status.envenom(dps * 0.7, 5.0);
        }
        if self.dice.chance(slow) {
            self.enemies[i].status.chill(2.5);
            self.sound(Sfx::Freeze);
        }
        let echo = self.dice.chance(echo_chance);
        self.sound(if armored { Sfx::HitArmor } else { Sfx::Hit });
        self.events.push(GameEvent::Particles {
            pos,
            z: 9.0,
            color: [255, 240, 200],
            count: 8,
            speed: 90.0,
        });
        let mut dealt = self.apply_damage_to_enemy(uid, damage, NumberKind::Enemy);
        if echo {
            dealt += self.apply_damage_to_enemy(uid, damage * 0.5, NumberKind::Echo);
        }
        let leech = self.derived.stat(Stat::Lifesteal).min(0.3);
        if leech > 0.0 {
            self.profile.player.hp =
                (self.profile.player.hp + dealt * leech).min(self.derived.max_hp);
        }
        if thorns && melee {
            let mut spikes = PlayerHit::plain(contact * 0.2, pos);
            spikes.knockback = 40.0;
            self.hurt_player(spikes);
        }
        true
    }

    /// Removes health, handling defeat. Returns the damage actually dealt.
    pub(crate) fn apply_damage_to_enemy(&mut self, uid: u32, amount: f32, kind: NumberKind) -> f32 {
        let Some(i) = self.enemies.iter().position(|e| e.uid == uid) else {
            return 0.0;
        };
        let e = &mut self.enemies[i];
        let amount = amount.max(1.0).round();
        let dealt = amount.min(e.hp);
        e.hp -= amount;
        if matches!(kind, NumberKind::Enemy | NumberKind::Echo) {
            e.flash = 0.12;
        }
        e.hurt_timer = 3.0;
        let offset = if kind == NumberKind::Echo { 7.0 } else { 0.0 };
        let pos = e.pos + v2(offset, -16.0 - e.radius);
        let dead = e.hp <= 0.0;
        if e.lit {
            self.events.push(GameEvent::Number {
                pos,
                amount: amount as i32,
                kind,
            });
        }
        if dead {
            let enemy = self.enemies.remove(i);
            self.defeat(enemy);
        }
        dealt
    }

    fn defeat(&mut self, e: Enemy) {
        let big = e.elite.is_some() || e.boss.is_some();
        self.sound(if big { Sfx::EliteDie } else { Sfx::EnemyDie });
        self.events.push(GameEvent::Particles {
            pos: e.pos,
            z: 8.0,
            color: e.tint.unwrap_or([240, 230, 210]),
            count: if big { 60 } else { 20 },
            speed: if big { 150.0 } else { 90.0 },
        });
        if big {
            self.events.push(GameEvent::Shake(5.0));
        }
        if e.boss.is_some() {
            self.defeat_guardian(&e);
            return;
        }
        self.grant_xp(e.xp);
        if !e.summoned {
            self.drop_loot(&e);
        }
        if let Some(id) = e.spawn {
            self.delta_mut().defeated_enemies.insert(id);
        }
        if e.has(ModFlag::Explodes) {
            self.zones.push(Zone::new(
                ZoneKind::Explosion,
                e.pos,
                30.0,
                0.8,
                0.2,
                e.damage * 1.2,
            ));
        }
        if e.has(ModFlag::SplitOnDeath) && !e.summoned {
            for side in [-1.0f32, 1.0] {
                let uid = self.new_uid();
                let mut child = e.clone();
                child.uid = uid;
                child.spawn = None;
                child.summoned = true;
                child.elite = None;
                child.flags.retain(|&f| f != ModFlag::SplitOnDeath);
                child.max_hp = e.max_hp * 0.3;
                child.hp = child.max_hp;
                child.damage *= 0.6;
                child.scale *= 0.65;
                child.radius = (child.radius * 0.65).max(3.5);
                child.xp = e.xp / 6;
                child.grace = 0.7;
                child.ai = super::actors::Ai::Chase;
                child.knock = v2(side * 140.0, 0.0);
                child.name = format!("Eco de {}", e.name);
                self.enemies.push(child);
            }
        }
        self.check_cleared();
    }

    fn check_cleared(&mut self) {
        if self.site.key.is_home() || self.delta().cleared || self.site.enemies.is_empty() {
            return;
        }
        if self.enemies.iter().any(|e| !e.summoned) {
            return;
        }
        self.delta_mut().cleared = true;
        self.profile.world.sites_cleared += 1;
        let bonus = 12 + self.site.depth * 5;
        self.events.push(GameEvent::Banner {
            title: "Sitio despejado".into(),
            subtitle: format!("+{bonus} de experiencia"),
            color: [140, 240, 150],
        });
        self.sound(Sfx::Victory);
        self.grant_xp(bonus);
    }

    fn defeat_guardian(&mut self, e: &Enemy) {
        let Some(boss) = e.boss.as_ref() else {
            return;
        };
        let Some(def) = self.data.enemies.guardians.get(boss.index).cloned() else {
            return;
        };
        let floor = self.site.depth;
        self.profile.world.guardians_defeated.insert(floor);
        self.delta_mut().cleared = true;
        // Minions and their attacks die with their master.
        self.enemies.retain(|m| !m.summoned);
        self.projectiles.retain(|p| p.from_player);
        self.zones.clear();
        self.darkness = 0.0;
        // Each designed guardian grants its fixed relic; infinite ones grant fragments.
        let first_time = !self.profile.player.has_relic(&def.relic);
        let relic_id = if floor <= def.floor && first_time {
            self.profile.player.relics.push(def.relic.clone());
            def.relic.clone()
        } else {
            self.profile.player.infinite_fragments += 1;
            "fragmento_infinito".to_string()
        };
        let relic = self
            .data
            .items
            .relics
            .iter()
            .find(|r| r.id == relic_id)
            .cloned();
        if let Some(r) = relic {
            self.events.push(GameEvent::Banner {
                title: r.name,
                subtitle: r.description,
                color: [255, 196, 56],
            });
        }
        self.events.push(GameEvent::Flash(Rarity::Legendary));
        self.sound(Sfx::LegendaryFanfare);
        // Guardians always drop an item made from their rarest word.
        if let Some((word, rarity)) = e.word {
            let text = self.data.lexicon.get(word).word.clone();
            let uid = self.profile.player.next_item_uid();
            let item = items::from_words(&self.data, uid, &text, None, rarity.max(Rarity::Epic));
            self.spawn_item_drop(e.pos, item);
        }
        for r in [Resource::Ink, Resource::Paper, Resource::Oil] {
            self.spawn_resource_drop(e.pos, r, 6 + floor / 3);
        }
        self.refresh();
        self.profile.player.hp = self.derived.max_hp;
        self.grant_xp(e.xp);
        if floor == crate::world::floors::FINAL_FLOOR && !self.profile.world.finished {
            self.profile.world.finished = true;
            self.events.push(GameEvent::Ending);
        }
        self.events.push(GameEvent::Autosave);
    }

    /// Words the enemy can leave behind, most personal first.
    fn loot_word(&mut self, e: &Enemy, dice: &mut Dice) -> Option<(String, Rarity)> {
        let lexicon = &self.data.lexicon;
        let pool: Vec<(WordId, Rarity)> = self
            .site
            .analysis
            .distinct()
            .into_iter()
            .filter(|&(w, _)| items::can_name_item(&self.data, w))
            .collect();
        let objects: Vec<(WordId, Rarity)> = pool
            .iter()
            .copied()
            .filter(|&(w, _)| lexicon.get(w).category == Category::Objeto)
            .collect();
        let choice = if !objects.is_empty() && dice.chance(0.5) {
            dice.pick(&objects).copied()
        } else if let Some(own) = e.word.filter(|_| dice.chance(0.5)) {
            Some(own)
        } else {
            dice.pick(&pool).copied().or(e.word)
        };
        let (word, rarity) = choice.or_else(|| {
            // A silent book still has its creature: name the loot after it.
            let id = lexicon.in_category(Category::Criatura);
            let name = &self.data.enemies.archetypes[e.archetype].id;
            let matching: Vec<WordId> = id
                .iter()
                .copied()
                .filter(|&w| lexicon.get(w).tag_value("arq") == Some(name.as_str()))
                .collect();
            dice.pick(&matching).map(|&w| {
                let entry = lexicon.get(w);
                let ctx = crate::babel::rarity::Context {
                    depth: self.site.depth,
                    ..Default::default()
                };
                (
                    w,
                    crate::babel::rarity::rarity_of(entry.base_rarity, &entry.word, ctx),
                )
            })
        })?;
        Some((lexicon.get(word).word.clone(), rarity))
    }

    fn drop_loot(&mut self, e: &Enemy) {
        let def = self.data.enemies.archetypes[e.archetype].clone();
        let label = format!("drop-{}", e.spawn.unwrap_or(e.uid));
        let mut dice = Dice::labelled(&self.site.seed, &label);
        let copies = if e.has(ModFlag::ExtraLoot) { 2 } else { 1 };
        for _ in 0..copies {
            if dice.chance(def.material_chance) || e.elite.is_some() {
                let amount = 1 + dice.below(2) + u32::from(e.elite.is_some()) * 2;
                self.spawn_resource_drop(e.pos, def.material, amount);
            }
        }
        // A second material ties the drop to the biome.
        let biome = &self.data.biomes[self.site.recipe.biome];
        if dice.chance(0.28)
            && let Some(&(node, _)) = biome.nodes.first()
            && let Some(&(resource, _, _)) = self.data.node(node).yields.first()
        {
            let node_def = self.data.node(node);
            if self.profile.player.tool_tier(node_def.tool) >= node_def.min_tier {
                self.spawn_resource_drop(e.pos, resource, 1);
            }
        }
        if dice.chance(0.06) {
            self.spawn_drop(e.pos, DropKind::Consumable(Consumable::Tonic, 1));
        }
        if let Some(elite) = e.elite.clone() {
            let noun = self.data.lexicon.get(elite.noun).word.clone();
            let quality = self.data.lexicon.get(elite.quality).word.clone();
            let uid = self.profile.player.next_item_uid();
            let item = items::from_words(&self.data, uid, &noun, Some(&quality), elite.rarity);
            self.spawn_item_drop(e.pos, item);
        } else if dice.chance(def.word_drop_chance)
            && let Some((word, rarity)) = self.loot_word(e, &mut dice)
        {
            // A modifier word gives the item its surname ("Daga ardiente").
            let quality = e
                .modifiers
                .first()
                .and_then(|&m| self.data.enemies.modifiers.get(m))
                .map(|m| m.name.clone())
                .filter(|q| *q != word);
            let uid = self.profile.player.next_item_uid();
            let item = items::from_words(&self.data, uid, &word, quality.as_deref(), rarity);
            self.spawn_item_drop(e.pos, item);
        }
    }

    pub(crate) fn spawn_drop(&mut self, pos: V2, kind: DropKind) {
        let angle = self.dice.between(0.0, std::f32::consts::TAU);
        let speed = self.dice.between(25.0, 65.0);
        let vz = self.dice.between(70.0, 120.0);
        self.drops.push(Drop {
            pos,
            vel: V2::from_angle(angle) * speed,
            z: 6.0,
            vz,
            age: 0.0,
            kind,
            magnet: false,
        });
    }

    pub(crate) fn spawn_resource_drop(&mut self, pos: V2, resource: Resource, amount: u32) {
        if amount == 0 {
            return;
        }
        // Several small pieces read better than one big one.
        let pieces = amount.min(4);
        let mut left = amount;
        for i in 0..pieces {
            let share = left.div_ceil(pieces - i);
            left -= share;
            self.spawn_drop(pos, DropKind::Resource(resource, share));
        }
    }

    pub(crate) fn spawn_item_drop(&mut self, pos: V2, item: Item) {
        let sound = match item.rarity {
            Rarity::Common | Rarity::Uncommon => Sfx::DropCommon,
            Rarity::Rare => Sfx::DropRare,
            Rarity::Epic => Sfx::DropEpic,
            Rarity::Legendary => Sfx::DropLegendary,
            Rarity::Canonical => Sfx::DropCanonical,
        };
        self.sound(sound);
        if item.rarity >= Rarity::Legendary {
            self.events.push(GameEvent::Flash(item.rarity));
        }
        self.profile.world.unclaimed.push(item.clone());
        self.spawn_drop(pos, DropKind::Item(Box::new(item)));
    }

    pub(crate) fn update_drops(&mut self, dt: f32) {
        let player = self.body.pos;
        let wide = if self.derived.has(Unlock::WideMagnet) {
            1.3
        } else {
            1.0
        };
        let range = 30.0 * wide * (1.0 + self.derived.stat(Stat::Magnet));
        let mut picked = Vec::new();
        for (i, d) in self.drops.iter_mut().enumerate() {
            d.age += dt;
            let dist = player.dist(d.pos);
            if d.age > 0.35 && dist < range {
                d.magnet = true;
            }
            if d.magnet {
                let pull = (player - d.pos).norm();
                let speed = 90.0 + d.age * 140.0;
                d.pos += pull * (speed.min(320.0) * dt);
                d.z = (d.z - dt * 40.0).max(4.0);
                if player.dist(d.pos) < 8.0 {
                    picked.push(i);
                }
            } else {
                let step = d.vel * dt;
                let moved = physics::move_body(
                    &self.site.grid,
                    &[],
                    d.pos,
                    step,
                    2.0,
                    physics::Mobility::Ground,
                );
                d.pos = moved.0;
                d.vz -= 420.0 * dt;
                d.z += d.vz * dt;
                if d.z <= 0.0 {
                    d.z = 0.0;
                    // Bounce with damping until it settles.
                    d.vz = if d.vz < -40.0 { -d.vz * 0.45 } else { 0.0 };
                    d.vel = d.vel * 0.6;
                }
            }
        }
        for i in picked.into_iter().rev() {
            let drop = self.drops.remove(i);
            self.pick_up(drop);
        }
    }

    pub(crate) fn pick_up(&mut self, drop: Drop) {
        let pos = self.body.pos + v2(0.0, -20.0);
        match drop.kind {
            DropKind::Resource(r, n) => {
                self.profile.player.add_resource(r, n);
                self.floating(pos, format!("+{n} {}", r.name()), r.color());
                self.sound(Sfx::Pickup);
            }
            DropKind::Consumable(c, n) => {
                self.profile.player.add_consumable(c, n);
                self.floating(pos, format!("+{n} {}", c.name()), [255, 255, 255]);
                self.sound(Sfx::Pickup);
            }
            DropKind::Item(item) => {
                let color = item.rarity.color();
                self.floating(pos, item.name.clone(), color);
                self.say(format!("{} ({})", item.name, item.rarity.label()));
                self.profile.world.unclaimed.retain(|i| i.uid != item.uid);
                self.profile.player.receive_item(*item);
                self.sound(Sfx::Pickup);
                self.refresh();
            }
        }
    }

    pub(crate) fn floating(&mut self, pos: V2, text: impl Into<String>, color: [u8; 3]) {
        self.events.push(GameEvent::Floating {
            pos,
            text: text.into(),
            color,
        });
    }

    pub fn grant_xp(&mut self, amount: u32) {
        if amount == 0 {
            return;
        }
        let bonus = 1.0 + self.derived.stat(Stat::XpBonus);
        let total = (amount as f32 * bonus).round() as u32;
        let levels = self.profile.player.gain_xp(total);
        if levels > 0 {
            let level = self.profile.player.level;
            self.sound(Sfx::LevelUp);
            self.events.push(GameEvent::LevelUp(level));
            self.events.push(GameEvent::Particles {
                pos: self.body.pos,
                z: 10.0,
                color: [255, 230, 110],
                count: 50,
                speed: 120.0,
            });
            self.refresh();
            self.profile.player.hp = self.derived.max_hp;
        }
    }

    pub(crate) fn lose_hp(&mut self, amount: f32) {
        if self.body.dead > 0.0 {
            return;
        }
        self.profile.player.hp -= amount;
        if self.profile.player.hp > 0.0 {
            return;
        }
        if self.derived.has(Unlock::SecondWind) && !self.body.second_wind_used {
            self.body.second_wind_used = true;
            self.profile.player.hp = self.derived.max_hp * 0.25;
            self.body.invulnerable = 1.6;
            self.floating(self.body.pos, "¡Segundo aliento!", [255, 230, 110]);
            self.sound(Sfx::Heal);
            return;
        }
        self.die();
    }

    pub fn hurt_player(&mut self, hit: PlayerHit) {
        if hit.damage <= 0.0 || self.body.invulnerable > 0.0 || self.body.dead > 0.0 {
            return;
        }
        let damage = (hit.damage * 22.0 / (22.0 + self.derived.defense.max(0.0)))
            .max(1.0)
            .round();
        let scale = 1.0 + self.site.depth as f32 * 0.12;
        if hit.burn {
            self.body.status.ignite(2.0 * scale, 2.5);
            self.sound(Sfx::Burn);
        }
        if hit.poison {
            self.body.status.envenom(1.5 * scale, 4.0);
            self.sound(Sfx::Poison);
        }
        if hit.slow {
            self.body.status.chill(2.2);
            self.sound(Sfx::Freeze);
        }
        if hit.drain {
            self.profile.player.energy = (self.profile.player.energy - 25.0).max(0.0);
        }
        self.body.knock = (self.body.pos - hit.from).norm() * hit.knockback;
        self.body.invulnerable = 0.6 * (1.0 + self.derived.stat(Stat::IFrames));
        self.body.flash = 0.14;
        self.body.charge = 0.0;
        self.hitstop = self.hitstop.max(0.05);
        self.events.push(GameEvent::Shake(3.0));
        self.events.push(GameEvent::Number {
            pos: self.body.pos + v2(0.0, -20.0),
            amount: damage as i32,
            kind: NumberKind::Player,
        });
        self.sound(Sfx::Hurt);
        if let Some(source) = hit.source
            && let Some(e) = self.enemies.iter_mut().find(|e| e.uid == source)
            && e.has(ModFlag::Lifesteal)
        {
            e.hp = (e.hp + damage * 1.5).min(e.max_hp);
        }
        self.lose_hp(damage);
    }

    pub(crate) fn update_projectiles(&mut self, dt: f32) {
        let mut shots = std::mem::take(&mut self.projectiles);
        let mut strikes: Vec<(u32, f32, V2)> = Vec::new();
        let mut hurts = Vec::new();
        shots.retain_mut(|p: &mut Projectile| {
            p.ttl -= dt;
            p.pos += p.vel * dt;
            if p.ttl <= 0.0 || !physics::tile_at(&self.site.grid, p.pos).flyable() {
                self.events.push(GameEvent::Particles {
                    pos: p.pos,
                    z: p.z,
                    color: [230, 220, 200],
                    count: 3,
                    speed: 30.0,
                });
                return false;
            }
            if p.from_player {
                let target = self
                    .enemies
                    .iter()
                    .find(|e| e.lit && e.pos.dist(p.pos) < e.radius + p.radius);
                if let Some(e) = target {
                    strikes.push((e.uid, p.damage, p.pos - p.vel * 0.05));
                    return false;
                }
                // Pages can also chip at resources.
                true
            } else if self.body.pos.dist(p.pos) < p.radius + 5.0 && self.body.invulnerable <= 0.0 {
                let mut hit = PlayerHit::plain(p.damage, p.pos - p.vel * 0.1);
                hit.knockback = 60.0;
                hit.burn = p.burn || p.kind == ShotKind::Flame;
                hit.slow = p.slow;
                hit.poison = p.poison;
                hurts.push(hit);
                false
            } else {
                true
            }
        });
        self.projectiles = shots;
        for (uid, damage, from) in strikes {
            self.strike_enemy(uid, damage, from, 60.0, false);
        }
        for h in hurts {
            self.hurt_player(h);
        }
    }

    pub(crate) fn update_zones(&mut self, dt: f32) {
        let mut zones = std::mem::take(&mut self.zones);
        let mut hurts = Vec::new();
        zones.retain_mut(|z| {
            if z.delay > 0.0 {
                z.delay -= dt;
                if z.delay <= 0.0 && z.kind != ZoneKind::Fire {
                    self.events.push(GameEvent::Particles {
                        pos: z.pos,
                        z: 2.0,
                        color: if z.kind == ZoneKind::Explosion {
                            [255, 170, 60]
                        } else {
                            [210, 190, 230]
                        },
                        count: 26,
                        speed: 110.0,
                    });
                    self.events.push(GameEvent::Sound(Sfx::Explosion));
                    self.events.push(GameEvent::Shake(2.0));
                }
                return true;
            }
            z.ttl -= dt;
            z.tick -= dt;
            let inside = self.body.pos.dist(z.pos) < z.radius + 3.0;
            if inside && z.tick <= 0.0 && !(z.spent && z.kind != ZoneKind::Fire) {
                let mut hit = PlayerHit::plain(z.damage, z.pos);
                hit.burn = z.kind == ZoneKind::Fire;
                hit.knockback = if z.kind == ZoneKind::Fire {
                    30.0
                } else {
                    150.0
                };
                hurts.push(hit);
                z.tick = 0.55;
                z.spent = true;
            }
            z.ttl > 0.0
        });
        // Zones created while updating (explosions) are appended after.
        zones.append(&mut self.zones);
        self.zones = zones;
        for h in hurts {
            self.hurt_player(h);
        }
    }
}

impl Enemy {
    /// Burning creatures leave short-lived flames behind.
    pub(crate) fn orbit_trail(&mut self, ctx: &mut AiCtx) {
        let phase = (self.anim / 0.55) as u32;
        let before = ((self.anim - ctx.dt) / 0.55) as u32;
        if phase != before && self.grace <= 0.0 {
            ctx.zones.push(Zone::new(
                ZoneKind::Fire,
                self.pos,
                7.0,
                0.25,
                1.3,
                self.damage * 0.25,
            ));
        }
    }
}
