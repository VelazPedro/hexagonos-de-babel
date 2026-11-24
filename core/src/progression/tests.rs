use super::player::{INVENTORY_SIZE, PlayerData, xp_to_next};
use super::{catalog, crafting, inscribe, items, reading, stats};
use crate::babel::rarity::Rarity;
use crate::data::{Resource, Station, ToolKind, Unlock, test_data};
use crate::world::site::{self, SiteKey};
use std::collections::BTreeSet;

#[test]
fn word_items_are_deterministic_and_scale_with_rarity() {
    let d = test_data();
    let a = items::from_words(d, 1, "daga", Some("ardiente"), Rarity::Epic);
    let b = items::from_words(d, 1, "daga", Some("ardiente"), Rarity::Epic);
    assert_eq!(a, b);
    assert_eq!(a.name, "Daga ardiente");
    assert_eq!(a.effects.len(), Rarity::Epic.effect_count());
    let common = items::from_words(d, 2, "daga", None, Rarity::Common);
    assert!(common.effects.is_empty());
    let sum = |i: &items::Item| stats::item_stats(d, i).iter().map(|s| s.1).sum::<f32>();
    assert!(sum(&a) > sum(&common));
}

#[test]
fn every_word_has_an_effect() {
    let d = test_data();
    for e in d.lexicon.entries() {
        let tag = items::word_effect(d, &e.word);
        assert!(d.items.effects.iter().any(|x| x.tag == tag), "{}", e.word);
    }
}

#[test]
fn leveling_grants_one_point_per_level_and_skills_respect_requirements() {
    let d = test_data();
    let mut p = PlayerData::default();
    assert_eq!(p.gain_xp(xp_to_next(1) + xp_to_next(2)), 2);
    assert_eq!((p.level, p.skill_points), (3, 2));
    assert!(!p.learn(d, "e_esquiva"), "parent not learned");
    assert!(p.learn(d, "e_paso"));
    assert!(p.learn(d, "e_esquiva"));
    assert!(stats::derive(d, &p).has(Unlock::Dash));
    assert!(!p.learn(d, "a_manos"), "no points left");
}

#[test]
fn crafting_is_atomic_and_ordered() {
    let d = test_data();
    let mut p = PlayerData::default();
    let derived = stats::derive(d, &p);
    let mut built = BTreeSet::new();
    let hammer2 = d.recipe("hammer_2").expect("recipe");
    let hammer1 = d.recipe("hammer_1").expect("recipe");
    assert_eq!(
        crafting::check(hammer1, &p, &derived, &built, true),
        Err(crafting::CraftError::NeedStation(Station::Forge))
    );
    built.insert(Station::Forge);
    assert_eq!(
        crafting::check(hammer2, &p, &derived, &built, true),
        Err(crafting::CraftError::NeedPreviousTier)
    );
    p.add_resource(Resource::Iron, 5);
    assert_eq!(
        crafting::craft(hammer1, &mut p, &derived, &built, true),
        Err(crafting::CraftError::MissingResources)
    );
    assert_eq!(p.resource(Resource::Iron), 5);
    p.add_resource(Resource::Stone, 6);
    assert!(crafting::craft(hammer1, &mut p, &derived, &built, true).is_ok());
    assert_eq!(p.tool_tier(ToolKind::Hammer), 1);
    assert_eq!(p.resource(Resource::Iron), 0);
}

#[test]
fn reading_fills_the_glossary_once_and_inscribing_spends_ink() {
    let d = test_data();
    let mut p = PlayerData::default();
    let derived = stats::derive(d, &p);
    let s = site::generate(d, 8, &SiteKey::walk(12, 2, 2));
    let first = reading::read(d, &s.analysis, &mut p, &derived, 3);
    assert!(!first.new_words.is_empty() && first.xp > 0);
    let again = reading::read(d, &s.analysis, &mut p, &derived, 3);
    assert!(again.new_words.is_empty());
    assert_eq!(again.xp, 0);

    let word = p.glossary.keys().next().cloned().expect("word");
    p.receive_item(items::from_words(d, 5, "daga", None, Rarity::Common));
    let target = inscribe::Target::Equipped(0);
    assert!(matches!(
        inscribe::inscribe(d, &mut p, &derived, target, &word),
        Err(inscribe::InscribeError::NotEnoughInk(_))
    ));
    p.add_resource(Resource::Ink, 100);
    assert!(inscribe::inscribe(d, &mut p, &derived, target, &word).is_ok());
    assert!(p.resource(Resource::Ink) < 100);
    assert_eq!(
        inscribe::inscribe(d, &mut p, &derived, target, &word),
        Err(inscribe::InscribeError::Duplicate)
    );
}

#[test]
fn full_inventory_recycles_instead_of_losing_value() {
    let d = test_data();
    let mut p = PlayerData::default();
    for i in 0..(INVENTORY_SIZE as u64 + 8) {
        p.receive_item(items::from_words(d, i, "daga", None, Rarity::Common));
    }
    assert_eq!(p.inventory.len(), INVENTORY_SIZE);
    assert!(p.resource(Resource::Ink) >= 7);
}

#[test]
fn catalog_cost_grows_with_word_rarity() {
    let d = test_data();
    assert_eq!(catalog::sanitize("  ¡Lobo SOMBRÍO! "), "lobo sombrio");
    let cheap = catalog::index_cost(d, "sol");
    let dear = catalog::index_cost(d, "basilisco incandescencia septentrion");
    assert!(dear > cheap, "{dear} vs {cheap}");
}
