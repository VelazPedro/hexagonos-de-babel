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

/// A player with everything needed to make every whetstone.
fn rich() -> PlayerData {
    let mut p = PlayerData::default();
    for resource in Resource::ALL {
        p.add_resource(resource, 500);
    }
    p
}

#[test]
fn whetstones_add_three_damage_each_and_go_in_order() {
    let d = test_data();
    let mut p = rich();
    let built = BTreeSet::from([Station::Forge]);
    let base = stats::derive(d, &p).damage;
    let make = |p: &mut PlayerData, id: &str| {
        let derived = stats::derive(d, p);
        crafting::craft(d.recipe(id).expect(id), p, &derived, &built, true)
    };
    assert_eq!(
        make(&mut p, "whet_2"),
        Err(crafting::CraftError::NeedPreviousTier)
    );
    assert_eq!(make(&mut p, "whet_1"), Ok(()));
    assert_eq!(stats::derive(d, &p).damage, base + 3.0);
    assert_eq!(
        make(&mut p, "whet_1"),
        Err(crafting::CraftError::AlreadyOwned)
    );
    for id in ["whet_2", "whet_3", "whet_4", "whet_5"] {
        assert_eq!(make(&mut p, id), Ok(()), "{id}");
    }
    assert_eq!(stats::derive(d, &p).damage, base + 15.0);
}

#[test]
fn whetstones_need_nothing_their_level_cannot_reach() {
    let d = test_data();
    for level in 1..=5u8 {
        let stone = d.recipe(&format!("whet_{level}")).expect("whetstone");
        for &(resource, _) in &stone.cost {
            let used_by_then = d.recipes.iter().any(|r| {
                matches!(r.output, crate::data::RecipeOutput::Tool(_, tier) if tier <= level)
                    && r.cost.iter().any(|c| c.0 == resource)
            });
            assert!(used_by_then, "whet_{level} asks for {resource:?} too early");
        }
    }
}

#[test]
fn old_saves_have_no_whetstones_and_edited_ones_are_clamped() {
    let d = test_data();
    let old: PlayerData = ron::from_str("(level: 7, xp: 3)").expect("old player data");
    assert_eq!(old.whetstones, 0);
    let edited = PlayerData {
        whetstones: 200,
        ..PlayerData::default()
    };
    assert_eq!(edited.sanitized(d).whetstones, 5);
}

#[test]
fn fights_are_shorter() {
    let d = test_data();
    assert!((d.enemies.curve.hp_per_floor - 0.11).abs() < 1e-6);
    let weapon = d
        .items
        .slots
        .iter()
        .find(|s| s.slot == crate::data::Slot::Weapon)
        .expect("weapon");
    assert_eq!(weapon.base, [(crate::data::Stat::Damage, 6.0)]);
    let guardians: Vec<f32> = d.enemies.guardians.iter().map(|g| g.hp).collect();
    assert_eq!(guardians, [336.0, 720.0, 1200.0, 1840.0, 2880.0, 4480.0]);
    // With one whetstone a hungry book on floor 1 falls to two blows.
    let book = &d.enemies.archetypes[d.archetype_index("libro")];
    let hp = book.hp * crate::game::actors::scaling(d, 1).hp;
    let damage = 7.0 + 3.0f32;
    assert!((hp - 19.2).abs() < 0.01, "{hp}");
    assert!(damage + (damage * 1.1).round() >= hp);
}
