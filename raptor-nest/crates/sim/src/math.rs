//! Minimal 2D vector type and geometry helpers.

use std::ops::{Add, AddAssign, Mul, Sub};

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct V2 {
    pub x: f32,
    pub y: f32,
}

impl V2 {
    pub const ZERO: V2 = V2 { x: 0.0, y: 0.0 };

    pub const fn new(x: f32, y: f32) -> Self {
        V2 { x, y }
    }

    pub fn from_angle(a: f32) -> Self {
        V2::new(a.cos(), a.sin())
    }

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

    pub fn norm(self) -> V2 {
        let l = self.len();
        if l > 1e-6 { self * (1.0 / l) } else { V2::ZERO }
    }

    pub fn angle(self) -> f32 {
        self.y.atan2(self.x)
    }
}

impl Add for V2 {
    type Output = V2;
    fn add(self, o: V2) -> V2 {
        V2::new(self.x + o.x, self.y + o.y)
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
        V2::new(self.x - o.x, self.y - o.y)
    }
}

impl Mul<f32> for V2 {
    type Output = V2;
    fn mul(self, k: f32) -> V2 {
        V2::new(self.x * k, self.y * k)
    }
}

impl V2 {
    pub fn dot(self, o: V2) -> f32 {
        self.x * o.x + self.y * o.y
    }
}

/// Distance from `p` to the segment `a..b`.
pub fn seg_dist(p: V2, a: V2, b: V2) -> f32 {
    let ab = b - a;
    let l2 = ab.dot(ab);
    if l2 < 1e-6 {
        return p.dist(a);
    }
    let t = ((p - a).dot(ab) / l2).clamp(0.0, 1.0);
    p.dist(a + ab * t)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn seg_dist_measures_perpendicular_and_endpoints() {
        let a = V2::new(0.0, 0.0);
        let b = V2::new(10.0, 0.0);
        assert!((seg_dist(V2::new(5.0, 3.0), a, b) - 3.0).abs() < 1e-5);
        assert!((seg_dist(V2::new(-4.0, 0.0), a, b) - 4.0).abs() < 1e-5);
    }

    #[test]
    fn norm_is_unit_or_zero() {
        assert!((V2::new(3.0, 4.0).norm().len() - 1.0).abs() < 1e-5);
        assert_eq!(V2::ZERO.norm(), V2::ZERO);
    }
}
