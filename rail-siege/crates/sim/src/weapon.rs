//! Weapon wagons. Weapons are company technology: every train carries a
//! wagon of each weapon the company owns. Each weapon has 5 levels.

use crate::math::V2;
use crate::track::Tile;

pub const MAX_LEVEL: u8 = 5;
pub const ARMOURED_RANGE: f32 = 6.0;
pub const SHOT_SPEED: f32 = 14.0;
pub const SHOT_RADIUS: f32 = 0.1;
pub const STEAM_PUSH: f32 = 8.0;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum WagonKind {
    /// Shoots the nearest enemy.
    Armoured,
    /// Drops burning coal on each tile it passes.
    Hopper,
    /// A steam blast around the wagon.
    Steam,
}

impl WagonKind {
    pub const ALL: [WagonKind; 3] = [WagonKind::Armoured, WagonKind::Hopper, WagonKind::Steam];

    pub fn name(self) -> &'static str {
        match self {
            WagonKind::Armoured => "Armoured wagon",
            WagonKind::Hopper => "Coal hopper",
            WagonKind::Steam => "Steam wagon",
        }
    }

    /// What the wagon does at `level`, or what the level adds.
    pub fn describe(self, level: u8) -> &'static str {
        use WagonKind::*;
        match (self, level) {
            (Armoured, 1) => "Shoots the nearest enemy in 6 tiles. 10 dmg, 1 shot/s.",
            (Armoured, 2) => "+50 % damage.",
            (Armoured, 3) => "+50 % fire rate.",
            (Armoured, 4) => "+1 bullet.",
            (Armoured, _) => "Bullets pierce 3 enemies. More damage.",
            (Hopper, 1) => "Drops burning coal on the track it passes. 8 dmg/s for 4 s.",
            (Hopper, 2) => "Coal burns longer.",
            (Hopper, 3) => "+50 % burn damage.",
            (Hopper, 4) => "Coal also burns adjacent tiles.",
            (Hopper, _) => "Coal burns longer and hotter.",
            (Steam, 1) => "Steam blast in 2.5 tiles every 4 s. 15 dmg, push back.",
            (Steam, 2) => "Larger blast radius.",
            (Steam, 3) => "+66 % blast damage.",
            (Steam, 4) => "Blast every 3 s.",
            (Steam, _) => "Larger and stronger blast.",
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub struct Armoured {
    pub damage: f32,
    pub rate: f32,
    pub bullets: usize,
    pub pierce: u8,
}

pub fn armoured(level: u8) -> Armoured {
    let i = (level.clamp(1, MAX_LEVEL) - 1) as usize;
    Armoured {
        damage: [10.0, 15.0, 15.0, 15.0, 20.0][i],
        rate: [1.0, 1.0, 1.5, 1.5, 1.5][i],
        bullets: [1, 1, 1, 2, 2][i],
        pierce: [1, 1, 1, 1, 3][i],
    }
}

#[derive(Clone, Copy, Debug)]
pub struct Hopper {
    pub duration: f32,
    pub dps: f32,
    pub adjacent: bool,
}

pub fn hopper(level: u8) -> Hopper {
    let i = (level.clamp(1, MAX_LEVEL) - 1) as usize;
    Hopper {
        duration: [4.0, 6.0, 6.0, 6.0, 8.0][i],
        dps: [8.0, 8.0, 12.0, 12.0, 16.0][i],
        adjacent: i >= 3,
    }
}

#[derive(Clone, Copy, Debug)]
pub struct Steam {
    pub radius: f32,
    pub damage: f32,
    pub cooldown: f32,
}

pub fn steam(level: u8) -> Steam {
    let i = (level.clamp(1, MAX_LEVEL) - 1) as usize;
    Steam {
        radius: [2.5, 3.0, 3.0, 3.0, 3.5][i],
        damage: [15.0, 15.0, 25.0, 25.0, 35.0][i],
        cooldown: [4.0, 4.0, 4.0, 3.0, 3.0][i],
    }
}

/// The state of one weapon wagon on one train.
#[derive(Clone, Copy, Debug)]
pub struct Gun {
    pub cooldown: f32,
    /// Last aim direction, for drawing a turret.
    pub aim: V2,
    pub last_tile: Option<Tile>,
}

impl Default for Gun {
    fn default() -> Self {
        Self {
            cooldown: 0.0,
            aim: V2::new(1.0, 0.0),
            last_tile: None,
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub struct Guard {
    pub damage: f32,
    pub rate: f32,
    pub range: f32,
}

/// The turret of the Town guards card.
pub fn guard(level: u8) -> Guard {
    let i = (level.clamp(1, MAX_LEVEL) - 1) as usize;
    Guard {
        damage: [8.0, 12.0, 12.0, 16.0, 20.0][i],
        rate: [1.0, 1.0, 1.5, 1.5, 2.0][i],
        range: [5.0, 5.0, 6.0, 6.0, 7.0][i],
    }
}

#[derive(Clone, Debug)]
pub struct Shot {
    pub pos: V2,
    pub vel: V2,
    pub damage: f32,
    pub pierce: u8,
    pub life: f32,
    /// Enemies this shot already hit.
    pub hit: Vec<u32>,
}

/// A steam blast, kept for drawing.
#[derive(Clone, Copy, Debug)]
pub struct Blast {
    pub pos: V2,
    pub radius: f32,
    pub ttl: f32,
}

pub const BLAST_TTL: f32 = 0.35;

/// Burning coal: time left and damage per second, one entry per tile.
#[derive(Clone, Debug)]
pub struct Fires {
    w: i32,
    h: i32,
    ttl: Vec<f32>,
    dps: Vec<f32>,
    active: Vec<usize>,
}

impl Fires {
    pub fn new(w: i32, h: i32) -> Self {
        let n = (w * h) as usize;
        Self {
            w,
            h,
            ttl: vec![0.0; n],
            dps: vec![0.0; n],
            active: Vec::new(),
        }
    }

    fn idx(&self, t: Tile) -> Option<usize> {
        (t.0 >= 0 && t.1 >= 0 && t.0 < self.w && t.1 < self.h)
            .then(|| (t.1 * self.w + t.0) as usize)
    }

    /// Lights a tile, or makes its fire longer and hotter.
    pub fn light(&mut self, t: Tile, ttl: f32, dps: f32) {
        let Some(i) = self.idx(t) else { return };
        if self.ttl[i] <= 0.0 {
            self.active.push(i);
        }
        self.ttl[i] = self.ttl[i].max(ttl);
        self.dps[i] = self.dps[i].max(dps);
    }

    pub fn tick(&mut self, dt: f32) {
        let (ttl, dps) = (&mut self.ttl, &mut self.dps);
        self.active.retain(|&i| {
            ttl[i] -= dt;
            if ttl[i] <= 0.0 {
                ttl[i] = 0.0;
                dps[i] = 0.0;
                false
            } else {
                true
            }
        });
    }

    /// Damage per second at a point.
    pub fn dps_at(&self, p: V2) -> f32 {
        self.idx((p.x.floor() as i32, p.y.floor() as i32))
            .map_or(0.0, |i| self.dps[i])
    }

    /// Burning tiles with their time left.
    pub fn iter(&self) -> impl Iterator<Item = (Tile, f32)> + '_ {
        self.active
            .iter()
            .map(|&i| (((i as i32) % self.w, (i as i32) / self.w), self.ttl[i]))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn levels_never_get_worse() {
        for l in 1..MAX_LEVEL {
            let (a, b) = (armoured(l), armoured(l + 1));
            assert!(b.damage * b.rate * b.bullets as f32 >= a.damage * a.rate * a.bullets as f32);
            assert!(
                hopper(l + 1).dps * hopper(l + 1).duration >= hopper(l).dps * hopper(l).duration
            );
            assert!(
                steam(l + 1).damage / steam(l + 1).cooldown >= steam(l).damage / steam(l).cooldown
            );
        }
    }

    #[test]
    fn guard_levels_never_get_worse() {
        for l in 1..MAX_LEVEL {
            let (a, b) = (guard(l), guard(l + 1));
            assert!(b.damage * b.rate >= a.damage * a.rate && b.range >= a.range);
        }
    }

    #[test]
    fn fires_burn_out() {
        let mut f = Fires::new(8, 8);
        f.light((2, 3), 1.0, 8.0);
        f.light((2, 3), 0.5, 4.0);
        assert_eq!(f.dps_at(V2::new(2.5, 3.5)), 8.0);
        assert_eq!(f.iter().count(), 1);
        f.tick(0.6);
        assert_eq!(f.dps_at(V2::new(2.5, 3.5)), 8.0);
        f.tick(0.6);
        assert_eq!(f.dps_at(V2::new(2.5, 3.5)), 0.0);
        assert_eq!(f.iter().count(), 0);
        f.light((-1, 0), 1.0, 1.0);
        assert_eq!(f.iter().count(), 0, "outside the map");
    }
}
