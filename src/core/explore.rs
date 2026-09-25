//! v4.0 Explore: walk Steve around 3D worlds (the Overworld, the Nether, the End, End City, Emerald City) and
//! catch Verity Pets. Up to 3 can be equipped: they follow Steve around, dancing, and boost your money or your
//! luck. Walking and catching give XP; levels open the next worlds and new clothes for Steve.
//! This is the saved part (levels, pets, outfit); the world itself is game/explore.rs.

use crate::core::data::rarities;
use serde_json::{Value, json};

pub struct Dimension {
    pub key: &'static str,
    pub name: &'static str,
    /// the level it opens at
    pub level: i64,
    /// the rarity tiers of the pets living there (inclusive)
    pub tiers: (usize, usize),
    /// a pet's boost there, in % (its stars pick where in the range)
    pub boost: (f64, f64),
    /// XP multiplier
    pub xp: f64,
    /// how many pets roam there at once
    pub pets: usize,
}

pub const DIMENSIONS: [Dimension; 5] = [
    Dimension { key: "overworld", name: "Overworld", level: 1, tiers: (0, 5), boost: (3.0, 10.0), xp: 1.0, pets: 8 },
    Dimension { key: "nether", name: "Nether", level: 5, tiers: (3, 8), boost: (8.0, 25.0), xp: 1.5, pets: 8 },
    Dimension { key: "end", name: "The End", level: 10, tiers: (6, 11), boost: (20.0, 50.0), xp: 2.0, pets: 7 },
    Dimension { key: "end_city", name: "End City", level: 15, tiers: (9, 13), boost: (40.0, 90.0), xp: 2.6, pets: 7 },
    Dimension { key: "emerald_city", name: "Emerald City", level: 20, tiers: (12, 15), boost: (70.0, 150.0), xp: 3.2, pets: 6 },
];

/// The OG verities (tier 16) only live in Emerald City, and rarely.
pub const OG_TIER: usize = 16;
pub const OG_CHANCE: f64 = 0.03;
pub const MAX_PETS: usize = 60;
pub const MAX_EQUIPPED: usize = 3;
pub const MAX_LEVEL: i64 = 99;

/// Total XP needed to reach a level (level 1 = 0 XP).
pub fn xp_for_level(level: i64) -> f64 {
    let l = (level - 1).max(0) as f64;
    40.0 * l * l + 60.0 * l
}

pub fn level_of(xp: f64) -> i64 {
    let mut l = 1;
    while l < MAX_LEVEL && xp >= xp_for_level(l + 1) {
        l += 1;
    }
    l
}

/// Steve's clothes: (name, colour, level it unlocks at).
pub const SHIRTS: [(&str, u32, i64); 8] = [
    ("Classic", 0x2fb5b5, 1),
    ("Red", 0xc73a3a, 2),
    ("Forest", 0x3f8a3a, 3),
    ("Royal", 0x3a4fc7, 6),
    ("Nether", 0x7a1d1d, 8),
    ("Purpur", 0xa66fc0, 12),
    ("Gold", 0xe0b030, 16),
    ("Emerald", 0x19c060, 20),
];
pub const PANTS: [(&str, u32, i64); 6] = [("Jeans", 0x3b3a9c, 1), ("Black", 0x26262e, 2), ("Khaki", 0x9c8456, 4), ("White", 0xdedee6, 9), ("Obsidian", 0x2a1840, 13), ("Gold", 0xc89a20, 18)];
pub const HATS: [(&str, i64); 6] = [("None", 1), ("Cap", 3), ("Top hat", 7), ("Crown", 11), ("Halo", 15), ("Emerald crown", 20)];

/// A caught Verity Pet.
#[derive(Clone, Debug, PartialEq)]
pub struct VerityPet {
    /// the rarity index (rarities())
    pub pet: usize,
    /// where it was caught (DIMENSIONS index)
    pub dim: usize,
    /// 1-5
    pub stars: u8,
    /// true: boosts luck; false: money
    pub luck: bool,
    /// the boost, in %
    pub boost: f64,
}

impl VerityPet {
    /// A new pet for a dimension: the rarity, the stars and the boost from rolls r0..r3 (0-1).
    pub fn roll(dim: usize, r: [f64; 4]) -> VerityPet {
        let d = &DIMENSIONS[dim.min(DIMENSIONS.len() - 1)];
        let og = d.key == "emerald_city" && r[0] < OG_CHANCE;
        let tier = if og {
            OG_TIER
        } else {
            // the rarer tiers of a world are less common: weights 1, 1/2, 1/4...
            let n = d.tiers.1 - d.tiers.0 + 1;
            let total: f64 = (0..n).map(|i| 0.5f64.powi(i as i32)).sum();
            let mut x = ((r[0] - if d.key == "emerald_city" { OG_CHANCE } else { 0.0 }).max(0.0) / (1.0 - if d.key == "emerald_city" { OG_CHANCE } else { 0.0 })) * total;
            let mut t = d.tiers.0;
            for i in 0..n {
                let w = 0.5f64.powi(i as i32);
                if x < w {
                    t = d.tiers.0 + i;
                    break;
                }
                x -= w;
                t = d.tiers.0 + i;
            }
            t
        };
        let options: Vec<usize> = rarities().iter().enumerate().filter(|(_, r)| r.tier == tier).map(|(i, _)| i).collect();
        let pet = options.get(((r[1] * options.len() as f64) as usize).min(options.len().saturating_sub(1))).copied().unwrap_or(0);
        // stars: 1 (common) .. 5 (rare)
        let stars = match r[2] {
            x if x < 0.45 => 1,
            x if x < 0.72 => 2,
            x if x < 0.88 => 3,
            x if x < 0.97 => 4,
            _ => 5,
        };
        let k = (stars as f64 - 1.0) / 4.0;
        let mut boost = d.boost.0 + (d.boost.1 - d.boost.0) * k;
        // a little better the rarer it is within its world
        boost *= 1.0 + (tier.saturating_sub(d.tiers.0)) as f64 * 0.08;
        if og {
            boost *= 2.0;
        }
        VerityPet { pet, dim: dim.min(DIMENSIONS.len() - 1), stars, luck: r[3] < 0.5, boost: (boost * 10.0).round() / 10.0 }
    }

    pub fn to_value(&self) -> Value {
        json!([rarities()[self.pet].pet, self.dim, self.stars, if self.luck { "luck" } else { "money" }, self.boost])
    }

    pub fn from_value(v: &Value) -> Option<VerityPet> {
        let a = v.as_array()?;
        let name = a.first()?.as_str()?;
        let pet = rarities().iter().position(|r| r.pet == name)?;
        let dim = (a.get(1)?.as_i64()?.max(0) as usize).min(DIMENSIONS.len() - 1);
        let stars = a.get(2)?.as_i64()?.clamp(1, 5) as u8;
        let luck = a.get(3)?.as_str()? == "luck";
        // never more than the best a pet could have (a hand-edited save doesn't get x1000)
        let boost = a.get(4)?.as_f64()?.clamp(0.0, 800.0);
        Some(VerityPet { pet, dim, stars, luck, boost })
    }
}

#[derive(Clone, Debug)]
pub struct ExploreState {
    pub xp: f64,
    pub pets: Vec<VerityPet>,
    /// indexes into pets
    pub equipped: Vec<usize>,
    /// SHIRTS / PANTS / HATS indexes
    pub shirt: usize,
    pub pants: usize,
    pub hat: usize,
    /// the world you were last in
    pub dim: usize,
    pub caught: i64,
}

impl Default for ExploreState {
    fn default() -> Self {
        ExploreState { xp: 0.0, pets: Vec::new(), equipped: Vec::new(), shirt: 0, pants: 0, hat: 0, dim: 0, caught: 0 }
    }
}

impl ExploreState {
    pub fn level(&self) -> i64 {
        level_of(self.xp)
    }

    pub fn unlocked(&self, dim: usize) -> bool {
        DIMENSIONS.get(dim).is_some_and(|d| self.level() >= d.level)
    }

    /// Adds XP; returns the new level if it went up.
    pub fn add_xp(&mut self, xp: f64) -> Option<i64> {
        let before = self.level();
        self.xp = (self.xp + xp.max(0.0)).min(xp_for_level(MAX_LEVEL) + 1.0);
        let after = self.level();
        (after > before).then_some(after)
    }

    /// The equipped pets' boost to money (x) - they add up: +20% and +30% = x1.5.
    pub fn money_mult(&self) -> f64 {
        1.0 + self.equipped.iter().filter_map(|&i| self.pets.get(i)).filter(|p| !p.luck).map(|p| p.boost / 100.0).sum::<f64>()
    }

    pub fn luck_mult(&self) -> f64 {
        1.0 + self.equipped.iter().filter_map(|&i| self.pets.get(i)).filter(|p| p.luck).map(|p| p.boost / 100.0).sum::<f64>()
    }

    /// Equips or unequips a pet. False if it can't be equipped (all slots taken).
    pub fn toggle_equip(&mut self, i: usize) -> bool {
        if i >= self.pets.len() {
            return false;
        }
        if let Some(p) = self.equipped.iter().position(|&e| e == i) {
            self.equipped.remove(p);
            return true;
        }
        if self.equipped.len() >= MAX_EQUIPPED {
            return false;
        }
        self.equipped.push(i);
        true
    }

    /// Adds a caught pet. False if the pet bag is full.
    pub fn add_pet(&mut self, p: VerityPet) -> bool {
        if self.pets.len() >= MAX_PETS {
            return false;
        }
        self.pets.push(p);
        self.caught += 1;
        true
    }

    /// Lets a pet go (the equipped indexes after it move down by one).
    pub fn release(&mut self, i: usize) {
        if i >= self.pets.len() {
            return;
        }
        self.pets.remove(i);
        self.equipped.retain(|&e| e != i);
        for e in self.equipped.iter_mut() {
            if *e > i {
                *e -= 1;
            }
        }
    }

    /// Equips the 3 with the biggest boosts.
    pub fn equip_best(&mut self) {
        let mut idx: Vec<usize> = (0..self.pets.len()).collect();
        idx.sort_by(|a, b| self.pets[*b].boost.partial_cmp(&self.pets[*a].boost).unwrap_or(std::cmp::Ordering::Equal));
        self.equipped = idx.into_iter().take(MAX_EQUIPPED).collect();
    }

    pub fn is_empty(&self) -> bool {
        self.xp <= 0.0 && self.pets.is_empty() && self.shirt == 0 && self.pants == 0 && self.hat == 0
    }

    pub fn to_value(&self) -> Value {
        json!({
            "xp": (self.xp * 10.0).round() / 10.0,
            "pets": self.pets.iter().map(|p| p.to_value()).collect::<Vec<_>>(),
            "equipped": self.equipped,
            "outfit": [self.shirt, self.pants, self.hat],
            "dim": self.dim,
            "caught": self.caught,
        })
    }

    pub fn from_value(v: &Value) -> ExploreState {
        let mut s = ExploreState { xp: v.get("xp").and_then(|x| x.as_f64()).unwrap_or(0.0).clamp(0.0, xp_for_level(MAX_LEVEL) + 1.0), ..Default::default() };
        if let Some(a) = v.get("pets").and_then(|x| x.as_array()) {
            s.pets = a.iter().filter_map(VerityPet::from_value).take(MAX_PETS).collect();
        }
        if let Some(a) = v.get("equipped").and_then(|x| x.as_array()) {
            for e in a.iter().filter_map(|x| x.as_u64()) {
                let e = e as usize;
                if e < s.pets.len() && !s.equipped.contains(&e) && s.equipped.len() < MAX_EQUIPPED {
                    s.equipped.push(e);
                }
            }
        }
        let outfit: Vec<usize> = v.get("outfit").and_then(|x| x.as_array()).map(|a| a.iter().filter_map(|x| x.as_u64()).map(|x| x as usize).collect()).unwrap_or_default();
        let lvl = s.level();
        // clothes above your level (a hand-edited save) go back to the default
        s.shirt = outfit.first().copied().filter(|&i| i < SHIRTS.len() && SHIRTS[i].2 <= lvl).unwrap_or(0);
        s.pants = outfit.get(1).copied().filter(|&i| i < PANTS.len() && PANTS[i].2 <= lvl).unwrap_or(0);
        s.hat = outfit.get(2).copied().filter(|&i| i < HATS.len() && HATS[i].1 <= lvl).unwrap_or(0);
        s.dim = v.get("dim").and_then(|x| x.as_u64()).map(|x| x as usize).filter(|&d| d < DIMENSIONS.len() && s.unlocked(d)).unwrap_or(0);
        s.caught = v.get("caught").and_then(|x| x.as_i64()).unwrap_or(0).max(0);
        s
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn levels() {
        assert_eq!(level_of(0.0), 1);
        for l in 1..40 {
            assert_eq!(level_of(xp_for_level(l)), l);
            assert_eq!(level_of(xp_for_level(l + 1) - 0.01), l);
        }
    }

    #[test]
    fn rolls_stay_in_their_world() {
        for dim in 0..DIMENSIONS.len() {
            for i in 0..200 {
                let f = |k: i64| ((i * 7919 + k * 104729) % 1000) as f64 / 1000.0;
                let p = VerityPet::roll(dim, [f(1), f(2), f(3), f(4)]);
                let t = rarities()[p.pet].tier;
                let d = &DIMENSIONS[dim];
                assert!((t >= d.tiers.0 && t <= d.tiers.1) || (t == OG_TIER && d.key == "emerald_city"), "{} {}", dim, t);
                assert!(p.boost > 0.0 && (1..=5).contains(&p.stars));
            }
        }
        // the OG ones are there, rarely
        assert_eq!(rarities()[VerityPet::roll(4, [0.0, 0.0, 0.0, 0.0]).pet].tier, OG_TIER);
    }

    #[test]
    fn save_round_trip() {
        let mut s = ExploreState { xp: xp_for_level(12) + 5.0, shirt: 3, pants: 1, hat: 2, dim: 1, ..Default::default() };
        s.add_pet(VerityPet::roll(1, [0.3, 0.5, 0.9, 0.2]));
        s.add_pet(VerityPet::roll(0, [0.1, 0.1, 0.1, 0.8]));
        s.toggle_equip(1);
        let back = ExploreState::from_value(&s.to_value());
        assert_eq!(back.pets, s.pets);
        assert_eq!(back.equipped, vec![1]);
        assert_eq!((back.shirt, back.pants, back.hat, back.dim), (3, 1, 2, 1));
        assert!(back.money_mult() > 1.0 || back.luck_mult() > 1.0);
        let mut r = back.clone();
        r.release(0);
        assert_eq!(r.equipped, vec![0]);
    }
}
