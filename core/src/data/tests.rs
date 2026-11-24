use super::*;
use crate::babel::alphabet;
use crate::babel::lexicon::Category;
use std::collections::{HashMap, HashSet};

#[test]
fn lexicon_has_no_duplicates_and_meets_minimums() {
    let lex = &test_data().lexicon;
    assert!(lex.len() >= 900, "lexicon has {} words", lex.len());
    let mut seen = HashSet::new();
    for e in lex.entries() {
        assert!(seen.insert(e.word.clone()), "duplicate word {}", e.word);
        assert!(
            e.word.chars().all(alphabet::is_letter),
            "bad word {}",
            e.word
        );
        assert!((1..=5).contains(&e.base_rarity), "rarity of {}", e.word);
    }
    for c in Category::ALL {
        let n = lex.in_category(c).len();
        assert!(n >= c.minimum(), "{c:?} has {n}, needs {}", c.minimum());
    }
}

#[test]
fn lexicon_tags_reference_existing_content() {
    let d = test_data();
    let effects: HashSet<&str> = d.items.effects.iter().map(|e| e.tag.as_str()).collect();
    for e in d.lexicon.entries() {
        if let Some(b) = e.tag_value("biome") {
            assert!(
                d.biomes.iter().any(|x| x.id == b),
                "biome {b} of {}",
                e.word
            );
        }
        if let Some(a) = e.tag_value("arq") {
            assert!(
                d.enemies.archetypes.iter().any(|x| x.id == a),
                "archetype {a} of {}",
                e.word
            );
        }
        if let Some(m) = e.tag_value("mod") {
            assert!(d.modifier_index(m).is_some(), "modifier {m} of {}", e.word);
        }
        for t in e.tags.iter().filter(|t| !t.contains(':')) {
            assert!(effects.contains(t.as_str()), "effect tag {t} of {}", e.word);
        }
        match e.category {
            Category::Criatura => assert!(e.tag_value("arq").is_some(), "{}", e.word),
            Category::Bioma => assert!(e.tag_value("biome").is_some(), "{}", e.word),
            Category::Luz => assert!(e.tag_value("lv").is_some(), "{}", e.word),
            Category::Orientacion => assert!(
                e.tag_value("layout").is_some() || e.tag_value("exit").is_some(),
                "{}",
                e.word
            ),
            _ => {}
        }
    }
    let used_modifiers: HashSet<&str> = d
        .lexicon
        .entries()
        .iter()
        .filter_map(|e| e.tag_value("mod"))
        .collect();
    assert!(d.enemies.modifiers.len() >= 20);
    for m in &d.enemies.modifiers {
        assert!(
            used_modifiers.contains(m.id.as_str()),
            "no word grants {}",
            m.id
        );
    }
}

#[test]
fn canon_phrases_use_lexicon_words() {
    let d = test_data();
    assert!(d.canon.len() >= 60);
    let mut seen = HashSet::new();
    for c in &d.canon {
        assert!(seen.insert(&c.text), "duplicate canon {}", c.text);
        let words: Vec<&str> = c.text.split(' ').collect();
        assert!((2..=4).contains(&words.len()), "{}", c.text);
        for w in words {
            assert!(d.lexicon.id_of(w).is_some(), "canon word {w} missing");
        }
    }
}

#[test]
fn recipes_only_use_existing_ingredients_and_cover_every_tier() {
    let d = test_data();
    let mut ids = HashSet::new();
    for r in &d.recipes {
        assert!(ids.insert(&r.id), "duplicate recipe {}", r.id);
        assert!(!r.cost.is_empty(), "{}", r.id);
        for (res, n) in &r.cost {
            assert!(*n > 0, "{} {res:?}", r.id);
            // Every ingredient must be obtainable from a node or a recipe.
            let from_node = d
                .items
                .nodes
                .iter()
                .any(|node| node.yields.iter().any(|y| y.0 == *res && y.2 > 0));
            assert!(from_node, "{res:?} has no source");
        }
    }
    for tool in ToolKind::ALL {
        for tier in 1..=MAX_TIER {
            assert!(
                d.recipes
                    .iter()
                    .any(|r| r.output == RecipeOutput::Tool(tool, tier)),
                "{tool:?} tier {tier} has no recipe"
            );
        }
    }
    for s in Station::ALL {
        assert!(d.recipes.iter().any(|r| r.output == RecipeOutput::Build(s)));
    }
    for res in Resource::ALL {
        let used = d.recipes.iter().any(|r| r.cost.iter().any(|c| c.0 == res));
        assert!(used, "{res:?} is never used");
    }
}

#[test]
fn skill_tree_has_no_orphans_or_cycles() {
    let d = test_data();
    assert_eq!(d.skills.len(), 32);
    let by_id: HashMap<&str, &SkillDef> = d.skills.iter().map(|s| (s.id.as_str(), s)).collect();
    assert_eq!(by_id.len(), d.skills.len(), "duplicate skill id");
    for s in &d.skills {
        for r in &s.requires {
            let parent = by_id
                .get(r.as_str())
                .unwrap_or_else(|| panic!("missing {r}"));
            assert_eq!(parent.branch, s.branch, "{} crosses branches", s.id);
            assert!(parent.row < s.row, "{} does not descend from {r}", s.id);
        }
        assert_eq!(s.row == 0, s.requires.is_empty(), "orphan {}", s.id);
    }
    for b in Branch::ALL {
        assert_eq!(d.skills.iter().filter(|s| s.branch == b).count(), 8);
        assert_eq!(
            d.skills
                .iter()
                .filter(|s| s.branch == b && s.row == 0)
                .count(),
            1
        );
    }
    let mut cells = HashSet::new();
    for s in &d.skills {
        assert!(cells.insert((s.branch, s.row, s.col)), "overlap {}", s.id);
    }
}

#[test]
fn biomes_enemies_and_guardians_are_consistent() {
    let d = test_data();
    assert!(d.biomes.len() >= 10);
    assert_eq!(d.biomes[0].id, "canonica");
    assert_eq!(d.enemies.archetypes.len(), 10);
    let effects: HashSet<&str> = d.items.effects.iter().map(|e| e.tag.as_str()).collect();
    assert!(!effects.is_empty());
    for b in &d.biomes {
        assert!(d.lexicon.id_of(&b.milestone_word).is_some(), "{}", b.id);
        for c in &b.creatures {
            assert!(d.enemies.archetypes.iter().any(|a| &a.id == c), "{c}");
        }
    }
    let floors: Vec<u32> = d.enemies.guardians.iter().map(|g| g.floor).collect();
    assert_eq!(floors, [5, 10, 15, 20, 25, 30]);
    let mut relics = HashSet::new();
    for g in &d.enemies.guardians {
        assert!(relics.insert(&g.relic), "relic repeated {}", g.relic);
        assert!(d.items.relics.iter().any(|r| r.id == g.relic));
        assert!((2..=3).contains(&g.phases.len()), "{}", g.name);
        assert!(d.enemies.archetypes.iter().any(|a| a.id == g.minion));
    }
    assert_eq!(d.items.lamp_radius.len(), 6);
    assert_eq!(d.items.tool_power.len(), 6);
    for n in &d.items.nodes {
        assert!(n.hits > 0);
    }
}
