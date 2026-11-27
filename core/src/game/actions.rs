//! Operations triggered from interface screens.

use super::Game;
use super::events::{GameEvent, Sfx};
use super::sync::{Action, ItemRef};
use crate::babel::detect::Place;
use crate::babel::rarity::Rarity;
use crate::data::{RecipeOutput, Resource, Slot, Station};
use crate::progression::inscribe::{self, Target};
use crate::progression::pet::{PetLook, clean_name};
use crate::progression::pins::PinResult;
use crate::progression::reading::{self, ReadResult};
use crate::progression::upgrades::{self, Offer, UpgradeError};
use crate::progression::{crafting, crafting::CraftError};
use std::collections::BTreeSet;

impl Game {
    pub fn set_look(&mut self, look: crate::progression::look::Look) {
        self.record(Action::SetLook(look));
        self.profile.player.look = look.sanitized();
    }

    pub fn set_name(&mut self, name: &str) {
        self.record(Action::SetName(name.to_string()));
        self.profile.player.name = clean_name(name);
    }

    pub fn set_pet(&mut self, name: &str, look: PetLook) {
        self.record(Action::SetPet {
            name: name.to_string(),
            look,
        });
        let pet = &mut self.profile.player.pet;
        pet.name = clean_name(name);
        pet.look = look.sanitized();
    }

    /// Keeps a recipe in sight, or lets it go.
    pub fn toggle_pin(&mut self, recipe_id: &str) -> PinResult {
        self.record(Action::TogglePin(recipe_id.to_string()));
        let data = self.data.clone();
        self.profile.player.toggle_pin(&data, recipe_id)
    }

    /// Teaches the pet something, paid with the hero's skill points.
    pub fn learn_pet_skill(&mut self, id: &str) -> bool {
        self.record(Action::LearnPet(id.to_string()));
        let data = self.data.clone();
        if !self.profile.player.learn_pet(&data, id) {
            self.sound(Sfx::UiError);
            return false;
        }
        self.refresh();
        self.sound(Sfx::SkillLearn);
        true
    }

    pub fn stations_built(&self) -> BTreeSet<Station> {
        self.profile.world.stations.iter().map(|s| s.0).collect()
    }

    pub fn upgrade_check(&self, id: &str) -> Result<Offer, UpgradeError> {
        upgrades::check(
            &self.data,
            &self.profile.player,
            &self.derived,
            &self.stations_built(),
            self.site.key.is_home(),
            id,
        )
    }

    /// Chooses, raises or changes an improvement at its station.
    pub fn upgrade(&mut self, id: &str) -> Result<(), UpgradeError> {
        self.record(Action::Upgrade(id.to_string()));
        let data = self.data.clone();
        let built = self.stations_built();
        let home = self.site.key.is_home();
        let done = upgrades::apply(
            &data,
            &mut self.profile.player,
            &self.derived,
            &built,
            home,
            id,
        );
        if let Err(e) = done {
            self.sound(Sfx::UiError);
            return Err(e);
        }
        self.count(|t| t.improved += 1);
        self.sound(Sfx::Craft);
        self.events.push(GameEvent::Particles {
            pos: self.body.pos,
            z: 12.0,
            color: [180, 220, 255],
            count: 24,
            speed: 80.0,
        });
        self.refresh();
        self.events.push(GameEvent::Autosave);
        Ok(())
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
        self.record(Action::Craft {
            recipe: recipe_id.to_string(),
            plot,
        });
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
        self.count(|t| t.crafted += 1);
        // What is made only once has nothing left to be saved up for.
        if matches!(
            recipe.output,
            RecipeOutput::Tool(..) | RecipeOutput::Whetstone(_) | RecipeOutput::Build(_)
        ) {
            self.profile.player.pins.retain(|p| p != recipe_id);
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
        self.grant_own_xp(recipe.xp);
        self.events.push(GameEvent::Autosave);
        Ok(())
    }

    pub fn learn_skill(&mut self, id: &str) -> bool {
        self.record(Action::Learn(id.to_string()));
        let data = self.data.clone();
        if !self.profile.player.learn(&data, id) {
            self.sound(Sfx::UiError);
            return false;
        }
        self.refresh();
        self.sound(Sfx::SkillLearn);
        true
    }

    /// Number of the item at a position of the bag.
    fn carried(&self, index: usize) -> Option<u64> {
        self.profile.player.inventory.get(index).map(|i| i.uid)
    }

    pub fn equip(&mut self, inventory_index: usize) -> bool {
        if let Some(uid) = self.carried(inventory_index) {
            self.record(Action::Equip(uid));
        }
        let ok = self.profile.player.equip(inventory_index);
        if ok {
            self.sound(Sfx::UiConfirm);
            self.refresh();
        }
        ok
    }

    pub fn unequip(&mut self, slot: Slot) -> bool {
        self.record(Action::Unequip(slot));
        let ok = self.profile.player.unequip(slot);
        self.sound(if ok { Sfx::UiConfirm } else { Sfx::UiError });
        self.refresh();
        ok
    }

    pub fn swap_inventory(&mut self, a: usize, b: usize) {
        if let Some(item) = self.carried(a) {
            let to = u32::try_from(b).unwrap_or(u32::MAX);
            self.record(Action::Move { item, to });
        }
        let inv = &mut self.profile.player.inventory;
        if a < inv.len() && b < inv.len() {
            inv.swap(a, b);
        } else if a < inv.len() {
            let item = inv.remove(a);
            inv.push(item);
        }
    }

    pub fn recycle(&mut self, inventory_index: usize) {
        if let Some(uid) = self.carried(inventory_index) {
            self.record(Action::Recycle(uid));
        }
        if let Some(ink) = self.profile.player.recycle(inventory_index) {
            self.sound(Sfx::NodeBreak);
            self.say(format!("Reciclado: +{ink} Tinta"));
        }
    }

    pub fn inscribe(&mut self, target: Target, word: &str) -> Result<(), String> {
        let named = match target {
            Target::Equipped(slot) => u8::try_from(slot).ok().map(ItemRef::Worn),
            Target::Inventory(index) => self.carried(index).map(ItemRef::Carried),
        };
        if let Some(target) = named {
            self.record(Action::Inscribe {
                target,
                word: word.to_string(),
            });
        }
        if !self.stations_built().contains(&Station::Scriptorium) || !self.site.key.is_home() {
            return Err("Necesitás el Scriptorium de tu hexágono".into());
        }
        let data = self.data.clone();
        match inscribe::inscribe(&data, &mut self.profile.player, &self.derived, target, word) {
            Ok(()) => {
                self.sound(Sfx::Inscribe);
                self.say(format!("Inscribiste «{word}»"));
                self.refresh();
                self.grant_own_xp(15);
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
        self.record(Action::ReadBook);
        let pages = reading::readable_pages(&self.profile.player, &self.derived);
        let data = self.data.clone();
        let result = reading::read(
            &data,
            &self.site.analysis,
            &mut self.profile.player,
            &self.derived,
            pages,
        );
        // Each book is told apart by what its gallery grew from.
        let mut first = [0u8; 8];
        first.copy_from_slice(&self.site.seed[..8]);
        let book = u64::from_le_bytes(first);
        let seen = u8::try_from(pages).unwrap_or(u8::MAX);
        self.profile.player.records.read_book(book, seen);
        for canon in &self.site.analysis.canons {
            let legible = match canon.place {
                Place::Inscription => true,
                Place::Body { page, .. } => usize::from(page) < pages,
            };
            if legible {
                self.profile.player.records.find_canon(canon.canon);
            }
        }
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
        self.grant_own_xp(result.xp + bonus);
        result
    }

    /// The chest holds the host's savings: a guest cannot move them.
    fn owns_chest(&mut self) -> bool {
        let owner = self.slot == super::coop::HOST;
        if !owner {
            self.warn("El cofre es del anfitrión");
        }
        owner
    }

    pub fn deposit(&mut self, resource: Resource, amount: u32) {
        if !self.owns_chest() {
            return;
        }
        let amount = amount.min(self.profile.player.resource(resource));
        if amount > 0 && self.profile.player.pay(&[(resource, amount)]) {
            *self.profile.world.storage.entry(resource).or_insert(0) += amount;
            self.sound(Sfx::UiConfirm);
        }
    }

    pub fn withdraw(&mut self, resource: Resource, amount: u32) {
        if !self.owns_chest() {
            return;
        }
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
