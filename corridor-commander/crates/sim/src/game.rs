//! Pure game rules for Corridor Commander.
//!
//! World space: `x` runs across the corridor (`0..WIDTH`), `y` runs forward.
//! The camera window covers `scroll..scroll + VIEW_H` and advances on its
//! own; the commander is kept inside it. Towers stay where they were built
//! and are lost once they scroll off the bottom edge.

use crate::rng::Rng;

pub const WIDTH: f32 = 480.0;
pub const VIEW_H: f32 = 640.0;
pub const SCROLL_SPEED: f32 = 34.0;
pub const GATE_SPACING: f32 = 2200.0;
pub const GATES: u32 = 3;
pub const PLAYER_R: f32 = 14.0;
pub const BUILD_RANGE: f32 = 240.0;
pub const TOWER_R: f32 = 16.0;
pub const DASH_TIME: f32 = 0.18;
pub const DASH_SPEED: f32 = 720.0;

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct V2 {
    pub x: f32,
    pub y: f32,
}

impl V2 {
    pub const fn new(x: f32, y: f32) -> Self {
        V2 { x, y }
    }
    pub fn len(self) -> f32 {
        (self.x * self.x + self.y * self.y).sqrt()
    }
    pub fn dist(self, o: V2) -> f32 {
        (self - o).len()
    }
    pub fn norm(self) -> V2 {
        let l = self.len();
        if l < 1e-6 {
            V2::default()
        } else {
            self * (1.0 / l)
        }
    }
}

impl std::ops::Add for V2 {
    type Output = V2;
    fn add(self, o: V2) -> V2 {
        V2::new(self.x + o.x, self.y + o.y)
    }
}
impl std::ops::Sub for V2 {
    type Output = V2;
    fn sub(self, o: V2) -> V2 {
        V2::new(self.x - o.x, self.y - o.y)
    }
}
impl std::ops::Mul<f32> for V2 {
    type Output = V2;
    fn mul(self, k: f32) -> V2 {
        V2::new(self.x * k, self.y * k)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EnemyKind {
    /// Fast, fragile skirmisher.
    Raider,
    /// Slow armoured brawler.
    Tank,
    /// Keeps distance and lobs shots.
    Gunner,
    /// Gate guardian; the corridor stops until it falls.
    Boss,
}

impl EnemyKind {
    /// (hp, speed, radius, contact damage, xp, scrap)
    pub fn stats(self) -> (f32, f32, f32, f32, u32, u32) {
        match self {
            EnemyKind::Raider => (14.0, 85.0, 11.0, 6.0, 1, 1),
            EnemyKind::Tank => (60.0, 42.0, 17.0, 16.0, 4, 3),
            EnemyKind::Gunner => (26.0, 55.0, 13.0, 8.0, 2, 2),
            EnemyKind::Boss => (900.0, 30.0, 40.0, 30.0, 30, 40),
        }
    }
}

#[derive(Clone, Debug)]
pub struct Enemy {
    pub kind: EnemyKind,
    pub pos: V2,
    pub hp: f32,
    pub max_hp: f32,
    pub cooldown: f32,
    pub flash: f32,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Owner {
    Player,
    Tower,
    Enemy,
}

#[derive(Clone, Debug)]
pub struct Bullet {
    pub pos: V2,
    pub vel: V2,
    pub dmg: f32,
    pub owner: Owner,
    pub life: f32,
}

#[derive(Clone, Debug)]
pub struct Tower {
    pub pos: V2,
    pub hp: f32,
    pub cooldown: f32,
    /// Seconds since it was built (for the build-in animation).
    pub age: f32,
}

#[derive(Clone, Debug)]
pub struct Gem {
    pub pos: V2,
    pub xp: u32,
    pub scrap: u32,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Upgrade {
    Damage,
    FireRate,
    MultiShot,
    Speed,
    Armor,
    TowerDamage,
    TowerRange,
    CheapTowers,
    Magnet,
    Dash,
}

pub const ALL_UPGRADES: [Upgrade; 10] = [
    Upgrade::Damage,
    Upgrade::FireRate,
    Upgrade::MultiShot,
    Upgrade::Speed,
    Upgrade::Armor,
    Upgrade::TowerDamage,
    Upgrade::TowerRange,
    Upgrade::CheapTowers,
    Upgrade::Magnet,
    Upgrade::Dash,
];

impl Upgrade {
    pub fn name(self) -> &'static str {
        match self {
            Upgrade::Damage => "Heavy Rounds",
            Upgrade::FireRate => "Autoloader",
            Upgrade::MultiShot => "Twin Barrels",
            Upgrade::Speed => "Servo Legs",
            Upgrade::Armor => "Armour Plating",
            Upgrade::TowerDamage => "Turret Caliber",
            Upgrade::TowerRange => "Turret Radar",
            Upgrade::CheapTowers => "Nano Frames",
            Upgrade::Magnet => "Reclaim Field",
            Upgrade::Dash => "Jump Jets",
        }
    }
    pub fn desc(self) -> &'static str {
        match self {
            Upgrade::Damage => "+30% gun damage",
            Upgrade::FireRate => "+25% fire rate",
            Upgrade::MultiShot => "+1 projectile per volley",
            Upgrade::Speed => "+15% move speed",
            Upgrade::Armor => "+30 max HP, full repair",
            Upgrade::TowerDamage => "+35% turret damage",
            Upgrade::TowerRange => "+20% turret range, +50% turret HP",
            Upgrade::CheapTowers => "Turrets cost 3 less scrap",
            Upgrade::Magnet => "+60% pickup radius",
            Upgrade::Dash => "-30% dash cooldown",
        }
    }
}

#[derive(Clone, Debug)]
pub struct Stats {
    pub damage: f32,
    pub fire_delay: f32,
    pub shots: u32,
    pub speed: f32,
    pub max_hp: f32,
    pub tower_damage: f32,
    pub tower_range: f32,
    pub tower_hp: f32,
    pub tower_cost: u32,
    pub magnet: f32,
    pub dash_cd: f32,
}

impl Default for Stats {
    fn default() -> Self {
        Stats {
            damage: 10.0,
            fire_delay: 0.32,
            shots: 1,
            speed: 170.0,
            max_hp: 100.0,
            tower_damage: 8.0,
            tower_range: 170.0,
            tower_hp: 60.0,
            tower_cost: 12,
            magnet: 60.0,
            dash_cd: 1.6,
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub enum Phase {
    Playing,
    LevelUp([Upgrade; 3]),
    Won,
    Lost,
}

#[derive(Clone, Debug, PartialEq)]
pub enum Event {
    Shot,
    TowerShot,
    EnemyShot,
    Hit,
    Kill { pos: V2, big: bool },
    PlayerHurt,
    TowerLost,
    Pickup,
    Built,
    BuildDenied,
    LevelUp,
    Dash,
    GateReached,
    GateCleared,
    Won,
    Lost,
}

#[derive(Clone, Debug, Default)]
pub struct Input {
    /// Desired move direction (any length; normalised internally).
    pub mv: V2,
    pub dash: bool,
    /// World position to build a turret at.
    pub place: Option<V2>,
    /// Upgrade choice while levelling up (0..3).
    pub choice: Option<usize>,
}

#[derive(Clone, Debug)]
pub struct Game {
    pub rng: Rng,
    pub phase: Phase,
    pub time: f32,
    pub scroll: f32,
    pub player: V2,
    pub hp: f32,
    pub stats: Stats,
    pub fire_cd: f32,
    pub dash_cd: f32,
    pub dash_t: f32,
    pub dash_dir: V2,
    pub invuln: f32,
    pub xp: u32,
    pub level: u32,
    pub scrap: u32,
    pub kills: u32,
    pub gates_cleared: u32,
    /// True while the corridor is halted at a gate with its boss alive.
    pub at_gate: bool,
    pub spawn_acc: f32,
    pub enemies: Vec<Enemy>,
    pub bullets: Vec<Bullet>,
    pub towers: Vec<Tower>,
    pub gems: Vec<Gem>,
    pub events: Vec<Event>,
}

/// XP needed to advance from `level` to the next one.
pub fn xp_needed(level: u32) -> u32 {
    5 + level * 4
}

impl Game {
    pub fn new(seed: u64) -> Self {
        let stats = Stats::default();
        Game {
            rng: Rng::new(seed),
            phase: Phase::Playing,
            time: 0.0,
            scroll: 0.0,
            player: V2::new(WIDTH / 2.0, VIEW_H * 0.3),
            hp: stats.max_hp,
            stats,
            fire_cd: 0.0,
            dash_cd: 0.0,
            dash_t: 0.0,
            dash_dir: V2::new(0.0, 1.0),
            invuln: 0.0,
            xp: 0,
            level: 1,
            scrap: 40,
            kills: 0,
            gates_cleared: 0,
            at_gate: false,
            spawn_acc: 0.0,
            enemies: Vec::new(),
            bullets: Vec::new(),
            towers: Vec::new(),
            gems: Vec::new(),
            events: Vec::new(),
        }
    }

    /// World y of the next gate.
    pub fn next_gate_y(&self) -> f32 {
        (self.gates_cleared + 1) as f32 * GATE_SPACING
    }

    /// Overall run progress in `0..=1`.
    pub fn progress(&self) -> f32 {
        ((self.scroll + VIEW_H * 0.75) / (GATE_SPACING * GATES as f32)).clamp(0.0, 1.0)
    }

    /// Difficulty grows with distance and gates beaten.
    pub fn threat(&self) -> f32 {
        1.0 + self.scroll / 900.0 + self.gates_cleared as f32 * 0.6
    }

    pub fn can_build(&self, at: V2) -> bool {
        self.scrap >= self.stats.tower_cost
            && at.dist(self.player) <= BUILD_RANGE
            && at.x >= TOWER_R
            && at.x <= WIDTH - TOWER_R
            && at.y >= self.scroll + TOWER_R
            && at.y <= self.scroll + VIEW_H - TOWER_R
            && self.towers.iter().all(|t| t.pos.dist(at) >= TOWER_R * 2.2)
    }

    fn roll_upgrades(&mut self) -> [Upgrade; 3] {
        let mut pool: Vec<Upgrade> = ALL_UPGRADES
            .iter()
            .copied()
            .filter(|u| *u != Upgrade::CheapTowers || self.stats.tower_cost > 4)
            .collect();
        let mut out = [Upgrade::Damage; 3];
        for slot in &mut out {
            let i = self.rng.below(pool.len());
            *slot = pool.swap_remove(i);
        }
        out
    }

    pub fn apply(&mut self, u: Upgrade) {
        let s = &mut self.stats;
        match u {
            Upgrade::Damage => s.damage *= 1.3,
            Upgrade::FireRate => s.fire_delay /= 1.25,
            Upgrade::MultiShot => s.shots += 1,
            Upgrade::Speed => s.speed *= 1.15,
            Upgrade::Armor => {
                s.max_hp += 30.0;
                self.hp = s.max_hp;
            }
            Upgrade::TowerDamage => s.tower_damage *= 1.35,
            Upgrade::TowerRange => {
                s.tower_range *= 1.2;
                s.tower_hp *= 1.5;
            }
            Upgrade::CheapTowers => s.tower_cost = s.tower_cost.saturating_sub(3).max(3),
            Upgrade::Magnet => s.magnet *= 1.6,
            Upgrade::Dash => s.dash_cd *= 0.7,
        }
    }

    pub fn spawn(&mut self, kind: EnemyKind, pos: V2) {
        let (hp, ..) = kind.stats();
        let hp = if kind == EnemyKind::Boss {
            hp * (1.0 + self.gates_cleared as f32 * 0.7)
        } else {
            hp * (1.0 + (self.threat() - 1.0) * 0.35)
        };
        self.enemies.push(Enemy {
            kind,
            pos,
            hp,
            max_hp: hp,
            cooldown: 1.0 + self.rng.f32(),
            flash: 0.0,
        });
    }

    fn spawn_wave(&mut self, dt: f32) {
        let threat = self.threat();
        let rate = if self.at_gate {
            0.5
        } else {
            0.4 + threat * 0.6
        };
        self.spawn_acc += rate * dt;
        while self.spawn_acc >= 1.0 {
            self.spawn_acc -= 1.0;
            // Share of enemies flanking from behind grows with distance.
            let rear_share = (0.12 + self.scroll / 9000.0).min(0.4);
            let rear = self.rng.chance(rear_share);
            let y = if rear {
                self.scroll - 30.0
            } else {
                self.scroll + VIEW_H + 30.0
            };
            let x = self.rng.range(20.0, WIDTH - 20.0);
            let r = self.rng.f32();
            let kind = if r < 0.12 * threat.min(3.0) {
                EnemyKind::Tank
            } else if r < 0.12 * threat.min(3.0) + 0.18 {
                EnemyKind::Gunner
            } else {
                EnemyKind::Raider
            };
            self.spawn(kind, V2::new(x, y));
        }
    }

    pub fn step(&mut self, input: &Input, dt: f32) {
        self.events.clear();
        match self.phase {
            Phase::Playing => {}
            Phase::LevelUp(opts) => {
                if let Some(c) = input.choice.filter(|c| *c < 3) {
                    self.apply(opts[c]);
                    self.phase = Phase::Playing;
                }
                return;
            }
            Phase::Won | Phase::Lost => return,
        }
        self.time += dt;

        // Corridor scroll and boss gates.
        if !self.at_gate {
            self.scroll += SCROLL_SPEED * dt;
            let gate = self.next_gate_y();
            if self.scroll + VIEW_H * 0.75 >= gate {
                self.scroll = gate - VIEW_H * 0.75;
                self.at_gate = true;
                self.events.push(Event::GateReached);
                self.spawn(EnemyKind::Boss, V2::new(WIDTH / 2.0, gate + 60.0));
            }
        }

        self.update_player(input, dt);
        if let Some(at) = input.place {
            if self.can_build(at) {
                self.scrap -= self.stats.tower_cost;
                self.towers.push(Tower {
                    pos: at,
                    hp: self.stats.tower_hp,
                    cooldown: 0.0,
                    age: 0.0,
                });
                self.events.push(Event::Built);
            } else {
                self.events.push(Event::BuildDenied);
            }
        }
        self.spawn_wave(dt);
        self.update_towers(dt);
        self.update_enemies(dt);
        self.update_bullets(dt);
        self.update_gems(dt);
        self.cull();

        if self.hp <= 0.0 {
            self.hp = 0.0;
            self.phase = Phase::Lost;
            self.events.push(Event::Lost);
        } else if self.xp >= xp_needed(self.level) && self.phase == Phase::Playing {
            self.xp -= xp_needed(self.level);
            self.level += 1;
            let opts = self.roll_upgrades();
            self.phase = Phase::LevelUp(opts);
            self.events.push(Event::LevelUp);
        }
    }

    fn update_player(&mut self, input: &Input, dt: f32) {
        self.fire_cd -= dt;
        self.dash_cd -= dt;
        self.invuln -= dt;
        let dir = input.mv.norm();
        if input.dash && self.dash_cd <= 0.0 {
            self.dash_dir = if dir.len() > 0.0 {
                dir
            } else {
                V2::new(0.0, 1.0)
            };
            self.dash_t = DASH_TIME;
            self.dash_cd = self.stats.dash_cd;
            self.invuln = self.invuln.max(DASH_TIME + 0.1);
            self.events.push(Event::Dash);
        }
        let vel = if self.dash_t > 0.0 {
            self.dash_t -= dt;
            self.dash_dir * DASH_SPEED
        } else {
            dir * self.stats.speed
        };
        self.player = self.player + vel * dt;
        self.player.x = self.player.x.clamp(PLAYER_R, WIDTH - PLAYER_R);
        self.player.y = self
            .player
            .y
            .clamp(self.scroll + PLAYER_R, self.scroll + VIEW_H - PLAYER_R);

        if self.fire_cd <= 0.0
            && let Some(target) = nearest(&self.enemies, self.player, 420.0)
        {
            self.fire_cd = self.stats.fire_delay;
            let aim = (self.enemies[target].pos - self.player).norm();
            let n = self.stats.shots;
            for i in 0..n {
                let spread = (i as f32 - (n - 1) as f32 / 2.0) * 0.14;
                let (s, c) = spread.sin_cos();
                let d = V2::new(aim.x * c - aim.y * s, aim.x * s + aim.y * c);
                self.bullets.push(Bullet {
                    pos: self.player,
                    vel: d * 520.0,
                    dmg: self.stats.damage,
                    owner: Owner::Player,
                    life: 1.0,
                });
            }
            self.events.push(Event::Shot);
        }
    }

    fn update_towers(&mut self, dt: f32) {
        for i in 0..self.towers.len() {
            let t = &mut self.towers[i];
            t.age += dt;
            t.cooldown -= dt;
            if t.cooldown > 0.0 {
                continue;
            }
            let pos = t.pos;
            if let Some(e) = nearest(&self.enemies, pos, self.stats.tower_range) {
                self.towers[i].cooldown = 0.55;
                let d = (self.enemies[e].pos - pos).norm();
                self.bullets.push(Bullet {
                    pos,
                    vel: d * 460.0,
                    dmg: self.stats.tower_damage,
                    owner: Owner::Tower,
                    life: 0.6,
                });
                self.events.push(Event::TowerShot);
            }
        }
    }

    fn update_enemies(&mut self, dt: f32) {
        let player = self.player;
        for i in 0..self.enemies.len() {
            let (_, speed, radius, contact, ..) = self.enemies[i].kind.stats();
            let kind = self.enemies[i].kind;
            let pos = self.enemies[i].pos;
            self.enemies[i].flash -= dt;
            self.enemies[i].cooldown -= dt;
            // Pick the closest of player and towers as the target.
            let mut target = player;
            let mut best = pos.dist(player);
            for t in &self.towers {
                let d = pos.dist(t.pos);
                if d < best * 0.8 {
                    best = d;
                    target = t.pos;
                }
            }
            let to = target - pos;
            let mv = match kind {
                EnemyKind::Gunner if best < 180.0 => to.norm() * -0.4,
                EnemyKind::Boss => {
                    // Hover in front of the gate, strafing.
                    let gate = self.next_gate_y();
                    let want = V2::new(
                        WIDTH / 2.0 + (self.time * 0.7).sin() * 150.0,
                        (gate - 40.0).min(self.scroll + VIEW_H - 70.0),
                    );
                    (want - pos).norm() * 1.5
                }
                _ => to.norm(),
            };
            self.enemies[i].pos = pos + mv * speed * dt;

            let shoots = matches!(kind, EnemyKind::Gunner | EnemyKind::Boss);
            if shoots && self.enemies[i].cooldown <= 0.0 && best < 450.0 {
                let aim = to.norm();
                if kind == EnemyKind::Boss {
                    self.enemies[i].cooldown = 1.1;
                    for k in -3..=3 {
                        let a = k as f32 * 0.22;
                        let (s, c) = a.sin_cos();
                        let d = V2::new(aim.x * c - aim.y * s, aim.x * s + aim.y * c);
                        self.bullets.push(Bullet {
                            pos,
                            vel: d * 200.0,
                            dmg: 10.0,
                            owner: Owner::Enemy,
                            life: 4.0,
                        });
                    }
                } else {
                    self.enemies[i].cooldown = 2.2;
                    self.bullets.push(Bullet {
                        pos,
                        vel: aim * 230.0,
                        dmg: 7.0,
                        owner: Owner::Enemy,
                        life: 3.0,
                    });
                }
                self.events.push(Event::EnemyShot);
            }

            // Contact damage.
            let pos = self.enemies[i].pos;
            if pos.dist(player) < radius + PLAYER_R && self.invuln <= 0.0 {
                self.hp -= contact;
                self.invuln = 0.8;
                self.events.push(Event::PlayerHurt);
            }
            for t in &mut self.towers {
                if pos.dist(t.pos) < radius + TOWER_R {
                    t.hp -= contact * dt * 2.0;
                }
            }
        }
    }

    fn update_bullets(&mut self, dt: f32) {
        let mut bullets = std::mem::take(&mut self.bullets);
        bullets.retain_mut(|b| {
            b.pos = b.pos + b.vel * dt;
            b.life -= dt;
            if b.life <= 0.0 {
                return false;
            }
            match b.owner {
                Owner::Enemy => {
                    if self.invuln <= 0.0 && b.pos.dist(self.player) < PLAYER_R {
                        self.hp -= b.dmg;
                        self.invuln = 0.35;
                        self.events.push(Event::PlayerHurt);
                        return false;
                    }
                    for t in &mut self.towers {
                        if b.pos.dist(t.pos) < TOWER_R {
                            t.hp -= b.dmg;
                            return false;
                        }
                    }
                    true
                }
                Owner::Player | Owner::Tower => {
                    for e in &mut self.enemies {
                        let (_, _, r, ..) = e.kind.stats();
                        if e.hp > 0.0 && b.pos.dist(e.pos) < r + 4.0 {
                            e.hp -= b.dmg;
                            e.flash = 0.08;
                            self.events.push(Event::Hit);
                            return false;
                        }
                    }
                    true
                }
            }
        });
        self.bullets = bullets;
    }

    fn update_gems(&mut self, dt: f32) {
        let player = self.player;
        let magnet = self.stats.magnet;
        let mut picked = Vec::new();
        self.gems.retain_mut(|g| {
            let d = g.pos.dist(player);
            if d < PLAYER_R + 6.0 {
                picked.push((g.xp, g.scrap));
                return false;
            }
            if d < magnet {
                g.pos = g.pos + (player - g.pos).norm() * 380.0 * dt;
            }
            true
        });
        for (xp, scrap) in picked {
            self.xp += xp;
            self.scrap += scrap;
            self.events.push(Event::Pickup);
        }
    }

    fn cull(&mut self) {
        // Dead enemies drop gems.
        let mut i = 0;
        while i < self.enemies.len() {
            if self.enemies[i].hp <= 0.0 {
                let e = self.enemies.swap_remove(i);
                let (.., xp, scrap) = e.kind.stats();
                self.kills += 1;
                let big = e.kind == EnemyKind::Boss;
                self.events.push(Event::Kill { pos: e.pos, big });
                self.gems.push(Gem {
                    pos: e.pos,
                    xp,
                    scrap,
                });
                if big {
                    self.gates_cleared += 1;
                    self.at_gate = false;
                    self.hp = (self.hp + 30.0).min(self.stats.max_hp);
                    if self.gates_cleared >= GATES {
                        self.phase = Phase::Won;
                        self.events.push(Event::Won);
                    } else {
                        self.events.push(Event::GateCleared);
                    }
                }
            } else {
                i += 1;
            }
        }
        let lo = self.scroll - 80.0;
        let hi = self.scroll + VIEW_H + 120.0;
        self.enemies
            .retain(|e| e.kind == EnemyKind::Boss || (e.pos.y > lo - 40.0 && e.pos.y < hi));
        self.bullets
            .retain(|b| b.pos.y > lo && b.pos.y < hi && b.pos.x > -20.0 && b.pos.x < WIDTH + 20.0);
        self.gems.retain(|g| g.pos.y > lo);
        let before = self.towers.len();
        let scroll = self.scroll;
        self.towers
            .retain(|t| t.hp > 0.0 && t.pos.y > scroll - TOWER_R);
        if self.towers.len() < before {
            self.events.push(Event::TowerLost);
        }
    }
}

fn nearest(enemies: &[Enemy], from: V2, range: f32) -> Option<usize> {
    enemies
        .iter()
        .enumerate()
        .filter(|(_, e)| e.hp > 0.0)
        .map(|(i, e)| (i, e.pos.dist(from)))
        .filter(|(_, d)| *d <= range)
        .min_by(|a, b| a.1.total_cmp(&b.1))
        .map(|(i, _)| i)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn idle() -> Input {
        Input::default()
    }

    #[test]
    fn corridor_scrolls_and_player_is_dragged_along() {
        let mut g = Game::new(1);
        g.player.y = 20.0;
        for _ in 0..120 {
            g.step(&idle(), 1.0 / 60.0);
            g.enemies.clear();
        }
        assert!(g.scroll > 30.0);
        assert!(g.player.y >= g.scroll + PLAYER_R - 0.01);
    }

    #[test]
    fn building_costs_scrap_and_respects_range() {
        let mut g = Game::new(2);
        let near = g.player + V2::new(0.0, 60.0);
        let far = g.player + V2::new(0.0, BUILD_RANGE + 50.0);
        let before = g.scrap;
        g.step(
            &Input {
                place: Some(far),
                ..idle()
            },
            0.0,
        );
        assert!(g.towers.is_empty());
        assert!(g.events.contains(&Event::BuildDenied));
        g.step(
            &Input {
                place: Some(near),
                ..idle()
            },
            0.0,
        );
        assert_eq!(g.towers.len(), 1);
        assert_eq!(g.scrap, before - g.stats.tower_cost);
        // Cannot stack on top of an existing tower.
        assert!(!g.can_build(near));
    }

    #[test]
    fn towers_are_lost_when_scrolled_off() {
        let mut g = Game::new(3);
        g.towers.push(Tower {
            pos: V2::new(100.0, 10.0),
            hp: 50.0,
            cooldown: 0.0,
            age: 0.0,
        });
        g.scroll = 100.0;
        g.cull();
        assert!(g.towers.is_empty());
        assert!(g.events.contains(&Event::TowerLost));
    }

    #[test]
    fn towers_shoot_enemies_behind_the_player() {
        let mut g = Game::new(4);
        g.towers.push(Tower {
            pos: V2::new(240.0, 40.0),
            hp: 50.0,
            cooldown: 0.0,
            age: 0.0,
        });
        g.spawn(EnemyKind::Raider, V2::new(240.0, 120.0));
        g.update_towers(0.016);
        assert!(g.events.contains(&Event::TowerShot));
        assert!(g.bullets.iter().any(|b| b.owner == Owner::Tower));
    }

    #[test]
    fn killing_enemies_drops_xp_and_levels_up() {
        let mut g = Game::new(5);
        g.gems.push(Gem {
            pos: g.player,
            xp: xp_needed(1),
            scrap: 2,
        });
        g.step(&idle(), 0.01);
        assert!(matches!(g.phase, Phase::LevelUp(_)));
        assert_eq!(g.level, 2);
        let dmg = g.stats.damage;
        let Phase::LevelUp(opts) = g.phase.clone() else {
            unreachable!()
        };
        g.step(
            &Input {
                choice: Some(0),
                ..idle()
            },
            0.01,
        );
        assert_eq!(g.phase, Phase::Playing);
        if opts[0] == Upgrade::Damage {
            assert!(g.stats.damage > dmg);
        }
    }

    #[test]
    fn upgrade_offers_are_distinct() {
        let mut g = Game::new(6);
        for _ in 0..50 {
            let o = g.roll_upgrades();
            assert!(o[0] != o[1] && o[1] != o[2] && o[0] != o[2]);
        }
    }

    #[test]
    fn gate_halts_scroll_until_boss_dies_and_last_gate_wins() {
        let mut g = Game::new(7);
        g.scroll = g.next_gate_y() - VIEW_H * 0.75 - 1.0;
        g.step(&idle(), 0.1);
        assert!(g.at_gate);
        assert!(g.events.contains(&Event::GateReached));
        let s = g.scroll;
        g.step(&idle(), 0.5);
        assert_eq!(g.scroll, s);
        for gate in 0..GATES {
            g.at_gate = true;
            g.enemies.retain(|e| e.kind == EnemyKind::Boss);
            if g.enemies.is_empty() {
                g.spawn(EnemyKind::Boss, g.player);
            }
            for e in &mut g.enemies {
                e.hp = 0.0;
            }
            g.cull();
            assert_eq!(g.gates_cleared, gate + 1);
        }
        assert_eq!(g.phase, Phase::Won);
    }

    #[test]
    fn dash_grants_invulnerability_and_cooldown() {
        let mut g = Game::new(8);
        g.step(
            &Input {
                dash: true,
                mv: V2::new(1.0, 0.0),
                ..idle()
            },
            0.01,
        );
        assert!(g.events.contains(&Event::Dash));
        assert!(g.invuln > 0.0);
        let x = g.player.x;
        g.step(
            &Input {
                dash: true,
                ..idle()
            },
            0.05,
        );
        assert!(g.player.x > x);
        assert!(!g.events.contains(&Event::Dash));
    }

    #[test]
    fn player_dies_and_game_is_lost() {
        let mut g = Game::new(9);
        g.hp = 1.0;
        g.bullets.push(Bullet {
            pos: g.player,
            vel: V2::default(),
            dmg: 5.0,
            owner: Owner::Enemy,
            life: 1.0,
        });
        g.step(&idle(), 0.01);
        assert_eq!(g.phase, Phase::Lost);
        assert!(g.events.contains(&Event::Lost));
    }

    #[test]
    fn rear_spawns_appear_and_threat_rises() {
        let mut g = Game::new(10);
        let t0 = g.threat();
        g.scroll = 3000.0;
        assert!(g.threat() > t0);
        for _ in 0..400 {
            g.spawn_wave(0.1);
        }
        assert!(g.enemies.iter().any(|e| e.pos.y < g.scroll));
        assert!(g.enemies.iter().any(|e| e.pos.y > g.scroll + VIEW_H));
    }
}
