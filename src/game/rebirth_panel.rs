//! Rebirth page (ui/rebirth_panel.py).

use super::base::Bo;
use super::{Game, cb};
use crate::config::{TOPBAR_H, VIRTUAL_H};
use crate::core::data::{REBIRTH_LUCK_PER, REBIRTH_MONEY_PER, REBIRTH_REWARDS, rebirth_reward_label};
use crate::core::formatting::format_number;
use crate::gfx::{Color, Rect, ti};
use crate::i18n::tr;
use crate::pyfmt::format as pyformat;
use crate::theme::*;
use super::prestige_panel::PRESTIGE_COLOR;
use crate::ui::drawing::{dim_overlay, draw_panel, draw_state_border};
use crate::ui::fonts::{fit_text, wrap_text};
use crate::{args, tr};
use std::rc::Rc;

const REWARD_ROW_H: i32 = 68;
const TAB_W: i32 = 150;
const TAB_H: i32 = 56;
const REWARD_GAP: i32 = 8;

/// core/rebirths.format_rebirth_reward
pub fn format_rebirth_reward(rewards: &[(&str, f64, bool)]) -> String {
    let mut parts = Vec::new();
    for &(kind, value, is_int) in rewards {
        match kind {
            "keep_upgrades" => parts.push(tr("Rebirth only resets your coins from now on")),
            "slots" => {
                let v = value as i64;
                parts.push(if v == 1 { tr!("+%d Equip Slot", v) } else { tr!("+%d Equip Slots", v) });
            }
            "offline_time" => parts.push(if is_int { tr!("+%g hours max offline time", value as i64) } else { tr!("+%g hours max offline time", value) }),
            _ => parts.push(tr!("+%.0f%% %s", value * 100.0, tr(rebirth_reward_label(kind)))),
        }
    }
    parts.join(", ")
}

impl Game {
    pub fn close_rebirth(&mut self) {
        self.rebirth_open = false;
        self.rebirth_confirm = false;
        self.prestige_picking = false;
    }

    /// The two tabs on the panel's right side: Rebirth (top) and Prestige (below). The one open is joined to the panel.
    fn draw_rebirth_tabs(&mut self, rect: Rect, tab_w: i32, mouse_pos: (f64, f64)) {
        let sb = self.f.small_b.clone();
        let tabs: [(&'static str, String, &'static str, Color); 2] = [
            ("rebirth", tr("Rebirth"), "rebirth", accent()),
            ("prestige", tr("Prestige"), "prestige", PRESTIGE_COLOR),
        ];
        for (i, (key, label, icon, color)) in tabs.into_iter().enumerate() {
            let active = self.rebirth_tab == key;
            // the active tab tucks under the panel's edge so it looks attached
            let r = Rect::new(rect.right() - if active { 16 } else { 8 }, rect.y + 96 + i as i32 * (TAB_H + 10), tab_w + if active { 16 } else { 8 }, TAB_H);
            let alert = key == "prestige" && self.state.prestige_available();
            self.button(
                r,
                &label,
                &sb,
                mouse_pos,
                if active { panel() } else { panel_light() },
                panel_lighter(),
                if active { color } else { WHITE },
                cb(move |g| g.set_rebirth_tab(key)),
                Bo::r(12).icon(icon).border(if active { Some(color) } else if alert { Some(PRESTIGE_COLOR) } else { None }),
            );
        }
    }

    pub fn toggle_rebirth(&mut self) {
        if self.rebirth_open {
            self.close_rebirth();
        } else {
            self.close_overlays();
            self.rebirth_open = true;
            self.rebirth_confirm = false;
            self.right_panel.close();
            self.left_panel.close();
            let nxt = self.state.next_rebirth_reward();
            self.rebirth_scroll = match nxt {
                None => 0.0,
                Some(n) => (n as i64 - 1).max(0) as f64 * (REWARD_ROW_H + REWARD_GAP) as f64,
            };
        }
    }

    pub fn do_rebirth_clicked(&mut self) {
        if !self.state.rebirth_available() {
            return;
        }
        if !self.rebirth_confirm {
            self.rebirth_confirm = true;
            self.rebirth_confirm_timer = 4.0;
            return;
        }
        if self.state.do_rebirth() {
            self.rebirth_confirm = false;
            self.play("rebirth", 0.0);
            let rb = self.state.rebirths;
            let mut msg = tr!("Rebirth #%d! Permanent +%.0f%% Money, +%.0f%% Luck.", rb, REBIRTH_MONEY_PER * 100.0, REBIRTH_LUCK_PER * 100.0);
            let reward_lines: Vec<String> = REBIRTH_REWARDS.iter().filter(|r| r.need == rb).map(|r| tr!("Reward unlocked: %s - %s", tr(r.name), format_rebirth_reward(r.rewards))).collect();
            if !reward_lines.is_empty() {
                msg += "\n";
                msg += &reward_lines.join("\n");
            }
            self.show_toast(&msg, if reward_lines.is_empty() { 1.8 } else { 4.5 });
        }
    }

    pub fn draw_rebirth_page(&mut self, mouse_pos: (f64, f64)) {
        let ov = dim_overlay(self.vw, VIRTUAL_H, 170);
        self.canvas.blit(&ov, 0, 0);

        let panel_w = 560.max((self.vw - 400).min(760));
        let top = TOPBAR_H + 12;
        let panel_h = VIRTUAL_H - top - 14;
        // v3.0: the Rebirth / Prestige tabs stick out of the panel's right side (panel + tabs centred together)
        let tab_w = TAB_W.min(((self.vw - panel_w) / 2 - 10).max(90) * 2 - 20);
        let rect = Rect::new((self.vw - panel_w - tab_w) / 2, top, panel_w, panel_h);
        self.draw_rebirth_tabs(rect, tab_w, mouse_pos);
        draw_panel(&mut self.canvas, rect, Some(panel()), 16, true, None);
        self.register_button(rect, Rc::new(|_: &mut Game| {}), None);

        let prestige_tab = self.rebirth_tab == "prestige";
        let title = self.f.big.render(&if prestige_tab { tr("Prestige") } else { tr("Rebirth") }, if prestige_tab { PRESTIGE_COLOR } else { WHITE });
        self.canvas.blit(&title, rect.x + 26, rect.y + 16);
        let close_rect = Rect::new(rect.right() - 48, rect.y + 18, 30, 30);
        let sb = self.f.small_b.clone();
        let small = self.f.small.clone();
        let med = self.f.med.clone();
        let tiny = self.f.tiny.clone();
        self.button(close_rect, "X", &sb, mouse_pos, panel_light(), BAD, WHITE, cb(|g| g.close_rebirth()), Bo::r(8));
        if prestige_tab {
            self.draw_prestige_body(rect, rect.y + 16 + title.h + 2, mouse_pos);
            return;
        }

        let keep = self.state.rebirth_keeps_upgrades();
        let keep_all = self.state.rebirth_keeps_everything();
        let sub_y = rect.y + 16 + title.h + 2;
        let sub_text = if keep_all {
            tr("A PERMANENT boost to Money and Luck. Thanks to Prestige III a Rebirth resets nothing - you even keep your coins.")
        } else if keep {
            tr("Reset your coins for a PERMANENT boost to Money and Luck. Upgrades, pets, traits, milestones and playtime are all kept.")
        } else {
            tr("Reset your coins and upgrades for a PERMANENT boost to Money and Luck. Pets, traits, milestones and playtime are all kept.")
        };
        let mut y = sub_y;
        for line in wrap_text(&sub_text, &small, rect.w - 52) {
            let s = small.render(&line, grey());
            self.canvas.blit(&s, rect.x + 26, y);
            y += small.get_height() + 2;
        }
        y += 14;

        let card_rect = Rect::new(rect.x + 26, y, rect.w - 52, 124);
        draw_panel(&mut self.canvas, card_rect, Some(panel_light()), 12, false, None);
        let rb = self.state.rebirths;
        let lines = [
            (tr("Rebirths"), format!("{}", rb)),
            (tr("Money bonus (permanent)"), pyformat("+%.0f%%", &args![REBIRTH_MONEY_PER * rb as f64 * 100.0])),
            (tr("Luck bonus, Rare+ (permanent)"), pyformat("+%.0f%%", &args![REBIRTH_LUCK_PER * rb as f64 * 100.0])),
        ];
        let mut ly = card_rect.y + 14;
        for (label, value) in &lines {
            let l_txt = med.render(label, grey());
            let v_txt = med.render(value, GOOD);
            self.canvas.blit(&l_txt, card_rect.x + 18, ly);
            self.canvas.blit(&v_txt, card_rect.right() - 18 - v_txt.w, ly);
            ly += 36;
        }
        y = card_rect.bottom() + 22;

        let cost = self.state.rebirth_cost();
        let can = self.state.rebirth_available();
        let cost_txt = med.render(&tr!("Next rebirth costs $%s  (you have $%s)", format_number(cost), format_number(self.state.coins)), if can { WHITE } else { grey() });
        let r = Rect::with_center(cost_txt.w, cost_txt.h, (rect.centerx(), y));
        self.canvas.blit(&cost_txt, r.x, r.y);
        y += 24;

        let btn_w = 420.min(rect.w - 60);
        let (label, color) = if self.rebirth_confirm {
            (if keep_all { tr("Click again to confirm!") } else if keep { tr("Click again to confirm - resets your coins!") } else { tr("Click again to confirm - resets coins & upgrades!") }, BAD)
        } else {
            (tr!("Rebirth  (+%.0f%% Money, +%.0f%% Luck)", REBIRTH_MONEY_PER * 100.0, REBIRTH_LUCK_PER * 100.0), if can { accent() } else { Color::rgb(70, 73, 88) })
        };
        self.button(
            Rect::new(rect.centerx() - btn_w / 2, y, btn_w, 50),
            &label,
            &sb,
            mouse_pos,
            color,
            accent_hover(),
            if can { BLACK } else { grey() },
            if can { cb(|g| g.do_rebirth_clicked()) } else { None },
            Bo::r(10).enabled(can).sfx(None),
        );
        y += 50 + 18;

        let note_text = if keep_all {
            tr("Nothing resets: your coins, upgrades, pets and traits all stay.")
        } else if keep {
            tr("Only your coins reset. Everything else (upgrades, pets, traits...) stays.")
        } else {
            tr("Only coins and upgrade levels reset. Everything else (pets, traits, milestones...) stays.")
        };
        let note = tiny.render(&note_text, grey());
        let r = Rect::with_center(note.w, note.h, (rect.centerx(), y));
        self.canvas.blit(&note, r.x, r.y);
        y += 22;

        let got = REBIRTH_REWARDS.iter().filter(|r| rb >= r.need).count() as i64;
        let head = med.render(&tr!("Rebirth Rewards  (%d/%d)", got, REBIRTH_REWARDS.len() as i64), accent());
        self.canvas.blit(&head, rect.x + 26, y);
        y += head.h + 8;
        let list_rect = Rect::new(rect.x + 22, y, rect.w - 44, rect.bottom() - 16 - y);
        self.draw_rebirth_rewards(list_rect, mouse_pos);
    }

    fn draw_rebirth_rewards(&mut self, rect: Rect, mouse_pos: (f64, f64)) {
        self.rebirth_list_rect = rect;
        let nxt = self.state.next_rebirth_reward();
        let rb = self.state.rebirths;
        let content_h = (REBIRTH_REWARDS.len() as i32 * (REWARD_ROW_H + REWARD_GAP) - REWARD_GAP) as f64;
        self.rebirth_max_scroll = (content_h - rect.h as f64).max(0.0);
        self.rebirth_scroll = self.rebirth_scroll.min(self.rebirth_max_scroll).max(0.0);
        let scroll = self.rebirth_scroll;

        self.push_clip(rect);
        let row_w = rect.w - 20;
        let sb = self.f.small_b.clone();
        let small = self.f.small.clone();
        let title_h = sb.get_height();
        let desc_h = small.get_height();
        let text_top = 8.max((REWARD_ROW_H - (title_h + 3 + desc_h)).div_euclid(2));
        for (i, rw) in REBIRTH_REWARDS.iter().enumerate() {
            let row = Rect::new(rect.x, ti((rect.top() + i as i32 * (REWARD_ROW_H + REWARD_GAP)) as f64 - scroll), row_w, REWARD_ROW_H);
            if row.bottom() < rect.top() || row.top() > rect.bottom() {
                continue;
            }
            let unlocked = rb >= rw.need;
            let is_next = nxt == Some(i);
            draw_panel(&mut self.canvas, row, Some(panel_light()), 10, false, None);
            if unlocked {
                draw_state_border(&mut self.canvas, row, Color::rgb(80, 150, 100), 10, 2);
            } else if is_next {
                draw_state_border(&mut self.canvas, row, accent(), 10, 2);
            }
            let status = sb.render(&if unlocked { tr("Unlocked") } else { format!("{} / {}", rb, rw.need) }, if unlocked { GOOD } else { grey() });
            self.canvas.blit(&status, row.right() - 16 - status.w, row.y + text_top);
            let title_txt = fit_text(&sb, &tr!("Rebirth %d  -  %s", rw.need, tr(rw.name)), row.w - 44 - status.w);
            let t = sb.render(&title_txt, if unlocked || is_next { WHITE } else { grey() });
            self.canvas.blit(&t, row.x + 16, row.y + text_top);
            let reward_txt = fit_text(&small, &format_rebirth_reward(rw.rewards), row.w - 32);
            let t = small.render(&reward_txt, if unlocked { GOOD } else { grey() });
            self.canvas.blit(&t, row.x + 16, row.y + text_top + title_h + 3);
        }
        self.pop_clip();
        self.draw_scrollbar(rect, scroll, content_h, Some("rebirth"), Some(mouse_pos));
    }
}
