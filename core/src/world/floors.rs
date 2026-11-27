//! Floor-level rules: where staircases are and which floors hold guardians.

use super::hex;
use crate::babel::seeds::{self, Dice};

pub const FINAL_FLOOR: u32 = 30;
pub const GUARDIAN_EVERY: u32 = 5;
const SPACING: i32 = 5;

/// Floors that ask for better tools to walk down to, and the tier. What each
/// tier is made of can always be gathered above its floor.
pub const GATES: [(u32, u8); 4] = [(4, 1), (8, 2), (13, 3), (18, 4)];

/// The knife and hammer tier asked for to walk down to `floor`.
pub fn gate(floor: u32) -> Option<u8> {
    GATES.iter().find(|g| g.0 == floor).map(|g| g.1)
}

pub fn is_guardian_floor(floor: u32) -> bool {
    floor > 0 && floor.is_multiple_of(GUARDIAN_EVERY)
}

/// The main staircases form a lattice, so one is always a few hexagons away.
pub fn stair_offset(world_seed: u64, floor: u32) -> (i32, i32) {
    let seed = seeds::site_seed(world_seed, floor, i32::MIN, i32::MIN);
    let mut dice = Dice::labelled(&seed, "stairs");
    // Never (0, 0), so the home hexagon is not a staircase.
    (dice.range(1, SPACING - 2), dice.range(1, SPACING - 2))
}

pub fn is_stair_site(world_seed: u64, floor: u32, q: i32, r: i32) -> bool {
    let (a, b) = stair_offset(world_seed, floor);
    (q.wrapping_sub(a)).rem_euclid(SPACING) == 0 && (r.wrapping_sub(b)).rem_euclid(SPACING) == 0
}

/// Closest lattice staircase to `(q, r)` by hex distance.
pub fn nearest_stairs(world_seed: u64, floor: u32, q: i32, r: i32) -> (i32, i32) {
    let (a, b) = stair_offset(world_seed, floor);
    let base_q = q - (q - a).rem_euclid(SPACING);
    let base_r = r - (r - b).rem_euclid(SPACING);
    let mut best = (base_q, base_r);
    for dq in 0..2 {
        for dr in 0..2 {
            let c = (base_q + dq * SPACING, base_r + dr * SPACING);
            if hex::distance((q, r), c) < hex::distance((q, r), best) {
                best = c;
            }
        }
    }
    best
}

pub fn is_home(floor: u32, q: i32, r: i32) -> bool {
    floor == 1 && q == 0 && r == 0
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stairs_are_always_close_and_never_at_home() {
        for seed in 0..20u64 {
            for floor in 1..40 {
                assert!(!is_stair_site(seed, floor, 0, 0));
                for (q, r) in [(0, 0), (7, -3), (-13, 4)] {
                    let s = nearest_stairs(seed, floor, q, r);
                    assert!(is_stair_site(seed, floor, s.0, s.1));
                    assert!(hex::distance((q, r), s) <= 6);
                }
            }
        }
    }
}
