//! How the player character looks. Purely cosmetic: every field is an index
//! into the colour and style tables of the presentation layer.

use crate::babel::seeds::Dice;
use serde::{Deserialize, Serialize};

pub const BODIES: u8 = 3;
pub const SKINS: u8 = 6;
pub const HAIRS: u8 = 7;
pub const HAIR_COLORS: u8 = 9;
pub const EYE_COLORS: u8 = 6;
pub const COATS: u8 = 8;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct Look {
    /// Gender presentation: man, woman or non-binary.
    pub body: u8,
    pub skin: u8,
    /// Hairstyle.
    pub hair: u8,
    pub hair_color: u8,
    pub eyes: u8,
    pub coat: u8,
    pub cape: bool,
    /// A mark on the forehead.
    pub brand: bool,
}

impl Default for Look {
    /// A storm-blessed spearman: tan skin, long dark waves and a blue coat.
    fn default() -> Self {
        Look {
            body: 0,
            skin: 3,
            hair: 0,
            hair_color: 0,
            eyes: 0,
            coat: 0,
            cape: true,
            brand: true,
        }
    }
}

impl Look {
    /// Wraps every index into range, so an edited save can never break the art.
    pub fn sanitized(self) -> Look {
        Look {
            body: self.body % BODIES,
            skin: self.skin % SKINS,
            hair: self.hair % HAIRS,
            hair_color: self.hair_color % HAIR_COLORS,
            eyes: self.eyes % EYE_COLORS,
            coat: self.coat % COATS,
            ..self
        }
    }

    pub fn random(dice: &mut Dice) -> Look {
        let mut pick = |n: u8| dice.below(u32::from(n)) as u8;
        Look {
            body: pick(BODIES),
            skin: pick(SKINS),
            hair: pick(HAIRS),
            hair_color: pick(HAIR_COLORS),
            eyes: pick(EYE_COLORS),
            coat: pick(COATS),
            cape: pick(2) == 1,
            brand: pick(4) == 0,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::game::profile::Profile;
    use crate::progression::player::PlayerData;

    #[test]
    fn saves_from_before_customisation_still_load() {
        let old: PlayerData = ron::from_str("(level: 7, xp: 3)").expect("old player data");
        assert_eq!(old.level, 7);
        assert_eq!(old.look, Look::default());
    }

    #[test]
    fn the_look_survives_a_save_roundtrip() {
        let mut profile = Profile::new(9);
        profile.player.look = Look::random(&mut Dice::from_u64(77));
        let text = ron::to_string(&profile).expect("serialise");
        let back = crate::save::parse(&text).expect("parse");
        assert_eq!(back.player.look, profile.player.look);
    }

    #[test]
    fn out_of_range_values_are_wrapped() {
        let wild = Look {
            body: 250,
            skin: 200,
            hair: 99,
            hair_color: 98,
            eyes: 97,
            coat: 96,
            cape: false,
            brand: false,
        }
        .sanitized();
        assert!(wild.body < BODIES && wild.skin < SKINS && wild.hair < HAIRS);
        assert!(wild.hair_color < HAIR_COLORS && wild.eyes < EYE_COLORS && wild.coat < COATS);
        let mut dice = Dice::from_u64(1);
        for _ in 0..200 {
            let look = Look::random(&mut dice);
            assert_eq!(look, look.sanitized());
        }
    }
}
