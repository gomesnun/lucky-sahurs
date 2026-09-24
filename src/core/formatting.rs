//! Number, chance and time formatting (core/formatting.py).

use crate::pyfmt::{Arg, format};
use crate::tr;

pub fn format_number(n: f64) -> String {
    let sign = if n < 0.0 { "-" } else { "" };
    let mut n = n.abs();
    if n < 1000.0 {
        if n < 10.0 && n != n.trunc() {
            return format("%s%.1f", &[Arg::from(sign), Arg::Float(n)]);
        }
        return format("%s%.0f", &[Arg::from(sign), Arg::Float(n)]);
    }
    const SUFFIXES: [&str; 12] = ["", "K", "M", "B", "T", "Qa", "Qi", "Sx", "Sp", "Oc", "No", "Dc"];
    let mut idx = 0;
    while n >= 1000.0 && idx < SUFFIXES.len() - 1 {
        n /= 1000.0;
        idx += 1;
    }
    format("%s%.2f%s", &[Arg::from(sign), Arg::Float(n), Arg::from(SUFFIXES[idx])])
}

/// Python's round() for floats (round-half-even on the exact value) returning an integer-valued f64.
pub fn py_round(x: f64) -> f64 {
    let r = x.round();
    if (x - x.trunc()).abs() == 0.5 {
        // tie: to even
        let t = x.trunc();
        if t % 2.0 == 0.0 { t } else { t + x.signum() }
    } else {
        r
    }
}

pub fn format_one_in(chance: f64) -> String {
    if chance <= 0.0 {
        return "-".into();
    }
    tr!("1 in %s", format_number(py_round(1.0 / chance)))
}

pub fn format_playtime(seconds: f64) -> String {
    let seconds = seconds.max(0.0) as i64;
    let d = seconds / 86400;
    let rem = seconds % 86400;
    let h = rem / 3600;
    let rem = rem % 3600;
    let m = rem / 60;
    let s = rem % 60;
    if d > 0 {
        return format!("{}d {:02}h", d, h);
    }
    if h > 0 {
        return format!("{}h {:02}m", h, m);
    }
    if m > 0 {
        return format!("{}m {:02}s", m, s);
    }
    format!("{}s", s)
}
