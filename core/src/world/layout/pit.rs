//! A wide uncrossable central well.

use crate::babel::seeds::Dice;
use crate::world::grid::{Grid, HEIGHT, Tile, WIDTH};

pub(super) fn paint(dice: &mut Dice, grid: &mut Grid) {
    let rx = dice.range(9, 11) as f32;
    let ry = dice.range(6, 7) as f32;
    for y in 0..HEIGHT {
        for x in 0..WIDTH {
            let dx = (x as f32 + 0.5 - 20.0) / rx;
            let dy = (y as f32 + 0.5 - 17.0) / ry;
            let d = dx * dx + dy * dy;
            if d <= 1.0 {
                grid.set_inner(x, y, Tile::Pit);
            } else if d <= 1.35 {
                grid.set_inner(x, y, Tile::Rail);
            }
        }
    }
}
