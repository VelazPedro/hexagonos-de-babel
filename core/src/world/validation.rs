//! Flood-fill reachability guarantees.

use super::grid::{Grid, HEIGHT, Pos, WIDTH};

#[derive(Clone, Debug)]
pub struct Reach {
    seen: Vec<bool>,
}

impl Reach {
    pub fn contains(&self, p: Pos) -> bool {
        (0..WIDTH).contains(&p.x)
            && (0..HEIGHT).contains(&p.y)
            && self.seen[(p.y * WIDTH + p.x) as usize]
    }

    pub fn count(&self) -> usize {
        self.seen.iter().filter(|&&b| b).count()
    }
}

/// 4-connected flood fill over walkable tiles, treating `blocked` as solid.
pub fn flood(grid: &Grid, start: Pos, blocked: &[Pos]) -> Reach {
    let mut seen = vec![false; (WIDTH * HEIGHT) as usize];
    let mut wall = vec![false; (WIDTH * HEIGHT) as usize];
    for b in blocked {
        if (0..WIDTH).contains(&b.x) && (0..HEIGHT).contains(&b.y) {
            wall[(b.y * WIDTH + b.x) as usize] = true;
        }
    }
    let mut stack = vec![start];
    while let Some(p) = stack.pop() {
        if !grid.walkable(p.x, p.y) {
            continue;
        }
        let i = (p.y * WIDTH + p.x) as usize;
        if seen[i] || wall[i] {
            continue;
        }
        seen[i] = true;
        stack.push(Pos::new(p.x + 1, p.y));
        stack.push(Pos::new(p.x - 1, p.y));
        stack.push(Pos::new(p.x, p.y + 1));
        stack.push(Pos::new(p.x, p.y - 1));
    }
    Reach { seen }
}

/// True when every target is reachable from `start`.
pub fn all_reachable(grid: &Grid, start: Pos, targets: &[Pos]) -> bool {
    let reach = flood(grid, start, &[]);
    targets.iter().all(|&t| reach.contains(t))
}

/// Walkable tile closest to `preferred` (deterministic scan order on ties).
pub fn nearest_walkable(grid: &Grid, preferred: Pos) -> Pos {
    grid.positions()
        .filter(|&p| grid.walkable(p.x, p.y))
        .min_by_key(|&p| p.dist2(preferred))
        .unwrap_or(preferred)
}
