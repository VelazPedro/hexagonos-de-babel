//! Guardian fights: phases with their own telegraphed patterns.

use super::actors::{Ai, Enemy, ShotKind, Zone, ZoneKind};
use super::ai::{self, AiCtx};
use super::events::{GameEvent, Sfx};
use super::physics::{self, Mobility};
use super::vec::V2;
use crate::data::{BossAttack, GameData};
use crate::world::grid::{CENTER, TILE};

#[derive(Default)]
pub struct BossOutcome {
    /// Minions requested this step.
    pub summon: u32,
    /// Seconds of enforced darkness requested.
    pub darkness: f32,
}

fn windup_for(attack: BossAttack) -> f32 {
    match attack {
        BossAttack::Charge => 0.8,
        BossAttack::Radial(_) => 0.75,
        BossAttack::Aimed(_) => 0.55,
        BossAttack::Slam(_) => 0.5,
        BossAttack::Summon(_) => 0.9,
        BossAttack::FireWave => 0.8,
        BossAttack::Blink => 0.5,
        BossAttack::Spiral(_) => 0.7,
        BossAttack::Darkness => 0.9,
    }
}

fn execute(
    e: &mut Enemy,
    ctx: &mut AiCtx,
    attack: BossAttack,
    dir: V2,
    out: &mut BossOutcome,
) -> Ai {
    let tau = std::f32::consts::TAU;
    match attack {
        BossAttack::Charge => {
            ctx.events.push(GameEvent::Sound(Sfx::SwingHeavy));
            return Ai::Attack {
                t: 0.65,
                total: 0.65,
                dir,
                speed: 235.0,
                hit: false,
            };
        }
        BossAttack::Radial(n) => {
            let offset = e.anim;
            for i in 0..n {
                let d = V2::from_angle(offset + i as f32 / f32::from(n) * tau);
                ctx.projectiles
                    .push(ai::shot(e, d, 88.0, ShotKind::Page, e.damage * 0.6));
            }
            ctx.events.push(GameEvent::Sound(Sfx::Throw));
        }
        BossAttack::Aimed(n) => {
            for i in 0..n {
                let spread = (f32::from(i) - f32::from(n.saturating_sub(1)) / 2.0) * 0.2;
                ctx.projectiles.push(ai::shot(
                    e,
                    dir.rotate(spread),
                    140.0,
                    ShotKind::Ink,
                    e.damage * 0.65,
                ));
            }
            ctx.events.push(GameEvent::Sound(Sfx::Throw));
        }
        BossAttack::Slam(n) => {
            if let Some(b) = e.boss.as_mut() {
                b.slams_left = u32::from(n);
                b.slam_timer = 0.0;
            }
        }
        BossAttack::Summon(n) => {
            out.summon += u32::from(n);
            ctx.events.push(GameEvent::Sound(Sfx::BossRoar));
        }
        BossAttack::FireWave => {
            let side = dir.perp();
            for step in 1..=5 {
                for lane in [-1.0f32, 0.0, 1.0] {
                    let along = step as f32 * 28.0;
                    let pos = e.pos + dir * along + side * (lane * along * 0.42);
                    ctx.zones.push(Zone::new(
                        ZoneKind::Fire,
                        pos,
                        14.0,
                        0.35 + step as f32 * 0.12,
                        1.4,
                        e.damage * 0.6,
                    ));
                }
            }
            ctx.events.push(GameEvent::Sound(Sfx::Burn));
        }
        BossAttack::Blink => {
            let angle = ctx.dice.between(0.0, tau);
            for k in 0..8 {
                let target = ctx.player + V2::from_angle(angle + k as f32 * 0.785) * 74.0;
                if !physics::blocked(ctx.grid, ctx.obstacles, target, 8.0, Mobility::Ground) {
                    ctx.events.push(GameEvent::Particles {
                        pos: e.pos,
                        z: 10.0,
                        color: [200, 140, 255],
                        count: 24,
                        speed: 80.0,
                    });
                    e.pos = target;
                    ctx.events.push(GameEvent::Sound(Sfx::Teleport));
                    break;
                }
            }
        }
        BossAttack::Spiral(n) => {
            if let Some(b) = e.boss.as_mut() {
                b.spiral_left = u32::from(n);
                b.spiral_angle = dir.angle();
                b.spiral_timer = 0.0;
            }
        }
        BossAttack::Darkness => {
            out.darkness = 6.0;
            ctx.events.push(GameEvent::Sound(Sfx::BossRoar));
        }
    }
    Ai::Recover { t: 0.6 }
}

/// Streams that keep running while the boss moves: spirals and slam chains.
fn sustain(e: &mut Enemy, ctx: &mut AiCtx) {
    let damage = e.damage;
    let (mut spiral_shot, mut slam) = (None, false);
    if let Some(b) = e.boss.as_mut() {
        if b.spiral_left > 0 {
            b.spiral_timer -= ctx.dt;
            if b.spiral_timer <= 0.0 {
                b.spiral_timer = 0.07;
                b.spiral_left -= 1;
                b.spiral_angle += 0.52;
                spiral_shot = Some(b.spiral_angle);
            }
        }
        if b.slams_left > 0 {
            b.slam_timer -= ctx.dt;
            if b.slam_timer <= 0.0 {
                b.slam_timer = 0.55;
                b.slams_left -= 1;
                slam = true;
            }
        }
    }
    if let Some(a) = spiral_shot {
        for k in 0..2 {
            let d = V2::from_angle(a + k as f32 * std::f32::consts::PI);
            ctx.projectiles
                .push(ai::shot(e, d, 82.0, ShotKind::Ink, damage * 0.5));
        }
    }
    if slam {
        ctx.zones.push(Zone::new(
            ZoneKind::Slam,
            ctx.player,
            28.0,
            0.75,
            0.2,
            damage * 0.9,
        ));
    }
}

pub fn update(data: &GameData, e: &mut Enemy, ctx: &mut AiCtx) -> BossOutcome {
    let mut out = BossOutcome::default();
    let dt = ctx.dt;
    e.anim += dt;
    e.timer -= dt;
    e.grace = (e.grace - dt).max(0.0);
    e.flash = (e.flash - dt).max(0.0);
    e.knock = V2::ZERO;
    let Some(boss) = e.boss.as_ref() else {
        return out;
    };
    let Some(def) = data.enemies.guardians.get(boss.index) else {
        return out;
    };
    let fraction = e.hp / e.max_hp;
    let phase = def
        .phases
        .iter()
        .rposition(|p| fraction <= p.below)
        .unwrap_or(0);
    if let Some(b) = e.boss.as_mut()
        && phase > b.phase
    {
        b.phase = phase;
        b.cursor = 0;
        b.cooldown = 1.0;
        let taunt = &def.phases[phase].taunt;
        if !taunt.is_empty() {
            ctx.events.push(GameEvent::Message(taunt.clone()));
        }
        ctx.events.push(GameEvent::Sound(Sfx::BossRoar));
        ctx.events.push(GameEvent::Shake(4.0));
    }
    if e.grace > 0.0 || !ctx.player_alive {
        return out;
    }
    sustain(e, ctx);

    match e.ai {
        Ai::Windup {
            t,
            total,
            dir,
            target,
        } => {
            let t = t - dt;
            if t > 0.0 {
                e.ai = Ai::Windup {
                    t,
                    total,
                    dir,
                    target,
                };
            } else {
                let pending = e.boss.as_mut().and_then(|b| b.pending.take());
                e.ai = match pending {
                    Some(a) => execute(e, ctx, a, dir, &mut out),
                    None => Ai::Chase,
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
            let (pos, blocked) = physics::move_body(
                ctx.grid,
                ctx.obstacles,
                e.pos,
                dir * (speed * dt),
                8.0,
                Mobility::Flying,
            );
            e.pos = pos;
            if !hit && ctx.player.dist(e.pos) < e.radius + 8.0 {
                ctx.hits.push(ai::hit_from(e));
                hit = true;
            }
            let t = t - dt;
            e.ai = if t <= 0.0 || blocked {
                if blocked {
                    ctx.events.push(GameEvent::Shake(3.0));
                }
                // The recovery after a charge is the opening to strike back.
                Ai::Recover { t: 0.9 }
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
                Ai::Chase
            } else {
                Ai::Recover { t }
            };
        }
        Ai::Chase | Ai::Hidden => {
            let to = ctx.player - e.pos;
            let dist = to.len();
            e.facing = to.norm();
            // Hold a mid distance, drifting back toward the arena when stranded.
            let center = V2 {
                x: CENTER.0 * TILE,
                y: CENTER.1 * TILE,
            };
            let goal = if dist > 62.0 {
                to.norm()
            } else {
                to.norm().perp() * 0.7 + (center - e.pos).norm() * 0.3
            };
            let slow = e.status.speed_factor().max(0.75);
            e.pos = physics::move_body(
                ctx.grid,
                ctx.obstacles,
                e.pos,
                goal.norm() * (e.speed * slow * dt),
                8.0,
                Mobility::Flying,
            )
            .0;
            let mut start = None;
            if let Some(b) = e.boss.as_mut() {
                b.cooldown -= dt;
                if b.cooldown <= 0.0 {
                    let p = &def.phases[b.phase.min(def.phases.len() - 1)];
                    if !p.attacks.is_empty() {
                        let attack = p.attacks[b.cursor % p.attacks.len()];
                        b.cursor += 1;
                        b.cooldown = p.cooldown;
                        b.pending = Some(attack);
                        start = Some(attack);
                    }
                }
            }
            if let Some(attack) = start {
                let total = windup_for(attack);
                e.ai = Ai::Windup {
                    t: total,
                    total,
                    dir: to.norm(),
                    target: ctx.player,
                };
            }
        }
    }
    out
}
