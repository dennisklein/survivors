//! Minimal 2D vector math.

use std::ops::{Add, AddAssign, Mul, Sub};

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct V2 {
    pub x: f32,
    pub y: f32,
}

impl V2 {
    pub const fn new(x: f32, y: f32) -> Self {
        Self { x, y }
    }

    pub fn len(self) -> f32 {
        self.x.hypot(self.y)
    }

    pub fn dist(self, o: V2) -> f32 {
        (self - o).len()
    }

    pub fn norm(self) -> V2 {
        let l = self.len();
        if l > 1e-6 {
            self * (1.0 / l)
        } else {
            V2::new(1.0, 0.0)
        }
    }

    pub fn dot(self, o: V2) -> f32 {
        self.x * o.x + self.y * o.y
    }

    /// Z component of the 3D cross product. With screen coordinates
    /// (y pointing down) a positive value means `o` lies clockwise of `self`.
    pub fn cross(self, o: V2) -> f32 {
        self.x * o.y - self.y * o.x
    }

    pub fn lerp(self, o: V2, t: f32) -> V2 {
        self + (o - self) * t
    }

    pub fn from_angle(a: f32) -> V2 {
        V2::new(a.cos(), a.sin())
    }

    pub fn rotate(self, a: f32) -> V2 {
        let (s, c) = a.sin_cos();
        V2::new(self.x * c - self.y * s, self.x * s + self.y * c)
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cross_sign_is_clockwise_on_screen() {
        let east = V2::new(1.0, 0.0);
        let south = V2::new(0.0, 1.0);
        assert!(east.cross(south) > 0.0);
        assert!(east.cross(V2::new(0.0, -1.0)) < 0.0);
    }

    #[test]
    fn norm_of_zero_is_safe() {
        assert_eq!(V2::default().norm(), V2::new(1.0, 0.0));
        assert!((V2::new(3.0, 4.0).len() - 5.0).abs() < 1e-6);
    }
}
