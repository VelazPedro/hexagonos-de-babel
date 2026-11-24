//! Layout algorithms chosen by orientation words.

mod arena;
mod canonical;
mod islands;
mod maze;
mod pit;
mod spiral;

use super::grid::{Grid, Pos, Tile, entry_pos};
use super::hex::Dir;
use super::validation::{all_reachable, nearest_walkable};
use crate::babel::seeds::{Dice, Seed};
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum LayoutKind {
    /// Borges' hexagon: shelves on four walls and a railed ventilation shaft.
    Canonical,
    Maze,
    Spiral,
    Arena,
    Islands,
    Pit,
    /// The player's own hexagon, with room to build.
    Home,
}

impl LayoutKind {
    pub fn from_tag(tag: &str) -> Option<LayoutKind> {
        match tag {
            "maze" => Some(LayoutKind::Maze),
            "spiral" => Some(LayoutKind::Spiral),
            "arena" => Some(LayoutKind::Arena),
            "islands" => Some(LayoutKind::Islands),
            "pit" => Some(LayoutKind::Pit),
            _ => None,
        }
    }
}

pub struct Layout {
    pub grid: Grid,
    /// Central reachable tile; everything is validated from here.
    pub hub: Pos,
    /// Retry index that produced a valid layout (`MAX_RETRIES` means the fallback).
    pub attempt: u32,
}

pub const MAX_RETRIES: u32 = 6;

fn preferred_hub(kind: LayoutKind) -> Pos {
    match kind {
        LayoutKind::Canonical | LayoutKind::Pit => Pos::new(20, 25),
        LayoutKind::Home => Pos::new(20, 18),
        _ => Pos::new(20, 17),
    }
}

fn paint(kind: LayoutKind, dice: &mut Dice, exits: &[bool; 6], grid: &mut Grid) {
    match kind {
        LayoutKind::Canonical => canonical::paint(dice, exits, grid),
        LayoutKind::Maze => maze::paint(dice, grid),
        LayoutKind::Spiral => spiral::paint(dice, grid),
        LayoutKind::Arena => arena::paint(dice, grid),
        LayoutKind::Islands => islands::paint(dice, grid),
        LayoutKind::Pit => pit::paint(dice, grid),
        LayoutKind::Home => canonical::paint_home(grid),
    }
}

/// Clears the landing zones so entering is never into a wall.
fn clear_landings(grid: &mut Grid, hub: Pos) {
    for d in Dir::ALL {
        grid.disc(entry_pos(d), 2, Tile::Floor);
    }
    grid.disc(hub, 2, Tile::Floor);
}

fn targets() -> [Pos; 6] {
    Dir::ALL.map(entry_pos)
}

/// Builds a layout where all six doorways are reachable from the hub. Doorway
/// tiles are only carved for sides listed in `exits`. Invalid attempts are
/// regenerated with a deterministic retry sub-seed.
pub fn build(seed: &Seed, kind: LayoutKind, exits: &[bool; 6]) -> Layout {
    let mut attempt = 0;
    let (mut grid, hub) = loop {
        let label = if attempt == 0 {
            "layout".to_string()
        } else {
            format!("layout-retry-{attempt}")
        };
        let mut dice = Dice::labelled(seed, &label);
        let mut grid = Grid::hexagon();
        paint(kind, &mut dice, exits, &mut grid);
        let hub = nearest_walkable(&grid, preferred_hub(kind));
        clear_landings(&mut grid, hub);
        if all_reachable(&grid, hub, &targets()) {
            break (grid, hub);
        }
        attempt += 1;
        if attempt >= MAX_RETRIES {
            // Guaranteed fallback: wide corridors from every doorway to the hub.
            for t in targets() {
                grid.thick_line(t, hub, 1, Tile::Floor, |tile| tile.walkable());
            }
            break (grid, hub);
        }
    };
    for d in Dir::ALL {
        if exits[d.index()] {
            grid.open_door(d);
        }
    }
    Layout { grid, hub, attempt }
}
