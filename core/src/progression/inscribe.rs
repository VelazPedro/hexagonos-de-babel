//! The Scriptorium: write glossary words onto equipment. Words are power.

use super::items::{Inscribed, Item, word_effect};
use super::player::PlayerData;
use super::stats::Derived;
use crate::babel::rarity::Rarity;
use crate::data::{GameData, Resource, Stat, Unlock};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum InscribeError {
    UnknownWord,
    NoSlots,
    Duplicate,
    NotEnoughInk(u32),
}

impl InscribeError {
    pub fn message(self) -> String {
        match self {
            InscribeError::UnknownWord => "Esa palabra no está en tu Glosario".into(),
            InscribeError::NoSlots => "No quedan renglones libres".into(),
            InscribeError::Duplicate => "Ya está inscrita".into(),
            InscribeError::NotEnoughInk(n) => format!("Necesitás {n} de tinta"),
        }
    }
}

pub fn ink_cost(rarity: Rarity, derived: &Derived) -> u32 {
    let base = [2.0, 4.0, 7.0, 11.0, 16.0, 24.0][rarity.tier() as usize];
    let discount = derived.stat(Stat::InscribeDiscount).clamp(0.0, 0.6);
    ((base * (1.0 - discount)) as f32).ceil() as u32
}

pub fn max_slots(item: &Item, derived: &Derived) -> usize {
    item.rarity.inscription_slots() + usize::from(derived.has(Unlock::DoubleInscription))
}

pub fn check(
    item: &Item,
    word: &str,
    player: &PlayerData,
    derived: &Derived,
) -> Result<u32, InscribeError> {
    let Some(gloss) = player.glossary.get(word) else {
        return Err(InscribeError::UnknownWord);
    };
    if item.inscriptions.iter().any(|i| i.word == word) {
        return Err(InscribeError::Duplicate);
    }
    if item.inscriptions.len() >= max_slots(item, derived) {
        return Err(InscribeError::NoSlots);
    }
    let cost = ink_cost(gloss.rarity, derived);
    if player.resource(Resource::Ink) < cost {
        return Err(InscribeError::NotEnoughInk(cost));
    }
    Ok(cost)
}

/// Where the item being inscribed lives.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Target {
    Equipped(usize),
    Inventory(usize),
}

pub fn item_at(player: &PlayerData, target: Target) -> Option<&Item> {
    match target {
        Target::Equipped(i) => player.equipment.get(i)?.as_ref(),
        Target::Inventory(i) => player.inventory.get(i),
    }
}

pub fn inscribe(
    data: &GameData,
    player: &mut PlayerData,
    derived: &Derived,
    target: Target,
    word: &str,
) -> Result<(), InscribeError> {
    let item = item_at(player, target).ok_or(InscribeError::NoSlots)?;
    let cost = check(item, word, player, derived)?;
    let rarity = player
        .glossary
        .get(word)
        .map(|g| g.rarity)
        .ok_or(InscribeError::UnknownWord)?;
    if !player.pay(&[(Resource::Ink, cost)]) {
        return Err(InscribeError::NotEnoughInk(cost));
    }
    let inscribed = Inscribed {
        word: word.to_string(),
        effect: word_effect(data, word),
        rarity,
    };
    let item = match target {
        Target::Equipped(i) => player.equipment.get_mut(i).and_then(Option::as_mut),
        Target::Inventory(i) => player.inventory.get_mut(i),
    };
    if let Some(item) = item {
        item.inscriptions.push(inscribed);
    }
    Ok(())
}
