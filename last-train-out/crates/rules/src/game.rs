//! Game state and the fixed-step simulation.

use crate::math::{CIRC, V2, arc_dist, track_pos, wrap};
use crate::rng::Rng;

/// Seconds the train must hold out before it can depart.
pub const GAME_LEN: f32 = 15.0 * 60.0;
/// Distance between the centres of neighbouring carriages along the track.
pub const CAR_PITCH: f32 = 70.0;
/// Visual carriage length and width.
pub const CAR_LEN: f32 = 62.0;
pub const CAR_W: f32 = 30.0;
pub const MAX_CARS: usize = 14;
/// Throttle notches: stopped, slow, cruise, full steam.
pub const SPEEDS: [f32; 4] = [0.0, 55.0, 105.0, 160.0];
/// How close (along the track) the engine must stop to a station.
pub const STATION_ZONE: f32 = 150.0;
pub const STATION_COUNT: usize = 3;
pub const STATION_MAX_WAITING: u32 = 14;
pub const COACH_SEATS: usize = 6;
pub const BASE_FARE: f32 = 6.0;
const ENEMY_CAP: usize = 420;
const LEASH: f32 = 1150.0;
const SPAWN_DIST: f32 = 820.0;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CarKind {
    Engine,
    Gun,
    Flak,
    Workshop,
    Infirmary,
    Tender,
    Coach,
}

impl CarKind {
    /// Carriages that can be bought at stations, in build-menu order.
    pub const BUYABLE: [CarKind; 6] = [
        CarKind::Gun,
        CarKind::Flak,
        CarKind::Coach,
        CarKind::Workshop,
        CarKind::Tender,
        CarKind::Infirmary,
    ];

    pub fn name(self) -> &'static str {
        match self {
            CarKind::Engine => "Engine",
            CarKind::Gun => "Gun Car",
            CarKind::Flak => "Flak Car",
            CarKind::Workshop => "Workshop",
            CarKind::Infirmary => "Infirmary",
            CarKind::Tender => "Coal Tender",
            CarKind::Coach => "Coach",
        }
    }

    pub fn blurb(self) -> &'static str {
        match self {
            CarKind::Engine => "Lose it and the line is closed.",
            CarKind::Gun => "Auto turret. Man it to aim and fire fast.",
            CarKind::Flak => "Slow exploding shells, hits crowds.",
            CarKind::Workshop => "Repairs itself and both neighbours.",
            CarKind::Infirmary => "Neighbour coaches +2 seats, fares x1.5.",
            CarKind::Tender => "Neighbour guns fire 35% faster, +8% speed.",
            CarKind::Coach => "Seats 6 refugees. Fares pay the bills.",
        }
    }

    pub fn base_price(self) -> u32 {
        match self {
            CarKind::Engine => 0,
            CarKind::Gun => 40,
            CarKind::Flak => 70,
            CarKind::Workshop => 55,
            CarKind::Infirmary => 60,
            CarKind::Tender => 45,
            CarKind::Coach => 30,
        }
    }

    pub fn base_hp(self) -> f32 {
        match self {
            CarKind::Engine => 600.0,
            CarKind::Gun | CarKind::Flak => 160.0,
            CarKind::Workshop => 200.0,
            CarKind::Tender => 180.0,
            CarKind::Infirmary | CarKind::Coach => 140.0,
        }
    }

    pub fn is_gun(self) -> bool {
        matches!(self, CarKind::Gun | CarKind::Flak)
    }
}

#[derive(Clone, Debug)]
pub struct Car {
    pub kind: CarKind,
    pub hp: f32,
    pub max_hp: f32,
    pub cooldown: f32,
    /// Station index where each seated passenger boarded.
    pub passengers: Vec<u8>,
    /// Turret angle in radians (world space).
    pub aim: f32,
    /// Seconds since the car was last hurt (for flashing).
    pub hurt: f32,
}

impl Car {
    pub fn new(kind: CarKind, armor_mul: f32) -> Self {
        let max_hp = kind.base_hp() * armor_mul;
        Car {
            kind,
            hp: max_hp,
            max_hp,
            cooldown: 0.5,
            passengers: Vec::new(),
            aim: 0.0,
            hurt: 10.0,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EnemyKind {
    Walker,
    Runner,
    Brute,
    Juggernaut,
}

impl EnemyKind {
    /// (hp, speed, radius, damage per second, xp)
    fn stats(self) -> (f32, f32, f32, f32, u32) {
        match self {
            EnemyKind::Walker => (12.0, 48.0, 11.0, 4.0, 1),
            EnemyKind::Runner => (8.0, 92.0, 9.0, 3.0, 1),
            EnemyKind::Brute => (70.0, 36.0, 17.0, 12.0, 4),
            EnemyKind::Juggernaut => (650.0, 30.0, 32.0, 30.0, 30),
        }
    }
}

#[derive(Clone, Debug)]
pub struct Enemy {
    pub kind: EnemyKind,
    pub pos: V2,
    pub hp: f32,
    pub max_hp: f32,
    pub speed: f32,
    pub radius: f32,
    pub dps: f32,
    pub xp: u32,
    pub hurt: f32,
    pub biting: bool,
    /// Random phase for animation.
    pub wobble: f32,
}

#[derive(Clone, Debug)]
pub struct Bullet {
    pub pos: V2,
    pub vel: V2,
    pub dmg: f32,
    pub life: f32,
    pub splash: f32,
}

#[derive(Clone, Debug)]
pub struct Coin {
    pub pos: V2,
    pub value: u32,
    pub age: f32,
}

#[derive(Clone, Debug)]
pub struct Station {
    pub s: f32,
    pub waiting: u32,
    pub timer: f32,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Phase {
    Playing,
    LevelUp,
    Won,
    Lost,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Upgrade {
    Damage,
    FireRate,
    Range,
    Armor,
    Fares,
    Repair,
    Multishot,
    Engine,
}

impl Upgrade {
    pub const ALL: [Upgrade; 8] = [
        Upgrade::Damage,
        Upgrade::FireRate,
        Upgrade::Range,
        Upgrade::Armor,
        Upgrade::Fares,
        Upgrade::Repair,
        Upgrade::Multishot,
        Upgrade::Engine,
    ];

    pub fn name(self) -> &'static str {
        match self {
            Upgrade::Damage => "Hollow Points",
            Upgrade::FireRate => "Greased Breech",
            Upgrade::Range => "Rangefinder",
            Upgrade::Armor => "Boiler Plate",
            Upgrade::Fares => "Ticket Inspector",
            Upgrade::Repair => "Spare Rivets",
            Upgrade::Multishot => "Twin Barrels",
            Upgrade::Engine => "Superheater",
        }
    }

    pub fn blurb(self) -> &'static str {
        match self {
            Upgrade::Damage => "All guns +25% damage",
            Upgrade::FireRate => "All guns fire 20% faster",
            Upgrade::Range => "Gun range +15%",
            Upgrade::Armor => "Cars +25% max HP, full repair",
            Upgrade::Fares => "Fares +30%",
            Upgrade::Repair => "Repairs 50% faster",
            Upgrade::Multishot => "Guns fire +1 extra shell",
            Upgrade::Engine => "+15% speed, ramming hurts more",
        }
    }
}

#[derive(Clone, Debug)]
pub struct Stats {
    pub dmg_mul: f32,
    pub rate_mul: f32,
    pub range_mul: f32,
    pub armor_mul: f32,
    pub fare_mul: f32,
    pub repair_mul: f32,
    pub multishot: u32,
    pub speed_mul: f32,
}

impl Default for Stats {
    fn default() -> Self {
        Stats {
            dmg_mul: 1.0,
            rate_mul: 1.0,
            range_mul: 1.0,
            armor_mul: 1.0,
            fare_mul: 1.0,
            repair_mul: 1.0,
            multishot: 0,
            speed_mul: 1.0,
        }
    }
}

/// Player intent for one simulation step.
#[derive(Clone, Copy, Debug, Default)]
pub struct Input {
    /// -1 walks toward the engine, +1 toward the tail.
    pub walk: f32,
    /// -1/+1 jumps one carriage forward/back (edge triggered).
    pub car_step: i32,
    /// Fire the manned gun (held).
    pub fire: bool,
    /// World-space aim point.
    pub aim: V2,
    /// -1/+1 moves the throttle one notch (edge triggered).
    pub throttle: i32,
}

/// Things that happened during a step, for sound and particles.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Event {
    Shot { heavy: bool, pos: V2 },
    Hit { pos: V2 },
    Boom { pos: V2, radius: f32 },
    Kill { pos: V2, big: bool },
    CarHit { car: usize },
    CarLost { pos: V2 },
    Coin { pos: V2, value: u32 },
    Board { pos: V2 },
    Fare { pos: V2, amount: u32 },
    Arrive { station: usize },
    LevelUp,
    Built,
    Surge,
    BossSpawn,
    Won,
    Lost,
}

pub struct Game {
    pub time: f32,
    pub phase: Phase,
    pub head_s: f32,
    pub speed: f32,
    pub throttle: usize,
    pub cars: Vec<Car>,
    /// Conductor position in carriage units (0 = engine).
    pub conductor: f32,
    pub enemies: Vec<Enemy>,
    pub bullets: Vec<Bullet>,
    pub coins: Vec<Coin>,
    pub stations: Vec<Station>,
    pub money: u32,
    pub rescued: u32,
    pub lost_passengers: u32,
    pub kills: u32,
    pub level: u32,
    pub xp: u32,
    pub xp_next: u32,
    pub stats: Stats,
    pub choices: Vec<Upgrade>,
    pub events: Vec<Event>,
    pub at_station: Option<usize>,
    rng: Rng,
    spawn_acc: f32,
    next_boss: f32,
    next_surge: f32,
    board_timer: f32,
}

impl Game {
    pub fn new(seed: u64) -> Self {
        let stats = Stats::default();
        let cars = [CarKind::Engine, CarKind::Gun, CarKind::Coach, CarKind::Gun]
            .into_iter()
            .map(|k| Car::new(k, 1.0))
            .collect();
        let stations = (0..STATION_COUNT)
            .map(|i| Station {
                s: CIRC * i as f32 / STATION_COUNT as f32 + 400.0,
                waiting: 6,
                timer: 0.0,
            })
            .collect();
        Game {
            time: 0.0,
            phase: Phase::Playing,
            head_s: 200.0,
            speed: 0.0,
            throttle: 1,
            cars,
            conductor: 1.0,
            enemies: Vec::new(),
            bullets: Vec::new(),
            coins: Vec::new(),
            stations,
            money: 50,
            rescued: 0,
            lost_passengers: 0,
            kills: 0,
            level: 1,
            xp: 0,
            xp_next: xp_for_level(1),
            stats,
            choices: Vec::new(),
            events: Vec::new(),
            at_station: None,
            rng: Rng::new(seed),
            spawn_acc: 0.0,
            next_boss: 180.0,
            next_surge: 60.0,
            board_timer: 0.0,
        }
    }

    // ----- geometry -------------------------------------------------------

    pub fn car_s(&self, i: usize) -> f32 {
        wrap(self.head_s - i as f32 * CAR_PITCH)
    }

    pub fn car_pos(&self, i: usize) -> V2 {
        track_pos(self.car_s(i))
    }

    pub fn conductor_car(&self) -> usize {
        (self.conductor.round().max(0.0) as usize).min(self.cars.len() - 1)
    }

    pub fn conductor_s(&self) -> f32 {
        wrap(self.head_s - self.conductor * CAR_PITCH)
    }

    pub fn conductor_pos(&self) -> V2 {
        track_pos(self.conductor_s())
    }

    pub fn time_left(&self) -> f32 {
        (GAME_LEN - self.time).max(0.0)
    }

    // ----- consist rules --------------------------------------------------

    fn neighbours(&self, i: usize) -> impl Iterator<Item = &Car> {
        let lo = i.saturating_sub(1);
        let hi = (i + 1).min(self.cars.len() - 1);
        (lo..=hi).filter(move |&j| j != i).map(|j| &self.cars[j])
    }

    fn next_to(&self, i: usize, kind: CarKind) -> bool {
        self.neighbours(i).any(|c| c.kind == kind)
    }

    pub fn seats(&self, i: usize) -> usize {
        if self.cars[i].kind != CarKind::Coach {
            return 0;
        }
        COACH_SEATS
            + if self.next_to(i, CarKind::Infirmary) {
                2
            } else {
                0
            }
    }

    pub fn capacity(&self) -> usize {
        (0..self.cars.len()).map(|i| self.seats(i)).sum()
    }

    pub fn aboard(&self) -> usize {
        self.cars.iter().map(|c| c.passengers.len()).sum()
    }

    pub fn fare(&self, i: usize) -> u32 {
        let infirmary = if self.next_to(i, CarKind::Infirmary) {
            1.5
        } else {
            1.0
        };
        (BASE_FARE * self.stats.fare_mul * infirmary).round() as u32
    }

    pub fn count(&self, kind: CarKind) -> usize {
        self.cars.iter().filter(|c| c.kind == kind).count()
    }

    pub fn price(&self, kind: CarKind) -> u32 {
        let base = kind.base_price();
        base + base * self.count(kind) as u32 / 2
    }

    pub fn max_speed(&self) -> f32 {
        let tenders = self.count(CarKind::Tender) as f32;
        SPEEDS[self.throttle] * self.stats.speed_mul * (1.0 + 0.08 * tenders)
    }

    /// Buys a carriage and couples it at the tail. Only works at a station.
    pub fn buy(&mut self, kind: CarKind) -> bool {
        let price = self.price(kind);
        if self.at_station.is_none()
            || kind == CarKind::Engine
            || self.cars.len() >= MAX_CARS
            || self.money < price
        {
            return false;
        }
        self.money -= price;
        self.cars.push(Car::new(kind, self.stats.armor_mul));
        self.events.push(Event::Built);
        true
    }

    /// Cost of fully repairing every carriage at a station yard.
    pub fn repair_cost(&self) -> u32 {
        let missing: f32 = self.cars.iter().map(|c| c.max_hp - c.hp).sum();
        (missing * 0.15).ceil() as u32
    }

    /// Repairs the whole train at a station, as far as the money goes.
    pub fn repair_all(&mut self) -> bool {
        let cost = self.repair_cost();
        if self.at_station.is_none() || cost == 0 || self.money == 0 {
            return false;
        }
        let k = (self.money as f32 / cost as f32).min(1.0);
        for c in &mut self.cars {
            c.hp += (c.max_hp - c.hp) * k;
        }
        self.money -= ((cost as f32 * k).ceil() as u32).min(self.money);
        self.events.push(Event::Built);
        true
    }

    /// Swaps the conductor's carriage with its neighbour (`dir` = -1 or +1).
    /// The engine always stays in front. Only works at a station.
    pub fn shunt(&mut self, dir: i32) -> bool {
        let i = self.conductor_car();
        let j = i as i32 + dir;
        if self.at_station.is_none() || i == 0 || j < 1 || j as usize >= self.cars.len() {
            return false;
        }
        self.cars.swap(i, j as usize);
        self.conductor = j as f32;
        self.trim_passengers();
        self.events.push(Event::Built);
        true
    }

    /// Drops passengers that no longer have a seat (after shunting or losses).
    fn trim_passengers(&mut self) {
        for i in 0..self.cars.len() {
            let seats = self.seats(i);
            let car = &mut self.cars[i];
            if car.passengers.len() > seats {
                let extra = car.passengers.len() - seats;
                car.passengers.truncate(seats);
                self.lost_passengers += extra as u32;
            }
        }
    }

    pub fn choose(&mut self, idx: usize) {
        if self.phase != Phase::LevelUp || idx >= self.choices.len() {
            return;
        }
        let up = self.choices[idx];
        let s = &mut self.stats;
        match up {
            Upgrade::Damage => s.dmg_mul += 0.25,
            Upgrade::FireRate => s.rate_mul *= 1.2,
            Upgrade::Range => s.range_mul += 0.15,
            Upgrade::Armor => {
                s.armor_mul += 0.25;
                let k = s.armor_mul;
                for c in &mut self.cars {
                    c.max_hp = c.kind.base_hp() * k;
                    c.hp = c.max_hp;
                }
            }
            Upgrade::Fares => s.fare_mul += 0.3,
            Upgrade::Repair => s.repair_mul += 0.5,
            Upgrade::Multishot => s.multishot += 1,
            Upgrade::Engine => s.speed_mul += 0.15,
        }
        self.choices.clear();
        self.phase = Phase::Playing;
        self.check_level();
    }

    fn check_level(&mut self) {
        if self.phase == Phase::Playing && self.xp >= self.xp_next {
            self.xp -= self.xp_next;
            self.level += 1;
            self.xp_next = xp_for_level(self.level);
            let mut pool: Vec<Upgrade> = Upgrade::ALL
                .into_iter()
                .filter(|u| *u != Upgrade::Multishot || self.stats.multishot < 3)
                .collect();
            self.choices.clear();
            for _ in 0..3 {
                let k = self.rng.below(pool.len());
                self.choices.push(pool.swap_remove(k));
            }
            self.phase = Phase::LevelUp;
            self.events.push(Event::LevelUp);
        }
    }

    // ----- simulation -----------------------------------------------------

    pub fn step(&mut self, dt: f32, input: &Input) {
        if self.phase != Phase::Playing {
            return;
        }
        self.time += dt;
        if self.time >= GAME_LEN {
            self.phase = Phase::Won;
            self.events.push(Event::Won);
            return;
        }
        self.drive(dt, input);
        self.stations_tick(dt);
        self.conductor_tick(dt, input);
        self.repairs(dt);
        self.guns(dt, input);
        self.bullets_tick(dt);
        self.enemies_tick(dt);
        self.reap();
        self.coins_tick(dt);
        self.wrecks();
        if self.phase == Phase::Playing {
            self.spawn(dt);
            self.check_level();
        }
        for c in &mut self.cars {
            c.hurt += dt;
        }
    }

    fn drive(&mut self, dt: f32, input: &Input) {
        self.throttle = (self.throttle as i32 + input.throttle).clamp(0, 3) as usize;
        let target = self.max_speed();
        let rate = if target > self.speed { 45.0 } else { 110.0 };
        let d = (target - self.speed).clamp(-rate * dt, rate * dt);
        self.speed += d;
        self.head_s = wrap(self.head_s + self.speed * dt);

        let was = self.at_station;
        self.at_station = if self.speed < 1.0 {
            self.stations
                .iter()
                .position(|st| arc_dist(st.s, self.head_s) < STATION_ZONE)
        } else {
            None
        };
        if let Some(k) = self.at_station
            && was != Some(k)
        {
            self.events.push(Event::Arrive { station: k });
            self.board_timer = 0.4;
        }
    }

    fn stations_tick(&mut self, dt: f32) {
        let refill = 4.0 - (self.time / GAME_LEN) * 2.0;
        for st in &mut self.stations {
            st.timer += dt;
            if st.timer >= refill {
                st.timer = 0.0;
                st.waiting = (st.waiting + 1).min(STATION_MAX_WAITING);
            }
        }
        let Some(k) = self.at_station else { return };
        self.board_timer -= dt;
        if self.board_timer > 0.0 {
            return;
        }
        self.board_timer = 0.2;
        // Unload passengers who boarded elsewhere first: they pay on arrival.
        let unload = (0..self.cars.len())
            .find(|&i| self.cars[i].passengers.iter().any(|&o| o as usize != k));
        if let Some(i) = unload {
            let car = &mut self.cars[i];
            let p = car
                .passengers
                .iter()
                .position(|&o| o as usize != k)
                .unwrap_or(0);
            car.passengers.swap_remove(p);
            let amount = self.fare(i);
            self.money += amount;
            self.rescued += 1;
            self.events.push(Event::Fare {
                pos: self.car_pos(i),
                amount,
            });
            return;
        }
        if self.stations[k].waiting == 0 {
            return;
        }
        let seat = (0..self.cars.len()).find(|&i| self.cars[i].passengers.len() < self.seats(i));
        if let Some(i) = seat {
            self.stations[k].waiting -= 1;
            self.cars[i].passengers.push(k as u8);
            self.events.push(Event::Board {
                pos: self.car_pos(i),
            });
        }
    }

    fn conductor_tick(&mut self, dt: f32, input: &Input) {
        let last = (self.cars.len() - 1) as f32;
        if input.car_step != 0 {
            self.conductor = (self.conductor.round() + input.car_step as f32).clamp(0.0, last);
        }
        self.conductor = (self.conductor + input.walk.clamp(-1.0, 1.0) * 2.6 * dt).clamp(0.0, last);
        let i = self.conductor_car();
        let c = &mut self.cars[i];
        c.hp = (c.hp + 5.0 * self.stats.repair_mul * dt).min(c.max_hp);
    }

    fn repairs(&mut self, dt: f32) {
        let amount = 3.5 * self.stats.repair_mul * dt;
        for i in 0..self.cars.len() {
            if self.cars[i].kind != CarKind::Workshop {
                continue;
            }
            let lo = i.saturating_sub(1);
            let hi = (i + 1).min(self.cars.len() - 1);
            for c in &mut self.cars[lo..=hi] {
                c.hp = (c.hp + amount).min(c.max_hp);
            }
        }
    }

    fn nearest_enemy(&self, from: V2, range: f32) -> Option<V2> {
        let r2 = range * range;
        self.enemies
            .iter()
            .map(|e| (e.pos.dist2(from), e.pos))
            .filter(|(d, _)| *d < r2)
            .min_by(|a, b| a.0.total_cmp(&b.0))
            .map(|(_, p)| p)
    }

    fn guns(&mut self, dt: f32, input: &Input) {
        let manned_idx = self.conductor_car();
        for i in 0..self.cars.len() {
            let kind = self.cars[i].kind;
            let manned = i == manned_idx && input.fire;
            // The conductor carries a revolver when not standing at a gun.
            if !kind.is_gun() && !manned {
                continue;
            }
            let tender = if self.next_to(i, CarKind::Tender) {
                1.35
            } else {
                1.0
            };
            let c = &mut self.cars[i];
            c.cooldown -= dt * self.stats.rate_mul * tender;
            if c.cooldown > 0.0 {
                continue;
            }
            let (cd, dmg, speed, range, splash, heavy) = match kind {
                CarKind::Gun => (0.45, 10.0, 560.0, 300.0, 0.0, false),
                CarKind::Flak => (1.4, 16.0, 400.0, 360.0, 65.0, true),
                _ => (0.4, 7.0, 600.0, 280.0, 0.0, false),
            };
            let range = range * self.stats.range_mul;
            let from = if i == manned_idx {
                self.conductor_pos()
            } else {
                self.car_pos(i)
            };
            let target = if manned {
                Some(input.aim)
            } else {
                self.nearest_enemy(from, range)
            };
            let Some(target) = target else {
                continue;
            };
            let c = &mut self.cars[i];
            c.cooldown = if manned { cd * 0.45 } else { cd };
            let dir = (target - from).angle();
            c.aim = dir;
            let shots = if kind.is_gun() {
                1 + self.stats.multishot
            } else {
                1
            };
            for k in 0..shots {
                let spread = (k as f32 - (shots - 1) as f32 / 2.0) * 0.12;
                let a = dir + spread;
                self.bullets.push(Bullet {
                    pos: from,
                    vel: V2::from_angle(a) * speed,
                    dmg: dmg * self.stats.dmg_mul,
                    life: range / speed * 1.15,
                    splash,
                });
            }
            self.events.push(Event::Shot { heavy, pos: from });
        }
    }

    fn bullets_tick(&mut self, dt: f32) {
        let mut i = 0;
        while i < self.bullets.len() {
            let b = &mut self.bullets[i];
            b.pos += b.vel * dt;
            b.life -= dt;
            let (pos, dmg, splash) = (b.pos, b.dmg, b.splash);
            let expired = b.life <= 0.0;
            let hit = self
                .enemies
                .iter()
                .position(|e| e.pos.dist2(pos) < (e.radius + 4.0) * (e.radius + 4.0));
            if hit.is_some() || (expired && splash > 0.0) {
                if splash > 0.0 {
                    for e in &mut self.enemies {
                        if e.pos.dist(pos) < splash + e.radius {
                            e.hp -= dmg;
                            e.hurt = 0.0;
                        }
                    }
                    self.events.push(Event::Boom {
                        pos,
                        radius: splash,
                    });
                } else if let Some(h) = hit {
                    let e = &mut self.enemies[h];
                    e.hp -= dmg;
                    e.hurt = 0.0;
                    e.pos += b_push(self.bullets[i].vel, 6.0);
                    self.events.push(Event::Hit { pos });
                }
                self.bullets.swap_remove(i);
            } else if expired {
                self.bullets.swap_remove(i);
            } else {
                i += 1;
            }
        }
    }

    fn enemies_tick(&mut self, dt: f32) {
        let car_pos: Vec<V2> = (0..self.cars.len()).map(|i| self.car_pos(i)).collect();
        let engine = car_pos[0];
        let ram = self.speed > 40.0;
        let ram_dps = self.speed * 0.8 * self.stats.speed_mul;
        let mut car_dmg = vec![0.0f32; self.cars.len()];
        let centre = self.conductor_pos();
        for k in 0..self.enemies.len() {
            if self.enemies[k].pos.dist(centre) > LEASH
                && self.enemies[k].kind != EnemyKind::Juggernaut
            {
                let p = self.spawn_point();
                self.enemies[k].pos = p;
            }
        }
        for e in &mut self.enemies {
            e.hurt += dt;
            e.wobble += dt;
            let (ci, d) = car_pos
                .iter()
                .enumerate()
                .map(|(i, p)| (i, p.dist(e.pos)))
                .min_by(|a, b| a.1.total_cmp(&b.1))
                .unwrap_or((0, f32::MAX));
            let reach = e.radius + CAR_W * 0.6;
            e.biting = d < reach;
            if e.biting {
                car_dmg[ci] += e.dps * dt;
            } else {
                let dir = (car_pos[ci] - e.pos).norm();
                e.pos += dir * (e.speed * dt);
            }
            if ram && e.pos.dist(engine) < e.radius + CAR_LEN * 0.5 {
                e.hp -= ram_dps * dt;
                e.hurt = 0.0;
                car_dmg[0] += e.dps * dt;
            }
        }
        // Light separation so hordes spread around the train.
        let n = self.enemies.len();
        for a in 0..n {
            for b in (a + 1)..n {
                let (pa, pb) = (self.enemies[a].pos, self.enemies[b].pos);
                let min = self.enemies[a].radius + self.enemies[b].radius;
                let d2 = pa.dist2(pb);
                if d2 < min * min && d2 > 1e-4 {
                    let d = d2.sqrt();
                    let push = (pa - pb) * ((min - d) / d * 0.25);
                    self.enemies[a].pos += push;
                    self.enemies[b].pos += push * -1.0;
                }
            }
        }
        for (i, dmg) in car_dmg.into_iter().enumerate() {
            if dmg > 0.0 {
                let c = &mut self.cars[i];
                c.hp -= dmg;
                if c.hurt > 0.35 {
                    c.hurt = 0.0;
                    self.events.push(Event::CarHit { car: i });
                }
            }
        }
    }

    fn reap(&mut self) {
        let mut i = 0;
        while i < self.enemies.len() {
            if self.enemies[i].hp > 0.0 {
                i += 1;
                continue;
            }
            let e = self.enemies.swap_remove(i);
            self.kills += 1;
            self.xp += e.xp;
            let big = matches!(e.kind, EnemyKind::Brute | EnemyKind::Juggernaut);
            self.events.push(Event::Kill { pos: e.pos, big });
            let (p, value) = match e.kind {
                EnemyKind::Juggernaut => (1.0, 40),
                EnemyKind::Brute => (0.8, 4),
                _ => (0.12, 1 + self.rng.below(2) as u32),
            };
            if self.rng.chance(p) {
                self.coins.push(Coin {
                    pos: e.pos,
                    value,
                    age: 0.0,
                });
            }
        }
    }

    fn coins_tick(&mut self, dt: f32) {
        let car_pos: Vec<V2> = (0..self.cars.len()).map(|i| self.car_pos(i)).collect();
        let magnet = 70.0;
        let mut i = 0;
        while i < self.coins.len() {
            let c = &mut self.coins[i];
            c.age += dt;
            if let Some(p) = car_pos.iter().find(|p| p.dist(c.pos) < magnet) {
                c.pos += (*p - c.pos) * (dt * 8.0).min(1.0);
                if p.dist(c.pos) < 18.0 {
                    let c = self.coins.swap_remove(i);
                    self.money += c.value;
                    self.events.push(Event::Coin {
                        pos: c.pos,
                        value: c.value,
                    });
                    continue;
                }
            }
            if c.age > 30.0 {
                self.coins.swap_remove(i);
            } else {
                i += 1;
            }
        }
    }

    fn wrecks(&mut self) {
        if self.cars[0].hp <= 0.0 {
            self.phase = Phase::Lost;
            self.events.push(Event::Lost);
            return;
        }
        let mut i = 1;
        while i < self.cars.len() {
            if self.cars[i].hp > 0.0 {
                i += 1;
                continue;
            }
            let pos = self.car_pos(i);
            let car = self.cars.remove(i);
            self.lost_passengers += car.passengers.len() as u32;
            self.events.push(Event::CarLost { pos });
            if self.conductor > i as f32 - 0.5 {
                self.conductor = (self.conductor - 1.0).max(0.0);
            }
        }
        self.conductor = self.conductor.min((self.cars.len() - 1) as f32);
        self.trim_passengers();
    }

    /// Enemies spawned per second at time `t`.
    pub fn spawn_rate(t: f32) -> f32 {
        0.8 + t * 0.009 + (t / 400.0).powi(2)
    }

    fn spawn(&mut self, dt: f32) {
        let t = self.time;
        self.spawn_acc += Self::spawn_rate(t) * dt;
        while self.spawn_acc >= 1.0 {
            self.spawn_acc -= 1.0;
            let r = self.rng.f32();
            let kind = if t > 240.0 && r < 0.08 + t / GAME_LEN * 0.08 {
                EnemyKind::Brute
            } else if t > 75.0 && r < 0.45 {
                EnemyKind::Runner
            } else {
                EnemyKind::Walker
            };
            self.spawn_one(kind, None);
        }
        if t >= self.next_surge {
            self.next_surge += 60.0;
            let n = 14 + (t / 25.0) as usize;
            let base = self.rng.range(0.0, std::f32::consts::TAU);
            for k in 0..n {
                let a = base + k as f32 / n as f32 * std::f32::consts::TAU;
                let kind = if k % 3 == 0 && t > 120.0 {
                    EnemyKind::Runner
                } else {
                    EnemyKind::Walker
                };
                self.spawn_one(kind, Some(a));
            }
            self.events.push(Event::Surge);
        }
        if t >= self.next_boss {
            self.next_boss += 180.0;
            self.spawn_one(EnemyKind::Juggernaut, None);
            self.events.push(Event::BossSpawn);
        }
    }

    /// Most of the horde gathers along the line ahead of the train, so
    /// running at full steam means ploughing through it.
    fn spawn_point(&mut self) -> V2 {
        if self.rng.chance(0.6) {
            let ahead = self.rng.range(500.0, 1000.0) + self.speed * 2.0;
            let s = self.head_s + ahead;
            let radial = self.rng.range(-420.0, 420.0);
            track_pos(s) + V2::from_angle(s / crate::math::LOOP_R) * radial
        } else {
            let a = self.rng.range(0.0, std::f32::consts::TAU);
            self.conductor_pos() + V2::from_angle(a) * (SPAWN_DIST + self.rng.range(0.0, 80.0))
        }
    }

    fn spawn_one(&mut self, kind: EnemyKind, angle: Option<f32>) {
        if self.enemies.len() >= ENEMY_CAP && kind != EnemyKind::Juggernaut {
            // Full horde: move the furthest straggler back into play instead.
            let centre = self.conductor_pos();
            let far = (0..self.enemies.len())
                .filter(|&i| self.enemies[i].kind != EnemyKind::Juggernaut)
                .max_by(|&a, &b| {
                    let da = self.enemies[a].pos.dist2(centre);
                    da.total_cmp(&self.enemies[b].pos.dist2(centre))
                });
            if let Some(i) = far {
                self.enemies[i].pos = self.spawn_point();
            }
            return;
        }
        let pos = match angle {
            Some(a) => self.conductor_pos() + V2::from_angle(a) * SPAWN_DIST,
            None => self.spawn_point(),
        };
        let (hp, speed, radius, dps, xp) = kind.stats();
        let scale = 1.0 + self.time / 240.0 + (self.time / 500.0).powi(2);
        let hp = hp * scale;
        self.enemies.push(Enemy {
            kind,
            pos,
            hp,
            max_hp: hp,
            speed: speed * self.rng.range(0.9, 1.1),
            radius,
            dps: dps * (1.0 + self.time / 300.0),
            xp,
            hurt: 10.0,
            biting: false,
            wobble: self.rng.range(0.0, 6.0),
        });
    }
}

fn b_push(vel: V2, k: f32) -> V2 {
    vel.norm() * k
}

pub fn xp_for_level(level: u32) -> u32 {
    6 + level * 5 + level * level / 2
}

#[cfg(test)]
mod tests {
    use super::*;

    fn stopped_at_station() -> Game {
        let mut g = Game::new(3);
        g.head_s = g.stations[0].s;
        g.throttle = 0;
        g.step(0.01, &Input::default());
        assert_eq!(g.at_station, Some(0));
        g
    }

    #[test]
    fn starts_with_engine_guns_and_coach() {
        let g = Game::new(3);
        assert_eq!(g.cars[0].kind, CarKind::Engine);
        assert_eq!(g.capacity(), COACH_SEATS);
        assert_eq!(g.phase, Phase::Playing);
    }

    #[test]
    fn throttle_clamps_and_train_moves() {
        let mut g = Game::new(3);
        let start = g.head_s;
        for _ in 0..10 {
            g.step(
                0.1,
                &Input {
                    throttle: 1,
                    ..Input::default()
                },
            );
        }
        assert_eq!(g.throttle, 3);
        assert!(g.speed > 0.0);
        assert!(g.head_s != start);
        g.step(
            0.1,
            &Input {
                throttle: -9,
                ..Input::default()
            },
        );
        assert_eq!(g.throttle, 0);
    }

    #[test]
    fn buying_needs_station_and_money() {
        let mut g = Game::new(3);
        assert!(!g.buy(CarKind::Gun), "not at a station");
        let mut g = stopped_at_station();
        g.money = 10;
        assert!(!g.buy(CarKind::Gun), "too poor");
        g.money = 1000;
        let n = g.cars.len();
        let p1 = g.price(CarKind::Coach);
        assert!(g.buy(CarKind::Coach));
        assert_eq!(g.cars.len(), n + 1);
        assert!(g.price(CarKind::Coach) > p1, "prices rise");
        assert!(!g.buy(CarKind::Engine));
    }

    #[test]
    fn consist_is_capped() {
        let mut g = stopped_at_station();
        g.money = 100_000;
        while g.buy(CarKind::Coach) {}
        assert_eq!(g.cars.len(), MAX_CARS);
    }

    #[test]
    fn passengers_board_then_pay_at_another_station() {
        let mut g = stopped_at_station();
        for _ in 0..200 {
            g.step(0.05, &Input::default());
        }
        assert_eq!(g.aboard(), COACH_SEATS);
        let money = g.money;
        g.head_s = g.stations[1].s;
        g.step(0.01, &Input::default());
        assert_eq!(g.at_station, Some(1));
        for _ in 0..40 {
            g.step(0.05, &Input::default());
        }
        assert_eq!(g.rescued, COACH_SEATS as u32);
        assert!(g.money >= money + COACH_SEATS as u32 * BASE_FARE as u32);
    }

    #[test]
    fn infirmary_adds_seats_and_fares() {
        let mut g = Game::new(3);
        let coach = 2;
        let base = g.fare(coach);
        g.cars.insert(3, Car::new(CarKind::Infirmary, 1.0));
        assert_eq!(g.seats(coach), COACH_SEATS + 2);
        assert!(g.fare(coach) > base);
    }

    #[test]
    fn yard_repairs_for_money() {
        let mut g = stopped_at_station();
        g.cars[0].hp = 100.0;
        g.money = 1000;
        let cost = g.repair_cost();
        assert!(cost > 0);
        assert!(g.repair_all());
        assert_eq!(g.money, 1000 - cost);
        assert!(g.cars[0].hp >= g.cars[0].max_hp - 0.01);
    }

    #[test]
    fn shunt_moves_car_but_never_engine() {
        let mut g = stopped_at_station();
        g.conductor = 2.0;
        assert!(g.shunt(1));
        assert_eq!(g.cars[3].kind, CarKind::Coach);
        assert_eq!(g.conductor_car(), 3);
        g.conductor = 1.0;
        assert!(!g.shunt(-1));
        assert_eq!(g.cars[0].kind, CarKind::Engine);
    }

    #[test]
    fn destroyed_car_is_uncoupled_with_its_passengers() {
        let mut g = Game::new(3);
        g.cars[2].passengers = vec![0, 0, 0];
        g.cars[2].hp = 0.0;
        g.step(0.01, &Input::default());
        assert_eq!(g.cars.len(), 3);
        assert_eq!(g.lost_passengers, 3);
        assert!(g.events.iter().any(|e| matches!(e, Event::CarLost { .. })));
    }

    #[test]
    fn losing_engine_ends_the_game() {
        let mut g = Game::new(3);
        g.cars[0].hp = -1.0;
        g.step(0.01, &Input::default());
        assert_eq!(g.phase, Phase::Lost);
    }

    #[test]
    fn surviving_fifteen_minutes_wins() {
        let mut g = Game::new(3);
        g.time = GAME_LEN - 0.01;
        g.step(0.05, &Input::default());
        assert_eq!(g.phase, Phase::Won);
    }

    #[test]
    fn guns_kill_enemies_and_grant_xp() {
        let mut g = Game::new(3);
        g.throttle = 0;
        g.speed = 0.0;
        g.spawn_one(EnemyKind::Walker, None);
        g.enemies[0].pos = g.car_pos(1) + V2::new(120.0, 0.0);
        for _ in 0..100 {
            g.step(0.05, &Input::default());
            if g.kills > 0 {
                break;
            }
        }
        assert!(g.kills >= 1);
        assert!(g.xp >= 1 || g.level > 1);
    }

    #[test]
    fn manned_gun_fires_at_aim_point() {
        let mut g = Game::new(3);
        g.conductor = 1.0;
        g.cars[1].cooldown = 0.0;
        let aim = g.car_pos(1) + V2::new(0.0, -100.0);
        g.step(
            0.01,
            &Input {
                fire: true,
                aim,
                ..Input::default()
            },
        );
        let b = &g.bullets[0];
        assert!(b.vel.y < 0.0 && b.vel.x.abs() < 1.0);
    }

    #[test]
    fn level_up_pauses_and_choice_applies() {
        let mut g = Game::new(3);
        g.xp = g.xp_next;
        g.step(0.01, &Input::default());
        assert_eq!(g.phase, Phase::LevelUp);
        assert_eq!(g.choices.len(), 3);
        let t = g.time;
        g.step(1.0, &Input::default());
        assert_eq!(g.time, t, "paused while choosing");
        g.choose(0);
        assert_eq!(g.phase, Phase::Playing);
        assert_eq!(g.level, 2);
    }

    #[test]
    fn waves_escalate() {
        assert!(Game::spawn_rate(600.0) > Game::spawn_rate(60.0) * 3.0);
        let mut g = Game::new(3);
        g.time = 179.9;
        g.next_surge = 1000.0;
        g.step(0.2, &Input::default());
        assert!(g.enemies.iter().any(|e| e.kind == EnemyKind::Juggernaut));
    }

    #[test]
    fn workshop_repairs_neighbours() {
        let mut g = Game::new(3);
        g.cars.push(Car::new(CarKind::Workshop, 1.0));
        g.conductor = 0.0;
        g.cars[3].hp = 50.0;
        g.step(1.0, &Input::default());
        assert!(g.cars[3].hp > 52.0);
    }

    #[test]
    fn coins_are_collected_by_passing_cars() {
        let mut g = Game::new(3);
        g.coins.push(Coin {
            pos: g.car_pos(2) + V2::new(5.0, 0.0),
            value: 3,
            age: 0.0,
        });
        let m = g.money;
        for _ in 0..20 {
            g.step(0.05, &Input::default());
        }
        assert_eq!(g.money, m + 3);
    }
}
