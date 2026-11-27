//! Everything that persists between sessions.

use crate::data::{Resource, Station};
use crate::progression::items::Item;
use crate::progression::player::PlayerData;
use crate::progression::records::Records;
use crate::world::deltas::SiteDelta;
use crate::world::grid::Pos;
use crate::world::site::SiteKey;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};

pub const SAVE_VERSION: u32 = 1;

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct VisitRecord {
    pub floor: u32,
    pub q: i32,
    pub r: i32,
    pub biome: usize,
    pub stairs: bool,
    /// Doorways seen open.
    pub exits: [bool; 6],
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct LampRecord {
    pub key: SiteKey,
    pub pos: Pos,
    pub label: String,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct DeathBag {
    pub site: SiteKey,
    pub x: f32,
    pub y: f32,
    pub resources: BTreeMap<Resource, u32>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct WorldData {
    pub current: SiteKey,
    /// Player position in world pixels inside `current`.
    pub position: (f32, f32),
    /// Last lit lamp; `None` means the home hexagon.
    pub respawn: Option<LampRecord>,
    pub deltas: BTreeMap<String, SiteDelta>,
    /// Built stations and the plot each occupies.
    pub stations: Vec<(Station, u8)>,
    /// Resources deposited in the chest: safe from death.
    pub storage: BTreeMap<Resource, u32>,
    pub lamps: Vec<LampRecord>,
    pub guardians_defeated: BTreeSet<u32>,
    pub visited: BTreeMap<String, VisitRecord>,
    pub biomes_found: BTreeSet<String>,
    pub death_bag: Option<DeathBag>,
    /// Word items lying on the ground. Anything still here when a site is
    /// (re)loaded goes straight to the player, so no reward is ever lost.
    pub unclaimed: Vec<Item>,
    /// Where the Catalog journey started.
    pub remote_return: Option<SiteKey>,
    /// Completed tutorial steps (bit flags).
    pub tutorial: u16,
    pub finished: bool,
    pub play_seconds: f64,
    pub deepest_floor: u32,
    pub sites_cleared: u32,
}

impl Default for WorldData {
    fn default() -> Self {
        WorldData {
            current: SiteKey::walk(1, 0, 0),
            position: (320.0, 296.0),
            respawn: None,
            deltas: BTreeMap::new(),
            stations: Vec::new(),
            storage: BTreeMap::new(),
            lamps: Vec::new(),
            guardians_defeated: BTreeSet::new(),
            visited: BTreeMap::new(),
            biomes_found: BTreeSet::new(),
            death_bag: None,
            unclaimed: Vec::new(),
            remote_return: None,
            tutorial: 0,
            finished: false,
            play_seconds: 0.0,
            deepest_floor: 1,
            sites_cleared: 0,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Profile {
    pub version: u32,
    pub world_seed: u64,
    pub player: PlayerData,
    pub world: WorldData,
}

impl Profile {
    pub fn new(world_seed: u64) -> Profile {
        Profile {
            version: SAVE_VERSION,
            world_seed,
            // A new character has nothing from before to take.
            player: PlayerData {
                records: Records {
                    begun: true,
                    ..Records::default()
                },
                ..PlayerData::default()
            },
            world: WorldData::default(),
        }
    }
}

impl Profile {
    /// Once only, takes for the character what a save from before records
    /// already knew about the way travelled.
    pub fn begin_records(&mut self) {
        let records = &mut self.player.records;
        if records.begun {
            return;
        }
        records.begun = true;
        let world = &self.world;
        let tally = &mut records.tally;
        tally.play_seconds = world.play_seconds.max(0.0) as u64;
        // A world never walked has nothing to tell.
        if !world.visited.is_empty() {
            tally.deepest_floor = world.deepest_floor;
        }
        tally.hexagons = u32::try_from(world.visited.len()).unwrap_or(u32::MAX);
        tally.guardians = u32::try_from(world.guardians_defeated.len()).unwrap_or(u32::MAX);
        *tally = tally.tidy();
    }
}

/// A typed seed: digits are used as-is, any other text is hashed.
pub fn parse_seed(text: &str) -> Option<u64> {
    let t = text.trim();
    if t.is_empty() {
        return None;
    }
    Some(
        t.parse::<u64>()
            .unwrap_or_else(|_| crate::babel::seeds::stable_hash(t)),
    )
}
