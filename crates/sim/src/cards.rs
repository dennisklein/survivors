//! Level-up cards: new wagons, wagon levels, engine upgrades and repair.

use crate::rng::Pcg32;
use crate::weapon::{MAX_LEVEL, Wagon, WagonKind};

pub const MAX_WAGONS: usize = 5;
pub const REPAIR_HP: f32 = 30.0;
pub const PICKUP_RADIUS: f32 = 1.5;

/// Engine upgrades: the passive items.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Upgrade {
    Boiler,
    HeavyRails,
    Armour,
    CargoHook,
}

impl Upgrade {
    pub const ALL: [Upgrade; 4] = [
        Upgrade::Boiler,
        Upgrade::HeavyRails,
        Upgrade::Armour,
        Upgrade::CargoHook,
    ];

    pub fn name(self) -> &'static str {
        match self {
            Upgrade::Boiler => "Boiler pressure",
            Upgrade::HeavyRails => "Heavy rails",
            Upgrade::Armour => "Armour plating",
            Upgrade::CargoHook => "Cargo hook",
        }
    }

    pub fn describe(self) -> &'static str {
        match self {
            Upgrade::Boiler => "-10 % weapon cooldown.",
            Upgrade::HeavyRails => "+10 % train speed.",
            Upgrade::Armour => "+20 max HP. Heals 20.",
            Upgrade::CargoHook => "+30 % pickup radius.",
        }
    }
}

/// Levels of the engine upgrades, indexed by `Upgrade as usize`.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Upgrades(pub [u8; 4]);

impl Upgrades {
    pub fn level(&self, u: Upgrade) -> u8 {
        self.0[u as usize]
    }

    pub fn cooldown_factor(&self) -> f32 {
        1.0 - 0.1 * self.level(Upgrade::Boiler) as f32
    }

    pub fn speed_bonus(&self) -> f32 {
        1.0 + 0.1 * self.level(Upgrade::HeavyRails) as f32
    }

    pub fn pickup_radius(&self) -> f32 {
        PICKUP_RADIUS * (1.0 + 0.3 * self.level(Upgrade::CargoHook) as f32)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Card {
    NewWagon(WagonKind),
    /// Level up the wagon in this slot.
    LevelWagon(usize),
    Upgrade(Upgrade),
    Repair,
}

impl Card {
    /// Title and text for the card.
    pub fn describe(&self, wagons: &[Wagon], ups: &Upgrades) -> (String, &'static str) {
        match *self {
            Card::NewWagon(k) => (format!("New: {}", k.name()), k.describe(1)),
            Card::LevelWagon(i) => {
                let w = &wagons[i];
                (
                    format!("{} Lv {}", w.kind.name(), w.level + 1),
                    w.kind.describe(w.level + 1),
                )
            }
            Card::Upgrade(u) => (
                format!("{} Lv {}", u.name(), ups.level(u) + 1),
                u.describe(),
            ),
            Card::Repair => ("Repair".to_owned(), "+30 HP."),
        }
    }
}

/// XP needed to go from `level` to the next level. The first level is 1.
pub fn xp_needed(level: u32) -> u32 {
    5 + 4 * (level - 1)
}

/// Every card that can come up now.
pub fn pool(wagons: &[Wagon], ups: &Upgrades) -> Vec<Card> {
    let mut cards = Vec::new();
    if wagons.len() < MAX_WAGONS {
        cards.extend(WagonKind::ALL.map(Card::NewWagon));
    }
    for (i, w) in wagons.iter().enumerate() {
        if w.level < MAX_LEVEL {
            cards.push(Card::LevelWagon(i));
        }
    }
    for u in Upgrade::ALL {
        if ups.level(u) < MAX_LEVEL {
            cards.push(Card::Upgrade(u));
        }
    }
    cards
}

/// Three different random cards from the pool. Repair fills empty places.
pub fn roll(wagons: &[Wagon], ups: &Upgrades, rng: &mut Pcg32) -> [Card; 3] {
    let mut pool = pool(wagons, ups);
    let mut out = [Card::Repair; 3];
    for slot in &mut out {
        if pool.is_empty() {
            break;
        }
        let i = rng.below(pool.len() as u32) as usize;
        *slot = pool.swap_remove(i);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn xp_curve() {
        assert_eq!(xp_needed(1), 5);
        assert_eq!(xp_needed(2), 9);
        assert_eq!(xp_needed(10), 41);
    }

    #[test]
    fn cards_are_distinct() {
        let wagons = vec![Wagon::new(WagonKind::Armoured)];
        let mut rng = Pcg32::new(4);
        for _ in 0..100 {
            let c = roll(&wagons, &Upgrades::default(), &mut rng);
            assert!(c[0] != c[1] && c[1] != c[2] && c[0] != c[2]);
        }
    }

    #[test]
    fn no_new_wagon_when_slots_are_full() {
        let wagons: Vec<Wagon> = (0..MAX_WAGONS)
            .map(|_| Wagon::new(WagonKind::Steam))
            .collect();
        let p = pool(&wagons, &Upgrades::default());
        assert!(!p.iter().any(|c| matches!(c, Card::NewWagon(_))));
        assert_eq!(p.len(), MAX_WAGONS + Upgrade::ALL.len());
    }

    #[test]
    fn no_maxed_items_and_repair_at_the_end() {
        let mut wagons: Vec<Wagon> = (0..MAX_WAGONS)
            .map(|_| Wagon::new(WagonKind::Hopper))
            .collect();
        for w in &mut wagons {
            w.level = MAX_LEVEL;
        }
        let mut ups = Upgrades([MAX_LEVEL; 4]);
        ups.0[Upgrade::Armour as usize] = 2;
        assert_eq!(pool(&wagons, &ups), vec![Card::Upgrade(Upgrade::Armour)]);
        let c = roll(&wagons, &ups, &mut Pcg32::new(1));
        assert_eq!(
            c,
            [Card::Upgrade(Upgrade::Armour), Card::Repair, Card::Repair]
        );
    }

    #[test]
    fn upgrade_effects() {
        let ups = Upgrades([2, 3, 0, 1]);
        assert!((ups.cooldown_factor() - 0.8).abs() < 1e-6);
        assert!((ups.speed_bonus() - 1.3).abs() < 1e-6);
        assert!((ups.pickup_radius() - 1.95).abs() < 1e-6);
    }
}
