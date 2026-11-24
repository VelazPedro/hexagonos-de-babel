//! 2D lighting geometry: visibility polygons over the tile grid and the
//! gameplay query "is this point lit?".

use super::vec::{V2, v2};
use crate::world::grid::{Grid, TILE};

/// Maximum simultaneous lights drawn.
pub const MAX_LIGHTS: usize = 64;
/// How far light bleeds into a wall so its faces are visible.
const WALL_BLEED: f32 = 11.0;
const FAN_RAYS: usize = 48;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Light {
    pub pos: V2,
    /// World pixels.
    pub radius: f32,
    /// Linear RGB multiplied by intensity.
    pub color: [f32; 3],
    /// 0 = steady, 1 = strong flicker.
    pub flicker: f32,
    /// Gameplay light reveals shadow creatures; faint glows do not.
    pub reveals: bool,
    /// Higher is kept first when culling.
    pub priority: u8,
}

fn opaque(grid: &Grid, x: i32, y: i32) -> bool {
    grid.get(x, y).opaque()
}

/// Distance travelled along `dir` before entering an opaque tile, up to `max`.
pub fn ray_distance(grid: &Grid, origin: V2, dir: V2, max: f32) -> f32 {
    let mut tx = (origin.x / TILE).floor() as i32;
    let mut ty = (origin.y / TILE).floor() as i32;
    if opaque(grid, tx, ty) {
        return 0.0;
    }
    let step_x = if dir.x >= 0.0 { 1 } else { -1 };
    let step_y = if dir.y >= 0.0 { 1 } else { -1 };
    let delta_x = if dir.x.abs() < 1e-6 {
        f32::INFINITY
    } else {
        TILE / dir.x.abs()
    };
    let delta_y = if dir.y.abs() < 1e-6 {
        f32::INFINITY
    } else {
        TILE / dir.y.abs()
    };
    let edge_x = (tx + i32::from(step_x > 0)) as f32 * TILE;
    let edge_y = (ty + i32::from(step_y > 0)) as f32 * TILE;
    let mut next_x = if delta_x.is_finite() {
        (edge_x - origin.x) / dir.x
    } else {
        f32::INFINITY
    };
    let mut next_y = if delta_y.is_finite() {
        (edge_y - origin.y) / dir.y
    } else {
        f32::INFINITY
    };
    loop {
        let t = next_x.min(next_y);
        if t >= max {
            return max;
        }
        if next_x < next_y {
            tx += step_x;
            next_x += delta_x;
        } else {
            ty += step_y;
            next_y += delta_y;
        }
        if opaque(grid, tx, ty) {
            return t;
        }
    }
}

pub fn line_clear(grid: &Grid, a: V2, b: V2) -> bool {
    let d = b - a;
    let len = d.len();
    len < 0.5 || ray_distance(grid, a, d.norm(), len) >= len - 0.5
}

/// Polygon (sorted by angle) of everything the light reaches.
pub fn visibility_polygon(grid: &Grid, origin: V2, radius: f32) -> Vec<V2> {
    let mut angles: Vec<f32> = (0..FAN_RAYS)
        .map(|i| i as f32 / FAN_RAYS as f32 * std::f32::consts::TAU - std::f32::consts::PI)
        .collect();
    let min_x = ((origin.x - radius) / TILE).floor() as i32;
    let max_x = ((origin.x + radius) / TILE).floor() as i32;
    let min_y = ((origin.y - radius) / TILE).floor() as i32;
    let max_y = ((origin.y + radius) / TILE).floor() as i32;
    // A grid vertex is a shadow-casting corner when the four tiles around it
    // hold an odd number of walls, or two diagonal ones.
    for vy in min_y..=max_y + 1 {
        for vx in min_x..=max_x + 1 {
            let a = opaque(grid, vx - 1, vy - 1);
            let b = opaque(grid, vx, vy - 1);
            let c = opaque(grid, vx - 1, vy);
            let d = opaque(grid, vx, vy);
            let count = u8::from(a) + u8::from(b) + u8::from(c) + u8::from(d);
            let corner = count == 1 || count == 3 || (count == 2 && a == d);
            if !corner {
                continue;
            }
            let p = v2(vx as f32 * TILE, vy as f32 * TILE) - origin;
            if p.len() > radius + TILE {
                continue;
            }
            let angle = p.angle();
            angles.extend([angle - 0.0025, angle + 0.0025]);
        }
    }
    angles.sort_by(f32::total_cmp);
    angles
        .into_iter()
        .map(|a| {
            let dir = V2::from_angle(a);
            let hit = ray_distance(grid, origin, dir, radius);
            origin + dir * (hit + WALL_BLEED).min(radius)
        })
        .collect()
}

/// True when any revealing light reaches `point` with a clear line of sight.
pub fn is_lit(grid: &Grid, lights: &[Light], point: V2) -> bool {
    lights
        .iter()
        .any(|l| l.reveals && l.pos.dist(point) < l.radius * 0.82 && line_clear(grid, l.pos, point))
}

/// Keeps at most `MAX_LIGHTS` lights that can touch the view rectangle.
pub fn cull(mut lights: Vec<Light>, view_min: V2, view_max: V2) -> Vec<Light> {
    lights.retain(|l| {
        l.pos.x + l.radius >= view_min.x
            && l.pos.x - l.radius <= view_max.x
            && l.pos.y + l.radius >= view_min.y
            && l.pos.y - l.radius <= view_max.y
    });
    if lights.len() > MAX_LIGHTS {
        let center = (view_min + view_max) * 0.5;
        lights.sort_by(|a, b| {
            b.priority
                .cmp(&a.priority)
                .then(a.pos.dist(center).total_cmp(&b.pos.dist(center)))
        });
        lights.truncate(MAX_LIGHTS);
    }
    lights
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::world::grid::Tile;

    #[test]
    fn walls_block_light_and_polygons_stay_in_radius() {
        let mut grid = Grid::hexagon();
        for y in 10..24 {
            grid.set(22, y, Tile::Wall);
        }
        let origin = v2(20.0 * TILE, 17.0 * TILE);
        let behind = v2(25.0 * TILE, 17.0 * TILE);
        let open = v2(16.0 * TILE, 17.0 * TILE);
        assert!(!line_clear(&grid, origin, behind));
        assert!(line_clear(&grid, origin, open));
        let light = Light {
            pos: origin,
            radius: 120.0,
            color: [1.0; 3],
            flicker: 0.0,
            reveals: true,
            priority: 1,
        };
        assert!(is_lit(&grid, &[light], open));
        assert!(!is_lit(&grid, &[light], behind));
        let poly = visibility_polygon(&grid, origin, 120.0);
        assert!(poly.len() >= FAN_RAYS);
        assert!(poly.iter().all(|p| p.dist(origin) <= 120.5));
        assert!(
            poly.iter()
                .all(|p| p.x <= 22.0 * TILE + WALL_BLEED + 1.0 || (p.y - origin.y).abs() > 60.0)
        );
    }

    #[test]
    fn culling_caps_the_light_count() {
        let l = Light {
            pos: v2(0.0, 0.0),
            radius: 10.0,
            color: [1.0; 3],
            flicker: 0.0,
            reveals: false,
            priority: 0,
        };
        let culled = cull(vec![l; 200], v2(-1.0, -1.0), v2(1.0, 1.0));
        assert_eq!(culled.len(), MAX_LIGHTS);
    }
}
