//! Equipment born from words.

use crate::babel::lexicon::{Category, WordId};
use crate::babel::rarity::Rarity;
use crate::babel::seeds::stable_hash;
use crate::data::{GameData, Slot};
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Item {
    pub uid: u64,
    pub name: String,
    pub slot: Slot,
    pub rarity: Rarity,
    /// Source words as written (stable across lexicon edits).
    pub words: Vec<String>,
    /// Effect tags granted by those words.
    pub effects: Vec<String>,
    #[serde(default)]
    pub inscriptions: Vec<Inscribed>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Inscribed {
    pub word: String,
    pub effect: String,
    pub rarity: Rarity,
}

/// Effect tag of a word: its own if it declares one, otherwise a stable pick,
/// so that every word in the Library means something.
pub fn word_effect(data: &GameData, word: &str) -> String {
    if let Some(entry) = data.lexicon.find(word)
        && let Some(tag) = entry
            .tags
            .iter()
            .find(|t| data.items.effects.iter().any(|e| &e.tag == *t))
    {
        return tag.clone();
    }
    let effects = &data.items.effects;
    let i = (stable_hash(word) % effects.len().max(1) as u64) as usize;
    effects.get(i).map_or_else(|| "fuerza".into(), |e| e.tag.clone())
}

fn capitalize(text: &str) -> String {
    let mut chars = text.chars();
    match chars.next() {
        Some(c) => c.to_uppercase().chain(chars).collect(),
        None => String::new(),
    }
}

/// Slot a word naturally belongs to.
pub fn word_slot(data: &GameData, word: &str) -> Slot {
    if let Some(slot) = data
        .lexicon
        .find(word)
        .and_then(|e| e.tag_value("slot"))
        .and_then(Slot::from_tag)
    {
        return slot;
    }
    Slot::ALL[(stable_hash(word) >> 8) as usize % Slot::ALL.len()]
}

fn base_noun(data: &GameData, slot: Slot) -> String {
    data.items
        .slots
        .iter()
        .find(|s| s.slot == slot)
        .map_or_else(|| slot.name().to_string(), |s| s.base_noun.clone())
}

/// Builds a name from the word: objects name themselves, qualities describe a
/// base item and anything else is "of" it.
fn item_name(data: &GameData, noun: &str, quality: Option<&str>, slot: Slot) -> String {
    let category = data.lexicon.find(noun).map(|e| e.category);
    let is_object = category == Some(Category::Objeto) && word_slot(data, noun) == slot;
    let head = if is_object {
        capitalize(noun)
    } else {
        base_noun(data, slot)
    };
    match quality {
        Some(q) if is_object => format!("{head} {q}"),
        Some(q) => format!("{head} de {noun} {q}"),
        None if is_object => head,
        None => match category {
            Some(Category::Cualidad) => format!("{head} {noun}"),
            Some(Category::Accion) => format!("{head} del verbo {noun}"),
            _ => format!("{head} de {noun}"),
        },
    }
}

/// The same word with the same rarity always yields the same item.
pub fn from_words(data: &GameData, uid: u64, noun: &str, quality: Option<&str>, rarity: Rarity) -> Item {
    let slot = word_slot(data, noun);
    let mut words = vec![noun.to_string()];
    words.extend(quality.map(str::to_string));
    let mut effects: Vec<String> = Vec::new();
    let wanted = rarity.effect_count();
    // Word effects first, then stable derived ones until the rarity is filled.
    for w in &words {
        if effects.len() < wanted {
            let e = word_effect(data, w);
            if !effects.contains(&e) {
                effects.push(e);
            }
        }
    }
    let pool = &data.items.effects;
    let mut h = stable_hash(&words.join(" "));
    while effects.len() < wanted && !pool.is_empty() {
        let candidate = &pool[(h % pool.len() as u64) as usize].tag;
        if !effects.contains(candidate) {
            effects.push(candidate.clone());
        }
        h = h.wrapping_mul(0x9E37_79B9_7F4A_7C15).rotate_left(17) ^ 0x5851_F42D;
    }
    Item {
        uid,
        name: item_name(data, noun, quality, slot),
        slot,
        rarity,
        words,
        effects,
        inscriptions: Vec::new(),
    }
}

/// A named object unlocked by completing a canonical phrase.
pub fn from_canon(data: &GameData, uid: u64, canon: usize, rarity: Rarity) -> Item {
    let def = &data.canon[canon.min(data.canon.len() - 1)];
    let words: Vec<String> = def.text.split(' ').map(str::to_string).collect();
    let first = words.first().cloned().unwrap_or_default();
    let mut item = from_words(data, uid, &first, words.get(1).map(String::as_str), rarity);
    item.name.clone_from(&def.name);
    item.slot = def.slot;
    item.words = words;
    item
}

/// An item of a fixed slot, used for guaranteed discovery rewards.
pub fn milestone_item(data: &GameData, uid: u64, word: &str, slot: Slot) -> Item {
    let mut item = from_words(data, uid, word, None, Rarity::Rare);
    item.slot = slot;
    item.name = item_name(data, word, None, slot);
    item
}

impl Item {
    pub fn effect_lines(&self, data: &GameData) -> Vec<String> {
        crate::progression::stats::item_stats(data, self)
            .iter()
            .map(|(s, v)| crate::progression::stats::describe(s, v))
            .collect()
    }

    /// Rough worth used to sort the inventory and to suggest upgrades.
    pub fn power(&self) -> f32 {
        self.rarity.stat_multiplier() + self.effects.len() as f32 * 0.3 + self.inscriptions.len() as f32 * 0.4
    }
}

/// Word ids useful for loot: everything that can name an object.
pub fn can_name_item(data: &GameData, word: WordId) -> bool {
    data.lexicon.get(word).category != Category::Orientacion
}
