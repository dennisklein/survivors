//! Minimal 2D vector math.

use std::ops::{Add, AddAssign, Mul, Sub};

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct V2 {
    pub x: f32,
    pub y: f32,
}

pub const fn v2(x: f32, y: f32) -> V2 {
    V2 { x, y }
}

impl V2 {
    pub fn len(self) -> f32 {
        self.x.hypot(self.y)
    }

    pub fn dist(self, o: V2) -> f32 {
        (self - o).len()
    }

    /// Unit vector, or zero for a zero-length input.
    pub fn norm(self) -> V2 {
        let l = self.len();
        if l > 1e-6 {
            self * (1.0 / l)
        } else {
            V2::default()
        }
    }
}

impl Add for V2 {
    type Output = V2;
    fn add(self, o: V2) -> V2 {
        v2(self.x + o.x, self.y + o.y)
    }
}

impl AddAssign for V2 {
    fn add_assign(&mut self, o: V2) {
        self.x += o.x;
        self.y += o.y;
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn norm_and_dist() {
        assert_eq!(v2(3.0, 4.0).len(), 5.0);
        assert!((v2(10.0, 0.0).norm().x - 1.0).abs() < 1e-6);
        assert_eq!(V2::default().norm(), V2::default());
        assert_eq!(v2(1.0, 1.0).dist(v2(4.0, 5.0)), 5.0);
    }
}
