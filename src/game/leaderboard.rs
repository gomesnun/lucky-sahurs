//! Leaderboard page (ui/leaderboard_panel.py).

use super::base::Bo;
use super::{Game, cb};
use crate::config::{TOPBAR_H, VIRTUAL_H};
use crate::core::formatting::{format_number, format_playtime};
use crate::core::state::now_ts;
use crate::gfx::{Color, Rect, draw, ti};
use crate::i18n::{tr, tr_short};
use crate::online::cloud_cache::{fetch_leaderboards, lb_data_complete, lb_fetch_period, lb_next_update, store_lb_cache};
use crate::online::firebase::{LEADERBOARD_SIZE, LEADERBOARD_SNAPSHOT_PERIOD, online_error_text};
use crate::storage::value_f64;
use crate::theme::*;
use crate::tr;
use crate::ui::drawing::{dim_overlay, draw_panel};
use crate::ui::fonts::{fit_text, wrap_text};
use crate::ui::icons::load_icon;
use serde_json::Value;
use std::rc::Rc;

const LB_TABS: [(&str, &str, &str, &str); 4] = [
    ("money", "Money", "Total coins earned", "No scores yet - be the first!"),
    ("playtime", "Playtime", "Total playtime", "No scores yet - be the first!"),
    ("rolls", "Rolls", "Total rolls", "No scores yet - be the first!"),
    ("rebirths", "Rebirths", "Rebirths", "Nobody has rebirthed yet - be the first!"),
];

fn lb_icon(tab: &str) -> &'static str {
    match tab {
        "money" => "cash",
        "playtime" => "clock",
        "rolls" => "dice",
        "rebirths" => "rebirth",
        _ => "",
    }
}
const LB_VALUE_ICON: i32 = 30;

fn entry_name(e: &Value) -> String {
    e.get("username").and_then(|v| v.as_str()).unwrap_or("").to_string()
}

fn entry_value(e: &Value) -> f64 {
    e.get("value").and_then(value_f64).unwrap_or(0.0)
}

impl Game {
    pub fn open_leaderboard(&mut self) {
        self.close_overlays();
        self.leaderboard_open = true;
        self.lb_scroll = 0.0;
        self.ensure_leaderboard(false);
    }

    pub fn close_leaderboard(&mut self) {
        self.leaderboard_open = false;
    }

    pub fn toggle_leaderboard(&mut self) {
        if self.leaderboard_open {
            self.close_leaderboard();
        } else {
            self.open_leaderboard();
        }
    }

    pub fn set_lb_tab(&mut self, tab: &'static str) {
        self.lb_tab = tab;
        self.lb_scroll = 0.0;
    }

    pub fn lb_login(&mut self) {
        self.leaderboard_open = false;
        self.open_account();
    }

    pub fn lb_retry(&mut self) {
        self.ensure_leaderboard(true);
    }

    pub fn ensure_leaderboard(&mut self, force: bool) {
        let Some(client) = self.client.clone() else { return };
        if self.lb_loading {
            return;
        }
        let now = now_ts();
        let period = lb_fetch_period(now);
        if !force {
            let mut fresh = match &self.lb_data {
                Some(d) if d.as_object().is_some_and(|o| !o.is_empty()) => {
                    d.get("period").and_then(value_f64).unwrap_or(-1.0) >= period as f64 && lb_data_complete(d)
                }
                _ => false,
            };
            if fresh && self.account.is_some() && self.pub_last_time != self.lb_refetch_done {
                let taken = self.lb_data.as_ref().and_then(|d| d.get("fetched_at")).and_then(value_f64).unwrap_or(0.0);
                if self.pub_last_time > taken {
                    fresh = false;
                }
            }
            if fresh {
                return;
            }
            if now < self.lb_retry_at {
                return;
            }
        }
        self.lb_refetch_done = self.pub_last_time;
        self.lb_loading = true;
        self.lb_error = None;
        self.run_job(
            move || fetch_leaderboards(&client, period),
            |g, res| {
                g.lb_loading = false;
                store_lb_cache(&res);
                g.lb_data = Some(res);
                g.lb_retry_at = 0.0;
            },
            |g, e| {
                g.lb_loading = false;
                g.lb_error = Some(online_error_text(&e));
                g.lb_retry_at = f64::INFINITY;
            },
        );
    }

    fn lb_format(&self, value: f64) -> String {
        match self.lb_tab {
            "money" => format!("${}", format_number(value)),
            "playtime" => format_playtime(value),
            _ => format_number(value),
        }
    }

    fn lb_tab_info(&self) -> (&'static str, &'static str, &'static str, &'static str) {
        LB_TABS.iter().find(|t| t.0 == self.lb_tab).copied().unwrap_or(LB_TABS[0])
    }

    pub fn draw_leaderboard(&mut self, mouse_pos: (f64, f64)) {
        self.ensure_leaderboard(false);
        let ov = dim_overlay(self.vw, VIRTUAL_H, 170);
        self.canvas.blit(&ov, 0, 0);

        let panel_w = 640;
        let top = TOPBAR_H + 12;
        let panel_h = VIRTUAL_H - top - 14;
        let rect = Rect::new(self.vw / 2 - panel_w / 2, top, panel_w, panel_h);
        draw_panel(&mut self.canvas, rect, Some(panel()), 16, true, None);
        self.register_button(rect, Rc::new(|_: &mut Game| {}), None);

        let sb = self.f.small_b.clone();
        let small = self.f.small.clone();
        let med = self.f.med.clone();
        let tiny = self.f.tiny.clone();
        let title = self.f.big.render(&tr("Leaderboard"), WHITE);
        self.canvas.blit(&title, rect.x + 26, rect.y + 16);
        let close_rect = Rect::new(rect.right() - 48, rect.y + 18, 30, 30);
        self.button(close_rect, "X", &sb, mouse_pos, panel_light(), BAD, WHITE, cb(|g| g.close_leaderboard()), Bo::r(8));
        let sub_y = rect.y + 16 + title.h + 2;
        let sub = fit_text(&small, &tr!("Top %d players - updates every %d minutes.", LEADERBOARD_SIZE as i64, (LEADERBOARD_SNAPSHOT_PERIOD / 60.0).floor() as i64), panel_w - 52 - 60);
        let s = small.render(&sub, grey());
        self.canvas.blit(&s, rect.x + 26, sub_y);

        let tabs_y = sub_y + small.get_height() + 12;
        let tab_gap = 10;
        let n = LB_TABS.len() as i32;
        let tab_w = (panel_w - 44 - tab_gap * (n - 1)) / n;
        for (i, (key, label, _, _)) in LB_TABS.iter().enumerate() {
            let label = tr_short(label);
            let trect = Rect::new(rect.x + 22 + i as i32 * (tab_w + tab_gap), tabs_y, tab_w, 40);
            let active = self.lb_tab == *key;
            let tab_font = if med.size(&label).0 + 8 + (trect.h - 12) <= trect.w - 16 { med.clone() } else { sb.clone() };
            let k: &'static str = key;
            self.button(
                trect,
                &label,
                &tab_font,
                mouse_pos,
                if active { accent() } else { panel_light() },
                if active { accent_hover() } else { panel_lighter() },
                if active { BLACK } else { WHITE },
                cb(move |g| g.set_lb_tab(k)),
                Bo::r(10).icon(lb_icon(k)),
            );
        }

        let head_y = tabs_y + 40 + 14;
        let value_label = tr(self.lb_tab_info().2);
        let t = sb.render("#", grey_dim());
        self.canvas.blit(&t, rect.x + 34, head_y);
        let t = sb.render(&tr("Player"), grey_dim());
        self.canvas.blit(&t, rect.x + 96, head_y);
        let vh = sb.render(&value_label, grey_dim());
        self.canvas.blit(&vh, rect.right() - 38 - vh.w, head_y);
        let line_y = head_y + sb.get_height() + 4;
        draw::line(&mut self.canvas, panel_light(), (rect.x + 22, line_y), (rect.right() - 22, line_y), 2);

        let back_rect = Rect::new(rect.x + 22, rect.bottom() - 22 - 40, 210, 40);
        self.button(back_rect, &tr("Back to Options"), &sb, mouse_pos, panel_light(), panel_lighter(), WHITE, cb(|g| g.open_options()), Bo::r(10));
        if self.lb_error.is_some() {
            self.button(Rect::new(back_rect.right() + 10, back_rect.y, 84, 40), &tr("Retry"), &sb, mouse_pos, accent(), accent_hover(), BLACK, cb(|g| g.lb_retry()), Bo::r(10));
        }
        let now = now_ts();
        let remaining = 0.max((lb_next_update(now) - now) as i64);
        let (line1, line2) = if self.lb_loading {
            (tr("Updating..."), String::new())
        } else if let Some(e) = &self.lb_error {
            (tr("Couldn't update."), fit_text(&tiny, e, 250))
        } else {
            let fetched = self.lb_data.as_ref().and_then(|d| d.get("fetched_at")).and_then(value_f64).filter(|v| *v != 0.0);
            let l1 = match fetched {
                Some(ts) => {
                    use chrono::TimeZone;
                    let dt = chrono::Local.timestamp_opt(ts as i64, 0).single();
                    tr!("Updated %s", dt.map(|d| d.format("%H:%M").to_string()).unwrap_or_default())
                }
                None => String::new(),
            };
            (l1, tr!("Next update in %d:%02d", remaining / 60, remaining % 60))
        };
        let mut fy = back_rect.y + 2;
        for (text, font, color) in [(line1, &small, grey()), (line2, &tiny, grey_dim())] {
            if !text.is_empty() {
                let t = font.render(&text, color);
                self.canvas.blit(&t, rect.right() - 26 - t.w, fy);
            }
            fy += 19;
        }

        let own_rect = Rect::new(rect.x + 22, back_rect.y - 10 - 44, panel_w - 44, 44);
        let entries: Vec<Value> = self.lb_data.as_ref().and_then(|d| d.get(self.lb_tab)).and_then(|v| v.as_array()).cloned().unwrap_or_default();
        let own_name = self.account.as_ref().map(|a| a.username.clone());
        self.draw_lb_own_row(own_rect, &entries, own_name.as_deref(), mouse_pos);

        let list_rect = Rect::new(rect.x + 22, line_y + 8, panel_w - 44, own_rect.y - 10 - (line_y + 8));
        self.lb_list_rect = list_rect;
        if self.client.is_none() {
            self.draw_lb_message(list_rect, &tr("Online features aren't set up yet - see FIREBASE_SETUP.md."));
        } else if entries.is_empty() {
            let msg = if self.lb_loading {
                tr("Loading leaderboard...")
            } else if self.lb_error.is_some() {
                tr("The leaderboard couldn't be loaded.")
            } else {
                tr(self.lb_tab_info().3)
            };
            self.draw_lb_message(list_rect, &msg);
        } else {
            self.draw_lb_rows(list_rect, &entries, own_name.as_deref(), mouse_pos);
        }
    }

    fn draw_lb_message(&mut self, rect: Rect, text: &str) {
        self.lb_max_scroll = 0.0;
        let mut y = rect.centery() - 20;
        let med = self.f.med.clone();
        for line in wrap_text(text, &med, rect.w - 60) {
            let t = med.render(&line, grey());
            let r = Rect::with_center(t.w, t.h, (rect.centerx(), y));
            self.canvas.blit(&t, r.x, r.y);
            y += 26;
        }
    }

    fn draw_lb_rows(&mut self, rect: Rect, entries: &[Value], own_name: Option<&str>, mouse_pos: (f64, f64)) {
        let (row_h, gap) = (44, 4);
        let row_w = rect.w - 16;
        let scroll = self.lb_scroll;
        let medal = |rank: usize| match rank {
            1 => Color::rgb(255, 205, 60),
            2 => Color::rgb(214, 218, 228),
            3 => Color::rgb(214, 148, 96),
            _ => WHITE,
        };
        let med = self.f.med.clone();
        self.push_clip(rect);
        let mut y = rect.top() as f64 - scroll;
        for (i, e) in entries.iter().enumerate() {
            let rrect = Rect::new(rect.x, ti(y), row_w, row_h);
            if rrect.bottom() >= rect.top() - 4 && rrect.top() <= rect.bottom() + 4 {
                let uname = entry_name(e);
                let is_own = own_name.is_some_and(|o| o == uname);
                draw::rect(&mut self.canvas, if is_own { panel_lighter() } else { panel_light() }, rrect, 0, 10);
                if is_own {
                    draw::rect(&mut self.canvas, accent(), rrect, 3, 10);
                }
                let rank = i + 1;
                let rt = med.render(&rank.to_string(), medal(rank));
                self.canvas.blit(&rt, rrect.x + 14, rrect.centery() - rt.h / 2);
                let value = med.render(&self.lb_format(entry_value(e)), if self.lb_tab == "money" { GOOD } else { WHITE });
                self.canvas.blit(&value, rrect.right() - 16 - value.w, rrect.centery() - value.h / 2);
                let icon_w = self.draw_lb_value_icon(rrect.right() - 16 - value.w - 6, rrect.centery(), e);
                let name = fit_text(&med, &uname, rrect.w - 74 - value.w - 40 - icon_w);
                let nt = med.render(&name, if is_own { accent() } else { WHITE });
                self.canvas.blit(&nt, rrect.x + 74, rrect.centery() - nt.h / 2);
            }
            y += (row_h + gap) as f64;
        }
        let content_h = (entries.len() as i32 * (row_h + gap) - gap) as f64;
        self.lb_max_scroll = (content_h - rect.h as f64).max(0.0);
        self.lb_scroll = self.lb_scroll.min(self.lb_max_scroll).max(0.0);
        self.pop_clip();
        self.draw_scrollbar(rect, scroll, content_h, Some("leaderboard"), Some(mouse_pos));
    }

    /// The tab's icon left of the value and, on Rebirths, the player's Prestige pill left of that. Returns the width.
    fn draw_lb_value_icon(&mut self, right: i32, cy: i32, e: &Value) -> i32 {
        let mut w = 0;
        if let Some(img) = load_icon(lb_icon(self.lb_tab), LB_VALUE_ICON) {
            let r = Rect::with_midright(img.w, img.h, (right, cy));
            self.canvas.blit(&img, r.x, r.y);
            w += img.w + 6;
        }
        let prestige = e.get("prestige").and_then(crate::storage::value_i64).unwrap_or(0);
        if self.lb_tab == "rebirths" && prestige > 0 {
            w += self.draw_prestige_pill(right - w, cy, prestige) + 8;
        }
        w
    }

    fn draw_lb_own_row(&mut self, rect: Rect, entries: &[Value], own_name: Option<&str>, mouse_pos: (f64, f64)) {
        draw::rect(&mut self.canvas, panel_light(), rect, 0, 10);
        let small = self.f.small.clone();
        let med = self.f.med.clone();
        let Some(own_name) = own_name else {
            let can_login = self.client.is_some() && (self.screen_mode == "title" || self.screen_mode == "saves");
            let text = if can_login { tr("Log in to appear on the leaderboard.") } else { tr("Log in from the main menu to appear on the leaderboard.") };
            let t = small.render(&fit_text(&small, &text, rect.w - if can_login { 150 } else { 30 }), grey());
            self.canvas.blit(&t, rect.x + 16, rect.centery() - t.h / 2);
            if can_login {
                let sb = self.f.small_b.clone();
                self.button(Rect::new(rect.right() - 122, rect.y + 6, 112, rect.h - 12), &tr("Log in"), &sb, mouse_pos, accent(), accent_hover(), BLACK, cb(|g| g.lb_login()), Bo::r(8));
            }
            return;
        };
        for (i, e) in entries.iter().enumerate() {
            if entry_name(e) == own_name {
                draw::rect(&mut self.canvas, accent(), rect, 3, 10);
                let head = med.render(&tr!("You: #%d", i as i64 + 1), accent());
                self.canvas.blit(&head, rect.x + 16, rect.centery() - head.h / 2);
                let val = med.render(&self.lb_format(entry_value(e)), if self.lb_tab == "money" { GOOD } else { WHITE });
                self.canvas.blit(&val, rect.right() - 16 - val.w, rect.centery() - val.h / 2);
                self.draw_lb_value_icon(rect.right() - 16 - val.w - 6, rect.centery(), e);
                return;
            }
        }
        let text = if self.lb_tab == "rebirths" { tr("You: not on this board yet - do a Rebirth!") } else { tr!("You: not in the top %d yet - keep playing!", LEADERBOARD_SIZE as i64) };
        let t = small.render(&fit_text(&small, &text, rect.w - 30), grey());
        self.canvas.blit(&t, rect.x + 16, rect.centery() - t.h / 2);
    }
}
