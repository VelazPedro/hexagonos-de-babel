//! Presentation-only state born from simulation events: particles, floating
//! numbers, banners, toasts, screen shake and flashes.

use babel_core::babel::rarity::Rarity;
use babel_core::game::events::{GameEvent, NumberKind};
use babel_core::game::vec::{V2, v2};

const MAX_PARTICLES: usize = 900;

#[derive(Clone, Copy)]
pub struct Particle {
    pub pos: V2,
    pub z: f32,
    pub vel: V2,
    pub vz: f32,
    pub color: [u8; 3],
    pub life: f32,
    pub max_life: f32,
    pub glow: bool,
    pub gravity: f32,
}

pub struct Floating {
    pub pos: V2,
    pub text: String,
    pub color: [u8; 3],
    pub age: f32,
    pub life: f32,
}

pub struct Banner {
    pub title: String,
    pub subtitle: String,
    pub color: [u8; 3],
    pub age: f32,
}

pub const BANNER_TIME: f32 = 3.6;
pub const TOAST_TIME: f32 = 3.2;

#[derive(Default)]
pub struct Fx {
    pub particles: Vec<Particle>,
    pub floats: Vec<Floating>,
    pub banners: Vec<Banner>,
    pub toasts: Vec<(String, f32)>,
    pub shake: f32,
    pub shake_enabled: bool,
    /// Colour and remaining strength of a full-screen flash.
    pub flash: Option<([u8; 3], f32)>,
    /// Rays celebrating a Legendary or Canonical find.
    pub celebration: Option<(Rarity, f32)>,
    seed: u32,
}

impl Fx {
    pub fn new() -> Fx {
        Fx {
            shake_enabled: true,
            seed: 0x1234_5678,
            ..Default::default()
        }
    }

    /// Cosmetic randomness; never affects the simulation.
    pub fn random(&mut self) -> f32 {
        self.seed ^= self.seed << 13;
        self.seed ^= self.seed >> 17;
        self.seed ^= self.seed << 5;
        (self.seed >> 8) as f32 / (1u32 << 24) as f32
    }

    pub fn spread(&mut self, amount: f32) -> f32 {
        (self.random() - 0.5) * 2.0 * amount
    }

    pub fn spawn(&mut self, p: Particle) {
        if self.particles.len() >= MAX_PARTICLES {
            self.particles.swap_remove(0);
        }
        self.particles.push(p);
    }

    pub fn burst(&mut self, pos: V2, z: f32, color: [u8; 3], count: u16, speed: f32) {
        for _ in 0..count {
            let angle = self.random() * std::f32::consts::TAU;
            let s = speed * (0.35 + self.random() * 0.65);
            let life = 0.25 + self.random() * 0.35;
            let vz = 20.0 + self.random() * 60.0;
            let bright = color.iter().map(|&c| u32::from(c)).sum::<u32>() > 520;
            self.spawn(Particle {
                pos,
                z,
                vel: V2::from_angle(angle) * s,
                vz,
                color,
                life,
                max_life: life,
                glow: bright,
                gravity: 220.0,
            });
        }
    }

    pub fn toast(&mut self, text: String) {
        if let Some(last) = self.toasts.last_mut()
            && last.0 == text
        {
            last.1 = 0.0;
            return;
        }
        self.toasts.push((text, 0.0));
        if self.toasts.len() > 4 {
            self.toasts.remove(0);
        }
    }

    /// Applies the visual part of an event.
    pub fn handle(&mut self, event: &GameEvent) {
        match event {
            GameEvent::Particles {
                pos,
                z,
                color,
                count,
                speed,
            } => self.burst(*pos, *z, *color, *count, *speed),
            GameEvent::Number { pos, amount, kind } => {
                let color = match kind {
                    NumberKind::Enemy => [255, 246, 214],
                    NumberKind::Echo => [150, 226, 255],
                    NumberKind::Player => [255, 96, 96],
                    NumberKind::Heal => [130, 240, 130],
                    NumberKind::Burn => [255, 164, 60],
                    NumberKind::Poison => [158, 232, 80],
                };
                let jitter = self.spread(5.0);
                self.floats.push(Floating {
                    pos: *pos + v2(jitter, 0.0),
                    text: amount.to_string(),
                    color,
                    age: 0.0,
                    life: 0.75,
                });
            }
            GameEvent::Floating { pos, text, color } => {
                // Stack simultaneous pickups instead of overlapping them.
                let waiting = self
                    .floats
                    .iter()
                    .filter(|f| f.age < 0.25 && f.pos.dist(*pos) < 12.0)
                    .count();
                self.floats.push(Floating {
                    pos: *pos + v2(0.0, -(waiting as f32) * 9.0),
                    text: text.clone(),
                    color: *color,
                    age: 0.0,
                    life: 1.25,
                });
            }
            GameEvent::Shake(amount) => {
                if self.shake_enabled {
                    self.shake = (self.shake + amount * 0.5).max(*amount).min(8.0);
                }
            }
            GameEvent::Message(text) => self.toast(text.clone()),
            GameEvent::Banner {
                title,
                subtitle,
                color,
            } => self.banners.push(Banner {
                title: title.clone(),
                subtitle: subtitle.clone(),
                color: *color,
                age: 0.0,
            }),
            GameEvent::Flash(rarity) => {
                self.flash = Some((rarity.color(), 0.65));
                self.celebration = Some((*rarity, 0.0));
            }
            GameEvent::LevelUp(level) => {
                self.banners.push(Banner {
                    title: format!("¡Nivel {level}!"),
                    subtitle: "Ganaste un punto de habilidad".into(),
                    color: [255, 224, 100],
                    age: 0.0,
                });
                self.flash = Some(([255, 230, 140], 0.25));
            }
            _ => {}
        }
    }

    pub fn clear_world(&mut self) {
        self.particles.clear();
        self.floats.clear();
    }

    pub fn update(&mut self, dt: f32) {
        for p in &mut self.particles {
            p.life -= dt;
            p.pos += p.vel * dt;
            p.vel = p.vel * (1.0 - (dt * 3.0).min(1.0));
            p.vz -= p.gravity * dt;
            p.z = (p.z + p.vz * dt).max(0.0);
        }
        self.particles.retain(|p| p.life > 0.0);
        for f in &mut self.floats {
            f.age += dt;
            f.pos.y -= 22.0 * dt * (1.0 - f.age / f.life).max(0.0);
        }
        self.floats.retain(|f| f.age < f.life);
        if let Some(b) = self.banners.first_mut() {
            b.age += dt;
        }
        // A queue: queued banners hurry the current one along.
        let limit = if self.banners.len() > 1 {
            BANNER_TIME * 0.55
        } else {
            BANNER_TIME
        };
        if self.banners.first().is_some_and(|b| b.age > limit) {
            self.banners.remove(0);
        }
        for t in &mut self.toasts {
            t.1 += dt;
        }
        self.toasts.retain(|t| t.1 < TOAST_TIME);
        self.shake = (self.shake - dt * 18.0).max(0.0);
        if let Some((_, strength)) = &mut self.flash {
            *strength -= dt * 1.3;
        }
        if self.flash.is_some_and(|f| f.1 <= 0.0) {
            self.flash = None;
        }
        if let Some((_, age)) = &mut self.celebration {
            *age += dt;
        }
        if self.celebration.is_some_and(|c| c.1 > 1.8) {
            self.celebration = None;
        }
    }
}
