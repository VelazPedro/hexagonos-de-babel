//! The pet that walks with each player. It cannot be hurt, and until it has
//! learned something it only keeps its owner company.

use super::Game;
use super::actors::Ai;
use super::coop::GUEST;
use super::events::{GameEvent, NumberKind, Sfx};
use super::physics::{self, Mobility};
use super::vec::{V2, v2};
use crate::data::ModFlag;
use serde::{Deserialize, Serialize};

/// Where the pet is and what it is doing. It travels with the player's body.
#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
pub struct PetBody {
    pub pos: V2,
    pub facing: V2,
    pub moving: bool,
    /// Drives the walk cycle.
    pub anim: f32,
    /// Seconds without moving: after a while it sits down.
    pub still: f32,
    /// Seconds until it can bite or dig again.
    pub cooldown: f32,
    /// Positive while a bite shows.
    pub bite: f32,
    /// Seconds pushing against something.
    pub stuck: f32,
    /// Seconds during which it only follows, after getting stuck.
    pub wary: f32,
    /// The resource its owner has just hit, and how long it remembers.
    pub digging: Option<(u32, f32)>,
}

/// Where it walks: to one side of its owner, never hidden behind them.
const HEEL_SIDE: f32 = 20.0;
/// How far to the side, or in front, still leaves it in sight.
const CLEAR_ASIDE: f32 = 15.0;
const CLEAR_IN_FRONT: f32 = 10.0;
/// In sight and this close, it need not move.
const BESIDE: f32 = 28.0;
/// It stops this close to where it was going, and starts again this far.
const ARRIVED: f32 = 3.0;
const RESTART: f32 = 11.0;
/// Farther than this from its owner it just reappears beside them.
const LOST: f32 = 150.0;
const STUCK_LIMIT: f32 = 1.0;
/// After getting stuck it forgets about errands for this long.
const WARY: f32 = 3.0;
const RADIUS: f32 = 3.0;

/// It minds enemies this close to its owner.
const GUARD: f32 = 80.0;
const LEAP_GUARD: f32 = 24.0;
/// Gap between its teeth and the edge of what it bites.
const REACH: f32 = 9.0;
const LEAP: f32 = 14.0;
const PUSH: f32 = 150.0;
const BITE_EVERY: f32 = 1.2;
const BITE_SHOWS: f32 = 0.22;

const DIG_EVERY: f32 = 0.9;
/// How long it remembers what its owner was harvesting.
pub const DIG_MEMORY: f32 = 2.5;
const DIG_RANGE: f32 = 48.0;

/// How close it must be to something lying on the ground.
const GRAB: f32 = 7.0;
const PICKABLE_AFTER: f32 = 0.35;

impl PetBody {
    pub fn beside(pos: V2) -> PetBody {
        PetBody {
            pos,
            facing: v2(0.0, 1.0),
            moving: false,
            anim: 0.0,
            still: 0.0,
            cooldown: 0.0,
            bite: 0.0,
            stuck: 0.0,
            wary: 0.0,
            digging: None,
        }
    }
}

impl Game {
    /// The two pets of a party keep to opposite sides, out of each other's way.
    fn pet_side(&self) -> f32 {
        if self.acting() == GUEST { 1.0 } else { -1.0 }
    }

    /// Puts the pet at its owner's side, on solid ground.
    pub(crate) fn place_pet(&mut self) {
        let side = self.pet_side();
        let at = self.safe_position(self.body.pos + v2(side * HEEL_SIDE, 3.0));
        self.body.pet = PetBody::beside(at);
    }

    /// Moves the acting player's pet: it fights, fetches, digs or follows,
    /// in that order, as far as it knows how.
    pub(crate) fn update_pet(&mut self, dt: f32) {
        let pet = &mut self.body.pet;
        pet.cooldown = (pet.cooldown - dt).max(0.0);
        pet.bite = (pet.bite - dt).max(0.0);
        pet.wary = (pet.wary - dt).max(0.0);
        pet.digging = pet
            .digging
            .map(|(node, left)| (node, left - dt))
            .filter(|&(_, left)| left > 0.0);
        pet.moving = false;
        if self.body.pet.pos.dist(self.body.pos) > LOST {
            self.place_pet();
        }
        if self.body.dead > 0.0 || self.body.downed {
            // It stays with whoever fell.
            let side = self.body.pos + v2(-11.0, 4.0);
            self.walk_pet(side, dt);
        } else {
            let busy = self.body.pet.wary <= 0.0
                && (self.pet_fights(dt) || self.pet_fetches(dt) || self.pet_digs(dt));
            if !busy {
                self.pet_follows(dt);
            }
        }
        let pet = &mut self.body.pet;
        if pet.moving {
            pet.anim += dt;
            pet.still = 0.0;
        } else {
            pet.still += dt;
        }
    }

    /// One step toward `target`. Returns whether it is there.
    fn walk_pet(&mut self, target: V2, dt: f32) -> bool {
        let from = self.body.pet.pos;
        let way = target - from;
        let distance = way.len();
        if distance <= ARRIVED {
            self.body.pet.stuck = 0.0;
            return true;
        }
        // A little faster than its owner, and faster still when left behind.
        let hurry = 1.0 + (distance / 60.0).min(1.0);
        let step = (self.derived.speed * hurry * dt).min(distance);
        let (to, _) = physics::move_body(
            &self.site.grid,
            &[],
            from,
            way.norm() * step,
            RADIUS,
            Mobility::Ground,
        );
        let pet = &mut self.body.pet;
        pet.stuck = if to.dist(from) < step * 0.3 {
            pet.stuck + dt
        } else {
            0.0
        };
        pet.pos = to;
        pet.facing = way.norm();
        pet.moving = true;
        false
    }

    /// An errand that cannot be reached is dropped for a while.
    fn pet_gives_up(&mut self) -> bool {
        if self.body.pet.stuck < STUCK_LIMIT {
            return false;
        }
        self.body.pet.stuck = 0.0;
        self.body.pet.wary = WARY;
        true
    }

    fn pet_follows(&mut self, dt: f32) {
        let owner = self.body.pos;
        let facing = self.body.facing;
        // Walking sideways it trails behind; otherwise it takes one side.
        let offset = if facing.x.abs() > facing.y.abs() {
            v2(-facing.x.signum() * HEEL_SIDE, 4.0)
        } else {
            v2(self.pet_side() * HEEL_SIDE, -facing.y.signum() * 5.0)
        };
        let open = |at: V2| !physics::blocked(&self.site.grid, &[], at, RADIUS, Mobility::Ground);
        let heel = [offset, v2(-offset.x, offset.y)]
            .into_iter()
            .map(|o| owner + o)
            .find(|&at| open(at))
            .unwrap_or(owner);
        let apart = self.body.pet.pos - owner;
        let in_sight = apart.x.abs() >= CLEAR_ASIDE || apart.y >= CLEAR_IN_FRONT;
        let beside = in_sight && apart.len() < BESIDE;
        // Once settled, it does not get up for every small shuffle.
        if beside && (self.body.pet.still > 0.0 || self.body.pet.pos.dist(heel) < RESTART) {
            return;
        }
        self.walk_pet(heel, dt);
        let near_owner = apart.len() < BESIDE;
        if self.body.pet.stuck >= STUCK_LIMIT && !near_owner {
            self.place_pet();
        }
    }

    fn pet_fights(&mut self, dt: f32) -> bool {
        let powers = self.derived.pet;
        if powers.bite <= 0.0 {
            return false;
        }
        let owner = self.body.pos;
        let from = self.body.pet.pos;
        let guard = GUARD + powers.pounce * LEAP_GUARD;
        // Only what can be seen and hurt: shields and reflections are the
        // owner's business.
        let target = self
            .enemies
            .iter()
            .filter(|e| e.lit && !e.shield && !e.illusion && !matches!(e.ai, Ai::Hidden))
            .filter(|e| e.pos.dist(owner) <= guard)
            .min_by(|a, b| a.pos.dist(from).total_cmp(&b.pos.dist(from)))
            .map(|e| (e.uid, e.pos, e.radius, e.has(ModFlag::Armored)));
        let Some((uid, pos, radius, armored)) = target else {
            return false;
        };
        if from.dist(pos) > radius + REACH + powers.pounce * LEAP {
            self.walk_pet(pos, dt);
            return !self.pet_gives_up();
        }
        self.body.pet.facing = (pos - from).norm();
        if self.body.pet.cooldown <= 0.0 {
            self.body.pet.cooldown = BITE_EVERY / (1.0 + powers.haste);
            self.body.pet.bite = BITE_SHOWS;
            let armor = if armored { 0.65 } else { 1.0 };
            if powers.pounce > 0.0
                && let Some(e) = self
                    .enemies
                    .iter_mut()
                    .find(|e| e.uid == uid && e.boss.is_none() && e.radius <= 9.0)
            {
                e.knock = (pos - from).norm() * PUSH;
            }
            self.sound(Sfx::Hit);
            self.events.push(GameEvent::Particles {
                pos,
                z: 7.0,
                color: [255, 224, 180],
                count: 5,
                speed: 70.0,
            });
            let damage = self.derived.damage * powers.bite * armor;
            self.apply_damage_to_enemy(uid, damage, NumberKind::Enemy);
        }
        true
    }

    fn pet_fetches(&mut self, dt: f32) -> bool {
        let range = self.derived.pet.fetch;
        if range <= 0.0 {
            return false;
        }
        let owner = self.body.pos;
        let from = self.body.pet.pos;
        // What is already flying toward someone needs no help.
        let target = self
            .drops
            .iter()
            .enumerate()
            .filter(|(_, d)| !d.magnet && d.age > PICKABLE_AFTER && d.pos.dist(owner) <= range)
            .min_by(|a, b| a.1.pos.dist(from).total_cmp(&b.1.pos.dist(from)))
            .map(|(i, d)| (i, d.pos));
        let Some((index, pos)) = target else {
            return false;
        };
        if from.dist(pos) <= GRAB {
            let found = self.drops.remove(index);
            self.pick_up(found);
            return true;
        }
        self.walk_pet(pos, dt);
        !self.pet_gives_up()
    }

    fn pet_digs(&mut self, dt: f32) -> bool {
        let power = self.derived.pet.dig;
        let Some((id, _)) = self.body.pet.digging.filter(|_| power > 0.0) else {
            return false;
        };
        let owner = self.body.pos;
        let Some(node) = self
            .nodes
            .iter()
            .find(|n| n.id == id && n.pos.dist(owner) <= DIG_RANGE)
            .map(|n| n.pos)
        else {
            self.body.pet.digging = None;
            return false;
        };
        // From whichever side it already is on.
        let side = if self.body.pet.pos.x < node.x {
            -1.0
        } else {
            1.0
        };
        let spot = node + v2(side * 11.0, 3.0);
        if self.body.pet.pos.dist(node) > 15.0 {
            self.walk_pet(spot, dt);
            return !self.pet_gives_up();
        }
        self.body.pet.facing = (node - self.body.pet.pos).norm();
        if self.body.pet.cooldown <= 0.0 {
            self.body.pet.cooldown = DIG_EVERY;
            self.body.pet.bite = BITE_SHOWS;
            self.pet_harvest(id, power);
        }
        true
    }
}
