//! Pet panels: Index (chances) and Bag (Equipped / Inventory) (ui/pets_panel.py).

use super::base::{Bo, blit_center};
use super::{Game, cb};
use crate::core::data::{MUT_ORDER, RARITY_TIERS, base_pet_chance, is_mutation, mutation, pet_order, rarities};
use crate::core::formatting::{format_number, format_one_in};
use crate::gfx::{Color, Rect, draw, ti};
use crate::i18n::tr;
use crate::theme::*;
use crate::tr;
use crate::ui::cards::{cell, cell3, render_pet_card};
use crate::ui::drawing::draw_panel;
use crate::ui::fonts::{Font, wrap_text};
use std::rc::Rc;

const INV_SORTS: [(&str, &str); 4] = [("money", "Money"), ("rarity", "Rarity"), ("mutation", "Mutation"), ("quantity", "Quantity")];
const INV_MUT_FILTERS: [(&str, &str); 4] = [("all", "All"), ("normal", "Normal"), ("golden", "Golden"), ("diamond", "Diamond")];

impl Game {
    pub fn draw_index_panel(&mut self, rect: Rect, mouse_pos: (f64, f64)) {
        self.panel_header(rect, &tr("Pet Index"), mouse_pos, Rc::new(|g: &mut Game| g.right_panel.close()));
        let tabs = [("normal", tr("Normal")), ("golden", tr("Golden")), ("diamond", tr("Diamond"))];
        let pad = 20;
        let tab_w = (rect.w - pad * 2 - 16) / 3;
        let mut tx = rect.x + pad;
        let ty = rect.y + 58;
        let sb = self.f.small_b.clone();
        for (key, label) in &tabs {
            let active = self.index_tab == *key;
            let k: &'static str = key;
            self.button(
                Rect::new(tx, ty, tab_w, 30),
                label,
                &sb,
                mouse_pos,
                if active { accent() } else { panel_light() },
                if !active { accent_hover() } else { accent() },
                if active { BLACK } else { WHITE },
                cb(move |g| g.index_tab = k),
                Bo::r(8),
            );
            tx += tab_w + 8;
        }
        let m = self.index_tab;
        let locked_mut = (m == "golden" && self.state.upgrade_level("golden_unlock") < 1) || (m == "diamond" && self.state.upgrade_level("diamond_unlock") < 1);
        let content = Rect::new(rect.x, ty + 40, rect.w, rect.bottom() - (ty + 40));
        let scroll = self.right_panel.get_scroll();
        self.push_clip(content);
        let mut top_y = content.top() as f64 + 6.0 - scroll;
        if locked_mut {
            let small = self.f.small.clone();
            for line in wrap_text(&tr("Mutation not unlocked yet. Buy the upgrade in Upgrades."), &small, rect.w - pad * 2) {
                let t = small.render(&line, BAD);
                self.canvas.blit(&t, rect.x + pad, ti(top_y));
                top_y += 18.0;
            }
            top_y += 8.0;
        }
        let cols = 2;
        let gap = 12;
        let card = (rect.w - pad * 2 - gap - 6) / cols;
        let live = self.state.pet_probs(1.0, None);
        let muts = self.state.mutation_chances();
        for (pos, &i) in pet_order().iter().enumerate() {
            let pos = pos as i32;
            let rarity = &rarities()[i];
            let (col, row) = (pos % cols, pos / cols);
            let crect = Rect::new(rect.x + pad + col * (card + gap), ti(top_y + (row * (card + gap)) as f64), card, card);
            if crect.bottom() < content.top() - 4 || crect.top() > content.bottom() + 4 {
                continue;
            }
            let owned = self.state.count_owned(i, m);
            let real = self.state.combined_chance(i, m, Some(&live), Some(muts));
            let base_line = format!("({})", format_one_in(base_pet_chance(i, m)));
            let income_line = tr!("+%s/sec", format_number(self.state.pet_income(i, m)));
            let plates = vec![vec![cell(tr("Income"), income_line)], vec![cell3(tr("Chance"), format_one_in(real), base_line)]];
            let locked = locked_mut || owned <= 0;
            let s = render_pet_card(rarity, m, card, card, &plates, 0, locked);
            self.canvas.blit(&s, crect.x, crect.y);
        }
        let rows = (rarities().len() as i32 + cols - 1) / cols;
        let content_h = (top_y + scroll - content.top() as f64) + (rows * (card + gap)) as f64 + 10.0;
        self.right_panel.set_max_scroll((content_h - content.h as f64).max(0.0));
        self.pop_clip();
        self.draw_scrollbar(content, scroll, content_h, Some("right"), Some(mouse_pos));
    }

    pub fn toggle_bag_view(&mut self) {
        self.bag_view = if self.bag_view == "inventory" { "equipped" } else { "inventory" };
        self.left_panel.scroll.insert(Some("bag"), 0.0);
    }

    pub fn draw_bag_panel(&mut self, rect: Rect, mouse_pos: (f64, f64)) {
        self.panel_header(rect, &tr("Bag"), mouse_pos, Rc::new(|g: &mut Game| g.left_panel.close()));
        let sub = self.f.small.render(&tr!("%d/%d slots  ·  %s $/sec", self.state.equipped.len() as i64, self.state.max_slots(), format_number(self.state.income_per_second())), grey());
        self.canvas.blit(&sub, rect.x + 22, rect.y + 50);
        let pad = 20;
        let row_h = 34;
        let btn_y = rect.y + 74;
        let btn_w = rect.w - pad * 2;
        let in_inv = self.bag_view == "inventory";
        let sb = self.f.small_b.clone();
        let view_rect = Rect::new(rect.x + pad, btn_y, btn_w, row_h);
        self.button(view_rect, &if in_inv { tr("Equipped Pets") } else { tr("Inventory") }, &sb, mouse_pos, panel_light(), panel_lighter(), WHITE, cb(|g| g.toggle_bag_view()), Bo::r(9));
        let equip_rect = Rect::new(rect.x + pad, view_rect.bottom() + 8, btn_w, row_h);
        self.button(
            equip_rect,
            &tr("Equip Best"),
            &sb,
            mouse_pos,
            panel_light(),
            panel_lighter(),
            WHITE,
            cb(|g| {
                g.state.equip_best();
                g.show_toast(&tr("Equipped your best money-makers!"), 1.8);
            }),
            Bo::r(9).sfx(Some("equip")),
        );
        let mut btn_bottom = equip_rect.bottom();
        if self.state.auto_equip_unlocked() {
            let on = self.state.auto_equip_best_on;
            let auto_rect = Rect::new(rect.x + pad, btn_bottom + 8, btn_w, row_h);
            self.button(
                auto_rect,
                &tr!("Auto Equip Best: %s", if on { tr("ON") } else { tr("OFF") }),
                &sb,
                mouse_pos,
                if on { Color::rgb(52, 120, 80) } else { panel_light() },
                panel_lighter(),
                WHITE,
                cb(|g| {
                    g.state.auto_equip_best_on = !g.state.auto_equip_best_on;
                    if g.state.auto_equip_best_on {
                        g.state.equip_best();
                    }
                }),
                Bo::r(9),
            );
            btn_bottom = auto_rect.bottom();
        } else {
            let mut hint_y = btn_bottom + 8;
            let tiny = self.f.tiny.clone();
            for line in wrap_text(&tr("Auto Equip Best is unlocked in Upgrades (Misc)."), &tiny, btn_w) {
                let t = tiny.render(&line, grey_dim());
                self.canvas.blit(&t, rect.x + pad, hint_y);
                hint_y += 16;
            }
            btn_bottom = hint_y - 8;
        }
        if in_inv {
            btn_bottom = self.draw_inventory_controls(rect, btn_bottom, pad, mouse_pos);
        }
        let content_top = btn_bottom + 14;
        let content = Rect::new(rect.x, content_top, rect.w, rect.bottom() - content_top);
        let scroll = self.left_panel.get_scroll();
        self.push_clip(content);
        let content_h = if in_inv { self.draw_bag_inventory(rect, content, scroll, mouse_pos) } else { self.draw_bag_equipped(rect, content, scroll, mouse_pos, pad) };
        self.left_panel.set_max_scroll((content_h - content.h as f64).max(0.0));
        self.pop_clip();
        self.draw_scrollbar(content, scroll, content_h, Some("left"), Some(mouse_pos));
    }

    fn draw_bag_equipped(&mut self, rect: Rect, content: Rect, scroll: f64, mouse_pos: (f64, f64), pad: i32) -> f64 {
        let cols = 2;
        let gap = 12;
        let card = (rect.w - pad * 2 - gap - 6) / cols;
        let n_slots = self.state.max_slots() as i32;
        let top_y = content.top() as f64 + 6.0 - scroll;
        let mut order: Vec<usize> = (0..self.state.equipped.len()).collect();
        {
            let st = &self.state;
            order.sort_by(|a, b| {
                let ia = -st.pet_income(st.equipped[*a].0, st.equipped[*a].1);
                let ib = -st.pet_income(st.equipped[*b].0, st.equipped[*b].1);
                ia.partial_cmp(&ib).unwrap_or(std::cmp::Ordering::Equal)
            });
        }
        for i in 0..n_slots.max(0) {
            let (col, row) = (i % cols, i / cols);
            let crect = Rect::new(rect.x + pad + col * (card + gap), ti(top_y + (row * (card + gap)) as f64), card, card);
            if crect.bottom() < content.top() - 4 || crect.top() > content.bottom() + 4 {
                continue;
            }
            if (i as usize) < order.len() {
                let actual = order[i as usize];
                let (r_idx, m) = self.state.equipped[actual];
                let income = self.state.pet_income(r_idx, m);
                let plates = vec![vec![cell(tr("Income"), tr!("+%s/sec", format_number(income)))]];
                let s = render_pet_card(&rarities()[r_idx], m, card, card, &plates, 0, false);
                self.canvas.blit(&s, crect.x, crect.y);
                if crect.collidepoint(mouse_pos) && content.collidepoint(mouse_pos) {
                    draw::rect(&mut self.canvas, WHITE, crect, 3, 12);
                }
                self.register_button(crect, Rc::new(move |g: &mut Game| g.state.remove_slot_at(actual)), Some("equip"));
            } else {
                draw_panel(&mut self.canvas, crect, Some(panel_light()), 12, false, None);
                let t = self.f.small.render(&tr("empty"), grey_dim());
                blit_center(&mut self.canvas, &t, crect.center());
            }
        }
        let rows = (n_slots + cols - 1) / cols;
        let mut info_y = top_y + (rows * (card + gap)) as f64 + 4.0;
        let tiny = self.f.tiny.clone();
        for line in wrap_text(&tr("Click a pet to remove it. Open the Inventory to equip more."), &tiny, rect.w - pad * 2) {
            let t = tiny.render(&line, grey());
            self.canvas.blit(&t, rect.x + pad, ti(info_y));
            info_y += 16.0;
        }
        (info_y + scroll - content.top() as f64) + 10.0
    }

    pub fn inventory_filter_active(&self) -> bool {
        self.inv_mut != "all" || self.inv_tier.is_some()
    }

    fn inventory_scroll_top(&mut self) {
        self.left_panel.scroll.insert(Some("bag"), 0.0);
    }

    pub fn cycle_inv_sort(&mut self) {
        let i = INV_SORTS.iter().position(|(k, _)| *k == self.inv_sort).unwrap_or(0);
        self.inv_sort = INV_SORTS[(i + 1) % INV_SORTS.len()].0;
        self.inventory_scroll_top();
    }

    pub fn toggle_inv_order(&mut self) {
        self.inv_high_first = !self.inv_high_first;
        self.inventory_scroll_top();
    }

    pub fn set_inv_mut(&mut self, key: &'static str) {
        self.inv_mut = key;
        self.inventory_scroll_top();
    }

    pub fn step_inv_tier(&mut self, step: i64) {
        let mut options: Vec<Option<usize>> = vec![None];
        options.extend((0..RARITY_TIERS.len()).map(Some));
        let pos = options.iter().position(|o| *o == self.inv_tier).unwrap_or(0) as i64;
        self.inv_tier = options[(pos + step).rem_euclid(options.len() as i64) as usize];
        self.inventory_scroll_top();
    }

    fn draw_inventory_controls(&mut self, rect: Rect, top: i32, pad: i32, mouse_pos: (f64, f64)) -> i32 {
        let w = rect.w - pad * 2;
        let h = 28;
        let gap = 6;
        let sb = self.f.small_b.clone();
        let tb = self.f.tiny_b.clone();
        let font_for = |label: &str, width: i32| -> Font { if sb.size(label).0 <= width - 10 { sb.clone() } else { tb.clone() } };
        let mut y = top + 8;
        let half = (w - gap) / 2;
        let sort_name = INV_SORTS.iter().find(|(k, _)| *k == self.inv_sort).map(|(_, l)| *l).unwrap_or("Money");
        let sort_label = tr!("Sort: %s", tr(sort_name));
        self.button(Rect::new(rect.x + pad, y, half, h), &sort_label, &font_for(&sort_label, half), mouse_pos, panel_light(), panel_lighter(), WHITE, cb(|g| g.cycle_inv_sort()), Bo::r(8));
        let order_label = if self.inv_high_first { tr("Highest first") } else { tr("Lowest first") };
        let order_w = w - half - gap;
        self.button(Rect::new(rect.x + pad + half + gap, y, order_w, h), &order_label, &font_for(&order_label, order_w), mouse_pos, panel_light(), panel_lighter(), WHITE, cb(|g| g.toggle_inv_order()), Bo::r(8));
        y += h + gap;
        let tab_w = (w - gap * 3) / 4;
        for (n, (key, label)) in INV_MUT_FILTERS.iter().enumerate() {
            let active = self.inv_mut == *key;
            let label = tr(label);
            let k: &'static str = key;
            self.button(
                Rect::new(rect.x + pad + n as i32 * (tab_w + gap), y, tab_w, h),
                &label,
                &font_for(&label, tab_w),
                mouse_pos,
                if active { accent() } else { panel_light() },
                if !active { accent_hover() } else { accent() },
                if active { BLACK } else { WHITE },
                cb(move |g| g.set_inv_mut(k)),
                Bo::r(8),
            );
        }
        y += h + gap;
        let arrow_w = 34;
        self.button(Rect::new(rect.x + pad, y, arrow_w, h), "<", &sb, mouse_pos, panel_light(), panel_lighter(), WHITE, cb(|g| g.step_inv_tier(-1)), Bo::r(8));
        self.button(Rect::new(rect.right() - pad - arrow_w, y, arrow_w, h), ">", &sb, mouse_pos, panel_light(), panel_lighter(), WHITE, cb(|g| g.step_inv_tier(1)), Bo::r(8));
        let mid = Rect::new(rect.x + pad + arrow_w + gap, y, w - 2 * (arrow_w + gap), h);
        let (label, base, text_color) = match self.inv_tier {
            None => (tr("Rarity: All"), panel_light(), WHITE),
            Some(t) => {
                let tier = &RARITY_TIERS[t];
                (tr!("Rarity: %s", tr(tier.name)), tier.color, tier.text)
            }
        };
        let hover = if self.inv_tier.is_none() { panel_lighter() } else { base };
        self.button(mid, &label, &font_for(&label, mid.w), mouse_pos, base, hover, text_color, cb(|g| g.step_inv_tier(1)), Bo::r(8));
        y + h
    }

    pub fn inventory_entries(&self) -> Vec<(usize, &'static str)> {
        let st = &self.state;
        let mut entries: Vec<(usize, &'static str)> = Vec::new();
        for (key, n) in &st.owned {
            let Some((idx_s, m)) = key.split_once('_') else { continue };
            let Ok(idx) = idx_s.trim().parse::<i64>() else { continue };
            if *n > 0 && idx >= 0 && (idx as usize) < rarities().len() && is_mutation(m) {
                let mk = crate::core::data::mut_key(m);
                if self.inv_mut != "all" && mk != self.inv_mut {
                    continue;
                }
                if let Some(t) = self.inv_tier {
                    if rarities()[idx as usize].tier != t {
                        continue;
                    }
                }
                entries.push((idx as usize, mk));
            }
        }
        let rank = |i: usize| pet_order().iter().position(|p| *p == i).unwrap_or(0) as f64;
        let money = |e: &(usize, &str)| rarities()[e.0].income * mutation(e.1).map(|m| m.mult).unwrap_or(1.0);
        let mi = |m: &str| MUT_ORDER.iter().position(|x| *x == m).unwrap_or(0) as f64;
        let primary = |e: &(usize, &str)| -> f64 {
            match self.inv_sort {
                "rarity" => rank(e.0),
                "mutation" => mi(e.1),
                "quantity" => st.count_owned(e.0, e.1) as f64,
                _ => money(e),
            }
        };
        entries.sort_by(|a, b| {
            let ka = (-primary(a), -money(a), -rank(a.0), -mi(a.1));
            let kb = (-primary(b), -money(b), -rank(b.0), -mi(b.1));
            ka.partial_cmp(&kb).unwrap_or(std::cmp::Ordering::Equal)
        });
        if !self.inv_high_first {
            entries.reverse();
        }
        entries
    }

    fn draw_bag_inventory(&mut self, rect: Rect, content: Rect, scroll: f64, mouse_pos: (f64, f64)) -> f64 {
        let entries = self.inventory_entries();
        let ipad = 14;
        let gap = 8;
        let cols = 3;
        let card_w = (rect.w - ipad * 2 - gap * (cols - 1) - 6) / cols;
        let card_h = 196;
        let top_y = content.top() as f64 + 6.0 - scroll;
        if entries.is_empty() {
            let msg = if self.inventory_filter_active() && !self.state.owned.is_empty() { tr("No pets match these filters.") } else { tr("You don't have any pets yet. Roll some!") };
            let small = self.f.small.clone();
            for (k, line) in wrap_text(&msg, &small, rect.w - ipad * 2).iter().enumerate() {
                let t = small.render(line, grey());
                self.canvas.blit(&t, rect.x + ipad, ti(top_y + (k as i32 * 20) as f64));
            }
            return 80.0;
        }
        let sb = self.f.small_b.clone();
        for (n, (idx, m)) in entries.iter().enumerate() {
            let (idx, m) = (*idx, *m);
            let n = n as i32;
            let (col, row) = (n % cols, n / cols);
            let crect = Rect::new(rect.x + ipad + col * (card_w + gap), ti(top_y + (row * (card_h + gap)) as f64), card_w, card_h);
            if crect.bottom() < content.top() - 4 || crect.top() > content.bottom() + 4 {
                continue;
            }
            let owned = self.state.count_owned(idx, m);
            let eq = self.state.equipped_count(idx, m);
            let income = self.state.pet_income(idx, m);
            let plates = vec![vec![cell(tr("Income"), tr!("+%s/sec", format_number(income)))], vec![cell(tr("Have"), format_number(owned as f64)), cell(tr("Equipped"), eq.to_string())]];
            let s = render_pet_card(&rarities()[idx], m, card_w, card_h, &plates, 30, false);
            self.canvas.blit(&s, crect.x, crect.y);
            let bw = (card_w - 18) / 2;
            let minus = Rect::new(crect.x + 6, crect.bottom() - 28, bw, 22);
            let plus = Rect::new(crect.right() - 6 - bw, crect.bottom() - 28, bw, 22);
            let can_minus = eq > 0;
            let can_plus = eq < owned && (self.state.equipped.len() as i64) < self.state.max_slots();
            self.button(minus, "-", &sb, mouse_pos, Color::rgb(38, 40, 52), BAD, WHITE, if can_minus { cb(move |g| { g.state.equip_remove_one(idx, m); }) } else { None }, Bo::r(7).enabled(can_minus).sfx(Some("equip")));
            self.button(plus, "+", &sb, mouse_pos, Color::rgb(38, 40, 52), GOOD, WHITE, if can_plus { cb(move |g| { g.state.equip_add(idx, m); }) } else { None }, Bo::r(7).enabled(can_plus).sfx(Some("equip")));
        }
        let rows = (entries.len() as i32 + cols - 1) / cols;
        (top_y + scroll - content.top() as f64) + (rows * (card_h + gap)) as f64 + 10.0
    }
}
