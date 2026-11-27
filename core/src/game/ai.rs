//! The ten archetype behaviours. Every attack is telegraphed by a windup and
//! damage only comes from attack hitboxes, never from merely touching.

use super::actors::{Ai, Enemy, Projectile, ShotKind, Zone, ZoneKind};
use super::events::{GameEvent, Sfx};
use super::light;
use super::physics::{self, Mobility, Obstacle};
use super::vec::{V2, v2};
use crate::babel::seeds::Dice;
use crate::data::{Behavior, ModFlag};
use crate::world::grid::Grid;

/// A melee or body hit an enemy landed on the player.
#[derive(Clone, Copy, Debug)]
pub struct PlayerHit {
    pub damage: f32,
    pub from: V2,
    pub knockback: f32,
    pub burn: bool,
    pub poison: bool,
    pub slow: bool,
    pub drain: bool,
    /// Enemy to heal for lifesteal, or to damage with thorns.
    pub source: Option<u32>,
}

impl PlayerHit {
    pub fn plain(damage: f32, from: V2) -> PlayerHit {
        PlayerHit {
            damage,
            from,
            knockback: 90.0,
            burn: false,
            poison: false,
            slow: false,
            drain: false,
            source: None,
        }
    }
}

pub struct AiCtx<'a> {
    pub grid: &'a Grid,
    pub obstacles: &'a [Obstacle],
    pub player: V2,
    pub player_alive: bool,
    pub dt: f32,
    pub depth: u32,
    pub projectiles: &'a mut Vec<Projectile>,
    pub zones: &'a mut Vec<Zone>,
    pub hits: &'a mut Vec<PlayerHit>,
    pub events: &'a mut Vec<GameEvent>,
    pub dice: &'a mut Dice,
}

pub fn hit_from(e: &Enemy) -> PlayerHit {
    let damage = e.damage * if e.enraged() { 1.25 } else { 1.0 };
    PlayerHit {
        damage,
        from: e.pos,
        knockback: if e.has(ModFlag::HeavyKnockback) {
            180.0
        } else {
            95.0
        },
        burn: e.has(ModFlag::BurnOnHit),
        poison: e.has(ModFlag::PoisonOnHit),
        slow: e.has(ModFlag::SlowOnHit),
        drain: e.has(ModFlag::DrainsEnergy),
        source: Some(e.uid),
    }
}

pub fn shot(e: &Enemy, dir: V2, speed: f32, kind: ShotKind, damage: f32) -> Projectile {
    let hit = hit_from(e);
    Projectile {
        owner: 0,
        pos: e.pos + dir * (e.radius + 3.0),
        vel: dir * speed,
        z: 8.0,
        ttl: 2.6,
        damage,
        radius: 4.0,
        from_player: false,
        kind,
        burn: hit.burn,
        slow: hit.slow,
        poison: hit.poison,
    }
}

fn walk(e: &mut Enemy, ctx: &AiCtx, dir: V2, speed: f32) -> bool {
    let slow = e.status.speed_factor();
    let rage = if e.enraged() { 1.4 } else { 1.0 };
    let delta = dir * (speed * slow * rage * ctx.dt);
    let mobility = e.mobility();
    let (pos, blocked) = physics::move_body(
        ctx.grid,
        ctx.obstacles,
        e.pos,
        delta,
        (e.radius * 0.8).min(7.0),
        mobility,
    );
    e.pos = pos;
    blocked
}

/// Walks toward a point, sidestepping when a straight line is blocked.
fn pursue(e: &mut Enemy, ctx: &AiCtx, target: V2, speed: f32) {
    let mut dir = (target - e.pos).norm();
    if e.mobility() == Mobility::Ground && !light::line_clear(ctx.grid, e.pos, target) {
        // Probe rotated headings and keep the first open one.
        let turn = if e.uid.is_multiple_of(2) { 1.0 } else { -1.0 };
        for step in [0.6f32, 1.15, 1.65, -0.6, -1.15, -1.65] {
            let candidate = dir.rotate(step * turn);
            if !physics::blocked(
                ctx.grid,
                ctx.obstacles,
                e.pos + candidate * 14.0,
                4.0,
                Mobility::Ground,
            ) {
                dir = candidate;
                break;
            }
        }
    }
    e.facing = dir;
    if walk(e, ctx, dir, speed) {
        e.wander = dir.perp() * if e.uid.is_multiple_of(2) { 1.0 } else { -1.0 };
        e.timer = 0.35;
    }
    if e.timer > 0.0 {
        let side = e.wander;
        walk(e, ctx, side, speed * 0.7);
    }
}

fn attack_range(b: Behavior) -> f32 {
    match b {
        Behavior::Erratic => 34.0,
        Behavior::Charger => 92.0,
        Behavior::Thrower => 150.0,
        Behavior::Burner => 52.0,
        Behavior::Ambusher => 76.0,
        Behavior::Tank => 32.0,
        Behavior::Phantom => 34.0,
        Behavior::Swarm => 24.0,
        Behavior::Patroller => 140.0,
        Behavior::Beast => 60.0,
    }
}

fn can_attack(e: &Enemy, ctx: &AiCtx, dist: f32) -> bool {
    if e.grace > 0.0 || e.cooldown > 0.0 || !ctx.player_alive {
        return false;
    }
    if dist > attack_range(e.behavior) + e.radius {
        return false;
    }
    match e.behavior {
        Behavior::Charger | Behavior::Thrower | Behavior::Beast => {
            light::line_clear(ctx.grid, e.pos, ctx.player)
        }
        Behavior::Patroller => {
            let d = ctx.player - e.pos;
            (d.x.abs() < 14.0 || d.y.abs() < 14.0) && light::line_clear(ctx.grid, e.pos, ctx.player)
        }
        _ => true,
    }
}

fn movement(e: &mut Enemy, ctx: &mut AiCtx, to: V2, dist: f32) {
    let aware = dist < 190.0 || e.hurt_timer > 0.0 || e.elite.is_some();
    if !aware || !ctx.player_alive {
        // Idle wandering.
        if e.timer <= 0.0 {
            e.timer = ctx.dice.between(0.8, 2.2);
            e.wander = if ctx.dice.chance(0.4) {
                V2::ZERO
            } else {
                V2::from_angle(ctx.dice.between(0.0, std::f32::consts::TAU))
            };
        }
        let dir = e.wander;
        if dir != V2::ZERO {
            e.facing = dir;
            walk(e, ctx, dir, e.speed * 0.35);
        }
        return;
    }
    let toward = to.norm();
    match e.behavior {
        Behavior::Erratic => {
            let wobble = toward.perp() * (e.anim * 7.0).sin() * 1.2;
            let dir = (toward + wobble).norm();
            e.facing = toward;
            walk(e, ctx, dir, e.speed);
        }
        Behavior::Swarm => {
            let side = if e.uid.is_multiple_of(2) { 1.0 } else { -1.0 };
            let orbit = toward.perp() * side;
            let pull = if dist > 30.0 { 0.9 } else { -0.2 };
            e.facing = toward;
            walk(e, ctx, (orbit * 0.7 + toward * pull).norm(), e.speed);
        }
        Behavior::Thrower => {
            if dist < 64.0 {
                // The lost reader runs away.
                e.facing = toward * -1.0;
                walk(e, ctx, toward * -1.0, e.speed * 1.15);
            } else if dist > 120.0 {
                pursue(e, ctx, ctx.player, e.speed);
            } else {
                e.facing = toward;
                let side = if (e.anim * 0.5).sin() > 0.0 {
                    1.0
                } else {
                    -1.0
                };
                walk(e, ctx, toward.perp() * side, e.speed * 0.45);
            }
        }
        Behavior::Beast => {
            if dist > 62.0 {
                pursue(e, ctx, ctx.player, e.speed);
            } else {
                let side = if e.uid.is_multiple_of(2) { 1.0 } else { -1.0 };
                e.facing = toward;
                walk(e, ctx, toward.perp() * side, e.speed * 0.7);
            }
        }
        Behavior::Patroller => {
            // Straight axis-aligned patrol that turns on contact.
            if e.wander == V2::ZERO || (e.wander.x != 0.0 && e.wander.y != 0.0) {
                e.wander = if e.uid.is_multiple_of(2) {
                    v2(1.0, 0.0)
                } else {
                    v2(0.0, 1.0)
                };
            }
            let dir = e.wander;
            e.facing = dir;
            e.timer -= ctx.dt * 0.0;
            if walk(e, ctx, dir, e.speed * 0.75) || e.timer <= 0.0 {
                e.timer = ctx.dice.between(1.4, 3.0);
                let turns = [v2(1.0, 0.0), v2(0.0, 1.0), v2(-1.0, 0.0), v2(0.0, -1.0)];
                let toward_axis = if to.x.abs() > to.y.abs() {
                    v2(to.x.signum(), 0.0)
                } else {
                    v2(0.0, to.y.signum())
                };
                e.wander = if ctx.dice.chance(0.55) {
                    toward_axis
                } else {
                    turns[ctx.dice.index(4)]
                };
            }
        }
        _ => {
            if dist > e.radius + 12.0 {
                pursue(e, ctx, ctx.player, e.speed);
            } else {
                e.facing = toward;
            }
        }
    }
}

/// Fires the attack once its windup has finished and returns the follow-up state.
fn release(e: &mut Enemy, ctx: &mut AiCtx, dir: V2, target: V2) -> Ai {
    if e.has(ModFlag::Shock) {
        for i in 0..4 {
            let d = V2::from_angle(i as f32 * std::f32::consts::FRAC_PI_2 + 0.785);
            ctx.projectiles
                .push(shot(e, d, 90.0, ShotKind::Spark, e.damage * 0.4));
        }
    }
    let lunge = |t: f32, speed: f32, dir: V2| Ai::Attack {
        t,
        total: t,
        dir,
        speed,
        hit: false,
    };
    match e.behavior {
        Behavior::Erratic => lunge(0.24, e.speed * 3.0, dir),
        Behavior::Charger => {
            ctx.events.push(GameEvent::Sound(Sfx::SwingHeavy));
            lunge(0.46, e.speed * 4.4, dir)
        }
        Behavior::Thrower => {
            let count = if ctx.depth >= 10 { 3 } else { 1 };
            for i in 0..count {
                let spread = (i as f32 - (count - 1) as f32 / 2.0) * 0.28;
                ctx.projectiles
                    .push(shot(e, dir.rotate(spread), 118.0, ShotKind::Page, e.damage));
            }
            ctx.events.push(GameEvent::Sound(Sfx::Throw));
            Ai::Recover { t: 0.55 }
        }
        Behavior::Burner => {
            ctx.zones.push(Zone::new(
                ZoneKind::Fire,
                target,
                26.0,
                0.28,
                2.4,
                e.damage * 0.75,
            ));
            ctx.events.push(GameEvent::Sound(Sfx::Burn));
            Ai::Recover { t: 0.8 }
        }
        Behavior::Ambusher => {
            let reach = (target - e.pos).len().min(96.0);
            lunge(0.34, reach / 0.34, (target - e.pos).norm())
        }
        Behavior::Tank => {
            let mut zone = Zone::new(ZoneKind::Slam, e.pos, 30.0 + e.radius, 0.0, 0.25, e.damage);
            zone.tick = 0.0;
            ctx.zones.push(zone);
            ctx.events.push(GameEvent::Sound(Sfx::Explosion));
            ctx.events.push(GameEvent::Shake(2.5));
            Ai::Recover { t: 1.0 }
        }
        Behavior::Phantom => lunge(0.28, e.speed * 4.0, dir),
        Behavior::Swarm => lunge(0.18, e.speed * 3.0, dir),
        Behavior::Patroller => {
            let axis = if dir.x.abs() > dir.y.abs() {
                v2(dir.x.signum(), 0.0)
            } else {
                v2(0.0, dir.y.signum())
            };
            lunge(0.6, e.speed * 3.4, axis)
        }
        Behavior::Beast => lunge(0.3, e.speed * 3.6, dir),
    }
}

/// Advances one regular enemy by `ctx.dt`.
pub fn update(e: &mut Enemy, ctx: &mut AiCtx) {
    let dt = ctx.dt;
    e.anim += dt;
    e.timer -= dt;
    e.cooldown = (e.cooldown - dt).max(0.0);
    e.grace = (e.grace - dt).max(0.0);
    e.flash = (e.flash - dt).max(0.0);
    e.hurt_timer = (e.hurt_timer - dt).max(0.0);

    if e.knock.len() > 4.0 {
        let step = e.knock * dt;
        let mobility = e.mobility();
        e.pos = physics::move_body(ctx.grid, ctx.obstacles, e.pos, step, 5.0, mobility).0;
        e.knock = e.knock * (1.0 - (dt * 9.0).min(1.0));
    }

    let to = ctx.player - e.pos;
    let dist = to.len();
    match e.ai {
        Ai::Hidden => {
            e.z = 0.0;
            if (dist < 78.0 && e.grace <= 0.0 && ctx.player_alive) || e.hurt_timer > 0.0 {
                let total = e.windup.max(0.35);
                e.ai = Ai::Windup {
                    t: total,
                    total,
                    dir: to.norm(),
                    target: ctx.player,
                };
            }
        }
        Ai::Chase => {
            movement(e, ctx, to, dist);
            if can_attack(e, ctx, dist) {
                let total = e.windup;
                e.facing = to.norm();
                e.ai = Ai::Windup {
                    t: total,
                    total,
                    dir: to.norm(),
                    target: ctx.player,
                };
            }
        }
        Ai::Windup {
            t,
            total,
            dir,
            target,
        } => {
            let t = t - dt;
            if t <= 0.0 {
                e.ai = release(e, ctx, dir, target);
            } else {
                e.ai = Ai::Windup {
                    t,
                    total,
                    dir,
                    target,
                };
            }
        }
        Ai::Attack {
            t,
            total,
            dir,
            speed,
            mut hit,
        } => {
            let blocked = walk(e, ctx, dir, speed);
            if matches!(e.behavior, Behavior::Ambusher | Behavior::Beast) {
                let phase = 1.0 - (t / total.max(0.01)).clamp(0.0, 1.0);
                e.z = (phase * std::f32::consts::PI).sin() * 12.0;
            }
            if !hit && ctx.player_alive && ctx.player.dist(e.pos) < e.radius + 8.0 && e.z < 9.0 {
                ctx.hits.push(hit_from(e));
                hit = true;
            }
            let t = t - dt;
            let stop = t <= 0.0 || (blocked && e.behavior != Behavior::Ambusher);
            e.ai = if stop {
                e.z = 0.0;
                let rest = match e.behavior {
                    Behavior::Charger | Behavior::Patroller => 0.85,
                    Behavior::Swarm => 0.35,
                    _ => 0.55,
                };
                Ai::Recover { t: rest }
            } else {
                Ai::Attack {
                    t,
                    total,
                    dir,
                    speed,
                    hit,
                }
            };
        }
        Ai::Recover { t } => {
            let t = t - dt;
            e.ai = if t <= 0.0 {
                e.cooldown = if e.has(ModFlag::Calm) { 2.2 } else { 1.0 };
                if e.behavior == Behavior::Ambusher && dist > 130.0 {
                    Ai::Hidden
                } else {
                    Ai::Chase
                }
            } else {
                Ai::Recover { t }
            };
        }
    }
}
