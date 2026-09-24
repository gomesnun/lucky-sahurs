//! Game languages (English / Português). Every visible text is written in English and goes
//! through tr(); the Portuguese catalogue lives in i18n_pt.rs.

use crate::i18n_pt::{PT, PT_SHORT};
use crate::pyfmt::{Arg, format as pyformat, try_format};
use std::collections::HashMap;
use std::sync::OnceLock;
use std::sync::atomic::{AtomicBool, Ordering};

pub const DEFAULT_LANGUAGE: &str = "en";
pub const LANGUAGES: [(&str, &str); 2] = [("en", "English"), ("pt", "Português")];
pub const LANGUAGE_CODES: [&str; 2] = ["en", "pt"];

static PT_ACTIVE: AtomicBool = AtomicBool::new(false);

fn pt_map() -> &'static HashMap<&'static str, &'static str> {
    static M: OnceLock<HashMap<&'static str, &'static str>> = OnceLock::new();
    M.get_or_init(|| PT.iter().cloned().collect())
}

fn pt_short_map() -> &'static HashMap<&'static str, &'static str> {
    static M: OnceLock<HashMap<&'static str, &'static str>> = OnceLock::new();
    M.get_or_init(|| PT_SHORT.iter().cloned().collect())
}

/// A pre-formatted English text that remembers its template (Python's L()/TStr).
#[derive(Clone, Debug, PartialEq)]
pub struct LStr {
    pub template: &'static str,
    pub args: Vec<Arg>,
}

impl LStr {
    pub fn plain(template: &'static str) -> LStr {
        LStr { template, args: Vec::new() }
    }
    /// The English text (what str(TStr) gives).
    pub fn english(&self) -> String {
        if self.args.is_empty() { self.template.to_string() } else { pyformat(self.template, &self.args) }
    }
    /// tr(tstr) - translated with the same numbers.
    pub fn tr(&self) -> String {
        if self.args.is_empty() {
            return tr(self.template);
        }
        let template = lookup(self.template);
        match try_format(template, &self.args) {
            Some(s) => s,
            None => self.english(),
        }
    }
}

/// L(template, args...)
pub fn l(template: &'static str, args: Vec<Arg>) -> LStr {
    LStr { template, args }
}

pub fn set_language(code: &str) -> &'static str {
    let pt = code == "pt";
    PT_ACTIVE.store(pt, Ordering::Relaxed);
    if pt { "pt" } else { "en" }
}

pub fn get_language() -> &'static str {
    if PT_ACTIVE.load(Ordering::Relaxed) { "pt" } else { "en" }
}

pub fn language_name(code: &str) -> String {
    for (c, name) in LANGUAGES {
        if c == code {
            return name.to_string();
        }
    }
    code.to_string()
}

pub fn next_language(code: &str) -> &'static str {
    match LANGUAGE_CODES.iter().position(|c| *c == code) {
        None => LANGUAGE_CODES[0],
        Some(i) => LANGUAGE_CODES[(i + 1) % LANGUAGE_CODES.len()],
    }
}

fn lookup(text: &str) -> &str {
    if PT_ACTIVE.load(Ordering::Relaxed) {
        if let Some(t) = pt_map().get(text) {
            return t;
        }
    }
    text
}

/// tr(text) without arguments.
pub fn tr(text: &str) -> String {
    lookup(text).to_string()
}

/// tr(text, *args)
pub fn tra(text: &str, args: &[Arg]) -> String {
    if args.is_empty() {
        return tr(text);
    }
    let template = lookup(text);
    if let Some(s) = try_format(template, args) {
        return s;
    }
    // broken translation: English
    try_format(text, args).unwrap_or_else(|| text.to_string())
}

/// tr_short(text, *args): short form for tight spots, if the language has one.
pub fn tr_short(text: &str) -> String {
    if PT_ACTIVE.load(Ordering::Relaxed) {
        if let Some(t) = pt_short_map().get(text) {
            return t.to_string();
        }
    }
    tr(text)
}

/// `tr!("text")` / `tr!("Slot %d", n)`
#[macro_export]
macro_rules! tr {
    ($t:expr) => { $crate::i18n::tr($t) };
    ($t:expr, $($a:expr),+ $(,)?) => { $crate::i18n::tra($t, &$crate::args![$($a),+]) };
}
