//! The spawn director keeps the number of live enemies near a cap that grows
//! with time. It spawns groups in a ring just outside the screen.

use std::f32::consts::TAU;

use crate::enemy::EnemyKind;
use crate::math::V2;
use crate::rng::Pcg32;

pub const RING_MIN: f32 = 12.0;
pub const RING_MAX: f32 = 16.0;
pub const SPAWN_INTERVAL: f32 = 0.4;
pub const BUS_FROM: f32 = 120.0;
/// Enemy HP grows by 8 % per minute.
pub const HP_PER_MINUTE: f32 = 0.08;

/// Live enemy cap after `time` seconds.
pub fn cap(time: f32) -> usize {
    (15.0 + 40.0 * time / 60.0).min(420.0) as usize
}

pub fn hp_scale(time: f32) -> f32 {
    1.0 + HP_PER_MINUTE * time / 60.0
}

#[derive(Clone, Debug, Default)]
pub struct Director {
    timer: f32,
}

/// One group to spawn.
pub struct Group {
    pub kind: EnemyKind,
    pub at: V2,
    pub count: u32,
}

impl Director {
    /// Returns a group to spawn this step, if any.
    pub fn tick(
        &mut self,
        dt: f32,
        time: f32,
        live: usize,
        centre: V2,
        size: f32,
        rng: &mut Pcg32,
    ) -> Option<Group> {
        self.timer -= dt;
        if self.timer > 0.0 || live >= cap(time) {
            return None;
        }
        self.timer = SPAWN_INTERVAL;
        let at = ring_point(centre, size, rng);
        let bus_chance = if time < BUS_FROM {
            0.0
        } else {
            (0.15 + 0.03 * (time - BUS_FROM) / 60.0).min(0.4)
        };
        Some(if rng.chance(bus_chance) {
            Group {
                kind: EnemyKind::Bus,
                at,
                count: 1 + rng.below(2),
            }
        } else {
            Group {
                kind: EnemyKind::Resident,
                at,
                count: 4 + rng.below(5),
            }
        })
    }
}

/// A random point 12 to 16 tiles from `centre`, inside the map if possible.
pub fn ring_point(centre: V2, size: f32, rng: &mut Pcg32) -> V2 {
    let inside = |p: V2| p.x > 1.0 && p.y > 1.0 && p.x < size - 1.0 && p.y < size - 1.0;
    let mut p = centre;
    for _ in 0..8 {
        p = centre + V2::from_angle(rng.range(0.0, TAU)) * rng.range(RING_MIN, RING_MAX);
        if inside(p) {
            return p;
        }
    }
    V2::new(p.x.clamp(1.0, size - 1.0), p.y.clamp(1.0, size - 1.0))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cap_grows_with_time() {
        assert!(cap(0.0) < cap(300.0));
        assert!(cap(3600.0) <= 420);
    }

    #[test]
    fn spawns_in_the_ring() {
        let mut rng = Pcg32::new(9);
        let c = V2::new(32.0, 32.0);
        for _ in 0..100 {
            let d = ring_point(c, 64.0, &mut rng).dist(c);
            assert!((RING_MIN..=RING_MAX).contains(&d));
        }
    }

    #[test]
    fn no_buses_before_two_minutes() {
        let mut d = Director::default();
        let mut rng = Pcg32::new(1);
        for i in 0..2000 {
            if let Some(g) = d.tick(0.05, 100.0, 0, V2::new(32.0, 32.0), 64.0, &mut rng) {
                assert_eq!(g.kind, EnemyKind::Resident, "step {i}");
            }
        }
    }

    #[test]
    fn respects_the_cap() {
        let mut d = Director::default();
        let mut rng = Pcg32::new(1);
        assert!(
            d.tick(1.0, 0.0, cap(0.0), V2::ZERO, 64.0, &mut rng)
                .is_none()
        );
    }
}
