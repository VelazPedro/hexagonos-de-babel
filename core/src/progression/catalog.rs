//! The Catalog: pay Indices to travel to the site whose inscription is your phrase.

use crate::babel::alphabet;
use crate::babel::detect::{Analysis, Place};
use crate::babel::inscription::LENGTH;
use crate::babel::rarity;
use crate::data::GameData;

pub const SEAL_RELIC: &str = "sello_catalogo";

/// Canonical form of a query: normalised alphabet, trimmed, at most 80 symbols.
pub fn sanitize(raw: &str) -> String {
    let text: String = alphabet::normalize(raw).chars().take(LENGTH).collect();
    text.trim().to_string()
}

/// Price in Indices: grows with the summed rarity of the searched words.
pub fn index_cost(data: &GameData, phrase: &str) -> u32 {
    let mut found = Analysis::default();
    data.detector
        .analyze_line(&data.lexicon, phrase, Place::Inscription, 1, &mut found);
    let total: u32 = found
        .hits
        .iter()
        .map(|h| {
            let e = data.lexicon.get(h.word);
            u32::from(e.base_rarity) + rarity::length_bonus(&e.word)
        })
        .sum();
    1 + total / 5
}
