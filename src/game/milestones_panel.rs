//! Milestones panel and the "milestone reached" toast (ui/milestones_panel.py).

use super::base::Bo;
use super::{Game, cb};
use crate::core::data::{MILESTONE_CAT_ORDER, milestone_cat, milestone_group, milestone_group_of, milestone_reward_label};
use crate::core::formatting::{format_number, format_playtime};
use crate::gfx::{Rect, draw, ti};
use crate::i18n::tr;
use crate::pyfmt::format as pyformat;
use crate::theme::*;
use crate::ui::drawing::{draw_panel, draw_state_border};
use crate::ui::fonts::fit_text;
use crate::{args, tr};
use crate::gfx::Color;
use std::rc::Rc;

fn reward_label(rt: &str) -> String {
    let l = milestone_reward_label(rt);
    tr(if l.is_empty() { rt } else { l })
}

pub fn format_milestone_reward(reward_type: &str, value: f64) -> String {
    pyformat("+%.0f%% %s", &args![value * 100.0, reward_label(reward_type)])
}

fn milestone_progress_text(category: &str, metric: f64, threshold: f64) -> String {
    match category {
        "playtime" => format!("{} / {}", format_playtime(metric), format_playtime(threshold)),
        "coins" => format!("${} / ${}", format_number(metric), format_number(threshold)),
        _ => format!("{} / {}", format_number(metric), format_number(threshold)),
    }
}

impl Game {
    pub fn check_milestones(&mut self) {
        let newly = self.state.check_milestones();
        if newly.is_empty() {
            return;
        }
        self.play("milestone", 0.0);
        if newly.len() == 1 {
            let (category, _i, _threshold, value) = newly[0];
            let d = milestone_cat(category).unwrap();
            let reward_txt = format_milestone_reward(d.reward_type, value);
            self.show_toast(&tr!("Milestone reached: %s!  (%s permanent)", tr(d.label), reward_txt), 1.8);
        } else {
            self.show_toast(&tr!("%d Milestones reached at once!", newly.len() as i64), 1.8);
        }
    }

    fn claimed_count(&self, key: &str, n: usize) -> usize {
        (0..n).filter(|i| self.state.milestones_claimed.contains(&format!("{}:{}", key, i))).count()
    }

    pub fn select_milestone_category(&mut self, category: Option<&'static str>) {
        self.milestones_selected_category = category;
        self.right_panel.scroll.insert(Some("milestones"), 0.0);
    }

    pub fn select_milestone_group(&mut self, group: Option<&'static str>) {
        self.milestones_selected_group = group;
        self.milestones_selected_category = None;
        self.right_panel.scroll.insert(Some("milestones"), 0.0);
    }

    /// Back: from a category's milestones to its group (if it has one) or to the main list.
    pub fn milestones_back(&mut self) {
        if self.milestones_selected_category.is_some() {
            self.select_milestone_category(None);
        } else {
            self.select_milestone_group(None);
        }
    }

    fn milestone_claimed_count(&self, category: &str) -> usize {
        milestone_cat(category).map(|d| self.claimed_count(category, d.tiers.len())).unwrap_or(0)
    }

    pub fn draw_milestones_panel(&mut self, rect: Rect, mouse_pos: (f64, f64)) {
        self.panel_header(rect, &tr("Milestones"), mouse_pos, Rc::new(|g: &mut Game| g.right_panel.close()));
        let mut top = rect.y + 62;
        let category = self.milestones_selected_category;
        let group = self.milestones_selected_group;
        if category.is_some() || group.is_some() {
            let back_to_group = category.is_some() && group.is_some();
            let back_rect = Rect::new(rect.x + 20, top, if back_to_group { 150 } else { 128 }, 32);
            let sb = self.f.small_b.clone();
            let label = if back_to_group { tr("<  Rarities") } else { tr("<  Categories") };
            self.button(back_rect, &label, &sb, mouse_pos, panel_light(), panel(), WHITE, cb(|g| g.milestones_back()), Bo::r(8));
            let (title, done_n, total) = match (category, group.and_then(milestone_group)) {
                (Some(cat), _) => {
                    let d = milestone_cat(cat).unwrap();
                    (tr(d.label), self.milestone_claimed_count(cat), d.tiers.len())
                }
                (None, Some(g)) => (
                    tr(g.label),
                    g.categories.iter().map(|c| self.milestone_claimed_count(c)).sum(),
                    g.categories.iter().map(|c| milestone_cat(c).map(|d| d.tiers.len()).unwrap_or(0)).sum(),
                ),
                _ => (String::new(), 0, 0),
            };
            let cat_txt = self.f.med.render(&format!("{}  ({}/{})", title, done_n, total), accent());
            self.canvas.blit(&cat_txt, rect.x + 20, top + 42);
            top += 84;
        }
        let content_rect = Rect::new(rect.x, top, rect.w, rect.bottom() - top);
        let scroll = self.right_panel.get_scroll();
        self.push_clip(content_rect);
        let content_h = match category {
            None => self.draw_milestones_category_list(content_rect, scroll, mouse_pos, group),
            Some(cat) => self.draw_milestones_tier_list(content_rect, scroll, cat),
        };
        self.pop_clip();
        self.right_panel.set_max_scroll((content_h - content_rect.h as f64).max(0.0));
        self.draw_scrollbar(content_rect, scroll, content_h, Some("right"), Some(mouse_pos));
    }

    /// The Milestones' first screen: a button per category, with how many you claimed (e.g. '1/8') and a filling
    /// bar. A group's categories (the rarity ones) show together as one row, which opens their list.
    fn draw_milestones_category_list(&mut self, content_rect: Rect, scroll: f64, mouse_pos: (f64, f64), group: Option<&'static str>) -> f64 {
        let pad = 20;
        let row_w = content_rect.w - pad * 2 - 6;
        let row_h = 78;
        let mut y = content_rect.top() as f64 + 6.0 - scroll;
        let med = self.f.med.clone();
        let tiny = self.f.tiny.clone();
        let sb = self.f.small_b.clone();
        let mut entries: Vec<(bool, &'static str)> = Vec::new(); // (is_group, key)
        match group.and_then(milestone_group) {
            Some(g) => entries.extend(g.categories.iter().map(|c| (false, *c))),
            None => {
                let mut seen: Vec<&'static str> = Vec::new();
                for c in MILESTONE_CAT_ORDER {
                    match milestone_group_of(c) {
                        None => entries.push((false, c)),
                        // the group takes the place of its 1st category
                        Some(gk) if !seen.contains(&gk) => {
                            seen.push(gk);
                            entries.push((true, gk));
                        }
                        Some(_) => {}
                    }
                }
            }
        }
        for (is_group, key) in entries {
            if is_group {
                y = self.draw_milestone_group_row(content_rect, y, row_w, row_h, pad, mouse_pos, key);
                continue;
            }
            let d = milestone_cat(key).unwrap();
            let tiers = d.tiers;
            let claimed_n = self.claimed_count(d.key, tiers.len());
            let done = claimed_n >= tiers.len();
            let row_rect = Rect::new(content_rect.x + pad, ti(y), row_w, row_h);
            if row_rect.bottom() > content_rect.top() && row_rect.top() < content_rect.bottom() {
                draw_panel(&mut self.canvas, row_rect, Some(panel_light()), 12, false, None);
                if done {
                    draw_state_border(&mut self.canvas, row_rect, Color::rgb(80, 150, 100), 12, 2);
                } else if row_rect.collidepoint(mouse_pos) && content_rect.collidepoint(mouse_pos) {
                    draw_state_border(&mut self.canvas, row_rect, accent(), 12, 2);
                }
                // long names (e.g. "Transcendent Pets Rolled") shrink so they don't touch the counter
                let name_txt = self.texto_que_cabe(&tr(d.label), &med, WHITE, row_rect.w - 110);
                self.canvas.blit(&name_txt, row_rect.x + 16, row_rect.y + 10);
                let reward_lbl = reward_label(d.reward_type);
                let active = self.state.milestone_bonus(d.reward_type);
                let sub_txt = tiny.render(&tr!("Bonus: %s  (active: +%.0f%%)", reward_lbl, active * 100.0), grey());
                self.canvas.blit(&sub_txt, row_rect.x + 16, row_rect.y + 36);
                let count_txt = sb.render(&format!("{}/{}", claimed_n, tiers.len()), if done { GOOD } else { WHITE });
                self.canvas.blit(&count_txt, row_rect.right() - count_txt.w - 40, row_rect.y + 12);
                let arrow_txt = med.render(">", grey());
                self.canvas.blit(&arrow_txt, row_rect.right() - 26, row_rect.centery() - 12);

                let bar_rect = Rect::new(row_rect.x + 16, row_rect.bottom() - 20, row_rect.w - 32, 10);
                draw::rect(&mut self.canvas, panel(), bar_rect, 0, 5);
                let frac = if tiers.is_empty() { 0.0 } else { claimed_n as f64 / tiers.len() as f64 };
                let fill_w = 0.max((bar_rect.w as f64 * frac) as i32);
                if fill_w > 0 {
                    draw::rect(&mut self.canvas, if done { GOOD } else { accent() }, Rect::new(bar_rect.x, bar_rect.y, fill_w, bar_rect.h), 0, 5);
                }
                let key = d.key;
                self.register_button(row_rect, Rc::new(move |g: &mut Game| g.select_milestone_category(Some(key))), Some("click"));
            }
            y += (row_h + 14) as f64;
        }
        (y + scroll) - (content_rect.top() + 6) as f64
    }

    /// A group's row in the main list: name, the categories it has and the summed progress.
    #[allow(clippy::too_many_arguments)]
    fn draw_milestone_group_row(&mut self, content_rect: Rect, y: f64, row_w: i32, row_h: i32, pad: i32, mouse_pos: (f64, f64), group: &'static str) -> f64 {
        let Some(g) = milestone_group(group) else { return y };
        let cats = g.categories;
        let claimed_n: usize = cats.iter().map(|c| self.milestone_claimed_count(c)).sum();
        let total: usize = cats.iter().map(|c| milestone_cat(c).map(|d| d.tiers.len()).unwrap_or(0)).sum();
        let done = claimed_n >= total;
        let row_rect = Rect::new(content_rect.x + pad, ti(y), row_w, row_h);
        if row_rect.bottom() > content_rect.top() && row_rect.top() < content_rect.bottom() {
            draw_panel(&mut self.canvas, row_rect, Some(panel_light()), 12, false, None);
            let bc = if done {
                Color::rgb(80, 150, 100)
            } else if row_rect.collidepoint(mouse_pos) && content_rect.collidepoint(mouse_pos) {
                accent()
            } else {
                Color::rgb(150, 120, 230) // shows that it opens another list
            };
            draw_state_border(&mut self.canvas, row_rect, bc, 12, 2);
            let med = self.f.med.clone();
            let tiny = self.f.tiny.clone();
            let name_txt = med.render(&tr(g.label), WHITE);
            self.canvas.blit(&name_txt, row_rect.x + 16, row_rect.y + 10);
            let sub = fit_text(&tiny, &tr!("%d categories: %s", cats.len() as i64, tr(g.desc)), row_rect.w - 60);
            let st = tiny.render(&sub, grey());
            self.canvas.blit(&st, row_rect.x + 16, row_rect.y + 36);
            let count_txt = self.f.small_b.render(&format!("{}/{}", claimed_n, total), if done { GOOD } else { WHITE });
            self.canvas.blit(&count_txt, row_rect.right() - count_txt.w - 40, row_rect.y + 12);
            let arrow_txt = med.render(">", grey());
            self.canvas.blit(&arrow_txt, row_rect.right() - 26, row_rect.centery() - 12);
            let bar_rect = Rect::new(row_rect.x + 16, row_rect.bottom() - 20, row_rect.w - 32, 10);
            draw::rect(&mut self.canvas, panel(), bar_rect, 0, 5);
            let frac = if total > 0 { claimed_n as f64 / total as f64 } else { 0.0 };
            let fill_w = (bar_rect.w as f64 * frac) as i32;
            if fill_w > 0 {
                draw::rect(&mut self.canvas, if done { GOOD } else { accent() }, Rect::new(bar_rect.x, bar_rect.y, fill_w, bar_rect.h), 0, 5);
            }
            self.register_button(row_rect, Rc::new(move |g: &mut Game| g.select_milestone_group(Some(group))), Some("click"));
        }
        y + (row_h + 14) as f64
    }

    fn draw_milestones_tier_list(&mut self, content_rect: Rect, scroll: f64, category: &'static str) -> f64 {
        let d = milestone_cat(category).unwrap();
        let metric = self.state.milestone_metric(category);
        let pad = 20;
        let row_w = content_rect.w - pad * 2 - 6;
        let row_h = 62;
        let mut y = content_rect.top() as f64 + 6.0 - scroll;
        let sb = self.f.small_b.clone();
        let tiny = self.f.tiny.clone();
        for (i, &(threshold, value)) in d.tiers.iter().enumerate() {
            let claimed = self.state.milestones_claimed.contains(&format!("{}:{}", category, i));
            let row_rect = Rect::new(content_rect.x + pad, ti(y), row_w, row_h);
            if row_rect.bottom() > content_rect.top() && row_rect.top() < content_rect.bottom() {
                draw_panel(&mut self.canvas, row_rect, Some(panel_light()), 10, false, None);
                if claimed {
                    draw_state_border(&mut self.canvas, row_rect, Color::rgb(80, 150, 100), 10, 2);
                }
                let name_txt = sb.render(&tr!("Milestone %d", i as i64 + 1), WHITE);
                self.canvas.blit(&name_txt, row_rect.x + 14, row_rect.y + 8);
                let reward_txt = sb.render(&format_milestone_reward(d.reward_type, value), if claimed { GOOD } else { grey() });
                self.canvas.blit(&reward_txt, row_rect.right() - reward_txt.w - 14, row_rect.y + 8);

                let bar_rect = Rect::new(row_rect.x + 14, row_rect.y + 32, row_rect.w - 28, 12);
                draw::rect(&mut self.canvas, panel(), bar_rect, 0, 6);
                let frac = if claimed { 1.0 } else { (if threshold != 0.0 { metric / threshold } else { 1.0 }).clamp(0.0, 1.0) };
                let fill_w = 0.max((bar_rect.w as f64 * frac) as i32);
                if fill_w > 0 {
                    draw::rect(&mut self.canvas, if claimed { GOOD } else { accent() }, Rect::new(bar_rect.x, bar_rect.y, fill_w, bar_rect.h), 0, 6);
                }
                let prog = if claimed { tr("Claimed!") } else { milestone_progress_text(category, metric, threshold) };
                let prog_txt = tiny.render(&prog, if claimed { GOOD } else { grey() });
                self.canvas.blit(&prog_txt, bar_rect.right() - prog_txt.w, bar_rect.bottom() + 3);
            }
            y += (row_h + 10) as f64;
        }
        (y + scroll) - (content_rect.top() + 6) as f64
    }
}
