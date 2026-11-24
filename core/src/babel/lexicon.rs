//! The curated Spanish lexicon.

use super::seeds::stable_hash;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Category {
    Bioma,
    Luz,
    Orientacion,
    Criatura,
    Material,
    Objeto,
    Cualidad,
    Accion,
}

impl Category {
    pub const ALL: [Category; 8] = [
        Category::Bioma,
        Category::Luz,
        Category::Orientacion,
        Category::Criatura,
        Category::Material,
        Category::Objeto,
        Category::Cualidad,
        Category::Accion,
    ];

    pub fn label(self) -> &'static str {
        match self {
            Category::Bioma => "Bioma",
            Category::Luz => "Luz",
            Category::Orientacion => "Orientación",
            Category::Criatura => "Criatura",
            Category::Material => "Material",
            Category::Objeto => "Objeto",
            Category::Cualidad => "Cualidad",
            Category::Accion => "Acción",
        }
    }

    /// Minimum number of entries required by the design.
    pub fn minimum(self) -> usize {
        match self {
            Category::Bioma => 60,
            Category::Luz => 40,
            Category::Orientacion => 50,
            Category::Criatura => 120,
            Category::Material => 100,
            Category::Objeto => 150,
            Category::Cualidad => 200,
            Category::Accion => 120,
        }
    }

    pub fn is_noun_for_phrase(self) -> bool {
        matches!(self, Category::Criatura | Category::Objeto)
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Entry {
    pub word: String,
    pub category: Category,
    #[serde(default)]
    pub tags: Vec<String>,
    pub base_rarity: u8,
    #[serde(default)]
    pub palette_hint: Option<String>,
}

impl Entry {
    /// Value of the first tag shaped like `key:value`.
    pub fn tag_value(&self, key: &str) -> Option<&str> {
        self.tags
            .iter()
            .find_map(|t| t.split_once(':').filter(|(k, _)| *k == key).map(|(_, v)| v))
    }

    pub fn has_tag(&self, tag: &str) -> bool {
        self.tags.iter().any(|t| t == tag)
    }

    pub fn variant_hash(&self) -> u64 {
        stable_hash(&self.word)
    }
}

pub type WordId = u32;

#[derive(Clone, Debug, Default)]
pub struct Lexicon {
    entries: Vec<Entry>,
    index: HashMap<String, WordId>,
    by_category: HashMap<Category, Vec<WordId>>,
}

impl Lexicon {
    pub fn new(entries: Vec<Entry>) -> Self {
        let mut index = HashMap::with_capacity(entries.len());
        let mut by_category: HashMap<Category, Vec<WordId>> = HashMap::new();
        for (i, e) in entries.iter().enumerate() {
            index.insert(e.word.clone(), i as WordId);
            by_category.entry(e.category).or_default().push(i as WordId);
        }
        Lexicon {
            entries,
            index,
            by_category,
        }
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    pub fn entries(&self) -> &[Entry] {
        &self.entries
    }

    /// Entry for an id. Ids come from this lexicon, so they are always valid;
    /// out-of-range ids fall back to the first entry.
    pub fn get(&self, id: WordId) -> &Entry {
        self.entries.get(id as usize).unwrap_or(&self.entries[0])
    }

    pub fn id_of(&self, word: &str) -> Option<WordId> {
        self.index.get(word).copied()
    }

    pub fn find(&self, word: &str) -> Option<&Entry> {
        self.id_of(word).map(|id| self.get(id))
    }

    pub fn in_category(&self, c: Category) -> &[WordId] {
        self.by_category.get(&c).map_or(&[], Vec::as_slice)
    }
}
