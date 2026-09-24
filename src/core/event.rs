//! Multipliers of the running global event (core/global_event.py). online/events writes, the
//! luck / money / speed maths reads.

use std::cell::Cell;

thread_local! {
    static MULTS: Cell<(f64, f64, f64)> = const { Cell::new((1.0, 1.0, 1.0)) };
}

pub fn set_event_mults(luck: f64, money: f64, speed: f64) {
    MULTS.with(|m| m.set((luck.max(1.0), money.max(1.0), speed.max(1.0))));
}

pub fn event_mult(kind: &str) -> f64 {
    let (l, m, s) = MULTS.with(|m| m.get());
    match kind {
        "luck" => l,
        "money" => m,
        "speed" => s,
        _ => 1.0,
    }
}
