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

pub enum Body {
    None,
    Json(Value),
    Form(Vec<(String, String)>),
}

pub fn http_json(method: &str, url: &str, body: Body, headers: &[(&str, String)]) -> Res<Value> {
    let a = agent();
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

fn event_from_doc(doc: &Value) -> Event {
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
    Event { kind: f.str_or("kind", "luck"), mult, ends_at: ends.filter(|e| *e != 0.0).unwrap_or(0.0), by: f.str_or("by", "") }
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
    pub time: Option<f64>,
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
    pub username: String,
    pub coins: f64,
    pub playtime: f64,
    pub rolls: i64,
    pub rebirths: i64,
    pub updated_at: Option<f64>,
}

#[derive(Clone, Debug)]
pub struct Message {
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

    pub fn publish_score(&self, coins: f64, playtime: f64, rolls: i64, rebirths: i64) -> Res<()> {
        let uid = self.need_uid()?;
        let name = format!("{}/leaderboard/{}", self.docs_root, uid);
        let rolls = (rolls as f64).min(9e18).max(0.0) as i64;
        let rebirths = (rebirths as f64).min(1e9).max(0.0) as i64;
        let body = json!({"writes": [{
            "update": {"name": name, "fields": {
                "username": fs_str(&self.username().unwrap_or_else(|| "None".into())),
                "coins": fs_f64(coins),
                "playtime": fs_f64(playtime),
                "rolls": fs_int(rolls),
                "rebirths": fs_int(rebirths),
            }},
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
            "updateTransforms": [{"fieldPath": "updated_at", "setToServerValue": "REQUEST_TIME"}],
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
        Some(Person { uid, username: username.into(), avatar_pet: None, avatar_mut: "normal".into(), time: None })
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
            out.push(Person { uid: other_uid, username: f.str_or(on, "?"), avatar_pet: None, avatar_mut: "normal".into(), time: parse_timestamp(f.get("created_at")) });
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
        let body = json!({"writes": [
            {"update": {"name": mine, "fields": {"username": fs_str(fu)}},
             "updateTransforms": [{"fieldPath": "since", "setToServerValue": "REQUEST_TIME"}]},
            {"update": {"name": theirs, "fields": {"username": fs_str(&self.username().unwrap_or_default())}},
             "updateTransforms": [{"fieldPath": "since", "setToServerValue": "REQUEST_TIME"}]},
            {"delete": format!("{}{}", self.docs_root, Self::request_path(&uid, from_uid))},
        ]});
        self.fs("POST", ":commit", Some(body), &[]).map(|_| ())
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
                out.push(Person { uid: fuid, username: f.str_or("username", "?"), avatar_pet: None, avatar_mut: "normal".into(), time: parse_timestamp(f.get("since")) });
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

    pub fn get_event(&self) -> Res<Option<Event>> {
        match self.fs("GET", "/events/current", None, &[]) {
            Ok(d) => Ok(Some(event_from_doc(&d))),
            Err(e) if e.code == "not_found" => Ok(None),
            Err(e) => Err(e),
        }
    }

    pub fn start_event(&self, kind: &str, mult: f64, seconds: f64) -> Res<Event> {
        let uid = self.need_uid()?;
        let ends = now() + seconds;
        let by = self.username().filter(|s| !s.is_empty()).unwrap_or(uid);
        let mut fields = Map::new();
        fields.insert("by".into(), fs_str(&by));
        fields.insert("ends_at".into(), json!({"timestampValue": iso_timestamp(ends)}));
        fields.insert("kind".into(), fs_str(kind));
        fields.insert("mult".into(), fs_f64(mult));
        let params: Vec<(String, String)> = fields.keys().map(|k| ("updateMask.fieldPaths".to_string(), k.clone())).collect();
        self.fs("PATCH", "/events/current", Some(json!({"fields": fields})), &params)?;
        Ok(Event { kind: kind.into(), mult, ends_at: ends, by })
    }

    pub fn stop_event(&self) -> Res<()> {
        match self.fs("DELETE", "/events/current", None, &[]) {
            Err(e) if e.code != "not_found" => Err(e),
            _ => Ok(()),
        }
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
                LbEntry { username, value: f.f64_or0(field) }
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

    pub fn inflight(&self) -> usize {
        self.pending.len()
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
