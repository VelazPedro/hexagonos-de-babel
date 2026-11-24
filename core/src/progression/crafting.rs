//! Recipes, costs and home stations.

use super::player::PlayerData;
use super::stats::Derived;
use crate::data::{RecipeDef, RecipeOutput, Resource, Stat, Station, ToolKind};
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
    }
    .clamp(0.0, 0.6);
    recipe
        .cost
        .iter()
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
        RecipeOutput::Tool(kind, tier) => {
            let owned = player.tool_tier(kind);
            if owned >= tier {
                return Err(CraftError::AlreadyOwned);
            }
            if owned + 1 != tier {
                return Err(CraftError::NeedPreviousTier);
            }
        }
        RecipeOutput::Build(s) if stations.contains(&s) => return Err(CraftError::AlreadyOwned),
        _ => {}
    }
    if !player.can_pay(&effective_cost(recipe, derived)) {
        return Err(CraftError::MissingResources);
    }
    Ok(())
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
        RecipeOutput::Item(c, n) => player.add_consumable(c, n),
        RecipeOutput::Material(r, n) => player.add_resource(r, n),
        RecipeOutput::Build(_) | RecipeOutput::ReturnLamp => {}
    }
    Ok(())
}
