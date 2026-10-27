//! The spawn director keeps the number of live enemies near a cap that grows
//! with time. It spawns groups at the map edge.

use crate::enemy::EnemyKind;
use crate::math::V2;
use crate::rng::Pcg32;

pub const SPAWN_INTERVAL: f32 = 0.4;
pub const SABOTEUR_FROM: f32 = 180.0;
pub const BUS_FROM: f32 = 300.0;
pub const MAX_LIVE: usize = 400;
/// Enemy HP grows by 8 % per minute.
pub const HP_PER_MINUTE: f32 = 0.08;

/// Live enemy cap after `time` seconds.
pub fn cap(time: f32) -> usize {
    ((20.0 + 28.0 * time / 60.0) as usize).min(MAX_LIVE)
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
        size: V2,
        rng: &mut Pcg32,
    ) -> Option<Group> {
        self.timer -= dt;
        if self.timer > 0.0 || live >= cap(time) {
            return None;
        }
        self.timer = SPAWN_INTERVAL;
        let at = edge_point(size, rng);
        let saboteur = if time < SABOTEUR_FROM { 0.0 } else { 0.12 };
        let bus = if time < BUS_FROM {
            0.0
        } else {
            (0.15 + 0.03 * (time - BUS_FROM) / 60.0).min(0.35)
        };
        let roll = rng.f32();
        Some(if roll < saboteur {
            Group {
                kind: EnemyKind::Saboteur,
                at,
                count: 1 + rng.below(2),
            }
        } else if roll < saboteur + bus {
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

/// A random point just inside the edge of a map of `size` tiles.
pub fn edge_point(size: V2, rng: &mut Pcg32) -> V2 {
    let m = 0.4;
    let along = rng.f32();
    match rng.below(4) {
        0 => V2::new(m + along * (size.x - 2.0 * m), m),
        1 => V2::new(m + along * (size.x - 2.0 * m), size.y - m),
        2 => V2::new(m, m + along * (size.y - 2.0 * m)),
        _ => V2::new(size.x - m, m + along * (size.y - 2.0 * m)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cap_follows_the_curve() {
        assert_eq!(cap(0.0), 20);
        assert_eq!(cap(180.0), 104);
        assert_eq!(cap(300.0), 160);
        assert_eq!(cap(600.0), 300);
        assert_eq!(cap(900.0), MAX_LIVE);
    }

    #[test]
    fn spawns_at_the_edge() {
        let mut rng = Pcg32::new(9);
        let size = V2::new(48.0, 32.0);
        for _ in 0..200 {
            let p = edge_point(size, &mut rng);
            let edge = p.x.min(p.y).min(size.x - p.x).min(size.y - p.y);
            assert!(edge < 0.5 && edge > 0.0, "{p:?}");
        }
    }

    #[test]
    fn kinds_join_on_time() {
        let mut d = Director::default();
        let mut rng = Pcg32::new(1);
        let size = V2::new(48.0, 32.0);
        let mut kinds = |time: f32| {
            let mut seen = Vec::new();
            for _ in 0..3000 {
                if let Some(g) = d.tick(1.0, time, 0, size, &mut rng)
                    && !seen.contains(&g.kind)
                {
                    seen.push(g.kind);
                }
            }
            seen
        };
        assert_eq!(kinds(100.0), vec![EnemyKind::Resident]);
        assert!(!kinds(200.0).contains(&EnemyKind::Bus));
        assert!(kinds(200.0).contains(&EnemyKind::Saboteur));
        assert!(kinds(400.0).contains(&EnemyKind::Bus));
    }

    #[test]
    fn respects_the_cap() {
        let mut d = Director::default();
        let mut rng = Pcg32::new(1);
        let size = V2::new(48.0, 32.0);
        assert!(d.tick(1.0, 0.0, cap(0.0), size, &mut rng).is_none());
    }
}
