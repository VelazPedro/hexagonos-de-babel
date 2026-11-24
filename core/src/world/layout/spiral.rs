//! Concentric hexagonal walls whose gaps rotate, forming a spiral walk.

use crate::babel::seeds::Dice;
use crate::world::grid::{Grid, HEIGHT, Tile, WIDTH, hex_metric};

/// Six axis directions (flat-top sides) used to place gaps.
const AXES: [(f32, f32); 6] = [
    (0.0, -1.0),
    (0.87, -0.5),
    (0.87, 0.5),
    (0.0, 1.0),
    (-0.87, 0.5),
    (-0.87, -0.5),
];

pub(super) fn paint(dice: &mut Dice, grid: &mut Grid) {
    let start = dice.index(6);
    let turn = if dice.chance(0.5) { 1 } else { 5 };
    let rings = [(150, 195), (300, 345), (450, 495)];
    for y in 0..HEIGHT {
        for x in 0..WIDTH {
            let m = hex_metric(x, y);
            for (i, &(lo, hi)) in rings.iter().enumerate() {
                if !(lo..=hi).contains(&m) {
                    continue;
                }
                let axis = AXES[(start + i * turn * 2) % 6];
                let dx = x as f32 + 0.5 - 20.0;
                let dy = y as f32 + 0.5 - 17.0;
                let along = dx * axis.0 + dy * axis.1;
                let across = (dx * axis.1 - dy * axis.0).abs();
                if !(along > 0.0 && across < 2.2) {
                    grid.set_inner(x, y, Tile::Wall);
                }
            }
        }
    }
}
