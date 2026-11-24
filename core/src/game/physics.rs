//! Circle-versus-tile movement.

use super::vec::{V2, v2};
use crate::world::grid::{Grid, TILE, Tile};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Mobility {
    /// Needs walkable floor and collides with solid objects.
    Ground,
    /// Crosses pits and rails, stopped by walls.
    Flying,
    /// Crosses walls, but never leaves the hexagon.
    Phasing,
}

/// A solid round obstacle (resource node, chest or station).
#[derive(Clone, Copy, Debug)]
pub struct Obstacle {
    pub pos: V2,
    pub radius: f32,
}

fn tile_blocks(tile: Tile, mobility: Mobility) -> bool {
    match mobility {
        Mobility::Ground => !tile.walkable(),
        Mobility::Flying => !tile.flyable(),
        Mobility::Phasing => tile == Tile::Void,
    }
}

pub fn tile_at(grid: &Grid, p: V2) -> Tile {
    grid.get((p.x / TILE).floor() as i32, (p.y / TILE).floor() as i32)
}

pub fn blocked(grid: &Grid, obstacles: &[Obstacle], p: V2, radius: f32, mobility: Mobility) -> bool {
    let r = radius;
    let d = r * 0.72;
    let samples = [
        v2(0.0, 0.0),
        v2(r, 0.0),
        v2(-r, 0.0),
        v2(0.0, r),
        v2(0.0, -r),
        v2(d, d),
        v2(-d, d),
        v2(d, -d),
        v2(-d, -d),
    ];
    if samples
        .iter()
        .any(|&s| tile_blocks(tile_at(grid, p + s), mobility))
    {
        return true;
    }
    mobility == Mobility::Ground
        && obstacles
            .iter()
            .any(|o| o.pos.dist(p) < o.radius + radius)
}

/// Moves with wall sliding. Returns the new position and whether anything blocked.
pub fn move_body(
    grid: &Grid,
    obstacles: &[Obstacle],
    mut pos: V2,
    delta: V2,
    radius: f32,
    mobility: Mobility,
) -> (V2, bool) {
    let steps = (delta.len() / 3.0).ceil().max(1.0) as u32;
    let step = delta * (1.0 / steps as f32);
    let mut hit = false;
    // A body already overlapping something may always move out of it.
    let stuck = blocked(grid, obstacles, pos, radius, mobility);
    for _ in 0..steps {
        for axis in [v2(step.x, 0.0), v2(0.0, step.y)] {
            if axis == V2::ZERO {
                continue;
            }
            let next = pos + axis;
            let allowed = if stuck {
                !tile_blocks(tile_at(grid, next), mobility)
            } else {
                !blocked(grid, obstacles, next, radius, mobility)
            };
            if allowed {
                pos = next;
            } else {
                hit = true;
            }
        }
    }
    (pos, hit)
}
