//! Python `%`-formatting (the subset the game's strings use: %s %d %i %f %g %e %x %% with flags,
//! width and precision), so translated templates behave exactly like `template % args`.

#[derive(Clone, Debug, PartialEq)]
pub enum Arg {
    Int(i64),
    Float(f64),
    Str(String),
}

impl From<i64> for Arg {
    fn from(v: i64) -> Arg {
        Arg::Int(v)
    }
}
impl From<i32> for Arg {
    fn from(v: i32) -> Arg {
        Arg::Int(v as i64)
    }
}
impl From<u32> for Arg {
    fn from(v: u32) -> Arg {
        Arg::Int(v as i64)
    }
}
impl From<usize> for Arg {
    fn from(v: usize) -> Arg {
        Arg::Int(v as i64)
    }
}
impl From<f64> for Arg {
    fn from(v: f64) -> Arg {
        Arg::Float(v)
    }
}
impl From<&str> for Arg {
    fn from(v: &str) -> Arg {
        Arg::Str(v.to_string())
    }
}
impl From<String> for Arg {
    fn from(v: String) -> Arg {
        Arg::Str(v)
    }
}
impl From<&String> for Arg {
    fn from(v: &String) -> Arg {
        Arg::Str(v.clone())
    }
}

/// Build a Vec<Arg> from heterogeneous values: `args![1, "x", 2.5]`
#[macro_export]
macro_rules! args {
    () => { Vec::<$crate::pyfmt::Arg>::new() };
    ($($e:expr),+ $(,)?) => { vec![$($crate::pyfmt::Arg::from($e)),+] };
}

/// Python repr-style str() of a float (shortest round-trip, like `str(1.5)` -> "1.5").
pub fn py_float_str(v: f64) -> String {
    if v.is_nan() {
        return "nan".into();
    }
    if v.is_infinite() {
        return if v > 0.0 { "inf".into() } else { "-inf".into() };
    }
    if v == 0.0 {
        return if v.is_sign_negative() { "-0.0".into() } else { "0.0".into() };
    }
    let a = v.abs();
    if (1e-4..1e16).contains(&a) {
        let s = format!("{}", v);
        if s.contains('.') { s } else { format!("{}.0", s) }
    } else {
        // exponent form: Python uses e.g. 1e+16, 1.5e-05
        let s = format!("{:e}", v);
        let (mant, exp) = s.split_once('e').unwrap();
        let exp: i32 = exp.parse().unwrap();
        format!("{}e{}{:02}", mant, if exp < 0 { '-' } else { '+' }, exp.abs())
    }
}

fn arg_str(a: &Arg) -> String {
    match a {
        Arg::Int(i) => i.to_string(),
        Arg::Float(f) => py_float_str(*f),
        Arg::Str(s) => s.clone(),
    }
}

fn arg_int(a: &Arg) -> Option<i64> {
    match a {
        Arg::Int(i) => Some(*i),
        Arg::Float(f) => Some(f.trunc() as i64),
        Arg::Str(_) => None,
    }
}

fn arg_float(a: &Arg) -> Option<f64> {
    match a {
        Arg::Int(i) => Some(*i as f64),
        Arg::Float(f) => Some(*f),
        Arg::Str(_) => None,
    }
}

/// C-style %e formatting with Python exponent format (at least 2 digits).
fn fmt_e(v: f64, prec: usize, upper: bool) -> String {
    let s = format!("{:.*e}", prec, v);
    let (mant, exp) = s.split_once('e').unwrap();
    let exp: i32 = exp.parse().unwrap();
    let e = if upper { 'E' } else { 'e' };
    format!("{}{}{}{:02}", mant, e, if exp < 0 { '-' } else { '+' }, exp.abs())
}

/// %g formatting (C semantics, as Python uses them).
fn fmt_g(v: f64, prec: usize, alt: bool, upper: bool) -> String {
    if v.is_nan() {
        return if upper { "NAN".into() } else { "nan".into() };
    }
    if v.is_infinite() {
        let s = if v > 0.0 { "inf" } else { "-inf" };
        return if upper { s.to_uppercase() } else { s.into() };
    }
    let p = if prec == 0 { 1 } else { prec };
    if v == 0.0 {
        let mut s = String::from(if v.is_sign_negative() { "-0" } else { "0" });
        if alt && p > 1 {
            s.push('.');
            s.push_str(&"0".repeat(p - 1));
        }
        return s;
    }
    // exponent after rounding to p significant digits
    let es = format!("{:.*e}", p - 1, v);
    let (_, exp) = es.split_once('e').unwrap();
    let x: i32 = exp.parse().unwrap();
    let mut s = if x < -4 || x >= p as i32 {
        fmt_e(v, p - 1, upper)
    } else {
        format!("{:.*}", (p as i32 - 1 - x).max(0) as usize, v)
    };
    if !alt {
        // strip trailing zeros in the fraction (and the dot)
        if let Some(epos) = s.find(['e', 'E']) {
            let (m, e) = s.split_at(epos);
            let mut m = m.to_string();
            if m.contains('.') {
                while m.ends_with('0') {
                    m.pop();
                }
                if m.ends_with('.') {
                    m.pop();
                }
            }
            s = format!("{}{}", m, e);
        } else if s.contains('.') {
            while s.ends_with('0') {
                s.pop();
            }
            if s.ends_with('.') {
                s.pop();
            }
        }
    }
    s
}

/// Rust's `{:.N}` rounds with round-half-to-even on the exact binary value, which is what C's
/// printf (and therefore Python) does too.
fn fmt_f(v: f64, prec: usize) -> String {
    if v.is_nan() {
        return "nan".into();
    }
    if v.is_infinite() {
        return if v > 0.0 { "inf".into() } else { "-inf".into() };
    }
    format!("{:.*}", prec, v)
}

/// `template % args`. Returns None where Python would raise (wrong count / type).
pub fn try_format(template: &str, args: &[Arg]) -> Option<String> {
    let mut out = String::with_capacity(template.len() + 16);
    let chars: Vec<char> = template.chars().collect();
    let mut i = 0;
    let mut ai = 0;
    while i < chars.len() {
        let c = chars[i];
        if c != '%' {
            out.push(c);
            i += 1;
            continue;
        }
        i += 1;
        if i >= chars.len() {
            return None;
        }
        // flags
        let (mut left, mut plus, mut space, mut zero, mut alt) = (false, false, false, false, false);
        while i < chars.len() {
            match chars[i] {
                '-' => left = true,
                '+' => plus = true,
                ' ' => space = true,
                '0' => zero = true,
                '#' => alt = true,
                _ => break,
            }
            i += 1;
        }
        let mut width = 0usize;
        while i < chars.len() && chars[i].is_ascii_digit() {
            width = width * 10 + chars[i].to_digit(10).unwrap() as usize;
            i += 1;
        }
        let mut prec: Option<usize> = None;
        if i < chars.len() && chars[i] == '.' {
            i += 1;
            let mut p = 0usize;
            while i < chars.len() && chars[i].is_ascii_digit() {
                p = p * 10 + chars[i].to_digit(10).unwrap() as usize;
                i += 1;
            }
            prec = Some(p);
        }
        if i >= chars.len() {
            return None;
        }
        let conv = chars[i];
        i += 1;
        if conv == '%' {
            out.push('%');
            continue;
        }
        let arg = args.get(ai)?;
        ai += 1;
        let (body, numeric, negative) = match conv {
            's' | 'r' => {
                let mut s = arg_str(arg);
                if let Some(p) = prec {
                    s = s.chars().take(p).collect();
                }
                (s, false, false)
            }
            'd' | 'i' | 'u' => {
                let v = arg_int(arg)?;
                (v.unsigned_abs().to_string(), true, v < 0)
            }
            'x' | 'X' => {
                let v = arg_int(arg)?;
                let s = format!("{:x}", v.unsigned_abs());
                (if conv == 'X' { s.to_uppercase() } else { s }, true, v < 0)
            }
            'f' | 'F' => {
                let v = arg_float(arg)?;
                let s = fmt_f(v.abs(), prec.unwrap_or(6));
                (s, true, v.is_sign_negative())
            }
            'e' | 'E' => {
                let v = arg_float(arg)?;
                (fmt_e(v.abs(), prec.unwrap_or(6), conv == 'E'), true, v < 0.0)
            }
            'g' | 'G' => {
                let v = arg_float(arg)?;
                (fmt_g(v.abs(), prec.unwrap_or(6), alt, conv == 'G'), true, v < 0.0 || (v == 0.0 && v.is_sign_negative()))
            }
            _ => return None,
        };
        let sign = if numeric {
            if negative {
                "-"
            } else if plus {
                "+"
            } else if space {
                " "
            } else {
                ""
            }
        } else {
            ""
        };
        let len = sign.chars().count() + body.chars().count();
        if len >= width {
            out.push_str(sign);
            out.push_str(&body);
        } else if left {
            out.push_str(sign);
            out.push_str(&body);
            out.push_str(&" ".repeat(width - len));
        } else if zero && numeric {
            out.push_str(sign);
            out.push_str(&"0".repeat(width - len));
            out.push_str(&body);
        } else {
            out.push_str(&" ".repeat(width - len));
            out.push_str(sign);
            out.push_str(&body);
        }
    }
    if ai != args.len() {
        return None; // "not all arguments converted"
    }
    Some(out)
}

/// `template % args`, falling back to the template itself on error.
pub fn format(template: &str, args: &[Arg]) -> String {
    try_format(template, args).unwrap_or_else(|| template.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn dump_cases() {
        let fl = [0.5, 1.5, 2.5, 0.25, 0.125, 1.005, 2.675, 1234.5678, 0.045, 99.995, -0.04, 1e-5, 123456789.0, 0.1 + 0.2, 1e16, 3.0, 12.0, 0.07 * 100.0, 16.0000001];
        let mut out = String::new();
        for v in fl {
            for t in ["%.0f", "%.1f", "%.2f", "%g", "%.3g", "%s", "%d", "%5.1f", "%02d"] {
                out.push_str(&format!("{}|{}\n", t, super::format(t, &[Arg::Float(v)])));
            }
        }
        std::fs::write(std::env::var("PYFMT_OUT").unwrap_or("/tmp/pyfmt_rs.txt".into()), out).unwrap();
    }
}
