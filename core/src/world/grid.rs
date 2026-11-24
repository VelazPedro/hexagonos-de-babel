//! The 40×34 tile grid of a site and its hexagonal geometry.

use super::hex::Dir;
use serde::{Deserialize, Serialize};

pub const WIDTH: i32 = 40;
pub const HEIGHT: i32 = 34;
/// Tile size in world pixels.
pub const TILE: f32 = 16.0;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Tile {
    /// Outside the hexagon.
    Void,
    /// Bookshelf wall: blocks movement and light.
    Wall,
    Floor,
    Bridge,
    /// Open doorway on the hexagon border.
    Door,
    /// Ventilation shaft: blocks movement, not light.
    Pit,
    Rail,
    /// Biome hazard patch.
    Hazard,
}

impl Tile {
    pub fn walkable(self) -> bool {
        matches!(self, Tile::Floor | Tile::Bridge | Tile::Door | Tile::Hazard)
    }

    pub fn opaque(self) -> bool {
        matches!(self, Tile::Wall | Tile::Void)
    }

    /// Whether projectiles and flying enemies can pass over it.
    pub fn flyable(self) -> bool {
        !self.opaque()
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Pos {
    pub x: i32,
    pub y: i32,
}

impl Pos {
    pub const fn new(x: i32, y: i32) -> Self {
        Pos { x, y }
    }

    /// World-pixel coordinates of the tile centre.
    pub fn center(self) -> (f32, f32) {
        ((self.x as f32 + 0.5) * TILE, (self.y as f32 + 0.5) * TILE)
    }

    pub fn from_world(x: f32, y: f32) -> Pos {
        Pos::new((x / TILE).floor() as i32, (y / TILE).floor() as i32)
    }

    pub fn dist2(self, o: Pos) -> i32 {
        (self.x - o.x).pow(2) + (self.y - o.y).pow(2)
    }
}

/// `(dx, dy)` from the centre of the hexagon, in half-tiles.
fn doubled(x: i32, y: i32) -> (i32, i32) {
    ((2 * x + 1 - WIDTH).abs(), (2 * y + 1 - HEIGHT).abs())
}

/// Normalised hexagonal "radius": 0 at the centre, 680 on the border.
pub fn hex_metric(x: i32, y: i32) -> i32 {
    let (dx, dy) = doubled(x, y);
    (dy * 20).max(17 * dx + 10 * dy)
}

pub const HEX_BORDER: i32 = 680;
/// Tiles with a metric below this are playable interior (the rest is the wall ring).
pub const HEX_INTERIOR: i32 = 612;

pub fn in_hex(x: i32, y: i32) -> bool {
    hex_metric(x, y) <= HEX_BORDER
}

pub fn in_interior(x: i32, y: i32) -> bool {
    hex_metric(x, y) <= HEX_INTERIOR
}

/// Midpoint of a hexagon side, in tile units.
pub fn door_mid(d: Dir) -> (f32, f32) {
    match d {
        Dir::N => (20.0, 0.0),
        Dir::NE => (35.0, 8.5),
        Dir::SE => (35.0, 25.5),
        Dir::S => (20.0, 34.0),
        Dir::SW => (5.0, 25.5),
        Dir::NW => (5.0, 8.5),
    }
}

pub const CENTER: (f32, f32) = (20.0, 17.0);

/// Where the player appears when entering through side `d`.
pub fn entry_pos(d: Dir) -> Pos {
    let (mx, my) = door_mid(d);
    let x = mx + (CENTER.0 - mx) * 0.24;
    let y = my + (CENTER.1 - my) * 0.24;
    Pos::new(x.floor() as i32, y.floor() as i32)
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Grid {
    tiles: Vec<Tile>,
}

impl Grid {
    /// An empty hexagon: wall ring around a floor interior.
    pub fn hexagon() -> Grid {
        let mut tiles = Vec::with_capacity((WIDTH * HEIGHT) as usize);
        for y in 0..HEIGHT {
            for x in 0..WIDTH {
                tiles.push(if in_interior(x, y) {
                    Tile::Floor
                } else if in_hex(x, y) {
                    Tile::Wall
                } else {
                    Tile::Void
                });
            }
        }
        Grid { tiles }
    }

    fn offset(x: i32, y: i32) -> Option<usize> {
        if (0..WIDTH).contains(&x) && (0..HEIGHT).contains(&y) {
            Some((y * WIDTH + x) as usize)
        } else {
            None
        }
    }

    pub fn get(&self, x: i32, y: i32) -> Tile {
        Grid::offset(x, y).map_or(Tile::Void, |i| self.tiles[i])
    }

    pub fn at(&self, p: Pos) -> Tile {
        self.get(p.x, p.y)
    }

    pub fn set(&mut self, x: i32, y: i32, t: Tile) {
        if let Some(i) = Grid::offset(x, y) {
            self.tiles[i] = t;
        }
    }

    /// Sets an interior tile only, never touching the outer wall ring.
    pub fn set_inner(&mut self, x: i32, y: i32, t: Tile) {
        if in_interior(x, y) {
            self.set(x, y, t);
        }
    }

    pub fn walkable(&self, x: i32, y: i32) -> bool {
        self.get(x, y).walkable()
    }

    pub fn positions(&self) -> impl Iterator<Item = Pos> + '_ {
        (0..HEIGHT).flat_map(|y| (0..WIDTH).map(move |x| Pos::new(x, y)))
    }

    /// All 8 neighbours and the tile itself are walkable, so blocking it cuts no path.
    pub fn open_area(&self, p: Pos) -> bool {
        (-1..=1).all(|dy| (-1..=1).all(|dx| self.walkable(p.x + dx, p.y + dy)))
    }

    /// Fills a disc of interior tiles.
    pub fn disc(&mut self, c: Pos, radius: i32, t: Tile) {
        for y in c.y - radius..=c.y + radius {
            for x in c.x - radius..=c.x + radius {
                if Pos::new(x, y).dist2(c) <= radius * radius + radius / 2 {
                    self.set_inner(x, y, t);
                }
            }
        }
    }

    /// Paints a thick line, replacing only tiles accepted by `keep`.
    pub fn thick_line(&mut self, a: Pos, b: Pos, half: i32, t: Tile, keep: impl Fn(Tile) -> bool) {
        let steps = (b.x - a.x).abs().max((b.y - a.y).abs()).max(1);
        for i in 0..=steps {
            let x = a.x + ((b.x - a.x) * i * 2 + steps).div_euclid(2 * steps);
            let y = a.y + ((b.y - a.y) * i * 2 + steps).div_euclid(2 * steps);
            for dy in -half..=half {
                for dx in -half..=half {
                    if !keep(self.get(x + dx, y + dy)) {
                        self.set_inner(x + dx, y + dy, t);
                    }
                }
            }
        }
    }

    /// Carves the doorway through the wall ring on side `d`.
    pub fn open_door(&mut self, d: Dir) {
        let (mx, my) = door_mid(d);
        let inner = entry_pos(d).center();
        let (ix, iy) = (inner.0 / TILE, inner.1 / TILE);
        for y in 0..HEIGHT {
            for x in 0..WIDTH {
                if !in_hex(x, y) || in_interior(x, y) {
                    continue;
                }
                if segment_distance2((x as f32 + 0.5, y as f32 + 0.5), (mx, my), (ix, iy)) <= 4.9 {
                    self.set(x, y, Tile::Door);
                }
            }
        }
    }
}

fn segment_distance2(p: (f32, f32), a: (f32, f32), b: (f32, f32)) -> f32 {
    let (abx, aby) = (b.0 - a.0, b.1 - a.1);
    let len2 = (abx * abx + aby * aby).max(0.0001);
    let t = (((p.0 - a.0) * abx + (p.1 - a.1) * aby) / len2).clamp(0.0, 1.0);
    let (dx, dy) = (p.0 - a.0 - abx * t, p.1 - a.1 - aby * t);
    dx * dx + dy * dy
}
