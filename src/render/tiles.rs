//! Procedural 16×16 tiles: the same site seed always paints the same gallery.

use super::canvas::{Canvas, Rgba, hex, mix, rgb, shade, with_alpha};
use babel_core::data::{BiomeDef, Hazard};
use babel_core::world::grid::{HEIGHT, Pos, Tile, WIDTH};
use babel_core::world::site::Site;

pub const T: i32 = 16;
/// How far a wall's top face rises above its footprint.
pub const WALL_RISE: i32 = 10;
pub const HAZARD_FRAMES: usize = 4;

pub const NIGHT: Rgba = rgb(26, 16, 40);

#[derive(Clone, Copy)]
pub struct Palette {
    pub floor: [Rgba; 3],
    pub top: Rgba,
    pub front: Rgba,
    pub trim: Rgba,
}

pub struct SiteArt {
    /// Everything flat, pre-rendered once.
    pub ground: Canvas,
    /// Wall sprites with their tile, drawn y-sorted.
    pub walls: Vec<(Pos, Canvas)>,
    pub hazard_frames: Vec<Canvas>,
    pub hazard_tiles: Vec<Pos>,
    pub hazard: Hazard,
}

/// Cheap deterministic per-pixel noise.
pub fn noise(seed: u32, x: i32, y: i32) -> u32 {
    let mut h = seed ^ (x as u32).wrapping_mul(0x8DA6_B343) ^ (y as u32).wrapping_mul(0xD816_3841);
    h ^= h >> 13;
    h = h.wrapping_mul(0x5BD1_E995);
    h ^ (h >> 15)
}

/// Distance-like value to the edges of a flat-top hexagonal inlay.
fn hex_line(x: i32, y: i32) -> bool {
    const W: i32 = 36;
    const H: i32 = 32;
    let column = x.div_euclid(W * 3 / 4);
    let local_x = x.rem_euclid(W * 3 / 4);
    let local_y = (y + if column % 2 == 0 { 0 } else { H / 2 }).rem_euclid(H);
    let slant = W / 4;
    if local_x >= slant {
        local_y == 0
    } else {
        // The two slanted edges meet in the middle of the cell.
        let up = (H / 2) - local_x * (H / 2) / slant;
        local_y == up || local_y == H - up
    }
}

fn floor_pixel(p: &Palette, seed: u32, wx: i32, wy: i32) -> Rgba {
    let tile = noise(seed ^ 0x51, wx.div_euclid(T), wy.div_euclid(T));
    let mut c = if tile.is_multiple_of(9) {
        p.floor[1]
    } else {
        p.floor[0]
    };
    let n = noise(seed, wx, wy) % 100;
    if n < 4 {
        c = p.floor[2];
    } else if n < 8 {
        c = p.floor[1];
    }
    if hex_line(wx, wy) {
        c = mix(c, p.floor[1], 0.85);
        c = shade(c, 0.94);
    } else if hex_line(wx, wy - 1) {
        c = mix(c, p.floor[2], 0.55);
    }
    c
}

fn paint_floor(g: &mut Canvas, p: &Palette, seed: u32, tx: i32, ty: i32) {
    for y in 0..T {
        for x in 0..T {
            let (wx, wy) = (tx * T + x, ty * T + y);
            g.set(wx, wy, floor_pixel(p, seed, wx, wy));
        }
    }
    // Occasional cracks and worn corners.
    let v = noise(seed ^ 0x77, tx, ty);
    if v.is_multiple_of(17) {
        let mut x = 3 + (v >> 8) as i32 % 8;
        for y in 4..11 {
            g.set(tx * T + x, ty * T + y, shade(p.floor[1], 0.78));
            x += ((noise(seed, y, v as i32) % 3) as i32) - 1;
        }
    }
}

fn paint_pit(g: &mut Canvas, p: &Palette, seed: u32, tx: i32, ty: i32, edge_above: bool) {
    for y in 0..T {
        for x in 0..T {
            let (wx, wy) = (tx * T + x, ty * T + y);
            let mut c = NIGHT;
            if noise(seed ^ 0x9, wx, wy).is_multiple_of(211) {
                c = mix(NIGHT, p.trim, 0.35);
            }
            if edge_above && y < 9 {
                // The inner face of the shaft: lower floors fading into the dark.
                let fade = y as f32 / 9.0;
                c = mix(shade(p.front, 0.9), NIGHT, fade);
                if y == 4 || (x % 4 == 0 && y > 4) {
                    c = shade(c, 0.72);
                }
                if y == 0 {
                    c = mix(p.trim, p.front, 0.4);
                }
            }
            g.set(wx, wy, c);
        }
    }
}

fn paint_rail(g: &mut Canvas, p: &Palette, seed: u32, tx: i32, ty: i32) {
    paint_floor(g, p, seed, tx, ty);
    let (ox, oy) = (tx * T, ty * T);
    g.fill(ox, oy + 13, T, 2, with_alpha(NIGHT, 80));
    g.fill(ox, oy + 5, T, 2, p.trim);
    g.fill(ox, oy + 7, T, 1, shade(p.trim, 0.55));
    for post in [2, 10] {
        g.fill(ox + post, oy + 4, 3, 10, shade(p.trim, 0.75));
        g.fill(ox + post, oy + 4, 1, 10, p.trim);
        g.fill(
            ox + post,
            oy + 3,
            3,
            2,
            mix(p.trim, rgb(255, 255, 255), 0.45),
        );
    }
}

fn paint_bridge(g: &mut Canvas, p: &Palette, tx: i32, ty: i32) {
    let (ox, oy) = (tx * T, ty * T);
    let wood = mix(p.top, p.floor[2], 0.45);
    for y in 0..T {
        let seam = y % 5 == 4;
        let c = if seam { shade(wood, 0.6) } else { wood };
        g.fill(ox, oy + y, T, 1, c);
    }
    g.fill(ox + 2, oy + 1, 1, 1, p.trim);
    g.fill(ox + 13, oy + 11, 1, 1, p.trim);
}

/// A wall: top face raised by `WALL_RISE` and, when open below, a shelf front.
fn wall_sprite(p: &Palette, biome: &BiomeDef, seed: u32, pos: Pos, open: [bool; 4]) -> Canvas {
    let [open_up, open_right, open_down, open_left] = open;
    let mut s = Canvas::new(T, T + WALL_RISE);
    let variant = noise(seed, pos.x, pos.y);
    for y in 0..T {
        for x in 0..T {
            let n = noise(variant, x, y) % 100;
            let mut c = if n < 6 { shade(p.top, 1.12) } else { p.top };
            if y % 8 == 7 {
                c = shade(p.top, 0.86);
            }
            s.set(x, y, c);
        }
    }
    let highlight = mix(p.top, p.trim, 0.55);
    let dark = shade(p.front, 0.7);
    if open_up {
        s.fill(0, 0, T, 2, highlight);
    }
    if open_left {
        s.fill(0, 0, 1, T, highlight);
    }
    if open_right {
        s.fill(T - 1, 0, 1, T, dark);
    }
    if open_down {
        s.fill(0, T - 2, T, 1, p.trim);
        s.fill(0, T - 1, T, 1, dark);
        // Front face: a shelf crowded with coloured spines.
        s.fill(0, T, T, WALL_RISE, shade(p.front, 0.75));
        let mut x = 1;
        let mut k = 0;
        while x < T - 1 {
            let h = noise(variant ^ 0xA5, x, k);
            let width = 2 + (h % 2) as i32;
            let height = 6 + ((h >> 4) % 3) as i32;
            let color = biome
                .spines
                .get((h >> 8) as usize % biome.spines.len().max(1))
                .map_or(p.trim, |c| hex(c));
            if !(h >> 16).is_multiple_of(9) {
                let w = width.min(T - 1 - x);
                s.fill(x, T + 8 - height, w, height, color);
                s.fill(
                    x,
                    T + 8 - height,
                    1,
                    height,
                    mix(color, rgb(255, 255, 255), 0.3),
                );
                s.fill(x, T + 10 - height, w, 1, shade(color, 0.65));
            }
            x += width + 1;
            k += 1;
        }
        s.fill(0, T + WALL_RISE - 2, T, 1, p.trim);
        s.fill(0, T + WALL_RISE - 1, T, 1, NIGHT);
        if open_left {
            s.fill(0, T, 1, WALL_RISE, dark);
        }
        if open_right {
            s.fill(T - 1, T, 1, WALL_RISE, NIGHT);
        }
    }
    // Biome weathering on the top face.
    let overlay = match biome.id.as_str() {
        "jardin" => Some((rgb(120, 216, 88), 5)),
        "agua" => Some((rgb(88, 220, 190), 9)),
        "hielo" => Some((rgb(244, 254, 255), 3)),
        "sal" => Some((rgb(255, 240, 246), 5)),
        "ceniza" => Some((rgb(150, 130, 140), 6)),
        "oxido" => Some((rgb(90, 208, 176), 9)),
        _ => None,
    };
    if let Some((color, rarity)) = overlay
        && variant.is_multiple_of(3)
    {
        for y in 0..7 {
            for x in 0..T {
                if noise(variant ^ 0x33, x / 2, y / 2).is_multiple_of(rarity) {
                    s.set(x, y, color);
                }
            }
        }
    }
    s
}

fn hazard_frame(hazard: Hazard, p: &Palette, frame: usize) -> Canvas {
    let mut c = Canvas::new(T, T);
    let f = frame as i32;
    let inside = |x: i32, y: i32| {
        // Rounded blob so patches do not look like squares.
        let (dx, dy) = (x - 8, y - 8);
        dx * dx + dy * dy <= 92
    };
    for y in 0..T {
        for x in 0..T {
            if !inside(x, y) {
                continue;
            }
            let n = noise(0x4A, x, y) % 100;
            let color = match hazard {
                Hazard::None => None,
                Hazard::Water => {
                    let wave = (x + y / 2 + f * 2).rem_euclid(8);
                    Some(if wave == 0 && y % 3 == 0 {
                        [214, 252, 255, 230]
                    } else {
                        [52, 150, 214, 175]
                    })
                }
                Hazard::Ice => {
                    let shine = (x - y + f * 3).rem_euclid(14);
                    Some(if shine < 2 {
                        [255, 255, 255, 190]
                    } else {
                        [196, 236, 255, 150]
                    })
                }
                Hazard::Embers => Some(if n < 14 && !(n as usize + frame).is_multiple_of(3) {
                    [255, 190, 66, 255]
                } else if n < 40 {
                    [214, 74, 46, 225]
                } else {
                    [70, 36, 52, 190]
                }),
                Hazard::Vines => {
                    let curl = ((x * 2 + (y + f / 2) * 3) % 11 < 3)
                        || ((x * 3 - y * 2).rem_euclid(13) < 2);
                    curl.then_some(if n < 30 {
                        [150, 232, 100, 255]
                    } else {
                        [48, 138, 84, 255]
                    })
                }
                Hazard::Brine => (n < 38).then_some(if n < 10 {
                    [255, 255, 255, 235]
                } else {
                    [248, 196, 216, 205]
                }),
                Hazard::Dunes => {
                    let ripple = (y + (x + f) / 3).rem_euclid(5);
                    match ripple {
                        0 => Some([255, 232, 160, 230]),
                        1 => Some([196, 146, 80, 190]),
                        _ => Some([232, 190, 106, 120]),
                    }
                }
                Hazard::Spikes => {
                    let tip = (x % 5 == 2) && (y % 6 < 4);
                    let base = (y % 6 == 3) && (x % 5 >= 1) && (x % 5 <= 3);
                    if tip {
                        Some([214, 222, 234, 255])
                    } else {
                        base.then_some([120, 72, 60, 255])
                    }
                }
                Hazard::Mirrors => {
                    let shine = (x + y + f * 4).rem_euclid(16) < 2;
                    Some(if shine {
                        [255, 255, 255, 220]
                    } else {
                        [168, 190, 255, 110]
                    })
                }
                Hazard::Gloom => {
                    let swirl = noise(0x91, (x + f) / 3, (y + f / 2) / 3).is_multiple_of(4);
                    Some(if swirl {
                        [120, 76, 190, 170]
                    } else {
                        [38, 24, 66, 150]
                    })
                }
                Hazard::Bones => (n < 16).then_some(if (x + y) % 3 == 0 {
                    [150, 134, 120, 255]
                } else {
                    [240, 232, 208, 255]
                }),
            };
            if let Some(color) = color {
                c.set(x, y, color);
            }
        }
    }
    // A thin rim ties the patch to the biome.
    if matches!(hazard, Hazard::Water | Hazard::Ice) {
        for y in 0..T {
            for x in 0..T {
                let d = (x - 8) * (x - 8) + (y - 8) * (y - 8);
                if (80..=92).contains(&d) {
                    c.set(x, y, with_alpha(p.trim, 150));
                }
            }
        }
    }
    c
}

pub fn palette_of(biome: &BiomeDef) -> Palette {
    Palette {
        floor: [
            hex(&biome.floor[0]),
            hex(&biome.floor[1]),
            hex(&biome.floor[2]),
        ],
        top: hex(&biome.wall_top),
        front: hex(&biome.wall_front),
        trim: hex(&biome.trim),
    }
}

pub fn build(site: &Site, biome: &BiomeDef) -> SiteArt {
    let palette = palette_of(biome);
    let seed = u32::from_le_bytes([site.seed[0], site.seed[1], site.seed[2], site.seed[3]]);
    let mut ground = Canvas::new(WIDTH * T, HEIGHT * T);
    let mut walls = Vec::new();
    let mut hazard_tiles = Vec::new();
    let grid = &site.grid;
    for ty in 0..HEIGHT {
        for tx in 0..WIDTH {
            match grid.get(tx, ty) {
                Tile::Void => {}
                Tile::Wall => {
                    let open = [(0, -1), (1, 0), (0, 1), (-1, 0)].map(|(dx, dy)| {
                        !matches!(grid.get(tx + dx, ty + dy), Tile::Wall | Tile::Void)
                    });
                    // Floor under the wall so its raised face has no gap.
                    paint_floor(&mut ground, &palette, seed, tx, ty);
                    let below = grid.get(tx, ty + 1);
                    let mut faces = open;
                    // The hexagon's lower rim also shows its front.
                    faces[2] = open[2] || below == Tile::Void;
                    let pos = Pos::new(tx, ty);
                    walls.push((pos, wall_sprite(&palette, biome, seed, pos, faces)));
                }
                Tile::Pit => {
                    let edge = grid.get(tx, ty - 1) != Tile::Pit;
                    paint_pit(&mut ground, &palette, seed, tx, ty, edge);
                }
                Tile::Rail => paint_rail(&mut ground, &palette, seed, tx, ty),
                Tile::Bridge => paint_bridge(&mut ground, &palette, tx, ty),
                Tile::Door => {
                    paint_floor(&mut ground, &palette, seed, tx, ty);
                    ground.fill(tx * T, ty * T, T, T, with_alpha(palette.trim, 46));
                    if (tx + ty) % 2 == 0 {
                        ground.fill(tx * T + 6, ty * T + 6, 4, 4, with_alpha(palette.trim, 150));
                    }
                }
                Tile::Floor => {
                    paint_floor(&mut ground, &palette, seed, tx, ty);
                    // Soft contact shadow under walls to the north.
                    if grid.get(tx, ty - 1) == Tile::Wall {
                        ground.fill(tx * T, ty * T, T, 4, with_alpha(NIGHT, 66));
                        ground.fill(tx * T, ty * T + 4, T, 3, with_alpha(NIGHT, 30));
                    }
                    if grid.get(tx - 1, ty) == Tile::Wall {
                        ground.fill(tx * T, ty * T, 3, T, with_alpha(NIGHT, 44));
                    }
                }
                Tile::Hazard => {
                    paint_floor(&mut ground, &palette, seed, tx, ty);
                    hazard_tiles.push(Pos::new(tx, ty));
                }
            }
        }
    }
    let hazard_frames = (0..HAZARD_FRAMES)
        .map(|f| hazard_frame(biome.hazard, &palette, f))
        .collect();
    SiteArt {
        ground,
        walls,
        hazard_frames,
        hazard_tiles,
        hazard: biome.hazard,
    }
}
