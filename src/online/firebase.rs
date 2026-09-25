//! Firebase client (accounts + Firestore over REST), network errors and the background Worker
//! (online/firebase.py).

use crate::config::save_dir;
use crate::i18n::{tr, tra};
use serde_json::{Map, Value, json};
use std::any::Any;
use std::collections::HashMap;
use std::sync::mpsc::{Receiver, Sender, channel};
use std::sync::{Arc, Mutex};
use std::time::Duration;

pub const FIREBASE_API_KEY: &str = "AIzaSyDDRAionwoM0sZ0RWGwqNk14RY77zY1KvI";
pub const FIREBASE_PROJECT_ID: &str = "lucky-sahurs";
pub const USERNAME_DOMAIN: &str = "luckysahurs.invalid";
pub const MIN_PASSWORD: usize = 6;
pub const NET_TIMEOUT: f64 = 8.0;

pub const LEADERBOARD_PERIOD: f64 = 600.0;
pub const LEADERBOARD_PUBLISH_JITTER: f64 = 30.0;
pub const LEADERBOARD_FETCH_DELAY: f64 = 60.0;
pub const LEADERBOARD_MIN_GAP: f64 = 360.0;
pub const LEADERBOARD_SIZE: usize = 50;
/// Reading the leaderboard is a shared "snapshot", taken at most once per period for everyone (see
/// backend/apps_script/Leaderboard.gs): the game reads just /public/leaderboard (1 read) instead of running the 4
/// queries (up to 200 reads) itself. Must equal PERIOD_SECONDS in the script.
pub const LEADERBOARD_SNAPSHOT_PERIOD: f64 = 30.0 * 60.0;
/// The Leaderboard.gs Web App URL (NOT secret). Empty = the game runs the queries itself, like before.
pub const LEADERBOARD_ENDPOINT: &str =
    "https://script.google.com/macros/s/AKfycbxZDkqzPQvPwj6mtm_ePAObaPUMaIu8wcb0kGPKyEwS85gAz64Tsef9zKeN5W0po32f/exec";
/// taking the snapshot (4 queries in the script) can take a few seconds
pub const LEADERBOARD_ENDPOINT_TIMEOUT: f64 = 30.0;
/// Friends online: while you play, your /profiles/{uid} gets the server time ("last_seen") every X seconds.
/// A friend counts as online if that time is less than PRESENCE_ONLINE seconds old.
pub const PRESENCE_INTERVAL: f64 = 120.0;
pub const PRESENCE_ONLINE: f64 = 5.0 * 60.0;
pub const CLOUD_SYNC_INTERVAL: f64 = 90.0;
pub const SESSION_HEARTBEAT: f64 = 30.0;
pub const SESSION_STALE: f64 = 90.0;

/// ^[a-z0-9_]{3,16}$
pub fn username_ok(s: &str) -> bool {
    let n = s.chars().count();
    (3..=16).contains(&n) && s.chars().all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_')
}

pub fn now() -> f64 {
    crate::core::state::now_ts()
}

pub fn load_firebase_config() -> (String, String) {
    let (mut key, mut pid) = (FIREBASE_API_KEY.trim().to_string(), FIREBASE_PROJECT_ID.trim().to_string());
    let path = save_dir().join("firebase_config.json");
    if (key.is_empty() || pid.is_empty()) && path.exists() {
        if let Ok(Value::Object(d)) = std::fs::read_to_string(&path).map(|t| serde_json::from_str(&t).unwrap_or(Value::Null)) {
            let get = |k: &str| d.get(k).map(|v| v.as_str().map(|s| s.to_string()).unwrap_or_else(|| v.to_string())).unwrap_or_default();
            if key.is_empty() {
                key = get("api_key").trim().to_string();
            }
            if pid.is_empty() {
                pid = get("project_id").trim().to_string();
            }
        }
    }
    (key, pid)
}

pub fn username_to_email(username: &str) -> String {
    format!("{}@{}", username.to_lowercase(), USERNAME_DOMAIN)
}

/// code: 'offline' | 'auth' | 'denied' | 'not_found' | 'conflict' | 'bad_request' | 'server'
#[derive(Clone, Debug)]
pub struct OnlineError {
    pub code: String,
    pub message: String,
    pub status: u16,
}

impl OnlineError {
    pub fn new(code: &str, message: impl Into<String>) -> OnlineError {
        OnlineError { code: code.into(), message: message.into(), status: 0 }
    }
    pub fn with_status(code: &str, message: impl Into<String>, status: u16) -> OnlineError {
        OnlineError { code: code.into(), message: message.into(), status }
    }
}

pub type Res<T> = Result<T, OnlineError>;

const AUTH_MESSAGES: [(&str, &str); 10] = [
    ("EMAIL_EXISTS", "That username is already taken."),
    ("INVALID_LOGIN_CREDENTIALS", "Wrong username or password."),
    ("INVALID_PASSWORD", "Wrong username or password."),
    ("EMAIL_NOT_FOUND", "Wrong username or password."),
    ("WEAK_PASSWORD", "Password must have at least 6 characters."),
    ("TOO_MANY_ATTEMPTS", "Too many attempts. Try again in a few minutes."),
    ("USER_DISABLED", "This account has been disabled."),
    ("OPERATION_NOT_ALLOWED", "Email/Password sign-in is not enabled in the Firebase project."),
    ("CONFIGURATION_NOT_FOUND", "Firebase Authentication isn't set up in this project yet."),
    ("API KEY", "The Firebase API key is not valid."),
];

pub fn online_error_text(e: &OnlineError) -> String {
    if e.code == "offline" {
        return tr("Can't reach the server. Check your internet connection.");
    }
    if e.code == "denied" {
        return tr("The server refused this. Check the Firestore rules.");
    }
    if e.status == 429 || e.message.to_uppercase().contains("RESOURCE_EXHAUSTED") {
        return tr("The server's free daily limit was reached. It comes back on its own later today.");
    }
    if e.code == "auth" {
        return tr("Session expired. Please log in again.");
    }
    if e.code == "conflict" {
        return tr("This was changed somewhere else. Try again.");
    }
    let msg = e.message.to_uppercase();
    for (key, text) in AUTH_MESSAGES {
        if msg.contains(key) {
            return tr(text);
        }
    }
    let m = if e.message.is_empty() { e.code.clone() } else { e.message.clone() };
    tra("Something went wrong (%s).", &crate::args![m])
}

fn classify_http_error(status: u16, body: &str) -> OnlineError {
    let payload: Value = serde_json::from_str(body).unwrap_or(Value::Object(Map::new()));
    let err = match &payload {
        Value::Object(o) => o.get("error").cloned().unwrap_or(Value::Object(Map::new())),
        _ => Value::Object(Map::new()),
    };
    let err = match err {
        Value::Object(o) => o,
        other => {
            let mut m = Map::new();
            m.insert("message".into(), Value::String(py_str(&other)));
            m
        }
    };
    let message = err.get("message").map(py_str).unwrap_or_default();
    let fs_status = err.get("status").map(py_str).unwrap_or_default();
    let short = message.split(" : ").next().unwrap_or("").trim().to_string();
    if status == 409 || status == 412 || ["ABORTED", "ALREADY_EXISTS", "FAILED_PRECONDITION"].contains(&fs_status.as_str()) {
        return OnlineError::with_status("conflict", short, status);
    }
    if status == 404 || fs_status == "NOT_FOUND" {
        return OnlineError::with_status("not_found", short, status);
    }
    if status == 401 || fs_status == "UNAUTHENTICATED" {
        return OnlineError::with_status("auth", short, status);
    }
    if status == 403 || fs_status == "PERMISSION_DENIED" {
        return OnlineError::with_status("denied", short, status);
    }
    if status == 400 {
        return OnlineError::with_status("bad_request", short, status);
    }
    let m = if short.is_empty() { format!("HTTP {}", status) } else { short };
    OnlineError::with_status("server", m, status)
}

/// Python str() of a JSON value.
fn py_str(v: &Value) -> String {
    match v {
        Value::String(s) => s.clone(),
        Value::Null => "None".into(),
        Value::Bool(b) => if *b { "True".into() } else { "False".into() },
        other => other.to_string(),
    }
}

// ---- server clock ----
static CLOCK: Mutex<(f64, bool)> = Mutex::new((0.0, false));

fn note_server_date(date: Option<&str>) {
    let Some(d) = date else { return };
    if let Ok(t) = chrono::DateTime::parse_from_rfc2822(d.trim()) {
        let stamp = t.timestamp() as f64;
        if let Ok(mut c) = CLOCK.lock() {
            *c = (stamp - now(), true);
        }
    }
}

pub fn server_now() -> f64 {
    now() + CLOCK.lock().map(|c| c.0).unwrap_or(0.0)
}

pub fn server_clock_synced() -> bool {
    CLOCK.lock().map(|c| c.1).unwrap_or(false)
}

/// '2026-09-21T12:34:56.789Z' -> seconds since the epoch.
pub fn parse_timestamp(v: Option<&FsVal>) -> Option<f64> {
    let Some(FsVal::Str(text)) = v else { return None };
    let head = text.trim().trim_end_matches('Z');
    let (head, frac) = match head.split_once('.') {
        Some((h, dot)) => {
            let digits: String = dot.chars().filter(|c| c.is_ascii_digit()).collect();
            (h, format!("0.{}", digits).parse::<f64>().unwrap_or(0.0))
        }
        None => (head, 0.0),
    };
    let dt = chrono::NaiveDateTime::parse_from_str(head, "%Y-%m-%dT%H:%M:%S").ok()?;
    Some(dt.and_utc().timestamp() as f64 + frac)
}

pub fn iso_timestamp(secs: f64) -> String {
    chrono::DateTime::from_timestamp(secs as i64, 0).map(|d| d.format("%Y-%m-%dT%H:%M:%SZ").to_string()).unwrap_or_default()
}

fn quote(s: &str, safe: &str) -> String {
    let mut out = String::new();
    for b in s.bytes() {
        let c = b as char;
        if c.is_ascii_alphanumeric() || "_.-~".contains(c) || safe.contains(c) {
            out.push(c);
        } else {
            out.push_str(&format!("%{:02X}", b));
        }
    }
    out
}

fn quote_plus(s: &str) -> String {
    quote(s, " ").replace(' ', "+")
}

fn urlencode(pairs: &[(String, String)]) -> String {
    pairs.iter().map(|(k, v)| format!("{}={}", quote_plus(k), quote_plus(v))).collect::<Vec<_>>().join("&")
}

fn agent() -> &'static ureq::Agent {
    static A: std::sync::OnceLock<ureq::Agent> = std::sync::OnceLock::new();
    A.get_or_init(|| {
        ureq::Agent::config_builder()
            .timeout_global(Some(Duration::from_secs_f64(NET_TIMEOUT)))
            .http_status_as_error(false)
            .build()
            .into()
    })
}

/// The same with a longer timeout (the leaderboard script).
fn slow_agent() -> &'static ureq::Agent {
    static A: std::sync::OnceLock<ureq::Agent> = std::sync::OnceLock::new();
    A.get_or_init(|| {
        ureq::Agent::config_builder()
            .timeout_global(Some(Duration::from_secs_f64(LEADERBOARD_ENDPOINT_TIMEOUT)))
            .http_status_as_error(false)
            .build()
            .into()
    })
}

pub enum Body {
    None,
    Json(Value),
    Form(Vec<(String, String)>),
}

pub fn http_json(method: &str, url: &str, body: Body, headers: &[(&str, String)]) -> Res<Value> {
    http_json_with(agent(), method, url, body, headers)
}

fn http_json_with(a: &ureq::Agent, method: &str, url: &str, body: Body, headers: &[(&str, String)]) -> Res<Value> {
    let mut hdrs: Vec<(String, String)> = vec![("Accept".into(), "application/json".into())];
    let data: Option<Vec<u8>> = match &body {
        Body::None => None,
        Body::Json(v) => {
            hdrs.push(("Content-Type".into(), "application/json".into()));
            Some(serde_json::to_vec(v).unwrap_or_default())
        }
        Body::Form(p) => {
            hdrs.push(("Content-Type".into(), "application/x-www-form-urlencoded".into()));
            Some(urlencode(p).into_bytes())
        }
    };
    for (k, v) in headers {
        hdrs.push((k.to_string(), v.clone()));
    }
    let result = match (method, data) {
        ("GET", _) => {
            let mut r = a.get(url);
            for (k, v) in &hdrs {
                r = r.header(k.as_str(), v.as_str());
            }
            r.call()
        }
        ("DELETE", _) => {
            let mut r = a.delete(url);
            for (k, v) in &hdrs {
                r = r.header(k.as_str(), v.as_str());
            }
            r.call()
        }
        (m, d) => {
            let mut r = match m {
                "PATCH" => a.patch(url),
                "PUT" => a.put(url),
                _ => a.post(url),
            };
            for (k, v) in &hdrs {
                r = r.header(k.as_str(), v.as_str());
            }
            match d {
                Some(d) => r.send(&d[..]),
                None => r.send_empty(),
            }
        }
    };
    let mut resp = result.map_err(|e| OnlineError::new("offline", e.to_string()))?;
    let date = resp.headers().get("date").and_then(|v| v.to_str().ok()).map(|s| s.to_string());
    note_server_date(date.as_deref());
    let status = resp.status().as_u16();
    let raw = resp
        .body_mut()
        .with_config()
        .limit(64 * 1024 * 1024)
        .read_to_string()
        .map_err(|e| OnlineError::new("offline", e.to_string()))?;
    if status >= 400 {
        return Err(classify_http_error(status, &raw));
    }
    if raw.is_empty() {
        return Ok(Value::Object(Map::new()));
    }
    serde_json::from_str(&raw).map_err(|_| OnlineError::new("server", "invalid response"))
}

// ---- Firestore values ----
#[derive(Clone, Debug, PartialEq)]
pub enum FsVal {
    Str(String),
    F(f64),
    I(i64),
    B(bool),
}

impl FsVal {
    /// Python truthiness
    pub fn truthy(&self) -> bool {
        match self {
            FsVal::Str(s) => !s.is_empty(),
            FsVal::F(f) => *f != 0.0,
            FsVal::I(i) => *i != 0,
            FsVal::B(b) => *b,
        }
    }
    pub fn py_str(&self) -> String {
        match self {
            FsVal::Str(s) => s.clone(),
            FsVal::F(f) => crate::pyfmt::py_float_str(*f),
            FsVal::I(i) => i.to_string(),
            FsVal::B(b) => if *b { "True".into() } else { "False".into() },
        }
    }
    pub fn to_f64(&self) -> f64 {
        match self {
            FsVal::Str(s) => s.trim().parse().unwrap_or(0.0),
            FsVal::F(f) => *f,
            FsVal::I(i) => *i as f64,
            FsVal::B(b) => *b as i64 as f64,
        }
    }
    pub fn to_i64(&self) -> i64 {
        match self {
            FsVal::Str(s) => s.trim().parse().unwrap_or(0),
            FsVal::F(f) => f.trunc() as i64,
            FsVal::I(i) => *i,
            FsVal::B(b) => *b as i64,
        }
    }
}

pub fn fs_str(s: &str) -> Value {
    json!({"stringValue": s})
}
pub fn fs_int(i: i64) -> Value {
    json!({"integerValue": i.to_string()})
}
pub fn fs_bool(b: bool) -> Value {
    json!({"booleanValue": b})
}
pub fn fs_f64(v: f64) -> Value {
    let v = if v.is_nan() { 0.0 } else { v.clamp(-1.7e308, 1.7e308) };
    json!({"doubleValue": v})
}

fn fs_decode(v: &Value) -> Option<FsVal> {
    let o = v.as_object()?;
    if let Some(s) = o.get("stringValue") {
        return Some(FsVal::Str(s.as_str().map(|x| x.to_string()).unwrap_or_else(|| py_str(s))));
    }
    if let Some(d) = o.get("doubleValue") {
        return Some(FsVal::F(match d {
            Value::Number(n) => n.as_f64().unwrap_or(0.0),
            Value::String(s) => match s.as_str() {
                "NaN" => f64::NAN,
                "Infinity" => f64::INFINITY,
                "-Infinity" => f64::NEG_INFINITY,
                x => x.parse().unwrap_or(0.0),
            },
            _ => 0.0,
        }));
    }
    if let Some(i) = o.get("integerValue") {
        return Some(FsVal::I(match i {
            Value::String(s) => s.parse().unwrap_or(0),
            Value::Number(n) => n.as_i64().unwrap_or(0),
            _ => 0,
        }));
    }
    if let Some(b) = o.get("booleanValue") {
        return Some(FsVal::B(b.as_bool().unwrap_or(false)));
    }
    if let Some(t) = o.get("timestampValue") {
        return Some(FsVal::Str(t.as_str().unwrap_or("").to_string()));
    }
    None
}

#[derive(Default, Debug, Clone)]
pub struct Fields(pub HashMap<String, FsVal>);

impl Fields {
    pub fn get(&self, k: &str) -> Option<&FsVal> {
        self.0.get(k)
    }
    /// str(f.get(k) or default)
    pub fn str_or(&self, k: &str, default: &str) -> String {
        match self.0.get(k) {
            Some(v) if v.truthy() => v.py_str(),
            _ => default.to_string(),
        }
    }
    /// f.get(k) or None (as a string)
    pub fn opt_str(&self, k: &str) -> Option<String> {
        match self.0.get(k) {
            Some(v) if v.truthy() => Some(v.py_str()),
            _ => None,
        }
    }
    pub fn f64_or0(&self, k: &str) -> f64 {
        match self.0.get(k) {
            Some(v) if v.truthy() => v.to_f64(),
            _ => 0.0,
        }
    }
    pub fn i64_or0(&self, k: &str) -> i64 {
        match self.0.get(k) {
            Some(v) if v.truthy() => v.to_i64(),
            _ => 0,
        }
    }
    pub fn truthy(&self, k: &str) -> bool {
        self.0.get(k).map(|v| v.truthy()).unwrap_or(false)
    }
}

pub fn fs_fields(doc: &Value) -> Fields {
    let mut out = HashMap::new();
    if let Some(Value::Object(f)) = doc.get("fields") {
        for (k, v) in f {
            if let Some(d) = fs_decode(v) {
                out.insert(k.clone(), d);
            }
        }
    }
    Fields(out)
}

fn doc_id(doc: &Value) -> String {
    let name = doc.get("name").map(py_str).unwrap_or_default();
    name.rsplit('/').next().unwrap_or("").to_string()
}

pub fn new_doc_id() -> String {
    crate::online::cloud_cache::install_id()[..20].to_string()
}

#[derive(Clone, Debug)]
pub struct Event {
    pub kind: String,
    pub mult: f64,
    pub ends_at: f64,
    pub by: String,
}

fn event_from_doc(doc: &Value, kind: &str) -> Event {
    let f = fs_fields(doc);
    let ends = match f.get("ends_at") {
        Some(FsVal::Str(_)) => parse_timestamp(f.get("ends_at")),
        Some(v) if v.truthy() => Some(v.to_f64()),
        _ => None,
    };
    let mult = match f.get("mult") {
        Some(v) if v.truthy() => v.to_f64(),
        _ => 1.0,
    };
    Event { kind: kind.to_string(), mult, ends_at: ends.filter(|e| *e != 0.0).unwrap_or(0.0), by: f.str_or("by", "") }
}

#[derive(Clone, Debug)]
pub struct Feedback {
    pub uid: String,
    pub username: String,
    pub text: String,
    pub updated_at: Option<f64>,
}

fn feedback_from_doc(doc: &Value) -> Feedback {
    let f = fs_fields(doc);
    Feedback { uid: doc_id(doc), username: f.str_or("username", "?"), text: f.str_or("text", ""), updated_at: parse_timestamp(f.get("updated_at")) }
}

/// A person in the friends lists / search results.
#[derive(Clone, Debug, Default)]
pub struct Person {
    pub uid: String,
    pub username: String,
    pub avatar_pet: Option<i64>,
    pub avatar_mut: String,
    #[allow(dead_code)]
    pub time: Option<f64>,
    /// the server time of their last "I'm playing" (None = never published / an old version)
    pub last_seen: Option<f64>,
    /// their equipped title (core/titles.rs), None = none / not loaded
    pub title: Option<String>,
}

fn profile_from_doc(doc: &Value) -> Person {
    let f = fs_fields(doc);
    let pet = match f.get("avatar_pet") {
        Some(FsVal::I(i)) => *i,
        Some(FsVal::F(x)) if x.is_finite() => x.trunc() as i64,
        Some(FsVal::B(b)) => *b as i64,
        Some(FsVal::Str(s)) => s.trim().parse().unwrap_or(-1),
        _ => -1,
    };
    Person {
        uid: doc_id(doc),
        username: f.str_or("username", "?"),
        avatar_pet: if pet < 0 { None } else { Some(pet) },
        avatar_mut: f.str_or("avatar_mut", "normal"),
        time: None,
        last_seen: parse_timestamp(f.get("last_seen")),
        title: None,
    }
}

/// Path of a pet's field in the wallet ("owned.<key>"). Pet keys start with a digit ("3_golden") and Firestore
/// only accepts that in a field path between backticks: `3_golden`. Without them the server refused EVERY wallet
/// increment (accepting trades, adding rolls, applying receipts).
pub fn wallet_field_path(key: &str) -> String {
    let simple = key.chars().next().is_some_and(|c| c.is_ascii_alphabetic() || c == '_')
        && key.chars().all(|c| c.is_ascii_alphanumeric() || c == '_');
    if simple { format!("owned.{}", key) } else { format!("owned.`{}`", key.replace('\\', "\\\\").replace('`', "\\`")) }
}

#[derive(Clone, Debug)]
pub struct Ban {
    pub uid: String,
    pub username: String,
    pub reason: String,
    pub by: String,
    pub at: Option<f64>,
}

fn ban_from_doc(doc: &Value) -> Ban {
    let f = fs_fields(doc);
    Ban { uid: doc_id(doc), username: f.str_or("username", "?"), reason: f.str_or("reason", ""), by: f.str_or("by", ""), at: parse_timestamp(f.get("at")) }
}

/// A pet trade between friends (only exists while it's pending).
#[derive(Clone, Debug, Default)]
pub struct Trade {
    pub id: String,
    pub from_uid: String,
    #[allow(dead_code)]
    pub to_uid: String,
    pub offer: Vec<(String, i64)>,
    pub request: Vec<(String, i64)>,
}

/// An online battle between two friends (/battles/{id}). Only the teams, a seed and each side's moves are stored:
/// both games work out the very same battle from them (core/battle.rs).
#[derive(Clone, Debug, Default)]
pub struct BattleDoc {
    pub id: String,
    pub from_uid: String,
    pub to_uid: String,
    pub from_name: String,
    pub to_name: String,
    /// "pet_mutation_phase,..." (up to 3)
    pub from_team: String,
    pub to_team: String,
    pub seed: i64,
    /// "pending" (waiting for the friend), "live" or "over"
    pub status: String,
    /// a ranked match (from the queue) and both players' ratings when it started
    pub ranked: bool,
    pub from_rating: i64,
    pub to_rating: i64,
    /// one letter per action: S Strike, P Special, G Guard, R Rest, 0-2 switch to that team member
    pub from_moves: String,
    pub to_moves: String,
    pub created: f64,
}

fn battle_from_doc(doc: &Value) -> BattleDoc {
    let f = fs_fields(doc);
    BattleDoc {
        id: doc_id(doc),
        from_uid: f.str_or("from_uid", ""),
        to_uid: f.str_or("to_uid", ""),
        from_name: f.str_or("from_name", "?"),
        to_name: f.str_or("to_name", "?"),
        from_team: f.str_or("from_team", ""),
        to_team: f.str_or("to_team", ""),
        seed: f.i64_or0("seed"),
        status: f.str_or("status", "pending"),
        ranked: f.truthy("ranked"),
        from_rating: f.i64_or0("from_rating"),
        to_rating: f.i64_or0("to_rating"),
        from_moves: f.str_or("from_moves", ""),
        to_moves: f.str_or("to_moves", ""),
        created: f.f64_or0("created"),
    }
}

/// Someone waiting in the ranked queue (/queue/{uid}).
#[derive(Clone, Debug, Default)]
pub struct QueueEntry {
    pub uid: String,
    pub name: String,
    pub rating: i64,
    pub team: String,
    /// server time of their last "still here"
    pub at: f64,
}

/// A line of the ranked top list (/ranks/{uid}).
#[derive(Clone, Debug, Default)]
pub struct RankEntry {
    pub name: String,
    pub rating: i64,
    pub wins: i64,
    pub losses: i64,
}

/// A receipt left in /users/{uid}/incoming by whoever accepted my trade: what I get and what I lose.
#[derive(Clone, Debug)]
pub struct Receipt {
    pub id: String,
    pub give: Vec<(String, i64)>,
    pub take: Vec<(String, i64)>,
}

pub const MAX_TRADE_ITEMS: usize = 3;

fn trade_item_fields(fields: &mut Map<String, Value>, prefix: &str, items: &[(String, i64)]) {
    for i in 0..MAX_TRADE_ITEMS {
        let (key, qty) = items.get(i).cloned().unwrap_or_default();
        fields.insert(format!("{}_key{}", prefix, i + 1), fs_str(&key));
        fields.insert(format!("{}_qty{}", prefix, i + 1), fs_int(qty));
    }
}

fn trade_items_from_fields(f: &Fields, prefix: &str) -> Vec<(String, i64)> {
    let mut out = Vec::new();
    for i in 0..MAX_TRADE_ITEMS {
        let key = f.str_or(&format!("{}_key{}", prefix, i + 1), "");
        let qty = f.i64_or0(&format!("{}_qty{}", prefix, i + 1));
        if !key.is_empty() && qty > 0 {
            out.push((key, qty));
        }
    }
    out
}

/// {"idx_mut": count} of a mapValue field (the wallet's "owned").
fn int_map(doc: &Value, field: &str) -> HashMap<String, i64> {
    let mut out = HashMap::new();
    if let Some(Value::Object(m)) = doc.get("fields").and_then(|f| f.get(field)).and_then(|v| v.get("mapValue")).and_then(|m| m.get("fields")) {
        for (k, v) in m {
            if let Some(x) = fs_decode(v) {
                out.insert(k.clone(), x.to_i64());
            }
        }
    }
    out
}

fn increments(delta: &[(String, i64)]) -> Vec<Value> {
    delta.iter().map(|(k, q)| json!({"fieldPath": wallet_field_path(k), "increment": fs_int(*q)})).collect()
}

/// Adds up a list of (key, qty) the way Python's dict does (first-seen order).
fn add_delta(delta: &mut Vec<(String, i64)>, key: &str, q: i64) {
    match delta.iter_mut().find(|(k, _)| k == key) {
        Some(e) => e.1 += q,
        None => delta.push((key.to_string(), q)),
    }
}

#[derive(Clone, Debug, Default)]
pub struct SaveSummary {
    pub coins: f64,
    pub total_earned: f64,
    pub playtime: f64,
    pub rolls: i64,
    pub rebirths: i64,
    pub time: Option<String>,
}

pub fn save_summary(d: &Value) -> SaveSummary {
    let num = |k: &str| -> f64 {
        let v = match d.as_object().and_then(|o| o.get(k)) {
            None => Some(0.0),
            Some(v) => crate::storage::value_f64(v),
        };
        match v {
            Some(x) if !x.is_nan() => x,
            _ => 0.0,
        }
    };
    let coins = num("coins");
    SaveSummary {
        coins,
        total_earned: coins.max(num("total_coins_earned")),
        playtime: num("playtime").max(0.0),
        rolls: num("total_rolls").min(9e18).max(0.0) as i64,
        rebirths: num("rebirths").min(1e9).max(0.0) as i64,
        time: None,
    }
}

#[derive(Clone, Debug)]
pub struct AccountProfile {
    pub email: Option<String>,
    pub email_verified: bool,
    pub pending_code: Option<String>,
    pub code_created: Option<f64>,
}

#[derive(Clone, Debug)]
pub struct SessionLock {
    pub session_id: String,
    pub active: bool,
    pub heartbeat: Option<f64>,
}

#[derive(Clone, Debug)]
pub struct PublicStats {
    #[allow(dead_code)]
    pub username: String,
    pub coins: f64,
    pub playtime: f64,
    pub rolls: i64,
    pub rebirths: i64,
    pub updated_at: Option<f64>,
}

#[derive(Clone, Debug)]
pub struct Message {
    #[allow(dead_code)]
    pub id: String,
    pub from_uid: String,
    pub text: String,
    pub sent_at: Option<f64>,
}

#[derive(Clone, Debug)]
pub struct CloudDoc {
    pub data: Value,
    pub time: Option<String>,
}

#[derive(Clone, Debug)]
pub struct LbEntry {
    pub username: String,
    pub value: f64,
    /// v3.0 (0 = none / an older version of the game)
    pub prestige: i64,
    pub rebirths: i64,
}

#[derive(Default)]
struct Tokens {
    uid: Option<String>,
    username: Option<String>,
    refresh_token: Option<String>,
    id_token: Option<String>,
    expires_at: f64,
}

/// Minimal Firebase Auth + Firestore client (REST). Safe to use from threads.
pub struct FirebaseClient {
    pub api_key: String,
    #[allow(dead_code)]
    pub project_id: String,
    identity_base: String,
    token_base: String,
    firestore_base: String,
    docs_root: String,
    tok: Mutex<Tokens>,
}

const SUMMARY_FIELDS: [&str; 5] = ["coins", "total_earned", "playtime", "rolls", "rebirths"];

fn expires(v: Option<&Value>, default: &str) -> f64 {
    let s = match v {
        None => default.to_string(),
        Some(Value::String(s)) => s.clone(),
        Some(Value::Number(n)) => n.to_string(),
        Some(_) => String::new(),
    };
    match s.trim().parse::<i64>() {
        Ok(i) => now() + i as f64,
        Err(_) => now() + 3000.0,
    }
}

impl FirebaseClient {
    pub fn new(api_key: &str, project_id: &str) -> FirebaseClient {
        FirebaseClient {
            api_key: api_key.into(),
            project_id: project_id.into(),
            identity_base: "https://identitytoolkit.googleapis.com/v1".into(),
            token_base: "https://securetoken.googleapis.com/v1".into(),
            firestore_base: "https://firestore.googleapis.com/v1".into(),
            docs_root: format!("projects/{}/databases/(default)/documents", project_id),
            tok: Mutex::new(Tokens::default()),
        }
    }

    fn t(&self) -> std::sync::MutexGuard<'_, Tokens> {
        self.tok.lock().unwrap_or_else(|e| e.into_inner())
    }

    pub fn signed_in(&self) -> bool {
        let t = self.t();
        t.uid.as_deref().is_some_and(|s| !s.is_empty()) && t.refresh_token.as_deref().is_some_and(|s| !s.is_empty())
    }
    pub fn uid(&self) -> Option<String> {
        self.t().uid.clone()
    }
    pub fn username(&self) -> Option<String> {
        self.t().username.clone()
    }
    pub fn refresh_token(&self) -> Option<String> {
        self.t().refresh_token.clone()
    }

    fn set_tokens(&self, uid: String, username: Option<String>, refresh_token: String, id_token: String, expires_in: Option<&Value>) {
        let mut t = self.t();
        t.uid = Some(uid);
        t.username = username;
        t.refresh_token = Some(refresh_token);
        t.id_token = Some(id_token);
        t.expires_at = expires(expires_in, "3600");
    }

    fn auth_call(&self, action: &str, email: &str, password: &str, username: &str) -> Res<()> {
        let body = json!({"email": email, "password": password, "returnSecureToken": true});
        let url = format!("{}/accounts:{}?key={}", self.identity_base, action, quote(&self.api_key, "/"));
        let r = http_json("POST", &url, Body::Json(body), &[])?;
        let get = |k: &str| r.get(k).and_then(|v| v.as_str()).map(|s| s.to_string());
        match (get("localId"), get("refreshToken"), get("idToken")) {
            (Some(uid), Some(rt), Some(it)) => {
                self.set_tokens(uid, Some(username.to_string()), rt, it, r.get("expiresIn"));
                Ok(())
            }
            _ => Err(OnlineError::new("server", "unexpected login response")),
        }
    }

    pub fn sign_up(&self, username: &str, password: &str) -> Res<()> {
        self.auth_call("signUp", &username_to_email(username), password, username)
    }

    pub fn resolve_login_email(&self, username: &str) -> Res<String> {
        let doc = match self.fs("GET", &format!("/usernames/{}", username), None, &[]) {
            Ok(d) => d,
            Err(e) if e.code == "not_found" => return Ok(username_to_email(username)),
            Err(e) => return Err(e),
        };
        Ok(fs_fields(&doc).opt_str("email").unwrap_or_else(|| username_to_email(username)))
    }

    pub fn sign_in(&self, username: &str, password: &str) -> Res<()> {
        let email = self.resolve_login_email(username)?;
        self.auth_call("signInWithPassword", &email, password, username)
    }

    pub fn find_username_by_email(&self, email: &str) -> Res<Option<String>> {
        let body = json!({"structuredQuery": {
            "from": [{"collectionId": "usernames"}],
            "where": {"fieldFilter": {"field": {"fieldPath": "email"}, "op": "EQUAL", "value": {"stringValue": email}}},
            "limit": 1,
        }});
        let url = format!("{}/{}:runQuery?key={}", self.firestore_base, self.docs_root, quote(&self.api_key, "/"));
        let results = http_json("POST", &url, Body::Json(body), &[])?;
        if let Value::Array(items) = results {
            for item in items {
                if let Some(doc) = item.get("document") {
                    if doc.get("name").is_some_and(|n| n.as_str().is_some_and(|s| !s.is_empty())) {
                        return Ok(Some(doc_id(doc)));
                    }
                }
            }
        }
        Ok(None)
    }

    pub fn sign_in_by_email(&self, email: &str, password: &str) -> Res<String> {
        let Some(username) = self.find_username_by_email(email)? else {
            return Err(OnlineError::new("bad_request", "EMAIL_NOT_FOUND"));
        };
        self.sign_in(&username, password)?;
        Ok(username)
    }

    pub fn reauth_pending_link(&self, username: &str, password: &str) -> Res<()> {
        self.auth_call("signInWithPassword", &username_to_email(username), password, username)
    }

    pub fn get_profile(&self) -> Res<Option<AccountProfile>> {
        let uid = self.need_uid()?;
        let doc = match self.fs("GET", &format!("/users/{}/account/profile", uid), None, &[]) {
            Ok(d) => d,
            Err(e) if e.code == "not_found" => return Ok(None),
            Err(e) => return Err(e),
        };
        let f = fs_fields(&doc);
        Ok(Some(AccountProfile {
            email: f.get("email").map(|v| v.py_str()),
            email_verified: f.truthy("email_verified"),
            pending_code: f.opt_str("pending_code"),
            code_created: parse_timestamp(f.get("code_created")),
        }))
    }

    pub fn set_pending_code(&self, email: &str, code: &str) -> Res<()> {
        let uid = self.need_uid()?;
        let name = format!("{}/users/{}/account/profile", self.docs_root, uid);
        let body = json!({"writes": [{
            "update": {"name": name, "fields": {"email": fs_str(email), "email_verified": fs_bool(false), "pending_code": fs_str(code)}},
            "updateTransforms": [{"fieldPath": "code_created", "setToServerValue": "REQUEST_TIME"}],
        }]});
        self.fs("POST", ":commit", Some(body), &[]).map(|_| ())
    }

    pub fn confirm_email(&self) -> Res<()> {
        let uid = self.need_uid()?;
        let name = format!("{}/users/{}/account/profile", self.docs_root, uid);
        let body = json!({"writes": [{
            "update": {"name": name, "fields": {"email_verified": fs_bool(true), "pending_code": fs_str("")}},
            "updateMask": {"fieldPaths": ["email_verified", "pending_code"]},
        }]});
        self.fs("POST", ":commit", Some(body), &[]).map(|_| ())
    }

    pub fn publish_username_email(&self, username: &str, email: &str) -> Res<()> {
        let uid = self.need_uid()?;
        let name = format!("{}/usernames/{}", self.docs_root, username);
        let body = json!({"writes": [{"update": {"name": name, "fields": {"uid": fs_str(&uid), "email": fs_str(email)}}}]});
        self.fs("POST", ":commit", Some(body), &[]).map(|_| ())
    }

    pub fn change_login_email(&self, new_email: &str) -> Res<()> {
        let uid = self.need_uid()?;
        let url = format!("{}/accounts:update?key={}", self.identity_base, quote(&self.api_key, "/"));
        let body = json!({"idToken": self.ensure_token(false)?, "email": new_email, "returnSecureToken": true});
        let r = http_json("POST", &url, Body::Json(body), &[])?;
        let rt = r.get("refreshToken").and_then(|v| v.as_str()).map(|s| s.to_string()).or_else(|| self.refresh_token());
        match (r.get("idToken").and_then(|v| v.as_str()), rt) {
            (Some(it), Some(rt)) => {
                let username = self.username();
                self.set_tokens(uid, username, rt, it.to_string(), r.get("expiresIn"));
                Ok(())
            }
            _ => Err(OnlineError::new("server", "unexpected update response")),
        }
    }

    pub fn send_password_reset(&self, email: &str) -> Res<()> {
        let url = format!("{}/accounts:sendOobCode?key={}", self.identity_base, quote(&self.api_key, "/"));
        http_json("POST", &url, Body::Json(json!({"requestType": "PASSWORD_RESET", "email": email})), &[]).map(|_| ())
    }

    pub fn restore(&self, uid: &str, username: &str, refresh_token: &str) {
        let mut t = self.t();
        t.uid = Some(uid.into());
        t.username = Some(username.into());
        t.refresh_token = Some(refresh_token.into());
        t.id_token = None;
        t.expires_at = 0.0;
    }

    pub fn sign_out(&self) {
        *self.t() = Tokens::default();
    }

    pub fn ensure_token(&self, force: bool) -> Res<String> {
        let mut t = self.t();
        let Some(rt) = t.refresh_token.clone().filter(|s| !s.is_empty()) else {
            return Err(OnlineError::new("auth", "not signed in"));
        };
        if !force {
            if let Some(it) = t.id_token.clone() {
                if now() < t.expires_at - 60.0 {
                    return Ok(it);
                }
            }
        }
        let url = format!("{}/token?key={}", self.token_base, quote(&self.api_key, "/"));
        let r = match http_json("POST", &url, Body::Form(vec![("grant_type".into(), "refresh_token".into()), ("refresh_token".into(), rt)]), &[]) {
            Ok(r) => r,
            Err(e) if ["bad_request", "auth", "denied"].contains(&e.code.as_str()) => {
                return Err(OnlineError::with_status("auth", e.message, e.status));
            }
            Err(e) => return Err(e),
        };
        let Some(it) = r.get("id_token").and_then(|v| v.as_str()) else {
            return Err(OnlineError::new("server", "unexpected token response"));
        };
        t.id_token = Some(it.to_string());
        if let Some(x) = r.get("refresh_token").and_then(|v| v.as_str()) {
            t.refresh_token = Some(x.into());
        }
        if let Some(x) = r.get("user_id").and_then(|v| v.as_str()) {
            t.uid = Some(x.into());
        }
        t.expires_at = match r.get("expires_in") {
            None => now() + 3600.0,
            v => expires(v, "3600"),
        };
        Ok(it.to_string())
    }

    fn fs(&self, method: &str, suffix: &str, body: Option<Value>, params: &[(String, String)]) -> Res<Value> {
        let mut query: Vec<(String, String)> = params.to_vec();
        query.push(("key".into(), self.api_key.clone()));
        let mut headers: Vec<(&str, String)> = Vec::new();
        if self.signed_in() {
            headers.push(("Authorization", format!("Bearer {}", self.ensure_token(false)?)));
        }
        let url = format!("{}/{}{}?{}", self.firestore_base, self.docs_root, suffix, urlencode(&query));
        http_json(method, &url, body.map(Body::Json).unwrap_or(Body::None), &headers)
    }

    fn need_uid(&self) -> Res<String> {
        if !self.signed_in() {
            return Err(OnlineError::new("auth", "not signed in"));
        }
        Ok(self.uid().unwrap_or_default())
    }

    pub fn put_save(&self, slot: i64, state: &Value, base_time: Option<&str>) -> Res<Option<String>> {
        let uid = self.need_uid()?;
        let s = save_summary(state);
        let mut fields: Vec<(&str, Value)> = vec![
            ("coins", fs_f64(s.coins)),
            ("total_earned", fs_f64(s.total_earned)),
            ("playtime", fs_f64(s.playtime)),
            ("rolls", fs_int(s.rolls)),
            ("rebirths", fs_int(s.rebirths)),
            ("data", fs_str(&crate::pyjson::dumps(state))),
        ];
        fields.sort_by(|a, b| a.0.cmp(b.0));
        let mut params: Vec<(String, String)> = fields.iter().map(|(k, _)| ("updateMask.fieldPaths".to_string(), k.to_string())).collect();
        match base_time {
            Some(b) if !b.is_empty() => params.push(("currentDocument.updateTime".into(), b.into())),
            _ => params.push(("currentDocument.exists".into(), "false".into())),
        }
        let fmap: Map<String, Value> = fields.into_iter().map(|(k, v)| (k.to_string(), v)).collect();
        let doc = self.fs("PATCH", &format!("/users/{}/saves/slot{}", uid, slot), Some(json!({"fields": fmap})), &params)?;
        Ok(doc.get("updateTime").and_then(|v| v.as_str()).map(|s| s.to_string()))
    }

    pub fn get_save(&self, slot: i64) -> Res<Option<CloudDoc>> {
        let uid = self.need_uid()?;
        let doc = match self.fs("GET", &format!("/users/{}/saves/slot{}", uid, slot), None, &[]) {
            Ok(d) => d,
            Err(e) if e.code == "not_found" => return Ok(None),
            Err(e) => return Err(e),
        };
        let f = fs_fields(&doc);
        let data = match f.get("data") {
            Some(FsVal::Str(s)) => serde_json::from_str::<Value>(s).ok().filter(|v| v.is_object()),
            _ => None,
        };
        let Some(data) = data else {
            return Err(OnlineError::new("server", "the cloud save is unreadable"));
        };
        Ok(Some(CloudDoc { data, time: doc.get("updateTime").and_then(|v| v.as_str()).map(|s| s.to_string()) }))
    }

    pub fn list_saves(&self) -> Res<HashMap<i64, SaveSummary>> {
        let uid = self.need_uid()?;
        let mut params: Vec<(String, String)> = SUMMARY_FIELDS.iter().map(|k| ("mask.fieldPaths".to_string(), k.to_string())).collect();
        params.push(("pageSize".into(), "10".into()));
        let res = self.fs("GET", &format!("/users/{}/saves", uid), None, &params)?;
        let mut out = HashMap::new();
        if let Some(Value::Array(docs)) = res.get("documents") {
            for doc in docs {
                let name = doc_id(doc);
                let Some(num) = name.strip_prefix("slot") else { continue };
                if num.is_empty() || !num.chars().all(|c| c.is_ascii_digit()) {
                    continue;
                }
                let f = fs_fields(doc);
                out.insert(
                    num.parse::<i64>().unwrap_or(0),
                    SaveSummary {
                        coins: f.f64_or0("coins"),
                        total_earned: f.f64_or0("total_earned"),
                        playtime: f.f64_or0("playtime"),
                        rolls: f.i64_or0("rolls"),
                        rebirths: f.i64_or0("rebirths"),
                        time: doc.get("updateTime").and_then(|v| v.as_str()).map(|s| s.to_string()),
                    },
                );
            }
        }
        Ok(out)
    }

    pub fn delete_save(&self, slot: i64) -> Res<()> {
        let uid = self.need_uid()?;
        match self.fs("DELETE", &format!("/users/{}/saves/slot{}", uid, slot), None, &[]) {
            Err(e) if e.code != "not_found" => Err(e),
            _ => Ok(()),
        }
    }

    pub fn read_session(&self) -> Res<Option<SessionLock>> {
        let uid = self.need_uid()?;
        let doc = match self.fs("GET", &format!("/users/{}/session/current", uid), None, &[]) {
            Ok(d) => d,
            Err(e) if e.code == "not_found" => return Ok(None),
            Err(e) => return Err(e),
        };
        let f = fs_fields(&doc);
        Ok(Some(SessionLock { session_id: f.str_or("session_id", ""), active: f.truthy("active"), heartbeat: parse_timestamp(f.get("heartbeat")) }))
    }

    pub fn session_blocker(&self, session_id: &str) -> Res<Option<SessionLock>> {
        let Some(lock) = self.read_session()? else { return Ok(None) };
        if lock.session_id == session_id || !lock.active {
            return Ok(None);
        }
        let Some(hb) = lock.heartbeat else { return Ok(None) };
        if !server_clock_synced() || server_now() - hb > SESSION_STALE {
            return Ok(None);
        }
        Ok(Some(lock))
    }

    pub fn write_session(&self, session_id: &str, active: bool) -> Res<()> {
        let uid = self.need_uid()?;
        let name = format!("{}/users/{}/session/current", self.docs_root, uid);
        let body = json!({"writes": [{
            "update": {"name": name, "fields": {"session_id": fs_str(session_id), "active": fs_bool(active)}},
            "updateTransforms": [{"fieldPath": "heartbeat", "setToServerValue": "REQUEST_TIME"}],
        }]});
        self.fs("POST", ":commit", Some(body), &[]).map(|_| ())
    }

    /// `prestige`: sent only when > 0 (v3.0; needs the "prestige" line in the /leaderboard rules).
    pub fn publish_score(&self, coins: f64, playtime: f64, rolls: i64, rebirths: i64, prestige: Option<i64>) -> Res<()> {
        let uid = self.need_uid()?;
        let name = format!("{}/leaderboard/{}", self.docs_root, uid);
        let rolls = (rolls as f64).min(9e18).max(0.0) as i64;
        let rebirths = (rebirths as f64).min(1e9).max(0.0) as i64;
        let mut fields = json!({
            "username": fs_str(&self.username().unwrap_or_else(|| "None".into())),
            "coins": fs_f64(coins),
            "playtime": fs_f64(playtime),
            "rolls": fs_int(rolls),
            "rebirths": fs_int(rebirths),
        });
        if let Some(p) = prestige.filter(|p| *p > 0) {
            fields["prestige"] = fs_int(p.min(5));
        }
        let body = json!({"writes": [{
            "update": {"name": name, "fields": fields},
            "updateTransforms": [{"fieldPath": "updated_at", "setToServerValue": "REQUEST_TIME"}],
        }]});
        self.fs("POST", ":commit", Some(body), &[]).map(|_| ())
    }

    pub fn publish_profile(&self, avatar_pet: Option<i64>, avatar_mut: &str) -> Res<()> {
        let uid = self.need_uid()?;
        let name = format!("{}/profiles/{}", self.docs_root, uid);
        let m = if avatar_mut.is_empty() { "normal" } else { avatar_mut };
        let body = json!({"writes": [{
            "update": {"name": name, "fields": {
                "username": fs_str(&self.username().unwrap_or_default()),
                "avatar_pet": fs_int(avatar_pet.unwrap_or(-1)),
                "avatar_mut": fs_str(m),
            }},
            // last_seen = "I'm playing now" (friends see you online); the time is the server's
            "updateTransforms": [
                {"fieldPath": "updated_at", "setToServerValue": "REQUEST_TIME"},
                {"fieldPath": "last_seen", "setToServerValue": "REQUEST_TIME"},
            ],
        }]});
        self.fs("POST", ":commit", Some(body), &[]).map(|_| ())
    }

    fn run_query(&self, suffix: &str, query: Value) -> Res<Vec<Value>> {
        let res = self.fs("POST", suffix, Some(json!({"structuredQuery": query})), &[])?;
        let mut docs = Vec::new();
        if let Value::Array(items) = res {
            for item in items {
                if let Some(doc) = item.get("document") {
                    if doc.is_object() && !doc.as_object().unwrap().is_empty() {
                        docs.push(doc.clone());
                    }
                }
            }
        }
        Ok(docs)
    }

    pub fn find_profile(&self, username: &str) -> Res<Option<Person>> {
        let q = json!({
            "from": [{"collectionId": "profiles"}],
            "where": {"fieldFilter": {"field": {"fieldPath": "username"}, "op": "EQUAL", "value": fs_str(username)}},
            "limit": 1,
        });
        if let Some(doc) = self.run_query(":runQuery", q)?.into_iter().next() {
            return Ok(Some(profile_from_doc(&doc)));
        }
        Ok(self.find_profile_fallback(username))
    }

    fn find_profile_fallback(&self, username: &str) -> Option<Person> {
        let q = json!({
            "from": [{"collectionId": "leaderboard"}],
            "where": {"fieldFilter": {"field": {"fieldPath": "username"}, "op": "EQUAL", "value": fs_str(username)}},
            "limit": 1,
        });
        let mut uid = match self.run_query(":runQuery", q) {
            Ok(docs) => docs.first().map(doc_id).unwrap_or_default(),
            Err(_) => String::new(),
        };
        if uid.is_empty() {
            let doc = self.fs("GET", &format!("/usernames/{}", username), None, &[]).ok()?;
            uid = fs_fields(&doc).str_or("uid", "");
        }
        if uid.is_empty() {
            return None;
        }
        Some(Person { uid, username: username.into(), avatar_pet: None, avatar_mut: "normal".into(), time: None, last_seen: None, title: None })
    }

    pub fn get_public_profile(&self, uid: &str) -> Res<Option<Person>> {
        match self.fs("GET", &format!("/profiles/{}", uid), None, &[]) {
            Ok(d) => Ok(Some(profile_from_doc(&d))),
            Err(e) if e.code == "not_found" => Ok(None),
            Err(e) => Err(e),
        }
    }

    pub fn get_public_stats(&self, uid: &str) -> Res<Option<PublicStats>> {
        let doc = match self.fs("GET", &format!("/leaderboard/{}", uid), None, &[]) {
            Ok(d) => d,
            Err(e) if e.code == "not_found" => return Ok(None),
            Err(e) => return Err(e),
        };
        let f = fs_fields(&doc);
        Ok(Some(PublicStats {
            username: f.str_or("username", ""),
            coins: f.f64_or0("coins"),
            playtime: f.f64_or0("playtime"),
            rolls: f.i64_or0("rolls"),
            rebirths: f.i64_or0("rebirths"),
            updated_at: parse_timestamp(f.get("updated_at")),
        }))
    }

    fn request_path(to_uid: &str, from_uid: &str) -> String {
        format!("/friend_requests/{}__{}", to_uid, from_uid)
    }

    pub fn send_friend_request(&self, to_uid: &str, to_username: &str) -> Res<()> {
        let uid = self.need_uid()?;
        if to_uid == uid {
            return Err(OnlineError::new("bad_request", "that is your own account"));
        }
        let name = format!("{}{}", self.docs_root, Self::request_path(to_uid, &uid));
        let body = json!({"writes": [{
            "update": {"name": name, "fields": {
                "from_uid": fs_str(&uid), "from_username": fs_str(&self.username().unwrap_or_default()),
                "to_uid": fs_str(to_uid), "to_username": fs_str(to_username),
            }},
            "updateTransforms": [{"fieldPath": "created_at", "setToServerValue": "REQUEST_TIME"}],
        }]});
        self.fs("POST", ":commit", Some(body), &[]).map(|_| ())
    }

    pub fn list_friend_requests(&self, incoming: bool) -> Res<Vec<Person>> {
        let uid = self.need_uid()?;
        let field = if incoming { "to_uid" } else { "from_uid" };
        let q = json!({
            "from": [{"collectionId": "friend_requests"}],
            "where": {"fieldFilter": {"field": {"fieldPath": field}, "op": "EQUAL", "value": fs_str(&uid)}},
            "limit": 60,
        });
        let mut out = Vec::new();
        for doc in self.run_query(":runQuery", q)? {
            let f = fs_fields(&doc);
            let (ou, on) = if incoming { ("from_uid", "from_username") } else { ("to_uid", "to_username") };
            let Some(other_uid) = f.opt_str(ou) else { continue };
            out.push(Person {
                uid: other_uid,
                username: f.str_or(on, "?"),
                avatar_pet: None,
                avatar_mut: "normal".into(),
                time: parse_timestamp(f.get("created_at")),
                last_seen: None,
                title: None,
            });
        }
        Ok(out)
    }

    pub fn delete_friend_request(&self, to_uid: &str, from_uid: &str) -> Res<()> {
        match self.fs("DELETE", &Self::request_path(to_uid, from_uid), None, &[]) {
            Err(e) if e.code != "not_found" => Err(e),
            _ => Ok(()),
        }
    }

    pub fn accept_friend_request(&self, from_uid: &str, from_username: &str) -> Res<()> {
        let uid = self.need_uid()?;
        let mine = format!("{}/users/{}/friends/{}", self.docs_root, uid, from_uid);
        let theirs = format!("{}/users/{}/friends/{}", self.docs_root, from_uid, uid);
        let fu = if from_username.is_empty() { "?" } else { from_username };
        let friends = vec![
            json!({"update": {"name": mine, "fields": {"username": fs_str(fu)}},
             "updateTransforms": [{"fieldPath": "since", "setToServerValue": "REQUEST_TIME"}]}),
            json!({"update": {"name": theirs, "fields": {"username": fs_str(&self.username().unwrap_or_default())}},
             "updateTransforms": [{"fieldPath": "since", "setToServerValue": "REQUEST_TIME"}]}),
        ];
        let request = json!({"delete": format!("{}{}", self.docs_root, Self::request_path(&uid, from_uid))});
        let mut all = friends.clone();
        all.push(request.clone());
        match self.fs("POST", ":commit", Some(json!({"writes": all})), &[]) {
            Ok(_) => Ok(()),
            Err(e) if e.code != "denied" => Err(e),
            Err(_) => {
                // The request no longer exists on the server (cancelled and sent again, or the list was stale):
                // deleting a missing document is refused by the rules and took the rest with it. Make the
                // friend lists alone, then try deleting the request separately (no harm if that fails).
                self.fs("POST", ":commit", Some(json!({"writes": friends})), &[])?;
                let _ = self.fs("POST", ":commit", Some(json!({"writes": [request]})), &[]);
                Ok(())
            }
        }
    }

    pub fn list_friends(&self) -> Res<Vec<Person>> {
        let uid = self.need_uid()?;
        let res = self.fs("GET", &format!("/users/{}/friends", uid), None, &[("pageSize".into(), "200".into())])?;
        let mut out = Vec::new();
        if let Some(Value::Array(docs)) = res.get("documents") {
            for doc in docs {
                let fuid = doc_id(doc);
                if fuid.is_empty() {
                    continue;
                }
                let f = fs_fields(doc);
                out.push(Person {
                    uid: fuid,
                    username: f.str_or("username", "?"),
                    avatar_pet: None,
                    avatar_mut: "normal".into(),
                    time: parse_timestamp(f.get("since")),
                    last_seen: None,
                    title: None,
                });
            }
        }
        out.sort_by(|a, b| a.username.cmp(&b.username));
        Ok(out)
    }

    pub fn remove_friend(&self, friend_uid: &str) -> Res<()> {
        let uid = self.need_uid()?;
        let body = json!({"writes": [
            {"delete": format!("{}/users/{}/friends/{}", self.docs_root, uid, friend_uid)},
            {"delete": format!("{}/users/{}/friends/{}", self.docs_root, friend_uid, uid)},
        ]});
        self.fs("POST", ":commit", Some(body), &[]).map(|_| ())
    }

    pub fn chat_id(&self, other_uid: &str) -> Res<String> {
        let uid = self.need_uid()?;
        let mut v = [uid, other_uid.to_string()];
        v.sort();
        Ok(format!("{}__{}", v[0], v[1]))
    }

    pub fn send_message(&self, other_uid: &str, text: &str) -> Res<()> {
        let cid = self.chat_id(other_uid)?;
        let uid = self.need_uid()?;
        let name = format!("{}/chats/{}/messages", self.docs_root, cid);
        let body = json!({"writes": [
            {"update": {"name": format!("{}/{}", name, new_doc_id()), "fields": {
                "from_uid": fs_str(&uid), "to_uid": fs_str(other_uid), "text": fs_str(text),
            }},
             "updateTransforms": [{"fieldPath": "sent_at", "setToServerValue": "REQUEST_TIME"}]},
            {"update": {"name": format!("{}/chats/{}", self.docs_root, cid), "fields": {"last_from": fs_str(&uid)}},
             "updateTransforms": [{"fieldPath": "last_at", "setToServerValue": "REQUEST_TIME"}]},
        ]});
        self.fs("POST", ":commit", Some(body), &[]).map(|_| ())
    }

    /// last_at of the conversation, or None if they never talked.
    pub fn get_chat_summary(&self, other_uid: &str) -> Res<Option<Option<f64>>> {
        let cid = self.chat_id(other_uid)?;
        match self.fs("GET", &format!("/chats/{}", cid), None, &[]) {
            Ok(d) => Ok(Some(parse_timestamp(fs_fields(&d).get("last_at")))),
            Err(e) if e.code == "not_found" => Ok(None),
            Err(e) => Err(e),
        }
    }

    pub fn list_messages(&self, other_uid: &str, limit: usize) -> Res<Vec<Message>> {
        let cid = self.chat_id(other_uid)?;
        let q = json!({
            "from": [{"collectionId": "messages"}],
            "orderBy": [{"field": {"fieldPath": "sent_at"}, "direction": "DESCENDING"}],
            "limit": limit,
        });
        let mut out: Vec<Message> = self
            .run_query(&format!("/chats/{}:runQuery", cid), q)?
            .iter()
            .map(|doc| {
                let f = fs_fields(doc);
                Message { id: doc_id(doc), from_uid: f.str_or("from_uid", ""), text: f.str_or("text", ""), sent_at: parse_timestamp(f.get("sent_at")) }
            })
            .collect();
        out.reverse();
        Ok(out)
    }

    pub fn is_admin(&self) -> Res<bool> {
        let uid = self.need_uid()?;
        match self.fs("GET", &format!("/admins/{}", uid), None, &[]) {
            Ok(_) => Ok(true),
            Err(e) if e.code == "not_found" => Ok(false),
            Err(e) => Err(e),
        }
    }

    /// Every global event running now: {"luck": ..., "money": ..., ...} - only the kinds that really have a
    /// document (0, 1, 2 or all 3 at once, one per kind - see game/events.rs).
    pub fn get_events(&self) -> Res<Vec<Event>> {
        let res = match self.fs("GET", "/events", None, &[]) {
            Ok(r) => r,
            Err(e) if e.code == "not_found" => return Ok(Vec::new()),
            Err(e) => return Err(e),
        };
        let mut out = Vec::new();
        if let Some(Value::Array(docs)) = res.get("documents") {
            for doc in docs {
                let kind = doc_id(doc);
                if !kind.is_empty() {
                    out.push(event_from_doc(doc, &kind));
                }
            }
        }
        Ok(out)
    }

    /// Starts (or replaces) the global event of THIS kind - it doesn't touch the other two, so all three can run
    /// at once. Only goes through if the rules let this uid write.
    pub fn start_event(&self, kind: &str, mult: f64, seconds: f64) -> Res<Event> {
        let uid = self.need_uid()?;
        // the SERVER's time, not the PC's: the rules compare ends_at with request.time, and a PC clock a few
        // seconds off made some durations go through and others be refused
        let server_ends = server_now() + seconds;
        let by = self.username().filter(|s| !s.is_empty()).unwrap_or(uid);
        let mut fields = Map::new();
        fields.insert("by".into(), fs_str(&by));
        fields.insert("ends_at".into(), json!({"timestampValue": iso_timestamp(server_ends)}));
        fields.insert("mult".into(), fs_f64(mult));
        let params: Vec<(String, String)> = fields.keys().map(|k| ("updateMask.fieldPaths".to_string(), k.clone())).collect();
        self.fs("PATCH", &format!("/events/{}", kind), Some(json!({"fields": fields})), &params)?;
        Ok(Event { kind: kind.into(), mult, ends_at: server_ends, by })
    }

    pub fn stop_event(&self, kind: &str) -> Res<()> {
        match self.fs("DELETE", &format!("/events/{}", kind), None, &[]) {
            Err(e) if e.code != "not_found" => Err(e),
            _ => Ok(()),
        }
    }

    // ---- bans (admins only; see /bans/{uid} in the rules) ----
    /// This account's ban, or None if it isn't banned.
    pub fn get_my_ban(&self) -> Res<Option<Ban>> {
        let uid = self.need_uid()?;
        match self.fs("GET", &format!("/bans/{}", uid), None, &[]) {
            Ok(d) => Ok(Some(ban_from_doc(&d))),
            Err(e) if e.code == "not_found" => Ok(None),
            Err(e) => Err(e),
        }
    }

    pub fn list_bans(&self, limit: usize) -> Res<Vec<Ban>> {
        let res = self.fs("GET", "/bans", None, &[("pageSize".into(), limit.to_string())])?;
        let mut out: Vec<Ban> = match res.get("documents") {
            Some(Value::Array(docs)) => docs.iter().map(ban_from_doc).collect(),
            _ => Vec::new(),
        };
        out.sort_by(|a, b| b.at.unwrap_or(0.0).partial_cmp(&a.at.unwrap_or(0.0)).unwrap_or(std::cmp::Ordering::Equal));
        Ok(out)
    }

    /// Bans the account 'uid' (with the reason) and takes it off the leaderboard.
    pub fn ban_player(&self, uid: &str, username: &str, reason: &str) -> Res<()> {
        let reason: String = reason.chars().take(300).collect();
        let body = json!({"writes": [{
            "update": {"name": format!("{}/bans/{}", self.docs_root, uid), "fields": {
                "username": fs_str(username),
                "reason": fs_str(&reason),
                "by": fs_str(&self.username().unwrap_or_default()),
            }},
            "updateTransforms": [{"fieldPath": "at", "setToServerValue": "REQUEST_TIME"}],
        }]});
        self.fs("POST", ":commit", Some(body), &[])?;
        // old rules (no "delete: if isAdmin()") - the ban counts anyway
        let _ = self.fs("DELETE", &format!("/leaderboard/{}", uid), None, &[]);
        Ok(())
    }

    pub fn unban_player(&self, uid: &str) -> Res<()> {
        match self.fs("DELETE", &format!("/bans/{}", uid), None, &[]) {
            Err(e) if e.code != "not_found" => Err(e),
            _ => Ok(()),
        }
    }

    // ---- pet trades (between friends) ----
    // The "wallet" (/users/{uid}/wallet/pets) is the Firestore copy of how many pets of each kind you have (same
    // format as the save's "owned"). It is the wallet, not the local save, that trades use to check you really
    // have what you offer - editing the save by hand doesn't change it. It syncs by itself whenever you roll
    // (see tick_wallet).
    //
    // A trade (/trades/{id}) only exists while pending: accepting creates a "receipt" in
    // /users/{other}/incoming/{id} with what they get/lose and deletes the trade; declining or cancelling just
    // deletes it, without touching any wallet.
    fn wallet_path(&self, uid: &str) -> String {
        format!("/users/{}/wallet/pets", uid)
    }

    /// {"3_normal": 2, ...}. An empty/new account = {} (never an error for not existing yet).
    pub fn get_wallet(&self, uid: Option<&str>) -> Res<HashMap<String, i64>> {
        let uid = match uid {
            Some(u) if !u.is_empty() => u.to_string(),
            _ => self.need_uid()?,
        };
        match self.fs("GET", &self.wallet_path(&uid), None, &[]) {
            Ok(doc) => Ok(int_map(&doc, "owned")),
            Err(e) if e.code == "not_found" => Ok(HashMap::new()),
            Err(e) => Err(e),
        }
    }

    /// Adds (or subtracts, with negative values) 'delta' to the wallet - whenever you roll new pets. Uses
    /// atomic increments: never loses an addition when two arrive at once.
    pub fn sync_wallet(&self, delta: &[(String, i64)]) -> Res<()> {
        if delta.is_empty() {
            return Ok(());
        }
        let uid = self.need_uid()?;
        let body = json!({"writes": [{
            "update": {"name": format!("{}{}", self.docs_root, self.wallet_path(&uid)), "fields": {}},
            "updateMask": {"fieldPaths": []},
            "updateTransforms": increments(delta),
        }]});
        self.fs("POST", ":commit", Some(body), &[]).map(|_| ())
    }

    /// Replaces the online wallet with the save's CURRENT totals (sets, doesn't add). Covers pets you had before
    /// the online wallet existed. Called before proposing a trade, so the wallet is never behind the save.
    pub fn set_wallet_full(&self, owned: &[(String, i64)]) -> Res<()> {
        let uid = self.need_uid()?;
        let mut m = Map::new();
        for (k, q) in owned {
            if *q > 0 {
                m.insert(k.clone(), fs_int(*q));
            }
        }
        let fields = json!({"owned": {"mapValue": {"fields": m}}});
        self.fs("PATCH", &self.wallet_path(&uid), Some(json!({"fields": fields})), &[("updateMask.fieldPaths".into(), "owned".into())]).map(|_| ())
    }

    /// offer / request: up to 3 (pet_key, qty) pairs. Whether you really have the pets is only enforced on the
    /// accepting side (see accept_trade).
    pub fn create_trade(&self, to_uid: &str, offer: &[(String, i64)], request: &[(String, i64)]) -> Res<String> {
        let uid = self.need_uid()?;
        let trade_id = new_doc_id();
        let mut fields = Map::new();
        fields.insert("from_uid".into(), fs_str(&uid));
        fields.insert("to_uid".into(), fs_str(to_uid));
        trade_item_fields(&mut fields, "offer", offer);
        trade_item_fields(&mut fields, "request", request);
        let mut keys: Vec<String> = fields.keys().cloned().collect();
        keys.sort();
        let mut params: Vec<(String, String)> = keys.into_iter().map(|k| ("updateMask.fieldPaths".to_string(), k)).collect();
        params.push(("currentDocument.exists".into(), "false".into()));
        self.fs("PATCH", &format!("/trades/{}", trade_id), Some(json!({"fields": fields})), &params)?;
        Ok(trade_id)
    }

    fn list_trades(&self, field: &str, uid: &str) -> Res<Vec<Trade>> {
        let q = json!({
            "from": [{"collectionId": "trades"}],
            "where": {"fieldFilter": {"field": {"fieldPath": field}, "op": "EQUAL", "value": {"stringValue": uid}}},
            "limit": 100,
        });
        Ok(self
            .run_query(":runQuery", q)?
            .iter()
            .filter(|doc| doc.get("name").is_some())
            .map(|doc| {
                let f = fs_fields(doc);
                Trade {
                    id: doc_id(doc),
                    from_uid: f.str_or("from_uid", ""),
                    to_uid: f.str_or("to_uid", ""),
                    offer: trade_items_from_fields(&f, "offer"),
                    request: trade_items_from_fields(&f, "request"),
                }
            })
            .collect())
    }

    pub fn trades_sent(&self) -> Res<Vec<Trade>> {
        let uid = self.need_uid()?;
        self.list_trades("from_uid", &uid)
    }

    pub fn trades_received(&self) -> Res<Vec<Trade>> {
        let uid = self.need_uid()?;
        self.list_trades("to_uid", &uid)
    }

    // ---- online battles (between friends) ----
    // /battles/{id}: the challenger creates it (their team + a seed), the friend accepts (their team, "live"), then
    // each side appends its moves to its own string. Each game polls the document while waiting for the other's
    // move. Reads: one per poll; writes: one per move.
    fn patch_fields(&self, path: &str, fields: Map<String, Value>, must_exist: bool) -> Res<()> {
        let mut keys: Vec<String> = fields.keys().cloned().collect();
        keys.sort();
        let mut params: Vec<(String, String)> = keys.into_iter().map(|k| ("updateMask.fieldPaths".to_string(), k)).collect();
        params.push(("currentDocument.exists".into(), if must_exist { "true" } else { "false" }.into()));
        self.fs("PATCH", path, Some(json!({"fields": fields})), &params).map(|_| ())
    }

    /// Challenges a friend with my team. Returns the battle's id.
    pub fn create_battle(&self, to_uid: &str, from_name: &str, to_name: &str, team: &str, seed: i64) -> Res<String> {
        let uid = self.need_uid()?;
        let id = new_doc_id();
        let mut f = Map::new();
        f.insert("from_uid".into(), fs_str(&uid));
        f.insert("to_uid".into(), fs_str(to_uid));
        f.insert("from_name".into(), fs_str(from_name));
        f.insert("to_name".into(), fs_str(to_name));
        f.insert("from_team".into(), fs_str(team));
        f.insert("to_team".into(), fs_str(""));
        f.insert("seed".into(), fs_int(seed));
        f.insert("status".into(), fs_str("pending"));
        f.insert("from_moves".into(), fs_str(""));
        f.insert("to_moves".into(), fs_str(""));
        f.insert("created".into(), fs_f64(server_now()));
        self.patch_fields(&format!("/battles/{}", id), f, false)?;
        Ok(id)
    }

    /// My battles: the ones I sent (field "from_uid") or got (field "to_uid").
    pub fn list_battles(&self, field: &str) -> Res<Vec<BattleDoc>> {
        let uid = self.need_uid()?;
        let q = json!({
            "from": [{"collectionId": "battles"}],
            "where": {"fieldFilter": {"field": {"fieldPath": field}, "op": "EQUAL", "value": {"stringValue": uid}}},
            "limit": 30,
        });
        Ok(self.run_query(":runQuery", q)?.iter().filter(|d| d.get("name").is_some()).map(battle_from_doc).collect())
    }

    /// One battle (None = it's gone: cancelled or declined).
    pub fn get_battle(&self, id: &str) -> Res<Option<BattleDoc>> {
        match self.fs("GET", &format!("/battles/{}", id), None, &[]) {
            Ok(doc) => Ok(Some(battle_from_doc(&doc))),
            Err(e) if e.code == "not_found" => Ok(None),
            Err(e) => Err(e),
        }
    }

    /// The friend accepts with their team: the battle is on.
    pub fn accept_battle(&self, id: &str, team: &str) -> Res<()> {
        let mut f = Map::new();
        f.insert("to_team".into(), fs_str(team));
        f.insert("status".into(), fs_str("live"));
        self.patch_fields(&format!("/battles/{}", id), f, true)
    }

    /// Saves my moves so far (field "from_moves" or "to_moves").
    pub fn push_battle_moves(&self, id: &str, field: &str, moves: &str) -> Res<()> {
        let mut f = Map::new();
        f.insert(field.to_string(), fs_str(moves));
        self.patch_fields(&format!("/battles/{}", id), f, true)
    }

    pub fn finish_battle(&self, id: &str) -> Res<()> {
        let mut f = Map::new();
        f.insert("status".into(), fs_str("over"));
        match self.patch_fields(&format!("/battles/{}", id), f, true) {
            Err(e) if e.code != "not_found" => Err(e),
            _ => Ok(()),
        }
    }

    // ---- ranked queue ----
    // /queue/{uid}: who's looking for a ranked match (their rating and team). Whoever has the smaller uid of a pair
    // creates the battle (so two players never create two battles for each other); the other finds it in its
    // "battles sent to me".
    pub fn queue_join(&self, name: &str, rating: i64, team: &str) -> Res<()> {
        let uid = self.need_uid()?;
        let mut f = Map::new();
        f.insert("uid".into(), fs_str(&uid));
        f.insert("name".into(), fs_str(name));
        f.insert("rating".into(), fs_int(rating));
        f.insert("team".into(), fs_str(team));
        f.insert("at".into(), fs_f64(server_now()));
        let mut keys: Vec<String> = f.keys().cloned().collect();
        keys.sort();
        let params: Vec<(String, String)> = keys.into_iter().map(|k| ("updateMask.fieldPaths".to_string(), k)).collect();
        self.fs("PATCH", &format!("/queue/{}", uid), Some(json!({"fields": f})), &params).map(|_| ())
    }

    pub fn queue_leave(&self) -> Res<()> {
        let uid = self.need_uid()?;
        match self.fs("DELETE", &format!("/queue/{}", uid), None, &[]) {
            Err(e) if e.code != "not_found" => Err(e),
            _ => Ok(()),
        }
    }

    /// The players waiting now (the most recent first).
    pub fn queue_list(&self) -> Res<Vec<QueueEntry>> {
        let q = json!({
            "from": [{"collectionId": "queue"}],
            "orderBy": [{"field": {"fieldPath": "at"}, "direction": "DESCENDING"}],
            "limit": 12,
        });
        Ok(self
            .run_query(":runQuery", q)?
            .iter()
            .filter(|d| d.get("name").is_some())
            .map(|d| {
                let f = fs_fields(d);
                QueueEntry { uid: f.str_or("uid", ""), name: f.str_or("name", "?"), rating: f.i64_or0("rating"), team: f.str_or("team", ""), at: f.f64_or0("at") }
            })
            .collect())
    }

    /// I (the smaller uid) start a ranked battle with someone from the queue: it's live right away.
    pub fn create_ranked_battle(&self, me_name: &str, me_rating: i64, my_team: &str, other: &QueueEntry, seed: i64) -> Res<String> {
        let uid = self.need_uid()?;
        let id = new_doc_id();
        let mut f = Map::new();
        f.insert("from_uid".into(), fs_str(&uid));
        f.insert("to_uid".into(), fs_str(&other.uid));
        f.insert("from_name".into(), fs_str(me_name));
        f.insert("to_name".into(), fs_str(&other.name));
        f.insert("from_team".into(), fs_str(my_team));
        f.insert("to_team".into(), fs_str(&other.team));
        f.insert("seed".into(), fs_int(seed));
        f.insert("status".into(), fs_str("live"));
        f.insert("from_moves".into(), fs_str(""));
        f.insert("to_moves".into(), fs_str(""));
        f.insert("created".into(), fs_f64(server_now()));
        f.insert("ranked".into(), fs_bool(true));
        f.insert("from_rating".into(), fs_int(me_rating));
        f.insert("to_rating".into(), fs_int(other.rating));
        self.patch_fields(&format!("/battles/{}", id), f, false)?;
        Ok(id)
    }

    /// My ranked standing, for the top list.
    pub fn push_rank(&self, name: &str, rating: i64, wins: i64, losses: i64) -> Res<()> {
        let uid = self.need_uid()?;
        let mut f = Map::new();
        f.insert("username".into(), fs_str(name));
        f.insert("rating".into(), fs_int(rating));
        f.insert("wins".into(), fs_int(wins));
        f.insert("losses".into(), fs_int(losses));
        let params: Vec<(String, String)> = ["losses", "rating", "username", "wins"].iter().map(|k| ("updateMask.fieldPaths".to_string(), k.to_string())).collect();
        self.fs("PATCH", &format!("/ranks/{}", uid), Some(json!({"fields": f})), &params).map(|_| ())
    }

    pub fn top_ranks(&self) -> Res<Vec<RankEntry>> {
        let q = json!({
            "from": [{"collectionId": "ranks"}],
            "orderBy": [{"field": {"fieldPath": "rating"}, "direction": "DESCENDING"}],
            "limit": 10,
        });
        Ok(self
            .run_query(":runQuery", q)?
            .iter()
            .filter(|d| d.get("name").is_some())
            .map(|d| {
                let f = fs_fields(d);
                RankEntry { name: f.str_or("username", "?"), rating: f.i64_or0("rating"), wins: f.i64_or0("wins"), losses: f.i64_or0("losses") }
            })
            .collect())
    }

    /// Cancel (the challenger), decline (the friend) or clean up a finished one.
    pub fn delete_battle(&self, id: &str) -> Res<()> {
        match self.fs("DELETE", &format!("/battles/{}", id), None, &[]) {
            Err(e) if e.code != "not_found" => Err(e),
            _ => Ok(()),
        }
    }

    /// Cancel (the proposer) or decline (the receiver) - the rules only allow deleting to one of the two.
    pub fn cancel_trade(&self, trade_id: &str) -> Res<()> {
        match self.fs("DELETE", &format!("/trades/{}", trade_id), None, &[]) {
            Err(e) if e.code != "not_found" => Err(e),
            _ => Ok(()),
        }
    }

    /// Accept: MY wallet changes now (I give what was asked, I get what was offered); at the same time a receipt
    /// lands in the proposer's inbox with what THEY still have to apply (only they can touch their wallet), and
    /// the trade disappears (it can never be accepted again).
    pub fn accept_trade(&self, trade_id: &str, from_uid: &str, offer: &[(String, i64)], request: &[(String, i64)]) -> Res<()> {
        let uid = self.need_uid()?;
        let mut my_delta: Vec<(String, i64)> = Vec::new();
        for (k, q) in request {
            add_delta(&mut my_delta, k, -q);
        }
        for (k, q) in offer {
            add_delta(&mut my_delta, k, *q);
        }
        let mut receipt = Map::new();
        trade_item_fields(&mut receipt, "give", offer); // what THEY get
        trade_item_fields(&mut receipt, "take", request); // what THEY lose
        let body = json!({"writes": [
            {"update": {"name": format!("{}{}", self.docs_root, self.wallet_path(&uid)), "fields": {}},
             "updateMask": {"fieldPaths": []},
             "updateTransforms": increments(&my_delta)},
            {"update": {"name": format!("{}/users/{}/incoming/{}", self.docs_root, from_uid, trade_id), "fields": receipt}},
            {"delete": format!("{}/trades/{}", self.docs_root, trade_id)},
        ]});
        self.fs("POST", ":commit", Some(body), &[]).map(|_| ())
    }

    pub fn list_incoming(&self) -> Res<Vec<Receipt>> {
        let uid = self.need_uid()?;
        let res = self.fs("GET", &format!("/users/{}/incoming", uid), None, &[])?;
        let mut out = Vec::new();
        if let Some(Value::Array(docs)) = res.get("documents") {
            for doc in docs {
                let f = fs_fields(doc);
                out.push(Receipt { id: doc_id(doc), give: trade_items_from_fields(&f, "give"), take: trade_items_from_fields(&f, "take") });
            }
        }
        Ok(out)
    }

    /// Applies a receipt (see list_incoming) to my wallet and deletes it - automatic, the player never sees it.
    pub fn apply_incoming(&self, receipt: &Receipt) -> Res<()> {
        let uid = self.need_uid()?;
        let mut delta: Vec<(String, i64)> = Vec::new();
        for (k, q) in &receipt.give {
            add_delta(&mut delta, k, *q);
        }
        for (k, q) in &receipt.take {
            add_delta(&mut delta, k, -q);
        }
        let body = json!({"writes": [
            {"update": {"name": format!("{}{}", self.docs_root, self.wallet_path(&uid)), "fields": {}},
             "updateMask": {"fieldPaths": []},
             "updateTransforms": increments(&delta)},
            {"delete": format!("{}/users/{}/incoming/{}", self.docs_root, uid, receipt.id)},
        ]});
        self.fs("POST", ":commit", Some(body), &[]).map(|_| ())
    }

    // ---- titles (/titles/{uid}: a separate document, so nothing else breaks if its rule isn't published) ----
    /// Someone's equipped title, or None.
    pub fn get_title(&self, uid: &str) -> Res<Option<String>> {
        match self.fs("GET", &format!("/titles/{}", uid), None, &[]) {
            Ok(d) => Ok(fs_fields(&d).opt_str("title")),
            Err(e) if e.code == "not_found" => Ok(None),
            Err(e) => Err(e),
        }
    }

    /// Publishes (or, with None, removes) this account's title.
    pub fn set_title(&self, title: Option<&str>) -> Res<()> {
        let uid = self.need_uid()?;
        let path = format!("/titles/{}", uid);
        match title {
            Some(t) => self.fs("PATCH", &path, Some(json!({"fields": {"title": fs_str(t)}})), &[]).map(|_| ()),
            None => match self.fs("DELETE", &path, None, &[]) {
                Err(e) if e.code != "not_found" => Err(e),
                _ => Ok(()),
            },
        }
    }

    // ---- shared leaderboard snapshot ----
    /// The shared leaderboard snapshot (/public/leaderboard), or None if it doesn't exist yet. 1 read only.
    /// v3.0.1: the season number (/public/season). A save from an older season is wiped (a reset of everyone's
    /// progress, from the admin menu). 0 when there is none, or the rules don't allow reading it yet.
    pub fn get_season(&self) -> Res<i64> {
        match self.fs("GET", "/public/season", None, &[]) {
            Ok(doc) => Ok(fs_fields(&doc).i64_or0("season").max(0)),
            Err(e) if e.code == "not_found" || e.code == "denied" => Ok(0),
            Err(e) => Err(e),
        }
    }

    /// v3.0.1: this account's own reset number (/resets/{uid}). An admin raises it to reset only this player: a
    /// save with a lower number starts again from 0. 0 when there is none (or the rules aren't published).
    pub fn get_my_reset(&self) -> Res<i64> {
        let uid = self.need_uid()?;
        match self.fs("GET", &format!("/resets/{}", uid), None, &[]) {
            Ok(doc) => Ok(fs_fields(&doc).i64_or0("n").max(0)),
            Err(e) if e.code == "not_found" || e.code == "denied" => Ok(0),
            Err(e) => Err(e),
        }
    }

    /// Admins only (rules): resets one player (their number + 1) and takes them off the leaderboard.
    pub fn reset_player(&self, uid: &str) -> Res<i64> {
        let n = match self.fs("GET", &format!("/resets/{}", uid), None, &[]) {
            Ok(doc) => fs_fields(&doc).i64_or0("n").max(0),
            Err(e) if e.code == "not_found" => 0,
            Err(e) => return Err(e),
        } + 1;
        self.fs("PATCH", &format!("/resets/{}", uid), Some(json!({"fields": {"n": fs_int(n)}})), &[])?;
        match self.fs("DELETE", &format!("/leaderboard/{}", uid), None, &[]) {
            Err(e) if e.code != "not_found" => return Err(e),
            _ => {}
        }
        self.log_admin_action("reset_player", &format!("uid={} n={}", uid, n));
        Ok(n)
    }

    /// Admins only (rules): starts season `n`.
    pub fn set_season(&self, n: i64) -> Res<()> {
        self.fs("PATCH", "/public/season", Some(json!({"fields": {"season": fs_int(n)}})), &[])?;
        self.log_admin_action("set_season", &format!("n={}", n));
        Ok(())
    }

    /// Best-effort: who did a destructive admin action (a season/player reset) and when, so it can be told apart
    /// from a bug later. /admin_log/{who}_{when}; never fails the action itself if this can't be written.
    fn log_admin_action(&self, action: &str, detail: &str) {
        let who = self.username().or_else(|| self.uid()).unwrap_or_else(|| "?".into());
        let id = format!("{}_{}", who, now() as i64);
        let fields = json!({"who": fs_str(&who), "action": fs_str(action), "detail": fs_str(detail)});
        let _ = self.fs("PATCH", &format!("/admin_log/{}", id), Some(json!({"fields": fields})), &[]);
    }

    /// Admins only (rules): empties the leaderboard (every /leaderboard/{uid}). Returns how many were removed.
    pub fn clear_leaderboard(&self) -> Res<usize> {
        let mut removed = 0;
        loop {
            let params = vec![("pageSize".to_string(), "300".to_string()), ("mask.fieldPaths".to_string(), "username".to_string())];
            let page = self.fs("GET", "/leaderboard", None, &params)?;
            let docs = page.get("documents").and_then(|d| d.as_array()).cloned().unwrap_or_default();
            if docs.is_empty() {
                return Ok(removed);
            }
            let before = removed;
            for d in docs {
                let Some(name) = d.get("name").and_then(|n| n.as_str()) else { continue };
                let Some(uid) = name.rsplit('/').next() else { continue };
                match self.fs("DELETE", &format!("/leaderboard/{}", uid), None, &[]) {
                    Ok(_) => removed += 1,
                    Err(e) if e.code == "not_found" => {}
                    Err(e) => return Err(e),
                }
            }
            if removed == before {
                return Ok(removed); // nothing more could go: don't loop forever
            }
        }
    }

    pub fn get_leaderboard_snapshot(&self) -> Res<Option<Value>> {
        let doc = match self.fs("GET", "/public/leaderboard", None, &[]) {
            Ok(d) => d,
            Err(e) if e.code == "not_found" => return Ok(None),
            Err(e) => return Err(e),
        };
        let text = fs_fields(&doc).str_or("json", "");
        Ok(serde_json::from_str::<Value>(&text).ok().filter(|v| v.is_object()))
    }

    /// Asks the script (LEADERBOARD_ENDPOINT) for this period's snapshot: if it doesn't exist yet, the script takes
    /// it (once for everyone) and returns it. None if the script isn't configured.
    pub fn request_leaderboard_snapshot() -> Res<Option<Value>> {
        let url = LEADERBOARD_ENDPOINT.trim();
        if url.is_empty() {
            return Ok(None);
        }
        let res = http_json_with(slow_agent(), "GET", url, Body::None, &[])?;
        if res.get("ok").map(|v| v.as_bool() == Some(true)).unwrap_or(false) {
            return Ok(res.get("snapshot").filter(|s| s.is_object()).cloned());
        }
        let err = res.get("error").map(py_str).unwrap_or_else(|| "leaderboard script failed".into());
        Err(OnlineError::new("server", err))
    }

    pub fn publish_feedback(&self, text: &str) -> Res<()> {
        let uid = self.need_uid()?;
        let name = format!("{}/feedback/{}", self.docs_root, uid);
        let body = json!({"writes": [{
            "update": {"name": name, "fields": {"username": fs_str(&self.username().unwrap_or_default()), "text": fs_str(text)}},
            "updateTransforms": [{"fieldPath": "updated_at", "setToServerValue": "REQUEST_TIME"}],
        }]});
        self.fs("POST", ":commit", Some(body), &[]).map(|_| ())
    }

    pub fn get_my_feedback(&self) -> Res<Option<Feedback>> {
        let uid = self.need_uid()?;
        match self.fs("GET", &format!("/feedback/{}", uid), None, &[]) {
            Ok(d) => Ok(Some(feedback_from_doc(&d))),
            Err(e) if e.code == "not_found" => Ok(None),
            Err(e) => Err(e),
        }
    }

    pub fn delete_feedback(&self) -> Res<()> {
        let uid = self.need_uid()?;
        match self.fs("DELETE", &format!("/feedback/{}", uid), None, &[]) {
            Err(e) if e.code != "not_found" => Err(e),
            _ => Ok(()),
        }
    }

    pub fn list_feedback(&self, limit: usize) -> Res<Vec<Feedback>> {
        let q = json!({
            "from": [{"collectionId": "feedback"}],
            "orderBy": [{"field": {"fieldPath": "updated_at"}, "direction": "DESCENDING"}],
            "limit": limit,
        });
        Ok(self.run_query(":runQuery", q)?.iter().map(feedback_from_doc).collect())
    }

    pub fn top_entries(&self, field: &str, limit: usize, min_value: Option<i64>) -> Res<Vec<LbEntry>> {
        let mut q = json!({
            "from": [{"collectionId": "leaderboard"}],
            "orderBy": [{"field": {"fieldPath": field}, "direction": "DESCENDING"}],
            "limit": limit,
        });
        if let Some(m) = min_value {
            q["where"] = json!({"fieldFilter": {"field": {"fieldPath": field}, "op": "GREATER_THAN_OR_EQUAL", "value": fs_int(m)}});
        }
        Ok(self
            .run_query(":runQuery", q)?
            .iter()
            .map(|doc| {
                let f = fs_fields(doc);
                let username = f.get("username").map(|v| v.py_str()).unwrap_or_else(|| "?".into());
                LbEntry { username, value: f.f64_or0(field), prestige: f.i64_or0("prestige"), rebirths: f.i64_or0("rebirths") }
            })
            .collect())
    }
}

// ================================================================ Worker
type Payload = Result<Box<dyn Any + Send>, OnlineError>;
type OkCb<C> = Box<dyn FnOnce(&mut C, Box<dyn Any + Send>)>;
type ErrCb<C> = Box<dyn FnOnce(&mut C, OnlineError)>;

/// Runs network calls on threads and hands the results back to the main loop (poll()).
pub struct Worker<C> {
    tx: Sender<(u64, Payload)>,
    rx: Receiver<(u64, Payload)>,
    next: u64,
    pending: HashMap<u64, (Option<OkCb<C>>, Option<ErrCb<C>>)>,
}

impl<C: 'static> Default for Worker<C> {
    fn default() -> Self {
        Self::new()
    }
}

impl<C: 'static> Worker<C> {
    pub fn new() -> Worker<C> {
        let (tx, rx) = channel();
        Worker { tx, rx, next: 0, pending: HashMap::new() }
    }

    pub fn run<T: Send + 'static>(
        &mut self,
        job: impl FnOnce() -> Res<T> + Send + 'static,
        ok: Option<Box<dyn FnOnce(&mut C, T)>>,
        err: Option<Box<dyn FnOnce(&mut C, OnlineError)>>,
    ) {
        self.next += 1;
        let id = self.next;
        let okb: Option<OkCb<C>> = ok.map(|f| {
            Box::new(move |c: &mut C, v: Box<dyn Any + Send>| {
                if let Ok(v) = v.downcast::<T>() {
                    f(c, *v);
                }
            }) as OkCb<C>
        });
        self.pending.insert(id, (okb, err));
        let tx = self.tx.clone();
        std::thread::spawn(move || {
            let r = std::panic::catch_unwind(std::panic::AssertUnwindSafe(job));
            let payload: Payload = match r {
                Ok(Ok(v)) => Ok(Box::new(v)),
                Ok(Err(e)) => Err(e),
                Err(_) => Err(OnlineError::new("server", "internal error")),
            };
            let _ = tx.send((id, payload));
        });
    }

    /// The finished jobs' callbacks, to be called with the context by the owner.
    pub fn take_done(&mut self) -> Vec<Box<dyn FnOnce(&mut C)>> {
        let mut out: Vec<Box<dyn FnOnce(&mut C)>> = Vec::new();
        while let Ok((id, payload)) = self.rx.try_recv() {
            let Some((ok, err)) = self.pending.remove(&id) else { continue };
            match payload {
                Ok(v) => {
                    if let Some(ok) = ok {
                        out.push(Box::new(move |c| ok(c, v)));
                    }
                }
                Err(e) => {
                    if let Some(err) = err {
                        out.push(Box::new(move |c| err(c, e)));
                    }
                }
            }
        }
        out
    }
}

pub type Client = Arc<FirebaseClient>;
