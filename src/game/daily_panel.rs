//! Daily missions panel (ui/daily_panel.py).

use super::base::Bo;
use super::{Game, cb};
use crate::core::data::mission_def;
use crate::core::state::DailyMission;
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

impl Game {
    pub fn claim_daily_mission(&mut self, index: usize) {
        let reward = self.state.claim_daily_mission(index);
        if reward != 0 {
            self.play("trait_charge", 0.0);
            self.show_toast(&if reward == 1 { tr!("Daily mission claimed: +%d trait charge!", reward) } else { tr!("Daily mission claimed: +%d trait charges!", reward) }, 1.8);
        }
    }

    pub fn draw_daily_panel(&mut self, rect: Rect, mouse_pos: (f64, f64)) {
        self.state.ensure_daily_missions();
        self.panel_header(rect, &tr("Daily Missions"), mouse_pos, Rc::new(|g: &mut Game| g.right_panel.close()));
        let mut top = rect.y + 62;
        let sub = self.f.small.render(&tr("Resets every day - rewards are trait charges."), grey());
        self.canvas.blit(&sub, rect.x + 20, top);
        top += sub.h + 14;
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
        let y_bar = y_reward + reward_h + 9;
        let y_prog = y_bar + bar_h + 5;
        let row_h = y_prog + prog_h + pad_v;
        let mut y = content_rect.top() as f64 + 6.0 - scroll;

        let missions = self.state.daily_missions.clone();
        for (i, mission) in missions.iter().enumerate() {
            let target = mission.target;
            let progress = self.state.daily_mission_progress(i);
            let claimed = self.state.daily_claimed.contains(&i);
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
                let rtxt = if mission.reward == 1 { tr!("Reward: %d trait charge", mission.reward) } else { tr!("Reward: %d trait charges", mission.reward) };
                let reward_txt = sb.render(&rtxt, if claimed { GOOD } else { grey() });
                self.canvas.blit(&reward_txt, row_rect.x + 16, row_rect.y + y_reward);

                let bar_rect = Rect::new(row_rect.x + 16, row_rect.y + y_bar, row_rect.w - 140, bar_h);
                draw::rect(&mut self.canvas, panel(), bar_rect, 0, 6);
                let frac = if done { 1.0 } else { (if target != 0 { progress as f64 / target as f64 } else { 1.0 }).clamp(0.0, 1.0) };
                let fill_w = 0.max((bar_rect.w as f64 * frac) as i32);
                if fill_w > 0 {
                    draw::rect(&mut self.canvas, if done { GOOD } else { accent() }, Rect::new(bar_rect.x, bar_rect.y, fill_w, bar_rect.h), 0, 6);
                }
                let prog_txt = tiny.render(&format!("{} / {}", progress.min(target), target), grey());
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
            let msg = med.render(&tr("No missions today."), grey());
            let r = Rect::with_center(msg.w, msg.h, (content_rect.centerx(), content_rect.top() + 40));
            self.canvas.blit(&msg, r.x, r.y);
        }
        (y + scroll) - (content_rect.top() + 6) as f64
    }
}
