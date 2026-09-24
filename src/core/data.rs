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
// v2.7: luck for the end game (the new rarities go up to 1 in 500 Qa)
pub const LUCK_2_PER_LEVEL: f64 = 0.10;
pub const LUCK_PRISM_2_PER_LEVEL: f64 = 0.06;
/// luck of each top rarity (for THAT rarity and the better ones), in Python dict order
pub const LUCK_TIER_PER_LEVEL: [(&str, f64); 5] =
    [("cosmico", 0.30), ("transcendente", 0.40), ("etereo", 0.50), ("celestial", 0.60), ("absoluto", 0.75)];
pub const LUCK_ULTRA_PER_LEVEL: f64 = 0.05;
/// v2.9: only this fraction of the "normal" luck bonus counts (the Shop's dice and potions make room)
/// v3.0: 0.85 -> 0.78, a small nerf now that Prestige multiplies Luck
pub const LUCK_BONUS_SCALE: f64 = 0.78;
pub const MONEY_PER_LEVEL: f64 = 0.07;
pub const MONEY_PRISM_PER_LEVEL: f64 = 0.16;
pub const INCOME_SCALE: f64 = 0.15;
/// v2.7: from half of each upgrade's levels on, the price grows slower (cost_mult ** UPGRADE_SOFT_EXP a level)
pub const UPGRADE_SOFT_START: f64 = 0.5;
pub const UPGRADE_SOFT_EXP: f64 = 0.6;
pub const MONEY_ULTRA_PER_LEVEL: f64 = 0.25;
pub const AUTO_UNLOCK_REBIRTHS: i64 = 2;
pub const AUTO_BASE_RPS: f64 = 1.0;
pub const AUTO_SPEED_PER_LEVEL: f64 = 0.22;
pub const AUTO_TURBO_PER_LEVEL: f64 = 0.28;
pub const GOLDEN_ROLL_EVERY: i64 = 14;
pub const GOLDEN_ROLL_MIN: i64 = 10;
pub const GOLDEN_ROLL_MULT: f64 = 4.0;
pub const TRAIT_CHARGE_ONE_IN: f64 = 250.0;
/// "Use All Charges": one by one up to here; above that by statistics (roll_traits_bulk)
pub const TRAIT_EXACT_MAX: i64 = 5000;
pub const REBIRTH_BASE_COST: f64 = 500000.0;
pub const REBIRTH_COST_MULT: f64 = 3.2;
pub const REBIRTH_MONEY_PER: f64 = 0.07;
pub const REBIRTH_LUCK_PER: f64 = 0.04;
pub const MAX_MANUAL_CPS: f64 = 20.0;
/// each pet sells for this many seconds of its money/sec (Bag > Inventory)
pub const PET_SELL_SECONDS: f64 = 10.0;

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

pub const RARITY_TIERS: [Tier; 16] = [
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
    Tier { key: "etereo", name: "Ethereal", color: Color::rgb(178, 226, 255), color2: Some(Color::rgb(238, 248, 255)), text: Color::rgb(20, 50, 90), one_in: 50000000000000.0, income: 900000000.0 },
    Tier { key: "celestial", name: "Celestial", color: Color::rgb(255, 236, 186), color2: Some(Color::rgb(255, 196, 70)), text: Color::rgb(80, 52, 6), one_in: 5000000000000000.0, income: 9000000000.0 },
    Tier { key: "absoluto", name: "Absolute", color: Color::rgb(18, 14, 10), color2: Some(Color::rgb(255, 198, 58)), text: WHITE, one_in: 5.0e+17, income: 90000000000.0 },
    Tier { key: "primordial", name: "Primordial", color: Color::rgb(26, 10, 8), color2: Some(Color::rgb(255, 112, 30)), text: WHITE, one_in: 5.0e+19, income: 900000000000.0 },
    Tier { key: "paradoxo", name: "Paradox", color: Color::rgb(246, 246, 250), color2: Some(Color::rgb(14, 14, 20)), text: Color::rgb(120, 60, 220), one_in: 5.0e+21, income: 9000000000000.0 },
];

pub const PET_DEFS: [(&str, &str); 64] = [
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
    ("comum", "Clarity"),
    ("comum", "Simplicity"),
    ("incomum", "Curiosity"),
    ("incomum", "Timidity"),
    ("raro", "Velocity"),
    ("raro", "Agility"),
    ("epico", "Toxicity"),
    ("epico", "Complexity"),
    ("lendario", "Tenacity"),
    ("lendario", "Prosperity"),
    ("mitico", "Authority"),
    ("mitico", "Celebrity"),
    ("exotico", "Hilarity"),
    ("exotico", "Tranquility"),
    ("secreto", "Opacity"),
    ("secreto", "Invisibility"),
    ("divino", "Purity"),
    ("divino", "Charity"),
    ("cosmico", "Relativity"),
    ("cosmico", "Polarity"),
    ("transcendente", "Unity"),
    ("transcendente", "Ubiquity"),
    ("etereo", "Serenity"),
    ("etereo", "Fragility"),
    ("etereo", "Spirituality"),
    ("etereo", "Lucidity"),
    ("celestial", "Felicity"),
    ("celestial", "Sublimity"),
    ("celestial", "Luminosity"),
    ("celestial", "Immensity"),
    ("absoluto", "Reality"),
    ("absoluto", "Finality"),
    ("absoluto", "Perpetuity"),
    ("absoluto", "Totality"),
    ("primordial", "Antiquity"),
    ("primordial", "Calamity"),
    ("primordial", "Intensity"),
    ("primordial", "Enormity"),
    ("paradoxo", "Duality"),
    ("paradoxo", "Absurdity"),
    ("paradoxo", "Parity"),
    ("paradoxo", "Impossibility"),
];

pub const TIER_SECRET: usize = 7;
pub const TIER_DIVINE: usize = 8;
pub const TIER_COSMIC: usize = 9;
pub const TIER_TRANSCENDENT: usize = 10;
pub const TIER_ETHEREAL: usize = 11;
pub const TIER_CELESTIAL: usize = 12;
pub const TIER_ABSOLUTE: usize = 13;

/// each pet of a rarity (in list order) rolls 2x LESS often than the one before: 8/15, 4/15, 2/15, 1/15
pub const PET_SHARE_RATIO: f64 = 0.5;
/// ...and earns 1.5x more than the one before (1x, 1.5x, 2.25x, 3.375x)
pub const NEXT_PET_INCOME_MULT: f64 = 1.5;

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
            let pos = same.iter().position(|&j| j == i).unwrap_or(0);
            out[i].n_pets = same.len();
            let norm: f64 = (0..same.len()).map(|k| PET_SHARE_RATIO.powi(k as i32)).sum();
            out[i].share = PET_SHARE_RATIO.powi(pos as i32) / norm;
            out[i].income *= NEXT_PET_INCOME_MULT.powi(pos as i32);
        }
        out
    })
}

pub const N_PETS: usize = 64;

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

/// the rainbow's "average" colour, for where only one colour fits (the real border is striped)
pub const RAINBOW_BORDER: Color = Color::rgb(255, 120, 210);

pub const MUTATIONS: [Mutation; 4] = [
    Mutation { key: "normal", label: "", mult: 1.0, border: None },
    Mutation { key: "golden", label: "Golden", mult: 3.0, border: Some(GOLD_BORDER) },
    Mutation { key: "diamond", label: "Diamond", mult: 9.0, border: Some(DIAMOND_BORDER) },
    Mutation { key: "rainbow", label: "Rainbow", mult: 27.0, border: Some(RAINBOW_BORDER) },
];
pub const MUT_ORDER: [&str; 4] = ["normal", "golden", "diamond", "rainbow"];
pub const INDEX_ENTRIES: usize = N_PETS * 4;

// ---------------------------------------------------------------- phases (stacking)
/// Each verity (per mutation) grows through 4 phases by stacking copies of itself into it: Phase 1 (as rolled),
/// Phase 2 (the grin), Phase 3 (worn out) and Phase 4, its Monster form. The art is icons/pets/phases/<slug>_p<n>.png.
pub struct PhaseDef {
    pub name: &'static str,
    pub mult: f64,
    pub color: Color,
}

pub const PHASES: [PhaseDef; 4] = [
    PhaseDef { name: "Phase 1", mult: 1.0, color: Color::rgb(170, 176, 196) },
    PhaseDef { name: "Phase 2", mult: 2.0, color: Color::rgb(255, 196, 60) },
    PhaseDef { name: "Phase 3", mult: 4.0, color: Color::rgb(186, 120, 255) },
    PhaseDef { name: "Monster", mult: 10.0, color: Color::rgb(255, 70, 70) },
];
pub const MAX_PHASE: usize = PHASES.len() - 1;

/// Copies used up to go from `phase` to `phase + 1` (None once it's a Monster). Rarer tiers need fewer copies:
/// Common-Legendary 5 / 15 / 40, Mythic-Divine 3 / 8 / 20, Cosmic and up 2 / 4 / 8.
pub fn stack_cost(tier: usize, phase: usize) -> Option<i64> {
    const COSTS: [[i64; 3]; 3] = [[5, 15, 40], [3, 8, 20], [2, 4, 8]];
    if phase >= MAX_PHASE {
        return None;
    }
    let band = if tier <= 4 { 0 } else if tier <= 8 { 1 } else { 2 };
    Some(COSTS[band][phase])
}

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
pub const RAINBOW_MAX_CHANCE: f64 = 0.025;
pub const GOLDEN_BASE: f64 = 0.02;
pub const GOLDEN_STEP_1: f64 = 0.0025;
pub const GOLDEN_STEP_2: f64 = 0.002;
pub const DIAMOND_BASE: f64 = 0.0025;
pub const DIAMOND_STEP_1: f64 = 0.0005;
pub const DIAMOND_STEP_2: f64 = 0.0015;
pub const RAINBOW_BASE: f64 = 0.0005;
pub const RAINBOW_STEP_1: f64 = 0.0003;
pub const RAINBOW_STEP_2: f64 = 0.0004;

pub fn base_pet_chance(rarity_index: usize, mutation: &str) -> f64 {
    let r = rarities();
    let weights: Vec<f64> = r.iter().map(|x| x.share / x.one_in).collect();
    let total: f64 = weights.iter().sum();
    let rarity_chance = if total != 0.0 { weights[rarity_index] / total } else { 0.0 };
    rarity_chance * mutation_factor(mutation, GOLDEN_BASE, DIAMOND_BASE, RAINBOW_BASE)
}

/// Chance of a roll coming out with this mutation, with chances g (Golden), d (Diamond) and r (Rainbow).
/// Rainbow is checked first, then Diamond, then Golden (same as roll()).
pub fn mutation_factor(mutation: &str, g: f64, d: f64, r: f64) -> f64 {
    match mutation {
        "rainbow" => r,
        "diamond" => (1.0 - r) * d,
        "golden" => (1.0 - r) * (1.0 - d) * g,
        _ => (1.0 - r) * (1.0 - d) * (1.0 - g),
    }
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
            u("luck", "Luck", l("+%g%% weight to all Rare pets or better.", a![8.0]), 40, 50.0, 1.581, None, 1, 0),
            u("luck_prism", "Prismatic Luck", l("Stacks: +%g%% to Epic+, again to Mythic+ and again to Secret+.", a![7.000000000000001]), 25, 60000.0, 1.632, Some("luck"), 10, 0),
            u("luck_cosmic", "Exotic Luck", l("+%g%% weight to Exotic or better, per level.", a![16.0]), 20, 2000000000.0, 1.836, Some("luck_prism"), 15, 0),
            u("luck_divine", "Divine Luck", l("+%g%% weight to Divine or better, per level.", a![25.0]), 15, 100000000000.0, 2.04, Some("luck_cosmic"), 10, 0),
            u("luck_2", "Luck II", l("+%g%% weight to all Rare pets or better (multiplies with Luck).", a![10.0]), 30, 500000000000.0, 1.479, Some("luck"), 40, 0),
            u("luck_prism_2", "Prismatic Luck II", l("Stacks: +%g%% to Divine+, again to Cosmic+, again to Transcendent+ and again to Ethereal+.", a![6.0]), 25, 5000000000000.0, 1.53, Some("luck_prism"), 25, 0),
            u("luck_ultra", "Ultimate Luck", l("+%g%% weight to all Rare pets or better, per level.", a![5.0]), 60, 100000000000000.0, 1.275, Some("luck_2"), 15, 0),
            u("luck_tier_cosmico", "Cosmic Luck", l("+%g%% weight to Cosmic or better, per level.", a![30.0]), 20, 2000000000000.0, 1.581, Some("luck_divine"), 10, 0),
            u("luck_tier_transcendente", "Transcendent Luck", l("+%g%% weight to Transcendent or better, per level.", a![40.0]), 20, 20000000000000.0, 1.581, Some("luck_tier_cosmico"), 10, 0),
            u("luck_tier_etereo", "Ethereal Luck", l("+%g%% weight to Ethereal or better, per level.", a![50.0]), 20, 200000000000000.0, 1.632, Some("luck_tier_transcendente"), 10, 0),
            u("luck_tier_celestial", "Celestial Luck", l("+%g%% weight to Celestial or better, per level.", a![60.0]), 20, 2000000000000000.0, 1.632, Some("luck_tier_etereo"), 10, 0),
            u("luck_tier_absoluto", "Absolute Luck", l("+%g%% weight to Absolute or better, per level.", a![75.0]), 20, 2.0e+16, 1.683, Some("luck_tier_celestial"), 10, 0),
            u("money", "Money", l("+%g%% money/sec per level.", a![7.000000000000001]), 70, 40.0, 1.53, None, 1, 0),
            u("money_prism", "Superior Money", l("+%g%% money/sec per level (multiplies with regular Money).", a![16.0]), 40, 20000000000.0, 1.632, Some("money"), 35, 0),
            u("money_ultra", "Ultimate Money", l("+%g%% money/sec per level (multiplies with the other Money upgrades).", a![25.0]), 40, 100000000000000.0, 1.479, Some("money_prism"), 25, 0),
            u("slots", "Equip Slots", p("+1 slot to equip pets."), 20, 300.0, 2.448, None, 1, 0),
            u("slots_plus", "Extra Slots", p("+1 slot to equip pets, once your Slots are maxed out."), 15, 20000000000.0, 2.652, Some("slots"), 20, 0),
            u("auto_unlock", "Auto Roller", l("Unlocks the Auto Roller (needs Rebirth %d): rolls by itself while turned on.", a![2i64]), 1, 120000.0, 1.0, None, 1, 2),
            u("auto_speed", "Auto Speed", l("+%g%% Auto Roller speed per level.", a![22.0]), 40, 25000.0, 1.4484, Some("auto_unlock"), 1, 0),
            u("auto_turbo", "Auto Turbo", l("+%g%% Auto Roller speed per level (multiplies).", a![28.000000000000004]), 25, 50000000000.0, 1.53, Some("auto_speed"), 20, 0),
            u("golden_unlock", "Unlock Golden", p("Allows Golden mutations to appear on rolls (x3 money)."), 1, 2000.0, 1.0, None, 1, 0),
            u("golden_chance", "Golden Luck", l("+%g%% chance of Golden mutation per level.", a![0.25]), 20, 800.0, 1.938, Some("golden_unlock"), 1, 0),
            u("golden_chance_2", "Golden Luck II", l("+%g%% chance of Golden mutation per level.", a![0.2]), 15, 1500000000.0, 1.887, Some("golden_chance"), 20, 0),
            u("diamond_unlock", "Unlock Diamond", p("Allows Diamond mutations to appear on rolls (x9 money)."), 1, 25000.0, 1.0, Some("golden_unlock"), 1, 0),
            u("diamond_chance", "Diamond Luck", l("+%g%% chance of Diamond mutation per level.", a![0.05]), 22, 10000.0, 2.04, Some("diamond_unlock"), 1, 0),
            u("diamond_chance_2", "Diamond Luck II", l("+%g%% chance of Diamond mutation per level.", a![0.15]), 15, 50000000000.0, 1.938, Some("diamond_chance"), 22, 0),
            u("rainbow_unlock", "Unlock Rainbow", p("Allows Rainbow mutations to appear on rolls (x27 money)."), 1, 1000000000000.0, 1.0, Some("diamond_chance"), 22, 0),
            u("rainbow_chance", "Rainbow Luck", l("+%g%% chance of Rainbow mutation per level.", a![0.03]), 20, 2000000000000.0, 1.734, Some("rainbow_unlock"), 1, 0),
            u("rainbow_chance_2", "Rainbow Luck II", l("+%g%% chance of Rainbow mutation per level.", a![0.04]), 15, 500000000000000.0, 1.836, Some("rainbow_chance"), 20, 0),
            u("trait_charge_luck", "Trait Charge Luck", p("+6% extra (relative) chance of getting 1 trait charge per roll, per level."), 18, 15000.0, 1.734, None, 1, 0),
            u("trait_charge_luck_2", "Trait Charge Luck II", p("+10% extra (relative) chance of getting 1 trait charge per roll, per level."), 15, 1000000000.0, 1.887, Some("trait_charge_luck"), 18, 0),
            u("trait_rarity_luck", "Trait Luck", p("+9% weight to traits from Instinctive (the 3rd) upward when you roll a trait, per level."), 20, 20000.0, 1.836, None, 1, 0),
            u("trait_rarity_luck_2", "Trait Luck II", p("+15% weight to traits from Instinctive upward when you roll a trait, per level."), 15, 8000000000.0, 1.938, Some("trait_rarity_luck"), 20, 0),
            u("cyclic_every", "Short Cycle", l("-1 roll in the Golden Roll cycle per level (minimum %d rolls).", a![10i64]), 4, 50000000.0, 5.1, None, 1, 0),
            u("cyclic_power", "Strong Golden Roll", p("+1 to the Golden Roll multiplier per level."), 10, 1000000000.0, 2.448, Some("cyclic_every"), 3, 0),
            u("diamond_roll_unlock", "Unlock Diamond Roll", p("Unlocks the Diamond Roll cycle: every so many rolls, the next roll gets a massive luck boost."), 1, 20000000000.0, 1.0, Some("cyclic_power"), 5, 0),
            u("diamond_roll_every", "Diamond Short Cycle", p("-3 rolls in the Diamond Roll cycle per level (minimum 70 rolls)."), 10, 40000000000.0, 2.652, Some("diamond_roll_unlock"), 1, 0),
            u("diamond_roll_power", "Strong Diamond Roll", p("+5 to the Diamond Roll multiplier per level."), 10, 80000000000.0, 2.652, Some("diamond_roll_every"), 5, 0),
            u("rainbow_roll_unlock", "Unlock Rainbow Roll", p("Unlocks the Rainbow Roll cycle: every so many rolls, the next roll gets an insane luck boost."), 1, 500000000000.0, 1.0, Some("diamond_roll_power"), 5, 0),
            u("rainbow_roll_every", "Rainbow Short Cycle", p("-30 rolls in the Rainbow Roll cycle per level (minimum 700 rolls)."), 10, 1000000000000.0, 2.856, Some("rainbow_roll_unlock"), 1, 0),
            u("rainbow_roll_power", "Strong Rainbow Roll", p("+25 to the Rainbow Roll multiplier per level."), 10, 2000000000000.0, 2.856, Some("rainbow_roll_every"), 5, 0),
            u("offline_rate", "Offline Earnings", l("+%g%% offline earnings per level (you start at %g%% of your money/sec while the game is closed).", a![2.0, 30.0]), 15, 8000.0, 1.632, None, 1, 0),
            u("offline_rate_2", "Offline Earnings II", l("+%g%% offline earnings per level, once Offline Earnings is maxed out.", a![2.0]), 5, 4000000000.0, 1.938, Some("offline_rate"), 15, 0),
            u("offline_time", "Offline Time", l("+%d hour of max offline time per level (you start at %d hours).", a![1i64, 8i64]), 12, 15000.0, 1.683, None, 1, 0),
            u("offline_time_2", "Offline Time II", l("+%d hour of max offline time per level, once Offline Time is maxed out.", a![1i64]), 4, 8000000000.0, 2.04, Some("offline_time"), 12, 0),
            u("auto_upgrade_unlock", "Auto Upgrader", p("Buys the cheapest upgrade you can afford by itself while turned on (toggle at the top of Upgrades). Never resets on Rebirth."), 1, 250000.0, 1.0, None, 1, 0),
            u("auto_trait_unlock", "Auto Trait Roller", p("Rolls your trait charges by itself while turned on (toggle on the Traits page). Never resets on Rebirth or Prestige."), 1, 2000000.0, 1.0, None, 1, 0),
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
    UpgradeCategory { key: "luck", label: "Luck", desc: "Rarer pets show up more often", upgrades: &["luck", "luck_prism", "luck_cosmic", "luck_divine", "luck_2", "luck_prism_2", "luck_ultra", "luck_tier_cosmico", "luck_tier_transcendente", "luck_tier_etereo", "luck_tier_celestial", "luck_tier_absoluto"] },
    UpgradeCategory { key: "mutations", label: "Mutation Chance", desc: "Golden, Diamond and Rainbow pets", upgrades: &["golden_unlock", "golden_chance", "golden_chance_2", "diamond_unlock", "diamond_chance", "diamond_chance_2", "rainbow_unlock", "rainbow_chance", "rainbow_chance_2"] },
    UpgradeCategory { key: "money", label: "Money", desc: "Earn more money per second", upgrades: &["money", "money_prism", "money_ultra"] },
    UpgradeCategory { key: "traits", label: "Traits", desc: "Trait charges and trait rarity", upgrades: &["auto_trait_unlock", "trait_charge_luck", "trait_charge_luck_2", "trait_rarity_luck", "trait_rarity_luck_2"] },
    UpgradeCategory { key: "bonus_rolls", label: "Bonus Rolls", desc: "Golden, Diamond and Rainbow Roll", upgrades: &["cyclic_every", "cyclic_power", "diamond_roll_unlock", "diamond_roll_every", "diamond_roll_power", "rainbow_roll_unlock", "rainbow_roll_every", "rainbow_roll_power"] },
    UpgradeCategory { key: "auto", label: "Auto Roller", desc: "Rolls by itself, faster", upgrades: &["auto_unlock", "auto_speed", "auto_turbo"] },
    UpgradeCategory { key: "offline", label: "Offline", desc: "Earn more while the game is closed", upgrades: &["offline_rate", "offline_rate_2", "offline_time", "offline_time_2"] },
    UpgradeCategory { key: "misc", label: "Misc", desc: "Auto Upgrader, Equip Slots and Auto Equip Best", upgrades: &["auto_upgrade_unlock", "slots", "slots_plus", "auto_equip_unlock"] },
];

pub fn upgrade_category(key: &str) -> Option<&'static UpgradeCategory> {
    UPGRADE_CATEGORIES.iter().find(|c| c.key == key)
}

pub const BASE_SLOTS: i64 = 3;

/// upgrades a Rebirth NEVER resets (not even the first ones, before "Rebirth Master")
pub const KEEP_ON_REBIRTH: [&str; 2] = ["auto_upgrade_unlock", "auto_trait_unlock"];
/// upgrades a Prestige doesn't reset either (the automation)
pub const KEEP_ON_PRESTIGE: [&str; 2] = ["auto_upgrade_unlock", "auto_trait_unlock"];
/// how often the Auto Trait Roller spends your charges
pub const AUTO_TRAIT_EVERY: f64 = 1.0;

// ---------------------------------------------------------------- prestige (v3.0)
/// One Prestige. The multipliers are the TOTAL you have once you reach it (not stacked on the previous ones), and
/// each Prestige jumps more than the one before. A Prestige resets coins, Rebirths, upgrades (not the automation)
/// and pets (except the one verity you keep); dice, potions, traits, milestones, the Index and stats stay.
pub struct PrestigeDef {
    /// Rebirths needed to do this Prestige
    pub need: i64,
    pub name: &'static str,
    pub money: f64,
    pub luck: f64,
    /// extra equip slots (total)
    pub slots: i64,
    /// Auto Roller speed (total multiplier)
    pub auto_speed: f64,
    /// trait charge chance (total multiplier)
    pub charge_chance: f64,
    /// what this Prestige unlocks besides the multipliers (English; the screen translates it)
    pub perk: &'static str,
}

pub const PRESTIGES: [PrestigeDef; 5] = [
    PrestigeDef { need: 10, name: "Prestige I", money: 3.0, luck: 3.0, slots: 1, auto_speed: 1.0, charge_chance: 1.0, perk: "+1 Equip Slot" },
    PrestigeDef { need: 15, name: "Prestige II", money: 8.0, luck: 6.0, slots: 1, auto_speed: 1.0, charge_chance: 1.0, perk: "Rebirths never reset your upgrades again" },
    PrestigeDef { need: 20, name: "Prestige III", money: 20.0, luck: 12.0, slots: 1, auto_speed: 1.5, charge_chance: 1.0, perk: "Rebirths don't reset anything, not even your coins. x1.5 Auto Roller speed" },
    PrestigeDef { need: 30, name: "Prestige IV", money: 60.0, luck: 25.0, slots: 2, auto_speed: 2.0, charge_chance: 2.0, perk: "+1 Equip Slot, x2 Auto Roller speed, x2 trait charge chance" },
    PrestigeDef { need: 40, name: "Prestige V", money: 200.0, luck: 60.0, slots: 3, auto_speed: 3.0, charge_chance: 3.0, perk: "+1 Equip Slot, x3 Auto Roller speed, x3 trait charge chance" },
];
/// the Prestige that makes Rebirths keep your upgrades / keep everything
pub const PRESTIGE_KEEP_UPGRADES: i64 = 2;
pub const PRESTIGE_REBIRTH_FREE: i64 = 3;
/// how often the Auto Upgrader tries to buy (seconds)
pub const AUTO_UPGRADE_EVERY: f64 = 0.5;
/// most levels bought at a time (so a lot of money doesn't freeze the game)
pub const AUTO_UPGRADE_MAX_BUYS: i64 = 25;

/// UPGRADE_ORDER: every upgrade, category by category
pub fn upgrade_order() -> impl Iterator<Item = &'static str> {
    UPGRADE_CATEGORIES.iter().flat_map(|c| c.upgrades.iter().copied())
}

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

pub const TRAITS: [Trait; 17] = [
    Trait { name: "Lucky", color: Color::rgb(110, 130, 145), text: WHITE, one_in: 3.0, buffs: &[("money", 0.03)] },
    Trait { name: "Sharp-Eyed", color: Color::rgb(60, 150, 195), text: WHITE, one_in: 8.0, buffs: &[("money", 0.05), ("luck", 0.04)] },
    Trait { name: "Instinctive", color: Color::rgb(55, 115, 225), text: WHITE, one_in: 20.0, buffs: &[("money", 0.07), ("luck", 0.06), ("charge_chance", 0.15)] },
    Trait { name: "Visionary", color: Color::rgb(110, 75, 220), text: WHITE, one_in: 60.0, buffs: &[("money", 0.1), ("luck", 0.08), ("charge_chance", 0.22), ("mutation", 0.02)] },
    Trait { name: "Prodigious", color: Color::rgb(165, 55, 210), text: WHITE, one_in: 200.0, buffs: &[("money", 0.13), ("luck", 0.11), ("charge_chance", 0.3), ("mutation", 0.03), ("auto_speed", 0.15)] },
    Trait { name: "Radiant", color: Color::rgb(220, 75, 160), text: WHITE, one_in: 800.0, buffs: &[("money", 0.18), ("luck", 0.14), ("charge_chance", 0.4), ("mutation", 0.04), ("auto_speed", 0.22)] },
    Trait { name: "Ascendant", color: Color::rgb(200, 60, 45), text: WHITE, one_in: 3000.0, buffs: &[("money", 0.24), ("luck", 0.18), ("charge_chance", 0.5), ("mutation", 0.06), ("auto_speed", 0.3), ("secret_luck", 0.25)] },
    Trait { name: "Stellar", color: Color::rgb(230, 150, 40), text: BLACK, one_in: 12000.0, buffs: &[("money", 0.33), ("luck", 0.23), ("charge_chance", 0.65), ("mutation", 0.08), ("auto_speed", 0.42), ("secret_luck", 0.45)] },
    Trait { name: "Absolute", color: Color::rgb(240, 205, 60), text: BLACK, one_in: 60000.0, buffs: &[("money", 0.45), ("luck", 0.3), ("charge_chance", 0.85), ("mutation", 0.12), ("auto_speed", 0.6), ("secret_luck", 0.7)] },
    Trait { name: "Luck Deity", color: Color::rgb(250, 250, 250), text: Color::rgb(90, 20, 55), one_in: 300000.0, buffs: &[("money", 0.6), ("luck", 0.42), ("charge_chance", 1.1), ("mutation", 0.16), ("auto_speed", 0.85), ("secret_luck", 1.2)] },
    Trait { name: "Immortal", color: Color::rgb(40, 215, 195), text: BLACK, one_in: 1500000.0, buffs: &[("money", 0.81), ("luck", 0.54), ("charge_chance", 1.4), ("mutation", 0.2), ("auto_speed", 1.1), ("secret_luck", 1.7)] },
    Trait { name: "Omnipotent", color: Color::rgb(22, 20, 34), text: Color::rgb(255, 214, 90), one_in: 8000000.0, buffs: &[("money", 1.05), ("luck", 0.66), ("charge_chance", 1.75), ("mutation", 0.25), ("auto_speed", 1.4), ("secret_luck", 2.3)] },
    Trait { name: "Eternal", color: Color::rgb(150, 220, 255), text: BLACK, one_in: 40000000.0, buffs: &[("money", 1.32), ("luck", 0.81), ("charge_chance", 2.1), ("mutation", 0.25), ("auto_speed", 1.7), ("secret_luck", 3.0)] },
    Trait { name: "Infinite", color: Color::rgb(120, 70, 230), text: WHITE, one_in: 200000000.0, buffs: &[("money", 1.68), ("luck", 0.99), ("charge_chance", 2.5), ("mutation", 0.25), ("auto_speed", 2.0), ("secret_luck", 3.8)] },
    Trait { name: "Godlike", color: Color::rgb(255, 225, 120), text: Color::rgb(90, 50, 0), one_in: 1000000000.0, buffs: &[("money", 2.1), ("luck", 1.2), ("charge_chance", 3.0), ("mutation", 0.25), ("auto_speed", 2.4), ("secret_luck", 4.8)] },
    Trait { name: "Primeval", color: Color::rgb(200, 60, 20), text: Color::rgb(255, 225, 170), one_in: 5000000000.0, buffs: &[("money", 2.64), ("luck", 1.47), ("charge_chance", 3.6), ("mutation", 0.25), ("auto_speed", 2.9), ("secret_luck", 6.0)] },
    Trait { name: "Supreme", color: Color::rgb(10, 10, 14), text: Color::rgb(255, 120, 255), one_in: 25000000000.0, buffs: &[("money", 3.3), ("luck", 1.8), ("charge_chance", 4.3), ("mutation", 0.25), ("auto_speed", 3.5), ("secret_luck", 7.5)] },
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
        "rainbow_luck" => "Rainbow Luck",
        "ethereal_luck" => "Ethereal Luck",
        "celestial_luck" => "Celestial Luck",
        "absolute_luck" => "Absolute Luck",
        "index_luck" => "Luck",
        _ => "",
    }
}

/// In MILESTONE_DEFS insertion order (which is also MILESTONE_CAT_ORDER).
pub const MILESTONES: [MilestoneCat; 15] = [
    MilestoneCat { key: "rolls", label: "Total Rolls", reward_type: "luck", tiers: &[(50.0, 0.01), (150.0, 0.02), (400.0, 0.02), (1000.0, 0.03), (2500.0, 0.04), (6000.0, 0.04), (15000.0, 0.05), (40000.0, 0.06), (100000.0, 0.07), (250000.0, 0.08), (600000.0, 0.1), (1500000.0, 0.11), (4000000.0, 0.12), (10000000.0, 0.14), (25000000.0, 0.16), (50000000.0, 0.18), (100000000.0, 0.21), (250000000.0, 0.24), (500000000.0, 0.27), (1000000000.0, 0.3), (2500000000.0, 0.36)] },
    MilestoneCat { key: "traits_rolled", label: "Traits Rolled", reward_type: "trait_luck", tiers: &[(5.0, 0.03), (15.0, 0.05), (30.0, 0.07), (60.0, 0.1), (120.0, 0.13), (250.0, 0.17), (500.0, 0.22), (1000.0, 0.28), (2000.0, 0.35), (4000.0, 0.43), (8000.0, 0.52), (15000.0, 0.62), (30000.0, 0.73), (60000.0, 0.85), (120000.0, 1.0), (250000.0, 1.15), (500000.0, 1.3), (1000000.0, 1.5)] },
    MilestoneCat { key: "golden_rolled", label: "Golden Pets Rolled", reward_type: "golden_luck", tiers: &[(5.0, 0.01), (15.0, 0.01), (35.0, 0.01), (75.0, 0.02), (150.0, 0.02), (300.0, 0.03), (600.0, 0.03), (1200.0, 0.04), (2500.0, 0.04), (5000.0, 0.05), (10000.0, 0.05), (20000.0, 0.06), (40000.0, 0.07), (80000.0, 0.08), (150000.0, 0.08), (300000.0, 0.09), (600000.0, 0.09), (1200000.0, 0.1), (2500000.0, 0.1)] },
    MilestoneCat { key: "diamond_rolled", label: "Diamond Pets Rolled", reward_type: "diamond_luck", tiers: &[(2.0, 0.01), (6.0, 0.01), (15.0, 0.01), (35.0, 0.02), (75.0, 0.02), (150.0, 0.03), (300.0, 0.03), (600.0, 0.04), (1200.0, 0.04), (2500.0, 0.05), (5000.0, 0.05), (10000.0, 0.06), (20000.0, 0.07), (40000.0, 0.08), (80000.0, 0.08), (160000.0, 0.09), (320000.0, 0.09), (640000.0, 0.1), (1250000.0, 0.1)] },
    MilestoneCat { key: "secret_rolled", label: "Secret Pets Rolled", reward_type: "secret_luck", tiers: &[(1.0, 0.02), (3.0, 0.03), (5.0, 0.04), (10.0, 0.05), (25.0, 0.07), (50.0, 0.09), (100.0, 0.12), (250.0, 0.15), (500.0, 0.19), (1000.0, 0.24), (2500.0, 0.3), (5000.0, 0.38), (10000.0, 0.5), (25000.0, 0.6), (50000.0, 0.7), (100000.0, 0.85)] },
    MilestoneCat { key: "divine_rolled", label: "Divine Pets Rolled", reward_type: "divine_luck", tiers: &[(1.0, 0.03), (2.0, 0.04), (3.0, 0.05), (5.0, 0.06), (10.0, 0.08), (20.0, 0.1), (40.0, 0.12), (80.0, 0.14), (150.0, 0.17), (300.0, 0.2), (600.0, 0.24), (1200.0, 0.29), (2500.0, 0.35), (5000.0, 0.42), (10000.0, 0.5), (25000.0, 0.6)] },
    MilestoneCat { key: "cosmic_rolled", label: "Cosmic Pets Rolled", reward_type: "cosmic_luck", tiers: &[(1.0, 0.04), (2.0, 0.05), (3.0, 0.06), (5.0, 0.08), (10.0, 0.1), (20.0, 0.12), (40.0, 0.15), (80.0, 0.18), (150.0, 0.22), (300.0, 0.26), (600.0, 0.3), (1200.0, 0.35), (2500.0, 0.42), (5000.0, 0.5)] },
    MilestoneCat { key: "transcendent_rolled", label: "Transcendent Pets Rolled", reward_type: "transcendent_luck", tiers: &[(1.0, 0.05), (2.0, 0.06), (3.0, 0.08), (5.0, 0.1), (10.0, 0.12), (20.0, 0.15), (40.0, 0.19), (80.0, 0.24), (150.0, 0.3), (300.0, 0.4), (600.0, 0.5), (1200.0, 0.6)] },
    MilestoneCat { key: "rainbow_rolled", label: "Rainbow Pets Rolled", reward_type: "rainbow_luck", tiers: &[(1.0, 0.02), (3.0, 0.03), (8.0, 0.03), (20.0, 0.04), (50.0, 0.05), (120.0, 0.05), (300.0, 0.06), (700.0, 0.07), (1500.0, 0.07), (3500.0, 0.08), (8000.0, 0.08), (20000.0, 0.1)] },
    MilestoneCat { key: "ethereal_rolled", label: "Ethereal Pets Rolled", reward_type: "ethereal_luck", tiers: &[(1.0, 0.06), (2.0, 0.07), (3.0, 0.09), (5.0, 0.11), (10.0, 0.14), (20.0, 0.17), (40.0, 0.21), (80.0, 0.26), (150.0, 0.32), (300.0, 0.4), (600.0, 0.5), (1200.0, 0.6)] },
    MilestoneCat { key: "celestial_rolled", label: "Celestial Pets Rolled", reward_type: "celestial_luck", tiers: &[(1.0, 0.07), (2.0, 0.09), (3.0, 0.11), (5.0, 0.14), (10.0, 0.17), (20.0, 0.21), (40.0, 0.26), (80.0, 0.32), (150.0, 0.4), (300.0, 0.5)] },
    MilestoneCat { key: "absolute_rolled", label: "Absolute Pets Rolled", reward_type: "absolute_luck", tiers: &[(1.0, 0.08), (2.0, 0.1), (3.0, 0.13), (5.0, 0.16), (10.0, 0.2), (20.0, 0.25), (40.0, 0.32), (80.0, 0.4)] },
    MilestoneCat { key: "indexed_pets", label: "Indexed Pets", reward_type: "index_luck", tiers: &[(2.0, 0.01), (4.0, 0.01), (7.0, 0.01), (10.0, 0.02), (14.0, 0.02), (19.0, 0.02), (25.0, 0.03), (32.0, 0.04), (40.0, 0.04), (49.0, 0.05), (57.0, 0.05), (70.0, 0.06), (90.0, 0.07), (115.0, 0.07), (140.0, 0.08), (170.0, 0.08), (200.0, 0.09), (256.0, 0.12)] },
    MilestoneCat { key: "coins", label: "Total Coins Earned", reward_type: "money", tiers: &[(1000.0, 0.02), (5000.0, 0.02), (25000.0, 0.04), (100000.0, 0.05), (500000.0, 0.06), (2500000.0, 0.08), (10000000.0, 0.1), (50000000.0, 0.12), (250000000.0, 0.15), (1000000000.0, 0.18), (5000000000.0, 0.22), (25000000000.0, 0.27), (100000000000.0, 0.33), (500000000000.0, 0.42), (1000000000000.0, 0.54), (10000000000000.0, 0.6), (100000000000000.0, 0.66), (1000000000000000.0, 0.75), (1.0e+16, 0.84), (1.0e+17, 0.96), (1.0e+18, 1.08), (1.0e+19, 1.2), (1.0e+20, 1.35), (1.0e+21, 1.5)] },
    MilestoneCat { key: "playtime", label: "Playtime", reward_type: "auto_speed", tiers: &[(300.0, 0.03), (900.0, 0.05), (1800.0, 0.07), (3600.0, 0.09), (7200.0, 0.12), (14400.0, 0.15), (28800.0, 0.19), (57600.0, 0.24), (86400.0, 0.3), (172800.0, 0.38), (345600.0, 0.48), (604800.0, 0.6), (1209600.0, 0.75), (2592000.0, 1.0), (5184000.0, 1.2), (7776000.0, 1.4)] },
];
pub const MILESTONE_CAT_ORDER: [&str; 15] = ["rolls", "traits_rolled", "golden_rolled", "diamond_rolled", "rainbow_rolled", "secret_rolled", "divine_rolled", "cosmic_rolled", "transcendent_rolled", "ethereal_rolled", "celestial_rolled", "absolute_rolled", "indexed_pets", "coins", "playtime"];

/// Groups: several categories in one row of the main list, which opens a list of just them (the rarity ones are
/// many: the list was huge).
pub struct MilestoneGroup {
    pub key: &'static str,
    pub label: &'static str,
    pub desc: &'static str,
    pub categories: &'static [&'static str],
}

pub const MILESTONE_GROUPS: [MilestoneGroup; 1] = [MilestoneGroup {
    key: "rarities",
    label: "Rarity Milestones",
    desc: "Secret, Divine, Cosmic, Transcendent, Ethereal, Celestial and Absolute pets rolled",
    categories: &["secret_rolled", "divine_rolled", "cosmic_rolled", "transcendent_rolled", "ethereal_rolled", "celestial_rolled", "absolute_rolled"],
}];

pub fn milestone_group(key: &str) -> Option<&'static MilestoneGroup> {
    MILESTONE_GROUPS.iter().find(|g| g.key == key)
}

/// MILESTONE_GROUP_OF: the group a category belongs to.
pub fn milestone_group_of(category: &str) -> Option<&'static str> {
    MILESTONE_GROUPS.iter().find(|g| g.categories.contains(&category)).map(|g| g.key)
}

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

// ---------------------------------------------------------------- daily / weekly quests
/// v3.0: 7 stages (by total rolls, see GameState::mission_stage); `min_stage` = the quest only shows from there.
/// `reward` = base trait charges: the real reward grows with what you have (GameState::quest_reward).
pub struct MissionDef {
    pub mtype: &'static str,
    pub label: &'static str,
    pub targets: [i64; 7],
    pub reward: [i64; 7],
    pub min_stage: usize,
}

/// total rolls where each quest stage starts (stage 0 below the first)
pub const MISSION_STAGE_ROLLS: [i64; 6] = [250, 2500, 25000, 250000, 2500000, 25000000];

pub const MISSION_POOL: [MissionDef; 8] = [
    MissionDef { mtype: "rolls", label: "Roll %d pets", targets: [150, 300, 600, 1200, 2500, 8000, 25000], reward: [3, 4, 5, 6, 8, 10, 12], min_stage: 0 },
    MissionDef { mtype: "golden", label: "Get %d Golden pets", targets: [2, 4, 8, 15, 30, 80, 250], reward: [3, 4, 5, 6, 8, 10, 12], min_stage: 0 },
    MissionDef { mtype: "diamond", label: "Get %d Diamond pets", targets: [1, 2, 4, 8, 15, 40, 120], reward: [4, 5, 6, 8, 10, 12, 15], min_stage: 0 },
    MissionDef { mtype: "traits", label: "Roll %d traits", targets: [2, 4, 7, 12, 20, 40, 100], reward: [3, 4, 5, 6, 8, 10, 12], min_stage: 0 },
    MissionDef { mtype: "rarity_epico", label: "Get %d Epic pet(s)", targets: [1, 2, 3, 5, 8, 20, 60], reward: [3, 4, 5, 6, 7, 9, 11], min_stage: 0 },
    MissionDef { mtype: "rarity_lendario", label: "Get %d Legendary pet(s)", targets: [1, 1, 2, 3, 5, 12, 35], reward: [4, 5, 6, 7, 8, 10, 12], min_stage: 0 },
    MissionDef { mtype: "rarity_mitico", label: "Get %d Mythic pet(s)", targets: [1, 1, 1, 2, 3, 8, 20], reward: [6, 6, 7, 8, 10, 12, 14], min_stage: 0 },
    MissionDef { mtype: "rarity_secreto", label: "Get a Secret pet", targets: [1, 1, 1, 1, 1, 1, 1], reward: [10, 10, 10, 10, 10, 12, 14], min_stage: 0 },
];
pub const DAILY_MISSION_COUNT: usize = 3;

/// v3.0 weekly quests: harder, bigger rewards (and a potion). Reset Monday 00:00, Lisbon time.
pub const WEEKLY_POOL: [MissionDef; 8] = [
    MissionDef { mtype: "rolls", label: "Roll %d pets", targets: [2500, 6000, 15000, 50000, 200000, 800000, 3000000], reward: [15, 20, 25, 30, 40, 50, 60], min_stage: 0 },
    MissionDef { mtype: "golden", label: "Get %d Golden pets", targets: [25, 50, 120, 400, 1500, 6000, 25000], reward: [15, 20, 25, 30, 40, 50, 60], min_stage: 0 },
    MissionDef { mtype: "diamond", label: "Get %d Diamond pets", targets: [10, 20, 50, 150, 600, 2500, 10000], reward: [20, 25, 30, 40, 50, 60, 75], min_stage: 0 },
    MissionDef { mtype: "traits", label: "Roll %d traits", targets: [15, 30, 60, 150, 500, 2000, 8000], reward: [15, 20, 25, 30, 40, 50, 60], min_stage: 0 },
    MissionDef { mtype: "rarity_mitico", label: "Get %d Mythic pet(s)", targets: [3, 5, 10, 25, 80, 300, 1200], reward: [20, 25, 30, 40, 50, 60, 75], min_stage: 0 },
    MissionDef { mtype: "rarity_secreto", label: "Get %d Secret pets", targets: [1, 2, 3, 6, 20, 80, 300], reward: [25, 30, 40, 50, 60, 75, 90], min_stage: 1 },
    MissionDef { mtype: "rebirths", label: "Do %d Rebirths", targets: [1, 1, 2, 3, 4, 5, 6], reward: [25, 30, 40, 50, 60, 75, 90], min_stage: 2 },
    MissionDef { mtype: "rainbow", label: "Get %d Rainbow pets", targets: [1, 1, 3, 10, 40, 150, 600], reward: [30, 40, 50, 60, 75, 90, 110], min_stage: 4 },
];
pub const WEEKLY_MISSION_COUNT: usize = 3;
/// what a quest's reward is worth, in minutes of your money/sec and of your Auto Roller's trait charges
pub const DAILY_REWARD_MINUTES: f64 = 15.0;
pub const WEEKLY_REWARD_MINUTES: f64 = 180.0;

/// The label of a quest type (daily or weekly).
pub fn mission_def(mtype: &str) -> Option<&'static MissionDef> {
    MISSION_POOL.iter().chain(WEEKLY_POOL.iter()).find(|m| m.mtype == mtype)
}
