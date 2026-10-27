//! Minimal 2D vector type and loop-track geometry.

use std::f32::consts::TAU;
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

/// Radius of the circular loop line in world units.
pub const LOOP_R: f32 = 900.0;
/// Length of the loop line.
pub const CIRC: f32 = TAU * LOOP_R;

/// Wraps an arc position into `[0, CIRC)`.
pub fn wrap(s: f32) -> f32 {
    s.rem_euclid(CIRC)
}

/// Shortest distance along the loop between two arc positions.
pub fn arc_dist(a: f32, b: f32) -> f32 {
    let d = wrap(a - b);
    d.min(CIRC - d)
}

/// World position of arc position `s`.
pub fn track_pos(s: f32) -> V2 {
    V2::from_angle(s / LOOP_R) * LOOP_R
}

/// Heading (radians) of a train travelling forward at arc position `s`.
pub fn track_heading(s: f32) -> f32 {
    s / LOOP_R + std::f32::consts::FRAC_PI_2
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn wrap_stays_in_range() {
        assert!((wrap(-1.0) - (CIRC - 1.0)).abs() < 1e-2);
        assert!(wrap(CIRC + 5.0) - 5.0 < 1e-2);
    }

    #[test]
    fn arc_dist_is_shortest_way_round() {
        assert!((arc_dist(10.0, CIRC - 10.0) - 20.0).abs() < 1e-2);
        assert!((arc_dist(100.0, 40.0) - 60.0).abs() < 1e-3);
    }

    #[test]
    fn track_is_on_circle_and_heading_is_tangent() {
        for s in [0.0, 500.0, 3000.0] {
            let p = track_pos(s);
            assert!((p.len() - LOOP_R).abs() < 1e-2);
            let ahead = track_pos(s + 1.0) - p;
            let h = V2::from_angle(track_heading(s));
            assert!((ahead.norm().x - h.x).abs() < 1e-2);
            assert!((ahead.norm().y - h.y).abs() < 1e-2);
        }
    }
}
