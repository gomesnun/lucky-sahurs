//! Global events: state, polling and admin control (online/events.py), plus the banner and the
//! admin page (ui/events_panel.py).

use super::base::{Bo, FieldRef};
use super::{Game, KeyEv, cb};
use crate::config::{TOPBAR_H, VIRTUAL_H};
use crate::core::event::set_event_mults;
use crate::gfx::{Color, Rect, draw};
use crate::i18n::tr;
use crate::online::firebase::{Event, online_error_text, server_now};
use crate::pyfmt::format as pyformat;
use crate::theme::*;
use crate::ui::drawing::{dim_overlay, draw_panel};
use crate::ui::fonts::fit_text;
use crate::ui::widgets::TextField;
use crate::{args, tr};
use std::collections::HashSet;
use std::rc::Rc;

/// seconds between reads of the events (each read costs 1 per active event, per player)
pub const EVENT_POLL: f64 = 15.0;
pub const EVENT_KINDS: [&str; 3] = ["luck", "money", "speed"];
/// quick presets - the admin can also type any number in the text fields
pub const EVENT_MULT_PRESETS: [i64; 15] = [2, 5, 10, 25, 50, 100, 250, 500, 1000, 5000, 10000, 50000, 100000, 500000, 1000000];
pub const EVENT_SECONDS_PRESETS: [i64; 12] = [5, 10, 30, 60, 300, 900, 1800, 3600, 7200, 21600, 43200, 86400];
/// 24 h: the same limit as in the Firestore rules
pub const EVENT_MAX_SECONDS: i64 = 86400;
pub const EVENT_MAX_MULT: i64 = 1000000;

pub fn event_kind_label(kind: &str) -> String {
    match kind {
        "luck" => tr("Luck"),
        "money" => tr("Money"),
        "speed" => tr("Auto Speed"),
        other => other.to_string(),
    }
}

pub fn event_seconds_label(seconds: i64) -> String {
    if seconds < 60 {
        tr!("%ds", seconds)
    } else if seconds < 3600 {
        tr!("%d min", seconds / 60)
    } else {
        tr!("%dh", seconds / 3600)
    }
}

/// 1000 -> "x1K", 1000000 -> "x1M" (only to fit on the button; the real value stays the same).
pub fn fmt_mult(n: i64) -> String {
    if n >= 1000000 && n % 1000000 == 0 {
        format!("x{}M", n / 1000000)
    } else if n >= 1000 && n % 1000 == 0 {
        format!("x{}K", n / 1000)
    } else {
        format!("x{}", n)
    }
}

fn g_fmt(v: f64) -> String {
    pyformat("%g", &args![v])
}

/// int(text) of a digits field (always digits; huge numbers saturate).
fn digits_value(text: &str) -> i64 {
    text.trim().parse::<i64>().unwrap_or(i64::MAX)
}

pub struct EventsUi {
    /// only the active ones, one per kind
    pub current: Vec<Event>,
    pub next_poll: f64,
    pub loading: bool,
    pub is_admin: bool,
    pub admin_checked: bool,
    pub admin_open: bool,
    pub admin_mult: i64,
    pub admin_seconds: i64,
    /// "mult" | "seconds" | None
    pub admin_focus: Option<&'static str>,
    pub mult_field: TextField,
    pub seconds_field: TextField,
    /// kinds with a request running (start or stop)
    pub busy: HashSet<&'static str>,
    pub msg: Option<(String, Color)>,
}

impl EventsUi {
    pub fn new() -> EventsUi {
        EventsUi {
            current: Vec::new(),
            next_poll: 0.0,
            loading: false,
            is_admin: false,
            admin_checked: false,
            admin_open: false,
            admin_mult: 10,
            admin_seconds: 300,
            admin_focus: None,
            mult_field: TextField::new("digits"),
            seconds_field: TextField::new("digits"),
            busy: HashSet::new(),
            msg: None,
        }
    }
}

impl Game {
    /// The event of this kind if it's still running, or None (also lets it drop by itself). Uses the SERVER's
    /// time, so it ends at the same moment for everyone.
    pub fn active_event(&self, kind: &str) -> Option<Event> {
        self.ev.current.iter().find(|e| e.kind == kind && e.ends_at > server_now()).cloned()
    }

    pub fn any_active_events(&self) -> Vec<&'static str> {
        EVENT_KINDS.iter().copied().filter(|k| self.active_event(k).is_some()).collect()
    }

    pub fn event_mult(&self, kind: &str) -> f64 {
        self.active_event(kind).map(|e| e.mult.max(1.0)).unwrap_or(1.0)
    }

    pub fn event_seconds_left(&self, kind: &str) -> f64 {
        self.active_event(kind).map(|e| (e.ends_at - server_now()).max(0.0)).unwrap_or(0.0)
    }

    fn events_ready(&self) -> bool {
        self.client.is_some() && self.account.is_some()
    }

    pub fn poll_events(&mut self, force: bool) {
        if !self.events_ready() || self.ev.loading {
            return;
        }
        let now = crate::core::state::now_ts();
        if !force && now < self.ev.next_poll {
            return;
        }
        self.ev.loading = true;
        self.ev.next_poll = now + EVENT_POLL;
        let client = self.client.clone().unwrap();
        self.run_job(
            move || client.get_events(),
            |g, evs: Vec<Event>| {
                g.ev.loading = false;
                g.ev.current = evs;
            },
            // failing to read the events isn't worth showing: the game goes on
            |g, _| g.ev.loading = false,
        );
    }

    fn check_admin(&mut self) {
        if !self.events_ready() || self.ev.admin_checked {
            return;
        }
        self.ev.admin_checked = true;
        let client = self.client.clone().unwrap();
        self.run_job(move || client.is_admin(), |g, v: bool| g.ev.is_admin = v, |_, _| {});
    }

    pub fn tick_events(&mut self, _now: f64) {
        if self.events_ready() {
            self.check_admin();
            self.poll_events(false);
        }
        // the GameState doesn't know about the online part: each kind's bonus goes through core/event.rs,
        // one by one (all three can be active at once)
        set_event_mults(self.event_mult("luck"), self.event_mult("money"), self.event_mult("speed"));
    }

    // ---------------------------------------------------------------- admin: multiplier / duration
    pub fn toggle_event_admin(&mut self) {
        self.ev.admin_open = !self.ev.admin_open;
        self.ev.msg = None;
        if !self.ev.admin_open {
            self.set_event_admin_focus(None);
        }
    }

    pub fn close_event_admin(&mut self) {
        self.ev.admin_open = false;
        self.ev.msg = None;
        self.set_event_admin_focus(None);
    }

    pub fn set_event_admin_focus(&mut self, field: Option<&'static str>) {
        if field == self.ev.admin_focus {
            return;
        }
        self.ev.admin_focus = field;
        if field.is_some() {
            self.start_text_input();
        } else {
            self.stop_text_input();
        }
    }

    pub fn set_event_mult(&mut self, mult: i64) {
        self.ev.admin_mult = mult.max(1);
        self.ev.mult_field.set_text(""); // picking a preset clears what was typed
    }

    pub fn set_event_seconds(&mut self, seconds: i64) {
        self.ev.admin_seconds = seconds.max(1);
        self.ev.seconds_field.set_text("");
    }

    /// What's typed in the field (if anything valid) becomes the multiplier to use.
    fn commit_event_fields(&mut self) {
        let t = self.ev.mult_field.text.trim().to_string();
        if !t.is_empty() {
            self.ev.admin_mult = digits_value(&t).clamp(1, EVENT_MAX_MULT);
        }
        let t = self.ev.seconds_field.text.trim().to_string();
        if !t.is_empty() {
            self.ev.admin_seconds = digits_value(&t).clamp(1, EVENT_MAX_SECONDS);
        }
    }

    // ---------------------------------------------------------------- admin: start / stop
    pub fn start_event(&mut self, kind: &'static str) {
        if !(self.events_ready() && self.ev.is_admin) || self.ev.busy.contains(kind) {
            return;
        }
        self.commit_event_fields();
        let seconds = EVENT_MAX_SECONDS.min(self.ev.admin_seconds);
        let mult = EVENT_MAX_MULT.min(self.ev.admin_mult);
        self.ev.busy.insert(kind);
        self.ev.msg = None;
        let client = self.client.clone().unwrap();
        self.run_job(
            move || client.start_event(kind, mult as f64, seconds as f64),
            move |g, ev: Event| {
                g.ev.busy.remove(kind);
                g.ev.current.retain(|e| e.kind != kind);
                g.ev.current.push(ev);
                g.ev.next_poll = 0.0;
                g.ev.msg = Some((tr!("%s event started for everyone!", event_kind_label(kind)), GOOD));
            },
            move |g, e| {
                g.ev.busy.remove(kind);
                g.ev.msg = Some((online_error_text(&e), BAD));
            },
        );
    }

    pub fn stop_event(&mut self, kind: &'static str) {
        if !(self.events_ready() && self.ev.is_admin) || self.ev.busy.contains(kind) {
            return;
        }
        self.ev.busy.insert(kind);
        self.ev.msg = None;
        let client = self.client.clone().unwrap();
        self.run_job(
            move || client.stop_event(kind),
            move |g, _: ()| {
                g.ev.busy.remove(kind);
                g.ev.current.retain(|e| e.kind != kind);
                g.ev.next_poll = 0.0;
                g.ev.msg = Some((tr!("%s event stopped.", event_kind_label(kind)), GOOD));
            },
            move |g, e| {
                g.ev.busy.remove(kind);
                g.ev.msg = Some((online_error_text(&e), BAD));
            },
        );
    }

    // ---------------------------------------------------------------- admin: keyboard (text fields)
    /// Keys with the multiplier or the duration selected. Returns true if the key was used.
    pub fn handle_event_admin_key(&mut self, ev: KeyEv) -> bool {
        use sdl2::keyboard::Keycode as K;
        let Some(focus) = self.ev.admin_focus else { return false };
        if !self.ev.admin_open {
            return false;
        }
        let field = if focus == "mult" { FieldRef::EventMult } else { FieldRef::EventSeconds };
        match ev.key {
            K::Return | K::KpEnter | K::Tab | K::Escape => {
                self.set_event_admin_focus(None);
                true
            }
            _ => self.edit_field_key(field, ev, true),
        }
    }

    // ---------------------------------------------------------------- banner
    fn event_banner_text(&self) -> Option<String> {
        let active = self.any_active_events();
        if active.is_empty() {
            return None;
        }
        let parts: Vec<String> = active
            .iter()
            .filter_map(|k| {
                let ev = self.active_event(k)?;
                let left = self.event_seconds_left(k) as i64;
                Some(format!("{}x {} ({}:{:02})", g_fmt(ev.mult), event_kind_label(k), left / 60, left % 60))
            })
            .collect();
        Some(tr!("ADMIN ABUSE: %s", parts.join(" | ")))
    }

    /// Thin strip under the top bar. Not clickable: just to be seen.
    pub fn draw_event_banner(&mut self) {
        let Some(text) = self.event_banner_text() else { return };
        let h = 26;
        let rect = Rect::new(0, TOPBAR_H + 2, self.vw, h);
        draw::rect(&mut self.canvas, accent(), rect, 0, 0);
        let sb = self.f.small_b.clone();
        let t = sb.render(&fit_text(&sb, &text, self.vw - 24), BLACK);
        let r = Rect::with_center(t.w, t.h, rect.center());
        self.canvas.blit(&t, r.x, r.y);
    }

    // ---------------------------------------------------------------- admin page ("Admin Abuse")
    pub fn draw_event_admin(&mut self, mouse_pos: (f64, f64)) {
        let ov = dim_overlay(self.vw, VIRTUAL_H, 170);
        self.canvas.blit(&ov, 0, 0);
        let panel_w = 600.min(self.vw - 40);
        let panel_h = (VIRTUAL_H - 40).min(730);
        let rect = Rect::new(self.vw / 2 - panel_w / 2, 20.max(VIRTUAL_H / 2 - panel_h / 2), panel_w, panel_h);
        draw_panel(&mut self.canvas, rect, Some(panel()), 16, true, None);
        self.register_button(rect, Rc::new(|_: &mut Game| {}), None);

        let sb = self.f.small_b.clone();
        let small = self.f.small.clone();
        let title = self.f.big.render(&tr("Admin Abuse"), WHITE);
        self.canvas.blit(&title, rect.x + 24, rect.y + 18);
        self.button(Rect::new(rect.right() - 46, rect.y + 20, 28, 28), "X", &sb, mouse_pos, panel_light(), BAD, WHITE, cb(|g| g.close_event_admin()), Bo::r(8));
        self.draw_back_button(rect, mouse_pos);
        let sub = small.render(&fit_text(&small, &tr("Each type runs on its own - start Luck, Money and Speed all at once."), panel_w - 48), grey());
        self.canvas.blit(&sub, rect.x + 24, rect.y + 20 + title.h);

        let x0 = rect.x + 24;
        let w = panel_w - 48;
        let mut y = rect.y + 30 + title.h + sub.h;

        // ---- multiplier: text field + presets ----
        y = self.draw_event_number_field(x0, y, w, &tr("Multiplier"), FieldRef::EventMult, "mult", self.ev.admin_mult, EVENT_MAX_MULT);
        let opts: Vec<(String, i64)> = EVENT_MULT_PRESETS.iter().map(|&m| (fmt_mult(m), m)).collect();
        let chosen = self.ev.admin_mult;
        y = self.draw_choice_grid(x0, y, w, mouse_pos, &opts, chosen, |g, v| g.set_event_mult(v), 5);
        y += 6;

        // ---- duration: text field + presets ----
        y = self.draw_event_number_field(x0, y, w, &tr("Time (seconds)"), FieldRef::EventSeconds, "seconds", self.ev.admin_seconds, EVENT_MAX_SECONDS);
        let opts: Vec<(String, i64)> = EVENT_SECONDS_PRESETS.iter().map(|&s| (event_seconds_label(s), s)).collect();
        let chosen = self.ev.admin_seconds;
        y = self.draw_choice_grid(x0, y, w, mouse_pos, &opts, chosen, |g, v| g.set_event_seconds(v), 4);
        y += 10;

        // ---- one row per kind: state + start/stop ----
        for kind in EVENT_KINDS {
            y = self.draw_event_kind_row(x0, y, w, kind, mouse_pos);
        }

        if let Some((text, color)) = self.ev.msg.clone() {
            let t = small.render(&fit_text(&small, &text, w), color);
            self.canvas.blit(&t, x0, rect.bottom() - 28);
        }
    }

    /// Label + a text field where the number is typed. The label shows the value that will REALLY be used: what's
    /// typed (if anything), already capped at the maximum.
    #[allow(clippy::too_many_arguments)]
    fn draw_event_number_field(&mut self, x0: i32, mut y: i32, w: i32, label: &str, field: FieldRef, focus_key: &'static str, current: i64, maximum: i64) -> i32 {
        let typed = self.field_mut(field).text.trim().to_string();
        let mut current = current;
        if !typed.is_empty() {
            current = digits_value(&typed).clamp(1, maximum);
        }
        let mut text = tr!("%s (now: %s)", label, g_fmt(current as f64));
        if !typed.is_empty() && digits_value(&typed) > maximum {
            text += &format!("  {}", tr!("- max is %s", g_fmt(maximum as f64)));
        }
        let sb = self.f.small_b.clone();
        let t = sb.render(&text, grey_dim());
        self.canvas.blit(&t, x0, y);
        y += t.h + 6;
        let field_rect = Rect::new(x0, y, w, 40);
        let display = self.field_mut(field).text.clone();
        let focused = self.ev.admin_focus == Some(focus_key);
        self.draw_text_field(field_rect, field, &display, &tr("Type a number..."), focused, Rc::new(move |g: &mut Game| g.set_event_admin_focus(Some(focus_key))));
        y + 40 + 10
    }

    /// Preset buttons in several rows (only one lit, and picking one clears the text field).
    #[allow(clippy::too_many_arguments)]
    pub fn draw_choice_grid(&mut self, x0: i32, mut y: i32, w: i32, mouse_pos: (f64, f64), options: &[(String, i64)], chosen: i64, on_pick: fn(&mut Game, i64), per_row: usize) -> i32 {
        let (height, gap) = (34, 6);
        let font = self.f.tiny_b.clone();
        for row in options.chunks(per_row) {
            let n = row.len() as i32;
            let bw = if n > 1 { (w - gap * (n - 1)) / n } else { w };
            for (i, (text, value)) in row.iter().enumerate() {
                let on = *value == chosen;
                let v = *value;
                self.button(
                    Rect::new(x0 + i as i32 * (bw + gap), y, bw, height),
                    text,
                    &font,
                    mouse_pos,
                    if on { panel_lighter() } else { panel_light() },
                    panel_lighter(),
                    if on { accent() } else { WHITE },
                    cb(move |g| on_pick(g, v)),
                    Bo::r(8).border(if on { Some(accent()) } else { None }),
                );
            }
            y += height + gap;
        }
        y
    }

    fn draw_event_kind_row(&mut self, x0: i32, y: i32, w: i32, kind: &'static str, mouse_pos: (f64, f64)) -> i32 {
        let ev = self.active_event(kind);
        let busy = self.ev.busy.contains(kind);
        let row_h = 56;
        let sb = self.f.small_b.clone();
        let tiny = self.f.tiny.clone();
        let label = sb.render(&event_kind_label(kind), WHITE);
        self.canvas.blit(&label, x0, y + 2);
        let (info, info_color) = match &ev {
            Some(e) => {
                let by = if e.by.is_empty() { "?".to_string() } else { e.by.clone() };
                (tr!("%sx, %d s left, by %s", g_fmt(e.mult), self.event_seconds_left(kind) as i64, by), accent())
            }
            None => (tr("Not running."), grey()),
        };
        let t = tiny.render(&fit_text(&tiny, &info, w - 140), info_color);
        self.canvas.blit(&t, x0, y + 2 + label.h + 2);
        let btn_w = 110;
        let btn_rect = Rect::new(x0 + w - btn_w, y, btn_w, row_h - 10);
        if ev.is_some() {
            self.button(btn_rect, &tr("Stop"), &sb, mouse_pos, Color::rgb(150, 60, 60), BAD, WHITE, if busy { None } else { cb(move |g| g.stop_event(kind)) }, Bo::r(10));
        } else {
            self.button(btn_rect, &tr("Start"), &sb, mouse_pos, accent(), accent_hover(), BLACK, if busy { None } else { cb(move |g| g.start_event(kind)) }, Bo::r(10));
        }
        y + row_h
    }
}
