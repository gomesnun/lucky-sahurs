//! Admin commands: the admin menu (Admin Abuse / Bans), the Bans page and the screen a banned account sees
//! (online/admin.py + ui/admin_panel.py). The Admin Abuse page (events) is in events.rs.
//!
//! Bans: an admin looks the account up by username, writes the reason and bans it. A /bans/{uid} document is
//! left (only admins write it - see the Firestore rules). A banned player sees a screen with the reason right
//! when the game opens and, if they're playing, within BAN_POLL seconds (10 min); they're sent to the menu and
//! can only log out or quit. Unbanning deletes the document: the account plays normally again.

use super::base::{Bo, FieldRef};
use super::{Game, KeyEv, cb};
use crate::config::VIRTUAL_H;
use crate::core::state::now_ts;
use crate::gfx::{Color, Rect, Surface, draw};
use crate::i18n::tr;
use super::events::{EVENT_KINDS, EVENT_MAX_MULT, EVENT_MAX_SECONDS, EVENT_MULT_PRESETS, EVENT_SECONDS_PRESETS, event_kind_label, event_seconds_label, fmt_mult};
use crate::online::firebase::{Ban, Event, LogEntry, Person, online_error_text, username_ok};
use crate::theme::*;
use crate::tr;
use crate::ui::drawing::{dim_overlay, draw_panel};
use crate::ui::fonts::{fit_text, wrap_text};
use crate::ui::widgets::TextField;
use std::rc::Rc;

/// how often this account checks whether it was banned (1 read, 10 min)
pub const BAN_POLL: f64 = 600.0;
/// the same limit as in the Firestore rules
pub const BAN_REASON_MAX: usize = 300;

pub struct AdminUi {
    pub menu_open: bool,
    pub ban_open: bool,
    /// v3.0.1: the Bans page also works as the Resets page ("ban" / "reset"), sharing the player search
    pub page: &'static str,
    pub search: TextField,
    pub reason: TextField,
    /// "search" | "reason" | None
    pub focus: Option<&'static str>,
    pub found: Option<Person>,
    /// search / ban / unban running
    pub busy: bool,
    /// the Ban button is at "Are you sure?"
    pub confirm: bool,
    pub msg: Option<(String, Color)>,
    pub list: Vec<Ban>,
    pub list_loading: bool,
    pub list_loaded: bool,
    pub scroll: f64,
    pub max_scroll: f64,
    pub list_rect: Rect,
    /// v4.0.1: the audit log page ("log") - who did a reset or a ban, and when
    pub log: Vec<LogEntry>,
    pub log_loading: bool,
    pub log_loaded: bool,

    /// THIS account's ban - None = can play
    pub info: Option<Ban>,
    pub check_at: f64,
    pub checking: bool,
    /// so it checks again right away when another account logs in
    pub checked_uid: Option<String>,
}

impl AdminUi {
    pub fn new() -> AdminUi {
        let mut reason = TextField::new("text");
        reason.max_len = BAN_REASON_MAX;
        AdminUi {
            menu_open: false,
            ban_open: false,
            page: "ban",
            search: TextField::new("username"),
            reason,
            focus: None,
            found: None,
            busy: false,
            confirm: false,
            msg: None,
            list: Vec::new(),
            list_loading: false,
            list_loaded: false,
            scroll: 0.0,
            max_scroll: 0.0,
            list_rect: Rect::ZERO,
            log: Vec::new(),
            log_loading: false,
            log_loaded: false,
            info: None,
            check_at: 0.0,
            checking: false,
            checked_uid: None,
        }
    }
}

impl Game {
    // ================================================================ admin menu
    pub fn toggle_admin_menu(&mut self) {
        if self.adm.menu_open {
            self.adm.menu_open = false;
            return;
        }
        self.close_overlays();
        self.adm.menu_open = true;
    }

    pub fn close_admin_menu(&mut self) {
        self.adm.menu_open = false;
    }

    pub fn open_admin_abuse(&mut self) {
        self.adm.menu_open = false;
        self.ev.admin_open = true;
        self.ev.msg = None;
    }

    /// Admin Abuse aimed at one player: search a username, then give (or clear) their own event.
    pub fn open_personal_admin(&mut self) {
        self.adm.menu_open = false;
        self.adm.ban_open = true;
        self.adm.page = "personal";
        self.adm.msg = None;
        self.adm.found = None;
        self.adm.confirm = false;
        self.set_ban_focus(Some("search"));
    }

    /// v3.0.1: Resets (one player, or everyone).
    pub fn open_reset_admin(&mut self) {
        self.adm.menu_open = false;
        self.adm.ban_open = true;
        self.adm.page = "reset";
        self.adm.msg = None;
        self.adm.found = None;
        self.season.player_confirm = false;
        self.season.confirm = false;
        self.set_ban_focus(Some("search"));
    }

    pub fn open_ban_admin(&mut self) {
        self.adm.menu_open = false;
        self.adm.ban_open = true;
        self.adm.page = "ban";
        self.adm.msg = None;
        self.adm.confirm = false;
        self.adm.scroll = 0.0;
        self.set_ban_focus(Some("search"));
        self.refresh_ban_list();
    }

    /// v4.0.1: the audit log - every reset and ban, who did it and when.
    pub fn open_admin_log(&mut self) {
        self.adm.menu_open = false;
        self.adm.ban_open = true;
        self.adm.page = "log";
        self.adm.msg = None;
        self.adm.scroll = 0.0;
        self.set_ban_focus(None);
        self.refresh_admin_log();
    }

    pub fn refresh_admin_log(&mut self) {
        if !self.friends_ready() || self.adm.log_loading {
            return;
        }
        self.adm.log_loading = true;
        let client = self.client.clone().unwrap();
        self.run_job(
            move || client.list_admin_log(300),
            |g, log: Vec<LogEntry>| {
                g.adm.log_loading = false;
                g.adm.log_loaded = true;
                g.adm.log = log;
            },
            |g, e| {
                g.adm.log_loading = false;
                g.adm.log_loaded = true;
                g.adm.msg = Some((online_error_text(&e), BAD));
            },
        );
    }

    pub fn close_ban_admin(&mut self) {
        self.adm.ban_open = false;
        self.adm.confirm = false;
        self.set_ban_focus(None);
    }

    /// The Back button of the Admin Abuse and Bans pages.
    pub fn back_to_admin_menu(&mut self) {
        if self.ev.admin_open {
            self.close_event_admin();
        }
        if self.adm.ban_open {
            self.close_ban_admin();
        }
        self.adm.menu_open = true;
    }

    // ================================================================ bans (the admin's page)
    pub fn set_ban_focus(&mut self, field: Option<&'static str>) {
        if field == self.adm.focus {
            return;
        }
        self.adm.focus = field;
        if field.is_some() {
            self.start_text_input();
        } else {
            self.stop_text_input();
        }
    }

    fn ban_field(&self) -> FieldRef {
        if self.adm.focus == Some("search") { FieldRef::BanSearch } else { FieldRef::BanReason }
    }

    pub fn ban_type(&mut self, text: &str) {
        if self.adm.focus.is_none() {
            return;
        }
        let before = self.adm.search.text.clone();
        let f = self.ban_field();
        self.field_mut(f).add(text);
        if self.adm.focus == Some("search") && self.adm.search.text != before {
            self.adm.found = None; // the name changed: the previous search no longer counts
        }
        self.adm.confirm = false;
    }

    /// Keys with the Bans page open. Returns true if the key was used.
    pub fn handle_ban_key(&mut self, ev: KeyEv) -> bool {
        use sdl2::keyboard::Keycode as K;
        if !self.adm.ban_open {
            return false;
        }
        if ev.key == K::Escape {
            if self.adm.focus.is_some() {
                self.set_ban_focus(None);
            } else {
                self.close_ban_admin();
            }
            return true;
        }
        let Some(focus) = self.adm.focus else { return false };
        match ev.key {
            K::Tab if self.adm.page == "reset" => {}
            K::Return | K::KpEnter if self.adm.page == "reset" => self.search_ban_player(),
            K::Return | K::KpEnter => {
                if focus == "search" {
                    self.search_ban_player();
                } else {
                    self.press_ban();
                }
            }
            K::Tab => self.set_ban_focus(Some(if focus == "search" { "reason" } else { "search" })),
            K::V if ev.ctrl => {
                let t = self.clipboard_text();
                self.ban_type(&t);
                return true;
            }
            _ => {
                let f = self.ban_field();
                if !self.edit_field_key(f, ev, true) {
                    return false;
                }
            }
        }
        if focus == "search" && matches!(ev.key, K::Backspace | K::Delete) {
            self.adm.found = None;
        }
        self.adm.confirm = false;
        true
    }

    pub fn search_ban_player(&mut self) {
        if !self.friends_ready() || self.adm.busy {
            return;
        }
        let name = self.adm.search.text.trim().to_lowercase();
        if !username_ok(&name) {
            self.adm.msg = Some((tr("Usernames have 3 to 16 letters, numbers or _."), BAD));
            return;
        }
        self.adm.busy = true;
        self.adm.found = None;
        self.adm.msg = None;
        self.adm.confirm = false;
        let client = self.client.clone().unwrap();
        self.run_job(
            move || client.find_profile(&name),
            |g, profile: Option<Person>| {
                g.adm.busy = false;
                match profile {
                    None => g.adm.msg = Some((tr("No player with that name."), BAD)),
                    Some(p) => {
                        g.adm.found = Some(p);
                        g.season.player_confirm = false;
                        g.set_ban_focus(if g.adm.page == "reset" || g.adm.page == "personal" { None } else { Some("reason") });
                    }
                }
            },
            |g, e| {
                g.adm.busy = false;
                g.adm.msg = Some((online_error_text(&e), BAD));
            },
        );
    }

    pub fn is_banned_uid(&self, uid: &str) -> bool {
        self.adm.list.iter().any(|b| b.uid == uid)
    }

    /// 1st click: "Are you sure?". 2nd click: bans (with the reason written).
    pub fn press_ban(&mut self) {
        let Some(found) = self.adm.found.clone() else { return };
        if self.adm.busy {
            return;
        }
        let reason = self.adm.reason.text.trim().to_string();
        if reason.is_empty() {
            self.adm.msg = Some((tr("Write a reason first."), BAD));
            self.set_ban_focus(Some("reason"));
            return;
        }
        if self.account.as_ref().is_some_and(|a| a.uid == found.uid) {
            self.adm.msg = Some((tr("You can't ban yourself."), BAD));
            return;
        }
        if !self.adm.confirm {
            self.adm.confirm = true;
            return;
        }
        self.adm.confirm = false;
        self.adm.busy = true;
        let (uid, username) = (found.uid.clone(), found.username.clone());
        let client = self.client.clone().unwrap();
        let name2 = username.clone();
        self.run_job(
            move || client.ban_player(&uid, &username, &reason),
            move |g, _: ()| {
                g.adm.busy = false;
                g.adm.msg = Some((tr!("%s was banned.", name2), GOOD));
                g.adm.found = None;
                g.adm.search.set_text("");
                g.adm.reason.set_text("");
                g.set_ban_focus(Some("search"));
                g.refresh_ban_list();
            },
            |g, e| {
                g.adm.busy = false;
                g.adm.msg = Some((online_error_text(&e), BAD));
            },
        );
    }

    /// Admin Abuse for ONE player: gives them (and only them) the multiplier/duration/kind picked on the page.
    pub fn give_personal_event(&mut self) {
        let Some(found) = self.adm.found.clone() else { return };
        if self.adm.busy || !self.ev.is_admin {
            return;
        }
        self.commit_event_fields();
        let kind = self.ev.target_kind;
        let seconds = EVENT_MAX_SECONDS.min(self.ev.admin_seconds) as f64;
        let mult = EVENT_MAX_MULT.min(self.ev.admin_mult) as f64;
        self.adm.busy = true;
        self.adm.msg = None;
        let (uid, username) = (found.uid.clone(), found.username.clone());
        let client = self.client.clone().unwrap();
        self.run_job(
            move || client.start_personal_event(&uid, kind, mult, seconds),
            move |g, _: Event| {
                g.adm.busy = false;
                g.adm.msg = Some((tr!("%s got a personal %s event.", username, event_kind_label(kind)), GOOD));
            },
            |g, e| {
                g.adm.busy = false;
                g.adm.msg = Some((online_error_text(&e), BAD));
            },
        );
    }

    /// Ends the found player's personal event early.
    pub fn clear_personal_event(&mut self) {
        let Some(found) = self.adm.found.clone() else { return };
        if self.adm.busy || !self.ev.is_admin {
            return;
        }
        self.adm.busy = true;
        self.adm.msg = None;
        let (uid, username) = (found.uid.clone(), found.username.clone());
        let client = self.client.clone().unwrap();
        self.run_job(
            move || client.stop_personal_event(&uid),
            move |g, _: ()| {
                g.adm.busy = false;
                g.adm.msg = Some((tr!("%s's personal event was cleared.", username), GOOD));
            },
            |g, e| {
                g.adm.busy = false;
                g.adm.msg = Some((online_error_text(&e), BAD));
            },
        );
    }

    pub fn unban(&mut self, entry: Ban) {
        if self.adm.busy {
            return;
        }
        self.adm.busy = true;
        let client = self.client.clone().unwrap();
        let uid = entry.uid.clone();
        self.run_job(
            move || client.unban_player(&uid),
            move |g, _: ()| {
                g.adm.busy = false;
                g.adm.msg = Some((tr!("%s was unbanned.", entry.username), GOOD));
                g.adm.list.retain(|b| b.uid != entry.uid);
            },
            |g, e| {
                g.adm.busy = false;
                g.adm.msg = Some((online_error_text(&e), BAD));
            },
        );
    }

    pub fn refresh_ban_list(&mut self) {
        if !self.friends_ready() || self.adm.list_loading {
            return;
        }
        self.adm.list_loading = true;
        let client = self.client.clone().unwrap();
        self.run_job(
            move || client.list_bans(300),
            |g, bans: Vec<Ban>| {
                g.adm.list_loading = false;
                g.adm.list_loaded = true;
                g.adm.list = bans;
            },
            |g, e| {
                g.adm.list_loading = false;
                g.adm.list_loaded = true;
                g.adm.msg = Some((online_error_text(&e), BAD));
            },
        );
    }

    // ================================================================ THIS account's ban
    /// Every frame (on any screen): right after logging in and then every BAN_POLL seconds, checks whether this
    /// account was banned (or unbanned).
    pub fn tick_ban(&mut self, now: f64) {
        let (Some(_), Some(acc)) = (self.client.as_ref(), self.account.as_ref()) else {
            self.adm.info = None;
            self.adm.checked_uid = None;
            return;
        };
        let uid = acc.uid.clone();
        if self.adm.checked_uid.as_deref() != Some(uid.as_str()) {
            self.adm.checked_uid = Some(uid.clone());
            self.adm.check_at = 0.0;
            self.adm.info = None;
        }
        if self.adm.checking || now < self.adm.check_at {
            return;
        }
        self.adm.checking = true;
        self.adm.check_at = now + BAN_POLL;
        let client = self.client.clone().unwrap();
        self.run_job(
            move || client.get_my_ban(),
            move |g, ban: Option<Ban>| {
                g.adm.checking = false;
                if g.account.as_ref().map(|a| a.uid.as_str()) != Some(uid.as_str()) {
                    return; // logged out meanwhile
                }
                let banned = ban.is_some();
                g.adm.info = ban;
                if banned && g.screen_mode == "game" {
                    g.close_overlays();
                    g.go_to_menu(); // saves and leaves the game; the ban screen stays on top
                }
            },
            // no network: try again at the next BAN_POLL
            |g, _| g.adm.checking = false,
        );
    }

    /// The "You are banned" screen is in front (blocks everything else).
    pub fn ban_screen_active(&self) -> bool {
        self.adm.info.is_some() && self.screen_mode != "account"
    }

    pub fn ban_log_out(&mut self) {
        self.adm.info = None;
        self.log_out();
        self.show_toast(&tr("Logged out."), 2.2);
    }

    // ================================================================ screens
    pub fn draw_admin_menu(&mut self, mouse_pos: (f64, f64)) {
        let ov = dim_overlay(self.vw, VIRTUAL_H, 170);
        self.canvas.blit(&ov, 0, 0);
        let (panel_w, panel_h) = (460.min(self.vw - 40), 468); // 4 buttons + v4.0.1's "Log"
        let rect = Rect::new(self.vw / 2 - panel_w / 2, VIRTUAL_H / 2 - panel_h / 2, panel_w, panel_h);
        draw_panel(&mut self.canvas, rect, Some(panel()), 16, true, None);
        self.register_blocker(rect);

        let sb = self.f.small_b.clone();
        let med = self.f.med.clone();
        let title = self.f.big.render(&tr("Admin commands"), WHITE);
        self.canvas.blit(&title, rect.x + 24, rect.y + 18);
        self.button(Rect::new(rect.right() - 46, rect.y + 20, 28, 28), "X", &sb, mouse_pos, panel_light(), BAD, WHITE, cb(|g| g.close_admin_menu()), Bo::r(8));
        let sub = self.f.small.render(&tr("Only admins can see this."), grey());
        self.canvas.blit(&sub, rect.x + 24, rect.y + 20 + title.h);

        let (x0, w) = (rect.x + 24, panel_w - 48);
        let mut y = rect.y + 104;
        self.button(Rect::new(x0, y, w, 54), &tr("Admin Abuse"), &med, mouse_pos, panel_light(), panel_lighter(), accent(), cb(|g| g.open_admin_abuse()), Bo::r(12).icon("admin_event"));
        y += 66;
        self.button(Rect::new(x0, y, w, 54), &tr("Target one player"), &med, mouse_pos, panel_light(), panel_lighter(), accent(), cb(|g| g.open_personal_admin()), Bo::r(12).icon("admin_event"));
        y += 66;
        self.button(Rect::new(x0, y, w, 54), &tr("Bans"), &med, mouse_pos, panel_light(), panel_lighter(), BAD, cb(|g| g.open_ban_admin()), Bo::r(12).icon("ban"));
        y += 66;
        self.button(Rect::new(x0, y, w, 54), &tr("Resets"), &med, mouse_pos, panel_light(), panel_lighter(), WHITE, cb(|g| g.open_reset_admin()), Bo::r(12).icon("rebirth"));
        y += 66;
        self.button(Rect::new(x0, y, w, 54), &tr("Log"), &med, mouse_pos, panel_light(), panel_lighter(), WHITE, cb(|g| g.open_admin_log()), Bo::r(12).icon("index"));
    }

    /// v3.0.1 Resets page: reset one player (search) or everyone (new season).
    pub fn draw_reset_admin(&mut self, mouse_pos: (f64, f64)) {
        let ov = dim_overlay(self.vw, VIRTUAL_H, 170);
        self.canvas.blit(&ov, 0, 0);
        let panel_w = 640.min(self.vw - 40);
        let panel_h = 520.min(VIRTUAL_H - 40);
        let rect = Rect::new(self.vw / 2 - panel_w / 2, VIRTUAL_H / 2 - panel_h / 2, panel_w, panel_h);
        draw_panel(&mut self.canvas, rect, Some(panel()), 16, true, None);
        self.register_button(rect, Rc::new(|g: &mut Game| g.set_ban_focus(None)), None);
        let sb = self.f.small_b.clone();
        let small = self.f.small.clone();
        let med = self.f.med.clone();
        let title = self.f.big.render(&tr("Resets"), WHITE);
        self.canvas.blit(&title, rect.x + 24, rect.y + 18);
        self.button(Rect::new(rect.right() - 46, rect.y + 20, 28, 28), "X", &sb, mouse_pos, panel_light(), BAD, WHITE, cb(|g| g.close_ban_admin()), Bo::r(8));
        self.draw_back_button(rect, mouse_pos);
        let (x0, w) = (rect.x + 24, panel_w - 48);
        let mut y = rect.y + 30 + title.h;

        // ---- 1. one player ----
        let t = med.render(&tr("Reset one player"), accent());
        self.canvas.blit(&t, x0, y);
        y += t.h + 4;
        for line in wrap_text(&tr("Wipes only their saves (cloud and every PC) and takes them off the leaderboard. Their account and friends stay."), &small, w) {
            let l = small.render(&line, grey());
            self.canvas.blit(&l, x0, y);
            y += l.h + 2;
        }
        y += 8;
        let field = Rect::new(x0, y, w - 130, 44);
        let display = self.adm.search.text.clone();
        let focused = self.adm.focus == Some("search");
        self.draw_text_field(field, FieldRef::BanSearch, &display, &tr("Username..."), focused, Rc::new(|g: &mut Game| g.set_ban_focus(Some("search"))));
        let busy = self.adm.busy || self.season.busy;
        self.button(Rect::new(field.right() + 10, y, 120, 44), &tr("Search"), &med, mouse_pos, accent(), accent_hover(), BLACK, cb(|g| g.search_ban_player()), Bo::r(10).enabled(!busy));
        y += 44 + 10;
        if let Some(found) = self.adm.found.clone() {
            let label = if self.season.player_confirm {
                tr!("Sure? Reset %s to 0", found.username.clone())
            } else {
                tr!("Reset %s's saves", found.username.clone())
            };
            let base = if self.season.player_confirm { BAD } else { Color::rgb(150, 60, 60) };
            self.button(Rect::new(x0, y, w, 44), &label, &med, mouse_pos, base, Color::rgb(235, 90, 90), WHITE, cb(|g| g.press_reset_player()), Bo::r(10).enabled(!busy).icon("rebirth"));
            y += 44 + 8;
        }
        if let Some((msg, color)) = self.adm.msg.clone() {
            for line in wrap_text(&msg, &small, w) {
                let l = small.render(&line, color);
                self.canvas.blit(&l, x0, y);
                y += l.h + 2;
            }
        }

        // ---- 2. everyone ----
        let y2 = rect.bottom() - 24 - 54 - 26 - 40;
        draw::line(&mut self.canvas, panel_light(), (x0, y2 - 14), (x0 + w, y2 - 14), 2);
        let t = med.render(&tr("Reset everyone"), BAD);
        self.canvas.blit(&t, x0, y2);
        let l = small.render(&fit_text(&small, &tr("A new season: every player's saves start again from 0 and the leaderboard is emptied."), w), grey());
        self.canvas.blit(&l, x0, y2 + t.h + 4);
        self.draw_reset_everyone_button(Rect::new(x0, rect.bottom() - 24 - 54, w, 54), mouse_pos);
    }

    /// Admin Abuse aimed at one player only: search them, pick a kind/multiplier/duration (the same fields as
    /// the global Admin Abuse page) and give it just to them.
    fn draw_personal_admin(&mut self, mouse_pos: (f64, f64)) {
        let ov = dim_overlay(self.vw, VIRTUAL_H, 170);
        self.canvas.blit(&ov, 0, 0);
        let panel_w = 640.min(self.vw - 40);
        let panel_h = (VIRTUAL_H - 40).min(760);
        let rect = Rect::new(self.vw / 2 - panel_w / 2, 20.max(VIRTUAL_H / 2 - panel_h / 2), panel_w, panel_h);
        draw_panel(&mut self.canvas, rect, Some(panel()), 16, true, None);
        self.register_button(rect, Rc::new(|g: &mut Game| g.set_ban_focus(None)), None);
        let sb = self.f.small_b.clone();
        let small = self.f.small.clone();
        let med = self.f.med.clone();
        let title = self.f.big.render(&tr("Target one player"), WHITE);
        self.canvas.blit(&title, rect.x + 24, rect.y + 18);
        self.button(Rect::new(rect.right() - 46, rect.y + 20, 28, 28), "X", &sb, mouse_pos, panel_light(), BAD, WHITE, cb(|g| g.close_ban_admin()), Bo::r(8));
        self.draw_back_button(rect, mouse_pos);
        let (x0, w) = (rect.x + 24, panel_w - 48);
        let mut y = rect.y + 30 + title.h;

        for line in wrap_text(&tr("The same Luck/Money/Auto Speed boost as Admin Abuse, but only this one player sees it."), &small, w) {
            let l = small.render(&line, grey());
            self.canvas.blit(&l, x0, y);
            y += l.h + 2;
        }
        y += 6;

        // ---- search ----
        let field = Rect::new(x0, y, w - 130, 44);
        let display = self.adm.search.text.clone();
        let focused = self.adm.focus == Some("search");
        self.draw_text_field(field, FieldRef::BanSearch, &display, &tr("Username..."), focused, Rc::new(|g: &mut Game| g.set_ban_focus(Some("search"))));
        let busy = self.adm.busy;
        self.button(Rect::new(field.right() + 10, y, 120, 44), &tr("Search"), &med, mouse_pos, accent(), accent_hover(), BLACK, cb(|g| g.search_ban_player()), Bo::r(10).enabled(!busy));
        y += 44 + 10;

        let Some(found) = self.adm.found.clone() else {
            let t = small.render(&tr("Search a username to target them."), grey());
            self.canvas.blit(&t, x0, y);
            if let Some((text, color)) = self.adm.msg.clone() {
                y += t.h + 8;
                let m = small.render(&fit_text(&small, &text, w), color);
                self.canvas.blit(&m, x0, y);
            }
            return;
        };
        let t = sb.render(&tr!("Found: %s", found.username.clone()), accent());
        self.canvas.blit(&t, x0, y);
        y += t.h + 10;

        // ---- kind ----
        let t = sb.render(&tr("Type"), grey_dim());
        self.canvas.blit(&t, x0, y);
        y += t.h + 6;
        let kind_opts: Vec<(String, i64)> = EVENT_KINDS.iter().enumerate().map(|(i, k)| (event_kind_label(k), i as i64)).collect();
        let chosen = EVENT_KINDS.iter().position(|k| *k == self.ev.target_kind).unwrap_or(1) as i64;
        y = self.draw_choice_grid(x0, y, w, mouse_pos, &kind_opts, chosen, |g, v| g.ev.target_kind = EVENT_KINDS[v as usize], 3);
        y += 6;

        // ---- multiplier / duration (same fields the global page uses) ----
        y = self.draw_event_number_field(x0, y, w, &tr("Multiplier"), FieldRef::EventMult, "mult", self.ev.admin_mult, EVENT_MAX_MULT);
        let opts: Vec<(String, i64)> = EVENT_MULT_PRESETS.iter().map(|&m| (fmt_mult(m), m)).collect();
        let chosen = self.ev.admin_mult;
        y = self.draw_choice_grid(x0, y, w, mouse_pos, &opts, chosen, |g, v| g.set_event_mult(v), 5);
        y += 6;
        y = self.draw_event_number_field(x0, y, w, &tr("Time (seconds)"), FieldRef::EventSeconds, "seconds", self.ev.admin_seconds, EVENT_MAX_SECONDS);
        let opts: Vec<(String, i64)> = EVENT_SECONDS_PRESETS.iter().map(|&s| (event_seconds_label(s), s)).collect();
        let chosen = self.ev.admin_seconds;
        y = self.draw_choice_grid(x0, y, w, mouse_pos, &opts, chosen, |g, v| g.set_event_seconds(v), 4);
        y += 10;

        let bw = (w - 10) / 2;
        self.button(Rect::new(x0, y, bw, 48), &tr!("Give to %s", found.username.clone()), &med, mouse_pos, accent(), accent_hover(), BLACK, cb(|g| g.give_personal_event()), Bo::r(10).enabled(!busy).icon("admin_event"));
        self.button(Rect::new(x0 + bw + 10, y, bw, 48), &tr("Clear their event"), &med, mouse_pos, Color::rgb(150, 60, 60), Color::rgb(235, 90, 90), WHITE, cb(|g| g.clear_personal_event()), Bo::r(10).enabled(!busy));
        y += 48 + 10;

        if let Some((text, color)) = self.adm.msg.clone() {
            let t = small.render(&fit_text(&small, &text, w), color);
            self.canvas.blit(&t, x0, y);
        }
    }

    /// The Back button (to the admin menu) in the top corner of an admin page, next to the X.
    pub fn draw_back_button(&mut self, rect: Rect, mouse_pos: (f64, f64)) {
        let sb = self.f.small_b.clone();
        self.button(Rect::new(rect.right() - 46 - 96, rect.y + 20, 88, 28), &tr("Back"), &sb, mouse_pos, panel_light(), panel_lighter(), WHITE, cb(|g| g.back_to_admin_menu()), Bo::r(8));
    }

    pub fn draw_ban_admin(&mut self, mouse_pos: (f64, f64)) {
        if self.adm.page == "reset" {
            self.draw_reset_admin(mouse_pos);
            return;
        }
        if self.adm.page == "log" {
            self.draw_admin_log(mouse_pos);
            return;
        }
        if self.adm.page == "personal" {
            self.draw_personal_admin(mouse_pos);
            return;
        }
        let ov = dim_overlay(self.vw, VIRTUAL_H, 170);
        self.canvas.blit(&ov, 0, 0);
        let panel_w = 640.min(self.vw - 40);
        let panel_h = (VIRTUAL_H - 40).min(740);
        let rect = Rect::new(self.vw / 2 - panel_w / 2, 20.max(VIRTUAL_H / 2 - panel_h / 2), panel_w, panel_h);
        draw_panel(&mut self.canvas, rect, Some(panel()), 16, true, None);
        self.register_button(rect, Rc::new(|g: &mut Game| g.set_ban_focus(None)), None);

        let sb = self.f.small_b.clone();
        let small = self.f.small.clone();
        let med = self.f.med.clone();
        let title = self.f.big.render(&tr("Bans"), WHITE);
        self.canvas.blit(&title, rect.x + 24, rect.y + 18);
        self.button(Rect::new(rect.right() - 46, rect.y + 20, 28, 28), "X", &sb, mouse_pos, panel_light(), BAD, WHITE, cb(|g| g.close_ban_admin()), Bo::r(8));
        self.draw_back_button(rect, mouse_pos);

        let (x0, w) = (rect.x + 24, panel_w - 48);
        let mut y = rect.y + 30 + title.h;

        // ---- 1. look the player up ----
        let t = sb.render(&tr("Search player"), grey_dim());
        self.canvas.blit(&t, x0, y);
        y += t.h + 6;
        let field = Rect::new(x0, y, w - 130, 44);
        let display = self.adm.search.text.clone();
        let focused = self.adm.focus == Some("search");
        self.draw_text_field(field, FieldRef::BanSearch, &display, &tr("Username..."), focused, Rc::new(|g: &mut Game| g.set_ban_focus(Some("search"))));
        let busy = self.adm.busy;
        self.button(Rect::new(field.right() + 10, y, 120, 44), &tr("Search"), &med, mouse_pos, accent(), accent_hover(), BLACK, cb(|g| g.search_ban_player()), Bo::r(10).enabled(!busy));
        y += 44 + 10;

        // ---- 2. who was found + reason + Ban ----
        if let Some(found) = self.adm.found.clone() {
            let already = self.is_banned_uid(&found.uid);
            let mut info = tr!("Found: %s", found.username.clone());
            if already {
                info += &format!("  -  {}", tr("already banned"));
            }
            let t = sb.render(&fit_text(&sb, &info, w), if already { BAD } else { accent() });
            self.canvas.blit(&t, x0, y);
            y += t.h + 6;
            let field = Rect::new(x0, y, w, 44);
            let display = self.adm.reason.text.clone();
            let focused = self.adm.focus == Some("reason");
            self.draw_text_field(field, FieldRef::BanReason, &display, &tr("Reason for the ban..."), focused, Rc::new(|g: &mut Game| g.set_ban_focus(Some("reason"))));
            y += 44 + 8;
            let label = if self.adm.confirm { tr!("Are you sure? Ban %s", found.username.clone()) } else { tr!("Ban %s", found.username.clone()) };
            let base = if self.adm.confirm { BAD } else { Color::rgb(150, 60, 60) };
            self.button(Rect::new(x0, y, w, 44), &label, &med, mouse_pos, base, Color::rgb(235, 90, 90), WHITE, cb(|g| g.press_ban()), Bo::r(10).enabled(!busy).icon("ban"));
            y += 44 + 8;
        } else {
            let t = small.render(&tr("Search a username to ban that player."), grey());
            self.canvas.blit(&t, x0, y);
            y += t.h + 8;
        }

        if let Some((text, color)) = self.adm.msg.clone() {
            let t = small.render(&fit_text(&small, &text, w), color);
            self.canvas.blit(&t, x0, y);
        }
        y += small.get_height() + 10;

        // ---- 3. the banned list ----
        draw::line(&mut self.canvas, panel_light(), (x0, y), (x0 + w, y), 2);
        y += 8;
        let head = sb.render(&tr!("Banned players (%d)", self.adm.list.len() as i64), grey_dim());
        self.canvas.blit(&head, x0, y + 6);
        let loading = self.adm.list_loading;
        self.button(Rect::new(x0 + w - 110, y, 110, 30), &tr("Refresh"), &sb, mouse_pos, panel_light(), panel_lighter(), WHITE, cb(|g| g.refresh_ban_list()), Bo::r(8).enabled(!loading));
        y += 38;
        let list_rect = Rect::new(x0, y, w, rect.bottom() - 20 - y);
        self.adm.list_rect = list_rect;
        self.draw_ban_list(list_rect, mouse_pos);
    }

    /// v4.0.1: the audit log - every season reset, player reset, ban and unban, who did it and when. Read-only.
    fn draw_admin_log(&mut self, mouse_pos: (f64, f64)) {
        let ov = dim_overlay(self.vw, VIRTUAL_H, 170);
        self.canvas.blit(&ov, 0, 0);
        let panel_w = 640.min(self.vw - 40);
        let panel_h = (VIRTUAL_H - 40).min(740);
        let rect = Rect::new(self.vw / 2 - panel_w / 2, 20.max(VIRTUAL_H / 2 - panel_h / 2), panel_w, panel_h);
        draw_panel(&mut self.canvas, rect, Some(panel()), 16, true, None);
        self.register_blocker(rect);
        let sb = self.f.small_b.clone();
        let small = self.f.small.clone();
        let title = self.f.big.render(&tr("Log"), WHITE);
        self.canvas.blit(&title, rect.x + 24, rect.y + 18);
        self.button(Rect::new(rect.right() - 46, rect.y + 20, 28, 28), "X", &sb, mouse_pos, panel_light(), BAD, WHITE, cb(|g| g.close_ban_admin()), Bo::r(8));
        self.draw_back_button(rect, mouse_pos);
        let (x0, w) = (rect.x + 24, panel_w - 48);
        let mut y = rect.y + 30 + title.h;
        for line in wrap_text(&tr("Season and player resets, bans and unbans - who did them and when. The newest 300."), &small, w) {
            let l = small.render(&line, grey());
            self.canvas.blit(&l, x0, y);
            y += l.h + 2;
        }
        y += 10;
        draw::line(&mut self.canvas, panel_light(), (x0, y), (x0 + w, y), 2);
        y += 8;
        let head = sb.render(&tr!("Entries (%d)", self.adm.log.len() as i64), grey_dim());
        self.canvas.blit(&head, x0, y + 6);
        let loading = self.adm.log_loading;
        self.button(Rect::new(x0 + w - 110, y, 110, 30), &tr("Refresh"), &sb, mouse_pos, panel_light(), panel_lighter(), WHITE, cb(|g| g.refresh_admin_log()), Bo::r(8).enabled(!loading));
        y += 38;
        let list_rect = Rect::new(x0, y, w, rect.bottom() - 20 - y);
        self.adm.list_rect = list_rect;
        self.draw_admin_log_list(list_rect, mouse_pos);
    }

    fn draw_admin_log_list(&mut self, rect: Rect, mouse_pos: (f64, f64)) {
        let small = self.f.small.clone();
        if self.adm.log.is_empty() {
            let msg = if self.adm.log_loading || !self.adm.log_loaded { tr("Loading...") } else { tr("Nothing logged yet.") };
            let t = small.render(&msg, grey());
            let r = Rect::with_center(t.w, t.h, rect.center());
            self.canvas.blit(&t, r.x, r.y);
            self.adm.max_scroll = 0.0;
            return;
        }
        let (row_h, gap) = (56, 6);
        let content_h = self.adm.log.len() as i32 * (row_h + gap);
        self.adm.max_scroll = (content_h - rect.h).max(0) as f64;
        self.adm.scroll = self.adm.scroll.clamp(0.0, self.adm.max_scroll);
        let (sb, tiny) = (self.f.small_b.clone(), self.f.tiny.clone());
        self.push_clip(rect);
        let mut y = rect.y - self.adm.scroll as i32;
        for entry in &self.adm.log {
            let row = Rect::new(rect.x, y, rect.w - 16, row_h);
            y += row_h + gap;
            if row.bottom() < rect.top() || row.top() > rect.bottom() {
                continue;
            }
            draw::rect(&mut self.canvas, panel_light(), row, 0, 10);
            draw::rect(&mut self.canvas, outline(), row, 2, 10);
            let when = entry
                .at
                .filter(|a| *a != 0.0)
                .and_then(|at| {
                    use chrono::TimeZone;
                    chrono::Local.timestamp_opt(at as i64, 0).single()
                })
                .map(|d| d.format("%Y-%m-%d %H:%M").to_string())
                .unwrap_or_else(|| tr("(unknown time)"));
            let head_line = format!("{}  -  {}  -  {}", entry.action, entry.who, when);
            let t = sb.render(&fit_text(&sb, &head_line, row.w - 24), WHITE);
            self.canvas.blit(&t, row.x + 12, row.y + 6);
            let d = tiny.render(&fit_text(&tiny, &entry.detail, row.w - 24), grey());
            self.canvas.blit(&d, row.x + 12, row.y + 8 + t.h);
        }
        self.pop_clip();
        self.draw_scrollbar(rect, self.adm.scroll, content_h as f64, None, Some(mouse_pos));
    }

    fn draw_ban_list(&mut self, rect: Rect, mouse_pos: (f64, f64)) {
        let small = self.f.small.clone();
        if self.adm.list.is_empty() {
            let msg = if self.adm.list_loading || !self.adm.list_loaded { tr("Loading...") } else { tr("Nobody is banned.") };
            let t = small.render(&msg, grey());
            let r = Rect::with_center(t.w, t.h, rect.center());
            self.canvas.blit(&t, r.x, r.y);
            self.adm.max_scroll = 0.0;
            return;
        }
        let (row_h, gap) = (62, 6);
        let content_h = self.adm.list.len() as i32 * (row_h + gap);
        self.adm.max_scroll = (content_h - rect.h).max(0) as f64;
        self.adm.scroll = self.adm.scroll.clamp(0.0, self.adm.max_scroll);
        let (sb, med, tiny) = (self.f.small_b.clone(), self.f.med.clone(), self.f.tiny.clone());
        let busy = self.adm.busy;
        self.push_clip(rect);
        let mut y = rect.y - self.adm.scroll as i32;
        for entry in self.adm.list.clone() {
            let row = Rect::new(rect.x, y, rect.w - 16, row_h);
            y += row_h + gap;
            if row.bottom() < rect.top() || row.top() > rect.bottom() {
                continue;
            }
            draw::rect(&mut self.canvas, panel_light(), row, 0, 10);
            draw::rect(&mut self.canvas, outline(), row, 2, 10);
            let btn = Rect::new(row.right() - 110, row.centery() - 16, 100, 32);
            let e2 = entry.clone();
            self.button(btn, &tr("Unban"), &sb, mouse_pos, accent(), accent_hover(), BLACK, cb(move |g| g.unban(e2.clone())), Bo::r(8).enabled(!busy));
            let text_w = btn.x - row.x - 24;
            let name = med.render(&fit_text(&med, &entry.username, text_w), WHITE);
            self.canvas.blit(&name, row.x + 12, row.y + 6);
            let mut detail = if entry.reason.is_empty() { tr("(no reason)") } else { entry.reason.clone() };
            if !entry.by.is_empty() {
                detail = tr!("%s  -  by %s", detail, entry.by);
            }
            let d = tiny.render(&fit_text(&tiny, &detail, text_w), grey());
            self.canvas.blit(&d, row.x + 12, row.y + 8 + name.h);
        }
        self.pop_clip();
        self.draw_scrollbar(rect, self.adm.scroll, content_h as f64, None, Some(mouse_pos));
    }

    /// On top of everything, with nothing clickable underneath: the account is banned.
    pub fn draw_ban_screen(&mut self, mouse_pos: (f64, f64)) {
        self.buttons.clear();
        self.scrollbar_hits.clear();
        let mut overlay = Surface::new_alpha(self.vw, VIRTUAL_H);
        overlay.fill(Color::rgba(0, 0, 0, 215), None);
        self.canvas.blit(&overlay, 0, 0);

        let ban = self.adm.info.clone();
        let med = self.f.med.clone();
        let panel_w = 560.min(self.vw - 40);
        let reason = ban.as_ref().map(|b| b.reason.clone()).filter(|r| !r.is_empty()).unwrap_or_else(|| tr("(no reason)"));
        let reason_lines: Vec<String> = wrap_text(&reason, &med, panel_w - 48).into_iter().take(6).collect();
        let panel_h = 250 + reason_lines.len() as i32 * (med.get_height() + 2);
        let rect = Rect::new(self.vw / 2 - panel_w / 2, VIRTUAL_H / 2 - panel_h / 2, panel_w, panel_h);
        draw_panel(&mut self.canvas, rect, Some(panel()), 16, true, None);
        self.register_blocker(rect);

        let x0 = rect.x + 24;
        let mut y = rect.y + 20;
        let title = self.f.big.render(&tr("You are banned"), BAD);
        self.canvas.blit(&title, x0, y);
        y += title.h + 6;
        let who = self.account.as_ref().map(|a| a.username.clone()).unwrap_or_default();
        let mut sub = tr!("The account %s can't play right now.", who);
        if let Some(at) = ban.as_ref().and_then(|b| b.at).filter(|a| *a != 0.0) {
            use chrono::TimeZone;
            if let Some(d) = chrono::Local.timestamp_opt(at as i64, 0).single() {
                sub += &format!("  {}", tr!("(since %s)", d.format("%Y-%m-%d").to_string()));
            }
        }
        let small = self.f.small.clone();
        let t = small.render(&fit_text(&small, &sub, panel_w - 48), grey());
        self.canvas.blit(&t, x0, y);
        y += t.h + 14;
        let t = self.f.small_b.render(&tr("Reason:"), grey_dim());
        self.canvas.blit(&t, x0, y);
        y += t.h + 4;
        for line in &reason_lines {
            let lt = med.render(line, WHITE);
            self.canvas.blit(&lt, x0, y);
            y += med.get_height() + 2;
        }

        let btn_y = rect.bottom() - 24 - 46;
        let half = (panel_w - 48 - 10) / 2;
        self.button(Rect::new(x0, btn_y, half, 46), &tr("Log out"), &med, mouse_pos, panel_light(), panel_lighter(), WHITE, cb(|g| g.ban_log_out()), Bo::r(10));
        self.button(Rect::new(x0 + half + 10, btn_y, half, 46), &tr("Quit Game"), &med, mouse_pos, Color::rgb(150, 60, 60), BAD, WHITE, cb(|g| g.quit_game()), Bo::r(10));
        let _ = now_ts;
    }
}
