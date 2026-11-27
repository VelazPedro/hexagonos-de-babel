//! The whole running session as a deterministic-friendly, engine-free simulation.
//! The Bevy shell feeds it [`events::Input`] and presents its events and state.

pub mod actions;
pub mod actors;
pub mod ai;
pub mod bosses;
pub mod combat;
pub mod coop;
pub mod events;
pub mod light;
pub mod pet;
#[cfg(test)]
mod pet_tests;
pub mod physics;
pub mod player;
pub mod profile;
pub mod sync;
pub mod travel;
pub mod tutorial;
pub mod vec;

#[cfg(test)]
mod coop_tests;
#[cfg(test)]
mod records_tests;
#[cfg(test)]
mod sync_tests;
#[cfg(test)]
mod tests;
#[cfg(test)]
mod upgrade_tests;

use crate::babel::feistel::Feistel;
use crate::babel::seeds::{Dice, stable_hash};
use crate::data::{GameData, ModFlag, Perk, ToolKind};
use crate::progression::records::Tally;
use crate::progression::stats::{self, Derived};
use crate::world::deltas::SiteDelta;
use crate::world::grid::{Pos, TILE};
use crate::world::site::{HOME_LAMP, HOME_PLOTS, Site};
use actors::{ChestState, Drop, Enemy, NodeState, Projectile, ShotKind, Zone, ZoneKind};
use events::{GameEvent, Input, Sfx};
use light::Light;
use physics::Obstacle;
use player::Body;
use profile::Profile;
use std::sync::Arc;
use vec::{V2, v2};

/// How much farther the lamp reaches when held.
pub(crate) const LAMP_IN_HAND_LIGHT: f32 = 1.22;

pub struct Game {
    pub data: Arc<GameData>,
    pub profile: Profile,
    pub site: Site,
    pub(crate) feistel: Feistel,
    pub derived: Derived,
    pub body: Body,
    pub enemies: Vec<Enemy>,
    pub nodes: Vec<NodeState>,
    pub chests: Vec<ChestState>,
    pub drops: Vec<Drop>,
    pub projectiles: Vec<Projectile>,
    pub zones: Vec<Zone>,
    pub events: Vec<GameEvent>,
    /// Lights of the current frame (player, fixtures, glows).
    pub lights: Vec<Light>,
    pub time: f32,
    pub hitstop: f32,
    /// Seconds of boss-induced darkness left.
    pub darkness: f32,
    /// Time played that is not yet written in the records.
    pub(crate) unrecorded_time: f32,
    /// Time gathered until burning torches deal their next second of damage.
    pub(crate) torch_clock: f32,
    /// Presses made while the world was frozen by hitstop.
    pub(crate) buffered: Input,
    pub(crate) dice: Dice,
    pub(crate) next_uid: u32,
    pub(crate) message_cooldown: f32,
    /// The second player, in co-op. See [`coop`].
    pub(crate) companion: Option<Box<coop::Companion>>,
    /// Whose data is in `body`, `profile.player` and `derived`.
    pub(crate) slot: u8,
    /// What the guest has to be shown.
    pub(crate) partner_events: Vec<GameEvent>,
    /// How many of `events` have been sorted between the two players.
    pub(crate) routed: usize,
    /// Counts down to waking up after both fell.
    pub(crate) wipe: f32,
    /// This is the guest's copy: it is drawn, never stepped. See [`sync`].
    pub(crate) mirrored: bool,
    /// What the guest did in menus, for the host to repeat.
    pub(crate) outbox: Vec<sync::Action>,
}

impl Game {
    /// Opens a session on `profile`, loading its current site.
    pub fn new(data: Arc<GameData>, mut profile: Profile) -> Game {
        profile.begin_records();
        let feistel = Feistel::new(profile.world_seed);
        let site = crate::world::site::generate_with(
            &data,
            &feistel,
            profile.world_seed,
            &profile.world.current,
        );
        let derived = stats::derive(&data, &profile.player);
        let position = v2(profile.world.position.0, profile.world.position.1);
        let dice = Dice::from_u64(profile.world_seed ^ stable_hash("runtime"));
        let mut game = Game {
            data,
            site,
            feistel,
            derived,
            body: Body::new(position),
            profile,
            enemies: Vec::new(),
            nodes: Vec::new(),
            chests: Vec::new(),
            drops: Vec::new(),
            projectiles: Vec::new(),
            zones: Vec::new(),
            events: Vec::new(),
            lights: Vec::new(),
            time: 0.0,
            hitstop: 0.0,
            darkness: 0.0,
            unrecorded_time: 0.0,
            torch_clock: 0.0,
            buffered: Input::default(),
            dice,
            next_uid: 1,
            message_cooldown: 0.0,
            companion: None,
            slot: coop::HOST,
            partner_events: Vec::new(),
            routed: 0,
            wipe: 0.0,
            mirrored: false,
            outbox: Vec::new(),
        };
        let key = game.profile.world.current.clone();
        game.enter(key, travel::Arrival::At(position));
        game
    }

    /// Written a few seconds at a time, so that a partner is not sent their
    /// data again on every step.
    fn record_time(&mut self, dt: f32) {
        const STRETCH: u64 = 5;
        self.unrecorded_time += dt;
        if self.unrecorded_time < STRETCH as f32 {
            return;
        }
        self.unrecorded_time -= STRETCH as f32;
        self.count_for_all(|t| t.play_seconds += STRETCH);
    }

    /// Counts something both players took part in.
    pub(crate) fn count_for_all(&mut self, count: impl Fn(&mut Tally)) {
        count(&mut self.profile.player.records.tally);
        if let Some(other) = self.companion.as_deref_mut() {
            count(&mut other.player.records.tally);
        }
    }

    /// Counts something only the acting player did.
    pub(crate) fn count(&mut self, count: impl FnOnce(&mut Tally)) {
        count(&mut self.profile.player.records.tally);
    }

    /// Recomputes derived stats after any change to equipment or skills.
    pub fn refresh(&mut self) {
        self.derived = stats::derive(&self.data, &self.profile.player);
        let p = &mut self.profile.player;
        p.hp = p.hp.min(self.derived.max_hp);
        p.energy = p.energy.min(self.derived.max_energy);
    }

    pub fn delta(&self) -> SiteDelta {
        self.profile
            .world
            .deltas
            .get(&self.site.key.id())
            .cloned()
            .unwrap_or_default()
    }

    pub(crate) fn delta_mut(&mut self) -> &mut SiteDelta {
        self.profile
            .world
            .deltas
            .entry(self.site.key.id())
            .or_default()
    }

    pub fn drain_events(&mut self) -> Vec<GameEvent> {
        self.flush_events();
        self.routed = 0;
        std::mem::take(&mut self.events)
    }

    pub(crate) fn sound(&mut self, s: Sfx) {
        self.events.push(GameEvent::Sound(s));
    }

    pub(crate) fn say(&mut self, text: impl Into<String>) {
        self.events.push(GameEvent::Message(text.into()));
    }

    /// A message that will not spam while a button is held.
    pub(crate) fn warn(&mut self, text: impl Into<String>) {
        if self.message_cooldown <= 0.0 {
            self.message_cooldown = 1.4;
            self.sound(Sfx::Locked);
            self.say(text);
        }
    }

    pub(crate) fn obstacles(&self) -> Vec<Obstacle> {
        let mut out: Vec<Obstacle> = self
            .nodes
            .iter()
            .map(|n| Obstacle {
                pos: n.pos,
                radius: 6.0,
            })
            .chain(self.chests.iter().map(|c| Obstacle {
                pos: c.pos,
                radius: 6.0,
            }))
            .collect();
        if self.site.key.is_home() {
            for &(_, plot) in &self.profile.world.stations {
                if let Some(&p) = HOME_PLOTS.get(plot as usize) {
                    out.push(Obstacle {
                        pos: tile_center(p),
                        radius: 8.0,
                    });
                }
            }
        }
        out
    }

    /// The return lamp of the current site, if lit.
    pub fn lamp_here(&self) -> Option<V2> {
        if self.site.key.is_home() {
            return Some(tile_center(HOME_LAMP));
        }
        self.delta().return_lamp.map(tile_center)
    }

    pub fn ambient(&self) -> f32 {
        let base = self.site.recipe.ambient;
        if self.darkness > 0.0 {
            (base * 0.35).max(0.08)
        } else {
            base
        }
    }

    /// Whether the stairs here are barred by an undefeated guardian.
    pub fn stairs_locked(&self) -> bool {
        self.site.guardian.is_some()
            && !self
                .profile
                .world
                .guardians_defeated
                .contains(&self.site.depth)
    }

    pub(crate) fn collect_lights(&mut self) {
        let mut lights = Vec::new();
        let flame = [1.0, 0.72, 0.42];
        for slot in 0..self.players() {
            let Some((body, player, derived)) = self.view(slot) else {
                continue;
            };
            if body.dead <= 0.0 {
                let mut radius = derived.light_radius * TILE;
                if player.active_tool == ToolKind::Lamp {
                    radius *= LAMP_IN_HAND_LIGHT;
                }
                lights.push(Light {
                    pos: body.pos + v2(0.0, -4.0),
                    radius,
                    color: [1.05, 0.86, 0.62],
                    flicker: 0.18,
                    reveals: true,
                    priority: 10,
                });
            }
        }
        let dim = if self.darkness > 0.0 { 0.35 } else { 1.0 };
        for &l in &self.site.lamps {
            let (x, y) = l.center();
            lights.push(Light {
                pos: v2(x, y),
                radius: 5.4 * TILE,
                color: [flame[0] * dim, flame[1] * dim, flame[2] * dim],
                flicker: 0.35,
                reveals: self.darkness <= 0.0,
                priority: 6,
            });
        }
        let torch_radius = 4.6 * TILE * (1.0 + self.party_perk(Perk::TorchReach).0);
        let delta = self.delta();
        for &t in &delta.torches {
            let (x, y) = t.center();
            lights.push(Light {
                pos: v2(x, y),
                radius: torch_radius,
                color: [1.1, 0.68, 0.32],
                flicker: 0.6,
                reveals: true,
                priority: 7,
            });
        }
        if let Some(p) = self.lamp_here() {
            lights.push(Light {
                pos: p,
                radius: 6.0 * TILE,
                color: [0.65, 0.95, 1.05],
                flicker: 0.12,
                reveals: true,
                priority: 8,
            });
        }
        if let Some(s) = self.site.stairs {
            lights.push(Light {
                pos: tile_center(s),
                radius: 3.0 * TILE,
                color: [0.55, 0.45, 0.95],
                flicker: 0.1,
                reveals: false,
                priority: 4,
            });
        }
        for p in &self.projectiles {
            if matches!(p.kind, ShotKind::Flame | ShotKind::Spark) {
                lights.push(Light {
                    pos: p.pos,
                    radius: 2.4 * TILE,
                    color: if p.kind == ShotKind::Spark {
                        [0.4, 0.85, 1.0]
                    } else {
                        [1.0, 0.55, 0.2]
                    },
                    flicker: 0.5,
                    reveals: true,
                    priority: 5,
                });
            }
        }
        for z in &self.zones {
            if z.kind == ZoneKind::Fire && z.delay <= 0.0 {
                lights.push(Light {
                    pos: z.pos,
                    radius: z.radius + 2.0 * TILE,
                    color: [0.95, 0.45, 0.14],
                    flicker: 0.8,
                    reveals: true,
                    priority: 3,
                });
            }
        }
        for d in &self.drops {
            let rarity = d.rarity();
            if rarity >= crate::babel::rarity::Rarity::Rare {
                let c = rarity.color();
                lights.push(Light {
                    pos: d.pos,
                    radius: 2.2 * TILE,
                    color: [
                        f32::from(c[0]) / 255.0,
                        f32::from(c[1]) / 255.0,
                        f32::from(c[2]) / 255.0,
                    ],
                    flicker: 0.15,
                    reveals: false,
                    priority: 4,
                });
            }
        }
        for e in &self.enemies {
            let glows = e.has(ModFlag::Glows) || self.data.enemies.archetypes[e.archetype].glows;
            if glows && !e.has(ModFlag::DarkOnly) {
                let c = e.tint.unwrap_or([150, 190, 255]);
                lights.push(Light {
                    pos: e.pos,
                    radius: 2.6 * TILE,
                    color: [
                        f32::from(c[0]) / 300.0,
                        f32::from(c[1]) / 300.0,
                        f32::from(c[2]) / 300.0,
                    ],
                    flicker: 0.2,
                    reveals: false,
                    priority: 2,
                });
            }
        }
        self.lights = lights;
    }

    fn update_visibility(&mut self) {
        let bright = self.ambient() >= 0.85;
        let grid = &self.site.grid;
        let lights = &self.lights;
        for e in &mut self.enemies {
            e.lit = !e.needs_light() || bright || light::is_lit(grid, lights, e.pos);
        }
    }

    /// Advances the simulation for a single player. Presentation effects are
    /// queued in `events`.
    pub fn update(&mut self, dt: f32, input: &Input) {
        self.step(dt, input, &Input::default());
    }

    /// Advances the simulation. `partner` is ignored without a companion.
    pub fn step(&mut self, dt: f32, input: &Input, partner: &Input) {
        let dt = dt.clamp(0.0, 0.05);
        self.profile.world.play_seconds += f64::from(dt);
        self.record_time(dt);
        self.message_cooldown = (self.message_cooldown - dt).max(0.0);
        if self.hitstop > 0.0 {
            self.hitstop -= dt;
            self.buffered.absorb_presses(input);
            if let Some(c) = self.companion.as_deref_mut() {
                c.buffered.absorb_presses(partner);
            }
            return;
        }
        let mut inputs = [*input, *partner];
        inputs[0].absorb_presses(&std::mem::take(&mut self.buffered));
        if let Some(c) = self.companion.as_deref_mut() {
            inputs[1].absorb_presses(&std::mem::take(&mut c.buffered));
        }
        self.time += dt;
        self.darkness = (self.darkness - dt).max(0.0);
        self.collect_lights();
        self.update_visibility();
        // Alone, the world waits while the player falls.
        if !self.is_coop() && self.body.dead > 0.0 {
            self.body.dead = (self.body.dead - dt).max(0.0001);
            return;
        }
        for slot in 0..self.players() {
            let input = inputs[usize::from(slot)];
            let travelled = self.as_player(slot, |g| {
                if g.body.dead > 0.0 {
                    g.body.dead = (g.body.dead - dt).max(0.0001);
                    return false;
                }
                g.update_player(dt, &input)
            });
            if travelled == Some(true) {
                // The player travelled: the new site starts next step.
                self.flush_events();
                return;
            }
        }
        if self.resolve_exit() || self.update_downed(dt) {
            self.flush_events();
            return;
        }
        self.update_enemies(dt);
        self.update_torches(dt);
        for slot in 0..self.players() {
            self.as_player(slot, |g| g.update_pet(dt));
        }
        self.update_projectiles(dt);
        self.update_zones(dt);
        self.update_drops(dt);
        for n in &mut self.nodes {
            n.shake = (n.shake - dt).max(0.0);
        }
        self.update_tutorial(&inputs[0]);
        let pos = self.body.pos;
        self.profile.world.position = (pos.x, pos.y);
        self.flush_events();
    }
}

pub fn tile_center(p: Pos) -> V2 {
    let (x, y) = p.center();
    v2(x, y)
}
