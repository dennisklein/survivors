//! The world: map, train, enemies and the fixed-step simulation.

use crate::director::{self, Director};
use crate::enemy::{self, Enemy};
use crate::grid::Grid;
use crate::math::V2;
use crate::rng::Pcg32;
use crate::track::{Dir, TileMap};
use crate::train::{Train, Turn};

/// One simulation step: 60 steps per second.
pub const DT: f32 = 1.0 / 60.0;
pub const MAP_SIZE: i32 = 64;
pub const START_HP: f32 = 100.0;
pub const SELF_HIT_DAMAGE: f32 = 20.0;
pub const CAR_RADIUS: f32 = 0.4;
/// Ram damage is this factor times the train speed.
pub const RAM_FACTOR: f32 = 10.0;
pub const RAM_COOLDOWN: f32 = 0.5;
/// After touch damage, the train cannot take touch damage for this time.
pub const TOUCH_IFRAMES: f32 = 1.0;

/// Player input for one step.
#[derive(Clone, Copy, Debug, Default)]
pub struct Input {
    /// Turn key held now.
    pub turn: Option<Turn>,
    /// Reverse key pressed since the last step.
    pub reverse: bool,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum State {
    Running,
    Lost,
}

pub struct World {
    pub map: TileMap,
    pub train: Train,
    pub enemies: Vec<Enemy>,
    pub rng: Pcg32,
    pub time: f32,
    pub state: State,
    pub hp: f32,
    pub max_hp: f32,
    pub kills: u32,
    iframes: f32,
    director: Director,
    grid: Grid,
    next_id: u32,
}

impl World {
    pub fn new(seed: u64) -> Self {
        let mut rng = Pcg32::new(seed);
        let mut map = TileMap::generate(MAP_SIZE, MAP_SIZE, 9.0, &mut rng);
        let c = MAP_SIZE / 2;
        let train = Train::new(&mut map, (c, c), Dir::E, 3);
        Self {
            map,
            train,
            enemies: Vec::new(),
            rng,
            time: 0.0,
            state: State::Running,
            hp: START_HP,
            max_hp: START_HP,
            kills: 0,
            iframes: 0.0,
            director: Director::default(),
            grid: Grid::new(MAP_SIZE, MAP_SIZE),
            next_id: 0,
        }
    }

    pub fn head_pos(&self) -> V2 {
        self.train.head().pos
    }

    pub fn step(&mut self, input: &Input) {
        if self.state != State::Running {
            return;
        }
        self.time += DT;
        // 1. Input.
        if let Some(turn) = input.turn {
            self.train.latch(turn);
        }
        if input.reverse && !self.train.is_stopped() {
            self.train.reverse();
        }
        // 2. Train.
        if self.train.step(&mut self.map, DT) {
            self.damage(SELF_HIT_DAMAGE);
        }
        // 3. Spawns.
        self.spawn();
        // 4. Enemies.
        self.move_enemies();
        // 6. Hits.
        self.ram_and_touch();
        self.remove_dead();
    }

    pub fn damage(&mut self, amount: f32) {
        self.hp -= amount;
        if self.hp <= 0.0 {
            self.hp = 0.0;
            self.state = State::Lost;
        }
    }

    pub fn spawn_enemy(&mut self, kind: enemy::EnemyKind, at: V2) {
        let e = Enemy::new(self.next_id, kind, at, director::hp_scale(self.time));
        self.next_id += 1;
        self.enemies.push(e);
    }

    fn spawn(&mut self) {
        let size = self.map.width() as f32;
        let centre = self.head_pos();
        let live = self.enemies.len();
        let Some(g) = self
            .director
            .tick(DT, self.time, live, centre, size, &mut self.rng)
        else {
            return;
        };
        for _ in 0..g.count {
            let jitter = V2::new(self.rng.range(-1.2, 1.2), self.rng.range(-1.2, 1.2));
            self.spawn_enemy(g.kind, g.at + jitter);
        }
    }

    fn move_enemies(&mut self) {
        let cars: Vec<V2> = self.train.cars().iter().map(|c| c.pos).collect();
        let size = self.map.width() as f32;
        for e in &mut self.enemies {
            let target = cars
                .iter()
                .copied()
                .min_by(|a, b| a.dist_sq(e.pos).total_cmp(&b.dist_sq(e.pos)))
                .unwrap_or(e.pos);
            e.think(target, DT);
            e.pos = V2::new(e.pos.x.clamp(0.0, size), e.pos.y.clamp(0.0, size));
        }
        self.rebuild_grid();
        enemy::separate(&mut self.enemies, &self.grid);
    }

    fn rebuild_grid(&mut self) {
        let pts: Vec<V2> = self.enemies.iter().map(|e| e.pos).collect();
        self.grid.build(&pts);
    }

    /// The head rams enemies. Enemies that touch any car hurt the train.
    fn ram_and_touch(&mut self) {
        let speed = self.train.speed();
        // The strongest touching enemy hurts the train, then a short grace time.
        let mut hurt: f32 = 0.0;
        self.iframes -= DT;
        for (ci, car) in self.train.cars().iter().enumerate() {
            let enemies = &mut self.enemies;
            self.grid.query(car.pos, CAR_RADIUS + 1.5, |i| {
                let e = &mut enemies[i];
                let reach = CAR_RADIUS + e.radius();
                let d = e.pos - car.pos;
                if d.len_sq() >= reach * reach {
                    return;
                }
                let away = d.norm();
                if ci == 0 && speed > 0.5 && e.ram_cd <= 0.0 {
                    e.hit(RAM_FACTOR * speed);
                    e.ram_cd = RAM_COOLDOWN;
                    e.push = away * 6.0;
                }
                if e.hp > 0.0 {
                    hurt = hurt.max(e.stats().touch);
                }
                // Enemies cannot stand on the train.
                e.pos = car.pos + away * reach;
            });
        }
        if hurt > 0.0 && self.iframes <= 0.0 {
            self.damage(hurt);
            self.iframes = TOUCH_IFRAMES;
        }
    }

    fn remove_dead(&mut self) {
        let mut i = 0;
        while i < self.enemies.len() {
            if self.enemies[i].hp <= 0.0 {
                self.enemies.swap_remove(i);
                self.kills += 1;
            } else {
                i += 1;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::enemy::EnemyKind;

    fn run(w: &mut World, steps: usize, turn: impl Fn(usize) -> Option<Turn>) {
        for i in 0..steps {
            w.step(&Input {
                turn: turn(i),
                reverse: i % 700 == 699,
            });
        }
    }

    #[test]
    fn self_hit_costs_hp_but_not_the_run() {
        let mut w = World::new(1);
        w.train.set_car_count(5);
        for _ in 0..600 {
            w.step(&Input {
                turn: Some(Turn::Right),
                reverse: false,
            });
            w.enemies.clear();
        }
        assert!(w.hp < START_HP);
        assert_eq!(w.state, State::Running);
    }

    #[test]
    fn same_seed_gives_the_same_run() {
        let turns = |i: usize| match i % 240 {
            0..40 => Some(Turn::Left),
            120..130 => Some(Turn::Right),
            _ => None,
        };
        let mut a = World::new(77);
        let mut b = World::new(77);
        run(&mut a, 3000, turns);
        run(&mut b, 3000, turns);
        assert!(!a.enemies.is_empty());
        assert_eq!(a.hp, b.hp);
        assert_eq!(a.kills, b.kills);
        assert_eq!(a.head_pos(), b.head_pos());
        let pa: Vec<V2> = a.enemies.iter().map(|e| e.pos).collect();
        let pb: Vec<V2> = b.enemies.iter().map(|e| e.pos).collect();
        assert_eq!(pa, pb);
    }

    #[test]
    fn ram_kills_a_resident() {
        let mut w = World::new(5);
        let ahead = w.head_pos() + V2::new(1.5, 0.0);
        w.spawn_enemy(EnemyKind::Resident, ahead);
        for _ in 0..30 {
            w.step(&Input::default());
        }
        assert_eq!(w.kills, 1);
    }

    #[test]
    fn touch_hurts_the_train() {
        let mut w = World::new(5);
        let tail = w.train.cars().last().unwrap().pos;
        w.spawn_enemy(EnemyKind::Bus, tail + V2::new(0.0, 0.5));
        w.step(&Input::default());
        assert_eq!(w.hp, START_HP - 15.0);
    }
}
