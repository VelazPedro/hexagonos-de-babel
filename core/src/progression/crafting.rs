//! Recipes, costs and home stations.

use super::player::PlayerData;
use super::stats::Derived;
use crate::data::{Consumable, Perk, RecipeDef, RecipeOutput, Resource, Stat, Station, ToolKind};
use std::collections::BTreeSet;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CraftError {
    /// The recipe needs a station that has not been built.
    NeedStation(Station),
    /// Station recipes can only be used at home.
    NotAtHome,
    MissingResources,
    /// Tools must be upgraded one tier at a time.
    NeedPreviousTier,
    AlreadyOwned,
    NotCraftable,
}

impl CraftError {
    pub fn message(self) -> String {
        match self {
            CraftError::NeedStation(s) => format!("Falta construir: {}", s.name()),
            CraftError::NotAtHome => "Solo en tu hexágono".into(),
            CraftError::MissingResources => "Faltan materiales".into(),
            CraftError::NeedPreviousTier => "Primero el tier anterior".into(),
            CraftError::AlreadyOwned => "Ya lo tenés".into(),
            CraftError::NotCraftable => "No se fabrica aquí".into(),
        }
    }
}

/// Cost after the craft discount (never below 1 per ingredient).
pub fn effective_cost(recipe: &RecipeDef, derived: &Derived) -> Vec<(Resource, u32)> {
    let discount = match recipe.output {
        RecipeOutput::ReturnLamp => derived.stat(Stat::LampDiscount),
        _ => derived.stat(Stat::CraftDiscount),
    };
    discounted(&recipe.cost, discount)
}

/// A price with a discount taken off, never below 1 per ingredient.
pub(crate) fn discounted(cost: &[(Resource, u32)], discount: f32) -> Vec<(Resource, u32)> {
    let discount = discount.clamp(0.0, 0.6);
    cost.iter()
        .map(|&(r, n)| (r, ((n as f32 * (1.0 - discount)).ceil() as u32).max(1)))
        .collect()
}

pub fn check(
    recipe: &RecipeDef,
    player: &PlayerData,
    derived: &Derived,
    stations: &BTreeSet<Station>,
    at_home: bool,
) -> Result<(), CraftError> {
    if recipe.output == RecipeOutput::ReturnLamp {
        return Err(CraftError::NotCraftable);
    }
    if !at_home {
        return Err(CraftError::NotAtHome);
    }
    if let Some(s) = recipe.station
        && !stations.contains(&s)
    {
        return Err(CraftError::NeedStation(s));
    }
    match recipe.output {
        RecipeOutput::Tool(kind, tier) => next_in_order(player.tool_tier(kind), tier)?,
        RecipeOutput::Whetstone(level) => next_in_order(player.whetstones, level)?,
        RecipeOutput::Build(s) if stations.contains(&s) => return Err(CraftError::AlreadyOwned),
        _ => {}
    }
    if !player.can_pay(&effective_cost(recipe, derived)) {
        return Err(CraftError::MissingResources);
    }
    Ok(())
}

/// How many more than the recipe says each batch brings this player.
pub fn extra_batch(output: RecipeOutput, derived: &Derived) -> u32 {
    let perk = match output {
        RecipeOutput::Item(Consumable::Tonic, _) => Perk::TonicBatch,
        RecipeOutput::Material(Resource::Ink, _) => Perk::InkBatch,
        _ => return 0,
    };
    derived.perk(perk).round() as u32
}

/// Upgrades are made one step at a time.
fn next_in_order(owned: u8, wanted: u8) -> Result<(), CraftError> {
    if owned >= wanted {
        Err(CraftError::AlreadyOwned)
    } else if owned + 1 != wanted {
        Err(CraftError::NeedPreviousTier)
    } else {
        Ok(())
    }
}

/// Pays for and applies a recipe. Station construction is recorded by the caller,
/// who also chooses the plot.
pub fn craft(
    recipe: &RecipeDef,
    player: &mut PlayerData,
    derived: &Derived,
    stations: &BTreeSet<Station>,
    at_home: bool,
) -> Result<(), CraftError> {
    check(recipe, player, derived, stations, at_home)?;
    if !player.pay(&effective_cost(recipe, derived)) {
        return Err(CraftError::MissingResources);
    }
    match recipe.output {
        RecipeOutput::Tool(kind, tier) => player.tools[ToolKind::index(kind)] = tier,
        RecipeOutput::Whetstone(level) => player.whetstones = level,
        RecipeOutput::Item(c, n) => {
            player.add_consumable(c, n + extra_batch(recipe.output, derived));
        }
        RecipeOutput::Material(r, n) => {
            player.add_resource(r, n + extra_batch(recipe.output, derived));
        }
        RecipeOutput::Build(_) | RecipeOutput::ReturnLamp => {}
    }
    Ok(())
}
