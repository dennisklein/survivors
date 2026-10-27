//! Static game data: cargo types, towns, enemy stats and upgrades.

/// Cargo carried by a supply line. Each one unlocks its own weapon family.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Cargo {
    Steel,
    Coal,
    Goods,
}

impl Cargo {
    pub const ALL: [Cargo; 3] = [Cargo::Steel, Cargo::Coal, Cargo::Goods];

    pub fn index(self) -> usize {
        self as usize
    }

    pub fn name(self) -> &'static str {
        match self {
            Cargo::Steel => "Steel",
            Cargo::Coal => "Coal",
            Cargo::Goods => "Goods",
        }
    }

    /// Name of the weapon family this cargo powers.
    pub fn weapon(self) -> &'static str {
        match self {
            Cargo::Steel => "Rivet Gun",
            Cargo::Coal => "Firebox Mortar",
            Cargo::Goods => "Parcel Orbit",
        }
    }

    pub fn weapon_blurb(self) -> &'static str {
        match self {
            Cargo::Steel => "Fires piercing rivets at the nearest foe",
            Cargo::Coal => "Lobs burning coal that explodes in an area",
            Cargo::Goods => "Crates circle you and batter anything close",
        }
    }
}

/// A town on the network map that can be connected with a supply line.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Town {
    pub name: &'static str,
    pub cargo: Cargo,
    /// Direction from the depot, in radians.
    pub angle: f32,
    /// Map distance (0..1) used only for drawing the network map.
    pub dist: f32,
    pub cost: u32,
}

const fn town(name: &'static str, cargo: Cargo, deg: f32, dist: f32, cost: u32) -> Town {
    Town {
        name,
        cargo,
        angle: deg * std::f32::consts::PI / 180.0,
        dist,
        cost,
    }
}

/// Index of the town whose line the player starts with.
pub const START_TOWN: usize = 0;

pub const TOWNS: [Town; 8] = [
    town("Ironbridge", Cargo::Steel, 200.0, 0.6, 0),
    town("Blackpit", Cargo::Coal, 330.0, 0.7, 60),
    town("Marketon", Cargo::Goods, 100.0, 0.55, 90),
    town("Smeltham", Cargo::Steel, 30.0, 0.85, 120),
    town("Cinderford", Cargo::Coal, 145.0, 0.9, 130),
    town("Wharfside", Cargo::Goods, 260.0, 0.8, 140),
    town("Forgeholm", Cargo::Steel, 300.0, 0.95, 170),
    town("Ashmoor", Cargo::Coal, 65.0, 0.75, 160),
];

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EnemyKind {
    Rustbug,
    Sprinter,
    Brute,
    Boss,
}

/// Base stats for an enemy kind before wave scaling.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct EnemyStats {
    pub hp: f32,
    pub speed: f32,
    pub radius: f32,
    /// Damage per hit on the player, and per second on the depot.
    pub damage: f32,
    pub xp: u32,
}

impl EnemyKind {
    pub fn stats(self) -> EnemyStats {
        match self {
            EnemyKind::Rustbug => EnemyStats {
                hp: 10.0,
                speed: 70.0,
                radius: 12.0,
                damage: 8.0,
                xp: 1,
            },
            EnemyKind::Sprinter => EnemyStats {
                hp: 6.0,
                speed: 135.0,
                radius: 9.0,
                damage: 6.0,
                xp: 1,
            },
            EnemyKind::Brute => EnemyStats {
                hp: 55.0,
                speed: 45.0,
                radius: 22.0,
                damage: 18.0,
                xp: 4,
            },
            EnemyKind::Boss => EnemyStats {
                hp: 900.0,
                speed: 48.0,
                radius: 42.0,
                damage: 30.0,
                xp: 40,
            },
        }
    }
}

/// A level-up choice.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Upgrade {
    /// Unlock or level up the weapon family of this cargo.
    Weapon(Cargo),
    Boots,
    Plating,
    Repair,
    Magnet,
    Overtime,
}

impl Upgrade {
    pub fn title(self) -> String {
        match self {
            Upgrade::Weapon(c) => c.weapon().to_owned(),
            Upgrade::Boots => "Hobnail Boots".to_owned(),
            Upgrade::Plating => "Boiler Plating".to_owned(),
            Upgrade::Repair => "Depot Repairs".to_owned(),
            Upgrade::Magnet => "Crane Magnet".to_owned(),
            Upgrade::Overtime => "Overtime".to_owned(),
        }
    }

    pub fn blurb(self) -> &'static str {
        match self {
            Upgrade::Weapon(c) => c.weapon_blurb(),
            Upgrade::Boots => "+12% move speed",
            Upgrade::Plating => "+20 max health, heal 20",
            Upgrade::Repair => "Restore 30% depot health",
            Upgrade::Magnet => "+35 pickup range",
            Upgrade::Overtime => "All weapons fire 10% faster",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn town_names_unique_and_start_is_free() {
        for (i, a) in TOWNS.iter().enumerate() {
            for b in &TOWNS[i + 1..] {
                assert_ne!(a.name, b.name);
            }
        }
        assert_eq!(TOWNS[START_TOWN].cost, 0);
    }

    #[test]
    fn every_cargo_has_a_town() {
        for c in Cargo::ALL {
            assert!(TOWNS.iter().any(|t| t.cargo == c));
        }
    }
}
