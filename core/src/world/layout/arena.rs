//! Open central arena ringed by pillars.

use crate::babel::seeds::Dice;
use crate::world::grid::{Grid, Pos, Tile};

pub(super) fn paint(dice: &mut Dice, grid: &mut Grid) {
    let pillars = [
        Pos::new(15, 9),
        Pos::new(24, 9),
        Pos::new(11, 16),
        Pos::new(28, 16),
        Pos::new(15, 23),
        Pos::new(24, 23),
    ];
    for p in pillars {
        for dy in 0..2 {
            for dx in 0..2 {
                grid.set_inner(p.x + dx, p.y + dy, Tile::Wall);
            }
        }
    }
    // A few short shelf stubs near the rim.
    for _ in 0..dice.range(2, 5) {
        let x = dice.range(6, 33);
        let y = if dice.chance(0.5) {
            dice.range(4, 7)
        } else {
            dice.range(26, 29)
        };
        for dx in 0..dice.range(2, 4) {
            grid.set_inner(x + dx, y, Tile::Wall);
        }
    }
}
