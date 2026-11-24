//! Flat-top hexagons in axial coordinates.

use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Dir {
    N,
    NE,
    SE,
    S,
    SW,
    NW,
}

impl Dir {
    pub const ALL: [Dir; 6] = [Dir::N, Dir::NE, Dir::SE, Dir::S, Dir::SW, Dir::NW];

    pub fn index(self) -> usize {
        self as usize
    }

    pub fn opposite(self) -> Dir {
        Dir::ALL[(self.index() + 3) % 6]
    }

    pub fn offset(self) -> (i32, i32) {
        match self {
            Dir::N => (0, -1),
            Dir::NE => (1, -1),
            Dir::SE => (1, 0),
            Dir::S => (0, 1),
            Dir::SW => (-1, 1),
            Dir::NW => (-1, 0),
        }
    }

    pub fn name(self) -> &'static str {
        match self {
            Dir::N => "norte",
            Dir::NE => "noreste",
            Dir::SE => "sureste",
            Dir::S => "sur",
            Dir::SW => "suroeste",
            Dir::NW => "noroeste",
        }
    }
}

pub fn neighbor(q: i32, r: i32, d: Dir) -> (i32, i32) {
    let (dq, dr) = d.offset();
    (q.wrapping_add(dq), r.wrapping_add(dr))
}

pub fn distance(a: (i32, i32), b: (i32, i32)) -> u32 {
    let dq = i64::from(a.0) - i64::from(b.0);
    let dr = i64::from(a.1) - i64::from(b.1);
    ((dq.abs() + dr.abs() + (dq + dr).abs()) / 2) as u32
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn neighbors_are_distance_one_and_reversible() {
        for d in Dir::ALL {
            let n = neighbor(3, -2, d);
            assert_eq!(distance((3, -2), n), 1);
            assert_eq!(neighbor(n.0, n.1, d.opposite()), (3, -2));
        }
    }
}
