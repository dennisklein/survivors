//! Minimal 2D vector maths.

use std::ops::{Add, AddAssign, Mul, Sub, SubAssign};

/// A 2D vector in world units.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct V2 {
    pub x: f32,
    pub y: f32,
}

impl V2 {
    pub const ZERO: V2 = V2 { x: 0.0, y: 0.0 };

    pub const fn new(x: f32, y: f32) -> Self {
        Self { x, y }
    }

    /// Unit vector pointing at `angle` radians.
    pub fn from_angle(angle: f32) -> Self {
        Self::new(angle.cos(), angle.sin())
    }

    pub fn len(self) -> f32 {
        self.x.hypot(self.y)
    }

    pub fn dist(self, other: V2) -> f32 {
        (self - other).len()
    }

    /// Unit vector in the same direction, or zero for a zero vector.
    pub fn norm(self) -> Self {
        let l = self.len();
        if l > 1e-6 { self * (1.0 / l) } else { V2::ZERO }
    }

    pub fn lerp(self, other: V2, t: f32) -> Self {
        self + (other - self) * t
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

impl Sub for V2 {
    type Output = V2;
    fn sub(self, o: V2) -> V2 {
        V2::new(self.x - o.x, self.y - o.y)
    }
}

impl Mul<f32> for V2 {
    type Output = V2;
    fn mul(self, s: f32) -> V2 {
        V2::new(self.x * s, self.y * s)
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
    fn norm_has_unit_length() {
        assert!((V2::new(3.0, 4.0).norm().len() - 1.0).abs() < 1e-5);
        assert_eq!(V2::ZERO.norm(), V2::ZERO);
    }

    #[test]
    fn lerp_endpoints() {
        let a = V2::new(1.0, 2.0);
        let b = V2::new(5.0, -2.0);
        assert_eq!(a.lerp(b, 0.0), a);
        assert_eq!(a.lerp(b, 1.0), b);
        assert_eq!(a.lerp(b, 0.5), V2::new(3.0, 0.0));
    }
}
