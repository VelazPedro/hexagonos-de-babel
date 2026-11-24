//! Format-preserving permutation over `[0, 30^80)`.
//!
//! A balanced Feistel network over 394 bits (two 197-bit halves) with a keyed blake3
//! round function, restricted to the target domain by cycle-walking.

use num_bigint::BigUint;

pub const HALF_BITS: u64 = 197;
pub const ROUNDS: usize = 10;
const TOP_LIMB_MASK: u64 = (1u64 << (HALF_BITS - 192)) - 1;

type Half = [u64; 4];

#[derive(Clone)]
pub struct Feistel {
    keys: [[u8; 32]; ROUNDS],
    domain: BigUint,
}

/// `30^80`: number of distinct inscriptions.
pub fn domain_size() -> BigUint {
    let mut n = BigUint::from(1u32);
    for _ in 0..80 {
        n *= 30u32;
    }
    n
}

fn split(x: &BigUint) -> (Half, Half) {
    let limbs = x.to_u64_digits();
    let get = |i: usize| limbs.get(i).copied().unwrap_or(0);
    let right = [get(0), get(1), get(2), get(3) & TOP_LIMB_MASK];
    let high = x >> HALF_BITS;
    let hl = high.to_u64_digits();
    let geth = |i: usize| hl.get(i).copied().unwrap_or(0);
    let left = [geth(0), geth(1), geth(2), geth(3) & TOP_LIMB_MASK];
    (left, right)
}

fn to_big(h: &Half) -> BigUint {
    let mut bytes = [0u8; 32];
    for (i, limb) in h.iter().enumerate() {
        bytes[i * 8..i * 8 + 8].copy_from_slice(&limb.to_le_bytes());
    }
    BigUint::from_bytes_le(&bytes)
}

fn join(left: &Half, right: &Half) -> BigUint {
    (to_big(left) << HALF_BITS) | to_big(right)
}

fn xor(a: &Half, b: &Half) -> Half {
    [a[0] ^ b[0], a[1] ^ b[1], a[2] ^ b[2], a[3] ^ b[3]]
}

impl Feistel {
    pub fn new(world_seed: u64) -> Self {
        let mut keys = [[0u8; 32]; ROUNDS];
        for (i, key) in keys.iter_mut().enumerate() {
            let mut material = [0u8; 16];
            material[..8].copy_from_slice(&world_seed.to_le_bytes());
            material[8..].copy_from_slice(&(i as u64).to_le_bytes());
            *key = blake3::derive_key("hexagonos-de-babel feistel round v1", &material);
        }
        Feistel {
            keys,
            domain: domain_size(),
        }
    }

    fn round(&self, i: usize, h: &Half) -> Half {
        let mut input = [0u8; 32];
        for (j, limb) in h.iter().enumerate() {
            input[j * 8..j * 8 + 8].copy_from_slice(&limb.to_le_bytes());
        }
        let out = blake3::keyed_hash(&self.keys[i], &input);
        let mut r = [0u64; 4];
        for (j, chunk) in out.as_bytes().chunks_exact(8).enumerate() {
            let mut b = [0u8; 8];
            b.copy_from_slice(chunk);
            r[j] = u64::from_le_bytes(b);
        }
        r[3] &= TOP_LIMB_MASK;
        r
    }

    fn permute(&self, x: &BigUint) -> BigUint {
        let (mut l, mut r) = split(x);
        for i in 0..ROUNDS {
            let next = xor(&l, &self.round(i, &r));
            l = r;
            r = next;
        }
        join(&l, &r)
    }

    fn unpermute(&self, y: &BigUint) -> BigUint {
        let (mut l, mut r) = split(y);
        for i in (0..ROUNDS).rev() {
            let prev = xor(&r, &self.round(i, &l));
            r = l;
            l = prev;
        }
        join(&l, &r)
    }

    /// `F`: maps text index to library address. Input must be `< 30^80`
    /// (larger values are reduced into the domain).
    pub fn forward(&self, x: &BigUint) -> BigUint {
        let start = if x < &self.domain {
            x.clone()
        } else {
            x % &self.domain
        };
        let mut y = self.permute(&start);
        while y >= self.domain {
            y = self.permute(&y);
        }
        y
    }

    /// `F⁻¹`: maps library address to text index.
    pub fn inverse(&self, y: &BigUint) -> BigUint {
        let start = if y < &self.domain {
            y.clone()
        } else {
            y % &self.domain
        };
        let mut x = self.unpermute(&start);
        while x >= self.domain {
            x = self.unpermute(&x);
        }
        x
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use proptest::prelude::*;

    #[test]
    fn domain_fits_in_394_bits() {
        assert!(domain_size().bits() <= 2 * HALF_BITS);
    }

    proptest! {
        #![proptest_config(ProptestConfig::with_cases(128))]

        #[test]
        fn inverse_undoes_forward(seed in any::<u64>(), bytes in proptest::collection::vec(any::<u8>(), 0..49)) {
            let f = Feistel::new(seed);
            let x = BigUint::from_bytes_le(&bytes) % domain_size();
            let y = f.forward(&x);
            prop_assert!(y < domain_size());
            prop_assert_eq!(f.inverse(&y), x.clone());
            prop_assert_eq!(f.forward(&f.inverse(&x)), x);
        }
    }
}
