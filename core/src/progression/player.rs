//! Persistent player state, experience and skills.

use super::items::Item;
use crate::babel::rarity::Rarity;
use crate::data::{Consumable, GameData, Resource, Slot, ToolKind};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};

pub const INVENTORY_SIZE: usize = 30;

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct GlossEntry {
    pub rarity: Rarity,
    pub times: u32,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct PlayerData {
    pub hp: f32,
    pub energy: f32,
    pub level: u32,
    pub xp: u32,
    pub skill_points: u32,
    pub skills: BTreeSet<String>,
    pub resources: BTreeMap<Resource, u32>,
    pub consumables: BTreeMap<Consumable, u32>,
    /// Tiers indexed by `ToolKind`.
    pub tools: [u8; 4],
    pub active_tool: ToolKind,
    /// Indexed by `Slot`.
    pub equipment: [Option<Item>; 5],
    pub inventory: Vec<Item>,
    pub relics: Vec<String>,
    pub infinite_fragments: u32,
    pub glossary: BTreeMap<String, GlossEntry>,
    pub next_uid: u64,
}

impl Default for PlayerData {
    fn default() -> Self {
        let mut consumables = BTreeMap::new();
        consumables.insert(Consumable::Tonic, 2);
        consumables.insert(Consumable::Torch, 3);
        PlayerData {
            hp: 100.0,
            energy: 100.0,
            level: 1,
            xp: 0,
            skill_points: 0,
            skills: BTreeSet::new(),
            resources: BTreeMap::new(),
            consumables,
            tools: [0; 4],
            active_tool: ToolKind::Knife,
            equipment: [None, None, None, None, None],
            inventory: Vec::new(),
            relics: Vec::new(),
            infinite_fragments: 0,
            glossary: BTreeMap::new(),
            next_uid: 1,
        }
    }
}

/// Experience needed to go from `level` to the next one.
pub fn xp_to_next(level: u32) -> u32 {
    let l = level.max(1);
    30 + 24 * l + 3 * l * l
}

impl PlayerData {
    pub fn tool_tier(&self, t: ToolKind) -> u8 {
        self.tools[t.index()]
    }

    pub fn resource(&self, r: Resource) -> u32 {
        self.resources.get(&r).copied().unwrap_or(0)
    }

    pub fn add_resource(&mut self, r: Resource, n: u32) {
        let slot = self.resources.entry(r).or_insert(0);
        *slot = slot.saturating_add(n);
    }

    pub fn can_pay(&self, cost: &[(Resource, u32)]) -> bool {
        cost.iter().all(|&(r, n)| self.resource(r) >= n)
    }

    /// Removes the whole cost, or nothing at all.
    pub fn pay(&mut self, cost: &[(Resource, u32)]) -> bool {
        if !self.can_pay(cost) {
            return false;
        }
        for &(r, n) in cost {
            if let Some(slot) = self.resources.get_mut(&r) {
                *slot -= n;
            }
        }
        true
    }

    pub fn consumable(&self, c: Consumable) -> u32 {
        self.consumables.get(&c).copied().unwrap_or(0)
    }

    pub fn add_consumable(&mut self, c: Consumable, n: u32) {
        let slot = self.consumables.entry(c).or_insert(0);
        *slot = slot.saturating_add(n);
    }

    pub fn use_consumable(&mut self, c: Consumable, n: u32) -> bool {
        match self.consumables.get_mut(&c) {
            Some(slot) if *slot >= n => {
                *slot -= n;
                true
            }
            _ => false,
        }
    }

    pub fn has_relic(&self, id: &str) -> bool {
        self.relics.iter().any(|r| r == id)
    }

    /// Adds experience and returns how many levels were gained.
    pub fn gain_xp(&mut self, amount: u32) -> u32 {
        self.xp = self.xp.saturating_add(amount);
        let mut levels = 0;
        while self.xp >= xp_to_next(self.level) {
            self.xp -= xp_to_next(self.level);
            self.level += 1;
            self.skill_points += 1;
            levels += 1;
        }
        levels
    }

    pub fn can_learn(&self, data: &GameData, id: &str) -> bool {
        self.skill_points > 0
            && !self.skills.contains(id)
            && data
                .skill(id)
                .is_some_and(|s| s.requires.iter().all(|r| self.skills.contains(r)))
    }

    pub fn learn(&mut self, data: &GameData, id: &str) -> bool {
        if !self.can_learn(data, id) {
            return false;
        }
        self.skill_points -= 1;
        self.skills.insert(id.to_string());
        true
    }

    pub fn next_item_uid(&mut self) -> u64 {
        self.next_uid += 1;
        self.next_uid
    }

    pub fn equipped(&self, slot: Slot) -> Option<&Item> {
        self.equipment[slot as usize].as_ref()
    }

    /// Stores a new item, equipping it right away if its slot is empty.
    /// When the inventory is full the weakest piece is recycled into ink and
    /// returned, so the caller can say so.
    pub fn receive_item(&mut self, item: Item) -> Option<Item> {
        let slot = item.slot as usize;
        if self.equipment[slot].is_none() {
            self.equipment[slot] = Some(item);
            return None;
        }
        self.inventory.push(item);
        let mut recycled = None;
        while self.inventory.len() > INVENTORY_SIZE {
            let weakest = self
                .inventory
                .iter()
                .enumerate()
                .min_by(|a, b| a.1.power().total_cmp(&b.1.power()))
                .map(|(i, _)| i);
            let Some(i) = weakest else { break };
            let gone = self.inventory.remove(i);
            self.add_resource(Resource::Ink, 1 + gone.rarity.tier());
            recycled = Some(gone);
        }
        recycled
    }

    /// Swaps an inventory item with whatever is equipped in its slot.
    pub fn equip(&mut self, inventory_index: usize) -> bool {
        if inventory_index >= self.inventory.len() {
            return false;
        }
        let item = self.inventory.remove(inventory_index);
        let slot = item.slot as usize;
        if let Some(old) = self.equipment[slot].replace(item) {
            self.inventory
                .insert(inventory_index.min(self.inventory.len()), old);
        }
        true
    }

    pub fn unequip(&mut self, slot: Slot) -> bool {
        if self.inventory.len() >= INVENTORY_SIZE {
            return false;
        }
        match self.equipment[slot as usize].take() {
            Some(item) => {
                self.inventory.push(item);
                true
            }
            None => false,
        }
    }

    /// Breaks an inventory item down into ink.
    pub fn recycle(&mut self, inventory_index: usize) -> Option<u32> {
        if inventory_index >= self.inventory.len() {
            return None;
        }
        let item = self.inventory.remove(inventory_index);
        let ink = 1 + item.rarity.tier();
        self.add_resource(Resource::Ink, ink);
        Some(ink)
    }
}
