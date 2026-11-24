//! Recursive-backtracker maze clipped to the hexagon.

use crate::babel::seeds::Dice;
use crate::world::grid::{Grid, Tile, in_interior};

const CELL: i32 = 5;
const COLS: i32 = 8;
const ROWS: i32 = 7;
const ORIGIN_X: i32 = 0;
const ORIGIN_Y: i32 = -1;

fn cell_valid(c: (i32, i32)) -> bool {
    (0..COLS).contains(&c.0)
        && (0..ROWS).contains(&c.1)
        && in_interior(ORIGIN_X + c.0 * CELL + 2, ORIGIN_Y + c.1 * CELL + 2)
}

pub(super) fn paint(dice: &mut Dice, grid: &mut Grid) {
    // Grid lines become walls; passages are carved between visited cells.
    for y in 0..crate::world::grid::HEIGHT {
        for x in 0..crate::world::grid::WIDTH {
            let lx = (x - ORIGIN_X).rem_euclid(CELL);
            let ly = (y - ORIGIN_Y).rem_euclid(CELL);
            if lx == 0 || ly == 0 {
                grid.set_inner(x, y, Tile::Wall);
            }
        }
    }
    let open_between = |grid: &mut Grid, a: (i32, i32), b: (i32, i32)| {
        let ax = ORIGIN_X + a.0 * CELL;
        let ay = ORIGIN_Y + a.1 * CELL;
        if a.0 != b.0 {
            let wall_x = ORIGIN_X + a.0.max(b.0) * CELL;
            for y in ay + 1..ay + CELL {
                grid.set_inner(wall_x, y, Tile::Floor);
            }
        } else {
            let wall_y = ORIGIN_Y + a.1.max(b.1) * CELL;
            for x in ax + 1..ax + CELL {
                grid.set_inner(x, wall_y, Tile::Floor);
            }
        }
    };
    let mut visited = vec![false; (COLS * ROWS) as usize];
    let index = |c: (i32, i32)| (c.1 * COLS + c.0) as usize;
    let start = (COLS / 2, ROWS / 2);
    visited[index(start)] = true;
    let mut stack = vec![start];
    while let Some(&current) = stack.last() {
        let mut options: Vec<(i32, i32)> = [(1, 0), (-1, 0), (0, 1), (0, -1)]
            .iter()
            .map(|d| (current.0 + d.0, current.1 + d.1))
            .filter(|&n| cell_valid(n) && !visited[index(n)])
            .collect();
        if options.is_empty() {
            stack.pop();
            continue;
        }
        let next = options.swap_remove(dice.index(options.len()));
        visited[index(next)] = true;
        open_between(grid, current, next);
        stack.push(next);
    }
    // A few loops keep combat from becoming a dead-end trap.
    for cy in 0..ROWS {
        for cx in 0..COLS {
            if cell_valid((cx, cy)) && cell_valid((cx + 1, cy)) && dice.chance(0.18) {
                open_between(grid, (cx, cy), (cx + 1, cy));
            }
            if cell_valid((cx, cy)) && cell_valid((cx, cy + 1)) && dice.chance(0.18) {
                open_between(grid, (cx, cy), (cx, cy + 1));
            }
        }
    }
    // Open the clipped rim so partial cells never become sealed pockets.
    for y in 0..crate::world::grid::HEIGHT {
        for x in 0..crate::world::grid::WIDTH {
            if crate::world::grid::hex_metric(x, y) > 530 {
                grid.set_inner(x, y, Tile::Floor);
            }
        }
    }
}
