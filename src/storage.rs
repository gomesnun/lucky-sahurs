//! Local save slots and global settings (storage.py).

use crate::config::{SAVE_SLOTS, save_dir, save_path_legacy};
use crate::i18n::{DEFAULT_LANGUAGE, LANGUAGE_CODES};
use crate::theme::{DEFAULT_THEME_MODE, THEME_MODES};
use serde_json::{Map, Value, json};
use std::path::PathBuf;

pub fn save_slot_path(slot: i64) -> PathBuf {
    save_dir().join(format!("savegame_slot{}.json", slot))
}

pub fn migrate_legacy_save() {
    let legacy = save_path_legacy();
    if !legacy.exists() {
        return;
    }
    if (1..=SAVE_SLOTS).any(|i| save_slot_path(i).exists()) {
        return;
    }
    if let Ok(data) = std::fs::read_to_string(&legacy) {
        let _ = std::fs::write(save_slot_path(1), data);
    }
}

#[derive(Clone, Debug)]
pub struct SlotPeek {
    pub coins: f64,
    pub total_rolls: i64,
    pub playtime: f64,
}

pub fn value_f64(v: &Value) -> Option<f64> {
    match v {
        Value::Number(n) => n.as_f64(),
        Value::Bool(b) => Some(if *b { 1.0 } else { 0.0 }),
        Value::String(s) => s.trim().parse::<f64>().ok(),
        _ => None,
    }
}

/// Python int(v): ints, floats (truncated), bools and numeric strings.
pub fn value_i64(v: &Value) -> Option<i64> {
    match v {
        Value::Number(n) => n.as_i64().or_else(|| n.as_f64().filter(|f| f.is_finite()).map(|f| f.trunc() as i64)),
        Value::Bool(b) => Some(*b as i64),
        Value::String(s) => s.trim().parse::<i64>().ok(),
        _ => None,
    }
}

/// Python bool(v)
pub fn value_truthy(v: &Value) -> bool {
    match v {
        Value::Null => false,
        Value::Bool(b) => *b,
        Value::Number(n) => n.as_f64().map(|f| f != 0.0).unwrap_or(true),
        Value::String(s) => !s.is_empty(),
        Value::Array(a) => !a.is_empty(),
        Value::Object(o) => !o.is_empty(),
    }
}

pub fn peek_slot(slot: i64) -> Option<SlotPeek> {
    let path = save_slot_path(slot);
    if !path.exists() {
        return None;
    }
    let fallback = SlotPeek { coins: 0.0, total_rolls: 0, playtime: 0.0 };
    let Ok(text) = std::fs::read_to_string(&path) else {
        return Some(fallback);
    };
    let Ok(Value::Object(d)) = serde_json::from_str::<Value>(&text) else {
        return Some(fallback);
    };
    let coins = match d.get("coins") {
        None => Some(0.0),
        Some(v) => value_f64(v),
    };
    let rolls = match d.get("total_rolls") {
        None => Some(0),
        Some(v) => value_i64(v),
    };
    let play = match d.get("playtime") {
        None => Some(0.0),
        Some(v) => value_f64(v),
    };
    match (coins, rolls, play) {
        (Some(c), Some(r), Some(p)) => Some(SlotPeek { coins: c, total_rolls: r, playtime: p }),
        _ => Some(fallback),
    }
}

pub fn delete_slot(slot: i64) {
    let path = save_slot_path(slot);
    if path.exists() {
        let _ = std::fs::remove_file(path);
    }
}

/// A timestamped copy of a slot's save (savegame_slot{N}_{label}_backup_{unix time}.json), so something
/// destructive touching it - a season or personal reset - is never unrecoverable. Kept forever (these are rare
/// and small); a player who needs one back can rename the file to savegame_slot{N}.json by hand.
fn backup_path(slot: i64, label: &str) -> PathBuf {
    let now = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0);
    let dir = save_dir();
    let _ = std::fs::create_dir_all(dir); // never silently skip a backup just because the folder isn't there yet
    dir.join(format!("savegame_slot{}_{}_backup_{}.json", slot, label, now))
}

/// Copies a slot's current save file to a backup (see backup_path). Used right before deleting a save file we
/// haven't parsed into memory (on_season_known's other-slots loop).
pub fn backup_slot_file(slot: i64, label: &str) {
    let Ok(data) = std::fs::read(save_slot_path(slot)) else { return };
    let _ = std::fs::write(backup_path(slot, label), data);
}

/// Writes a save (already parsed and re-serialized in memory) to a backup file. Used when the save being backed
/// up is about to be overwritten on disk in the same breath (enforce_season_state) - backup_slot_file would be
/// too late (it re-reads the file, and by then it may already be the new one).
pub fn backup_json_file(slot: i64, label: &str, data: &Value) {
    if let Ok(text) = serde_json::to_string(data) {
        let _ = std::fs::write(backup_path(slot, label), text);
    }
}

/// delete_slot, but backs the file up first (see backup_slot_file).
pub fn delete_slot_with_backup(slot: i64, label: &str) {
    backup_slot_file(slot, label);
    delete_slot(slot);
}

pub fn settings_path() -> PathBuf {
    save_dir().join("lucky_verities_settings.json")
}

pub const SFX_CATEGORIES: [(&str, &str); 6] = [
    ("click", "Clicks"),
    ("roll", "Rolling"),
    ("traits", "Traits"),
    ("upgrades", "Upgrades"),
    ("milestones", "Milestones"),
    ("rebirth", "Rebirth"),
];

pub const CUTSCENE_RARITIES: [&str; 10] = ["secreto", "divino", "cosmico", "transcendente", "etereo", "celestial", "absoluto", "primordial", "paradoxo", "og"];

/// The settings dict (kept as an ordered JSON object, like the Python dict it mirrors).
#[derive(Clone, Debug)]
pub struct Settings {
    pub map: Map<String, Value>,
}

impl Settings {
    pub fn defaults() -> Settings {
        let mut m = Map::new();
        m.insert("sound_on".into(), json!(true));
        m.insert("volume".into(), json!(0.6));
        m.insert("animations".into(), json!(true));
        m.insert("fullscreen".into(), json!(true));
        m.insert("trait_notifications".into(), json!(true));
        m.insert("buy_mode".into(), json!(1));
        m.insert("music_on".into(), json!(true));
        m.insert("music_volume".into(), json!(0.5));
        m.insert("sfx_volume".into(), json!(1.0));
        m.insert("sfx_on".into(), json!(true));
        m.insert("language".into(), json!(DEFAULT_LANGUAGE));
        m.insert("theme_mode".into(), json!(DEFAULT_THEME_MODE));
        for (k, _) in SFX_CATEGORIES {
            m.insert(format!("sfx_{}", k), json!(true));
        }
        for k in CUTSCENE_RARITIES {
            m.insert(format!("cutscenes_{}", k), json!(true));
        }
        Settings { map: m }
    }

    pub fn get_bool(&self, key: &str, default: bool) -> bool {
        self.map.get(key).map(value_truthy).unwrap_or(default)
    }
    pub fn get_f64(&self, key: &str, default: f64) -> f64 {
        self.map.get(key).and_then(value_f64).unwrap_or(default)
    }
    pub fn get_str(&self, key: &str, default: &str) -> String {
        match self.map.get(key) {
            Some(Value::String(s)) => s.clone(),
            _ => default.to_string(),
        }
    }
    pub fn set_bool(&mut self, key: &str, v: bool) {
        self.map.insert(key.into(), json!(v));
    }
    pub fn set_f64(&mut self, key: &str, v: f64) {
        self.map.insert(key.into(), json!(v));
    }
    pub fn set_str(&mut self, key: &str, v: &str) {
        self.map.insert(key.into(), json!(v));
    }
    pub fn set_value(&mut self, key: &str, v: Value) {
        self.map.insert(key.into(), v);
    }
    pub fn get(&self, key: &str) -> Option<&Value> {
        self.map.get(key)
    }
}

pub fn load_settings() -> Settings {
    let mut s = Settings::defaults();
    let path = settings_path();
    if !path.exists() {
        return s;
    }
    let Ok(text) = std::fs::read_to_string(&path) else {
        return s;
    };
    let Ok(Value::Object(d)) = serde_json::from_str::<Value>(&text) else {
        return s;
    };
    // mirrors the Python: stop at the first bad value (the rest keeps its default)
    let run = |s: &mut Settings| -> Option<()> {
        let b = |k: &str, def: bool| d.get(k).map(value_truthy).unwrap_or(def);
        let f = |k: &str, def: f64| -> Option<f64> {
            match d.get(k) {
                None => Some(def),
                Some(v) => value_f64(v),
            }
        };
        s.set_bool("sound_on", b("sound_on", true));
        s.set_f64("volume", f("volume", 0.6)?.clamp(0.0, 1.0));
        s.set_bool("animations", b("animations", true));
        s.set_bool("fullscreen", b("fullscreen", true));
        s.set_bool("trait_notifications", b("trait_notifications", true));
        let legacy = b("cutscenes_enabled", true);
        for key in CUTSCENE_RARITIES {
            let k = format!("cutscenes_{}", key);
            s.set_bool(&k, b(&k, legacy));
        }
        s.set_bool("music_on", b("music_on", true));
        let mode = match d.get("theme_mode") {
            Some(Value::String(m)) if THEME_MODES.contains(&m.as_str()) => m.clone(),
            None => DEFAULT_THEME_MODE.to_string(),
            _ => DEFAULT_THEME_MODE.to_string(),
        };
        s.set_str("theme_mode", &mode);
        s.set_f64("music_volume", f("music_volume", 0.5)?.clamp(0.0, 1.0));
        s.set_f64("sfx_volume", f("sfx_volume", 1.0)?.clamp(0.0, 1.0));
        s.set_bool("sfx_on", b("sfx_on", true));
        let lang = match d.get("language") {
            Some(Value::String(l)) if LANGUAGE_CODES.contains(&l.as_str()) => l.clone(),
            _ => DEFAULT_LANGUAGE.to_string(),
        };
        s.set_str("language", &lang);
        for (key, _) in SFX_CATEGORIES {
            let k = format!("sfx_{}", key);
            s.set_bool(&k, b(&k, true));
        }
        let bm = d.get("buy_mode").cloned().unwrap_or(json!(1));
        let ok = bm == json!(1) || bm == json!(10) || bm == json!("max") || bm == json!(1.0) || bm == json!(10.0);
        s.set_value("buy_mode", if ok { bm } else { json!(1) });
        if let Some(v) = d.get("android_gpu_scale") {
            let _ = v;
        }
        Some(())
    };
    let _ = run(&mut s);
    s
}

pub fn save_settings(settings: &Settings) {
    let path = settings_path();
    let tmp = path.with_extension("json.tmp");
    let text = crate::pyjson::dumps(&Value::Object(settings.map.clone()));
    if std::fs::write(&tmp, text).is_ok() {
        let _ = std::fs::rename(&tmp, &path);
    }
}


// ----------------------------------------------------------------------------------
// CHAT: the last message already seen from each friend (it belongs to the online account, not a slot)
// ----------------------------------------------------------------------------------
fn chat_seen_path() -> std::path::PathBuf {
    crate::config::save_dir().join("lucky_verities_chat_seen.json")
}

/// uid -> time (epoch) of the last message already seen. Kept apart so the red "unread message" dot doesn't
/// light up again by itself when the game reopens.
pub fn load_chat_seen() -> std::collections::HashMap<String, f64> {
    let mut out = std::collections::HashMap::new();
    let Ok(t) = std::fs::read_to_string(chat_seen_path()) else { return out };
    if let Ok(serde_json::Value::Object(o)) = serde_json::from_str::<serde_json::Value>(&t) {
        for (k, v) in o {
            match value_f64(&v) {
                Some(f) => {
                    out.insert(k, f);
                }
                None => return std::collections::HashMap::new(),
            }
        }
    }
    out
}

pub fn save_chat_seen(seen: &std::collections::HashMap<String, f64>) {
    let path = chat_seen_path();
    let tmp = path.with_extension("json.tmp");
    let m: serde_json::Map<String, serde_json::Value> = seen.iter().map(|(k, v)| (k.clone(), crate::pyjson::float(*v))).collect();
    if std::fs::write(&tmp, crate::pyjson::dumps(&serde_json::Value::Object(m))).is_ok() {
        let _ = std::fs::rename(&tmp, &path);
    }
}
