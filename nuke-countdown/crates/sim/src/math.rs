//! Minimal 2D vector maths.

use std::ops::{Add, AddAssign, Mul, Sub, SubAssign};

#[derive(Clone, Copy, Debug, Default, PartialEq)]
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
        self.x.hypot(self.y)
    }

    pub fn dist(self, o: V2) -> f32 {
        (self - o).len()
    }

    pub fn dist2(self, o: V2) -> f32 {
        let d = self - o;
        d.x * d.x + d.y * d.y
    }

    /// Unit vector, or zero for a (near) zero vector.
    pub fn norm(self) -> V2 {
        let l = self.len();
        if l < 1e-6 { V2::ZERO } else { self * (1.0 / l) }
    }

    pub fn dot(self, o: V2) -> f32 {
        self.x * o.x + self.y * o.y
    }

    pub fn from_angle(a: f32) -> V2 {
        v2(a.cos(), a.sin())
    }

    /// Distance from this point to the segment `a`-`b`.
    pub fn seg_dist(self, a: V2, b: V2) -> f32 {
        let ab = b - a;
        let l2 = ab.dot(ab);
        if l2 < 1e-6 {
            return self.dist(a);
        }
        let t = ((self - a).dot(ab) / l2).clamp(0.0, 1.0);
        self.dist(a + ab * t)
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
    fn mul(self, k: f32) -> V2 {
        v2(self.x * k, self.y * k)
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn norm_and_segment_distance() {
        assert!((v2(3.0, 4.0).norm().len() - 1.0).abs() < 1e-5);
        assert_eq!(V2::ZERO.norm(), V2::ZERO);
        let p = v2(5.0, 3.0);
        assert!((p.seg_dist(v2(0.0, 0.0), v2(10.0, 0.0)) - 3.0).abs() < 1e-5);
        assert!((p.seg_dist(v2(10.0, 0.0), v2(20.0, 0.0)) - 34f32.sqrt()).abs() < 1e-4);
    }
}
