//! Turns the words detected in a site's book into the rules that shape it.

use super::hex::Dir;
use super::layout::LayoutKind;
use crate::babel::detect::{Analysis, Hit, Place};
use crate::babel::lexicon::{Category, WordId};
use crate::babel::rarity::Rarity;
use crate::data::{GameData, Resource};
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct SiteRecipe {
    pub biome: usize,
    /// Ambient light level in `[0.15, 1.0]`.
    pub ambient: f32,
    /// Linear RGB tint of the ambient light.
    pub tint: [f32; 3],
    pub lamp_count: u32,
    pub layout: LayoutKind,
    /// Sides that orientation words force open, one flag per `Dir`.
    pub forced_exits: [bool; 6],
    pub has_orientation: bool,
    pub stairs_word: bool,
    /// No descriptive or orientation word: a quiet canonical site.
    pub calm: bool,
    /// Extra resource nodes granted by material words.
    pub materials: Vec<Resource>,
}

pub fn resource_from_tag(tag: &str) -> Option<Resource> {
    Some(match tag {
        "papel" => Resource::Paper,
        "cuero" => Resource::Leather,
        "fibra" => Resource::Fiber,
        "tinta" => Resource::Ink,
        "aceite" => Resource::Oil,
        "piedra" => Resource::Stone,
        "hierro" => Resource::Iron,
        "laton" => Resource::Brass,
        "plata" => Resource::Silver,
        "cristal" => Resource::Crystal,
        "ceniza" => Resource::Ash,
        "sal" => Resource::Salt,
        "hueso" => Resource::Bone,
        "vitral" => Resource::Vitral,
        _ => return None,
    })
}

/// How much a single occurrence counts: rarer words and the inscription weigh more.
fn weight(hit: &Hit) -> u32 {
    let base = 2 + hit.rarity.tier();
    if hit.place == Place::Inscription {
        base * 3
    } else {
        base
    }
}

/// Ambient light when no light word is present.
pub fn default_ambient(depth: u32) -> f32 {
    (0.62 - depth as f32 * 0.012).max(0.30)
}

/// Heaviest key; ties resolve to the one seen first.
fn heaviest<K: PartialEq + Copy>(tally: &[(K, u32)]) -> Option<K> {
    let mut best: Option<(K, u32)> = None;
    for &(k, w) in tally {
        if best.is_none_or(|(_, bw)| w > bw) {
            best = Some((k, w));
        }
    }
    best.map(|(k, _)| k)
}

fn add<K: PartialEq + Copy>(tally: &mut Vec<(K, u32)>, key: K, w: u32) {
    match tally.iter_mut().find(|(k, _)| *k == key) {
        Some(slot) => slot.1 += w,
        None => tally.push((key, w)),
    }
}

pub fn build(data: &GameData, analysis: &Analysis, depth: u32) -> SiteRecipe {
    let mut biomes: Vec<(usize, u32)> = Vec::new();
    let mut layouts: Vec<(LayoutKind, u32)> = Vec::new();
    let mut forced_exits = [false; 6];
    let mut materials = Vec::new();
    let (mut light_sum, mut light_weight) = (0.0f32, 0u32);
    let (mut warm, mut cold) = (0u32, 0u32);
    let mut stairs_word = false;
    let mut has_orientation = false;
    let mut descriptive = false;

    for hit in &analysis.hits {
        let entry = data.lexicon.get(hit.word);
        let w = weight(hit);
        match entry.category {
            Category::Bioma => {
                descriptive = true;
                if let Some(id) = entry.tag_value("biome") {
                    add(&mut biomes, data.biome_index(id), w);
                }
            }
            Category::Luz => {
                descriptive = true;
                let level: f32 = entry
                    .tag_value("lv")
                    .and_then(|v| v.parse().ok())
                    .unwrap_or(60.0);
                light_sum += level / 100.0 * w as f32;
                light_weight += w;
                match entry.tag_value("temp") {
                    Some("calido") => warm += w,
                    Some("frio") => cold += w,
                    _ => {}
                }
            }
            Category::Orientacion => {
                has_orientation = true;
                match entry.tag_value("layout") {
                    Some("stairs") => stairs_word = true,
                    Some(tag) => {
                        if let Some(kind) = LayoutKind::from_tag(tag) {
                            add(&mut layouts, kind, w);
                        }
                    }
                    None => {}
                }
                let sides: &[Dir] = match entry.tag_value("exit") {
                    Some("n") => &[Dir::N],
                    Some("s") => &[Dir::S],
                    Some("e") => &[Dir::NE, Dir::SE],
                    Some("w") => &[Dir::NW, Dir::SW],
                    Some("ne") => &[Dir::NE],
                    Some("nw") => &[Dir::NW],
                    Some("se") => &[Dir::SE],
                    Some("sw") => &[Dir::SW],
                    _ => &[],
                };
                for d in sides {
                    forced_exits[d.index()] = true;
                }
            }
            Category::Material => {
                if let Some(r) = entry.tag_value("res").and_then(resource_from_tag)
                    && materials.len() < 6
                {
                    materials.push(r);
                }
            }
            Category::Cualidad => descriptive = true,
            Category::Criatura | Category::Objeto | Category::Accion => {}
        }
    }

    let biome = heaviest(&biomes).unwrap_or(0);
    let biome_def = &data.biomes[biome.min(data.biomes.len() - 1)];
    let mut ambient = if light_weight > 0 {
        light_sum / light_weight as f32
    } else {
        default_ambient(depth)
    };
    if biome_def.id == "noche" {
        ambient *= 0.6;
    }
    let ambient = ambient.clamp(0.15, 1.0);

    let mut tint = biome_def.light_tint;
    if warm > cold {
        tint = [tint[0], tint[1] * 0.93, tint[2] * 0.80];
    } else if cold > warm {
        tint = [tint[0] * 0.80, tint[1] * 0.92, tint[2]];
    }

    // Darker sites need more lamps to stay readable; light words add fixtures too.
    let lamp_count = 2 + ((1.0 - ambient) * 3.0) as u32 + light_weight.min(24) / 8;

    SiteRecipe {
        biome,
        ambient,
        tint,
        lamp_count,
        layout: heaviest(&layouts).unwrap_or(LayoutKind::Canonical),
        forced_exits,
        has_orientation,
        stairs_word,
        calm: !descriptive && !has_orientation,
        materials,
    }
}

/// The rarest distinct words of a book, used to assemble guardians.
pub fn rarest_words(data: &GameData, analysis: &Analysis, count: usize) -> Vec<(WordId, Rarity)> {
    let mut words = analysis.distinct();
    words.sort_by(|a, b| {
        let ra = data.lexicon.get(a.0).base_rarity;
        let rb = data.lexicon.get(b.0).base_rarity;
        b.1.cmp(&a.1)
            .then(rb.cmp(&ra))
            .then(a.0.cmp(&b.0))
    });
    words.truncate(count);
    words
}
