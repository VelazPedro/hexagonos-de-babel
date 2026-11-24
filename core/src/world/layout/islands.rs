//! Islands over the abyss joined by bridges.

use crate::babel::seeds::Dice;
use crate::world::grid::{Grid, HEIGHT, Pos, Tile, WIDTH, entry_pos};
use crate::world::hex::Dir;

pub(super) fn paint(dice: &mut Dice, grid: &mut Grid) {
    for y in 0..HEIGHT {
        for x in 0..WIDTH {
            grid.set_inner(x, y, Tile::Pit);
        }
    }
    let hub = Pos::new(20, 17);
    let mut islands = vec![(hub, dice.range(4, 5))];
    for d in Dir::ALL {
        islands.push((entry_pos(d), dice.range(3, 4)));
    }
    for _ in 0..dice.range(2, 4) {
        let p = Pos::new(dice.range(10, 29), dice.range(8, 25));
        islands.push((p, dice.range(2, 4)));
    }
    for &(c, radius) in &islands {
        grid.disc(c, radius, Tile::Floor);
    }
    // Spoke bridges plus a ring between neighbouring doorway islands.
    for i in 1..islands.len() {
        let target = islands[i].0;
        grid.thick_line(target, hub, 1, Tile::Bridge, |t| t == Tile::Floor);
    }
    for i in 0..6 {
        if dice.chance(0.5) {
            let a = islands[1 + i].0;
            let b = islands[1 + (i + 1) % 6].0;
            grid.thick_line(a, b, 0, Tile::Bridge, |t| t.walkable());
        }
    }
}
