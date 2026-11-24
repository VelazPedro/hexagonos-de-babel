use crate::babel::seeds::Dice;
use crate::world::grid::{Grid, HEIGHT, Pos, Tile, WIDTH, entry_pos, hex_metric};
use crate::world::hex::Dir;

/// Which side of the hexagon a tile faces.
fn side_of(x: i32, y: i32) -> Dir {
    let dx = (2 * x + 1 - WIDTH) as f32 / 2.0;
    let dy = (2 * y + 1 - HEIGHT) as f32 / 2.0;
    let slanted = 17.0 * dx.abs() + 10.0 * dy.abs();
    if dy.abs() * 20.0 >= slanted {
        if dy < 0.0 { Dir::N } else { Dir::S }
    } else {
        match (dx > 0.0, dy < 0.0) {
            (true, true) => Dir::NE,
            (true, false) => Dir::SE,
            (false, false) => Dir::SW,
            (false, true) => Dir::NW,
        }
    }
}

/// Free-standing bookshelves parallel to four walls.
fn shelf_walls(grid: &mut Grid, shelved: &[Dir]) {
    for y in 0..HEIGHT {
        for x in 0..WIDTH {
            let m = hex_metric(x, y);
            if !(470..=525).contains(&m) {
                continue;
            }
            let side = side_of(x, y);
            if !shelved.contains(&side) {
                continue;
            }
            // Leave a passage in the middle of each shelf run.
            let landing = entry_pos(side);
            if Pos::new(x, y).dist2(landing) <= 10 {
                continue;
            }
            grid.set_inner(x, y, Tile::Wall);
        }
    }
}

fn shaft(grid: &mut Grid, rx: i32, ry: i32) {
    for y in 0..HEIGHT {
        for x in 0..WIDTH {
            let dx = (2 * x + 1 - WIDTH).abs();
            let dy = (2 * y + 1 - HEIGHT).abs();
            if dx <= 2 * rx && dy <= 2 * ry && dx * ry + dy * rx / 2 <= 2 * rx * ry {
                grid.set_inner(x, y, Tile::Pit);
            } else if dx <= 2 * rx + 2 && dy <= 2 * ry + 2 && dx * ry + dy * rx / 2 <= 2 * rx * ry + rx + ry
            {
                grid.set_inner(x, y, Tile::Rail);
            }
        }
    }
}

pub(super) fn paint(dice: &mut Dice, exits: &[bool; 6], grid: &mut Grid) {
    // Prefer shelving the closed sides.
    let mut closed: Vec<Dir> = Dir::ALL.into_iter().filter(|d| !exits[d.index()]).collect();
    let mut open: Vec<Dir> = Dir::ALL.into_iter().filter(|d| exits[d.index()]).collect();
    dice.shuffle(&mut closed);
    dice.shuffle(&mut open);
    closed.extend(open);
    closed.truncate(4);
    shelf_walls(grid, &closed);
    let rx = dice.range(4, 5);
    shaft(grid, rx, 3);
}

/// The home hexagon keeps its centre clear for the building grid.
pub(super) fn paint_home(grid: &mut Grid) {
    shelf_walls(grid, &[Dir::N, Dir::NE, Dir::NW, Dir::SE]);
    // A small ventilation shaft to the south-west keeps the Borgesian silhouette.
    for y in 22..=24 {
        for x in 11..=14 {
            grid.set_inner(x, y, Tile::Pit);
        }
    }
    for y in 21..=25 {
        for x in 10..=15 {
            if grid.get(x, y) == Tile::Floor {
                grid.set_inner(x, y, Tile::Rail);
            }
        }
    }
}
