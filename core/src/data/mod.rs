//! Balance and content data, loaded from `assets/data/*.ron`.

mod types;
pub use types::*;

use crate::babel::body_gen::WordSampler;
use crate::babel::detect::Detector;
use crate::babel::lexicon::{Entry, Lexicon};
use serde::de::DeserializeOwned;
use std::fmt;
use std::path::{Path, PathBuf};

#[derive(Debug)]
pub struct DataError {
    pub file: PathBuf,
    pub message: String,
}

impl fmt::Display for DataError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}: {}", self.file.display(), self.message)
    }
}

impl std::error::Error for DataError {}

pub struct GameData {
    pub lexicon: Lexicon,
    pub canon: Vec<CanonDef>,
    pub canon_texts: Vec<String>,
    pub biomes: Vec<BiomeDef>,
    pub enemies: EnemiesFile,
    pub items: ItemsFile,
    pub recipes: Vec<RecipeDef>,
    pub skills: Vec<SkillDef>,
    pub sampler: WordSampler,
    pub detector: Detector,
}

fn read<T: DeserializeOwned>(dir: &Path, name: &str) -> Result<T, DataError> {
    let file = dir.join("data").join(name);
    let fail = |message: String| DataError {
        file: file.clone(),
        message,
    };
    let text = std::fs::read_to_string(&file).map_err(|e| fail(e.to_string()))?;
    ron::Options::default()
        .with_default_extension(ron::extensions::Extensions::IMPLICIT_SOME)
        .from_str(&text)
        .map_err(|e| fail(e.to_string()))
}

impl GameData {
    /// Loads all content from an `assets` directory.
    pub fn load(assets: &Path) -> Result<GameData, DataError> {
        let entries: Vec<Entry> = read(assets, "lexicon.ron")?;
        if entries.is_empty() {
            return Err(DataError {
                file: assets.join("data/lexicon.ron"),
                message: "the lexicon is empty".into(),
            });
        }
        let lexicon = Lexicon::new(entries);
        let canon: Vec<CanonDef> = read(assets, "canon.ron")?;
        let canon_texts: Vec<String> = canon.iter().map(|c| c.text.clone()).collect();
        let biomes: Vec<BiomeDef> = read(assets, "biomes.ron")?;
        let enemies: EnemiesFile = read(assets, "enemies.ron")?;
        let items: ItemsFile = read(assets, "items.ron")?;
        let recipes: Vec<RecipeDef> = read(assets, "recipes.ron")?;
        let skills: Vec<SkillDef> = read(assets, "skills.ron")?;
        let invalid = |file: &str, message: &str| DataError {
            file: assets.join("data").join(file),
            message: message.into(),
        };
        if biomes.is_empty() {
            return Err(invalid("biomes.ron", "no biomes"));
        }
        if enemies.archetypes.is_empty() || enemies.guardians.is_empty() {
            return Err(invalid("enemies.ron", "no archetypes or guardians"));
        }
        let sampler = WordSampler::new(&lexicon);
        let detector = Detector::new(&lexicon, &canon_texts);
        Ok(GameData {
            lexicon,
            canon,
            canon_texts,
            biomes,
            enemies,
            items,
            recipes,
            skills,
            sampler,
            detector,
        })
    }

    /// Biome by id, falling back to the first (canonical gallery).
    pub fn biome(&self, id: &str) -> &BiomeDef {
        self.biomes
            .iter()
            .find(|b| b.id == id)
            .unwrap_or(&self.biomes[0])
    }

    pub fn biome_index(&self, id: &str) -> usize {
        self.biomes.iter().position(|b| b.id == id).unwrap_or(0)
    }

    pub fn archetype_index(&self, id: &str) -> usize {
        self.enemies
            .archetypes
            .iter()
            .position(|a| a.id == id)
            .unwrap_or(0)
    }

    pub fn modifier_index(&self, id: &str) -> Option<usize> {
        self.enemies.modifiers.iter().position(|m| m.id == id)
    }

    pub fn skill(&self, id: &str) -> Option<&SkillDef> {
        self.skills.iter().find(|s| s.id == id)
    }

    pub fn recipe(&self, id: &str) -> Option<&RecipeDef> {
        self.recipes.iter().find(|r| r.id == id)
    }

    pub fn node(&self, kind: NodeKind) -> &NodeDef {
        self.items
            .nodes
            .iter()
            .find(|n| n.kind == kind)
            .unwrap_or(&self.items.nodes[0])
    }
}

/// Assets directory used by tests and tools.
pub fn dev_assets_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../assets")
}

#[cfg(test)]
pub(crate) fn test_data() -> &'static GameData {
    use std::sync::OnceLock;
    static DATA: OnceLock<GameData> = OnceLock::new();
    DATA.get_or_init(|| GameData::load(&dev_assets_dir()).expect("game data must load"))
}

#[cfg(test)]
mod tests;
