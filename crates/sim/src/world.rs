//! The world: map, train and the fixed-step simulation.

use crate::math::V2;
use crate::rng::Pcg32;
use crate::track::{Dir, TileMap};
use crate::train::{Train, Turn};

/// One simulation step: 60 steps per second.
pub const DT: f32 = 1.0 / 60.0;
pub const MAP_SIZE: i32 = 64;
pub const START_HP: f32 = 100.0;
pub const SELF_HIT_DAMAGE: f32 = 20.0;

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
    pub rng: Pcg32,
    pub time: f32,
    pub state: State,
    pub hp: f32,
    pub max_hp: f32,
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
            rng,
            time: 0.0,
            state: State::Running,
            hp: START_HP,
            max_hp: START_HP,
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
        if let Some(turn) = input.turn {
            self.train.latch(turn);
        }
        if input.reverse && !self.train.is_stopped() {
            self.train.reverse();
        }
        if self.train.step(&mut self.map, DT) {
            self.damage(SELF_HIT_DAMAGE);
        }
    }

    pub fn damage(&mut self, amount: f32) {
        self.hp -= amount;
        if self.hp <= 0.0 {
            self.hp = 0.0;
            self.state = State::Lost;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn self_hit_costs_hp_but_not_the_run() {
        let mut w = World::new(1);
        w.train.set_car_count(5);
        let input = Input {
            turn: Some(Turn::Right),
            reverse: false,
        };
        for _ in 0..600 {
            w.step(&input);
        }
        assert!(w.hp < START_HP);
        assert_eq!(w.state, State::Running);
    }
}
