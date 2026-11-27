//! The pet that goes along: its name, its looks and what it has learned. The
//! player's name is cleaned here too, since both follow the same rules.

use crate::babel::seeds::Dice;
use crate::data::PetPerk;
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;

/// Characters a name may have, so that its label stays small.
pub const NAME_LIMIT: usize = 12;

pub const FURS: u8 = 7;
pub const EARS: u8 = 3;
pub const TAILS: u8 = 3;
pub const PET_EYES: u8 = 5;
/// The first one is no collar.
pub const COLLARS: u8 = 6;

/// Whether a name may contain `c`.
pub fn name_char(c: char) -> bool {
    c.is_ascii_alphanumeric() || "áéíóúüñÁÉÍÓÚÜÑ -'".contains(c)
}

/// A name as it is kept and shown: only allowed characters, single spaces and
/// no more than [`NAME_LIMIT`] of them. May be empty.
pub fn clean_name(raw: &str) -> String {
    let allowed: String = raw.chars().filter(|&c| name_char(c)).collect();
    let single = allowed.split_whitespace().collect::<Vec<_>>().join(" ");
    let cut: String = single.chars().take(NAME_LIMIT).collect();
    cut.trim_end().to_string()
}

/// Cosmetic choices. All zeros is the pet everyone starts with: short tawny
/// fur, big upright ears, dark eyes and a tail with a dark tip.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct PetLook {
    pub fur: u8,
    pub ears: u8,
    pub tail: u8,
    pub eyes: u8,
    pub collar: u8,
}

impl PetLook {
    /// Wraps every index into range, so an edited save can never break the art.
    pub fn sanitized(self) -> PetLook {
        PetLook {
            fur: self.fur % FURS,
            ears: self.ears % EARS,
            tail: self.tail % TAILS,
            eyes: self.eyes % PET_EYES,
            collar: self.collar % COLLARS,
        }
    }

    pub fn random(dice: &mut Dice) -> PetLook {
        let mut pick = |n: u8| dice.below(u32::from(n)) as u8;
        PetLook {
            fur: pick(FURS),
            ears: pick(EARS),
            tail: pick(TAILS),
            eyes: pick(PET_EYES),
            collar: pick(COLLARS),
        }
    }
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct PetData {
    pub name: String,
    pub look: PetLook,
    pub skills: BTreeSet<String>,
}

/// What the pet can do, added up from its skills. All zeros: it only follows.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct PetPowers {
    /// Share of the owner's damage each bite deals.
    pub bite: f32,
    /// How much faster it bites again.
    pub haste: f32,
    /// Above zero it leaps from farther and pushes what it bites.
    pub pounce: f32,
    /// How far from the owner, in pixels, it goes to fetch.
    pub fetch: f32,
    /// Share of a harvesting hit it adds.
    pub dig: f32,
}

impl PetPowers {
    pub(crate) fn add(&mut self, perk: PetPerk, value: f32) {
        let slot = match perk {
            PetPerk::Bite => &mut self.bite,
            PetPerk::BiteHaste => &mut self.haste,
            PetPerk::Pounce => &mut self.pounce,
            PetPerk::Fetch => &mut self.fetch,
            PetPerk::Dig => &mut self.dig,
        };
        *slot += value;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::data::PetBranch;
    use crate::data::test_data;
    use crate::progression::player::PlayerData;
    use crate::progression::stats;

    #[test]
    fn old_saves_get_the_default_pet_and_no_name() {
        let old: PlayerData = ron::from_str("(level: 7, xp: 3)").expect("old player data");
        assert_eq!(old.name, "");
        assert_eq!(old.pet, PetData::default());
        assert_eq!(old.pet.look, PetLook::default());
    }

    #[test]
    fn names_are_cleaned() {
        assert_eq!(clean_name("  José   Ñandú  "), "José Ñandú");
        assert_eq!(clean_name("a\u{7}b🐶\nc"), "abc");
        assert_eq!(clean_name("   "), "");
        assert_eq!(clean_name(&"x".repeat(40)).chars().count(), NAME_LIMIT);
        // The cut never leaves a space hanging.
        assert_eq!(clean_name("abcdefghijk lmn"), "abcdefghijk");
    }

    #[test]
    fn pet_skills_cost_the_heros_points_and_need_the_one_before() {
        let data = test_data();
        let mut p = PlayerData {
            skill_points: 1,
            ..PlayerData::default()
        };
        assert!(!p.learn_pet(data, "m_agil"));
        assert!(p.learn_pet(data, "m_mordisco"));
        assert_eq!(p.skill_points, 0);
        assert!(!p.learn(data, "c_pulso"));
        assert!(!p.learn_pet(data, "m_mordisco"));
        assert!(!p.learn_pet(data, "no_existe"));
        assert!(p.skills.is_empty());
    }

    #[test]
    fn a_pet_without_skills_has_no_powers_and_skills_add_them() {
        let data = test_data();
        let mut p = PlayerData::default();
        let before = stats::derive(data, &p);
        assert_eq!(before.pet, PetPowers::default());
        p.skill_points = 10;
        for id in ["m_mordisco", "m_agil", "m_fuerte", "m_trote"] {
            assert!(p.learn_pet(data, id), "{id}");
        }
        let after = stats::derive(data, &p);
        assert!((after.pet.bite - 0.42).abs() < 1e-5);
        assert!((after.pet.haste - 0.30).abs() < 1e-5);
        assert!(after.speed > before.speed);
    }

    #[test]
    fn pet_tree_is_complete() {
        let data = test_data();
        assert_eq!(data.pet_skills.len(), 10);
        let ids: BTreeSet<&str> = data.pet_skills.iter().map(|s| s.id.as_str()).collect();
        assert_eq!(ids.len(), 10, "duplicate pet skill id");
        for s in &data.pet_skills {
            // Ids never clash with the hero's: both name icons the same way.
            assert!(data.skill(&s.id).is_none(), "{}", s.id);
            for r in &s.requires {
                let parent = data.pet_skill(r).unwrap_or_else(|| panic!("missing {r}"));
                assert_eq!(parent.branch, s.branch, "{} crosses branches", s.id);
                assert!(parent.row < s.row, "{} does not descend from {r}", s.id);
            }
            assert_eq!(s.row == 0, s.requires.is_empty(), "orphan {}", s.id);
        }
        let in_branch = |b| data.pet_skills.iter().filter(|s| s.branch == b).count();
        assert_eq!(PetBranch::ALL.map(in_branch), [4, 3, 3], "fang, nose, bond");
    }

    #[test]
    fn sanitized_cleans_names_and_forgets_unknown_pet_skills() {
        let data = test_data();
        let mut p = PlayerData {
            name: format!("\u{1b}[31m{}", "A".repeat(5000)),
            ..PlayerData::default()
        };
        p.pet.name = "  \n ".into();
        p.pet.look = PetLook {
            fur: 250,
            ears: 99,
            tail: 98,
            eyes: 97,
            collar: 96,
        };
        p.pet.skills.insert("m_mordisco".into());
        p.pet.skills.insert("inventada".into());
        p.pet.skills.insert("c_pulso".into());
        let tidy = p.sanitized(data);
        assert_eq!(tidy.name.chars().count(), NAME_LIMIT);
        assert!(tidy.name.chars().all(name_char));
        assert_eq!(tidy.pet.name, "");
        assert_eq!(tidy.pet.look, tidy.pet.look.sanitized());
        assert_eq!(
            tidy.pet.skills.into_iter().collect::<Vec<_>>(),
            ["m_mordisco"]
        );
        let mut dice = Dice::from_u64(3);
        for _ in 0..200 {
            let look = PetLook::random(&mut dice);
            assert_eq!(look, look.sanitized());
        }
    }
}
