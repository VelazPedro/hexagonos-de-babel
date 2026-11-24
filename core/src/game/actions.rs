//! Operations triggered from interface screens.

use super::Game;
use super::events::{GameEvent, Sfx};
use crate::babel::rarity::Rarity;
use crate::data::{RecipeOutput, Resource, Slot, Station};
use crate::progression::inscribe::{self, Target};
use crate::progression::reading::{self, ReadResult};
use crate::progression::{crafting, crafting::CraftError};
use std::collections::BTreeSet;

impl Game {
    pub fn stations_built(&self) -> BTreeSet<Station> {
        self.profile.world.stations.iter().map(|s| s.0).collect()
    }

    pub fn craft_check(&self, recipe_id: &str) -> Result<(), CraftError> {
        let recipe = self
            .data
            .recipe(recipe_id)
            .ok_or(CraftError::NotCraftable)?;
        crafting::check(
            recipe,
            &self.profile.player,
            &self.derived,
            &self.stations_built(),
            self.site.key.is_home(),
        )
    }

    /// Crafts a recipe. Station recipes also need the plot to build on.
    pub fn craft(&mut self, recipe_id: &str, plot: Option<u8>) -> Result<(), CraftError> {
        let recipe = self
            .data
            .recipe(recipe_id)
            .cloned()
            .ok_or(CraftError::NotCraftable)?;
        let built = self.stations_built();
        if let RecipeOutput::Build(_) = recipe.output {
            let free = plot.is_some_and(|p| {
                (p as usize) < crate::world::site::HOME_PLOTS.len()
                    && !self.profile.world.stations.iter().any(|s| s.1 == p)
            });
            if !free {
                return Err(CraftError::NotCraftable);
            }
        }
        let home = self.site.key.is_home();
        if let Err(e) = crafting::craft(
            &recipe,
            &mut self.profile.player,
            &self.derived,
            &built,
            home,
        ) {
            self.sound(Sfx::UiError);
            return Err(e);
        }
        if let (RecipeOutput::Build(station), Some(plot)) = (recipe.output, plot) {
            self.profile.world.stations.push((station, plot));
            self.sound(Sfx::Build);
            if station == Station::Binder {
                self.mark_tutorial(super::tutorial::Step::Build);
            }
            // The player may be standing where the station appeared.
            let pos = self.body.pos;
            if let Some(&tile) = crate::world::site::HOME_PLOTS.get(plot as usize) {
                let center = super::tile_center(tile);
                if pos.dist(center) < 14.0 {
                    self.body.pos = center + super::vec::v2(0.0, 18.0);
                }
            }
        } else {
            self.sound(Sfx::Craft);
        }
        self.events.push(GameEvent::Particles {
            pos: self.body.pos,
            z: 12.0,
            color: [255, 220, 120],
            count: 24,
            speed: 80.0,
        });
        self.say(format!("Fabricado: {}", recipe.name));
        self.refresh();
        self.grant_xp(recipe.xp);
        self.events.push(GameEvent::Autosave);
        Ok(())
    }

    pub fn learn_skill(&mut self, id: &str) -> bool {
        let data = self.data.clone();
        if !self.profile.player.learn(&data, id) {
            self.sound(Sfx::UiError);
            return false;
        }
        self.refresh();
        self.sound(Sfx::SkillLearn);
        true
    }

    pub fn equip(&mut self, inventory_index: usize) -> bool {
        let ok = self.profile.player.equip(inventory_index);
        if ok {
            self.sound(Sfx::UiConfirm);
            self.refresh();
        }
        ok
    }

    pub fn unequip(&mut self, slot: Slot) -> bool {
        let ok = self.profile.player.unequip(slot);
        self.sound(if ok { Sfx::UiConfirm } else { Sfx::UiError });
        self.refresh();
        ok
    }

    pub fn swap_inventory(&mut self, a: usize, b: usize) {
        let inv = &mut self.profile.player.inventory;
        if a < inv.len() && b < inv.len() {
            inv.swap(a, b);
        } else if a < inv.len() {
            let item = inv.remove(a);
            inv.push(item);
        }
    }

    pub fn recycle(&mut self, inventory_index: usize) {
        if let Some(ink) = self.profile.player.recycle(inventory_index) {
            self.sound(Sfx::NodeBreak);
            self.say(format!("Reciclado: +{ink} Tinta"));
        }
    }

    pub fn inscribe(&mut self, target: Target, word: &str) -> Result<(), String> {
        if !self.stations_built().contains(&Station::Scriptorium) || !self.site.key.is_home() {
            return Err("Necesitás el Scriptorium de tu hexágono".into());
        }
        let data = self.data.clone();
        match inscribe::inscribe(&data, &mut self.profile.player, &self.derived, target, word) {
            Ok(()) => {
                self.sound(Sfx::Inscribe);
                self.say(format!("Inscribiste «{word}»"));
                self.refresh();
                self.grant_xp(15);
                self.events.push(GameEvent::Autosave);
                Ok(())
            }
            Err(e) => {
                self.sound(Sfx::UiError);
                Err(e.message())
            }
        }
    }

    /// Reads the current book with the Lens, filling the Glossary.
    pub fn read_book(&mut self) -> ReadResult {
        let pages = reading::readable_pages(&self.profile.player, &self.derived);
        let data = self.data.clone();
        let result = reading::read(
            &data,
            &self.site.analysis,
            &mut self.profile.player,
            &self.derived,
            pages,
        );
        self.sound(Sfx::ReadPage);
        let first_read = !self.delta().read;
        self.delta_mut().read = true;
        self.mark_tutorial(super::tutorial::Step::Read);
        if !result.new_words.is_empty() {
            self.sound(Sfx::WordFound);
        }
        let best = result
            .new_words
            .iter()
            .chain(&result.improved)
            .map(|w| w.1)
            .max();
        if let Some(r) = best.filter(|&r| r >= Rarity::Legendary) {
            self.events.push(GameEvent::Flash(r));
            self.sound(Sfx::LegendaryFanfare);
        }
        let bonus = if first_read { 4 } else { 0 };
        self.grant_xp(result.xp + bonus);
        result
    }

    pub fn deposit(&mut self, resource: Resource, amount: u32) {
        let amount = amount.min(self.profile.player.resource(resource));
        if amount > 0 && self.profile.player.pay(&[(resource, amount)]) {
            *self.profile.world.storage.entry(resource).or_insert(0) += amount;
            self.sound(Sfx::UiConfirm);
        }
    }

    pub fn withdraw(&mut self, resource: Resource, amount: u32) {
        let Some(stored) = self.profile.world.storage.get_mut(&resource) else {
            return;
        };
        let amount = amount.min(*stored);
        if amount > 0 {
            *stored -= amount;
            self.profile.player.add_resource(resource, amount);
            self.sound(Sfx::UiConfirm);
        }
    }
}
