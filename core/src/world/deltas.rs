//! Player changes to a site, saved apart from the deterministic generation.

use super::grid::Pos;
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct SiteDelta {
    pub depleted_nodes: BTreeSet<u32>,
    pub opened_chests: BTreeSet<u32>,
    pub defeated_enemies: BTreeSet<u32>,
    /// Player-placed torches.
    pub torches: Vec<Pos>,
    /// Lit return lamp, if any.
    pub return_lamp: Option<Pos>,
    /// Doorways opened by walking through them.
    pub opened_doors: [bool; 6],
    /// Staircase back to the floor above.
    pub stairs_up: Option<Pos>,
    pub cleared: bool,
    pub read: bool,
}

impl SiteDelta {
    pub fn is_empty(&self) -> bool {
        *self == SiteDelta::default()
    }
}
