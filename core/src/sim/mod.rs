//! Headless progression bot. It walks the Library through open doors only,
//! harvests, goes home to craft and descends, proving that every tool tier is
//! always reachable without luck.

use crate::data::{GameData, RecipeOutput, ToolKind};
use crate::game::Game;
use crate::game::events::NumberKind;
use crate::game::profile::Profile;
use crate::game::travel::Arrival;
use crate::world::floors;
use crate::world::hex::{self, Dir};
use crate::world::site::{self, SiteKey};
use std::collections::{HashMap, HashSet, VecDeque};
use std::sync::Arc;

#[derive(Debug, Default)]
pub struct Report {
    /// Floor on which all four tools first reached each tier (index 0 = tier 1).
    pub tier_floor: [Option<u32>; 5],
    pub sites_visited: u32,
    pub deepest_floor: u32,
    pub guardians: u32,
    pub finished: bool,
}

pub struct Bot {
    pub game: Game,
    pub report: Report,
}

/// Directions to walk from `from` until `goal` is met, using only open doorways.
fn path_to(
    game: &Game,
    floor: u32,
    from: (i32, i32),
    entry: Option<Dir>,
    goal: impl Fn((i32, i32)) -> bool,
) -> Option<Vec<Dir>> {
    let mut queue = VecDeque::from([(from, entry)]);
    let mut parent: HashMap<(i32, i32), ((i32, i32), Dir)> = HashMap::new();
    let mut seen = HashSet::from([from]);
    while let Some((at, came)) = queue.pop_front() {
        if goal(at) {
            let mut path = Vec::new();
            let mut node = at;
            while node != from {
                let (previous, d) = parent[&node];
                path.push(d);
                node = previous;
            }
            path.reverse();
            return Some(path);
        }
        if seen.len() > 300 {
            return None;
        }
        let here = site::generate_with(
            &game.data,
            &game.feistel,
            game.profile.world_seed,
            &SiteKey::walk(floor, at.0, at.1),
        );
        let opened = game
            .profile
            .world
            .deltas
            .get(&here.key.id())
            .map(|d| d.opened_doors)
            .unwrap_or_default();
        for d in Dir::ALL {
            if !(here.exits[d.index()] || opened[d.index()] || came == Some(d)) {
                continue;
            }
            let next = hex::neighbor(at.0, at.1, d);
            if seen.insert(next) {
                parent.insert(next, (at, d));
                queue.push_back((next, Some(d.opposite())));
            }
        }
    }
    None
}

impl Bot {
    pub fn new(data: Arc<GameData>, world_seed: u64) -> Bot {
        Bot {
            game: Game::new(data, Profile::new(world_seed)),
            report: Report::default(),
        }
    }

    fn collect_drops(&mut self) {
        while let Some(drop) = self.game.drops.pop() {
            self.game.pick_up(drop);
        }
    }

    /// Harvests every node the current tools allow.
    fn harvest_all(&mut self) {
        let ids: Vec<u32> = self.game.nodes.iter().map(|n| n.id).collect();
        for id in ids {
            let Some(node) = self.game.nodes.iter().find(|n| n.id == id) else {
                continue;
            };
            let def = self.game.data.node(node.kind);
            if self.game.profile.player.tool_tier(def.tool) < def.min_tier {
                continue;
            }
            self.game.profile.player.active_tool = def.tool;
            for _ in 0..64 {
                if self.game.nodes.iter().all(|n| n.id != id) {
                    break;
                }
                self.game.harvest(id, 1.0);
            }
        }
        self.collect_drops();
        self.game.events.clear();
    }

    /// Builds missing stations and crafts every affordable tool upgrade.
    fn craft_everything(&mut self) {
        let data = self.game.data.clone();
        loop {
            let mut progressed = false;
            for recipe in &data.recipes {
                let plot = match recipe.output {
                    RecipeOutput::Build(_) => Some(self.game.profile.world.stations.len() as u8),
                    RecipeOutput::Tool(..) => None,
                    _ => continue,
                };
                if self.game.craft_check(&recipe.id).is_ok()
                    && self.game.craft(&recipe.id, plot).is_ok()
                {
                    progressed = true;
                }
            }
            // Turn spare paper into ink when a lens upgrade needs it.
            let ink = self
                .game
                .profile
                .player
                .resource(crate::data::Resource::Ink);
            if ink < 14 && self.game.craft_check("ink_paper").is_ok() {
                let paper = self
                    .game
                    .profile
                    .player
                    .resource(crate::data::Resource::Paper);
                if paper > 40 && self.game.craft("ink_paper", None).is_ok() {
                    progressed = true;
                }
            }
            if !progressed {
                break;
            }
        }
        self.game.events.clear();
    }

    fn lowest_tier(&self) -> u8 {
        ToolKind::ALL
            .iter()
            .map(|&t| self.game.profile.player.tool_tier(t))
            .min()
            .unwrap_or(0)
    }

    fn note_tiers(&mut self, floor: u32) {
        for tier in 1..=self.lowest_tier() {
            let slot = &mut self.report.tier_floor[tier as usize - 1];
            if slot.is_none() {
                *slot = Some(floor);
            }
        }
    }

    /// Visits home to craft, then returns to `back`.
    fn restock(&mut self, back: SiteKey) {
        self.game.travel_home();
        self.harvest_all();
        self.craft_everything();
        self.game.enter(back, Arrival::Hub);
        self.game.events.clear();
    }

    /// Explores `per_floor` sites on each floor, then takes real stairs down.
    pub fn run(&mut self, last_floor: u32, per_floor: u32) -> Result<(), String> {
        let (mut q, mut r) = (0, 0);
        let mut entry: Option<Dir> = None;
        for floor in 1..=last_floor {
            self.report.deepest_floor = floor;
            let mut explored: HashSet<(i32, i32)> = HashSet::from([(q, r)]);
            self.harvest_all();
            for visit in 0..per_floor {
                let goal = |p: (i32, i32)| !explored.contains(&p);
                let Some(path) = path_to(&self.game, floor, (q, r), entry, goal) else {
                    return Err(format!("floor {floor}: nowhere new to walk from {q},{r}"));
                };
                for d in path {
                    (q, r) = hex::neighbor(q, r, d);
                    entry = Some(d.opposite());
                    self.game
                        .enter(SiteKey::walk(floor, q, r), Arrival::Door(d.opposite()));
                }
                explored.insert((q, r));
                self.report.sites_visited += 1;
                self.harvest_all();
                if visit % 3 == 2 {
                    self.restock(SiteKey::walk(floor, q, r));
                    self.note_tiers(floor);
                }
            }
            // Walk to the nearest reachable lattice staircase.
            let seed = self.game.profile.world_seed;
            let goal = |p: (i32, i32)| floors::is_stair_site(seed, floor, p.0, p.1);
            let Some(path) = path_to(&self.game, floor, (q, r), entry, goal) else {
                return Err(format!("floor {floor}: no walkable path to any staircase"));
            };
            for d in path {
                (q, r) = hex::neighbor(q, r, d);
                self.game
                    .enter(SiteKey::walk(floor, q, r), Arrival::Door(d.opposite()));
                self.report.sites_visited += 1;
                self.harvest_all();
            }
            if self.game.site.stairs.is_none() {
                return Err(format!("floor {floor}: lattice site without stairs"));
            }
            // This bot checks resources, not combat: guardians fall at once.
            if self.game.stairs_locked() {
                let boss = self
                    .game
                    .enemies
                    .iter()
                    .find(|e| e.boss.is_some())
                    .map(|e| (e.uid, e.hp));
                let Some((uid, hp)) = boss else {
                    return Err(format!("floor {floor}: locked stairs without a guardian"));
                };
                self.game
                    .apply_damage_to_enemy(uid, hp + 1.0, NumberKind::Enemy);
                self.report.guardians += 1;
                self.collect_drops();
                if self.game.stairs_locked() {
                    return Err(format!("floor {floor}: guardian did not unlock the stairs"));
                }
            }
            self.restock(SiteKey::walk(floor, q, r));
            self.note_tiers(floor);
            if !self.game.descend() {
                return Err(format!("floor {floor}: could not descend"));
            }
            self.game.events.clear();
            entry = None;
        }
        self.report.finished = self.game.profile.world.finished;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::data::{dev_assets_dir, test_data};

    fn shared() -> Arc<GameData> {
        Arc::new(GameData::load(&dev_assets_dir()).expect("data"))
    }

    fn check(seed: u64, last_floor: u32, per_floor: u32) -> Report {
        let mut bot = Bot::new(shared(), seed);
        if let Err(e) = bot.run(last_floor, per_floor) {
            panic!("seed {seed}: {e}");
        }
        bot.report
    }

    #[test]
    fn first_tiers_never_softlock() {
        let _ = test_data();
        let report = check(1, 7, 6);
        assert!(report.tier_floor[0].is_some_and(|f| f <= 3), "{report:?}");
        assert!(report.tier_floor[1].is_some_and(|f| f <= 6), "{report:?}");
        assert!(report.guardians >= 1);
    }

    /// Full descent over several worlds; run in release.
    #[test]
    #[ignore = "full progression simulation, run in release"]
    fn every_tier_and_the_ending_are_reachable() {
        for seed in 0..12u64 {
            let report = check(seed * 7919 + 3, 31, 9);
            println!("seed {seed}: {report:?}");
            for (i, limit) in [3, 6, 11, 17, 24].into_iter().enumerate() {
                assert!(
                    report.tier_floor[i].is_some_and(|f| f <= limit),
                    "seed {seed}: tier {} not reached by floor {limit}: {report:?}",
                    i + 1
                );
            }
            assert_eq!(report.guardians, 6, "{report:?}");
            assert!(report.finished, "{report:?}");
        }
    }
}
