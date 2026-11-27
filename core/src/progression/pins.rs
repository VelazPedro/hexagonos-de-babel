//! Pinned recipes: what the player is saving up for, and how much is still
//! missing, so it can be seen while out gathering.

use super::crafting;
use super::player::PlayerData;
use super::stats::Derived;
use crate::data::{GameData, RecipeOutput, Resource, Station};
use std::collections::BTreeSet;

/// As many as fit on the screen while playing.
pub const MAX_PINS: usize = 3;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PinResult {
    Pinned,
    Unpinned,
    /// There are already [`MAX_PINS`].
    Full,
    Unknown,
}

/// Whether a recipe is one made from the crafting or building screens.
pub fn can_pin(data: &GameData, recipe: &str) -> bool {
    data.recipe(recipe)
        .is_some_and(|r| r.output != RecipeOutput::ReturnLamp)
}

/// How far one pinned recipe is from being made.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Tracked {
    pub id: String,
    pub name: String,
    /// What is still short, and how much of it.
    pub missing: Vec<(Resource, u32)>,
    /// A station that has to be built first.
    pub station: Option<Station>,
}

impl Tracked {
    /// Only the trip home is left.
    pub fn ready(&self) -> bool {
        self.missing.is_empty() && self.station.is_none()
    }
}

impl PlayerData {
    /// Pins a recipe, or lets it go if it was pinned.
    pub fn toggle_pin(&mut self, data: &GameData, recipe: &str) -> PinResult {
        if let Some(at) = self.pins.iter().position(|p| p == recipe) {
            self.pins.remove(at);
            PinResult::Unpinned
        } else if !can_pin(data, recipe) {
            PinResult::Unknown
        } else if self.pins.len() >= MAX_PINS {
            PinResult::Full
        } else {
            self.pins.push(recipe.to_string());
            PinResult::Pinned
        }
    }

    pub fn pinned(&self, recipe: &str) -> bool {
        self.pins.iter().any(|p| p == recipe)
    }
}

/// The pinned recipes in the order they were pinned, measured against what the
/// player carries: that is what crafting spends.
pub fn tracker(
    data: &GameData,
    player: &PlayerData,
    derived: &Derived,
    stations: &BTreeSet<Station>,
) -> Vec<Tracked> {
    player
        .pins
        .iter()
        .filter_map(|id| data.recipe(id))
        .map(|recipe| Tracked {
            id: recipe.id.clone(),
            name: recipe.name.clone(),
            missing: crafting::effective_cost(recipe, derived)
                .into_iter()
                .filter_map(|(resource, need)| {
                    let short = need.saturating_sub(player.resource(resource));
                    (short > 0).then_some((resource, short))
                })
                .collect(),
            station: recipe.station.filter(|s| !stations.contains(s)),
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::data::test_data;
    use crate::progression::stats;

    fn look(player: &PlayerData, stations: &[Station]) -> Vec<Tracked> {
        let d = test_data();
        let built = stations.iter().copied().collect();
        tracker(d, player, &stats::derive(d, player), &built)
    }

    #[test]
    fn pins_toggle_and_stop_at_three() {
        let d = test_data();
        let mut p = PlayerData::default();
        assert_eq!(p.toggle_pin(d, "knife_1"), PinResult::Pinned);
        assert!(p.pinned("knife_1"));
        assert_eq!(p.toggle_pin(d, "knife_1"), PinResult::Unpinned);
        assert!(!p.pinned("knife_1"));
        for id in ["knife_1", "hammer_1", "torch"] {
            assert_eq!(p.toggle_pin(d, id), PinResult::Pinned, "{id}");
        }
        assert_eq!(p.toggle_pin(d, "tonic"), PinResult::Full);
        assert_eq!(p.pins, ["knife_1", "hammer_1", "torch"]);
        assert_eq!(p.toggle_pin(d, "hammer_1"), PinResult::Unpinned);
        assert_eq!(p.toggle_pin(d, "tonic"), PinResult::Pinned);
        assert_eq!(p.toggle_pin(d, "no_existe"), PinResult::Unknown);
        assert_eq!(p.toggle_pin(d, "return_lamp"), PinResult::Unknown);
    }

    #[test]
    fn the_tracker_says_exactly_what_is_still_missing() {
        let d = test_data();
        let mut p = PlayerData::default();
        p.toggle_pin(d, "knife_1");
        p.add_resource(Resource::Iron, 1);
        p.add_resource(Resource::Leather, 9);
        let seen = look(&p, &[Station::Binder]);
        assert_eq!(seen.len(), 1);
        assert_eq!(seen[0].name, "Cuchilla de Hierro");
        assert_eq!(seen[0].missing, [(Resource::Iron, 3)]);
        assert_eq!(seen[0].station, None);
        assert!(!seen[0].ready());
        p.add_resource(Resource::Iron, 3);
        assert!(look(&p, &[Station::Binder])[0].ready());
        // Without the bench there is still something to do.
        let without = &look(&p, &[])[0];
        assert_eq!(without.station, Some(Station::Binder));
        assert!(!without.ready());
    }

    #[test]
    fn the_crafting_discount_counts() {
        let d = test_data();
        let mut p = PlayerData::default();
        p.toggle_pin(d, "hammer_1");
        let before = look(&p, &[Station::Forge])[0].missing.clone();
        p.skills.insert("a_ahorro".into());
        p.skills.insert("a_ahorro2".into());
        let after = look(&p, &[Station::Forge])[0].missing.clone();
        let total = |m: &[(Resource, u32)]| m.iter().map(|x| x.1).sum::<u32>();
        assert!(total(&after) < total(&before), "{before:?} {after:?}");
    }

    #[test]
    fn old_saves_pin_nothing_and_edited_pins_are_cleaned() {
        let d = test_data();
        let old: PlayerData = ron::from_str("(level: 2)").expect("old player data");
        assert!(old.pins.is_empty());
        let edited = PlayerData {
            pins: [
                "torch",
                "torch",
                "inventada",
                "return_lamp",
                "knife_1",
                "tonic",
                "ink",
            ]
            .map(String::from)
            .to_vec(),
            ..PlayerData::default()
        };
        assert_eq!(edited.sanitized(d).pins, ["torch", "knife_1", "tonic"]);
    }
}
