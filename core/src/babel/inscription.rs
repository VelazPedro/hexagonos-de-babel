//! The 80-character inscription: a bijection between library addresses and text.

use super::alphabet;
use super::feistel::Feistel;
use num_bigint::BigUint;
use num_traits::{ToPrimitive, Zero};

pub const LENGTH: usize = 80;

pub fn zigzag(v: i32) -> u32 {
    ((v << 1) ^ (v >> 31)) as u32
}

pub fn unzigzag(v: u32) -> i32 {
    ((v >> 1) as i32) ^ -((v & 1) as i32)
}

/// Packs a walkable site into a library address: `floor:32 | zz(q):32 | zz(r):32`.
pub fn pack(floor: u32, q: i32, r: i32) -> BigUint {
    (BigUint::from(floor) << 64u32) | (BigUint::from(zigzag(q)) << 32u32) | BigUint::from(zigzag(r))
}

/// Returns `(floor, q, r)` when the address belongs to a walkable site.
pub fn unpack(address: &BigUint) -> Option<(u32, i32, i32)> {
    if address.bits() > 96 {
        return None;
    }
    let mask = BigUint::from(u32::MAX);
    let floor = (address >> 64u32).to_u32()?;
    let q = ((address >> 32u32) & &mask).to_u32()?;
    let r = (address & &mask).to_u32()?;
    if floor == 0 {
        return None;
    }
    Some((floor, unzigzag(q), unzigzag(r)))
}

/// Text index → 80 symbols, most significant first.
pub fn decode_base30(mut x: BigUint) -> String {
    let mut digits = [alphabet::SPACE; LENGTH];
    let base = BigUint::from(alphabet::SIZE);
    for slot in digits.iter_mut().rev() {
        if x.is_zero() {
            *slot = 0;
            continue;
        }
        *slot = (&x % &base).to_u8().unwrap_or(0);
        x /= &base;
    }
    alphabet::decode(&digits)
}

/// 80-symbol text → index. Shorter phrases are padded with spaces.
pub fn encode_base30(text: &str) -> Option<BigUint> {
    let symbols = alphabet::encode(text)?;
    if symbols.len() > LENGTH {
        return None;
    }
    let mut x = BigUint::zero();
    for i in 0..LENGTH {
        let s = symbols.get(i).copied().unwrap_or(alphabet::SPACE);
        x = x * alphabet::SIZE + u32::from(s);
    }
    Some(x)
}

pub fn inscription_for(f: &Feistel, address: &BigUint) -> String {
    decode_base30(f.inverse(address))
}

/// Library address whose inscription is exactly `phrase` (padded with spaces).
pub fn address_of(f: &Feistel, phrase: &str) -> Option<BigUint> {
    encode_base30(phrase).map(|x| f.forward(&x))
}

#[cfg(test)]
mod tests {
    use super::*;
    use proptest::prelude::*;

    #[test]
    fn pack_roundtrip() {
        for (f, q, r) in [(1, 0, 0), (30, -5, 7), (u32::MAX, i32::MIN, i32::MAX)] {
            assert_eq!(unpack(&pack(f, q, r)), Some((f, q, r)));
        }
    }

    #[test]
    fn walkable_sites_have_distinct_inscriptions() {
        let f = Feistel::new(99);
        let a = inscription_for(&f, &pack(1, 0, 0));
        let b = inscription_for(&f, &pack(1, 1, 0));
        assert_eq!(a.chars().count(), LENGTH);
        assert_ne!(a, b);
        assert_eq!(address_of(&f, &a), Some(pack(1, 0, 0)));
    }

    proptest! {
        #![proptest_config(ProptestConfig::with_cases(96))]

        #[test]
        fn phrase_to_address_to_phrase(seed in any::<u64>(), phrase in "[a-zñ ,.]{0,80}") {
            let f = Feistel::new(seed);
            let address = address_of(&f, &phrase).expect("valid phrase");
            let text = inscription_for(&f, &address);
            let padded = format!("{phrase}{}", " ".repeat(LENGTH - phrase.chars().count()));
            prop_assert_eq!(text, padded);
        }
    }
}
