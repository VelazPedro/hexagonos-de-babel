//! Rarity is decided by the word, never by luck.

use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub enum Rarity {
    Common,
    Uncommon,
    Rare,
    Epic,
    Legendary,
    Canonical,
}

impl Rarity {
    pub const ALL: [Rarity; 6] = [
        Rarity::Common,
        Rarity::Uncommon,
        Rarity::Rare,
        Rarity::Epic,
        Rarity::Legendary,
        Rarity::Canonical,
    ];

    pub fn label(self) -> &'static str {
        match self {
            Rarity::Common => "Común",
            Rarity::Uncommon => "Poco común",
            Rarity::Rare => "Raro",
            Rarity::Epic => "Épico",
            Rarity::Legendary => "Legendario",
            Rarity::Canonical => "Canónico",
        }
    }

    /// Glow colour as sRGB. Canonical is animated by the renderer; this is its base.
    pub fn color(self) -> [u8; 3] {
        match self {
            Rarity::Common => [200, 200, 214],
            Rarity::Uncommon => [88, 222, 110],
            Rarity::Rare => [76, 160, 255],
            Rarity::Epic => [186, 104, 255],
            Rarity::Legendary => [255, 196, 56],
            Rarity::Canonical => [240, 248, 255],
        }
    }

    /// Fixed stat multiplier table.
    pub fn stat_multiplier(self) -> f32 {
        match self {
            Rarity::Common => 1.0,
            Rarity::Uncommon => 1.25,
            Rarity::Rare => 1.6,
            Rarity::Epic => 2.05,
            Rarity::Legendary => 2.7,
            Rarity::Canonical => 3.4,
        }
    }

    /// How many word effects an item of this rarity carries.
    pub fn effect_count(self) -> usize {
        match self {
            Rarity::Common => 0,
            Rarity::Uncommon | Rarity::Rare => 1,
            Rarity::Epic => 2,
            Rarity::Legendary | Rarity::Canonical => 3,
        }
    }

    /// Maximum number of Scriptorium inscriptions.
    pub fn inscription_slots(self) -> usize {
        match self {
            Rarity::Common | Rarity::Uncommon => 1,
            Rarity::Rare | Rarity::Epic => 2,
            Rarity::Legendary | Rarity::Canonical => 3,
        }
    }

    pub fn tier(self) -> u32 {
        self as u32
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Context {
    pub in_phrase: bool,
    pub in_inscription: bool,
    pub canonical: bool,
    pub depth: u32,
}

pub fn length_bonus(word: &str) -> u32 {
    match word.chars().count() {
        n if n >= 10 => 2,
        n if n >= 7 => 1,
        _ => 0,
    }
}

pub fn score(base_rarity: u8, word: &str, ctx: Context) -> u32 {
    u32::from(base_rarity)
        + length_bonus(word)
        + u32::from(ctx.in_phrase)
        + u32::from(ctx.in_inscription)
        + ctx.depth / 10
}

pub fn from_score(score: u32) -> Rarity {
    match score {
        0..=2 => Rarity::Common,
        3 => Rarity::Uncommon,
        4 => Rarity::Rare,
        5 | 6 => Rarity::Epic,
        _ => Rarity::Legendary,
    }
}

pub fn rarity_of(base_rarity: u8, word: &str, ctx: Context) -> Rarity {
    if ctx.canonical {
        Rarity::Canonical
    } else {
        from_score(score(base_rarity, word, ctx))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn score_table() {
        let expected = [
            (0, Rarity::Common),
            (2, Rarity::Common),
            (3, Rarity::Uncommon),
            (4, Rarity::Rare),
            (5, Rarity::Epic),
            (6, Rarity::Epic),
            (7, Rarity::Legendary),
            (20, Rarity::Legendary),
        ];
        for (s, r) in expected {
            assert_eq!(from_score(s), r, "score {s}");
        }
    }

    #[test]
    fn bonuses_add_up() {
        let ctx = Context {
            in_phrase: true,
            in_inscription: true,
            canonical: false,
            depth: 25,
        };
        assert_eq!(length_bonus("lobo"), 0);
        assert_eq!(length_bonus("antiguo"), 1);
        assert_eq!(length_bonus("laberintico"), 2);
        assert_eq!(score(2, "antiguo", ctx), 2 + 1 + 1 + 1 + 2);
    }

    #[test]
    fn canonical_overrides_score() {
        let ctx = Context {
            canonical: true,
            ..Default::default()
        };
        assert_eq!(rarity_of(1, "sol", ctx), Rarity::Canonical);
    }

    #[test]
    fn same_word_same_context_same_rarity() {
        let ctx = Context {
            depth: 12,
            ..Default::default()
        };
        assert_eq!(
            rarity_of(3, "obsidiana", ctx),
            rarity_of(3, "obsidiana", ctx)
        );
    }
}
