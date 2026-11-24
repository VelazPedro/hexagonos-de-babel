//! Stable seed derivation (blake3) and a deterministic dice built on ChaCha8.
//!
//! All sampling helpers are implemented here on top of raw `next_u32`/`next_u64`
//! so generation output never changes with distribution changes in third-party crates.

use rand_chacha::ChaCha8Rng;
use rand_chacha::rand_core::{Rng, SeedableRng};

pub type Seed = [u8; 32];

pub fn site_seed(world_seed: u64, floor: u32, q: i32, r: i32) -> Seed {
    let mut buf = [0u8; 20];
    buf[..8].copy_from_slice(&world_seed.to_le_bytes());
    buf[8..12].copy_from_slice(&floor.to_le_bytes());
    buf[12..16].copy_from_slice(&q.to_le_bytes());
    buf[16..20].copy_from_slice(&r.to_le_bytes());
    *blake3::hash(&buf).as_bytes()
}

/// Seed for a Catalog (remote) site identified by its library address.
pub fn remote_seed(world_seed: u64, address: &[u8]) -> Seed {
    let mut h = blake3::Hasher::new();
    h.update(b"remote");
    h.update(&world_seed.to_le_bytes());
    h.update(address);
    *h.finalize().as_bytes()
}

/// Labelled sub-seed so that changing one subsystem never perturbs the others.
pub fn sub_seed(seed: &Seed, label: &str) -> Seed {
    *blake3::keyed_hash(seed, label.as_bytes()).as_bytes()
}

/// Stable 64-bit hash of arbitrary text (for per-word variants).
pub fn stable_hash(text: &str) -> u64 {
    let h = blake3::hash(text.as_bytes());
    let mut b = [0u8; 8];
    b.copy_from_slice(&h.as_bytes()[..8]);
    u64::from_le_bytes(b)
}

#[derive(Clone, Debug)]
pub struct Dice(ChaCha8Rng);

impl Dice {
    pub fn new(seed: Seed) -> Self {
        Dice(ChaCha8Rng::from_seed(seed))
    }

    pub fn labelled(seed: &Seed, label: &str) -> Self {
        Dice::new(sub_seed(seed, label))
    }

    pub fn from_u64(v: u64) -> Self {
        Dice::new(*blake3::hash(&v.to_le_bytes()).as_bytes())
    }

    pub fn next_u32(&mut self) -> u32 {
        self.0.next_u32()
    }

    pub fn next_u64(&mut self) -> u64 {
        self.0.next_u64()
    }

    /// Uniform integer in `[0, n)`. Returns 0 when `n == 0`.
    pub fn below(&mut self, n: u32) -> u32 {
        ((u64::from(self.next_u32()) * u64::from(n)) >> 32) as u32
    }

    pub fn index(&mut self, len: usize) -> usize {
        self.below(len as u32) as usize
    }

    /// Uniform integer in `[lo, hi]`.
    pub fn range(&mut self, lo: i32, hi: i32) -> i32 {
        if hi <= lo {
            return lo;
        }
        lo + self.below((hi - lo + 1) as u32) as i32
    }

    /// Uniform float in `[0, 1)`.
    pub fn unit(&mut self) -> f32 {
        (self.next_u32() >> 8) as f32 / (1u32 << 24) as f32
    }

    pub fn between(&mut self, lo: f32, hi: f32) -> f32 {
        lo + (hi - lo) * self.unit()
    }

    pub fn chance(&mut self, p: f32) -> bool {
        self.unit() < p
    }

    pub fn pick<'a, T>(&mut self, items: &'a [T]) -> Option<&'a T> {
        if items.is_empty() {
            None
        } else {
            items.get(self.index(items.len()))
        }
    }

    /// Index sampled proportionally to integer weights. `None` if all weights are zero.
    pub fn weighted(&mut self, weights: &[u32]) -> Option<usize> {
        let total: u32 = weights.iter().sum();
        if total == 0 {
            return None;
        }
        let mut roll = self.below(total);
        for (i, &w) in weights.iter().enumerate() {
            if roll < w {
                return Some(i);
            }
            roll -= w;
        }
        None
    }

    pub fn shuffle<T>(&mut self, items: &mut [T]) {
        for i in (1..items.len()).rev() {
            let j = self.index(i + 1);
            items.swap(i, j);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn labels_are_independent_and_stable() {
        let s = site_seed(7, 1, 2, -3);
        assert_ne!(sub_seed(&s, "text"), sub_seed(&s, "layout"));
        assert_eq!(site_seed(7, 1, 2, -3), s);
        assert_ne!(site_seed(7, 2, 2, -3), s);
    }

    #[test]
    fn below_is_in_range() {
        let mut d = Dice::from_u64(1);
        for _ in 0..1000 {
            assert!(d.below(7) < 7);
            let r = d.range(-3, 3);
            assert!((-3..=3).contains(&r));
            let u = d.unit();
            assert!((0.0..1.0).contains(&u));
        }
    }
}
