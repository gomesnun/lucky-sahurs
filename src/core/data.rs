//! Static game tables: rarities, pets, mutations, upgrades, traits, milestones, rebirth rewards,
//! daily missions and the balance constants (post-BALANCE scaling, exactly as the Python computes them).

use crate::gfx::Color;
use crate::i18n::{LStr, l};
use crate::theme::{BLACK, DIAMOND_BORDER, GOLD_BORDER, WHITE};
use std::sync::OnceLock;

// ---------------------------------------------------------------- balance
pub const LUCK_PER_LEVEL: f64 = 0.08;
pub const LUCK_PRISM_PER_LEVEL: f64 = 0.07;
pub const LUCK_COSMIC_PER_LEVEL: f64 = 0.16;
pub const LUCK_DIVINE_PER_LEVEL: f64 = 0.25;
pub const MONEY_PER_LEVEL: f64 = 0.07;
pub const MONEY_PRISM_PER_LEVEL: f64 = 0.16;
pub const INCOME_SCALE: f64 = 0.15;
pub const AUTO_UNLOCK_REBIRTHS: i64 = 2;
pub const AUTO_BASE_RPS: f64 = 1.0;
pub const AUTO_SPEED_PER_LEVEL: f64 = 0.22;
pub const AUTO_TURBO_PER_LEVEL: f64 = 0.28;
pub const GOLDEN_ROLL_EVERY: i64 = 14;
pub const GOLDEN_ROLL_MIN: i64 = 10;
pub const GOLDEN_ROLL_MULT: f64 = 4.0;
pub const TRAIT_CHARGE_ONE_IN: f64 = 250.0;
pub const REBIRTH_BASE_COST: f64 = 500000.0;
pub const REBIRTH_COST_MULT: f64 = 3.2;
pub const REBIRTH_MONEY_PER: f64 = 0.07;
pub const REBIRTH_LUCK_PER: f64 = 0.04;
pub const MAX_MANUAL_CPS: f64 = 20.0;

// ---------------------------------------------------------------- rarities / pets
pub struct Tier {
    pub key: &'static str,
    pub name: &'static str,
    pub color: Color,
    pub color2: Option<Color>,
    pub text: Color,
    pub one_in: f64,
    pub income: f64,
}

pub const RARITY_TIERS: [Tier; 11] = [
    Tier { key: "comum", name: "Common", color: Color::rgb(235, 235, 235), color2: None, text: BLACK, one_in: 2.0, income: 1.0 },
    Tier { key: "incomum", name: "Uncommon", color: Color::rgb(60, 190, 90), color2: None, text: BLACK, one_in: 4.0, income: 4.0 },
    Tier { key: "raro", name: "Rare", color: Color::rgb(45, 120, 230), color2: None, text: WHITE, one_in: 15.0, income: 15.0 },
    Tier { key: "epico", name: "Epic", color: Color::rgb(150, 60, 220), color2: None, text: WHITE, one_in: 80.0, income: 60.0 },
    Tier { key: "lendario", name: "Legendary", color: Color::rgb(230, 195, 35), color2: None, text: BLACK, one_in: 800.0, income: 300.0 },
    Tier { key: "mitico", name: "Mythic", color: Color::rgb(220, 45, 45), color2: None, text: WHITE, one_in: 10000.0, income: 1800.0 },
    Tier { key: "exotico", name: "Exotic", color: Color::rgb(230, 140, 30), color2: Some(Color::rgb(60, 190, 90)), text: WHITE, one_in: 150000.0, income: 12000.0 },
    Tier { key: "secreto", name: "Secret", color: Color::rgb(14, 14, 16), color2: Some(Color::rgb(245, 245, 245)), text: WHITE, one_in: 2500000.0, income: 90000.0 },
    Tier { key: "divino", name: "Divine", color: Color::rgb(240, 145, 190), color2: Some(Color::rgb(255, 255, 255)), text: Color::rgb(80, 18, 50), one_in: 50000000.0, income: 750000.0 },
    Tier { key: "cosmico", name: "Cosmic", color: Color::rgb(96, 58, 224), color2: Some(Color::rgb(14, 8, 48)), text: WHITE, one_in: 3000000000.0, income: 8000000.0 },
    Tier { key: "transcendente", name: "Transcendent", color: Color::rgb(255, 226, 120), color2: Some(Color::rgb(120, 235, 255)), text: BLACK, one_in: 500000000000.0, income: 90000000.0 },
];

pub const PET_DEFS: [(&str, &str); 22] = [
    ("comum", "Levity"),
    ("incomum", "Humility"),
    ("raro", "Amity"),
    ("epico", "Fidelity"),
    ("lendario", "Audacity"),
    ("mitico", "Nobility"),
    ("exotico", "Serendipity"),
    ("secreto", "Obscurity"),
    ("divino", "Divinity"),
    ("comum", "Gravity"),
    ("incomum", "Vanity"),
    ("raro", "Hostility"),
    ("epico", "Duplicity"),
    ("lendario", "Ferocity"),
    ("mitico", "Immortality"),
    ("exotico", "Ambiguity"),
    ("secreto", "Anonymity"),
    ("divino", "Sanctity"),
    ("cosmico", "Infinity"),
    ("transcendente", "Eternity"),
    ("cosmico", "Singularity"),
    ("transcendente", "Omnity"),
];

pub const TIER_SECRET: usize = 7;
pub const TIER_DIVINE: usize = 8;
pub const TIER_COSMIC: usize = 9;
pub const TIER_TRANSCENDENT: usize = 10;

pub const SECOND_PET_SHARE: f64 = 1.0 / 3.0;
pub const SECOND_PET_INCOME_MULT: f64 = 1.5;

/// One entry per pet (index = what saves store).
pub struct Rarity {
    pub key: &'static str,
    pub name: &'static str,
    pub color: Color,
    pub color2: Option<Color>,
    pub text: Color,
    pub one_in: f64,
    pub income: f64,
    pub pet: &'static str,
    pub tier: usize,
    pub n_pets: usize,
    pub share: f64,
}

pub fn tier_index(key: &str) -> usize {
    RARITY_TIERS.iter().position(|t| t.key == key).unwrap_or(0)
}

pub fn rarities() -> &'static [Rarity] {
    static R: OnceLock<Vec<Rarity>> = OnceLock::new();
    R.get_or_init(|| {
        let mut out: Vec<Rarity> = PET_DEFS
            .iter()
            .map(|(key, pet)| {
                let ti = tier_index(key);
                let t = &RARITY_TIERS[ti];
                Rarity {
                    key: t.key,
                    name: t.name,
                    color: t.color,
                    color2: t.color2,
                    text: t.text,
                    one_in: t.one_in,
                    income: t.income,
                    pet,
                    tier: ti,
                    n_pets: 0,
                    share: 0.0,
                }
            })
            .collect();
        let tiers: Vec<usize> = out.iter().map(|r| r.tier).collect();
        for i in 0..out.len() {
            let same: Vec<usize> = (0..tiers.len()).filter(|&j| tiers[j] == tiers[i]).collect();
            out[i].n_pets = same.len();
            if same.len() == 2 {
                out[i].share = if i == same[1] { SECOND_PET_SHARE } else { 1.0 - SECOND_PET_SHARE };
                if i == same[1] {
                    out[i].income *= SECOND_PET_INCOME_MULT;
                }
            } else {
                out[i].share = 1.0 / same.len() as f64;
            }
        }
        out
    })
}

pub const N_PETS: usize = 22;

/// PET_ORDER: display order (by tier, then index)
pub fn pet_order() -> &'static [usize] {
    static R: OnceLock<Vec<usize>> = OnceLock::new();
    R.get_or_init(|| {
        let r = rarities();
        let mut v: Vec<usize> = (0..r.len()).collect();
        v.sort_by_key(|&i| (r[i].tier, i));
        v
    })
}

pub fn tier_first_pet(tier: usize) -> usize {
    rarities().iter().position(|r| r.tier == tier).unwrap_or(0)
}

// ---------------------------------------------------------------- mutations
pub struct Mutation {
    pub key: &'static str,
    pub label: &'static str,
    pub mult: f64,
    pub border: Option<Color>,
}

pub const MUTATIONS: [Mutation; 3] = [
    Mutation { key: "normal", label: "", mult: 1.0, border: None },
    Mutation { key: "golden", label: "Golden", mult: 3.0, border: Some(GOLD_BORDER) },
    Mutation { key: "diamond", label: "Diamond", mult: 9.0, border: Some(DIAMOND_BORDER) },
];
pub const MUT_ORDER: [&str; 3] = ["normal", "golden", "diamond"];
pub const INDEX_ENTRIES: usize = N_PETS * 3;

pub fn mutation(key: &str) -> Option<&'static Mutation> {
    MUTATIONS.iter().find(|m| m.key == key)
}
pub fn is_mutation(key: &str) -> bool {
    mutation(key).is_some()
}
/// Canonical &'static str for a mutation key (defaults to "normal").
pub fn mut_key(key: &str) -> &'static str {
    mutation(key).map(|m| m.key).unwrap_or("normal")
}

pub const GOLDEN_MAX_CHANCE: f64 = 0.25;
pub const DIAMOND_MAX_CHANCE: f64 = 0.09;
pub const GOLDEN_BASE: f64 = 0.02;
pub const GOLDEN_STEP_1: f64 = 0.0025;
pub const GOLDEN_STEP_2: f64 = 0.002;
pub const DIAMOND_BASE: f64 = 0.0025;
pub const DIAMOND_STEP_1: f64 = 0.0005;
pub const DIAMOND_STEP_2: f64 = 0.0015;

pub fn base_pet_chance(rarity_index: usize, mutation: &str) -> f64 {
    let r = rarities();
    let weights: Vec<f64> = r.iter().map(|x| x.share / x.one_in).collect();
    let total: f64 = weights.iter().sum();
    let rarity_chance = if total != 0.0 { weights[rarity_index] / total } else { 0.0 };
    let factor = match mutation {
        "diamond" => DIAMOND_BASE,
        "golden" => (1.0 - DIAMOND_BASE) * GOLDEN_BASE,
        _ => (1.0 - DIAMOND_BASE) * (1.0 - GOLDEN_BASE),
    };
    rarity_chance * factor
}

pub fn cap_chance(raw: f64, cap: f64) -> f64 {
    0f64.max(cap.min(raw))
}

// ---------------------------------------------------------------- upgrades
pub const OFFLINE_BASE_RATE: f64 = 0.30;
pub const OFFLINE_RATE_STEP: f64 = 0.02;
pub const OFFLINE_BASE_HOURS: f64 = 8.0;
pub const OFFLINE_TIME_STEP: f64 = 1.0;
pub const OFFLINE_MAX_HOURS: f64 = 48.0;

pub struct UpgradeDef {
    pub key: &'static str,
    pub name: &'static str,
    pub desc: LStr,
    pub max_level: i64,
    pub base_cost: f64,
    /// already multiplied by UPGRADE_COST_GROWTH and rounded to 4 places
    pub cost_mult: f64,
    pub requires: Option<&'static str>,
    pub requires_level: i64,
    pub requires_rebirths: i64,
}

macro_rules! a {
    ($($e:expr),*) => { crate::args![$($e),*] };
}

pub fn upgrade_defs() -> &'static [UpgradeDef] {
    static R: OnceLock<Vec<UpgradeDef>> = OnceLock::new();
    R.get_or_init(|| {
        let p = LStr::plain;
        let u = |key, name, desc, max_level, base_cost: f64, cost_mult: f64, requires, requires_level, requires_rebirths| UpgradeDef {
            key,
            name,
            desc,
            max_level,
            base_cost,
            cost_mult,
            requires,
            requires_level,
            requires_rebirths,
        };
        vec![
            u("luck", "Luck", l("+%g%% weight to all Rare pets or better.", a![LUCK_PER_LEVEL * 100.0]), 40, 50.0, 1.581, None, 1, 0),
            u("luck_prism", "Prismatic Luck", l("Stacks: +%g%% to Epic+, again to Mythic+ and again to Secret+.", a![LUCK_PRISM_PER_LEVEL * 100.0]), 25, 60000.0, 1.632, Some("luck"), 10, 0),
            u("luck_cosmic", "Exotic Luck", l("+%g%% weight to Exotic or better, per level.", a![LUCK_COSMIC_PER_LEVEL * 100.0]), 20, 2000000000.0, 1.836, Some("luck_prism"), 15, 0),
            u("luck_divine", "Divine Luck", l("+%g%% weight to Divine or better, per level.", a![LUCK_DIVINE_PER_LEVEL * 100.0]), 15, 100000000000.0, 2.04, Some("luck_cosmic"), 10, 0),
            u("money", "Money", l("+%g%% money/sec per level.", a![MONEY_PER_LEVEL * 100.0]), 70, 40.0, 1.53, None, 1, 0),
            u("money_prism", "Superior Money", l("+%g%% money/sec per level (multiplies with regular Money).", a![MONEY_PRISM_PER_LEVEL * 100.0]), 40, 20000000000.0, 1.632, Some("money"), 35, 0),
            u("slots", "Equip Slots", p("+1 slot to equip pets."), 20, 300.0, 2.448, None, 1, 0),
            u("slots_plus", "Extra Slots", p("+1 slot to equip pets, once your Slots are maxed out."), 15, 20000000000.0, 2.652, Some("slots"), 20, 0),
            u("auto_unlock", "Auto Roller", l("Unlocks the Auto Roller (needs Rebirth %d): rolls by itself while turned on.", a![AUTO_UNLOCK_REBIRTHS]), 1, 120000.0, 1.0, None, 1, AUTO_UNLOCK_REBIRTHS),
            u("auto_speed", "Auto Speed", l("+%g%% Auto Roller speed per level.", a![AUTO_SPEED_PER_LEVEL * 100.0]), 40, 25000.0, 1.4484, Some("auto_unlock"), 1, 0),
            u("auto_turbo", "Auto Turbo", l("+%g%% Auto Roller speed per level (multiplies).", a![AUTO_TURBO_PER_LEVEL * 100.0]), 25, 50000000000.0, 1.53, Some("auto_speed"), 20, 0),
            u("golden_unlock", "Unlock Golden", p("Allows Golden mutations to appear on rolls (x3 money)."), 1, 2000.0, 1.0, None, 1, 0),
            u("golden_chance", "Golden Luck", l("+%g%% chance of Golden mutation per level.", a![GOLDEN_STEP_1 * 100.0]), 20, 800.0, 1.938, Some("golden_unlock"), 1, 0),
            u("golden_chance_2", "Golden Luck II", l("+%g%% chance of Golden mutation per level.", a![GOLDEN_STEP_2 * 100.0]), 15, 1500000000.0, 1.887, Some("golden_chance"), 20, 0),
            u("diamond_unlock", "Unlock Diamond", p("Allows Diamond mutations to appear on rolls (x9 money)."), 1, 25000.0, 1.0, Some("golden_unlock"), 1, 0),
            u("diamond_chance", "Diamond Luck", l("+%g%% chance of Diamond mutation per level.", a![DIAMOND_STEP_1 * 100.0]), 22, 10000.0, 2.04, Some("diamond_unlock"), 1, 0),
            u("diamond_chance_2", "Diamond Luck II", l("+%g%% chance of Diamond mutation per level.", a![DIAMOND_STEP_2 * 100.0]), 15, 50000000000.0, 1.938, Some("diamond_chance"), 22, 0),
            u("trait_charge_luck", "Trait Charge Luck", p("+6% extra (relative) chance of getting 1 trait charge per roll, per level."), 18, 15000.0, 1.734, None, 1, 0),
            u("trait_charge_luck_2", "Trait Charge Luck II", p("+10% extra (relative) chance of getting 1 trait charge per roll, per level."), 15, 1000000000.0, 1.887, Some("trait_charge_luck"), 18, 0),
            u("trait_rarity_luck", "Trait Luck", p("+9% weight to traits from Instinctive (the 3rd) upward when you roll a trait, per level."), 20, 20000.0, 1.836, None, 1, 0),
            u("trait_rarity_luck_2", "Trait Luck II", p("+15% weight to traits from Instinctive upward when you roll a trait, per level."), 15, 8000000000.0, 1.938, Some("trait_rarity_luck"), 20, 0),
            u("cyclic_every", "Short Cycle", l("-1 roll in the Golden Roll cycle per level (minimum %d rolls).", a![GOLDEN_ROLL_MIN]), 4, 50000000.0, 5.1, None, 1, 0),
            u("cyclic_power", "Strong Golden Roll", p("+1 to the Golden Roll multiplier per level."), 10, 1000000000.0, 2.448, Some("cyclic_every"), 3, 0),
            u("diamond_roll_unlock", "Unlock Diamond Roll", p("Unlocks the Diamond Roll cycle: every so many rolls, the next roll gets a massive luck boost."), 1, 20000000000.0, 1.0, Some("cyclic_power"), 5, 0),
            u("diamond_roll_every", "Diamond Short Cycle", p("-3 rolls in the Diamond Roll cycle per level (minimum 70 rolls)."), 10, 40000000000.0, 2.652, Some("diamond_roll_unlock"), 1, 0),
            u("diamond_roll_power", "Strong Diamond Roll", p("+5 to the Diamond Roll multiplier per level."), 10, 80000000000.0, 2.652, Some("diamond_roll_every"), 5, 0),
            u("rainbow_roll_unlock", "Unlock Rainbow Roll", p("Unlocks the Rainbow Roll cycle: every so many rolls, the next roll gets an insane luck boost."), 1, 500000000000.0, 1.0, Some("diamond_roll_power"), 5, 0),
            u("rainbow_roll_every", "Rainbow Short Cycle", p("-30 rolls in the Rainbow Roll cycle per level (minimum 700 rolls)."), 10, 1000000000000.0, 2.856, Some("rainbow_roll_unlock"), 1, 0),
            u("rainbow_roll_power", "Strong Rainbow Roll", p("+25 to the Rainbow Roll multiplier per level."), 10, 2000000000000.0, 2.856, Some("rainbow_roll_every"), 5, 0),
            u("offline_rate", "Offline Earnings", l("+%g%% offline earnings per level (you start at %g%% of your money/sec while the game is closed).", a![OFFLINE_RATE_STEP * 100.0, OFFLINE_BASE_RATE * 100.0]), 15, 8000.0, 1.632, None, 1, 0),
            u("offline_rate_2", "Offline Earnings II", l("+%g%% offline earnings per level, once Offline Earnings is maxed out.", a![OFFLINE_RATE_STEP * 100.0]), 5, 4000000000.0, 1.938, Some("offline_rate"), 15, 0),
            u("offline_time", "Offline Time", l("+%d hour of max offline time per level (you start at %d hours).", a![1i64, 8i64]), 12, 15000.0, 1.683, None, 1, 0),
            u("offline_time_2", "Offline Time II", l("+%d hour of max offline time per level, once Offline Time is maxed out.", a![1i64]), 4, 8000000000.0, 2.04, Some("offline_time"), 12, 0),
            u("auto_equip_unlock", "Auto Equip Best", p("Unlocks the Auto Equip Best toggle in the Bag: automatically keeps your highest-earning pets equipped as you roll."), 1, 750000.0, 1.0, None, 1, 0),
        ]
    })
}

pub fn upgrade_def(key: &str) -> &'static UpgradeDef {
    upgrade_defs().iter().find(|d| d.key == key).unwrap_or_else(|| panic!("unknown upgrade {key}"))
}
pub fn upgrade_index(key: &str) -> usize {
    upgrade_defs().iter().position(|d| d.key == key).unwrap_or_else(|| panic!("unknown upgrade {key}"))
}

pub struct UpgradeCategory {
    pub key: &'static str,
    pub label: &'static str,
    pub desc: &'static str,
    pub upgrades: &'static [&'static str],
}

pub const UPGRADE_CATEGORIES: [UpgradeCategory; 8] = [
    UpgradeCategory { key: "luck", label: "Luck", desc: "Rarer pets show up more often", upgrades: &["luck", "luck_prism", "luck_cosmic", "luck_divine"] },
    UpgradeCategory {
        key: "mutations",
        label: "Mutation Chance",
        desc: "Golden and Diamond pets",
        upgrades: &["golden_unlock", "golden_chance", "golden_chance_2", "diamond_unlock", "diamond_chance", "diamond_chance_2"],
    },
    UpgradeCategory { key: "money", label: "Money", desc: "Earn more money per second", upgrades: &["money", "money_prism"] },
    UpgradeCategory {
        key: "traits",
        label: "Traits",
        desc: "Trait charges and trait rarity",
        upgrades: &["trait_charge_luck", "trait_charge_luck_2", "trait_rarity_luck", "trait_rarity_luck_2"],
    },
    UpgradeCategory {
        key: "bonus_rolls",
        label: "Bonus Rolls",
        desc: "Golden, Diamond and Rainbow Roll",
        upgrades: &[
            "cyclic_every",
            "cyclic_power",
            "diamond_roll_unlock",
            "diamond_roll_every",
            "diamond_roll_power",
            "rainbow_roll_unlock",
            "rainbow_roll_every",
            "rainbow_roll_power",
        ],
    },
    UpgradeCategory { key: "auto", label: "Auto Roller", desc: "Rolls by itself, faster", upgrades: &["auto_unlock", "auto_speed", "auto_turbo"] },
    UpgradeCategory {
        key: "offline",
        label: "Offline",
        desc: "Earn more while the game is closed",
        upgrades: &["offline_rate", "offline_rate_2", "offline_time", "offline_time_2"],
    },
    UpgradeCategory { key: "misc", label: "Misc", desc: "Equip Slots and Auto Equip Best", upgrades: &["slots", "slots_plus", "auto_equip_unlock"] },
];

pub fn upgrade_category(key: &str) -> Option<&'static UpgradeCategory> {
    UPGRADE_CATEGORIES.iter().find(|c| c.key == key)
}

pub const BASE_SLOTS: i64 = 3;

pub const DIAMOND_ROLL_EVERY_BASE: i64 = 100;
pub const DIAMOND_ROLL_EVERY_MIN: i64 = 70;
pub const DIAMOND_ROLL_EVERY_STEP: i64 = 3;
pub const DIAMOND_ROLL_MULT_BASE: f64 = 50.0;
pub const DIAMOND_ROLL_MULT_STEP: f64 = 5.0;
pub const RAINBOW_ROLL_EVERY_BASE: i64 = 1000;
pub const RAINBOW_ROLL_EVERY_MIN: i64 = 700;
pub const RAINBOW_ROLL_EVERY_STEP: i64 = 30;
pub const RAINBOW_ROLL_MULT_BASE: f64 = 250.0;
pub const RAINBOW_ROLL_MULT_STEP: f64 = 25.0;

// ---------------------------------------------------------------- traits
pub struct Trait {
    pub name: &'static str,
    pub color: Color,
    pub text: Color,
    pub one_in: f64,
    /// (buff key, value) in the order Python's dict holds them (already scaled)
    pub buffs: &'static [(&'static str, f64)],
}

impl Trait {
    pub fn buff(&self, key: &str) -> f64 {
        self.buffs.iter().find(|(k, _)| *k == key).map(|(_, v)| *v).unwrap_or(0.0)
    }
}

pub const TRAITS: [Trait; 12] = [
    Trait { name: "Lucky", color: Color::rgb(110, 130, 145), text: WHITE, one_in: 3.0, buffs: &[("money", 0.03)] },
    Trait { name: "Sharp-Eyed", color: Color::rgb(60, 150, 195), text: WHITE, one_in: 8.0, buffs: &[("money", 0.05), ("luck", 0.04)] },
    Trait { name: "Instinctive", color: Color::rgb(55, 115, 225), text: WHITE, one_in: 20.0, buffs: &[("money", 0.07), ("luck", 0.06), ("charge_chance", 0.15)] },
    Trait {
        name: "Visionary",
        color: Color::rgb(110, 75, 220),
        text: WHITE,
        one_in: 60.0,
        buffs: &[("money", 0.1), ("luck", 0.08), ("charge_chance", 0.22), ("mutation", 0.02)],
    },
    Trait {
        name: "Prodigious",
        color: Color::rgb(165, 55, 210),
        text: WHITE,
        one_in: 200.0,
        buffs: &[("money", 0.13), ("luck", 0.11), ("charge_chance", 0.3), ("mutation", 0.03), ("auto_speed", 0.15)],
    },
    Trait {
        name: "Radiant",
        color: Color::rgb(220, 75, 160),
        text: WHITE,
        one_in: 800.0,
        buffs: &[("money", 0.18), ("luck", 0.14), ("charge_chance", 0.4), ("mutation", 0.04), ("auto_speed", 0.22)],
    },
    Trait {
        name: "Ascendant",
        color: Color::rgb(200, 60, 45),
        text: WHITE,
        one_in: 3000.0,
        buffs: &[("money", 0.24), ("luck", 0.18), ("charge_chance", 0.5), ("mutation", 0.06), ("auto_speed", 0.3), ("secret_luck", 0.25)],
    },
    Trait {
        name: "Stellar",
        color: Color::rgb(230, 150, 40),
        text: BLACK,
        one_in: 12000.0,
        buffs: &[("money", 0.33), ("luck", 0.23), ("charge_chance", 0.65), ("mutation", 0.08), ("auto_speed", 0.42), ("secret_luck", 0.45)],
    },
    Trait {
        name: "Absolute",
        color: Color::rgb(240, 205, 60),
        text: BLACK,
        one_in: 60000.0,
        buffs: &[("money", 0.45), ("luck", 0.3), ("charge_chance", 0.85), ("mutation", 0.12), ("auto_speed", 0.6), ("secret_luck", 0.7)],
    },
    Trait {
        name: "Luck Deity",
        color: Color::rgb(250, 250, 250),
        text: Color::rgb(90, 20, 55),
        one_in: 300000.0,
        buffs: &[("money", 0.6), ("luck", 0.42), ("charge_chance", 1.1), ("mutation", 0.16), ("auto_speed", 0.85), ("secret_luck", 1.2)],
    },
    Trait {
        name: "Immortal",
        color: Color::rgb(40, 215, 195),
        text: BLACK,
        one_in: 1500000.0,
        buffs: &[("money", 0.81), ("luck", 0.54), ("charge_chance", 1.4), ("mutation", 0.2), ("auto_speed", 1.1), ("secret_luck", 1.7)],
    },
    Trait {
        name: "Omnipotent",
        color: Color::rgb(22, 20, 34),
        text: Color::rgb(255, 214, 90),
        one_in: 8000000.0,
        buffs: &[("money", 1.05), ("luck", 0.66), ("charge_chance", 1.75), ("mutation", 0.25), ("auto_speed", 1.4), ("secret_luck", 2.3)],
    },
];

// ---------------------------------------------------------------- milestones
pub struct MilestoneCat {
    pub key: &'static str,
    pub label: &'static str,
    pub reward_type: &'static str,
    pub tiers: &'static [(f64, f64)],
}

pub fn milestone_reward_label(rt: &str) -> &'static str {
    match rt {
        "luck" => "Luck",
        "trait_luck" => "Trait Luck",
        "golden_luck" => "Golden Luck",
        "diamond_luck" => "Diamond Luck",
        "money" => "Money",
        "auto_speed" => "Auto Speed",
        "secret_luck" => "Secret Luck",
        "divine_luck" => "Divine Luck",
        "cosmic_luck" => "Cosmic Luck",
        "transcendent_luck" => "Transcendent Luck",
        "index_luck" => "Luck",
        _ => "",
    }
}

/// In MILESTONE_DEFS insertion order (which is also MILESTONE_CAT_ORDER).
pub const MILESTONES: [MilestoneCat; 11] = [
    MilestoneCat {
        key: "rolls",
        label: "Total Rolls",
        reward_type: "luck",
        tiers: &[
            (50.0, 0.01),
            (150.0, 0.02),
            (400.0, 0.02),
            (1000.0, 0.03),
            (2500.0, 0.04),
            (6000.0, 0.04),
            (15000.0, 0.05),
            (40000.0, 0.06),
            (100000.0, 0.07),
            (250000.0, 0.08),
            (600000.0, 0.1),
            (1500000.0, 0.11),
            (4000000.0, 0.12),
            (10000000.0, 0.14),
            (25000000.0, 0.16),
            (50000000.0, 0.18),
            (100000000.0, 0.21),
        ],
    },
    MilestoneCat {
        key: "traits_rolled",
        label: "Traits Rolled",
        reward_type: "trait_luck",
        tiers: &[
            (5.0, 0.03),
            (15.0, 0.05),
            (30.0, 0.07),
            (60.0, 0.1),
            (120.0, 0.13),
            (250.0, 0.17),
            (500.0, 0.22),
            (1000.0, 0.28),
            (2000.0, 0.35),
            (4000.0, 0.43),
            (8000.0, 0.52),
            (15000.0, 0.62),
            (30000.0, 0.73),
            (60000.0, 0.85),
            (120000.0, 1.0),
        ],
    },
    MilestoneCat {
        key: "golden_rolled",
        label: "Golden Pets Rolled",
        reward_type: "golden_luck",
        tiers: &[
            (5.0, 0.01),
            (15.0, 0.01),
            (35.0, 0.01),
            (75.0, 0.02),
            (150.0, 0.02),
            (300.0, 0.03),
            (600.0, 0.03),
            (1200.0, 0.04),
            (2500.0, 0.04),
            (5000.0, 0.05),
            (10000.0, 0.05),
            (20000.0, 0.06),
            (40000.0, 0.07),
            (80000.0, 0.08),
            (150000.0, 0.08),
        ],
    },
    MilestoneCat {
        key: "diamond_rolled",
        label: "Diamond Pets Rolled",
        reward_type: "diamond_luck",
        tiers: &[
            (2.0, 0.01),
            (6.0, 0.01),
            (15.0, 0.01),
            (35.0, 0.02),
            (75.0, 0.02),
            (150.0, 0.03),
            (300.0, 0.03),
            (600.0, 0.04),
            (1200.0, 0.04),
            (2500.0, 0.05),
            (5000.0, 0.05),
            (10000.0, 0.06),
            (20000.0, 0.07),
            (40000.0, 0.08),
            (80000.0, 0.08),
        ],
    },
    MilestoneCat {
        key: "secret_rolled",
        label: "Secret Pets Rolled",
        reward_type: "secret_luck",
        tiers: &[
            (1.0, 0.02),
            (3.0, 0.03),
            (5.0, 0.04),
            (10.0, 0.05),
            (25.0, 0.07),
            (50.0, 0.09),
            (100.0, 0.12),
            (250.0, 0.15),
            (500.0, 0.19),
            (1000.0, 0.24),
            (2500.0, 0.3),
            (5000.0, 0.38),
            (10000.0, 0.5),
        ],
    },
    MilestoneCat {
        key: "divine_rolled",
        label: "Divine Pets Rolled",
        reward_type: "divine_luck",
        tiers: &[
            (1.0, 0.03),
            (2.0, 0.04),
            (3.0, 0.05),
            (5.0, 0.06),
            (10.0, 0.08),
            (20.0, 0.1),
            (40.0, 0.12),
            (80.0, 0.14),
            (150.0, 0.17),
            (300.0, 0.2),
            (600.0, 0.24),
            (1200.0, 0.29),
            (2500.0, 0.35),
            (5000.0, 0.42),
        ],
    },
    MilestoneCat {
        key: "cosmic_rolled",
        label: "Cosmic Pets Rolled",
        reward_type: "cosmic_luck",
        tiers: &[
            (1.0, 0.04),
            (2.0, 0.05),
            (3.0, 0.06),
            (5.0, 0.08),
            (10.0, 0.1),
            (20.0, 0.12),
            (40.0, 0.15),
            (80.0, 0.18),
            (150.0, 0.22),
            (300.0, 0.26),
            (600.0, 0.3),
            (1200.0, 0.35),
        ],
    },
    MilestoneCat {
        key: "transcendent_rolled",
        label: "Transcendent Pets Rolled",
        reward_type: "transcendent_luck",
        tiers: &[
            (1.0, 0.05),
            (2.0, 0.06),
            (3.0, 0.08),
            (5.0, 0.1),
            (10.0, 0.12),
            (20.0, 0.15),
            (40.0, 0.19),
            (80.0, 0.24),
            (150.0, 0.3),
            (300.0, 0.4),
        ],
    },
    MilestoneCat {
        key: "indexed_pets",
        label: "Indexed Pets",
        reward_type: "index_luck",
        tiers: &[
            (2.0, 0.01),
            (4.0, 0.01),
            (7.0, 0.01),
            (10.0, 0.02),
            (14.0, 0.02),
            (19.0, 0.02),
            (25.0, 0.03),
            (32.0, 0.04),
            (40.0, 0.04),
            (49.0, 0.05),
            (57.0, 0.05),
            (66.0, 0.06),
        ],
    },
    MilestoneCat {
        key: "coins",
        label: "Total Coins Earned",
        reward_type: "money",
        tiers: &[
            (1000.0, 0.02),
            (5000.0, 0.02),
            (25000.0, 0.04),
            (100000.0, 0.05),
            (500000.0, 0.06),
            (2500000.0, 0.08),
            (10000000.0, 0.1),
            (50000000.0, 0.12),
            (250000000.0, 0.15),
            (1000000000.0, 0.18),
            (5000000000.0, 0.22),
            (25000000000.0, 0.27),
            (100000000000.0, 0.33),
            (500000000000.0, 0.42),
            (1000000000000.0, 0.54),
        ],
    },
    MilestoneCat {
        key: "playtime",
        label: "Playtime",
        reward_type: "auto_speed",
        tiers: &[
            (300.0, 0.03),
            (900.0, 0.05),
            (1800.0, 0.07),
            (3600.0, 0.09),
            (7200.0, 0.12),
            (14400.0, 0.15),
            (28800.0, 0.19),
            (57600.0, 0.24),
            (86400.0, 0.3),
            (172800.0, 0.38),
            (345600.0, 0.48),
            (604800.0, 0.6),
            (1209600.0, 0.75),
            (2592000.0, 1.0),
        ],
    },
];

pub fn milestone_cat(key: &str) -> Option<&'static MilestoneCat> {
    MILESTONES.iter().find(|m| m.key == key)
}

// ---------------------------------------------------------------- rebirth rewards (already scaled)
pub struct RebirthReward {
    pub need: i64,
    pub name: &'static str,
    /// (kind, value, value_is_int) in Python dict order
    pub rewards: &'static [(&'static str, f64, bool)],
}

pub const REBIRTH_REWARDS: [RebirthReward; 17] = [
    RebirthReward { need: 2, name: "Second Wind", rewards: &[("money", 0.06, false)] },
    RebirthReward { need: 4, name: "Lucky Start", rewards: &[("luck", 0.03, false)] },
    RebirthReward { need: 6, name: "Full House", rewards: &[("slots", 1.0, true)] },
    RebirthReward { need: 8, name: "Cash Flow", rewards: &[("money", 0.09, false)] },
    RebirthReward { need: 10, name: "Rebirth Master", rewards: &[("keep_upgrades", 1.0, true)] },
    RebirthReward { need: 12, name: "Shiny Hunter", rewards: &[("golden_luck", 0.05, false), ("diamond_luck", 0.05, false)] },
    RebirthReward { need: 14, name: "Night Owl", rewards: &[("offline_rate", 0.1, false)] },
    RebirthReward { need: 16, name: "Trait Collector", rewards: &[("charge_chance", 0.15, false)] },
    RebirthReward { need: 18, name: "Trait Whisperer", rewards: &[("trait_luck", 0.1, false)] },
    RebirthReward { need: 20, name: "Glitter Storm", rewards: &[("golden_luck", 0.05, false), ("diamond_luck", 0.05, false)] },
    RebirthReward { need: 23, name: "Extra Seat", rewards: &[("slots", 1.0, true)] },
    RebirthReward { need: 26, name: "Shiny Expert", rewards: &[("golden_luck", 0.05, false), ("diamond_luck", 0.05, false)] },
    RebirthReward { need: 29, name: "Secret Keeper", rewards: &[("secret_luck", 0.1, false)] },
    RebirthReward { need: 32, name: "Shiny Master", rewards: &[("golden_luck", 0.05, false), ("diamond_luck", 0.05, false)] },
    RebirthReward { need: 35, name: "Sleepless", rewards: &[("offline_time", 2.0, true), ("money", 0.09, false)] },
    RebirthReward { need: 38, name: "Turbo Roller", rewards: &[("auto_speed", 0.1, false), ("luck", 0.03, false)] },
    RebirthReward {
        need: 40,
        name: "Shiny Legend",
        rewards: &[("golden_luck", 0.05, false), ("diamond_luck", 0.05, false), ("money", 0.15, false)],
    },
];

pub fn rebirth_reward_label(kind: &str) -> &str {
    match kind {
        "money" => "Money",
        "luck" => "Luck",
        "secret_luck" => "Secret+ Luck",
        "auto_speed" => "Auto Speed",
        "trait_luck" => "Trait Luck",
        "charge_chance" => "Trait Charge Chance",
        "golden_luck" => "Golden Luck",
        "diamond_luck" => "Diamond Luck",
        "offline_rate" => "Offline Earnings",
        other => other,
    }
}

// ---------------------------------------------------------------- daily missions
pub struct MissionDef {
    pub mtype: &'static str,
    pub label: &'static str,
    pub targets: [i64; 5],
    pub reward: [i64; 5],
}

pub const MISSION_POOL: [MissionDef; 8] = [
    MissionDef { mtype: "rolls", label: "Roll %d pets", targets: [150, 300, 600, 1200, 2500], reward: [3, 4, 5, 6, 8] },
    MissionDef { mtype: "golden", label: "Get %d Golden pets", targets: [2, 4, 8, 15, 30], reward: [3, 4, 5, 6, 8] },
    MissionDef { mtype: "diamond", label: "Get %d Diamond pets", targets: [1, 2, 4, 8, 15], reward: [4, 5, 6, 8, 10] },
    MissionDef { mtype: "traits", label: "Roll %d traits", targets: [2, 4, 7, 12, 20], reward: [3, 4, 5, 6, 8] },
    MissionDef { mtype: "rarity_epico", label: "Get %d Epic pet(s)", targets: [1, 2, 3, 5, 8], reward: [3, 4, 5, 6, 7] },
    MissionDef { mtype: "rarity_lendario", label: "Get %d Legendary pet(s)", targets: [1, 1, 2, 3, 5], reward: [4, 5, 6, 7, 8] },
    MissionDef { mtype: "rarity_mitico", label: "Get %d Mythic pet(s)", targets: [1, 1, 1, 2, 3], reward: [6, 6, 7, 8, 10] },
    MissionDef { mtype: "rarity_secreto", label: "Get a Secret pet", targets: [1, 1, 1, 1, 1], reward: [10, 10, 10, 10, 10] },
];
pub const DAILY_MISSION_COUNT: usize = 3;

pub fn mission_def(mtype: &str) -> Option<&'static MissionDef> {
    MISSION_POOL.iter().find(|m| m.mtype == mtype)
}
