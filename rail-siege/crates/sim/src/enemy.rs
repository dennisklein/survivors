//! Enemies: they move freely over grass and water and do not need track.
//! Each kind has its own target, so the horde attacks the whole company.

use crate::grid::Grid;
use crate::math::V2;
use crate::track::Tile;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EnemyKind {
    /// Walks to the town. Hits trains on the way. Comes in large groups.
    Resident,
    /// Walks to the nearest track and breaks it.
    Saboteur,
    /// Stops, aims at a train for 1 s, then charges in a straight line.
    Bus,
}

#[derive(Clone, Copy, Debug)]
pub struct Stats {
    pub hp: f32,
    pub speed: f32,
    pub radius: f32,
    /// Damage to a train it touches.
    pub touch: f32,
    /// Damage to the town when it reaches the town.
    pub town: f32,
}

impl EnemyKind {
    pub const fn stats(self) -> Stats {
        match self {
            EnemyKind::Resident => Stats {
                hp: 10.0,
                speed: 1.0,
                radius: 0.28,
                touch: 5.0,
                town: 1.0,
            },
            EnemyKind::Saboteur => Stats {
                hp: 20.0,
                speed: 1.4,
                radius: 0.3,
                touch: 5.0,
                town: 1.0,
            },
            EnemyKind::Bus => Stats {
                hp: 30.0,
                speed: 3.0,
                radius: 0.38,
                touch: 15.0,
                town: 3.0,
            },
        }
    }

    /// Flying enemies ignore knockback and other enemies.
    pub fn flies(self) -> bool {
        false
    }
}

pub const BUS_AIM_RANGE: f32 = 7.0;
pub const BUS_AIM_TIME: f32 = 1.0;
pub const BUS_CHARGE_TIME: f32 = 1.4;
pub const BUS_CHARGE_SPEED: f32 = 7.5;
pub const BUS_REST_TIME: f32 = 1.2;
/// Knockback speed loss per second (exponential).
const PUSH_DECAY: f32 = 6.0;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Phase {
    Seek,
    Aim,
    Charge,
    Rest,
}

#[derive(Clone, Debug)]
pub struct Enemy {
    pub id: u32,
    pub kind: EnemyKind,
    pub pos: V2,
    pub hp: f32,
    pub max_hp: f32,
    /// Facing, or the charge line of a bus.
    pub dir: V2,
    pub phase: Phase,
    pub timer: f32,
    /// Knockback velocity.
    pub push: V2,
    pub ram_cd: f32,
    /// Time left of the white hit flash.
    pub flash: f32,
    /// Saboteurs: the track tile they go to, and the time spent on it.
    pub goal: Option<Tile>,
    pub work: f32,
}

impl Enemy {
    pub fn new(id: u32, kind: EnemyKind, pos: V2, hp_scale: f32) -> Self {
        let hp = kind.stats().hp * hp_scale;
        Self {
            id,
            kind,
            pos,
            hp,
            max_hp: hp,
            dir: V2::new(1.0, 0.0),
            phase: Phase::Seek,
            timer: 0.0,
            push: V2::ZERO,
            ram_cd: 0.0,
            flash: 0.0,
            goal: None,
            work: 0.0,
        }
    }

    pub fn stats(&self) -> Stats {
        self.kind.stats()
    }

    pub fn radius(&self) -> f32 {
        self.stats().radius
    }

    pub fn hit(&mut self, dmg: f32) {
        self.hp -= dmg;
        self.flash = 0.1;
    }

    /// Moves one step toward `target`.
    pub fn think(&mut self, target: V2, dt: f32) {
        self.ram_cd -= dt;
        self.flash -= dt;
        self.timer -= dt;
        let to = target - self.pos;
        let speed = self.stats().speed;
        let vel = match self.kind {
            EnemyKind::Resident => {
                self.dir = to.norm();
                self.dir * speed
            }
            EnemyKind::Saboteur => {
                // Slow down on the target tile, so it stays there.
                self.dir = to.norm();
                self.dir * speed.min(to.len() * 4.0)
            }
            EnemyKind::Bus => self.bus(to, speed),
        };
        if self.kind.flies() {
            self.push = V2::ZERO;
        }
        self.pos += (vel + self.push) * dt;
        self.push = self.push * (-PUSH_DECAY * dt).exp();
    }

    fn bus(&mut self, to: V2, speed: f32) -> V2 {
        match self.phase {
            Phase::Seek => {
                self.dir = to.norm();
                if to.len() < BUS_AIM_RANGE {
                    self.phase = Phase::Aim;
                    self.timer = BUS_AIM_TIME;
                }
                self.dir * speed
            }
            Phase::Aim => {
                if self.timer <= 0.0 {
                    self.phase = Phase::Charge;
                    self.timer = BUS_CHARGE_TIME;
                }
                V2::ZERO
            }
            Phase::Charge => {
                if self.timer <= 0.0 {
                    self.phase = Phase::Rest;
                    self.timer = BUS_REST_TIME;
                }
                self.dir * BUS_CHARGE_SPEED
            }
            Phase::Rest => {
                if self.timer <= 0.0 {
                    self.phase = Phase::Seek;
                }
                V2::ZERO
            }
        }
    }
}

/// Pushes overlapping enemies apart.
pub fn separate(enemies: &mut [Enemy], grid: &Grid) {
    for i in 0..enemies.len() {
        if enemies[i].kind.flies() {
            continue;
        }
        let (p, r) = (enemies[i].pos, enemies[i].radius());
        let mut shift = V2::ZERO;
        grid.query(p, 2.0 * r, |j| {
            if j == i {
                return;
            }
            let o = &enemies[j];
            if o.kind.flies() {
                return;
            }
            let d = p - o.pos;
            let min = r + o.radius();
            let dist_sq = d.len_sq();
            if dist_sq < min * min {
                let dist = dist_sq.sqrt();
                let away = if dist > 1e-4 {
                    d * (1.0 / dist)
                } else {
                    V2::new(1.0, 0.0)
                };
                shift += away * ((min - dist) * 0.5);
            }
        });
        enemies[i].pos += shift;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resident_walks_to_target() {
        let mut e = Enemy::new(0, EnemyKind::Resident, V2::new(0.0, 0.0), 1.0);
        for _ in 0..60 {
            e.think(V2::new(10.0, 0.0), 1.0 / 60.0);
        }
        assert!((e.pos.x - 1.0).abs() < 1e-3);
    }

    #[test]
    fn bus_aims_then_charges() {
        let mut e = Enemy::new(0, EnemyKind::Bus, V2::new(0.0, 0.0), 1.0);
        let target = V2::new(5.0, 0.0);
        e.think(target, 1.0 / 60.0);
        assert_eq!(e.phase, Phase::Aim);
        let at = e.pos;
        for _ in 0..61 {
            e.think(target, 1.0 / 60.0);
        }
        assert_eq!(e.phase, Phase::Charge);
        assert!(e.pos.dist(at) < 1e-3, "stands still while aiming");
        // The charge line does not follow the target.
        let dir = e.dir;
        e.think(V2::new(0.0, 5.0), 1.0 / 60.0);
        assert_eq!(e.dir, dir);
    }

    #[test]
    fn saboteur_stops_on_its_tile() {
        let mut e = Enemy::new(0, EnemyKind::Saboteur, V2::new(0.0, 0.0), 1.0);
        for _ in 0..600 {
            e.think(V2::new(3.0, 0.0), 1.0 / 60.0);
        }
        assert!(e.pos.dist(V2::new(3.0, 0.0)) < 0.05);
    }

    #[test]
    fn hp_scales() {
        let e = Enemy::new(0, EnemyKind::Bus, V2::ZERO, 1.5);
        assert_eq!(e.max_hp, 45.0);
    }

    #[test]
    fn separation_pushes_apart() {
        let mut es = vec![
            Enemy::new(0, EnemyKind::Resident, V2::new(5.0, 5.0), 1.0),
            Enemy::new(1, EnemyKind::Resident, V2::new(5.1, 5.0), 1.0),
        ];
        let mut g = Grid::new(16, 16);
        g.build(&es.iter().map(|e| e.pos).collect::<Vec<_>>());
        separate(&mut es, &g);
        assert!(es[0].pos.x < 5.0 && es[1].pos.x > 5.1);
    }
}
