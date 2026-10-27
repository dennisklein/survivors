//! Cards: company technology and town upgrades. Every item has 5 levels.

use crate::rng::Pcg32;

pub const MAX_LEVEL: u8 = 5;
/// Money for the card that comes when every item is at the top level.
pub const SUBSIDY: f32 = 150.0;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    Company,
    Town,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Item {
    HeavyRails,
    BiggerWagons,
    TrackGang,
}

impl Item {
    pub const ALL: [Item; 3] = [Item::HeavyRails, Item::BiggerWagons, Item::TrackGang];

    pub fn name(self) -> &'static str {
        match self {
            Item::HeavyRails => "Heavy rails",
            Item::BiggerWagons => "Bigger wagons",
            Item::TrackGang => "Track gang",
        }
    }

    pub fn kind(self) -> Kind {
        Kind::Company
    }

    /// What the item does at `level`, or what the level adds.
    pub fn describe(self, _level: u8) -> &'static str {
        match self {
            Item::HeavyRails => "+10 % train speed.",
            Item::BiggerWagons => "+20 % wagon capacity.",
            Item::TrackGang => "-15 % track and repair cost.",
        }
    }
}

/// Levels of all items, indexed by `Item as usize`.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Levels([u8; Item::ALL.len()]);

impl Levels {
    pub fn level(&self, i: Item) -> u8 {
        self.0[i as usize]
    }

    pub fn raise(&mut self, i: Item) {
        let l = &mut self.0[i as usize];
        *l = (*l + 1).min(MAX_LEVEL);
    }

    fn factor(&self, i: Item, per: f32) -> f32 {
        1.0 + per * self.level(i) as f32
    }

    pub fn speed_bonus(&self) -> f32 {
        self.factor(Item::HeavyRails, 0.1)
    }

    pub fn capacity_bonus(&self) -> f32 {
        self.factor(Item::BiggerWagons, 0.2)
    }

    /// Factor for the cost of track and repairs.
    pub fn build_factor(&self) -> f32 {
        self.factor(Item::TrackGang, -0.15)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Card {
    Item(Item),
    /// Money, when every item is at the top level.
    Subsidy,
}

impl Card {
    /// Title and text for the card.
    pub fn describe(&self, levels: &Levels) -> (String, &'static str) {
        match *self {
            Card::Item(i) => {
                let next = levels.level(i) + 1;
                let title = if next == 1 {
                    format!("New: {}", i.name())
                } else {
                    format!("{} Lv {next}", i.name())
                };
                (title, i.describe(next))
            }
            Card::Subsidy => ("Subsidy".to_owned(), "+$150."),
        }
    }
}

/// XP needed to go from `level` to the next level. The first level is 1.
pub fn xp_needed(level: u32) -> u32 {
    15 + 10 * level
}

/// Every item that can still go up a level.
pub fn pool(levels: &Levels) -> Vec<Card> {
    Item::ALL
        .into_iter()
        .filter(|&i| levels.level(i) < MAX_LEVEL)
        .map(Card::Item)
        .collect()
}

/// Three different random cards from the pool. Subsidy fills empty places.
pub fn roll(levels: &Levels, rng: &mut Pcg32) -> [Card; 3] {
    let mut pool = pool(levels);
    let mut out = [Card::Subsidy; 3];
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
        assert_eq!(xp_needed(1), 25);
        assert_eq!(xp_needed(10), 115);
    }

    #[test]
    fn cards_are_distinct() {
        let mut rng = Pcg32::new(4);
        for _ in 0..100 {
            let c = roll(&Levels::default(), &mut rng);
            assert!(c[0] != c[1] && c[1] != c[2] && c[0] != c[2]);
        }
    }

    #[test]
    fn maxed_items_leave_the_pool_and_subsidy_fills_up() {
        let mut l = Levels::default();
        for i in Item::ALL.into_iter().skip(1) {
            for _ in 0..7 {
                l.raise(i);
            }
            assert_eq!(l.level(i), MAX_LEVEL);
        }
        assert_eq!(pool(&l), vec![Card::Item(Item::ALL[0])]);
        let c = roll(&l, &mut Pcg32::new(1));
        assert_eq!(c, [Card::Item(Item::ALL[0]), Card::Subsidy, Card::Subsidy]);
    }

    #[test]
    fn item_effects() {
        let mut l = Levels::default();
        l.raise(Item::HeavyRails);
        l.raise(Item::TrackGang);
        l.raise(Item::TrackGang);
        assert!((l.speed_bonus() - 1.1).abs() < 1e-6);
        assert!((l.build_factor() - 0.7).abs() < 1e-6);
        assert_eq!(l.capacity_bonus(), 1.0);
    }
}
