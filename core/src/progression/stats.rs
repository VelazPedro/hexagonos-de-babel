//! Aggregated bonuses from equipment, inscriptions, skills and relics.

use super::items::Item;
use super::player::PlayerData;
use crate::data::{GameData, Stat, ToolKind, Unlock};
use std::collections::{BTreeMap, BTreeSet};

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Stats(BTreeMap<Stat, f32>);

impl Stats {
    pub fn get(&self, s: Stat) -> f32 {
        self.0.get(&s).copied().unwrap_or(0.0)
    }

    pub fn add(&mut self, s: Stat, v: f32) {
        *self.0.entry(s).or_insert(0.0) += v;
    }

    pub fn merge(&mut self, other: &Stats) {
        for (s, v) in other.iter() {
            self.add(s, v);
        }
    }

    pub fn iter(&self) -> impl Iterator<Item = (Stat, f32)> + '_ {
        self.0.iter().map(|(&s, &v)| (s, v))
    }
}

pub fn describe(stat: Stat, value: f32) -> String {
    if stat.is_percent() {
        format!("+{}% {}", (value * 100.0).round() as i32, stat.label())
    } else if (value - value.round()).abs() < 0.05 {
        format!("+{} {}", value.round() as i32, stat.label())
    } else {
        format!("+{value:.1} {}", stat.label())
    }
}

fn effect_stats(data: &GameData, tag: &str, scale: f32, out: &mut Stats) {
    if let Some(e) = data.items.effects.iter().find(|e| e.tag == tag) {
        out.add(e.stat, e.amount * scale);
    }
}

pub fn item_stats(data: &GameData, item: &Item) -> Stats {
    let mut out = Stats::default();
    let mult = item.rarity.stat_multiplier();
    if let Some(def) = data.items.slots.iter().find(|s| s.slot == item.slot) {
        for &(stat, v) in &def.base {
            out.add(stat, v * mult);
        }
    }
    for tag in &item.effects {
        effect_stats(data, tag, mult, &mut out);
    }
    for i in &item.inscriptions {
        effect_stats(data, &i.effect, i.rarity.stat_multiplier(), &mut out);
    }
    out
}

/// Everything derived from a player's persistent state.
#[derive(Clone, Debug, Default)]
pub struct Derived {
    pub stats: Stats,
    pub unlocks: BTreeSet<Unlock>,
    pub max_hp: f32,
    pub max_energy: f32,
    pub damage: f32,
    pub defense: f32,
    /// Pixels per second.
    pub speed: f32,
    /// Lantern radius in tiles.
    pub light_radius: f32,
    pub dash_charges: u8,
}

impl Derived {
    pub fn has(&self, u: Unlock) -> bool {
        self.unlocks.contains(&u)
    }

    pub fn stat(&self, s: Stat) -> f32 {
        self.stats.get(s)
    }
}

pub fn derive(data: &GameData, player: &PlayerData) -> Derived {
    let mut stats = Stats::default();
    let mut unlocks = BTreeSet::new();
    for item in player.equipment.iter().flatten() {
        stats.merge(&item_stats(data, item));
    }
    for id in &player.skills {
        if let Some(skill) = data.skill(id) {
            for &(s, v) in &skill.stats {
                stats.add(s, v);
            }
            if let Some(u) = skill.unlock {
                unlocks.insert(u);
            }
        }
    }
    for id in &player.relics {
        if let Some(relic) = data.items.relics.iter().find(|r| &r.id == id) {
            for &(s, v) in &relic.stats {
                stats.add(s, v);
            }
        }
    }
    for _ in 0..player.infinite_fragments {
        if let Some(relic) = data
            .items
            .relics
            .iter()
            .find(|r| r.id == "fragmento_infinito")
        {
            for &(s, v) in &relic.stats {
                stats.add(s, v);
            }
        }
    }
    let lamp_tier = player.tool_tier(ToolKind::Lamp) as usize;
    let base_light = data
        .items
        .lamp_radius
        .get(lamp_tier)
        .copied()
        .unwrap_or(4.5);
    Derived {
        max_hp: 100.0 + stats.get(Stat::MaxHp),
        max_energy: 100.0 + stats.get(Stat::MaxEnergy),
        damage: 7.0 + f32::from(player.tool_tier(ToolKind::Knife)) * 4.0 + stats.get(Stat::Damage),
        defense: stats.get(Stat::Defense),
        speed: 84.0 * (1.0 + stats.get(Stat::MoveSpeed).min(0.8)),
        light_radius: base_light * (1.0 + stats.get(Stat::LightRadius).min(1.2)),
        dash_charges: if player.has_relic("botas_dobles") {
            2
        } else {
            1
        },
        stats,
        unlocks,
    }
}
