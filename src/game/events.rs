//! Global events: state, polling and admin control (online/events.py), plus the banner and the
//! admin page (ui/events_panel.py).

use super::base::Bo;
use super::{Game, cb};
use crate::config::{TOPBAR_H, VIRTUAL_H};
use crate::core::event::set_event_mults;
use crate::core::state::now_ts;
use crate::gfx::{Color, Rect, draw};
use crate::i18n::tr;
use crate::online::firebase::{Event, online_error_text};
use crate::pyfmt::format as pyformat;
use crate::theme::*;
use crate::ui::drawing::{dim_overlay, draw_panel};
use crate::ui::fonts::fit_text;
use crate::{args, tr};
use std::rc::Rc;

pub const EVENT_POLL: f64 = 45.0;
pub const EVENT_KINDS: [&str; 3] = ["luck", "money", "speed"];
pub const EVENT_MULTS: [i64; 4] = [2, 5, 10, 25];
pub const EVENT_MINUTES: [i64; 4] = [1, 5, 15, 30];
pub const EVENT_MAX_SECONDS: i64 = 3600;

pub fn event_kind_label(kind: &str) -> String {
    match kind {
        "luck" => tr("Luck"),
        "money" => tr("Money"),
        "speed" => tr("Auto Speed"),
        other => other.to_string(),
    }
}

fn g_fmt(v: f64) -> String {
    pyformat("%g", &args![v])
}

pub struct EventsUi {
    pub current: Option<Event>,
    pub next_poll: f64,
    pub loading: bool,
    pub is_admin: bool,
    pub admin_checked: bool,
    pub admin_open: bool,
    pub admin_mult: i64,
    pub admin_minutes: i64,
    pub busy: bool,
    pub msg: Option<(String, Color)>,
}

impl EventsUi {
    pub fn new() -> EventsUi {
        EventsUi {
            current: None,
            next_poll: 0.0,
            loading: false,
            is_admin: false,
            admin_checked: false,
            admin_open: false,
            admin_mult: 10,
            admin_minutes: 5,
            busy: false,
            msg: None,
        }
    }
}

impl Game {
    pub fn active_event(&self) -> Option<Event> {
        self.ev.current.clone().filter(|e| e.ends_at > now_ts())
    }

    pub fn event_seconds_left(&self) -> f64 {
        self.active_event().map(|e| (e.ends_at - now_ts()).max(0.0)).unwrap_or(0.0)
    }

    fn events_ready(&self) -> bool {
        self.client.is_some() && self.account.is_some()
    }

    pub fn poll_events(&mut self, force: bool) {
        if !self.events_ready() || self.ev.loading {
            return;
        }
        let now = now_ts();
        if !force && now < self.ev.next_poll {
            return;
        }
        self.ev.loading = true;
        self.ev.next_poll = now + EVENT_POLL;
        let client = self.client.clone().unwrap();
        self.run_job(
            move || client.get_event(),
            |g, ev: Option<Event>| {
                g.ev.loading = false;
                g.ev.current = ev;
            },
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
        let ev = self.active_event();
        let kind = ev.as_ref().map(|e| e.kind.as_str()).unwrap_or("");
        let mult = ev.as_ref().map(|e| e.mult).unwrap_or(1.0);
        set_event_mults(if kind == "luck" { mult } else { 1.0 }, if kind == "money" { mult } else { 1.0 }, if kind == "speed" { mult } else { 1.0 });
    }

    pub fn toggle_event_admin(&mut self) {
        self.ev.admin_open = !self.ev.admin_open;
        self.ev.msg = None;
    }

    pub fn close_event_admin(&mut self) {
        self.ev.admin_open = false;
        self.ev.msg = None;
    }

    pub fn start_event(&mut self, kind: &'static str) {
        if !(self.events_ready() && self.ev.is_admin) || self.ev.busy {
            return;
        }
        let seconds = EVENT_MAX_SECONDS.min(self.ev.admin_minutes * 60);
        let mult = self.ev.admin_mult;
        self.ev.busy = true;
        self.ev.msg = None;
        let client = self.client.clone().unwrap();
        self.run_job(
            move || client.start_event(kind, mult as f64, seconds as f64),
            |g, ev: Event| {
                g.ev.busy = false;
                g.ev.current = Some(ev);
                g.ev.next_poll = 0.0;
                g.ev.msg = Some((tr("Event started for everyone!"), GOOD));
            },
            |g, e| {
                g.ev.busy = false;
                g.ev.msg = Some((online_error_text(&e), BAD));
            },
        );
    }

    pub fn stop_event(&mut self) {
        if !(self.events_ready() && self.ev.is_admin) || self.ev.busy {
            return;
        }
        self.ev.busy = true;
        self.ev.msg = None;
        let client = self.client.clone().unwrap();
        self.run_job(
            move || client.stop_event(),
            |g, _: ()| {
                g.ev.busy = false;
                g.ev.current = None;
                g.ev.next_poll = 0.0;
                g.ev.msg = Some((tr("Event stopped."), GOOD));
            },
            |g, e| {
                g.ev.busy = false;
                g.ev.msg = Some((online_error_text(&e), BAD));
            },
        );
    }

    // ---------------------------------------------------------------- banner
    fn event_banner_text(&self) -> Option<String> {
        let ev = self.active_event()?;
        let left = self.event_seconds_left() as i64;
        let clock = format!("{}:{:02}", left / 60, left % 60);
        Some(tr!("GLOBAL EVENT: %sx %s - %s left", g_fmt(ev.mult), event_kind_label(&ev.kind), clock))
    }

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

    // ---------------------------------------------------------------- admin page
    pub fn draw_event_admin(&mut self, mouse_pos: (f64, f64)) {
        let ov = dim_overlay(self.vw, VIRTUAL_H, 170);
        self.canvas.blit(&ov, 0, 0);
        let panel_w = 520.min(self.vw - 40);
        let panel_h = 430;
        let rect = Rect::new(self.vw / 2 - panel_w / 2, 20.max(VIRTUAL_H / 2 - panel_h / 2), panel_w, panel_h);
        draw_panel(&mut self.canvas, rect, Some(panel()), 16, true, None);
        self.register_button(rect, Rc::new(|_: &mut Game| {}), None);

        let sb = self.f.small_b.clone();
        let small = self.f.small.clone();
        let title = self.f.big.render(&tr("Global event"), WHITE);
        self.canvas.blit(&title, rect.x + 24, rect.y + 18);
        self.button(Rect::new(rect.right() - 46, rect.y + 20, 28, 28), "X", &sb, mouse_pos, panel_light(), BAD, WHITE, cb(|g| g.close_event_admin()), Bo::r(8));
        let sub = small.render(&tr("Starts a bonus for every player online."), grey());
        self.canvas.blit(&sub, rect.x + 24, rect.y + 20 + title.h);

        let x0 = rect.x + 24;
        let w = panel_w - 48;
        let mut y = rect.y + 30 + title.h + sub.h;

        let mults: Vec<(String, i64)> = EVENT_MULTS.iter().map(|&m| (format!("{}x", g_fmt(m as f64)), m)).collect();
        let chosen = self.ev.admin_mult;
        y = self.draw_event_choices(x0, y, w, &tr("Multiplier"), mouse_pos, &mults, chosen, |g, v| g.ev.admin_mult = v);
        let mins: Vec<(String, i64)> = EVENT_MINUTES.iter().map(|&m| (tr!("%d min", m), m)).collect();
        let chosen = self.ev.admin_minutes;
        y = self.draw_event_choices(x0, y, w, &tr("Duration"), mouse_pos, &mins, chosen, |g, v| g.ev.admin_minutes = v);

        let label = sb.render(&tr("Start for everyone"), grey_dim());
        self.canvas.blit(&label, x0, y);
        y += label.h + 6;
        let gap = 8;
        let n = EVENT_KINDS.len() as i32;
        let bw = (w - gap * (n - 1)) / n;
        let busy = self.ev.busy;
        for (i, kind) in EVENT_KINDS.iter().enumerate() {
            let k: &'static str = kind;
            self.button(Rect::new(x0 + i as i32 * (bw + gap), y, bw, 44), &event_kind_label(k), &sb, mouse_pos, accent(), accent_hover(), BLACK, if busy { None } else { cb(move |g| g.start_event(k)) }, Bo::r(10));
        }
        y += 56;

        if let Some(ev) = self.active_event() {
            let by = if ev.by.is_empty() { "?".to_string() } else { ev.by.clone() };
            let info = tr!("Now: %sx %s (%d s left), by %s", g_fmt(ev.mult), event_kind_label(&ev.kind), self.event_seconds_left() as i64, by);
            let t = small.render(&fit_text(&small, &info, w), accent());
            self.canvas.blit(&t, x0, y);
            y += t.h + 8;
            self.button(Rect::new(x0, y, w, 40), &tr("Stop event"), &sb, mouse_pos, Color::rgb(150, 60, 60), BAD, WHITE, if busy { None } else { cb(|g| g.stop_event()) }, Bo::r(10));
        } else {
            let t = small.render(&tr("No event running."), grey());
            self.canvas.blit(&t, x0, y);
        }

        if let Some((text, color)) = self.ev.msg.clone() {
            let t = small.render(&fit_text(&small, &text, w), color);
            self.canvas.blit(&t, x0, rect.bottom() - 30);
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn draw_event_choices(&mut self, x0: i32, mut y: i32, w: i32, label: &str, mouse_pos: (f64, f64), options: &[(String, i64)], chosen: i64, on_pick: fn(&mut Game, i64)) -> i32 {
        let sb = self.f.small_b.clone();
        let t = sb.render(label, grey_dim());
        self.canvas.blit(&t, x0, y);
        y += t.h + 6;
        let gap = 8;
        let n = options.len() as i32;
        let bw = (w - gap * (n - 1)) / n;
        for (i, (text, value)) in options.iter().enumerate() {
            let on = *value == chosen;
            let v = *value;
            self.button(
                Rect::new(x0 + i as i32 * (bw + gap), y, bw, 40),
                text,
                &sb,
                mouse_pos,
                if on { panel_lighter() } else { panel_light() },
                panel_lighter(),
                if on { accent() } else { WHITE },
                cb(move |g| on_pick(g, v)),
                Bo::r(9).border(if on { Some(accent()) } else { None }),
            );
        }
        y + 52
    }
}
