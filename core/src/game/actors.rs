//! Runtime entities of the current site.

use super::physics::Mobility;
use super::vec::V2;
use crate::babel::lexicon::WordId;
use crate::babel::rarity::Rarity;
use crate::data::{Behavior, Consumable, GameData, ModFlag, NodeKind, Resource};
use crate::progression::items::Item;
use crate::world::grid::Pos;
use crate::world::site::{ChestReward, Elite, EnemySpawn, GuardianSpawn};
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, Default, Serialize, Deserialize)]
pub struct Status {
    pub burn: f32,
    pub burn_dps: f32,
    pub poison: f32,
    pub poison_dps: f32,
    pub slow: f32,
    /// Fractions of damage-over-time not yet applied.
    pub burn_acc: f32,
    pub poison_acc: f32,
}

impl Status {
    pub fn ignite(&mut self, dps: f32, time: f32) {
        self.burn = self.burn.max(time);
        self.burn_dps = self.burn_dps.max(dps);
    }

    pub fn envenom(&mut self, dps: f32, time: f32) {
        self.poison = self.poison.max(time);
        self.poison_dps = self.poison_dps.max(dps);
    }

    pub fn chill(&mut self, time: f32) {
        self.slow = self.slow.max(time);
    }

    pub fn speed_factor(&self) -> f32 {
        if self.slow > 0.0 { 0.58 } else { 1.0 }
    }

    /// Advances timers and returns whole damage points `(burn, poison)` due.
    pub fn tick(&mut self, dt: f32) -> (i32, i32) {
        self.slow = (self.slow - dt).max(0.0);
        let mut out = (0, 0);
        if self.burn > 0.0 {
            self.burn -= dt;
            self.burn_acc += self.burn_dps * dt;
            out.0 = self.burn_acc as i32;
            self.burn_acc -= out.0 as f32;
        } else {
            self.burn_dps = 0.0;
            self.burn_acc = 0.0;
        }
        if self.poison > 0.0 {
            self.poison -= dt;
            self.poison_acc += self.poison_dps * dt;
            out.1 = self.poison_acc as i32;
            self.poison_acc -= out.1 as f32;
        } else {
            self.poison_dps = 0.0;
            self.poison_acc = 0.0;
        }
        out
    }

    pub fn clear(&mut self) {
        *self = Status::default();
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub enum Ai {
    Chase,
    /// Ambushers waiting motionless.
    Hidden,
    /// Telegraph: the attack lands after `t` reaches zero.
    Windup {
        t: f32,
        total: f32,
        dir: V2,
        target: V2,
    },
    Attack {
        t: f32,
        total: f32,
        dir: V2,
        speed: f32,
        hit: bool,
    },
    Recover {
        t: f32,
    },
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct BossState {
    pub index: usize,
    pub phase: usize,
    pub cursor: usize,
    pub cooldown: f32,
    pub spiral_left: u32,
    pub spiral_angle: f32,
    pub spiral_timer: f32,
    pub slams_left: u32,
    pub slam_timer: f32,
    /// Attack waiting for the current windup to finish.
    pub pending: Option<crate::data::BossAttack>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Enemy {
    /// Unique within the loaded site.
    pub uid: u32,
    /// Spawn id in the generated site, recorded in the delta on defeat.
    pub spawn: Option<u32>,
    pub archetype: usize,
    pub behavior: Behavior,
    pub name: String,
    pub sprite: String,
    pub pos: V2,
    pub knock: V2,
    pub z: f32,
    pub hp: f32,
    pub max_hp: f32,
    pub damage: f32,
    pub speed: f32,
    pub radius: f32,
    pub scale: f32,
    pub windup: f32,
    pub modifiers: Vec<usize>,
    pub flags: Vec<ModFlag>,
    pub tint: Option<[u8; 3]>,
    /// Derived from the creature word: `% 6` rotates the palette and `/ 6`
    /// picks an accessory.
    pub variant: u32,
    pub ai: Ai,
    pub facing: V2,
    pub wander: V2,
    pub flash: f32,
    pub status: Status,
    /// No attacks while this is positive (spawn grace).
    pub grace: f32,
    pub cooldown: f32,
    pub anim: f32,
    pub timer: f32,
    pub elite: Option<Elite>,
    pub word: Option<(WordId, Rarity)>,
    pub xp: u32,
    pub boss: Option<Box<BossState>>,
    pub summoned: bool,
    /// A mirror decoy: harmless, pale and gone at the first touch.
    pub illusion: bool,
    pub shield: bool,
    pub lit: bool,
    pub hits_taken: u32,
    /// Health bar stays visible for a while after being hit.
    pub hurt_timer: f32,
}

impl Enemy {
    pub fn has(&self, f: ModFlag) -> bool {
        self.flags.contains(&f)
    }

    /// Shadowed creatures and wraiths exist for the player only inside light.
    pub fn needs_light(&self) -> bool {
        self.has(ModFlag::DarkOnly) || self.behavior == Behavior::Phantom
    }

    pub fn visible(&self) -> bool {
        !self.needs_light() || self.lit
    }

    pub fn mobility(&self) -> Mobility {
        if self.behavior == Behavior::Phantom || self.has(ModFlag::PhaseWalls) {
            Mobility::Phasing
        } else if matches!(self.behavior, Behavior::Erratic | Behavior::Swarm) {
            Mobility::Flying
        } else {
            Mobility::Ground
        }
    }

    pub fn enraged(&self) -> bool {
        self.has(ModFlag::Enrages) && self.hp < self.max_hp * 0.5
    }
}

pub struct Scaling {
    pub hp: f32,
    pub damage: f32,
    pub xp: f32,
}

pub fn scaling(data: &GameData, depth: u32) -> Scaling {
    let c = &data.enemies.curve;
    let steps = depth.saturating_sub(1) as f32;
    Scaling {
        hp: 1.0 + c.hp_per_floor * steps,
        damage: 1.0 + c.damage_per_floor * steps,
        xp: 1.0 + c.xp_per_floor * steps,
    }
}

fn capitalize(s: &str) -> String {
    let mut c = s.chars();
    c.next()
        .map_or_else(String::new, |f| f.to_uppercase().chain(c).collect())
}

/// Stats = archetype base × depth curve × modifiers (× elite bonus).
pub fn make_enemy(data: &GameData, uid: u32, depth: u32, spawn: &EnemySpawn) -> Enemy {
    let index = spawn.archetype.min(data.enemies.archetypes.len() - 1);
    let def = &data.enemies.archetypes[index];
    let s = scaling(data, depth);
    let mut hp = def.hp * s.hp;
    let mut damage = def.damage * s.damage;
    let mut speed = def.speed;
    let mut size = 1.0f32;
    let mut xp = def.xp as f32 * s.xp;
    let mut flags: Vec<ModFlag> = Vec::new();
    let mut tint = None;
    let mut adjectives = Vec::new();
    for &m in &spawn.modifiers {
        let Some(md) = data.enemies.modifiers.get(m) else {
            continue;
        };
        hp *= md.hp;
        damage *= md.damage;
        speed *= md.speed;
        size *= md.size;
        xp *= md.xp;
        tint = Some(md.tint);
        adjectives.push(md.name.clone());
        for f in &md.flags {
            if !flags.contains(f) {
                flags.push(*f);
            }
        }
    }
    if spawn.elite.is_some() {
        hp *= 2.3;
        damage *= 1.2;
        size *= 1.22;
        xp *= 3.0;
    }
    if flags.contains(&ModFlag::Calm) {
        damage *= 0.9;
    }
    let base_name = spawn.word.map_or_else(
        || def.name.clone(),
        |(w, _)| capitalize(&data.lexicon.get(w).word),
    );
    let name = spawn.elite.as_ref().map_or_else(
        || {
            if adjectives.is_empty() {
                base_name.clone()
            } else {
                format!("{base_name} {}", adjectives.join(" "))
            }
        },
        |e| e.name.clone(),
    );
    let variant = spawn
        .word
        .map_or(0, |(w, _)| (data.lexicon.get(w).variant_hash() % 24) as u32);
    let behavior = def.behavior;
    let center = spawn.pos.center();
    Enemy {
        uid,
        spawn: Some(spawn.id),
        archetype: index,
        behavior,
        name,
        sprite: def.sprite.clone(),
        pos: V2 {
            x: center.0,
            y: center.1,
        },
        knock: V2::ZERO,
        z: 0.0,
        hp,
        max_hp: hp,
        damage,
        speed,
        radius: def.radius * size.min(1.8),
        scale: size,
        windup: def.windup,
        shield: flags.contains(&ModFlag::FirstHitShield),
        modifiers: spawn.modifiers.clone(),
        flags,
        tint,
        variant,
        ai: if behavior == Behavior::Ambusher {
            Ai::Hidden
        } else {
            Ai::Chase
        },
        facing: V2 { x: 0.0, y: 1.0 },
        wander: V2::ZERO,
        flash: 0.0,
        status: Status::default(),
        grace: 1.1,
        cooldown: 0.6 + (uid % 7) as f32 * 0.13,
        anim: (uid % 11) as f32 * 0.31,
        timer: 0.0,
        elite: spawn.elite.clone(),
        word: spawn.word,
        xp: xp.round() as u32,
        boss: None,
        summoned: false,
        illusion: false,
        lit: true,
        hits_taken: 0,
        hurt_timer: 0.0,
    }
}

/// Guardians are assembled from the rarest words of their staircase site.
pub fn make_guardian(data: &GameData, uid: u32, depth: u32, spawn: &GuardianSpawn) -> Enemy {
    let def = &data.enemies.guardians[spawn.index.min(data.enemies.guardians.len() - 1)];
    // Past the final floor the patterns repeat, but strength continues from
    // the final boss, so infinite mode never gets easier.
    let last = data.enemies.guardians.last().unwrap_or(def);
    let (mut hp, mut damage) = if depth > last.floor {
        let beyond = (depth - last.floor) as f32;
        (
            last.hp * (1.0 + beyond * 0.09),
            last.damage * (1.0 + beyond * 0.05),
        )
    } else {
        // A word-made staircase on a designed floor uses that guardian as is.
        (def.hp, def.damage)
    };
    let mut flags = Vec::new();
    let mut tint = None;
    for &m in &spawn.modifiers {
        if let Some(md) = data.enemies.modifiers.get(m) {
            hp *= 1.0 + (md.hp - 1.0).max(0.0) * 0.3;
            damage *= 1.0 + (md.damage - 1.0).max(0.0) * 0.4;
            tint = Some(md.tint);
            for f in &md.flags {
                // Flags that would make a boss unfair or unreadable are skipped.
                if matches!(
                    f,
                    ModFlag::BurnTrail
                        | ModFlag::BurnOnHit
                        | ModFlag::PoisonOnHit
                        | ModFlag::SlowOnHit
                        | ModFlag::Glows
                        | ModFlag::Shock
                ) {
                    flags.push(*f);
                }
            }
        }
    }
    let epithet = spawn
        .words
        .first()
        .map(|&(w, _)| data.lexicon.get(w).word.clone());
    let name = epithet.map_or_else(|| def.name.clone(), |w| format!("{} «{w}»", def.name));
    let archetype = data.archetype_index(&def.minion);
    let center = spawn.pos.center();
    Enemy {
        uid,
        spawn: None,
        archetype,
        behavior: Behavior::Tank,
        name,
        sprite: def.sprite.clone(),
        pos: V2 {
            x: center.0,
            y: center.1,
        },
        knock: V2::ZERO,
        z: 0.0,
        hp,
        max_hp: hp,
        damage,
        speed: def.speed,
        radius: def.radius,
        scale: 1.0,
        windup: 0.75,
        modifiers: spawn.modifiers.clone(),
        flags,
        tint,
        variant: 0,
        ai: Ai::Chase,
        facing: V2 { x: 0.0, y: 1.0 },
        wander: V2::ZERO,
        flash: 0.0,
        status: Status::default(),
        grace: 2.0,
        cooldown: 1.5,
        anim: 0.0,
        timer: 0.0,
        elite: None,
        word: spawn.words.first().copied(),
        xp: 150 + depth * 30,
        boss: Some(Box::new(BossState {
            index: spawn.index,
            phase: 0,
            cursor: 0,
            cooldown: 2.0,
            spiral_left: 0,
            spiral_angle: 0.0,
            spiral_timer: 0.0,
            slams_left: 0,
            slam_timer: 0.0,
            pending: None,
        })),
        summoned: false,
        illusion: false,
        shield: false,
        lit: true,
        hits_taken: 0,
        hurt_timer: 0.0,
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum ShotKind {
    Page,
    Ink,
    Spark,
    Flame,
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
pub struct Projectile {
    pub pos: V2,
    pub vel: V2,
    pub z: f32,
    pub ttl: f32,
    pub damage: f32,
    pub radius: f32,
    pub from_player: bool,
    /// Which player threw it.
    pub owner: u8,
    pub kind: ShotKind,
    pub burn: bool,
    pub slow: bool,
    pub poison: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum ZoneKind {
    /// One-shot shockwave.
    Slam,
    /// Lingering flames.
    Fire,
    Explosion,
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
pub struct Zone {
    pub pos: V2,
    pub radius: f32,
    /// Telegraph time before it becomes harmful.
    pub delay: f32,
    pub total_delay: f32,
    pub ttl: f32,
    pub damage: f32,
    pub kind: ZoneKind,
    /// Cooldown between damage applications.
    pub tick: f32,
    pub spent: bool,
}

impl Zone {
    pub fn new(kind: ZoneKind, pos: V2, radius: f32, delay: f32, ttl: f32, damage: f32) -> Zone {
        Zone {
            pos,
            radius,
            delay,
            total_delay: delay,
            ttl,
            damage,
            kind,
            tick: 0.0,
            spent: false,
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum DropKind {
    Resource(Resource, u32),
    Consumable(Consumable, u32),
    Item(Box<Item>),
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Drop {
    pub pos: V2,
    pub vel: V2,
    pub z: f32,
    pub vz: f32,
    pub age: f32,
    pub kind: DropKind,
    pub magnet: bool,
}

impl Drop {
    pub fn rarity(&self) -> Rarity {
        match &self.kind {
            DropKind::Item(i) => i.rarity,
            _ => Rarity::Common,
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct NodeState {
    pub id: u32,
    pub kind: NodeKind,
    pub tile: Pos,
    pub pos: V2,
    pub hp: f32,
    pub max_hp: f32,
    pub shake: f32,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ChestState {
    pub id: u32,
    pub pos: V2,
    pub reward: ChestReward,
    pub open: bool,
}
