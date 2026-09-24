//! Saved session, local cache of the account saves, and leaderboard periods (online/cloud_cache.py).

use crate::config::save_dir;
use crate::pyjson;
use crate::storage::value_truthy;
use serde_json::{Map, Value, json};
use std::path::PathBuf;

pub fn session_path() -> PathBuf {
    save_dir().join("lucky_verities_session.json")
}
pub fn cache_dir() -> PathBuf {
    save_dir().join("cloud_cache")
}
pub fn leaderboard_cache_path() -> PathBuf {
    save_dir().join("lucky_verities_leaderboard.json")
}

fn write_atomic(path: &PathBuf, v: &Value) {
    let mut tmp = path.clone().into_os_string();
    tmp.push(".tmp");
    let tmp = PathBuf::from(tmp);
    if std::fs::write(&tmp, pyjson::dumps(v)).is_ok() {
        let _ = std::fs::rename(&tmp, path);
    }
}

#[derive(Clone, Debug)]
pub struct Session {
    pub uid: String,
    pub username: String,
    pub refresh_token: String,
    pub raw: Map<String, Value>,
}

pub fn load_session() -> Option<Session> {
    let text = std::fs::read_to_string(session_path()).ok()?;
    let Value::Object(d) = serde_json::from_str::<Value>(&text).ok()? else {
        return None;
    };
    let get = |k: &str| match d.get(k) {
        Some(Value::String(s)) if !s.is_empty() => Some(s.clone()),
        _ => None,
    };
    Some(Session { uid: get("uid")?, username: get("username")?, refresh_token: get("refresh_token")?, raw: d.clone() })
}

pub fn store_session(uid: &str, username: &str, refresh_token: &str, last_pub_time: f64) {
    let mut m = Map::new();
    m.insert("uid".into(), json!(uid));
    m.insert("username".into(), json!(username));
    m.insert("refresh_token".into(), json!(refresh_token));
    m.insert("last_pub_time".into(), pyjson::float(last_pub_time));
    write_atomic(&session_path(), &Value::Object(m));
}

pub fn clear_session() {
    let p = session_path();
    if p.exists() {
        let _ = std::fs::remove_file(p);
    }
}

/// A fresh identifier per game launch (uuid4().hex).
pub fn install_id() -> String {
    let mut b = [0u8; 16];
    if let Ok(mut f) = std::fs::File::open("/dev/urandom") {
        use std::io::Read;
        let _ = f.read_exact(&mut b);
    } else {
        let t = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_nanos()).unwrap_or(0);
        b = t.to_le_bytes();
    }
    b[6] = (b[6] & 0x0f) | 0x40;
    b[8] = (b[8] & 0x3f) | 0x80;
    b.iter().map(|x| format!("{:02x}", x)).collect()
}

pub fn cache_path(uid: &str, slot: i64, suffix: &str) -> PathBuf {
    let mut safe: String = uid.chars().filter(|c| c.is_ascii_alphanumeric() || *c == '_' || *c == '-').collect();
    if safe.is_empty() {
        safe = "unknown".into();
    }
    cache_dir().join(safe).join(format!("slot{}{}.json", slot, suffix))
}

#[derive(Clone, Debug)]
pub struct CacheEntry {
    pub state: Value,
    pub base_time: Option<String>,
    pub base_time_raw: Value,
    pub dirty: bool,
}

pub fn read_cache(uid: &str, slot: i64) -> Option<CacheEntry> {
    let text = std::fs::read_to_string(cache_path(uid, slot, "")).ok()?;
    let Value::Object(d) = serde_json::from_str::<Value>(&text).ok()? else {
        return None;
    };
    let state = d.get("state")?;
    if !state.is_object() {
        return None;
    }
    let raw = d.get("base_time").cloned().unwrap_or(Value::Null);
    Some(CacheEntry {
        state: state.clone(),
        base_time: raw.as_str().map(|s| s.to_string()),
        base_time_raw: raw,
        dirty: d.get("dirty").map(value_truthy).unwrap_or(true),
    })
}

pub fn write_cache_suffix(uid: &str, slot: i64, base_time: Option<&str>, dirty: bool, state: &Value, suffix: &str) {
    let path = cache_path(uid, slot, suffix);
    if let Some(dir) = path.parent() {
        if std::fs::create_dir_all(dir).is_err() {
            return;
        }
    }
    let mut m = Map::new();
    m.insert("base_time".into(), base_time.map(|s| json!(s)).unwrap_or(Value::Null));
    m.insert("dirty".into(), json!(dirty));
    m.insert("state".into(), state.clone());
    write_atomic(&path, &Value::Object(m));
}

pub fn write_cache(uid: &str, slot: i64, base_time: Option<&str>, dirty: bool, state: &Value) {
    write_cache_suffix(uid, slot, base_time, dirty, state, "");
}

pub fn delete_cache(uid: &str, slot: i64) {
    let p = cache_path(uid, slot, "");
    if p.exists() {
        let _ = std::fs::remove_file(p);
    }
}

pub fn backup_state(uid: &str, slot: i64, label: &str, state: &Value) {
    let now = crate::core::state::now_ts() as i64;
    write_cache_suffix(uid, slot, None, false, state, &format!("_{}_backup_{}", label, now));
}

pub struct CloudSave {
    pub data: Value,
    pub time: Option<String>,
}

pub struct Picked {
    pub state: Option<Value>,
    pub base_time: Option<String>,
    pub dirty: bool,
    pub note: Option<String>,
    pub loser: Option<(&'static str, Value)>,
}

pub fn pick_save(cloud: Option<&CloudSave>, cache: Option<&CacheEntry>) -> Picked {
    let p = |state: Option<Value>, bt: Option<String>, dirty: bool| Picked { state, base_time: bt, dirty, note: None, loser: None };
    match (cloud, cache) {
        (None, None) => p(None, None, true),
        (Some(c), None) => p(Some(c.data.clone()), c.time.clone(), false),
        (None, Some(k)) => p(Some(k.state.clone()), None, true),
        (Some(c), Some(k)) => {
            let same = match (&k.base_time_raw, &c.time) {
                (Value::String(a), Some(b)) => a == b,
                (Value::Null, None) => true,
                _ => false,
            };
            if same {
                if k.dirty {
                    return p(Some(k.state.clone()), c.time.clone(), true);
                }
                return p(Some(c.data.clone()), c.time.clone(), false);
            }
            if !k.dirty {
                return p(Some(c.data.clone()), c.time.clone(), false);
            }
            let pt = |v: &Value| -> Option<f64> {
                match v.as_object()?.get("playtime") {
                    None => Some(0.0),
                    Some(x) => crate::storage::value_f64(x),
                }
            };
            let (local_pt, cloud_pt) = match (pt(&k.state), pt(&c.data)) {
                (Some(a), Some(b)) => (a, b),
                _ => (0.0, 0.0),
            };
            if local_pt > cloud_pt {
                return Picked {
                    state: Some(k.state.clone()),
                    base_time: c.time.clone(),
                    dirty: true,
                    note: Some(crate::i18n::tr("Kept this PC's progress (it was ahead of the cloud). The cloud version was backed up.")),
                    loser: Some(("cloud", c.data.clone())),
                };
            }
            Picked {
                state: Some(c.data.clone()),
                base_time: c.time.clone(),
                dirty: false,
                note: Some(crate::i18n::tr("The cloud save was ahead of this PC. Your local version was backed up.")),
                loser: Some(("local", k.state.clone())),
            }
        }
    }
}

#[derive(Clone, Debug)]
pub struct CachePeek {
    pub coins: f64,
    pub total_rolls: i64,
    pub playtime: f64,
    pub rebirths: i64,
}

pub fn peek_cache(uid: &str, slot: i64) -> Option<CachePeek> {
    let c = read_cache(uid, slot)?;
    let s = crate::online::firebase::save_summary(&c.state);
    Some(CachePeek { coins: s.coins, total_rolls: s.rolls, playtime: s.playtime, rebirths: s.rebirths })
}

// ---- leaderboard: 10-minute periods on the clock ----
use crate::online::firebase::{Client, LEADERBOARD_FETCH_DELAY, LEADERBOARD_PERIOD, LEADERBOARD_SIZE, Res};

pub fn lb_publish_period(now: f64) -> i64 {
    (now / LEADERBOARD_PERIOD).floor() as i64
}
pub fn lb_fetch_period(now: f64) -> i64 {
    ((now - LEADERBOARD_FETCH_DELAY) / LEADERBOARD_PERIOD).floor() as i64
}
pub fn lb_next_update(now: f64) -> f64 {
    (lb_fetch_period(now) + 1) as f64 * LEADERBOARD_PERIOD + LEADERBOARD_FETCH_DELAY
}

pub const LB_FIELDS: [(&str, &str); 4] = [("money", "coins"), ("playtime", "playtime"), ("rolls", "rolls"), ("rebirths", "rebirths")];

pub fn fetch_leaderboards(client: &Client, period: i64) -> Res<Value> {
    let mut data = Map::new();
    for (key, field) in LB_FIELDS {
        let entries = client.top_entries(field, LEADERBOARD_SIZE, if key == "rebirths" { Some(1) } else { None })?;
        let arr: Vec<Value> = entries
            .into_iter()
            .map(|e| {
                let mut m = Map::new();
                m.insert("username".into(), json!(e.username));
                m.insert("value".into(), pyjson::float(e.value));
                Value::Object(m)
            })
            .collect();
        data.insert(key.into(), Value::Array(arr));
    }
    data.insert("period".into(), json!(period));
    data.insert("fetched_at".into(), pyjson::float(crate::core::state::now_ts()));
    Ok(Value::Object(data))
}

pub fn lb_data_complete(d: &Value) -> bool {
    d.is_object() && LB_FIELDS.iter().all(|(k, _)| d.get(k).is_some_and(|v| v.is_array()))
}

pub fn load_lb_cache() -> Option<Value> {
    let t = std::fs::read_to_string(leaderboard_cache_path()).ok()?;
    let d: Value = serde_json::from_str(&t).ok()?;
    if d.is_object() && d.get("money").is_some_and(|v| v.is_array()) && d.get("playtime").is_some_and(|v| v.is_array()) {
        return Some(d);
    }
    None
}

pub fn store_lb_cache(data: &Value) {
    write_atomic(&leaderboard_cache_path(), data);
}
