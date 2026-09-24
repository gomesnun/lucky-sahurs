//! GameState: a complete save (pets, upgrades, traits, milestones, rebirths, daily missions,
//! offline earnings, economy) - core/game_state.py and its mixins.

use super::data::*;
use super::event::event_mult;
use crate::pyjson;
use crate::pyrand::PyRandom;
use crate::storage::{save_slot_path, value_f64, value_i64, value_truthy};
use indexmap::IndexMap;
use serde_json::{Map, Value, json};
use std::cell::{Cell, RefCell};
use std::collections::{BTreeSet, HashMap, HashSet};

pub type Pet = (usize, &'static str);

#[derive(Clone, Debug)]
pub struct DailyMission {
    pub mtype: &'static str,
    pub target: i64,
    pub reward: i64,
}

thread_local! {
    /// The module-level `random` used by rolls / traits.
    pub static RNG: RefCell<PyRandom> = RefCell::new(PyRandom::from_entropy());
}

pub fn rand_random() -> f64 {
    RNG.with(|r| r.borrow_mut().random())
}
pub fn rand_uniform(a: f64, b: f64) -> f64 {
    RNG.with(|r| r.borrow_mut().uniform(a, b))
}
pub fn rand_gauss(mu: f64, sigma: f64) -> f64 {
    RNG.with(|r| r.borrow_mut().gauss(mu, sigma))
}

/// A random number from a Poisson distribution (for the bulk rolls).
pub fn poisson(lam: f64) -> i64 {
    if lam <= 0.0 {
        return 0;
    }
    if lam < 30.0 {
        let limit = (-lam).exp();
        let (mut k, mut p) = (0i64, 1.0f64);
        loop {
            p *= rand_random();
            if p <= limit {
                return k;
            }
            k += 1;
        }
    }
    let v = crate::core::formatting::py_round(rand_gauss(lam, lam.sqrt()));
    if v <= 0.0 { 0 } else if v >= 9.0e18 { i64::MAX / 4 } else { v as i64 }
}

/// Python's RARITY_COUNTER: which lifetime counter each top rarity bumps.
fn rarity_counter<'a>(st: &'a mut GameState, rkey: &str) -> Option<&'a mut i64> {
    Some(match rkey {
        "secreto" => &mut st.total_secret_rolled,
        "divino" => &mut st.total_divine_rolled,
        "cosmico" => &mut st.total_cosmic_rolled,
        "transcendente" => &mut st.total_transcendent_rolled,
        "etereo" => &mut st.total_ethereal_rolled,
        "celestial" => &mut st.total_celestial_rolled,
        "absoluto" => &mut st.total_absolute_rolled,
        _ => return None,
    })
}

pub struct GameState {
    /// identity of this object (Python keeps references to a state inside network callbacks)
    pub obj_id: u64,
    pub slot: Option<i64>,
    pub coins: f64,
    pub owned: IndexMap<String, i64>,
    /// "{rarity_index}_{mutation}" ever rolled (for the Index; never shrinks)
    pub seen_pets: BTreeSet<String>,
    pub equipped: Vec<Pet>,
    pub avatar: Option<Pet>,
    /// the equipped title (core/titles.rs), shown to other players; None = no title
    pub title: Option<&'static str>,
    /// levels in UPGRADE_DEFS order
    pub upgrades: Vec<i64>,
    pub last_roll: Option<Pet>,
    pub total_rolls: i64,
    pub auto_on: bool,

    pub trait_charges: i64,
    pub owned_traits: BTreeSet<usize>,
    pub equipped_trait: Option<usize>,
    pub last_trait_roll: Option<usize>,
    pub last_trait_batch: Option<IndexMap<usize, i64>>,
    pub total_traits_rolled: i64,

    pub total_coins_earned: f64,
    pub playtime: f64,
    pub total_golden_rolled: i64,
    pub total_diamond_rolled: i64,
    pub total_secret_rolled: i64,
    pub total_divine_rolled: i64,
    pub total_cosmic_rolled: i64,
    pub total_transcendent_rolled: i64,
    pub total_rainbow_rolled: i64,
    pub total_ethereal_rolled: i64,
    pub total_celestial_rolled: i64,
    pub total_absolute_rolled: i64,
    pub milestones_claimed: HashSet<String>,
    ms_bonus: HashMap<&'static str, f64>,

    pub cyclic_roll_count: i64,
    pub cyclic_bonus_ready: bool,
    pub diamond_roll_count: i64,
    pub diamond_bonus_ready: bool,
    pub rainbow_roll_count: i64,
    pub rainbow_bonus_ready: bool,
    pub cycle_paused: IndexMap<&'static str, bool>,

    pub auto_equip_best_on: bool,
    pub auto_upgrade_on: bool,
    /// v3.0: the Auto Trait Roller switch
    pub auto_trait_on: bool,
    /// dice, potions and what was bought this period (core/shop.rs)
    pub shop: crate::core::shop::ShopState,
    /// with no bonus roll, the chances are the same for every roll of a frame: the Auto Roller keeps them here
    pub probs_cache: Option<Vec<f64>>,

    pub cloud_uid: Option<String>,
    pub cloud_base_time: Option<String>,
    pub dirty: bool,
    /// never saved: what still has to go to /users/{uid}/wallet/pets (see tick_wallet in game/trades.rs)
    pub wallet_pending: IndexMap<String, i64>,
    pub sync_conflict: bool,
    pub save_seq: i64,

    pub rebirths: i64,
    /// v3.0: how many Prestiges (0-5)
    pub prestige: i64,
    rb_cache: RefCell<Option<(i64, HashMap<&'static str, f64>)>>,

    pub last_seen: Option<f64>,

    pub daily_date: Option<String>,
    pub daily_missions: Vec<DailyMission>,
    pub daily_counts: IndexMap<String, i64>,
    pub daily_claimed: BTreeSet<usize>,
    /// v3.0: your settings (all but fullscreen), saved with the save so they follow your account to other PCs
    pub prefs: Option<Map<String, Value>>,
    /// v3.0.1: the season this save belongs to (a reset of everyone's progress starts a new one)
    pub season: i64,
    /// v3.0.1: this account's reset number when the save was made (an admin can reset one player)
    pub player_reset: i64,
    /// v3.0 weekly quests (reset Monday 00:00, Lisbon)
    pub weekly_week: Option<String>,
    pub weekly_missions: Vec<DailyMission>,
    pub weekly_counts: IndexMap<String, i64>,
    pub weekly_claimed: BTreeSet<usize>,
    /// cached "today" string (refreshed a few times per second at most)
    today_cache: Cell<(f64, [u8; 10])>,
}

/// v3.0: the day (and the quests) change at 00:00 Lisbon time for everyone, not at the PC's midnight.
pub fn today_str() -> String {
    lisbon_date(now_ts()).format("%Y-%m-%d").to_string()
}

/// Lisbon's offset from UTC at this moment, in seconds: +1 h in summer time (EU rule: last Sunday of March
/// 01:00 UTC to last Sunday of October 01:00 UTC), else 0.
pub fn lisbon_offset(utc: f64) -> i64 {
    use chrono::{Datelike, NaiveDate};
    let Some(dt) = chrono::DateTime::from_timestamp(utc.floor() as i64, 0) else { return 0 };
    let year = dt.year();
    let last_sunday = |month: u32| -> i64 {
        let first_next = if month == 12 { NaiveDate::from_ymd_opt(year + 1, 1, 1) } else { NaiveDate::from_ymd_opt(year, month + 1, 1) }.unwrap();
        let last = first_next.pred_opt().unwrap();
        let back = last.weekday().num_days_from_sunday() as i64;
        let d = last - chrono::Duration::days(back);
        d.and_hms_opt(1, 0, 0).unwrap().and_utc().timestamp()
    };
    let t = dt.timestamp();
    if t >= last_sunday(3) && t < last_sunday(10) { 3600 } else { 0 }
}

pub fn lisbon_date(utc: f64) -> chrono::NaiveDate {
    let local = utc.floor() as i64 + lisbon_offset(utc);
    chrono::DateTime::from_timestamp(local, 0).map(|d| d.date_naive()).unwrap_or_default()
}

/// "2026-W39": the ISO week (Monday to Sunday) of a "YYYY-MM-DD" day.
pub fn week_key(date: &str) -> String {
    use chrono::Datelike;
    match chrono::NaiveDate::parse_from_str(date, "%Y-%m-%d") {
        Ok(d) => {
            let w = d.iso_week();
            format!("{}-W{:02}", w.year(), w.week())
        }
        Err(_) => date.to_string(),
    }
}

/// Seconds until the next 00:00 in Lisbon (`weekly`: the next Monday 00:00).
pub fn secs_until_lisbon_reset(utc: f64, weekly: bool) -> f64 {
    use chrono::Datelike;
    let date = lisbon_date(utc);
    let days = if weekly { 7 - date.weekday().num_days_from_monday() as i64 } else { 1 };
    let next = (date + chrono::Duration::days(days)).and_hms_opt(0, 0, 0).unwrap().and_utc().timestamp();
    // that moment is local time: back to UTC with the offset in force then
    let at = next - lisbon_offset((next - 3600) as f64);
    (at as f64 - utc).max(0.0)
}

/// What claiming a quest gives right now: coins (minutes of your money/sec), trait charges (grow with your
/// Auto Roller) and, for weekly quests, a potion.
#[derive(Clone, Debug, PartialEq)]
pub struct QuestReward {
    pub coins: f64,
    pub charges: i64,
    pub potion: Option<(&'static str, i64)>,
}

static FAKE_TIME: std::sync::Mutex<Option<f64>> = std::sync::Mutex::new(None);

/// Freezes time.time() (screenshot tests).
pub fn set_fake_time(t: Option<f64>) {
    if let Ok(mut f) = FAKE_TIME.lock() {
        *f = t;
    }
}

/// time.perf_counter() (follows the fake clock in tests, like time.time()).
pub fn perf_counter() -> f64 {
    static START: std::sync::OnceLock<std::time::Instant> = std::sync::OnceLock::new();
    if let Ok(f) = FAKE_TIME.lock() {
        if let Some(t) = *f {
            return t;
        }
    }
    START.get_or_init(std::time::Instant::now).elapsed().as_secs_f64()
}

pub fn fake_time_active() -> bool {
    FAKE_TIME.lock().map(|f| f.is_some()).unwrap_or(false)
}

/// time.time()
pub fn now_ts() -> f64 {
    if let Ok(f) = FAKE_TIME.lock() {
        if let Some(t) = *f {
            return t;
        }
    }
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs_f64()).unwrap_or(0.0)
}

impl Default for GameState {
    fn default() -> Self {
        Self::new()
    }
}

impl GameState {
    pub fn new() -> GameState {
        let mut cp = IndexMap::new();
        cp.insert("golden", false);
        cp.insert("diamond", false);
        cp.insert("rainbow", false);
        static NEXT: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(1);
        GameState {
            obj_id: NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed),
            slot: None,
            coins: 0.0,
            owned: IndexMap::new(),
            seen_pets: BTreeSet::new(),
            equipped: Vec::new(),
            avatar: None,
            title: None,
            upgrades: vec![0; upgrade_defs().len()],
            last_roll: None,
            total_rolls: 0,
            auto_on: true,
            trait_charges: 0,
            owned_traits: BTreeSet::new(),
            equipped_trait: None,
            last_trait_roll: None,
            last_trait_batch: None,
            total_traits_rolled: 0,
            total_coins_earned: 0.0,
            playtime: 0.0,
            total_golden_rolled: 0,
            total_diamond_rolled: 0,
            total_secret_rolled: 0,
            total_divine_rolled: 0,
            total_cosmic_rolled: 0,
            total_transcendent_rolled: 0,
            total_rainbow_rolled: 0,
            total_ethereal_rolled: 0,
            total_celestial_rolled: 0,
            total_absolute_rolled: 0,
            milestones_claimed: HashSet::new(),
            ms_bonus: HashMap::new(),
            cyclic_roll_count: 0,
            cyclic_bonus_ready: false,
            diamond_roll_count: 0,
            diamond_bonus_ready: false,
            rainbow_roll_count: 0,
            rainbow_bonus_ready: false,
            cycle_paused: cp,
            auto_equip_best_on: true,
            auto_upgrade_on: true,
            auto_trait_on: true,
            shop: Default::default(),
            probs_cache: None,
            cloud_uid: None,
            cloud_base_time: None,
            dirty: false,
            wallet_pending: IndexMap::new(),
            sync_conflict: false,
            save_seq: 0,
            rebirths: 0,
            prestige: 0,
            rb_cache: RefCell::new(None),
            last_seen: None,
            daily_date: None,
            daily_missions: Vec::new(),
            daily_counts: IndexMap::new(),
            daily_claimed: BTreeSet::new(),
            prefs: None,
            season: 0,
            player_reset: 0,
            weekly_week: None,
            weekly_missions: Vec::new(),
            weekly_counts: IndexMap::new(),
            weekly_claimed: BTreeSet::new(),
            today_cache: Cell::new((-1.0, [0; 10])),
        }
    }

    fn today(&self) -> String {
        // datetime.date.today() - cheap cache (the clock is read at most every 0.25 s)
        let t = std::time::Instant::now();
        thread_local! { static START: std::time::Instant = std::time::Instant::now(); }
        let secs = START.with(|s| t.duration_since(*s).as_secs_f64());
        let (at, buf) = self.today_cache.get();
        if at >= 0.0 && secs - at < 0.25 {
            return String::from_utf8_lossy(&buf).to_string();
        }
        let s = today_str();
        let mut b = [0u8; 10];
        let bytes = s.as_bytes();
        if bytes.len() == 10 {
            b.copy_from_slice(bytes);
            self.today_cache.set((secs, b));
        }
        s
    }

    // ================================================================ economy
    pub fn max_slots(&self) -> i64 {
        BASE_SLOTS + self.upgrade_level("slots") + self.upgrade_level("slots_plus") + self.rebirth_bonus("slots") as i64 + self.prestige_def().map_or(0, |p| p.slots)
    }

    pub fn money_multiplier(&self) -> f64 {
        let base = (1.0 + MONEY_PER_LEVEL * self.upgrade_level("money") as f64)
            * (1.0 + MONEY_PRISM_PER_LEVEL * self.upgrade_level("money_prism") as f64)
            * (1.0 + MONEY_ULTRA_PER_LEVEL * self.upgrade_level("money_ultra") as f64);
        base * (1.0 + self.trait_buff("money"))
            * (1.0 + self.milestone_bonus("money"))
            * self.rebirth_money_mult()
            * (1.0 + self.rebirth_bonus("money"))
            * INCOME_SCALE
            * event_mult("money")
            * self.potion_mult("money")
            * self.prestige_def().map_or(1.0, |p| p.money)
    }

    pub fn auto_unlocked(&self) -> bool {
        self.upgrade_level("auto_unlock") >= 1 && self.rebirths >= AUTO_UNLOCK_REBIRTHS
    }

    pub fn auto_rolls_per_second(&self) -> f64 {
        if !self.auto_unlocked() {
            return 0.0;
        }
        let mut base = AUTO_BASE_RPS * (1.0 + AUTO_SPEED_PER_LEVEL * self.upgrade_level("auto_speed") as f64);
        base *= 1.0 + AUTO_TURBO_PER_LEVEL * self.upgrade_level("auto_turbo") as f64;
        base * (1.0 + self.trait_buff("auto_speed"))
            * (1.0 + self.milestone_bonus("auto_speed"))
            * (1.0 + self.rebirth_bonus("auto_speed"))
            * event_mult("speed")
            * self.potion_mult("speed")
            * self.prestige_def().map_or(1.0, |p| p.auto_speed)
    }

    pub fn pet_income(&self, rarity_index: usize, mutation_key: &str) -> f64 {
        rarities()[rarity_index].income * mutation(mutation_key).map(|m| m.mult).unwrap_or(1.0) * self.money_multiplier()
    }

    pub fn income_per_second(&self) -> f64 {
        let r = rarities();
        let mut total = 0.0;
        for (idx, m) in &self.equipped {
            total += r[*idx].income * mutation(m).map(|m| m.mult).unwrap_or(1.0);
        }
        total * self.money_multiplier()
    }

    // ================================================================ upgrades
    pub fn upgrade_level(&self, key: &str) -> i64 {
        self.upgrades[upgrade_index(key)]
    }

    /// Cost of going from level `lvl` to `lvl + 1` (Python int(base * mult ** lvl)).
    pub fn upgrade_cost_at(&self, key: &str, lvl: i64) -> i128 {
        let d = upgrade_def(key);
        if d.cost_mult == 1.0 {
            return d.base_cost as i128;
        }
        // up to half the levels it grows as always; from there on slower (see UPGRADE_SOFT_*)
        let soft_from = (d.max_level as f64 * UPGRADE_SOFT_START) as i64;
        let v = if lvl <= soft_from {
            d.base_cost * d.cost_mult.powf(lvl as f64)
        } else {
            d.base_cost * d.cost_mult.powf(soft_from as f64) * d.cost_mult.powf(UPGRADE_SOFT_EXP).powf((lvl - soft_from) as f64)
        };
        v.trunc() as i128
    }

    pub fn upgrade_cost(&self, key: &str) -> i128 {
        self.upgrade_cost_at(key, self.upgrade_level(key))
    }

    pub fn upgrade_locked_by(&self, key: &str) -> Option<&'static str> {
        let d = upgrade_def(key);
        let req = d.requires?;
        if self.upgrade_level(req) < d.requires_level {
            return Some(req);
        }
        None
    }

    pub fn upgrade_rebirths_needed(&self, key: &str) -> i64 {
        (upgrade_def(key).requires_rebirths - self.rebirths).max(0)
    }

    pub fn upgrade_available(&self, key: &str) -> bool {
        let d = upgrade_def(key);
        if self.upgrade_level(key) >= d.max_level {
            return false;
        }
        self.upgrade_locked_by(key).is_none() && self.upgrade_rebirths_needed(key) == 0
    }

    pub fn upgrade_affordable(&self, key: &str) -> bool {
        self.upgrade_available(key) && py_ge(self.coins, self.upgrade_cost(key))
    }

    pub fn affordable_upgrades_count(&self) -> usize {
        upgrade_defs().iter().filter(|d| self.upgrade_affordable(d.key)).count()
    }

    /// mode: 1, 10 or 0 for "max". Returns (levels, total cost, can pay).
    pub fn upgrade_bulk_quote(&self, key: &str, mode: i64) -> (i64, i128, bool) {
        let d = upgrade_def(key);
        let lvl = self.upgrade_level(key);
        let remaining = d.max_level - lvl;
        if remaining <= 0 {
            return (0, 0, false);
        }
        if mode == 0 {
            let (mut n, mut total) = (0i64, 0i128);
            while n < remaining {
                let c = self.upgrade_cost_at(key, lvl + n);
                if py_gt_int_float(total + c, self.coins) {
                    break;
                }
                total += c;
                n += 1;
            }
            if n == 0 {
                return (1, self.upgrade_cost_at(key, lvl), false);
            }
            return (n, total, true);
        }
        let n = mode.min(remaining);
        let total: i128 = (0..n).map(|i| self.upgrade_cost_at(key, lvl + i)).sum();
        (n, total, py_ge(self.coins, total))
    }

    pub fn buy_upgrade_bulk(&mut self, key: &str, mode: i64) -> i64 {
        if !self.upgrade_available(key) {
            return 0;
        }
        let (n, total, can_pay) = self.upgrade_bulk_quote(key, mode);
        if !can_pay || n <= 0 {
            return 0;
        }
        self.coins -= int_to_f64(total);
        let i = upgrade_index(key);
        self.upgrades[i] += n;
        if key == "auto_equip_unlock" {
            self.auto_equip_best_on = true;
            self.equip_best();
        }
        n
    }

    /// v3.0.1: the Golden Roll is an upgrade now (like the Diamond and Rainbow ones)
    pub fn golden_roll_unlocked(&self) -> bool {
        self.upgrade_level("golden_roll_unlock") >= 1
    }
    pub fn golden_roll_every(&self) -> i64 {
        GOLDEN_ROLL_MIN.max(GOLDEN_ROLL_EVERY - self.upgrade_level("cyclic_every"))
    }
    pub fn golden_roll_mult(&self) -> f64 {
        GOLDEN_ROLL_MULT + 1.0 * self.upgrade_level("cyclic_power") as f64
    }
    pub fn rolls_until_golden_roll(&self) -> i64 {
        if self.cyclic_bonus_ready {
            return 0;
        }
        (self.golden_roll_every() - self.cyclic_roll_count).max(0)
    }
    pub fn diamond_roll_unlocked(&self) -> bool {
        self.upgrade_level("diamond_roll_unlock") >= 1
    }
    pub fn diamond_roll_every(&self) -> i64 {
        DIAMOND_ROLL_EVERY_MIN.max(DIAMOND_ROLL_EVERY_BASE - DIAMOND_ROLL_EVERY_STEP * self.upgrade_level("diamond_roll_every"))
    }
    pub fn diamond_roll_mult(&self) -> f64 {
        DIAMOND_ROLL_MULT_BASE + DIAMOND_ROLL_MULT_STEP * self.upgrade_level("diamond_roll_power") as f64
    }
    pub fn rolls_until_diamond_roll(&self) -> Option<i64> {
        if !self.diamond_roll_unlocked() {
            return None;
        }
        if self.diamond_bonus_ready {
            return Some(0);
        }
        Some((self.diamond_roll_every() - self.diamond_roll_count).max(0))
    }
    pub fn rainbow_roll_unlocked(&self) -> bool {
        self.upgrade_level("rainbow_roll_unlock") >= 1
    }
    pub fn rainbow_roll_every(&self) -> i64 {
        RAINBOW_ROLL_EVERY_MIN.max(RAINBOW_ROLL_EVERY_BASE - RAINBOW_ROLL_EVERY_STEP * self.upgrade_level("rainbow_roll_every"))
    }
    pub fn rainbow_roll_mult(&self) -> f64 {
        RAINBOW_ROLL_MULT_BASE + RAINBOW_ROLL_MULT_STEP * self.upgrade_level("rainbow_roll_power") as f64
    }
    pub fn rolls_until_rainbow_roll(&self) -> Option<i64> {
        if !self.rainbow_roll_unlocked() {
            return None;
        }
        if self.rainbow_bonus_ready {
            return Some(0);
        }
        Some((self.rainbow_roll_every() - self.rainbow_roll_count).max(0))
    }
    pub fn is_cycle_paused(&self, kind: &str) -> bool {
        self.cycle_paused.get(kind).copied().unwrap_or(false)
    }
    pub fn toggle_cycle_pause(&mut self, kind: &str) {
        if let Some(v) = self.cycle_paused.get_mut(kind) {
            *v = !*v;
        }
    }
    pub fn auto_equip_unlocked(&self) -> bool {
        self.upgrade_level("auto_equip_unlock") >= 1
    }

    // ---------------- auto upgrader ----------------
    pub fn auto_upgrade_unlocked(&self) -> bool {
        self.upgrade_level("auto_upgrade_unlock") >= 1
    }

    /// Buys, one level at a time, the cheapest upgrade it can pay for (up to AUTO_UPGRADE_MAX_BUYS).
    /// Returns how many levels it bought.
    pub fn auto_upgrade_step(&mut self) -> i64 {
        if !(self.auto_upgrade_unlocked() && self.auto_upgrade_on) {
            return 0;
        }
        let mut bought = 0;
        while bought < AUTO_UPGRADE_MAX_BUYS {
            let mut best: Option<(&'static str, i128)> = None;
            for key in upgrade_order() {
                if key == "auto_upgrade_unlock" || !self.upgrade_available(key) {
                    continue;
                }
                let cost = self.upgrade_cost(key);
                if py_ge(self.coins, cost) && best.map(|b| cost < b.1).unwrap_or(true) {
                    best = Some((key, cost));
                }
            }
            match best {
                Some((key, _)) if self.buy_upgrade_bulk(key, 1) > 0 => bought += 1,
                _ => break,
            }
        }
        bought
    }

    // ================================================================ pets
    pub fn equip_best(&mut self) {
        let mut options: Vec<(f64, usize, &'static str, i64)> = Vec::new();
        for r_idx in 0..rarities().len() {
            for m in MUT_ORDER {
                let owned = self.count_owned(r_idx, m);
                if owned > 0 {
                    options.push((self.pet_income(r_idx, m), r_idx, m, owned));
                }
            }
        }
        // stable sort by -income (Python's sort is stable)
        options.sort_by(|a, b| b.0.partial_cmp(&a.0).unwrap_or(std::cmp::Ordering::Equal));
        let slots = self.max_slots();
        let mut new_eq: Vec<Pet> = Vec::new();
        for (_inc, r_idx, m, mut owned) in options {
            while owned > 0 && (new_eq.len() as i64) < slots {
                new_eq.push((r_idx, m));
                owned -= 1;
            }
            if new_eq.len() as i64 >= slots {
                break;
            }
        }
        self.equipped = new_eq;
    }

    pub fn luck_multiplier(&self, rarity_index: usize) -> f64 {
        let tier = rarities()[rarity_index].tier;
        if tier < 2 {
            return 1.0;
        }
        let mut m = 1.0 + LUCK_PER_LEVEL * self.upgrade_level("luck") as f64;
        let prism = self.upgrade_level("luck_prism") as f64;
        if tier >= 3 {
            m *= 1.0 + LUCK_PRISM_PER_LEVEL * prism;
        }
        if tier >= 5 {
            m *= 1.0 + LUCK_PRISM_PER_LEVEL * prism;
        }
        if tier >= TIER_SECRET {
            m *= 1.0 + LUCK_PRISM_PER_LEVEL * prism;
        }
        if tier >= 6 {
            m *= 1.0 + LUCK_COSMIC_PER_LEVEL * self.upgrade_level("luck_cosmic") as f64;
        }
        if tier >= TIER_DIVINE {
            m *= 1.0 + LUCK_DIVINE_PER_LEVEL * self.upgrade_level("luck_divine") as f64;
        }
        // ---- v2.7: end-game luck ----
        m *= 1.0 + LUCK_2_PER_LEVEL * self.upgrade_level("luck_2") as f64;
        m *= 1.0 + LUCK_ULTRA_PER_LEVEL * self.upgrade_level("luck_ultra") as f64;
        let prism2 = 1.0 + LUCK_PRISM_2_PER_LEVEL * self.upgrade_level("luck_prism_2") as f64;
        for start in [TIER_DIVINE, TIER_COSMIC, TIER_TRANSCENDENT, TIER_ETHEREAL] {
            if tier >= start {
                m *= prism2;
            }
        }
        for (key, per_level) in LUCK_TIER_PER_LEVEL {
            if tier >= tier_index(key) {
                m *= 1.0 + per_level * self.upgrade_level(&format!("luck_tier_{}", key)) as f64;
            }
        }
        m *= 1.0 + self.trait_buff("luck");
        if tier >= TIER_SECRET {
            m *= 1.0 + self.trait_buff("secret_luck");
        }
        m *= 1.0 + self.milestone_bonus("luck");
        m *= 1.0 + self.milestone_bonus("index_luck");
        if tier >= TIER_SECRET {
            m *= 1.0 + self.milestone_bonus("secret_luck");
        }
        if tier >= TIER_DIVINE {
            m *= 1.0 + self.milestone_bonus("divine_luck");
        }
        if tier >= TIER_COSMIC {
            m *= 1.0 + self.milestone_bonus("cosmic_luck");
        }
        if tier >= TIER_TRANSCENDENT {
            m *= 1.0 + self.milestone_bonus("transcendent_luck");
        }
        if tier >= TIER_ETHEREAL {
            m *= 1.0 + self.milestone_bonus("ethereal_luck");
        }
        if tier >= TIER_CELESTIAL {
            m *= 1.0 + self.milestone_bonus("celestial_luck");
        }
        if tier >= TIER_ABSOLUTE {
            m *= 1.0 + self.milestone_bonus("absolute_luck");
        }
        m *= self.rebirth_luck_mult();
        m *= 1.0 + self.rebirth_bonus("luck");
        if tier >= TIER_SECRET {
            m *= 1.0 + self.rebirth_bonus("secret_luck");
        }
        // v2.9: a bit less (see LUCK_BONUS_SCALE). The global event, the die, the luck potion and the
        // Golden/Diamond/Rainbow Rolls are NOT here: they are "flat" luck (see flat_luck / pet_probs)
        // v3.0: the Prestige luck multiplies everything, at full strength
        (1.0 + (m - 1.0) * LUCK_BONUS_SCALE) * self.prestige_def().map_or(1.0, |p| p.luck)
    }

    /// "Flat" luck (like Sol's RNG): admin event x equipped die x luck potion x the Golden/Diamond/Rainbow Roll
    /// bonus. Divides each rarity's "1 in N" - x1M really turns a 1 in 500 B into 1 in 500 K.
    pub fn flat_luck(&self, bonus_mult: f64) -> f64 {
        (event_mult("luck") * self.dice_luck_mult() * self.potion_mult("luck") * bonus_mult).max(1.0)
    }

    /// Chance of each pet on a roll (sums to 1).
    /// 1) The normal luck (luck_multiplier) gives the base distribution, by weights.
    /// 2) The flat luck F (flat_luck) looks at the rarities from the RAREST to the most common (Rare or better
    ///    only): each one comes out with min(1, F x its base chance); if none does, it falls to Common/Uncommon.
    pub fn pet_probs(&self, bonus_mult: f64, weights: Option<&[f64]>) -> Vec<f64> {
        let owned_w;
        let weights = match weights {
            Some(w) => w,
            None => {
                owned_w = self.roll_weights(true);
                &owned_w
            }
        };
        let mut total: f64 = weights.iter().sum();
        if total == 0.0 {
            total = 1.0;
        }
        let base: Vec<f64> = weights.iter().map(|w| w / total).collect();
        let flat = self.flat_luck(bonus_mult);
        if flat <= 1.0 {
            return base;
        }
        let r = rarities();
        let n_tiers = RARITY_TIERS.len();
        let mut tier_p = vec![0.0f64; n_tiers];
        let mut tier_has = vec![false; n_tiers];
        for (i, p) in base.iter().enumerate() {
            tier_p[r[i].tier] += p;
            tier_has[r[i].tier] = true;
        }
        let mut out = vec![0.0f64; base.len()];
        let mut remaining = 1.0;
        for t in (0..n_tiers).rev() {
            if !tier_has[t] || t < 2 || remaining <= 0.0 || tier_p[t] <= 0.0 {
                continue;
            }
            let take = remaining * (flat * tier_p[t]).min(1.0);
            for i in 0..base.len() {
                if r[i].tier == t {
                    out[i] = take * base[i] / tier_p[t];
                }
            }
            remaining -= take;
        }
        let low: f64 = (0..n_tiers).filter(|&t| t < 2 && tier_has[t]).map(|t| tier_p[t]).sum();
        if remaining > 0.0 && low > 0.0 {
            for i in 0..base.len() {
                if r[i].tier < 2 {
                    out[i] = remaining * base[i] / low;
                }
            }
        }
        out
    }

    /// Weights of the NORMAL luck (without the flat luck: see pet_probs).
    pub fn roll_weights(&self, with_luck: bool) -> Vec<f64> {
        // luck only depends on the RARITY: computed once per rarity, not per pet
        let mut tier_mult: Vec<Option<f64>> = vec![None; RARITY_TIERS.len()];
        rarities()
            .iter()
            .enumerate()
            .map(|(i, r)| {
                let mut w = r.share / r.one_in;
                if with_luck {
                    let m = match tier_mult[r.tier] {
                        Some(m) => m,
                        None => {
                            let m = self.luck_multiplier(i);
                            tier_mult[r.tier] = Some(m);
                            m
                        }
                    };
                    w *= m;
                }
                w
            })
            .collect()
    }

    /// (Golden, Diamond, Rainbow): the REAL chance of each mutation per roll.
    pub fn mutation_chances(&self) -> (f64, f64, f64) {
        let mut g = 0.0;
        let mut d = 0.0;
        let mut r = 0.0;
        let base_mult = 1.0 + self.trait_buff("mutation");
        let golden_mult = base_mult * (1.0 + self.milestone_bonus("golden_luck")) * (1.0 + self.rebirth_bonus("golden_luck"));
        let diamond_mult = base_mult * (1.0 + self.milestone_bonus("diamond_luck")) * (1.0 + self.rebirth_bonus("diamond_luck"));
        if self.upgrade_level("golden_unlock") >= 1 {
            g = (GOLDEN_BASE
                + GOLDEN_STEP_1 * self.upgrade_level("golden_chance") as f64
                + GOLDEN_STEP_2 * self.upgrade_level("golden_chance_2") as f64)
                * golden_mult;
        }
        if self.upgrade_level("diamond_unlock") >= 1 {
            d = (DIAMOND_BASE
                + DIAMOND_STEP_1 * self.upgrade_level("diamond_chance") as f64
                + DIAMOND_STEP_2 * self.upgrade_level("diamond_chance_2") as f64)
                * diamond_mult;
        }
        if self.upgrade_level("rainbow_unlock") >= 1 {
            let rainbow_mult = base_mult * (1.0 + self.milestone_bonus("rainbow_luck"));
            r = (RAINBOW_BASE
                + RAINBOW_STEP_1 * self.upgrade_level("rainbow_chance") as f64
                + RAINBOW_STEP_2 * self.upgrade_level("rainbow_chance_2") as f64)
                * rainbow_mult;
        }
        (cap_chance(g, GOLDEN_MAX_CHANCE), cap_chance(d, DIAMOND_MAX_CHANCE), cap_chance(r, RAINBOW_MAX_CHANCE))
    }

    /// REAL chance of this pet coming out WITH this mutation, per roll, with your current luck (upgrades + trait +
    /// milestones + event/die/potion) and mutation chances. Doesn't count the temporary Golden / Diamond /
    /// Rainbow Roll bonus. Callers in a loop (Index) pass probs and muts already computed.
    pub fn combined_chance(&self, rarity_index: usize, mutation_key: &str, probs: Option<&[f64]>, muts: Option<(f64, f64, f64)>) -> f64 {
        let owned_p;
        let probs = match probs {
            Some(p) => p,
            None => {
                owned_p = self.pet_probs(1.0, None);
                &owned_p
            }
        };
        let (g, d, r) = muts.unwrap_or_else(|| self.mutation_chances());
        probs[rarity_index] * mutation_factor(mutation_key, g, d, r)
    }

    /// Returns (rarity_index, mutation, gained_charge, bonus_active)
    pub fn roll(&mut self) -> (usize, &'static str, bool, bool) {
        let golden_paused = self.is_cycle_paused("golden");
        let diamond_paused = self.is_cycle_paused("diamond");
        let rainbow_paused = self.is_cycle_paused("rainbow");
        let golden_active = self.cyclic_bonus_ready && self.golden_roll_unlocked() && !golden_paused;
        let diamond_active = self.diamond_bonus_ready && self.diamond_roll_unlocked() && !diamond_paused;
        let rainbow_active = self.rainbow_bonus_ready && self.rainbow_roll_unlocked() && !rainbow_paused;
        let bonus_active = golden_active || diamond_active || rainbow_active;

        let mut mults = Vec::new();
        if golden_active {
            mults.push(self.golden_roll_mult());
        }
        if diamond_active {
            mults.push(self.diamond_roll_mult());
        }
        if rainbow_active {
            mults.push(self.rainbow_roll_mult());
        }
        let bonus_mult: f64 = if mults.is_empty() { 1.0 } else { mults.iter().sum() };

        // with no bonus the chances are the same for every roll of the frame: the Auto Roller keeps them
        // (probs_cache) instead of computing them 50 times
        let computed;
        let probs: &[f64] = match (&self.probs_cache, bonus_mult == 1.0) {
            (Some(c), true) => c,
            _ => {
                computed = self.pet_probs(bonus_mult, None);
                &computed
            }
        };
        let roll_val = rand_random();
        let mut cum = 0.0;
        let mut rarity_index = rarities().len() - 1;
        for (i, p) in probs.iter().enumerate() {
            cum += p;
            if roll_val <= cum {
                rarity_index = i;
                break;
            }
        }

        let (g, d, r) = self.mutation_chances();
        let mut m = "normal";
        if r > 0.0 && rand_random() < r {
            m = "rainbow";
        } else if d > 0.0 && rand_random() < d {
            m = "diamond";
        } else if g > 0.0 && rand_random() < g {
            m = "golden";
        }

        self.record_pets(rarity_index, m, 1);
        self.last_roll = Some((rarity_index, m));
        self.total_rolls += 1;
        self.daily_add_progress("rolls", 1);

        if golden_active {
            self.cyclic_bonus_ready = false;
            self.cyclic_roll_count = 0;
        } else if !golden_paused && self.golden_roll_unlocked() {
            self.cyclic_roll_count += 1;
            if self.cyclic_roll_count >= self.golden_roll_every() {
                self.cyclic_roll_count = 0;
                self.cyclic_bonus_ready = true;
            }
        }
        if self.diamond_roll_unlocked() {
            if diamond_active {
                self.diamond_bonus_ready = false;
                self.diamond_roll_count = 0;
            } else if !diamond_paused {
                self.diamond_roll_count += 1;
                if self.diamond_roll_count >= self.diamond_roll_every() {
                    self.diamond_roll_count = 0;
                    self.diamond_bonus_ready = true;
                }
            }
        }
        if self.rainbow_roll_unlocked() {
            if rainbow_active {
                self.rainbow_bonus_ready = false;
                self.rainbow_roll_count = 0;
            } else if !rainbow_paused {
                self.rainbow_roll_count += 1;
                if self.rainbow_roll_count >= self.rainbow_roll_every() {
                    self.rainbow_roll_count = 0;
                    self.rainbow_bonus_ready = true;
                }
            }
        }

        let mut gained = false;
        if rand_random() < self.trait_charge_chance() {
            self.trait_charges += 1;
            gained = true;
        }
        (rarity_index, m, gained, bonus_active)
    }

    /// Adds 'count' pets of this kind to the collection and updates the counters (milestones, missions, wallet).
    fn record_pets(&mut self, rarity_index: usize, m: &'static str, count: i64) {
        let key = format!("{}_{}", rarity_index, m);
        *self.owned.entry(key.clone()).or_insert(0) += count;
        *self.wallet_pending.entry(key.clone()).or_insert(0) += count; // for the online wallet (trades)
        self.seen_pets.insert(key); // stays in the Index forever, even if owned drops to 0 (trade/sale)
        let rkey = rarities()[rarity_index].key;
        self.daily_add_progress(&format!("rarity_{}", rkey), count);
        match m {
            "golden" => {
                self.total_golden_rolled += count;
                self.daily_add_progress("golden", count);
            }
            "diamond" => {
                self.total_diamond_rolled += count;
                self.daily_add_progress("diamond", count);
            }
            "rainbow" => {
                self.total_rainbow_rolled += count;
                self.daily_add_progress("diamond", count); // a Rainbow also counts for the "Diamond" mission
                self.daily_add_progress("rainbow", count);
            }
            _ => {}
        }
        if let Some(c) = rarity_counter(self, rkey) {
            *c += count;
        }
    }

    /// n rolls at once, by statistics (for a very fast Auto Roller - e.g. a x1M speed event - where rolling one
    /// by one is impossible). Each pet+mutation comes out a Poisson-drawn number of times with the right chance.
    /// Doesn't use the Golden/Diamond/Rainbow Roll bonuses, but the cycles advance.
    /// Returns (best pet, mutation, trait charges).
    pub fn roll_bulk(&mut self, n: i64) -> (Option<Pet>, i64) {
        if n <= 0 {
            return (None, 0);
        }
        let probs = self.pet_probs(1.0, None);
        let (g, d, r) = self.mutation_chances();
        let order = pet_order();
        let mut best: Option<((usize, usize), usize, &'static str)> = None;
        for (i, p) in probs.iter().enumerate() {
            if *p <= 0.0 {
                continue;
            }
            for (mi, m) in MUT_ORDER.iter().enumerate() {
                let k = poisson(n as f64 * p * mutation_factor(m, g, d, r));
                if k > 0 {
                    self.record_pets(i, m, k);
                    let rank = order.iter().position(|&x| x == i).unwrap_or(0);
                    let score = (rank, mi);
                    if best.map(|b| score > b.0).unwrap_or(true) {
                        best = Some((score, i, m));
                    }
                }
            }
        }
        self.total_rolls += n;
        self.daily_add_progress("rolls", n);
        // cycles: they advance n rolls (the bonus gets ready, and waits for the next normal roll)
        let golden_every = self.golden_roll_every();
        let diamond_every = self.diamond_roll_every();
        let rainbow_every = self.rainbow_roll_every();
        let diamond_on = self.diamond_roll_unlocked();
        let rainbow_on = self.rainbow_roll_unlocked();
        let paused = (self.is_cycle_paused("golden"), self.is_cycle_paused("diamond"), self.is_cycle_paused("rainbow"));
        let adv = |unlocked: bool, is_paused: bool, count: &mut i64, ready: &mut bool, every: i64| {
            if !unlocked || is_paused || *ready {
                return;
            }
            let total = *count + n;
            if total >= every {
                *ready = true;
                *count = 0;
            } else {
                *count = total;
            }
        };
        let golden_on = self.golden_roll_unlocked();
        adv(golden_on, paused.0, &mut self.cyclic_roll_count, &mut self.cyclic_bonus_ready, golden_every);
        adv(diamond_on, paused.1, &mut self.diamond_roll_count, &mut self.diamond_bonus_ready, diamond_every);
        adv(rainbow_on, paused.2, &mut self.rainbow_roll_count, &mut self.rainbow_bonus_ready, rainbow_every);
        let charges = poisson(n as f64 * self.trait_charge_chance());
        self.trait_charges += charges;
        match best {
            Some((_, i, m)) => {
                self.last_roll = Some((i, m));
                (Some((i, m)), charges)
            }
            None => (None, charges),
        }
    }

    pub fn owned_of_rarity(&self, rarity_key: &str) -> i64 {
        let r = rarities();
        let mut total = 0;
        for (k, v) in &self.owned {
            let first = k.split('_').next().unwrap_or("");
            if let Ok(i) = first.parse::<i64>() {
                // Python negative indexes wrap around the list
                let idx = if i < 0 { i + r.len() as i64 } else { i };
                if idx >= 0 && (idx as usize) < r.len() && r[idx as usize].key == rarity_key {
                    total += *v;
                }
            }
        }
        total
    }

    /// How many DIFFERENT Index cards (pet + mutation) you ever had - even if you have none of them now
    /// (traded or sold). Repeats don't count.
    pub fn indexed_pets_count(&self) -> i64 {
        self.seen_pets.len() as i64
    }

    /// Whether this pet+mutation ever came out (it stays in the Index forever, even with none left).
    pub fn is_indexed(&self, rarity_index: usize, m: &str) -> bool {
        self.seen_pets.contains(&format!("{}_{}", rarity_index, m))
    }

    pub fn count_owned(&self, rarity_index: usize, m: &str) -> i64 {
        self.owned.get(&format!("{}_{}", rarity_index, m)).copied().unwrap_or(0)
    }

    pub fn equipped_count(&self, rarity_index: usize, m: &str) -> i64 {
        self.equipped.iter().filter(|p| p.0 == rarity_index && p.1 == m).count() as i64
    }

    pub fn equip_add(&mut self, rarity_index: usize, m: &str) -> bool {
        if self.equipped_count(rarity_index, m) < self.count_owned(rarity_index, m) && (self.equipped.len() as i64) < self.max_slots() {
            self.equipped.push((rarity_index, mut_key(m)));
            return true;
        }
        false
    }

    pub fn equip_remove_one(&mut self, rarity_index: usize, m: &str) -> bool {
        for i in (0..self.equipped.len()).rev() {
            if self.equipped[i].0 == rarity_index && self.equipped[i].1 == m {
                self.equipped.remove(i);
                return true;
            }
        }
        false
    }

    // ---------------- selling ----------------
    /// What ONE pet sells for: a few seconds of its money/sec (see PET_SELL_SECONDS).
    pub fn sell_price(&self, rarity_index: usize, m: &str) -> f64 {
        self.pet_income(rarity_index, m) * PET_SELL_SECONDS
    }

    /// Sells 'amount' pets of this kind (asking for more than you have sells them all). If you end up with fewer
    /// than you have equipped, the extra ones are unequipped. Returns (how many were sold, money earned).
    pub fn sell_pets(&mut self, rarity_index: usize, m: &str, amount: i64) -> (i64, f64) {
        let key = format!("{}_{}", rarity_index, m);
        let sold = amount.min(self.count_owned(rarity_index, m)).max(0);
        if sold <= 0 {
            return (0, 0.0);
        }
        let gain = self.sell_price(rarity_index, m) * sold as f64;
        let left = self.count_owned(rarity_index, m) - sold;
        if left > 0 {
            self.owned.insert(key.clone(), left);
        } else {
            self.owned.shift_remove(&key); // stays in the Index anyway (seen_pets)
        }
        while self.equipped_count(rarity_index, m) > left {
            self.equip_remove_one(rarity_index, m);
        }
        // the online wallet (trades) loses these pets too (see tick_wallet)
        *self.wallet_pending.entry(key).or_insert(0) -= sold;
        self.coins += gain;
        self.total_coins_earned += gain;
        (sold, gain)
    }

    pub fn remove_slot_at(&mut self, index: usize) {
        if index < self.equipped.len() {
            self.equipped.remove(index);
        }
    }

    // ================================================================ traits
    pub fn trait_buff(&self, key: &str) -> f64 {
        match self.equipped_trait {
            None => 0.0,
            Some(i) => TRAITS[i].buff(key),
        }
    }

    pub fn trait_charge_chance(&self) -> f64 {
        let mut c = (1.0 / TRAIT_CHARGE_ONE_IN) * (1.0 + self.trait_buff("charge_chance"));
        c *= 1.0 + 0.06 * self.upgrade_level("trait_charge_luck") as f64;
        c *= 1.0 + 0.10 * self.upgrade_level("trait_charge_luck_2") as f64;
        c *= 1.0 + self.rebirth_bonus("charge_chance");
        c *= self.prestige_def().map_or(1.0, |p| p.charge_chance);
        c.min(1.0)
    }

    pub fn trait_roll_weights(&self) -> Vec<f64> {
        let rarity_mult = (1.0 + 0.09 * self.upgrade_level("trait_rarity_luck") as f64)
            * (1.0 + 0.15 * self.upgrade_level("trait_rarity_luck_2") as f64)
            * (1.0 + self.milestone_bonus("trait_luck"))
            * (1.0 + self.rebirth_bonus("trait_luck"));
        TRAITS
            .iter()
            .enumerate()
            .map(|(i, t)| {
                let mut w = 1.0 / t.one_in;
                if i >= 2 {
                    w *= rarity_mult;
                }
                w
            })
            .collect()
    }

    pub fn trait_chance(&self, idx: usize) -> f64 {
        let w = self.trait_roll_weights();
        let total: f64 = w.iter().sum();
        if total != 0.0 { w[idx] / total } else { 0.0 }
    }

    pub fn roll_trait(&mut self) -> Option<usize> {
        if self.trait_charges <= 0 {
            return None;
        }
        self.trait_charges -= 1;
        let w = self.trait_roll_weights();
        let total: f64 = w.iter().sum();
        let roll_val = rand_random() * total;
        let mut cum = 0.0;
        let mut idx = TRAITS.len() - 1;
        for (i, x) in w.iter().enumerate() {
            cum += x;
            if roll_val <= cum {
                idx = i;
                break;
            }
        }
        self.owned_traits.insert(idx);
        self.last_trait_roll = Some(idx);
        self.total_traits_rolled += 1;
        self.daily_add_progress("traits", 1);
        Some(idx)
    }

    /// Spends n charges at once and returns {trait_index: count}. Up to TRAIT_EXACT_MAX one by one; above that
    /// (e.g. 600M charges after a speed event) each trait comes out a Poisson-drawn number of times - rolling
    /// 600M one by one froze the game until it closed.
    pub fn roll_traits_bulk(&mut self, n: i64) -> IndexMap<usize, i64> {
        let n = n.min(self.trait_charges);
        let mut results: IndexMap<usize, i64> = IndexMap::new();
        if n <= 0 {
            return results;
        }
        if n <= TRAIT_EXACT_MAX {
            for _ in 0..n {
                if let Some(idx) = self.roll_trait() {
                    *results.entry(idx).or_insert(0) += 1;
                }
            }
            return results;
        }
        let weights = self.trait_roll_weights();
        let total: f64 = weights.iter().sum();
        for i in 1..TRAITS.len() {
            let k = poisson(n as f64 * weights[i] / total);
            if k > 0 {
                results.insert(i, k);
            }
        }
        // the 1st trait (the most common) takes the rest, so the sum is exactly n
        let rest = n - results.values().sum::<i64>();
        if rest > 0 {
            results.insert(0, rest);
        } else if rest < 0 {
            let v = results.get(&0).copied().unwrap_or(0) + rest;
            if v <= 0 {
                results.shift_remove(&0);
            } else {
                results.insert(0, v);
            }
        }
        self.trait_charges -= n;
        self.total_traits_rolled += n;
        self.daily_add_progress("traits", n);
        self.owned_traits.extend(results.keys().copied());
        self.last_trait_roll = results.keys().copied().max(); // the rarest one that came out
        results
    }

    pub fn equip_trait(&mut self, idx: usize) -> bool {
        if !self.owned_traits.contains(&idx) {
            return false;
        }
        self.equipped_trait = if self.equipped_trait == Some(idx) { None } else { Some(idx) };
        true
    }

    // ================================================================ milestones
    pub fn milestone_metric(&self, category: &str) -> f64 {
        match category {
            "indexed_pets" => self.indexed_pets_count() as f64,
            "secret_rolled" => self.total_secret_rolled as f64,
            "divine_rolled" => self.total_divine_rolled as f64,
            "cosmic_rolled" => self.total_cosmic_rolled as f64,
            "transcendent_rolled" => self.total_transcendent_rolled as f64,
            "rainbow_rolled" => self.total_rainbow_rolled as f64,
            "ethereal_rolled" => self.total_ethereal_rolled as f64,
            "celestial_rolled" => self.total_celestial_rolled as f64,
            "absolute_rolled" => self.total_absolute_rolled as f64,
            "rolls" => self.total_rolls as f64,
            "traits_rolled" => self.total_traits_rolled as f64,
            "golden_rolled" => self.total_golden_rolled as f64,
            "diamond_rolled" => self.total_diamond_rolled as f64,
            "coins" => self.total_coins_earned,
            "playtime" => self.playtime,
            _ => 0.0,
        }
    }

    pub fn recalc_milestone_bonus(&mut self) {
        let mut totals: HashMap<&'static str, f64> = HashMap::new();
        for cat in MILESTONES.iter() {
            for (i, (_th, v)) in cat.tiers.iter().enumerate() {
                if self.milestones_claimed.contains(&format!("{}:{}", cat.key, i)) {
                    *totals.entry(cat.reward_type).or_insert(0.0) += v;
                }
            }
        }
        self.ms_bonus = totals;
    }

    pub fn milestone_bonus(&self, rt: &str) -> f64 {
        self.ms_bonus.get(rt).copied().unwrap_or(0.0)
    }

    /// Returns newly claimed (category, index, threshold, value)
    pub fn check_milestones(&mut self) -> Vec<(&'static str, usize, f64, f64)> {
        let mut newly = Vec::new();
        for cat in MILESTONES.iter() {
            let metric = self.milestone_metric(cat.key);
            for (i, (th, v)) in cat.tiers.iter().enumerate() {
                let key = format!("{}:{}", cat.key, i);
                if !self.milestones_claimed.contains(&key) && metric >= *th {
                    self.milestones_claimed.insert(key);
                    newly.push((cat.key, i, *th, *v));
                }
            }
        }
        if !newly.is_empty() {
            self.recalc_milestone_bonus();
        }
        newly
    }

    // ================================================================ rebirths
    pub fn rebirth_cost_n(&self, n: i64) -> f64 {
        REBIRTH_BASE_COST * REBIRTH_COST_MULT.powf(n as f64)
    }
    pub fn rebirth_cost(&self) -> f64 {
        self.rebirth_cost_n(self.rebirths)
    }
    pub fn rebirth_available(&self) -> bool {
        self.coins >= self.rebirth_cost() && !self.rebirth_locked()
    }

    /// v3.0.1: at 10 / 15 / 20 / 30 / 40 Rebirths you have to Prestige before rebirthing again (after Prestige V
    /// there's no limit).
    pub fn rebirth_locked(&self) -> bool {
        self.next_prestige().is_some_and(|p| self.rebirths >= p.need)
    }
    pub fn rebirth_money_mult(&self) -> f64 {
        1.0 + REBIRTH_MONEY_PER * self.rebirths as f64
    }
    pub fn rebirth_luck_mult(&self) -> f64 {
        1.0 + REBIRTH_LUCK_PER * self.rebirths as f64
    }

    pub fn rebirth_bonus(&self, kind: &str) -> f64 {
        let mut cache = self.rb_cache.borrow_mut();
        let stale = match &*cache {
            None => true,
            Some((n, _)) => *n != self.rebirths,
        };
        if stale {
            let mut totals: HashMap<&'static str, f64> = HashMap::new();
            for rr in REBIRTH_REWARDS.iter() {
                if self.rebirths >= rr.need {
                    for (k, v, _) in rr.rewards {
                        *totals.entry(k).or_insert(0.0) += v;
                    }
                }
            }
            *cache = Some((self.rebirths, totals));
        }
        cache.as_ref().unwrap().1.get(kind).copied().unwrap_or(0.0)
    }

    pub fn rebirth_keeps_upgrades(&self) -> bool {
        self.rebirth_bonus("keep_upgrades") > 0.0 || self.prestige >= PRESTIGE_KEEP_UPGRADES
    }

    /// v3.0 Prestige III: a Rebirth resets nothing, you even keep your coins
    pub fn rebirth_keeps_everything(&self) -> bool {
        self.prestige >= PRESTIGE_REBIRTH_FREE
    }

    pub fn next_rebirth_reward(&self) -> Option<usize> {
        REBIRTH_REWARDS.iter().position(|r| self.rebirths < r.need)
    }

    pub fn do_rebirth(&mut self) -> bool {
        if !self.rebirth_available() {
            return false;
        }
        let keep = self.rebirth_keeps_upgrades();
        self.rebirths += 1;
        self.daily_add_progress("rebirths", 1);
        if !self.rebirth_keeps_everything() {
            self.coins = 0.0;
        }
        if !keep {
            for (i, u) in upgrade_defs().iter().enumerate() {
                if !KEEP_ON_REBIRTH.contains(&u.key) {
                    // the Auto Upgrader always stays
                    self.upgrades[i] = 0;
                }
            }
            let ms = self.max_slots().max(0) as usize;
            self.equipped.truncate(ms);
        }
        true
    }

    // ================================================================ prestige (v3.0)
    /// The Prestige you have (None = none yet).
    pub fn prestige_def(&self) -> Option<&'static PrestigeDef> {
        if self.prestige <= 0 { None } else { PRESTIGES.get((self.prestige - 1).min(PRESTIGES.len() as i64 - 1) as usize) }
    }

    /// The next Prestige (None = all 5 done).
    pub fn next_prestige(&self) -> Option<&'static PrestigeDef> {
        PRESTIGES.get(self.prestige.max(0) as usize)
    }

    pub fn prestige_available(&self) -> bool {
        self.next_prestige().is_some_and(|p| self.rebirths >= p.need)
    }

    /// The verity a Prestige keeps when you don't pick one: the one that earns the most.
    pub fn best_owned_pet(&self) -> Option<Pet> {
        let mut best: Option<(f64, Pet)> = None;
        for r_idx in 0..rarities().len() {
            for m in MUT_ORDER {
                if self.count_owned(r_idx, m) > 0 {
                    let inc = self.pet_income(r_idx, m);
                    if best.map_or(true, |b| inc > b.0) {
                        best = Some((inc, (r_idx, m)));
                    }
                }
            }
        }
        best.map(|b| b.1)
    }

    /// Resets coins, Rebirths, upgrades (not the automation) and pets, keeping ONE copy of `keep`. Dice, potions,
    /// traits and charges, milestones, the Index, titles, stats and playtime stay.
    pub fn do_prestige(&mut self, keep: Option<Pet>) -> bool {
        if !self.prestige_available() {
            return false;
        }
        let keep = keep.filter(|(r, m)| self.count_owned(*r, m) > 0);
        self.prestige += 1;
        self.rebirths = 0;
        *self.rb_cache.borrow_mut() = None;
        self.coins = 0.0;
        for (i, u) in upgrade_defs().iter().enumerate() {
            if !KEEP_ON_PRESTIGE.contains(&u.key) {
                self.upgrades[i] = 0;
            }
        }
        self.owned.clear();
        self.equipped.clear();
        if let Some((r, m)) = keep {
            self.owned.insert(format!("{}_{}", r, m), 1);
            self.equipped.push((r, m));
        }
        self.cyclic_roll_count = 0;
        self.cyclic_bonus_ready = false;
        self.diamond_roll_count = 0;
        self.diamond_bonus_ready = false;
        self.rainbow_roll_count = 0;
        self.rainbow_bonus_ready = false;
        self.probs_cache = None;
        self.dirty = true;
        true
    }

    // ---------------- auto trait roller ----------------
    pub fn auto_trait_unlocked(&self) -> bool {
        self.upgrade_level("auto_trait_unlock") >= 1
    }

    // ================================================================ daily missions
    pub fn mission_stage(&self) -> usize {
        MISSION_STAGE_ROLLS.iter().filter(|&&r| self.total_rolls >= r).count()
    }

    fn generate_quests(&self, seed: &str, pool: &'static [MissionDef], count: usize) -> Vec<DailyMission> {
        let mut rng = PyRandom::from_str(seed);
        let stage = self.mission_stage();
        let open: Vec<&MissionDef> = pool.iter().filter(|m| stage >= m.min_stage).collect();
        let chosen = rng.sample_indices(open.len(), count.min(open.len()));
        chosen
            .into_iter()
            .map(|i| {
                let m = open[i];
                let idx = stage.min(m.targets.len() - 1);
                DailyMission { mtype: m.mtype, target: m.targets[idx], reward: m.reward[idx] }
            })
            .collect()
    }

    pub fn generate_daily_missions(&self, date_str: &str) -> Vec<DailyMission> {
        let slot = match self.slot {
            Some(s) => s.to_string(),
            None => "None".to_string(),
        };
        self.generate_quests(&format!("{}:slot{}", date_str, slot), &MISSION_POOL, DAILY_MISSION_COUNT)
    }

    pub fn ensure_weekly_missions(&mut self) {
        let week = week_key(&self.today());
        if self.weekly_week.as_deref() == Some(week.as_str()) && !self.weekly_missions.is_empty() {
            return;
        }
        let slot = self.slot.map_or("None".to_string(), |s| s.to_string());
        self.weekly_missions = self.generate_quests(&format!("{}:slot{}:weekly", week, slot), &WEEKLY_POOL, WEEKLY_MISSION_COUNT);
        self.weekly_week = Some(week);
        self.weekly_counts = IndexMap::new();
        self.weekly_claimed = BTreeSet::new();
    }

    pub fn weekly_mission_progress(&self, index: usize) -> i64 {
        match self.weekly_missions.get(index) {
            None => 0,
            Some(m) => self.weekly_counts.get(m.mtype).copied().unwrap_or(0),
        }
    }

    /// The reward of a quest if claimed now: it grows with what you have. `base` = the quest's trait charges.
    pub fn quest_reward(&self, weekly: bool, index: usize, base: i64) -> QuestReward {
        let minutes = if weekly { WEEKLY_REWARD_MINUTES } else { DAILY_REWARD_MINUTES };
        // your own money/sec and rolls/sec, without the temporary boosts (potions, events)
        let income = self.income_per_second() / (self.potion_mult("money") * event_mult("money")).max(1e-9);
        let rps = self.auto_rolls_per_second() / (self.potion_mult("speed") * event_mult("speed")).max(1e-9);
        let charges = base + (rps * self.trait_charge_chance() * 60.0 * minutes).round().min(1e15) as i64;
        let potion = weekly.then(|| {
            let kind = ["luck", "money", "speed"][index % 3];
            let level = (1 + self.rebirths / 10 + self.prestige).clamp(1, crate::core::shop::POTION_LEVELS);
            (kind, level)
        });
        QuestReward { coins: (income * 60.0 * minutes).max(0.0), charges, potion }
    }

    fn give_quest_reward(&mut self, r: &QuestReward) {
        self.coins += r.coins;
        self.total_coins_earned += r.coins;
        self.trait_charges += r.charges;
        if let Some((kind, lvl)) = r.potion {
            *self.shop.potions.entry(crate::core::shop::potion_id(kind, lvl)).or_insert(0) += 1;
        }
        self.dirty = true;
    }

    pub fn claim_weekly_mission(&mut self, index: usize) -> Option<QuestReward> {
        self.ensure_weekly_missions();
        if index >= self.weekly_missions.len() || self.weekly_claimed.contains(&index) {
            return None;
        }
        let m = self.weekly_missions[index].clone();
        if self.weekly_mission_progress(index) < m.target {
            return None;
        }
        self.weekly_claimed.insert(index);
        let r = self.quest_reward(true, index, m.reward);
        self.give_quest_reward(&r);
        Some(r)
    }

    pub fn ensure_daily_missions(&mut self) {
        let today = self.today();
        if self.daily_date.as_deref() == Some(today.as_str()) && !self.daily_missions.is_empty() {
            return;
        }
        self.daily_missions = self.generate_daily_missions(&today);
        self.daily_date = Some(today);
        self.daily_counts = IndexMap::new();
        self.daily_claimed = BTreeSet::new();
    }

    pub fn daily_mission_progress(&self, index: usize) -> i64 {
        match self.daily_missions.get(index) {
            None => 0,
            Some(m) => self.daily_counts.get(m.mtype).copied().unwrap_or(0),
        }
    }

    pub fn daily_add_progress(&mut self, mtype: &str, n: i64) {
        self.ensure_daily_missions();
        self.ensure_weekly_missions();
        *self.daily_counts.entry(mtype.to_string()).or_insert(0) += n;
        *self.weekly_counts.entry(mtype.to_string()).or_insert(0) += n;
    }

    pub fn claim_daily_mission(&mut self, index: usize) -> Option<QuestReward> {
        self.ensure_daily_missions();
        if index >= self.daily_missions.len() || self.daily_claimed.contains(&index) {
            return None;
        }
        let m = self.daily_missions[index].clone();
        if self.daily_mission_progress(index) < m.target {
            return None;
        }
        self.daily_claimed.insert(index);
        let r = self.quest_reward(false, index, m.reward);
        self.give_quest_reward(&r);
        Some(r)
    }

    // ================================================================ offline earnings
    pub fn offline_earn_rate(&self) -> f64 {
        let rate = OFFLINE_BASE_RATE
            + OFFLINE_RATE_STEP * (self.upgrade_level("offline_rate") + self.upgrade_level("offline_rate_2")) as f64
            + self.rebirth_bonus("offline_rate");
        rate.min(1.0)
    }

    pub fn offline_max_seconds(&self) -> f64 {
        let hours = OFFLINE_BASE_HOURS
            + OFFLINE_TIME_STEP * (self.upgrade_level("offline_time") + self.upgrade_level("offline_time_2")) as f64
            + self.rebirth_bonus("offline_time");
        OFFLINE_MAX_HOURS.min(hours) * 3600.0
    }

    pub fn claim_offline_earnings(&mut self) -> (f64, f64) {
        let now = now_ts();
        let last = self.last_seen;
        self.last_seen = Some(now);
        let Some(last) = last else {
            return (0.0, 0.0);
        };
        let mut elapsed = now - last;
        if elapsed < 60.0 {
            return (0.0, 0.0);
        }
        elapsed = elapsed.min(self.offline_max_seconds());
        // the money potion only counts with the game open (its time doesn't run offline)
        let gain = self.income_per_second() / self.potion_mult("money") * elapsed * self.offline_earn_rate();
        if gain > 0.0 {
            self.coins += gain;
            self.total_coins_earned += gain;
        }
        (gain, elapsed)
    }

    // ================================================================ save / load
    pub fn to_dict(&self) -> Value {
        let mut d = Map::new();
        d.insert("coins".into(), pyjson::float(self.coins));
        let owned: Map<String, Value> = self.owned.iter().map(|(k, v)| (k.clone(), json!(v))).collect();
        d.insert("owned".into(), Value::Object(owned));
        d.insert("seen_pets".into(), Value::Array(self.seen_pets.iter().map(|k| json!(k)).collect()));
        d.insert("equipped".into(), Value::Array(self.equipped.iter().map(|(i, m)| json!([i, m])).collect()));
        d.insert("avatar".into(), match self.avatar {
            Some((i, m)) => json!([i, m]),
            None => Value::Null,
        });
        let ups: Map<String, Value> = upgrade_defs().iter().enumerate().map(|(i, u)| (u.key.to_string(), json!(self.upgrades[i]))).collect();
        d.insert("upgrades".into(), Value::Object(ups));
        if let Some(t) = self.title {
            // only written when there is one, so a save without a title is exactly what the Python game writes
            d.insert("title".into(), json!(t));
        }
        d.insert("total_rolls".into(), json!(self.total_rolls));
        d.insert("shop".into(), self.shop.to_dict());
        let mut st = Map::new();
        st.insert("auto_on".into(), json!(self.auto_on));
        st.insert("auto_equip_best_on".into(), json!(self.auto_equip_best_on));
        st.insert("auto_upgrade_on".into(), json!(self.auto_upgrade_on));
        st.insert("auto_trait_on".into(), json!(self.auto_trait_on));
        d.insert("settings".into(), Value::Object(st));
        let mut tr = Map::new();
        tr.insert("charges".into(), json!(self.trait_charges));
        tr.insert("owned".into(), Value::Array(self.owned_traits.iter().map(|i| json!(i)).collect()));
        tr.insert("equipped".into(), match self.equipped_trait {
            Some(i) => json!(i),
            None => Value::Null,
        });
        tr.insert("total_rolled".into(), json!(self.total_traits_rolled));
        d.insert("traits".into(), Value::Object(tr));
        d.insert("total_coins_earned".into(), pyjson::float(self.total_coins_earned));
        d.insert("playtime".into(), pyjson::float(self.playtime));
        d.insert("total_golden_rolled".into(), json!(self.total_golden_rolled));
        d.insert("total_diamond_rolled".into(), json!(self.total_diamond_rolled));
        d.insert("total_secret_rolled".into(), json!(self.total_secret_rolled));
        d.insert("total_divine_rolled".into(), json!(self.total_divine_rolled));
        d.insert("total_cosmic_rolled".into(), json!(self.total_cosmic_rolled));
        d.insert("total_transcendent_rolled".into(), json!(self.total_transcendent_rolled));
        d.insert("total_rainbow_rolled".into(), json!(self.total_rainbow_rolled));
        d.insert("total_ethereal_rolled".into(), json!(self.total_ethereal_rolled));
        d.insert("total_celestial_rolled".into(), json!(self.total_celestial_rolled));
        d.insert("total_absolute_rolled".into(), json!(self.total_absolute_rolled));
        let mut claimed: Vec<&String> = self.milestones_claimed.iter().collect();
        claimed.sort();
        d.insert("milestones_claimed".into(), Value::Array(claimed.into_iter().map(|s| json!(s)).collect()));
        d.insert("cyclic_roll_count".into(), json!(self.cyclic_roll_count));
        d.insert("cyclic_bonus_ready".into(), json!(self.cyclic_bonus_ready));
        d.insert("diamond_roll_count".into(), json!(self.diamond_roll_count));
        d.insert("diamond_bonus_ready".into(), json!(self.diamond_bonus_ready));
        d.insert("rainbow_roll_count".into(), json!(self.rainbow_roll_count));
        d.insert("rainbow_bonus_ready".into(), json!(self.rainbow_bonus_ready));
        let cp: Map<String, Value> = self.cycle_paused.iter().map(|(k, v)| (k.to_string(), json!(v))).collect();
        d.insert("cycle_paused".into(), Value::Object(cp));
        d.insert("rebirths".into(), json!(self.rebirths));
        if self.prestige > 0 {
            // only once there is one: saves without Prestige stay exactly as before
            d.insert("prestige".into(), json!(self.prestige));
        }
        d.insert("last_seen".into(), match self.last_seen {
            Some(t) => pyjson::float(t),
            None => Value::Null,
        });
        let mut daily = Map::new();
        daily.insert("date".into(), match &self.daily_date {
            Some(s) => json!(s),
            None => Value::Null,
        });
        daily.insert(
            "missions".into(),
            Value::Array(
                self.daily_missions
                    .iter()
                    .map(|m| {
                        let mut o = Map::new();
                        o.insert("type".into(), json!(m.mtype));
                        o.insert("target".into(), json!(m.target));
                        o.insert("reward".into(), json!(m.reward));
                        Value::Object(o)
                    })
                    .collect(),
            ),
        );
        let counts: Map<String, Value> = self.daily_counts.iter().map(|(k, v)| (k.clone(), json!(v))).collect();
        daily.insert("counts".into(), Value::Object(counts));
        daily.insert("claimed".into(), Value::Array(self.daily_claimed.iter().map(|i| json!(i)).collect()));
        d.insert("daily".into(), Value::Object(daily));
        let mut weekly = Map::new();
        weekly.insert("week".into(), self.weekly_week.as_ref().map_or(Value::Null, |s| json!(s)));
        weekly.insert("missions".into(), missions_json(&self.weekly_missions));
        weekly.insert("counts".into(), Value::Object(self.weekly_counts.iter().map(|(k, v)| (k.clone(), json!(v))).collect()));
        weekly.insert("claimed".into(), Value::Array(self.weekly_claimed.iter().map(|i| json!(i)).collect()));
        d.insert("weekly".into(), Value::Object(weekly));
        if let Some(p) = &self.prefs {
            d.insert("prefs".into(), Value::Object(p.clone()));
        }
        if self.season > 0 {
            d.insert("season".into(), json!(self.season));
        }
        if self.player_reset > 0 {
            d.insert("player_reset".into(), json!(self.player_reset));
        }
        Value::Object(d)
    }

    /// load_dict: mirrors the Python, including stopping at the first value that would raise.
    pub fn load_dict(&mut self, d: &Value) -> Result<(), ()> {
        let Value::Object(d) = d else {
            return Err(());
        };
        let get_f = |k: &str, def: f64| -> Result<f64, ()> {
            match d.get(k) {
                None => Ok(def),
                Some(v) => value_f64(v).ok_or(()),
            }
        };
        let get_i = |k: &str, def: i64| -> Result<i64, ()> {
            match d.get(k) {
                None => Ok(def),
                Some(v) => value_i64(v).ok_or(()),
            }
        };
        self.coins = get_f("coins", 0.0)?;
        let mut owned = IndexMap::new();
        match d.get("owned") {
            None => {}
            Some(Value::Object(o)) => {
                for (k, v) in o {
                    owned.insert(k.clone(), value_i64(v).ok_or(())?);
                }
            }
            Some(_) => return Err(()),
        }
        self.owned = owned;
        let valid_key = |k: &str| -> bool {
            match k.split_once('_') {
                Some((idx_s, m)) => idx_s.trim().parse::<i64>().map(|i| i >= 0 && (i as usize) < rarities().len()).unwrap_or(false) && is_mutation(m),
                None => false,
            }
        };
        self.seen_pets = BTreeSet::new();
        if d.contains_key("seen_pets") {
            for k in &py_iter(d.get("seen_pets"))? {
                let k = match k {
                    Value::String(s) => s.clone(),
                    other => other.to_string(),
                };
                if valid_key(&k) {
                    self.seen_pets.insert(k);
                }
            }
        } else {
            // a save from before this feature: everything you had counts as "seen", so nobody loses Index
            // progress because of this update
            for (k, v) in &self.owned {
                if *v > 0 && valid_key(k) {
                    self.seen_pets.insert(k.clone());
                }
            }
        }
        self.equipped = Vec::new();
        {
            for p in &py_iter(d.get("equipped"))? {
                let Value::Array(p) = p else { continue };
                let (Some(a), Some(b)) = (p.first(), p.get(1)) else { continue };
                let Some(idx) = value_i64(a) else { continue };
                let m = match b {
                    Value::String(s) => s.clone(),
                    other => other.to_string(),
                };
                if idx >= 0 && (idx as usize) < rarities().len() && is_mutation(&m) {
                    self.equipped.push((idx as usize, mut_key(&m)));
                }
            }
        }
        self.avatar = None;
        if let Some(Value::Array(av)) = d.get("avatar") {
            if let (Some(a), Some(b)) = (av.first(), av.get(1)) {
                if let Some(idx) = value_i64(a) {
                    let m = match b {
                        Value::String(s) => s.clone(),
                        other => other.to_string(),
                    };
                    if idx >= 0 && (idx as usize) < rarities().len() && is_mutation(&m) {
                        self.avatar = Some((idx as usize, mut_key(&m)));
                    }
                }
            }
        }
        self.title = d.get("title").and_then(|v| v.as_str()).and_then(crate::core::titles::title_key);
        self.rebirths = get_i("rebirths", 0)?.max(0);
        self.prestige = get_i("prestige", 0)?.clamp(0, PRESTIGES.len() as i64);
        *self.rb_cache.borrow_mut() = None;
        let empty = Map::new();
        let loaded_up = match d.get("upgrades") {
            Some(Value::Object(o)) => o,
            None => &empty,
            Some(_) => return Err(()),
        };
        for (i, u) in upgrade_defs().iter().enumerate() {
            let lvl = match loaded_up.get(u.key) {
                None => 0,
                Some(v) => value_i64(v).ok_or(())?,
            };
            self.upgrades[i] = lvl.min(u.max_level).max(0);
        }
        // v3.0.1: saves from before the Golden Roll needed unlocking keep it if they already upgraded it
        if self.upgrade_level("cyclic_every") > 0 || self.upgrade_level("cyclic_power") > 0 {
            self.upgrades[upgrade_index("golden_roll_unlock")] = 1;
        }
        self.total_rolls = get_i("total_rolls", 0)?;
        let st = match d.get("settings") {
            Some(Value::Object(o)) => o.clone(),
            None => Map::new(),
            Some(_) => return Err(()),
        };
        self.auto_on = st.get("auto_on").map(value_truthy).unwrap_or(true);
        self.auto_equip_best_on = st.get("auto_equip_best_on").map(value_truthy).unwrap_or(true);
        self.auto_upgrade_on = st.get("auto_upgrade_on").map(value_truthy).unwrap_or(true);
        self.auto_trait_on = st.get("auto_trait_on").map(value_truthy).unwrap_or(true);
        let ms = self.max_slots().max(0) as usize;
        self.equipped.truncate(ms);

        let tr = match d.get("traits") {
            Some(Value::Object(o)) => o.clone(),
            None => Map::new(),
            Some(_) => return Err(()),
        };
        self.trait_charges = match tr.get("charges") {
            None => 0,
            Some(v) => value_i64(v).ok_or(())?,
        }
        .max(0);
        let mut ot = BTreeSet::new();
        {
            for v in &py_iter(tr.get("owned"))? {
                let i = value_i64(v).ok_or(())?;
                if i >= 0 && (i as usize) < TRAITS.len() {
                    ot.insert(i as usize);
                }
            }
        }
        self.owned_traits = ot;
        self.equipped_trait = match tr.get("equipped") {
            None | Some(Value::Null) => None,
            Some(v) => {
                let i = value_i64(v).ok_or(())?;
                if i >= 0 && self.owned_traits.contains(&(i as usize)) { Some(i as usize) } else { None }
            }
        };
        self.total_traits_rolled = match tr.get("total_rolled") {
            None => 0,
            Some(v) => value_i64(v).ok_or(())?,
        }
        .max(0);

        self.total_coins_earned = self.coins.max(get_f("total_coins_earned", 0.0)?);
        self.playtime = get_f("playtime", 0.0)?.max(0.0);
        self.total_golden_rolled = get_i("total_golden_rolled", 0)?.max(0);
        self.total_diamond_rolled = get_i("total_diamond_rolled", 0)?.max(0);
        self.total_secret_rolled = get_i("total_secret_rolled", self.owned_of_rarity("secreto"))?.max(0);
        self.total_divine_rolled = get_i("total_divine_rolled", self.owned_of_rarity("divino"))?.max(0);
        self.total_cosmic_rolled = get_i("total_cosmic_rolled", self.owned_of_rarity("cosmico"))?.max(0);
        self.total_transcendent_rolled = get_i("total_transcendent_rolled", self.owned_of_rarity("transcendente"))?.max(0);
        self.total_rainbow_rolled = get_i("total_rainbow_rolled", 0)?.max(0);
        self.total_ethereal_rolled = get_i("total_ethereal_rolled", self.owned_of_rarity("etereo"))?.max(0);
        self.total_celestial_rolled = get_i("total_celestial_rolled", self.owned_of_rarity("celestial"))?.max(0);
        self.total_absolute_rolled = get_i("total_absolute_rolled", self.owned_of_rarity("absoluto"))?.max(0);
        let mut claimed = HashSet::new();
        {
            for k in &py_iter(d.get("milestones_claimed"))? {
                let k = match k {
                    Value::String(s) => s.clone(),
                    other => other.to_string(),
                };
                let parts: Vec<&str> = k.split(':').collect();
                if parts.len() != 2 {
                    continue;
                }
                if let (Some(cat), Ok(idx)) = (milestone_cat(parts[0]), parts[1].trim().parse::<i64>()) {
                    if idx >= 0 && (idx as usize) < cat.tiers.len() {
                        claimed.insert(k.clone());
                    }
                }
            }
        }
        self.milestones_claimed = claimed;
        self.recalc_milestone_bonus();
        self.cyclic_roll_count = get_i("cyclic_roll_count", 0)?.min(self.golden_roll_every() - 1).max(0);
        self.cyclic_bonus_ready = d.get("cyclic_bonus_ready").map(value_truthy).unwrap_or(false);
        self.diamond_roll_count = get_i("diamond_roll_count", 0)?.min(self.diamond_roll_every() - 1).max(0);
        self.diamond_bonus_ready = d.get("diamond_bonus_ready").map(value_truthy).unwrap_or(false);
        self.rainbow_roll_count = get_i("rainbow_roll_count", 0)?.min(self.rainbow_roll_every() - 1).max(0);
        self.rainbow_bonus_ready = d.get("rainbow_bonus_ready").map(value_truthy).unwrap_or(false);
        let paused = match d.get("cycle_paused") {
            Some(Value::Object(o)) => o.clone(),
            _ => Map::new(),
        };
        for k in ["golden", "diamond", "rainbow"] {
            self.cycle_paused.insert(k, paused.get(k).map(value_truthy).unwrap_or(false));
        }
        self.last_seen = match d.get("last_seen") {
            Some(Value::Number(n)) => n.as_f64(),
            Some(Value::Bool(b)) => Some(if *b { 1.0 } else { 0.0 }),
            _ => None,
        };
        self.shop = crate::core::shop::ShopState::load_dict(d.get("shop"));

        let daily = match d.get("daily") {
            Some(Value::Object(o)) => o.clone(),
            _ => Map::new(),
        };
        self.daily_date = match daily.get("date") {
            Some(Value::String(s)) => Some(s.clone()),
            _ => None,
        };
        self.daily_missions = Vec::new();
        if let Some(Value::Array(ms)) = daily.get("missions") {
            for m in ms {
                let Value::Object(m) = m else { continue };
                let (Some(t), Some(ta), Some(re)) = (m.get("type"), m.get("target"), m.get("reward")) else { continue };
                let t = match t {
                    Value::String(s) => s.clone(),
                    other => other.to_string(),
                };
                let (Some(ta), Some(re)) = (value_i64(ta), value_i64(re)) else { continue };
                if let Some(def) = mission_def(&t) {
                    if ta > 0 && re > 0 {
                        self.daily_missions.push(DailyMission { mtype: def.mtype, target: ta, reward: re });
                    }
                }
            }
        }
        self.daily_counts = IndexMap::new();
        if let Some(Value::Object(c)) = daily.get("counts") {
            for (k, v) in c {
                if let Some(i) = value_i64(v) {
                    self.daily_counts.insert(k.clone(), i.max(0));
                }
            }
        }
        self.daily_claimed = BTreeSet::new();
        if let Some(Value::Array(a)) = daily.get("claimed") {
            for v in a {
                if let Some(i) = value_i64(v) {
                    if i >= 0 && (i as usize) < self.daily_missions.len() {
                        self.daily_claimed.insert(i as usize);
                    }
                }
            }
        }
        self.ensure_daily_missions();
        let weekly = match d.get("weekly") {
            Some(Value::Object(o)) => o.clone(),
            _ => Map::new(),
        };
        self.weekly_week = weekly.get("week").and_then(|v| v.as_str()).map(|s| s.to_string());
        self.weekly_missions = missions_from_json(weekly.get("missions"));
        self.weekly_counts = IndexMap::new();
        if let Some(Value::Object(c)) = weekly.get("counts") {
            for (k, v) in c {
                if let Some(i) = value_i64(v) {
                    self.weekly_counts.insert(k.clone(), i.max(0));
                }
            }
        }
        self.weekly_claimed = weekly
            .get("claimed")
            .and_then(|v| v.as_array())
            .map(|a| a.iter().filter_map(value_i64).filter(|i| *i >= 0 && (*i as usize) < self.weekly_missions.len()).map(|i| i as usize).collect())
            .unwrap_or_default();
        self.ensure_weekly_missions();
        self.season = d.get("season").and_then(value_i64).unwrap_or(0).max(0);
        self.player_reset = d.get("player_reset").and_then(value_i64).unwrap_or(0).max(0);
        self.prefs = match d.get("prefs") {
            Some(Value::Object(p)) => Some(p.clone()),
            _ => None,
        };
        Ok(())
    }

    pub fn save(&mut self) {
        let Some(slot) = self.slot else {
            return;
        };
        if let Some(uid) = self.cloud_uid.clone() {
            self.save_seq += 1;
            self.dirty = true;
            crate::online::cloud_cache::write_cache(&uid, slot, self.cloud_base_time.as_deref(), true, &self.to_dict());
            return;
        }
        let path = save_slot_path(slot);
        let tmp = path.with_extension("json.tmp");
        let text = pyjson::dumps(&self.to_dict());
        if std::fs::write(&tmp, text).is_ok() {
            let _ = std::fs::rename(&tmp, &path);
        }
    }

    pub fn load(&mut self) {
        let Some(slot) = self.slot else {
            return;
        };
        let path = save_slot_path(slot);
        if path.exists() {
            if let Ok(text) = std::fs::read_to_string(&path) {
                if let Ok(v) = serde_json::from_str::<Value>(&text) {
                    let _ = self.load_dict(&v);
                }
            }
        }
    }
}

/// Python `float >= int`, exact (no rounding of the int).
fn missions_json(ms: &[DailyMission]) -> Value {
    Value::Array(ms.iter().map(|m| json!({"type": m.mtype, "target": m.target, "reward": m.reward})).collect())
}

fn missions_from_json(v: Option<&Value>) -> Vec<DailyMission> {
    let mut out = Vec::new();
    for m in v.and_then(|v| v.as_array()).map(|a| a.as_slice()).unwrap_or(&[]) {
        let (Some(t), Some(ta), Some(re)) = (m.get("type").and_then(|v| v.as_str()), m.get("target").and_then(value_i64), m.get("reward").and_then(value_i64)) else { continue };
        if let Some(def) = mission_def(t) {
            if ta > 0 && re > 0 {
                out.push(DailyMission { mtype: def.mtype, target: ta, reward: re });
            }
        }
    }
    out
}

pub fn py_ge(a: f64, b: i128) -> bool {
    !a.is_nan() && !py_gt_int_float(b, a)
}

/// Python `for x in v` over a JSON value (missing = empty; a dict yields its keys, a str its chars;
/// anything else raises TypeError, which aborts load_dict).
fn py_iter(v: Option<&Value>) -> Result<Vec<Value>, ()> {
    match v {
        None => Ok(Vec::new()),
        Some(Value::Array(a)) => Ok(a.clone()),
        Some(Value::Object(o)) => Ok(o.keys().map(|k| Value::String(k.clone())).collect()),
        Some(Value::String(s)) => Ok(s.chars().map(|c| Value::String(c.to_string())).collect()),
        Some(_) => Err(()),
    }
}

/// Python `int > float`, exact.
pub fn py_gt_int_float(i: i128, f: f64) -> bool {
    if f.is_nan() {
        return false;
    }
    if f.is_infinite() {
        return f < 0.0;
    }
    let fl = f.floor();
    if fl.abs() >= 1.7e38 {
        return (i as f64) > f;
    }
    let fi = fl as i128;
    // i > floor(f)  <=>  i >= floor(f) + 1 > f
    i > fi
}

pub fn int_to_f64(i: i128) -> f64 {
    i as f64
}

#[cfg(test)]
mod prestige_tests {
    use super::*;

    #[test]
    fn prestige_resets_and_keeps_one_verity() {
        let mut s = GameState::new();
        let money0 = s.money_multiplier();
        s.rebirths = 12;
        s.coins = 1e15;
        s.owned.insert("5_golden".into(), 7);
        s.owned.insert("2_normal".into(), 100);
        s.equipped = vec![(5, "golden"), (2, "normal")];
        s.trait_charges = 50;
        s.upgrades[upgrade_index("money")] = 10;
        s.upgrades[upgrade_index("auto_trait_unlock")] = 1;
        assert!(s.prestige_available());
        assert!(s.do_prestige(Some((5, "golden"))));
        assert_eq!((s.prestige, s.rebirths, s.coins), (1, 0, 0.0));
        assert_eq!(s.owned.len(), 1);
        assert_eq!(s.count_owned(5, "golden"), 1);
        assert_eq!(s.equipped, vec![(5, "golden")]);
        assert_eq!(s.trait_charges, 50); // traits stay
        assert_eq!(s.upgrade_level("money"), 0);
        assert_eq!(s.upgrade_level("auto_trait_unlock"), 1); // the automation stays
        assert!((s.money_multiplier() / money0 - 3.0).abs() < 1e-9);
        assert!(!s.prestige_available()); // Prestige II needs 15 Rebirths
        // saved and loaded
        let d = s.to_dict();
        let mut t = GameState::new();
        t.load_dict(&d).unwrap();
        assert_eq!(t.prestige, 1);
    }

    #[test]
    fn rebirths_lock_until_the_next_prestige() {
        let mut s = GameState::new();
        s.rebirths = 9;
        s.coins = 1e300;
        assert!(s.rebirth_available());
        assert!(s.do_rebirth());
        assert_eq!(s.rebirths, 10);
        assert!(s.rebirth_locked() && !s.rebirth_available() && !s.do_rebirth()); // stuck at 10 until Prestige I
        assert!(s.do_prestige(None));
        s.coins = 1e300;
        assert!(s.rebirth_available()); // Prestige I: rebirths again, up to 15
        s.rebirths = 15;
        assert!(s.rebirth_locked());
        s.prestige = 5;
        s.rebirths = 400;
        assert!(!s.rebirth_locked()); // after Prestige V: no limit
    }

    #[test]
    fn golden_roll_needs_its_unlock() {
        let mut s = GameState::new();
        for _ in 0..50 {
            s.roll();
        }
        assert!(!s.cyclic_bonus_ready && s.cyclic_roll_count == 0); // locked: the cycle doesn't run
        s.upgrades[upgrade_index("golden_roll_unlock")] = 1;
        for _ in 0..GOLDEN_ROLL_EVERY {
            s.roll();
        }
        assert!(s.cyclic_bonus_ready);
        assert_eq!(s.golden_roll_mult(), 10.0);
        // a save from before the unlock existed, with Golden Roll upgrades: it stays unlocked
        let mut old = GameState::new();
        old.upgrades[upgrade_index("cyclic_every")] = 2;
        let mut t = GameState::new();
        t.load_dict(&old.to_dict()).unwrap();
        assert!(t.golden_roll_unlocked());
    }

    #[test]
    fn prestige_three_makes_rebirth_free() {
        let mut s = GameState::new();
        s.prestige = 3;
        s.coins = s.rebirth_cost() * 2.0;
        s.upgrades[upgrade_index("money")] = 5;
        let coins = s.coins;
        assert!(s.do_rebirth());
        assert_eq!(s.coins, coins);
        assert_eq!(s.upgrade_level("money"), 5);
    }
}

#[cfg(test)]
mod quest_tests {
    use super::*;

    fn utc(s: &str) -> f64 {
        chrono::NaiveDateTime::parse_from_str(s, "%Y-%m-%d %H:%M").unwrap().and_utc().timestamp() as f64
    }

    #[test]
    fn lisbon_clock() {
        assert_eq!(lisbon_offset(utc("2026-01-15 12:00")), 0);
        assert_eq!(lisbon_offset(utc("2026-07-01 12:00")), 3600);
        // 2026: summer time from Sunday 29 March 01:00 UTC to Sunday 25 October 01:00 UTC
        assert_eq!(lisbon_offset(utc("2026-03-29 00:59")), 0);
        assert_eq!(lisbon_offset(utc("2026-03-29 01:00")), 3600);
        assert_eq!(lisbon_offset(utc("2026-10-25 00:59")), 3600);
        assert_eq!(lisbon_offset(utc("2026-10-25 01:00")), 0);
        // 23:30 UTC in summer is already the next day in Lisbon
        assert_eq!(lisbon_date(utc("2026-06-30 23:30")).to_string(), "2026-07-01");
        assert_eq!(lisbon_date(utc("2026-01-30 23:30")).to_string(), "2026-01-30");
        assert_eq!(week_key("2026-09-24"), "2026-W39");
        assert_eq!(week_key("2026-09-28"), "2026-W40"); // Monday: new week
        // Thursday 23:30 in Lisbon (22:30 UTC, summer): 30 minutes to the daily reset
        assert_eq!(secs_until_lisbon_reset(utc("2026-09-24 22:30"), false), 1800.0);
        // Sunday 23:30 in Lisbon: 30 minutes to the weekly reset; a Monday morning: almost a week
        assert_eq!(secs_until_lisbon_reset(utc("2026-09-27 22:30"), true), 1800.0);
        assert_eq!(secs_until_lisbon_reset(utc("2026-09-27 23:00"), true), 7.0 * 86400.0);
        // winter: midnight in Lisbon is midnight UTC
        assert_eq!(secs_until_lisbon_reset(utc("2026-12-01 23:00"), false), 3600.0);
    }

    #[test]
    fn weekly_quests_and_rewards_grow() {
        let mut s = GameState::new();
        s.slot = Some(1);
        s.ensure_weekly_missions();
        assert_eq!(s.weekly_missions.len(), WEEKLY_MISSION_COUNT);
        let early = s.quest_reward(true, 0, 10);
        assert!(early.potion.is_some());
        // someone further along gets more for the same quest
        s.equipped = vec![(20, "golden"); 5];
        s.rebirths = 25;
        let later = s.quest_reward(true, 0, 10);
        assert!(later.coins > early.coins);
        assert!(later.potion.unwrap().1 > early.potion.unwrap().1);
        // completing and claiming one
        let m = s.weekly_missions[0].clone();
        s.weekly_counts.insert(m.mtype.to_string(), m.target);
        let coins = s.coins;
        let r = s.claim_weekly_mission(0).expect("claimable");
        assert!(s.coins > coins && r.charges >= m.reward);
        assert!(s.claim_weekly_mission(0).is_none()); // only once
        // saved and loaded
        let mut t = GameState::new();
        t.slot = Some(1);
        t.load_dict(&s.to_dict()).unwrap();
        assert_eq!(t.weekly_week, s.weekly_week);
        assert!(t.weekly_claimed.contains(&0));
    }

    #[test]
    fn settings_travel_with_the_save() {
        let mut s = GameState::new();
        assert!(!s.to_dict().as_object().unwrap().contains_key("prefs")); // old-style save until there are some
        let mut p = Map::new();
        p.insert("animations".into(), json!(false));
        p.insert("cutscenes_secreto".into(), json!(false));
        s.prefs = Some(p.clone());
        let mut t = GameState::new();
        t.load_dict(&s.to_dict()).unwrap();
        assert_eq!(t.prefs, Some(p));
    }
}
