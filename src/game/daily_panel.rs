//! Quests panel (ui/daily_panel.py): Daily quests (reset 00:00 Lisbon) and, since v3.0, Weekly quests (reset Monday
//! 00:00 Lisbon), harder with bigger rewards. Rewards grow with what you have (GameState::quest_reward).

use super::base::Bo;
use super::{Game, cb};
use crate::core::data::mission_def;
use crate::core::formatting::format_number;
use crate::core::shop::potion_by_key;
use crate::core::state::{DailyMission, QuestReward, now_ts, secs_until_lisbon_reset};
use crate::ui::fonts::fit_text;
use crate::gfx::{Color, Rect, draw, ti};
use crate::i18n::tr;
use crate::theme::*;
use crate::tr;
use crate::ui::drawing::{draw_panel, draw_state_border};
use std::rc::Rc;

pub fn mission_label(mission: &DailyMission) -> String {
    match mission_def(mission.mtype) {
        None => "???".into(),
        Some(m) if m.label.contains('%') => tr!(m.label, mission.target),
        Some(m) => tr(m.label),
    }
}

/// "Luck Potion III"
pub fn quest_potion_text(r: &QuestReward) -> Option<String> {
    let (kind, lvl) = r.potion?;
    let name = potion_by_key(kind).map(|p| tr(p.name)).unwrap_or_default();
    Some(format!("{} {}", name, ["I", "II", "III", "IV", "V"][(lvl.clamp(1, 5) - 1) as usize]))
}

/// "$1.2M + 14 trait charges" (+ the potion when `with_potion`)
pub fn quest_reward_text(r: &QuestReward) -> String {
    quest_reward_line(r, true)
}

pub fn quest_reward_line(r: &QuestReward, with_potion: bool) -> String {
    let mut parts = Vec::new();
    if r.coins >= 1.0 {
        parts.push(format!("${}", format_number(r.coins)));
    }
    parts.push(if r.charges == 1 { tr!("%d trait charge", r.charges) } else { tr!("%s trait charges", format_number(r.charges as f64)) });
    if with_potion {
        parts.extend(quest_potion_text(r));
    }
    parts.join(" + ")
}

/// "5h 12m" / "3d 4h"
fn reset_in(secs: f64) -> String {
    let s = secs.max(0.0) as i64;
    let (d, h, m) = (s / 86400, s % 86400 / 3600, s % 3600 / 60);
    if d > 0 { format!("{}d {}h", d, h) } else if h > 0 { format!("{}h {}m", h, m) } else { format!("{}m", m.max(1)) }
}

impl Game {
    pub fn claim_daily_mission(&mut self, index: usize) {
        let weekly = self.quests_tab == "weekly";
        let reward = if weekly { self.state.claim_weekly_mission(index) } else { self.state.claim_daily_mission(index) };
        if let Some(r) = reward {
            self.play("trait_charge", 0.0);
            let head = if weekly { tr("Weekly quest claimed!") } else { tr("Daily quest claimed!") };
            self.show_toast(&format!("{} +{}", head, quest_reward_text(&r)), 2.2);
        }
    }

    /// Quests you can claim now (the alert on the side button).
    pub fn quests_claimable(&mut self) -> bool {
        self.state.ensure_daily_missions();
        self.state.ensure_weekly_missions();
        let st = &self.state;
        (0..st.daily_missions.len()).any(|i| !st.daily_claimed.contains(&i) && st.daily_mission_progress(i) >= st.daily_missions[i].target)
            || (0..st.weekly_missions.len()).any(|i| !st.weekly_claimed.contains(&i) && st.weekly_mission_progress(i) >= st.weekly_missions[i].target)
    }

    pub fn draw_daily_panel(&mut self, rect: Rect, mouse_pos: (f64, f64)) {
        self.state.ensure_daily_missions();
        self.state.ensure_weekly_missions();
        self.panel_header(rect, &tr("Quests"), mouse_pos, Rc::new(|g: &mut Game| g.right_panel.close()));
        let mut top = rect.y + 62;
        // Daily / Weekly tabs
        let sb = self.f.small_b.clone();
        let tab_w = (rect.w - 40 - 10) / 2;
        for (n, (key, label)) in [("daily", tr("Daily")), ("weekly", tr("Weekly"))].into_iter().enumerate() {
            let active = self.quests_tab == key;
            self.button(
                Rect::new(rect.x + 20 + n as i32 * (tab_w + 10), top, tab_w, 36),
                &label,
                &sb,
                mouse_pos,
                if active { accent() } else { panel_light() },
                if active { accent() } else { panel_lighter() },
                if active { BLACK } else { WHITE },
                cb(move |g| {
                    g.quests_tab = key;
                    g.right_panel.set_scroll_abs(0.0);
                }),
                Bo::r(9),
            );
        }
        top += 46;
        let weekly = self.quests_tab == "weekly";
        let left = secs_until_lisbon_reset(now_ts(), weekly);
        let sub_text = if weekly {
            tr!("Harder, bigger rewards. New ones in %s (Monday 00:00, Lisbon time).", reset_in(left))
        } else {
            tr!("New quests in %s (00:00, Lisbon time). Rewards grow as you do.", reset_in(left))
        };
        let small = self.f.small.clone();
        for line in crate::ui::fonts::wrap_text(&sub_text, &small, rect.w - 40) {
            let sub = small.render(&line, grey());
            self.canvas.blit(&sub, rect.x + 20, top);
            top += sub.h + 2;
        }
        top += 12;
        let content_rect = Rect::new(rect.x, top, rect.w, rect.bottom() - top);
        let scroll = self.right_panel.get_scroll();
        self.push_clip(content_rect);
        let content_h = self.draw_daily_list(content_rect, scroll, mouse_pos);
        self.pop_clip();
        self.right_panel.set_max_scroll((content_h - content_rect.h as f64).max(0.0));
        self.draw_scrollbar(content_rect, scroll, content_h, Some("right"), Some(mouse_pos));
    }

    fn draw_daily_list(&mut self, content_rect: Rect, scroll: f64, mouse_pos: (f64, f64)) -> f64 {
        let pad = 20;
        let row_w = content_rect.w - pad * 2;
        let pad_v = 14;
        let bar_h = 12;
        let med = self.f.med.clone();
        let sb = self.f.small_b.clone();
        let tiny = self.f.tiny.clone();
        let name_h = med.get_height();
        let reward_h = sb.get_height();
        let prog_h = tiny.get_height();
        let y_name = pad_v;
        let y_reward = y_name + name_h + 4;
        let weekly = self.quests_tab == "weekly";
        let potion_h = if weekly { reward_h + 2 } else { 0 };
        let y_bar = y_reward + reward_h + potion_h + 9;
        let y_prog = y_bar + bar_h + 5;
        let row_h = y_prog + prog_h + pad_v;
        let mut y = content_rect.top() as f64 + 6.0 - scroll;

        let missions = if weekly { self.state.weekly_missions.clone() } else { self.state.daily_missions.clone() };
        for (i, mission) in missions.iter().enumerate() {
            let target = mission.target;
            let progress = if weekly { self.state.weekly_mission_progress(i) } else { self.state.daily_mission_progress(i) };
            let claimed = if weekly { self.state.weekly_claimed.contains(&i) } else { self.state.daily_claimed.contains(&i) };
            let done = progress >= target;
            let row_rect = Rect::new(content_rect.x + pad, ti(y), row_w, row_h);
            if row_rect.bottom() > content_rect.top() && row_rect.top() < content_rect.bottom() {
                draw_panel(&mut self.canvas, row_rect, Some(panel_light()), 12, false, None);
                if claimed {
                    draw_state_border(&mut self.canvas, row_rect, Color::rgb(80, 150, 100), 12, 2);
                } else if done {
                    draw_state_border(&mut self.canvas, row_rect, accent(), 12, 2);
                }
                let name_txt = med.render(&mission_label(mission), WHITE);
                self.canvas.blit(&name_txt, row_rect.x + 16, row_rect.y + y_name);
                let reward = self.state.quest_reward(weekly, i, mission.reward);
                let rtxt = fit_text(&sb, &tr!("Reward: %s", quest_reward_line(&reward, false)), row_w - 150);
                let reward_txt = sb.render(&rtxt, if claimed { GOOD } else { grey() });
                self.canvas.blit(&reward_txt, row_rect.x + 16, row_rect.y + y_reward);
                if let Some(p) = quest_potion_text(&reward) {
                    let pt = sb.render(&fit_text(&sb, &format!("+ {}", p), row_w - 150), if claimed { GOOD } else { Color::rgb(186, 104, 255) });
                    self.canvas.blit(&pt, row_rect.x + 16, row_rect.y + y_reward + reward_h + 2);
                }

                let bar_rect = Rect::new(row_rect.x + 16, row_rect.y + y_bar, row_rect.w - 140, bar_h);
                draw::rect(&mut self.canvas, panel(), bar_rect, 0, 6);
                let frac = if done { 1.0 } else { (if target != 0 { progress as f64 / target as f64 } else { 1.0 }).clamp(0.0, 1.0) };
                let fill_w = 0.max((bar_rect.w as f64 * frac) as i32);
                if fill_w > 0 {
                    draw::rect(&mut self.canvas, if done { GOOD } else { accent() }, Rect::new(bar_rect.x, bar_rect.y, fill_w, bar_rect.h), 0, 6);
                }
                let prog_txt = tiny.render(&format!("{} / {}", format_number(progress.min(target) as f64), format_number(target as f64)), grey());
                self.canvas.blit(&prog_txt, bar_rect.x, row_rect.y + y_prog);

                let btn_rect = Rect::new(row_rect.right() - 116, row_rect.y + (row_h - 40) / 2, 100, 40);
                if claimed {
                    self.button(btn_rect, &tr("Claimed"), &sb, mouse_pos, panel(), panel(), grey(), None, Bo::r(8).enabled(false));
                } else {
                    self.button(
                        btn_rect,
                        &tr("Claim"),
                        &sb,
                        mouse_pos,
                        if done { accent() } else { Color::rgb(70, 73, 88) },
                        accent(),
                        if done { WHITE } else { grey() },
                        if done { cb(move |g| g.claim_daily_mission(i)) } else { None },
                        Bo::r(8).enabled(done).sfx(None),
                    );
                }
            }
            y += (row_h + 14) as f64;
        }
        if missions.is_empty() {
            let msg = med.render(&tr("No quests right now."), grey());
            let r = Rect::with_center(msg.w, msg.h, (content_rect.centerx(), content_rect.top() + 40));
            self.canvas.blit(&msg, r.x, r.y);
        }
        (y + scroll) - (content_rect.top() + 6) as f64
    }
}
