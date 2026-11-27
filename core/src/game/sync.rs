//! What travels between two computers to play together: the host's picture of
//! the current gallery, and what the guest does in menus. The host simulates;
//! the guest keeps a *mirror* of the game that is only drawn, never stepped.
//!
//! Nothing that arrives is trusted: states are rejected and events, controls
//! and actions are brought within limits, so that no message can break the
//! game on either side.

use super::Game;
use super::actors::{Ai, ChestState, Drop, DropKind, Enemy, NodeState, Projectile, Zone};
use super::coop::{Companion, GUEST};
use super::events::{GameEvent, Input, ScreenRequest};
use super::pet::PetBody;
use super::player::Body;
use super::profile::{Profile, SAVE_VERSION, WorldData};
use super::vec::V2;
use crate::data::{GameData, Slot};
use crate::progression::inscribe::Target;
use crate::progression::look::Look;
use crate::progression::pet::PetLook;
use crate::progression::player::PlayerData;
use crate::progression::records::Tally;
use crate::progression::stats;
use crate::world::deltas::SiteDelta;
use crate::world::grid::Pos;
use crate::world::site::{HOME_PLOTS, SiteKey};
use serde::{Deserialize, Serialize};
use std::sync::Arc;

/// How far the walkable library may be explored by a received key.
const MAX_FLOOR: u32 = 100_000;
const MAX_COORDINATE: i32 = 100_000_000;
const MAX_ADDRESS: usize = 64;
/// The most of each kind of thing a gallery can hold.
const MAX_ENEMIES: usize = 300;
const MAX_NODES: usize = 800;
const MAX_CHESTS: usize = 64;
const MAX_DROPS: usize = 1200;
const MAX_SHOTS: usize = 2000;
const MAX_ZONES: usize = 600;
const MAX_TORCHES: usize = 64;
/// Nothing in the game is larger than these.
const MAX_ZONE_RADIUS: f32 = 400.0;
const MAX_BODY_RADIUS: f32 = 64.0;
const MAX_SCALE: f32 = 5.0;
const MAX_SPEED: f32 = 5000.0;
const MAX_EVENTS: usize = 600;
const MAX_TEXT: usize = 240;
const MAX_PARTICLES: u16 = 150;
const MAX_ACTION_TEXT: usize = 120;

/// The gallery as the host sees it, sent many times a second.
#[derive(Clone, Serialize, Deserialize)]
pub struct State {
    pub key: SiteKey,
    pub delta: SiteDelta,
    pub time: f32,
    pub darkness: f32,
    pub host_body: Body,
    pub guest_body: Body,
    /// What the guest needs to draw the host.
    pub host: Box<PlayerData>,
    pub guest_health: f32,
    pub guest_energy: f32,
    /// The guest's counts, which change too often to travel with the rest.
    pub guest_tally: Tally,
    /// The guest's own data, only when something else in it changed.
    pub guest: Option<Box<PlayerData>>,
    /// The host's world without the gallery changes, only when it changed.
    pub world: Option<Box<WorldData>>,
    pub enemies: Vec<Enemy>,
    pub nodes: Vec<NodeState>,
    pub chests: Vec<ChestState>,
    pub drops: Vec<Drop>,
    pub projectiles: Vec<Projectile>,
    pub zones: Vec<Zone>,
}

/// Remembers what the guest already has, to send only what changes.
#[derive(Default)]
pub struct Publisher {
    guest: Option<PlayerData>,
    world: Option<WorldData>,
}

impl Publisher {
    /// Makes the next state carry everything again.
    pub fn resend(&mut self) {
        *self = Publisher::default();
    }

    /// The current state, or `None` without a guest. Long histories are set
    /// aside while comparing and copying, so that this stays cheap however far
    /// the players have gone. `game` is left as it was.
    pub fn capture(&mut self, game: &mut Game) -> Option<State> {
        game.companion.as_ref()?;
        let delta = game.delta();

        // The host's own data is in place between steps.
        let glossary = std::mem::take(&mut game.profile.player.glossary);
        let inventory = std::mem::take(&mut game.profile.player.inventory);
        let opened = std::mem::take(&mut game.profile.player.records.opened);
        let mut host = Box::new(game.profile.player.clone());
        // Nothing of this shows on the partner's screen.
        host.resources.clear();
        game.profile.player.glossary = glossary;
        game.profile.player.inventory = inventory;
        game.profile.player.records.opened = opened;

        let other = game.companion.as_deref_mut()?;
        let (health, energy) = (other.player.hp, other.player.energy);
        (other.player.hp, other.player.energy) = (0.0, 0.0);
        // The counts change with every blow: they are compared apart.
        let tally = std::mem::take(&mut other.player.records.tally);
        let guest = (self.guest.as_ref() != Some(&other.player)).then(|| {
            self.guest = Some(other.player.clone());
            let mut full = other.player.clone();
            (full.hp, full.energy) = (health, energy);
            full.records.tally = tally;
            Box::new(full)
        });
        (other.player.hp, other.player.energy) = (health, energy);
        other.player.records.tally = tally;
        let guest_body = other.body.clone();

        let world = &mut game.profile.world;
        let deltas = std::mem::take(&mut world.deltas);
        let unclaimed = std::mem::take(&mut world.unclaimed);
        let (seconds, position) = (world.play_seconds, world.position);
        (world.play_seconds, world.position) = (0.0, (0.0, 0.0));
        let changed = (self.world.as_ref() != Some(world)).then(|| {
            self.world = Some(world.clone());
            Box::new(world.clone())
        });
        (world.play_seconds, world.position) = (seconds, position);
        world.deltas = deltas;
        world.unclaimed = unclaimed;

        Some(State {
            key: game.site.key.clone(),
            delta,
            time: game.time,
            darkness: game.darkness,
            host_body: game.body.clone(),
            guest_body,
            host,
            guest_health: health,
            guest_energy: energy,
            guest_tally: tally,
            guest,
            world: changed,
            enemies: game.enemies.clone(),
            nodes: game.nodes.clone(),
            chests: game.chests.clone(),
            drops: game.drops.clone(),
            projectiles: game.projectiles.clone(),
            zones: game.zones.clone(),
        })
    }
}

/// Why a received state was thrown away.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Invalid(pub &'static str);

fn sound_key(key: &SiteKey) -> bool {
    match key {
        SiteKey::Walk { floor, q, r } => {
            (1..=MAX_FLOOR).contains(floor)
                && q.unsigned_abs() <= MAX_COORDINATE as u32
                && r.unsigned_abs() <= MAX_COORDINATE as u32
        }
        SiteKey::Remote {
            address,
            difficulty,
        } => address.len() <= MAX_ADDRESS && (1..=MAX_FLOOR).contains(difficulty),
    }
}

fn finite(v: V2) -> bool {
    v.x.is_finite() && v.y.is_finite() && v.x.abs() < 1.0e5 && v.y.abs() < 1.0e5
}

/// A direction: about one unit long.
fn heading(v: V2) -> bool {
    finite(v) && v.len() <= 2.0
}

/// What an attack shows is drawn along these, so they decide how long drawing
/// it takes.
fn sound_ai(ai: &Ai) -> bool {
    match *ai {
        Ai::Windup {
            t,
            total,
            dir,
            target,
        } => t.is_finite() && total.is_finite() && heading(dir) && finite(target),
        Ai::Attack {
            t,
            total,
            dir,
            speed,
            ..
        } => {
            t.is_finite() && total.is_finite() && heading(dir) && (0.0..=MAX_SPEED).contains(&speed)
        }
        Ai::Recover { t } => t.is_finite(),
        Ai::Chase | Ai::Hidden => true,
    }
}

fn sound_body(b: &Body) -> bool {
    b.z.is_finite()
        && b.revive.is_finite()
        && b.charge.is_finite()
        && heading(b.dash_dir)
        && finite(b.pos)
        && finite(b.vel)
        && finite(b.facing)
        && b.anim.is_finite()
        && b.swing.is_none_or(|s| {
            finite(s.dir) && s.t.is_finite() && s.duration.is_finite() && s.duration > 0.0
        })
        && sound_pet(&b.pet)
}

fn sound_pet(p: &PetBody) -> bool {
    finite(p.pos)
        && heading(p.facing)
        && p.anim.is_finite()
        && p.still.is_finite()
        && p.bite.is_finite()
}

fn sound_enemy(data: &GameData, e: &Enemy) -> bool {
    let words = data.lexicon.len();
    finite(e.pos)
        && finite(e.facing)
        && e.hp.is_finite()
        && e.max_hp.is_finite()
        && e.scale.is_finite()
        && (0.05..=MAX_SCALE).contains(&e.scale)
        && e.radius.is_finite()
        && (0.0..=MAX_BODY_RADIUS).contains(&e.radius)
        && sound_ai(&e.ai)
        && e.anim.is_finite()
        && e.z.is_finite()
        && e.name.len() <= MAX_TEXT
        && e.sprite.len() <= MAX_TEXT
        && e.flags.len() <= 64
        && e.archetype < data.enemies.archetypes.len()
        && e.modifiers.len() <= 16
        && e.modifiers
            .iter()
            .all(|&m| m < data.enemies.modifiers.len())
        && e.word.is_none_or(|(w, _)| (w as usize) < words)
        && e.elite
            .as_ref()
            .is_none_or(|elite| (elite.noun as usize) < words && (elite.quality as usize) < words)
        && e.boss
            .as_ref()
            .is_none_or(|b| b.index < data.enemies.guardians.len())
}

impl State {
    /// Whether every part is something the game can draw.
    pub fn check(&self, data: &GameData) -> Result<(), Invalid> {
        let require = |ok: bool, what| if ok { Ok(()) } else { Err(Invalid(what)) };
        require(sound_key(&self.key), "sitio")?;
        require(self.time.is_finite() && self.darkness.is_finite(), "tiempo")?;
        require(
            sound_body(&self.host_body) && sound_body(&self.guest_body),
            "jugadores",
        )?;
        require(
            self.guest_health.is_finite() && self.guest_energy.is_finite(),
            "salud",
        )?;
        let grid = |p: &Pos| (-64..=128).contains(&p.x) && (-64..=128).contains(&p.y);
        require(
            self.delta.torches.len() <= MAX_TORCHES
                && self.delta.torches.iter().all(grid)
                && self.delta.stairs_up.iter().all(grid)
                && self.delta.return_lamp.iter().all(grid),
            "antorchas",
        )?;
        require(
            self.enemies.len() <= MAX_ENEMIES && self.enemies.iter().all(|e| sound_enemy(data, e)),
            "enemigos",
        )?;
        require(
            self.nodes.len() <= MAX_NODES && self.nodes.iter().all(|n| finite(n.pos)),
            "recursos",
        )?;
        require(
            self.chests.len() <= MAX_CHESTS && self.chests.iter().all(|c| finite(c.pos)),
            "cofres",
        )?;
        require(
            self.drops.len() <= MAX_DROPS
                && self.drops.iter().all(|d| {
                    finite(d.pos)
                        && d.z.is_finite()
                        && match &d.kind {
                            DropKind::Item(item) => item.name.len() <= MAX_TEXT,
                            _ => true,
                        }
                }),
            "botín",
        )?;
        require(
            self.projectiles.len() <= MAX_SHOTS
                && self
                    .projectiles
                    .iter()
                    .all(|p| finite(p.pos) && finite(p.vel) && p.radius.is_finite()),
            "proyectiles",
        )?;
        require(
            self.zones.len() <= MAX_ZONES
                && self.zones.iter().all(|z| {
                    finite(z.pos)
                        && (0.0..=MAX_ZONE_RADIUS).contains(&z.radius)
                        && z.delay.is_finite()
                }),
            "zonas",
        )?;
        if let Some(world) = &self.world {
            require(
                sound_key(&world.current)
                    && world.stations.len() <= HOME_PLOTS.len()
                    && world.lamps.iter().all(|l| sound_key(&l.key))
                    && world.visited.values().all(|v| {
                        v.biome < data.biomes.len() && sound_key(&SiteKey::walk(v.floor, v.q, v.r))
                    }),
                "mundo",
            )?;
        }
        Ok(())
    }
}

impl Game {
    /// The guest's copy of the host's game, built from a complete state.
    pub fn mirror(data: Arc<GameData>, world_seed: u64, state: State) -> Result<Game, Invalid> {
        state.check(&data)?;
        let (Some(guest), Some(world)) = (state.guest.clone(), state.world.clone()) else {
            return Err(Invalid("el primer estado está incompleto"));
        };
        let guest = guest.sanitized(&data);
        let host = state.host.clone().sanitized(&data);
        let mut profile = Profile {
            version: SAVE_VERSION,
            world_seed,
            player: guest,
            world: *world,
        };
        profile.world.current = state.key.clone();
        let mut game = Game::new(data, profile);
        game.mirrored = true;
        game.slot = GUEST;
        game.companion = Some(Box::new(Companion {
            body: state.host_body.clone(),
            derived: stats::derive(&game.data, &host),
            player: host,
            buffered: Input::default(),
        }));
        game.apply_state(state)?;
        game.events.clear();
        Ok(game)
    }

    /// Whether this is the guest's copy.
    pub fn is_mirror(&self) -> bool {
        self.mirrored
    }

    /// Brings the mirror up to date with a state from the host.
    pub fn apply_state(&mut self, state: State) -> Result<(), Invalid> {
        state.check(&self.data)?;
        if !self.mirrored {
            return Err(Invalid("solo la copia del invitado recibe estados"));
        }
        if let Some(mut world) = state.world {
            world.deltas = std::mem::take(&mut self.profile.world.deltas);
            self.profile.world = *world;
        }
        if state.key != self.site.key {
            self.site = crate::world::site::generate_with(
                &self.data,
                &self.feistel,
                self.profile.world_seed,
                &state.key,
            );
        }
        self.profile.world.current = state.key;
        for d in crate::world::hex::Dir::ALL {
            if state.delta.opened_doors[d.index()] {
                self.site.grid.open_door(d);
            }
        }
        // Only the gallery being looked at is kept.
        self.profile.world.deltas.clear();
        self.profile
            .world
            .deltas
            .insert(self.site.key.id(), state.delta);

        let mut refresh = false;
        if let Some(guest) = state.guest {
            self.profile.player = guest.sanitized(&self.data);
            refresh = true;
        }
        if let Some(other) = self.companion.as_deref_mut() {
            let host = state.host.sanitized(&self.data);
            other.derived = stats::derive(&self.data, &host);
            other.player = host;
            other.body = state.host_body;
        }
        if refresh {
            self.derived = stats::derive(&self.data, &self.profile.player);
        }
        self.profile.player.records.tally = state.guest_tally.tidy();
        let limit = |v: f32, max: f32| v.clamp(0.0, max);
        self.profile.player.hp = limit(state.guest_health, self.derived.max_hp);
        self.profile.player.energy = limit(state.guest_energy, self.derived.max_energy);
        self.body = state.guest_body;
        self.time = state.time;
        self.darkness = state.darkness;
        self.enemies = state.enemies;
        self.nodes = state.nodes;
        self.chests = state.chests;
        self.drops = state.drops;
        self.projectiles = state.projectiles;
        self.zones = state.zones;
        self.collect_lights();
        Ok(())
    }

    /// What the guest did in menus since the last call.
    pub fn take_actions(&mut self) -> Vec<Action> {
        std::mem::take(&mut self.outbox)
    }

    /// Notes an action of the guest's mirror so that the host repeats it.
    pub(crate) fn record(&mut self, action: Action) {
        if self.mirrored {
            self.outbox.push(action);
        }
    }

    /// The host carries out something the guest did in a menu.
    pub fn apply_action(&mut self, action: Action) {
        if !action.is_reasonable() {
            return;
        }
        self.as_player(GUEST, |g| g.perform(action));
    }

    fn perform(&mut self, action: Action) {
        let index =
            |g: &Game, uid: u64| g.profile.player.inventory.iter().position(|i| i.uid == uid);
        match action {
            Action::Craft { recipe, plot } => {
                if let Err(e) = self.craft(&recipe, plot) {
                    self.say(e.message());
                }
            }
            Action::Learn(id) => {
                self.learn_skill(&id);
            }
            Action::Equip(uid) => {
                if let Some(i) = index(self, uid) {
                    self.equip(i);
                }
            }
            Action::Unequip(slot) => {
                self.unequip(slot);
            }
            Action::Move { item, to } => {
                if let Some(i) = index(self, item) {
                    self.swap_inventory(i, to as usize);
                }
            }
            Action::Recycle(uid) => {
                if let Some(i) = index(self, uid) {
                    self.recycle(i);
                }
            }
            Action::Inscribe { target, word } => {
                let target = match target {
                    ItemRef::Worn(slot) => Some(Target::Equipped(usize::from(slot))),
                    ItemRef::Carried(uid) => index(self, uid).map(Target::Inventory),
                };
                if let Some(target) = target
                    && let Err(e) = self.inscribe(target, &word)
                {
                    self.say(e);
                }
            }
            Action::ReadBook => {
                self.read_book();
            }
            Action::LampTravel(i) => {
                self.travel_to_lamp(i as usize);
            }
            Action::CatalogTravel(phrase) => {
                if let Err(e) = self.catalog_travel(&phrase) {
                    self.say(e);
                }
            }
            Action::SetLook(look) => self.profile.player.look = look.sanitized(),
            Action::LearnPet(id) => {
                self.learn_pet_skill(&id);
            }
            Action::TogglePin(recipe) => {
                self.toggle_pin(&recipe);
            }
            Action::SetName(name) => self.set_name(&name),
            Action::Upgrade(id) => {
                if let Err(e) = self.upgrade(&id) {
                    self.say(e.message());
                }
            }
            Action::SetPet { name, look } => self.set_pet(&name, look),
        }
    }
}

/// An item named by something that does not change when others move.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum ItemRef {
    /// By equipment slot number.
    Worn(u8),
    Carried(u64),
}

/// Something the guest did in a menu.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum Action {
    Craft {
        recipe: String,
        plot: Option<u8>,
    },
    Learn(String),
    Equip(u64),
    Unequip(Slot),
    /// Moves a carried item to a position of the bag.
    Move {
        item: u64,
        to: u32,
    },
    Recycle(u64),
    Inscribe {
        target: ItemRef,
        word: String,
    },
    ReadBook,
    LampTravel(u32),
    CatalogTravel(String),
    SetLook(Look),
    LearnPet(String),
    TogglePin(String),
    Upgrade(String),
    SetName(String),
    SetPet {
        name: String,
        look: PetLook,
    },
}

impl Action {
    fn is_reasonable(&self) -> bool {
        let short = |s: &str| s.len() <= MAX_ACTION_TEXT;
        match self {
            Action::Craft { recipe, .. } => short(recipe),
            Action::Learn(id) => short(id),
            Action::Inscribe { word, .. } => short(word),
            Action::CatalogTravel(phrase) => short(phrase),
            Action::LearnPet(id) | Action::TogglePin(id) | Action::Upgrade(id) => short(id),
            Action::SetName(name) | Action::SetPet { name, .. } => short(name),
            _ => true,
        }
    }
}

/// Controls received from the guest, within what a real device can send.
pub fn tidy_input(mut input: Input) -> Input {
    let vector = |v: V2| finite(v).then_some(v);
    input.movement = vector(input.movement).map_or(V2::ZERO, |m| m.clamp_len(1.0));
    input.aim_at = input.aim_at.and_then(vector);
    input.aim_dir = input.aim_dir.and_then(vector);
    input.cycle_tool = input.cycle_tool.signum();
    input
}

fn clip(text: &mut String) {
    if text.chars().count() > MAX_TEXT {
        *text = text.chars().take(MAX_TEXT).collect();
    }
}

/// Events received from the host, within limits.
pub fn tidy_events(mut events: Vec<GameEvent>) -> Vec<GameEvent> {
    events.truncate(MAX_EVENTS);
    events.retain_mut(|event| match event {
        GameEvent::Particles {
            pos,
            z,
            count,
            speed,
            ..
        } => {
            *count = (*count).min(MAX_PARTICLES);
            finite(*pos) && z.is_finite() && speed.is_finite()
        }
        GameEvent::Number { pos, .. } => finite(*pos),
        GameEvent::Floating { pos, text, .. } => {
            clip(text);
            finite(*pos)
        }
        GameEvent::Shake(amount) => {
            *amount = if amount.is_finite() {
                amount.clamp(0.0, 12.0)
            } else {
                0.0
            };
            true
        }
        GameEvent::Message(text) => {
            clip(text);
            true
        }
        GameEvent::Banner {
            title, subtitle, ..
        } => {
            clip(title);
            clip(subtitle);
            true
        }
        GameEvent::Open(ScreenRequest::Build(plot)) => usize::from(*plot) < HOME_PLOTS.len(),
        _ => true,
    });
    events
}
