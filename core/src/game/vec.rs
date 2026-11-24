//! Minimal 2D vector in world pixels (y grows downwards).

use serde::{Deserialize, Serialize};
use std::ops::{Add, AddAssign, Mul, Sub, SubAssign};

#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct V2 {
    pub x: f32,
    pub y: f32,
}

pub const fn v2(x: f32, y: f32) -> V2 {
    V2 { x, y }
}

impl V2 {
    pub const ZERO: V2 = v2(0.0, 0.0);

    pub fn len(self) -> f32 {
        (self.x * self.x + self.y * self.y).sqrt()
    }

    pub fn dist(self, o: V2) -> f32 {
        (self - o).len()
    }

    /// Unit vector, or zero for a (near) zero vector.
    pub fn norm(self) -> V2 {
        let l = self.len();
        if l < 1e-5 { V2::ZERO } else { self * (1.0 / l) }
    }

    pub fn clamp_len(self, max: f32) -> V2 {
        if self.len() > max {
            self.norm() * max
        } else {
            self
        }
    }

    pub fn dot(self, o: V2) -> f32 {
        self.x * o.x + self.y * o.y
    }

    pub fn perp(self) -> V2 {
        v2(-self.y, self.x)
    }

    pub fn from_angle(a: f32) -> V2 {
        v2(a.cos(), a.sin())
    }

    pub fn angle(self) -> f32 {
        self.y.atan2(self.x)
    }

    pub fn rotate(self, a: f32) -> V2 {
        let (s, c) = a.sin_cos();
        v2(self.x * c - self.y * s, self.x * s + self.y * c)
    }

    pub fn approach(self, target: V2, step: f32) -> V2 {
        let d = target - self;
        if d.len() <= step {
            target
        } else {
            self + d.norm() * step
        }
    }
}

impl Add for V2 {
    type Output = V2;
    fn add(self, o: V2) -> V2 {
        v2(self.x + o.x, self.y + o.y)
    }
}

impl Sub for V2 {
    type Output = V2;
    fn sub(self, o: V2) -> V2 {
        v2(self.x - o.x, self.y - o.y)
    }
}

impl Mul<f32> for V2 {
    type Output = V2;
    fn mul(self, s: f32) -> V2 {
        v2(self.x * s, self.y * s)
    }
}

impl AddAssign for V2 {
    fn add_assign(&mut self, o: V2) {
        *self = *self + o;
    }
}

impl SubAssign for V2 {
    fn sub_assign(&mut self, o: V2) {
        *self = *self - o;
    }
}
