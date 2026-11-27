//! Player data that arrives from another computer is put in order before use:
//! nothing in it may break the game, whoever edited it.

use super::items::Item;
use super::pet::clean_name;
use super::pins::{MAX_PINS, can_pin};
use super::player::{INVENTORY_SIZE, PlayerData, xp_to_next};
use super::stats::MAX_WHETSTONES;
use super::upgrades;
use crate::data::{GameData, Slot};

const MAX_LEVEL: u32 = 999;
/// Of any material, consumable or fragment.
const MAX_AMOUNT: u32 = 1_000_000;
const MAX_FRAGMENTS: u32 = 500;
const MAX_TIER: u8 = 5;
const MAX_NAME: usize = 80;
const MAX_WORD: usize = 40;
const MAX_LIST: usize = 8;
const MAX_GLOSSARY: usize = 6000;
/// Item numbers stay far from overflowing.
const MAX_UID: u64 = u64::MAX / 2;

fn short(text: &str, limit: usize) -> String {
    text.chars()
        .filter(|c| !c.is_control())
        .take(limit)
        .collect()
}

fn tidy_item(mut item: Item) -> Item {
    item.uid = item.uid.min(MAX_UID);
    item.name = short(&item.name, MAX_NAME);
    for list in [&mut item.words, &mut item.effects] {
        list.truncate(MAX_LIST);
        for text in list {
            *text = short(text, MAX_WORD);
        }
    }
    item.inscriptions.truncate(MAX_LIST);
    for i in &mut item.inscriptions {
        i.word = short(&i.word, MAX_WORD);
        i.effect = short(&i.effect, MAX_WORD);
    }
    item
}

impl PlayerData {
    /// The same player with every value within the limits of the game.
    pub fn sanitized(mut self, data: &GameData) -> PlayerData {
        let finite = |v: f32| {
            if v.is_finite() {
                v.clamp(0.0, 1.0e6)
            } else {
                0.0
            }
        };
        self.hp = finite(self.hp);
        self.energy = finite(self.energy);
        self.level = self.level.clamp(1, MAX_LEVEL);
        self.xp = self.xp.min(xp_to_next(self.level) - 1);
        self.skill_points = self.skill_points.min(self.level);
        self.skills.retain(|id| data.skill(id).is_some());
        for amount in self
            .resources
            .values_mut()
            .chain(self.consumables.values_mut())
        {
            *amount = (*amount).min(MAX_AMOUNT);
        }
        self.tools = self.tools.map(|t| t.min(MAX_TIER));
        self.whetstones = self.whetstones.min(MAX_WHETSTONES);
        let mut kept = std::collections::BTreeSet::new();
        self.pins
            .retain(|id| can_pin(data, id) && kept.insert(id.clone()));
        self.pins.truncate(MAX_PINS);
        self.records.tidy();
        self.upgrades
            .retain(|thing, f| upgrades::belongs(data, thing, &f.id));
        for fitted in self.upgrades.values_mut() {
            fitted.level = fitted.level.clamp(1, upgrades::MAX_LEVEL);
        }
        // Each piece of equipment must be in the slot made for it.
        for (i, piece) in self.equipment.iter_mut().enumerate() {
            *piece = piece
                .take()
                .filter(|item| item.slot as usize == i)
                .map(tidy_item);
        }
        debug_assert_eq!(self.equipment.len(), Slot::ALL.len());
        self.inventory.truncate(INVENTORY_SIZE);
        self.inventory = self.inventory.into_iter().map(tidy_item).collect();
        let mut seen = std::collections::BTreeSet::new();
        self.relics
            .retain(|id| data.items.relics.iter().any(|r| &r.id == id) && seen.insert(id.clone()));
        self.infinite_fragments = self.infinite_fragments.min(MAX_FRAGMENTS);
        self.glossary = std::mem::take(&mut self.glossary)
            .into_iter()
            .filter(|(word, _)| word.chars().count() <= MAX_WORD)
            .take(MAX_GLOSSARY)
            .collect();
        let highest = self
            .equipment
            .iter()
            .flatten()
            .chain(&self.inventory)
            .map(|i| i.uid)
            .max()
            .unwrap_or(0);
        self.next_uid = self.next_uid.min(MAX_UID).max(highest);
        self.look = self.look.sanitized();
        self.name = clean_name(&self.name);
        self.pet.name = clean_name(&self.pet.name);
        self.pet.look = self.pet.look.sanitized();
        self.pet.skills.retain(|id| data.pet_skill(id).is_some());
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::data::dev_assets_dir;

    fn data() -> GameData {
        GameData::load(&dev_assets_dir()).expect("data")
    }

    #[test]
    fn an_honest_player_is_left_as_it_is() {
        let data = data();
        let mut player = PlayerData::default();
        player.gain_xp(900);
        assert_eq!(player.clone().sanitized(&data), player);
    }

    #[test]
    fn hostile_values_are_brought_within_limits() {
        let data = data();
        let wild = PlayerData {
            hp: f32::NAN,
            energy: f32::INFINITY,
            level: u32::MAX,
            xp: u32::MAX,
            skill_points: u32::MAX,
            skills: ["no-existe".to_string()].into(),
            tools: [200; 4],
            relics: vec!["inventada".to_string(); 1000],
            infinite_fragments: u32::MAX,
            next_uid: u64::MAX,
            ..PlayerData::default()
        };
        let tidy = wild.sanitized(&data);
        assert!(tidy.hp.is_finite() && tidy.energy.is_finite());
        assert!(tidy.level <= MAX_LEVEL && tidy.xp < xp_to_next(tidy.level));
        assert!(tidy.skill_points <= tidy.level);
        assert!(tidy.skills.is_empty() && tidy.relics.is_empty());
        assert!(tidy.tools.iter().all(|&t| t <= MAX_TIER));
        assert!(tidy.infinite_fragments <= MAX_FRAGMENTS);
        // Deriving stats and numbering items must not overflow or hang.
        let mut tidy = tidy;
        let derived = crate::progression::stats::derive(&data, &tidy);
        assert!(derived.max_hp.is_finite());
        tidy.next_item_uid();
        tidy.gain_xp(u32::MAX);
    }
}
