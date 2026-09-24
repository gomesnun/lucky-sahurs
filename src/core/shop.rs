//! Shop (unlocks at Rebirth 1): roll dice and potions - core/shop.py.
//!
//! GLOBAL STOCK WITH NO READS: the stock changes every SHOP_PERIOD seconds (10 min, on the wall clock) and is
//! computed from the period number with a fixed-seed random generator. So every player sees EXACTLY the same stock
//! at the same time, without anyone reading anything from Firestore. Callers pass the time (the server's, see
//! server_now in online/firebase.rs), so PC clocks don't mix this up. Each item in stock is 1 only: buying it takes
//! it out of THIS player's stock until the next period.
//!
//! DICE: bought once, kept forever. Only 1 equipped; gives extra luck and changes the ROLL button's style.
//! POTIONS: 3 kinds (money, roll speed, luck) x 5 levels. Used in the Bag, they last POTION_SECONDS of play.
//! 5 potions of a level combine into one of the next level.

use super::state::GameState;
use crate::gfx::Color;
use crate::pyrand::PyRandom;
use crate::storage::{value_f64, value_i64};
use indexmap::IndexMap;
use serde_json::{Map, Value, json};

pub const SHOP_PERIOD: f64 = 10.0 * 60.0;
pub const SHOP_UNLOCK_REBIRTHS: i64 = 1;

/// Colours of the ROLL button while a die is equipped (effect: "rainbow" / "stars" / none).
pub struct DiceStyle {
    pub base: Color,
    pub hover: Color,
    pub border: Color,
    pub text: Color,
    pub effect: Option<&'static str>,
}

pub struct Dice {
    pub key: &'static str,
    pub name: &'static str,
    pub price: f64,
    /// chance of showing up in each period's stock
    pub chance: f64,
    pub luck: f64,
    pub style: DiceStyle,
}

const fn st(base: (u8, u8, u8), hover: (u8, u8, u8), border: (u8, u8, u8), text: (u8, u8, u8), effect: Option<&'static str>) -> DiceStyle {
    DiceStyle {
        base: Color::rgb(base.0, base.1, base.2),
        hover: Color::rgb(hover.0, hover.1, hover.2),
        border: Color::rgb(border.0, border.1, border.2),
        text: Color::rgb(text.0, text.1, text.2),
        effect,
    }
}

/// v3.0.1: the stock chances about halved (the dice showed up too often)
pub const DICE: [Dice; 10] = [
    Dice { key: "wood", name: "Wooden Dice", price: 25000.0, chance: 0.7, luck: 0.10, style: st((150, 98, 58), (176, 118, 72), (96, 60, 32), (255, 240, 220), None) },
    Dice { key: "stone", name: "Stone Dice", price: 1000000.0, chance: 0.4, luck: 0.20, style: st((118, 122, 132), (140, 145, 156), (70, 74, 84), (255, 255, 255), None) },
    Dice { key: "iron", name: "Iron Dice", price: 50000000.0, chance: 0.25, luck: 0.35, style: st((84, 96, 116), (104, 118, 140), (190, 200, 215), (235, 242, 255), None) },
    Dice { key: "gold", name: "Golden Dice", price: 2500000000.0, chance: 0.15, luck: 0.55, style: st((232, 180, 40), (250, 200, 70), (150, 100, 10), (70, 40, 0), None) },
    Dice { key: "emerald", name: "Emerald Dice", price: 150000000000.0, chance: 0.1, luck: 0.80, style: st((30, 150, 90), (44, 176, 108), (150, 255, 190), (230, 255, 240), None) },
    Dice { key: "ruby", name: "Ruby Dice", price: 10000000000000.0, chance: 0.06, luck: 1.10, style: st((190, 30, 60), (215, 48, 80), (255, 150, 170), (255, 235, 240), None) },
    Dice { key: "sapphire", name: "Sapphire Dice", price: 750000000000000.0, chance: 0.04, luck: 1.50, style: st((36, 70, 190), (52, 92, 220), (150, 190, 255), (235, 245, 255), None) },
    Dice { key: "amethyst", name: "Amethyst Dice", price: 60000000000000000.0, chance: 0.025, luck: 2.00, style: st((120, 50, 190), (142, 66, 220), (220, 170, 255), (250, 240, 255), None) },
    Dice { key: "cosmic", name: "Cosmic Dice", price: 5000000000000000000.0, chance: 0.015, luck: 2.75, style: st((22, 16, 60), (36, 26, 90), (150, 120, 255), (220, 210, 255), Some("stars")) },
    Dice { key: "prism", name: "Prism Dice", price: 500000000000000000000.0, chance: 0.008, luck: 4.00, style: st((250, 250, 255), (255, 255, 255), (255, 255, 255), (60, 40, 90), Some("rainbow")) },
];

pub fn dice_by_key(key: &str) -> Option<&'static Dice> {
    DICE.iter().find(|d| d.key == key)
}

pub struct PotionType {
    pub key: &'static str,
    pub name: &'static str,
    /// effect per level I..V: "money" and "speed" multiply money/sec and the Auto Roller's rolls/sec,
    /// "luck" multiplies luck (Rare or better)
    pub effect: [f64; 5],
    pub stat: &'static str,
}

pub const POTION_TYPES: [PotionType; 3] = [
    PotionType { key: "money", name: "Coin Potion", effect: [1.5, 2.0, 3.0, 5.0, 8.0], stat: "Money" },
    PotionType { key: "speed", name: "Speed Potion", effect: [1.2, 1.4, 1.7, 2.0, 2.5], stat: "Roll Speed" },
    PotionType { key: "luck", name: "Luck Potion", effect: [1.25, 1.5, 2.0, 3.0, 5.0], stat: "Luck" },
];

pub fn potion_by_key(key: &str) -> Option<&'static PotionType> {
    POTION_TYPES.iter().find(|p| p.key == key)
}

pub const POTION_LEVELS: i64 = 5;
/// each potion lasts 10 min of play (using another of the same adds 10 more)
pub const POTION_SECONDS: f64 = 10.0 * 60.0;
/// at most 2 h stacked of each kind
pub const POTION_MAX_SECONDS: f64 = 2.0 * 60.0 * 60.0;
/// 5 of level N = 1 of level N+1
pub const POTION_COMBINE: i64 = 5;
/// chance of each level being in stock each period (each kind x level is 1 item, stock 1)
pub const POTION_STOCK_CHANCE: [f64; 5] = [0.75, 0.40, 0.18, 0.07, 0.02];
/// price = X seconds of your money/sec (so it's worth it at any stage of the game)
pub const POTION_PRICE_SECONDS: [f64; 5] = [90.0, 300.0, 900.0, 2700.0, 7200.0];
pub const POTION_MIN_PRICE: f64 = 5000.0;
pub const ROMAN: [&str; 5] = ["I", "II", "III", "IV", "V"];

pub fn potion_id(kind: &str, level: i64) -> String {
    format!("{}_{}", kind, level)
}

pub fn shop_period(now: f64) -> i64 {
    (now / SHOP_PERIOD).floor() as i64
}

pub fn shop_seconds_left(now: f64) -> f64 {
    SHOP_PERIOD - now.rem_euclid(SHOP_PERIOD)
}

pub struct ShopStock {
    pub dice: Vec<&'static str>,
    pub potions: Vec<String>,
}

/// The period's stock (the same for everyone).
pub fn shop_stock(period: i64) -> ShopStock {
    let mut rng = PyRandom::from_str(&format!("lucky-verities-shop-{}", period));
    let dice = DICE.iter().filter(|d| rng.random() < d.chance).map(|d| d.key).collect();
    let mut potions = Vec::new();
    for p in POTION_TYPES.iter() {
        for lvl in 1..=POTION_LEVELS {
            if rng.random() < POTION_STOCK_CHANCE[(lvl - 1) as usize] {
                potions.push(potion_id(p.key, lvl));
            }
        }
    }
    ShopStock { dice, potions }
}

/// Dice and potions inside a GameState.
#[derive(Clone, Debug)]
pub struct ShopState {
    /// keys of the dice bought (forever, not even a Rebirth takes them)
    pub dice_owned: Vec<&'static str>,
    pub dice_equipped: Option<&'static str>,
    /// "luck_2" -> how many in the inventory
    pub potions: IndexMap<String, i64>,
    /// kind -> (level, seconds left)
    pub active_potions: IndexMap<&'static str, (i64, f64)>,
    /// period in which the items below were bought
    pub bought_period: i64,
    /// items ("dice:wood", "potion:luck_2") already bought this period
    pub bought: Vec<String>,
}

impl Default for ShopState {
    fn default() -> Self {
        ShopState { dice_owned: Vec::new(), dice_equipped: None, potions: IndexMap::new(), active_potions: IndexMap::new(), bought_period: -1, bought: Vec::new() }
    }
}

impl ShopState {
    pub fn to_dict(&self) -> Value {
        let mut d = Map::new();
        d.insert("dice_owned".into(), json!(self.dice_owned));
        d.insert("dice_equipped".into(), match self.dice_equipped {
            Some(k) => json!(k),
            None => Value::Null,
        });
        let pots: Map<String, Value> = self.potions.iter().map(|(k, v)| (k.clone(), json!(v))).collect();
        d.insert("potions".into(), Value::Object(pots));
        let act: Map<String, Value> =
            self.active_potions.iter().map(|(k, (lvl, secs))| (k.to_string(), json!([lvl, crate::pyjson::float(*secs)]))).collect();
        d.insert("active_potions".into(), Value::Object(act));
        d.insert("bought_period".into(), json!(self.bought_period));
        d.insert("bought".into(), json!(self.bought));
        Value::Object(d)
    }

    pub fn load_dict(d: Option<&Value>) -> ShopState {
        let mut s = ShopState::default();
        let Some(Value::Object(d)) = d else { return s };
        if let Some(Value::Array(a)) = d.get("dice_owned") {
            for k in a {
                if let Some(dd) = k.as_str().and_then(dice_by_key) {
                    s.dice_owned.push(dd.key);
                }
            }
        }
        if let Some(eq) = d.get("dice_equipped").and_then(|v| v.as_str()) {
            s.dice_equipped = s.dice_owned.iter().copied().find(|k| *k == eq);
        }
        if let Some(Value::Object(p)) = d.get("potions") {
            for (k, q) in p {
                let Some((kind, lvl)) = k.rsplit_once('_') else { continue };
                let (Some(pt), Ok(lvl), Some(q)) = (potion_by_key(kind), lvl.parse::<i64>(), value_i64(q)) else { continue };
                if (1..=POTION_LEVELS).contains(&lvl) && q > 0 {
                    s.potions.insert(potion_id(pt.key, lvl), q);
                }
            }
        }
        if let Some(Value::Object(a)) = d.get("active_potions") {
            for (k, v) in a {
                let Some(pt) = potion_by_key(k) else { continue };
                let Value::Array(v) = v else { continue };
                let (Some(lvl), Some(secs)) = (v.first().and_then(value_i64), v.get(1).and_then(value_f64)) else { continue };
                if (1..=POTION_LEVELS).contains(&lvl) && secs > 0.0 {
                    s.active_potions.insert(pt.key, (lvl, secs.min(POTION_MAX_SECONDS)));
                }
            }
        }
        s.bought_period = d.get("bought_period").and_then(value_i64).unwrap_or(-1);
        if let Some(Value::Array(a)) = d.get("bought") {
            s.bought = a.iter().filter_map(|x| x.as_str().map(|s| s.to_string())).collect();
        }
        s
    }
}

impl GameState {
    // ---------------- shop ----------------
    pub fn shop_unlocked(&self) -> bool {
        self.rebirths >= SHOP_UNLOCK_REBIRTHS
    }

    fn shop_sync_period(&mut self, now: f64) -> i64 {
        let period = shop_period(now);
        if period != self.shop.bought_period {
            self.shop.bought_period = period;
            self.shop.bought.clear();
        }
        period
    }

    /// item = "dice:<key>" or "potion:<id>". True if it's in stock for THIS player now.
    pub fn shop_in_stock(&mut self, item: &str, now: f64) -> bool {
        let period = self.shop_sync_period(now);
        if self.shop.bought.iter().any(|b| b == item) {
            return false;
        }
        let Some((kind, key)) = item.split_once(':') else { return false };
        let stock = shop_stock(period);
        if kind == "dice" { stock.dice.contains(&key) } else { stock.potions.iter().any(|p| p == key) }
    }

    pub fn potion_price(&self, _kind: &str, level: i64) -> f64 {
        // without counting the active money potion
        let base_income = self.income_per_second() / self.potion_mult("money");
        POTION_MIN_PRICE.max(base_income * POTION_PRICE_SECONDS[(level - 1) as usize])
    }

    pub fn buy_dice(&mut self, key: &str, now: f64) -> bool {
        let Some(d) = dice_by_key(key) else { return false };
        if self.shop.dice_owned.contains(&d.key) || !self.shop_unlocked() {
            return false;
        }
        if !self.shop_in_stock(&format!("dice:{}", key), now) || self.coins < d.price {
            return false;
        }
        self.coins -= d.price;
        self.shop.dice_owned.push(d.key);
        self.shop.bought.push(format!("dice:{}", key));
        if self.shop.dice_equipped.is_none() {
            self.shop.dice_equipped = Some(d.key); // the 1st die bought is equipped right away
        }
        true
    }

    pub fn buy_potion(&mut self, kind: &str, level: i64, now: f64) -> bool {
        let pid = potion_id(kind, level);
        if potion_by_key(kind).is_none() || !self.shop_unlocked() {
            return false;
        }
        let price = self.potion_price(kind, level);
        if !self.shop_in_stock(&format!("potion:{}", pid), now) || self.coins < price {
            return false;
        }
        self.coins -= price;
        *self.shop.potions.entry(pid.clone()).or_insert(0) += 1;
        self.shop.bought.push(format!("potion:{}", pid));
        true
    }

    // ---------------- dice ----------------
    pub fn equip_dice(&mut self, key: &str) {
        if let Some(k) = self.shop.dice_owned.iter().copied().find(|k| *k == key) {
            self.shop.dice_equipped = if self.shop.dice_equipped == Some(k) { None } else { Some(k) };
        }
    }

    pub fn dice_luck_mult(&self) -> f64 {
        match self.shop.dice_equipped.and_then(dice_by_key) {
            Some(d) => 1.0 + d.luck,
            None => 1.0,
        }
    }

    pub fn roll_button_style(&self) -> Option<&'static DiceStyle> {
        self.shop.dice_equipped.and_then(dice_by_key).map(|d| &d.style)
    }

    // ---------------- potions ----------------
    /// Uses a potion from the inventory. With one of this kind already active: the same level adds its time; a
    /// higher level replaces it (keeping the time left + the new one's); a lower level isn't allowed (returns
    /// false) so the stronger one isn't wasted.
    pub fn use_potion(&mut self, kind: &str, level: i64) -> bool {
        let pid = potion_id(kind, level);
        let Some(pt) = potion_by_key(kind) else { return false };
        if self.shop.potions.get(&pid).copied().unwrap_or(0) <= 0 {
            return false;
        }
        let cur = self.shop.active_potions.get(pt.key).copied();
        if let Some((lvl, _)) = cur {
            if lvl > level {
                return false;
            }
        }
        let remaining = cur.map(|c| c.1).unwrap_or(0.0);
        self.shop.active_potions.insert(pt.key, (level, POTION_MAX_SECONDS.min(remaining + POTION_SECONDS)));
        let left = self.shop.potions.get(&pid).copied().unwrap_or(0) - 1;
        if left <= 0 {
            self.shop.potions.shift_remove(&pid);
        } else {
            self.shop.potions.insert(pid, left);
        }
        true
    }

    /// 5 potions of level N -> 1 of level N+1.
    pub fn combine_potions(&mut self, kind: &str, level: i64) -> bool {
        let pid = potion_id(kind, level);
        let have = self.shop.potions.get(&pid).copied().unwrap_or(0);
        if level >= POTION_LEVELS || have < POTION_COMBINE {
            return false;
        }
        if have - POTION_COMBINE <= 0 {
            self.shop.potions.shift_remove(&pid);
        } else {
            self.shop.potions.insert(pid, have - POTION_COMBINE);
        }
        *self.shop.potions.entry(potion_id(kind, level + 1)).or_insert(0) += 1;
        true
    }

    pub fn tick_potions(&mut self, dt: f64) {
        self.shop.active_potions.retain(|_, v| {
            v.1 -= dt;
            v.1 > 0.0
        });
    }

    pub fn potion_mult(&self, kind: &str) -> f64 {
        match (self.shop.active_potions.get(kind), potion_by_key(kind)) {
            (Some((lvl, _)), Some(pt)) => pt.effect[(*lvl - 1) as usize],
            _ => 1.0,
        }
    }
}
