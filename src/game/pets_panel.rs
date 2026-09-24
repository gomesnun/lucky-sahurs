//! Pet panels: Index (chances) and Bag (Equipped / Inventory) (ui/pets_panel.py).

use super::base::{Bo, blit_center};
use super::{Game, cb};
use crate::config::{TOPBAR_H, VIRTUAL_H};
use crate::core::data::{INDEX_ENTRIES, MUT_ORDER, RARITY_TIERS, base_pet_chance, is_mutation, mutation, pet_order, rarities};
use crate::core::formatting::{format_number, format_one_in};
use crate::gfx::{Color, Rect, draw, ti};
use crate::i18n::tr;
use crate::theme::*;
use crate::tr;
use crate::ui::cards::{cell, cell3, render_pet_card};
use crate::ui::drawing::{dim_overlay, draw_panel, draw_rainbow_border};
use crate::ui::fonts::{Font, wrap_text};
use crate::ui::icons::load_icon;
use std::rc::Rc;

const INV_SORTS: [(&str, &str); 4] = [("money", "Money"), ("rarity", "Rarity"), ("mutation", "Mutation"), ("quantity", "Quantity")];
const INV_MUT_FILTERS: [(&str, &str); 5] = [("all", "All"), ("normal", "Normal"), ("golden", "Golden"), ("diamond", "Diamond"), ("rainbow", "Rainbow")];

/// the upgrade that unlocks each mutation
fn mutation_unlock(m: &str) -> Option<&'static str> {
    match m {
        "golden" => Some("golden_unlock"),
        "diamond" => Some("diamond_unlock"),
        "rainbow" => Some("rainbow_unlock"),
        _ => None,
    }
}

impl Game {
    pub fn draw_index_panel(&mut self, rect: Rect, mouse_pos: (f64, f64)) {
        self.panel_header(rect, &tr("Pet Index"), mouse_pos, Rc::new(|g: &mut Game| g.right_panel.close()));
        let tabs = [("normal", tr("Normal")), ("golden", tr("Golden")), ("diamond", tr("Diamond")), ("rainbow", tr("Rainbow"))];
        let pad = 20;
        let tab_w = (rect.w - pad * 2 - 8 * (tabs.len() as i32 - 1)) / tabs.len() as i32;
        let mut tx = rect.x + pad;
        let ty = rect.y + 58;
        let sb = self.f.small_b.clone();
        let tb = self.f.tiny_b.clone();
        for (key, label) in &tabs {
            let active = self.index_tab == *key;
            let k: &'static str = key;
            let trect = Rect::new(tx, ty, tab_w, 30);
            let font = if sb.size(label).0 <= tab_w - 10 { sb.clone() } else { tb.clone() };
            self.button(
                trect,
                label,
                &font,
                mouse_pos,
                if active { accent() } else { panel_light() },
                if !active { accent_hover() } else { accent() },
                if active { BLACK } else { WHITE },
                cb(move |g| g.index_tab = k),
                Bo::r(8),
            );
            if *key == "rainbow" && !active {
                draw_rainbow_border(&mut self.canvas, trect.inflate(-6, -6), 6, 2);
            }
            tx += tab_w + 8;
        }
        let m = self.index_tab;
        let locked_mut = mutation_unlock(m).is_some_and(|u| self.state.upgrade_level(u) < 1);
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
            let real = self.state.combined_chance(i, m, Some(&live), Some(muts));
            let base_line = format!("({})", format_one_in(base_pet_chance(i, m)));
            let income_line = tr!("+%s/sec", format_number(self.state.pet_income(i, m)));
            let plates = vec![vec![cell(tr("Income"), income_line)], vec![cell3(tr("Chance"), format_one_in(real), base_line)]];
            let locked = locked_mut || !self.state.is_indexed(i, m);
            let s = render_pet_card(rarity, m, card, card, &plates, 0, locked);
            self.canvas.blit(&s, crect.x, crect.y);
        }
        let rows = (rarities().len() as i32 + cols - 1) / cols;
        let content_h = (top_y + scroll - content.top() as f64) + (rows * (card + gap)) as f64 + 10.0;
        self.right_panel.set_max_scroll((content_h - content.h as f64).max(0.0));
        self.pop_clip();
        self.draw_scrollbar(content, scroll, content_h, Some("right"), Some(mouse_pos));
    }

    // ---------------------------------------------------------------- the Index page (like the Traits one)
    // With 64 verities x 4 mutations the side panel wasn't enough: the Index now opens as a big page with more
    // columns. (draw_index_panel, above, stays only for compatibility.)
    pub fn toggle_index_page(&mut self) {
        if self.index_open {
            self.close_index_page();
        } else {
            self.close_overlays();
            self.index_open = true;
            self.right_panel.close();
            self.left_panel.close();
        }
    }

    pub fn close_index_page(&mut self) {
        self.index_open = false;
    }

    pub fn draw_index_page(&mut self, mouse_pos: (f64, f64)) {
        let ov = dim_overlay(self.vw, VIRTUAL_H, 170);
        self.canvas.blit(&ov, 0, 0);
        let panel_w = 640.max((self.vw - 240).min(1300)); // wider than the Traits one: 4+ columns fit
        let top = TOPBAR_H + 12;
        let rect = Rect::new(self.vw / 2 - panel_w / 2, top, panel_w, VIRTUAL_H - top - 14);
        draw_panel(&mut self.canvas, rect, Some(panel()), 16, true, None);
        self.register_blocker(rect);

        let sb = self.f.small_b.clone();
        let title = self.f.big.render(&tr("Pet Index"), WHITE);
        self.canvas.blit(&title, rect.x + 26, rect.y + 16);
        self.button(Rect::new(rect.right() - 48, rect.y + 18, 30, 30), "X", &sb, mouse_pos, panel_light(), BAD, WHITE, cb(|g| g.close_index_page()), Bo::r(8));
        let got = self.state.indexed_pets_count();
        let sub = self.f.small.render(&tr!("Indexed: %d/%d  ·  %d verities, %d rarities", got, INDEX_ENTRIES as i64, rarities().len() as i64, RARITY_TIERS.len() as i64), grey());
        self.canvas.blit(&sub, rect.x + 26, rect.y + 18 + title.h);

        // tabs per mutation
        let pad = 24;
        let tabs = [("normal", tr("Normal")), ("golden", tr("Golden")), ("diamond", tr("Diamond")), ("rainbow", tr("Rainbow"))];
        let tab_w = 170.min((rect.w - pad * 2 - 8 * (tabs.len() as i32 - 1)) / tabs.len() as i32);
        let ty = rect.y + 26 + title.h + sub.h;
        for (n, (key, label)) in tabs.iter().enumerate() {
            let trect = Rect::new(rect.x + pad + n as i32 * (tab_w + 8), ty, tab_w, 34);
            let active = self.index_tab == *key;
            let k: &'static str = key;
            self.button(
                trect,
                label,
                &sb,
                mouse_pos,
                if active { accent() } else { panel_light() },
                if !active { accent_hover() } else { accent() },
                if active { BLACK } else { WHITE },
                cb(move |g| {
                    g.index_tab = k;
                    g.index_scroll = 0.0;
                }),
                Bo::r(8),
            );
            if *key == "rainbow" && !active {
                draw_rainbow_border(&mut self.canvas, trect.inflate(-6, -6), 6, 2);
            }
        }

        let m = self.index_tab;
        let locked_mut = mutation_unlock(m).is_some_and(|u| self.state.upgrade_level(u) < 1);
        let content = Rect::new(rect.x + 12, ty + 46, rect.w - 24, rect.bottom() - 16 - (ty + 46));
        self.index_list_rect = content;
        let scroll = self.index_scroll;
        self.push_clip(content);
        let mut top_y = content.top() as f64 + 4.0 - scroll;
        if locked_mut {
            let t = self.f.small.render(&tr("Mutation not unlocked yet. Buy the upgrade in Upgrades."), BAD);
            self.canvas.blit(&t, content.x + 12, ti(top_y));
            top_y += 26.0;
        }
        let gap = 12;
        let cols = 3.max((content.w - 24 + gap).div_euclid(190 + gap));
        let card = (content.w - 24 - gap * (cols - 1)) / cols;
        let live = self.state.pet_probs(1.0, None);
        let muts = self.state.mutation_chances();
        for (pos, &i) in pet_order().iter().enumerate() {
            let pos = pos as i32;
            let (col, row) = (pos % cols, pos / cols);
            let crect = Rect::new(content.x + 6 + col * (card + gap), ti(top_y + (row * (card + gap)) as f64), card, card);
            if crect.bottom() < content.top() - 4 || crect.top() > content.bottom() + 4 {
                continue;
            }
            let real = self.state.combined_chance(i, m, Some(&live), Some(muts));
            let base_line = format!("({})", format_one_in(base_pet_chance(i, m)));
            let income_line = tr!("+%s/sec", format_number(self.state.pet_income(i, m)));
            let plates = vec![vec![cell(tr("Income"), income_line)], vec![cell3(tr("Chance"), format_one_in(real), base_line)]];
            let locked = locked_mut || !self.state.is_indexed(i, m);
            let s = render_pet_card(&rarities()[i], m, card, card, &plates, 0, locked);
            self.canvas.blit(&s, crect.x, crect.y);
        }
        let rows = (pet_order().len() as i32 + cols - 1) / cols;
        let content_h = (top_y + scroll - content.top() as f64) + (rows * (card + gap)) as f64 + 10.0;
        self.index_max_scroll = (content_h - content.h as f64).max(0.0);
        self.index_scroll = self.index_scroll.clamp(0.0, self.index_max_scroll);
        self.pop_clip();
        self.draw_scrollbar(content, scroll, content_h, Some("index"), Some(mouse_pos));
    }

    /// The Bag switch: equipped pets <-> Inventory (every pet you have). From Potions it goes back to the pets.
    pub fn toggle_bag_view(&mut self) {
        self.bag_view = if self.bag_view == "inventory" { "equipped" } else { "inventory" };
        self.left_panel.scroll.insert(Some("bag"), 0.0);
    }

    pub fn show_bag_potions(&mut self) {
        self.bag_view = if self.bag_view == "potions" { "equipped" } else { "potions" };
        self.left_panel.scroll.insert(Some("bag"), 0.0);
    }

    /// v3.0.1: the Bag opens as a big page (like the Index): the buttons in a column on the left, the pets in a
    /// grid with as many columns as fit.
    pub fn draw_bag_page(&mut self, mouse_pos: (f64, f64)) {
        let ov = dim_overlay(self.vw, VIRTUAL_H, 170);
        self.canvas.blit(&ov, 0, 0);
        let panel_w = 640.max((self.vw - 240).min(1300));
        let top = TOPBAR_H + 12;
        let rect = Rect::new(self.vw / 2 - panel_w / 2, top, panel_w, VIRTUAL_H - top - 14);
        draw_panel(&mut self.canvas, rect, Some(panel()), 16, true, None);
        self.register_blocker(rect);
        self.left_rect = rect; // the mouse wheel scrolls the grid anywhere on the page
        self.draw_bag_panel(rect, mouse_pos);
    }

    pub fn draw_bag_panel(&mut self, full: Rect, mouse_pos: (f64, f64)) {
        self.panel_header(full, &tr("Bag"), mouse_pos, Rc::new(|g: &mut Game| g.left_panel.close()));
        let sub = self.f.small.render(&tr!("%d/%d slots  ·  %s $/sec", self.state.equipped.len() as i64, self.state.max_slots(), format_number(self.state.income_per_second())), grey());
        self.canvas.blit(&sub, full.x + 22, full.y + 50);
        if full.w >= 760 {
            self.draw_bag_page_body(full, mouse_pos);
            return;
        }
        // wide (the page): the buttons in two columns across the top, the grid (all the width) below
        let wide = full.w >= 760;
        let half_w = full.w / 2;
        let rect = if wide { Rect::new(full.x, full.y, half_w, full.h) } else { full };
        let grid = full;
        let pad = 20;
        let row_h = 34;
        let btn_y = rect.y + 74;
        let btn_w = rect.w - pad * 2;
        let in_inv = self.bag_view == "inventory";
        let in_potions = self.bag_view == "potions";
        let sb = self.f.small_b.clone();
        // [Equipped Pets / Inventory] [Potions] side by side
        let half = (btn_w - 8) / 2;
        let view_rect = Rect::new(rect.x + pad, btn_y, half, row_h);
        self.button(view_rect, &if in_inv { tr("Equipped Pets") } else { tr("Inventory") }, &sb, mouse_pos, panel_light(), panel_lighter(), WHITE, cb(|g| g.toggle_bag_view()), Bo::r(9));
        let pot_rect = Rect::new(view_rect.right() + 8, btn_y, btn_w - half - 8, row_h);
        let n_pot: i64 = self.state.shop.potions.values().sum();
        let pot_label = if n_pot > 0 { tr!("Potions (%d)", n_pot) } else { tr("Potions") };
        self.button(
            pot_rect,
            &pot_label,
            &sb,
            mouse_pos,
            if in_potions { accent() } else { panel_light() },
            if !in_potions { accent_hover() } else { accent() },
            if in_potions { BLACK } else { WHITE },
            cb(|g| g.show_bag_potions()),
            Bo::r(9).icon("shop/potion_luck"),
        );
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
            btn_bottom = if wide {
                let right = Rect::new(full.x + half_w - pad, full.y, full.w - half_w + pad, full.h);
                btn_bottom.max(self.draw_inventory_controls(right, btn_y - 8, pad, mouse_pos))
            } else {
                self.draw_inventory_controls(rect, btn_bottom, pad, mouse_pos)
            };
        }
        let content_top = btn_bottom + 14;
        let content = Rect::new(grid.x, content_top, grid.w, grid.bottom() - 10 - content_top);
        let scroll = self.left_panel.get_scroll();
        self.push_clip(content);
        let content_h = if in_potions {
            self.draw_bag_potions(grid, content, scroll, mouse_pos) as f64
        } else if in_inv {
            self.draw_bag_inventory(grid, content, scroll, mouse_pos)
        } else {
            self.draw_bag_equipped(grid, content, scroll, mouse_pos, pad)
        };
        self.left_panel.set_max_scroll((content_h - content.h as f64).max(0.0));
        self.pop_clip();
        self.draw_scrollbar(content, scroll, content_h, Some("left"), Some(mouse_pos));
    }

    fn set_bag_view(&mut self, view: &'static str) {
        self.bag_view = view;
        self.left_panel.scroll.insert(Some("bag"), 0.0);
    }

    /// v3.0.3, the Bag page: tabs (Equipped / Inventory / Potions) on the left of one row and the equip buttons on
    /// its right, the Inventory filters in one row under it, then the cards.
    fn draw_bag_page_body(&mut self, full: Rect, mouse_pos: (f64, f64)) {
        let pad = 22;
        let sb = self.f.small_b.clone();
        let y = full.y + 80;
        let h = 38;
        let n_inv = self.state.owned.values().filter(|n| **n > 0).count() as i64;
        let n_pot: i64 = self.state.shop.potions.values().sum();
        let tabs: [(&'static str, String, &'static str); 3] = [
            ("equipped", tr!("Equipped %d/%d", self.state.equipped.len() as i64, self.state.max_slots()), "bag"),
            ("inventory", tr!("Inventory (%d)", n_inv), "index"),
            ("potions", if n_pot > 0 { tr!("Potions (%d)", n_pot) } else { tr("Potions") }, "shop/potion_luck"),
        ];
        let tab_w = 200.min((full.w - pad * 2 - 16) / 5);
        let mut x = full.x + pad;
        for (key, label, icon) in tabs {
            let on = self.bag_view == key;
            self.button(
                Rect::new(x, y, tab_w, h),
                &label,
                &sb,
                mouse_pos,
                if on { accent() } else { panel_light() },
                if on { accent() } else { panel_lighter() },
                if on { BLACK } else { WHITE },
                cb(move |g| g.set_bag_view(key)),
                Bo::r(10).icon(icon),
            );
            x += tab_w + 8;
        }
        // the equip buttons, on the right of the same row
        let act_w = 190;
        let mut rx = full.right() - pad - act_w;
        if self.state.auto_equip_unlocked() {
            let on = self.state.auto_equip_best_on;
            self.button(
                Rect::new(rx, y, act_w, h),
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
                Bo::r(10),
            );
            rx -= act_w + 8;
        }
        if rx >= x {
            self.button(
                Rect::new(rx, y, act_w, h),
                &tr("Equip Best"),
                &sb,
                mouse_pos,
                Color::rgb(52, 120, 80),
                Color::rgb(66, 150, 100),
                WHITE,
                cb(|g| {
                    g.state.equip_best();
                    g.show_toast(&tr("Equipped your best money-makers!"), 1.8);
                }),
                Bo::r(10).sfx(Some("equip")),
            );
        }
        let mut bottom = y + h;
        if self.bag_view == "inventory" {
            bottom = self.draw_inventory_filter_row(Rect::new(full.x + pad, bottom + 10, full.w - pad * 2, 32), mouse_pos);
        }
        draw::rect(&mut self.canvas, panel_light(), Rect::new(full.x + pad, bottom + 12, full.w - pad * 2, 2), 0, 0);
        let content_top = bottom + 20;
        let content = Rect::new(full.x, content_top, full.w, full.bottom() - 10 - content_top);
        let scroll = self.left_panel.get_scroll();
        self.push_clip(content);
        let content_h = match self.bag_view {
            "potions" => self.draw_bag_potions(full, content, scroll, mouse_pos) as f64,
            "inventory" => self.draw_bag_inventory(full, content, scroll, mouse_pos),
            _ => self.draw_bag_equipped(full, content, scroll, mouse_pos, pad),
        };
        self.left_panel.set_max_scroll((content_h - content.h as f64).max(0.0));
        self.pop_clip();
        self.draw_scrollbar(content, scroll, content_h, Some("left"), Some(mouse_pos));
    }

    /// The Inventory's sort and filters in one row: [Sort] [Order] [< Mutation >] [< Rarity >].
    fn draw_inventory_filter_row(&mut self, row: Rect, mouse_pos: (f64, f64)) -> i32 {
        let gap = 8;
        let sb = self.f.small_b.clone();
        let tb = self.f.tiny_b.clone();
        let font_for = |label: &str, width: i32| -> Font { if sb.size(label).0 <= width - 10 { sb.clone() } else { tb.clone() } };
        let unit = (row.w - gap * 3) / 10;
        let (sort_w, order_w, pick_w) = (unit * 2, unit * 2, (row.w - gap * 3 - unit * 4) / 2);
        let (y, h) = (row.y, row.h);
        let mut x = row.x;
        let sort_name = INV_SORTS.iter().find(|(k, _)| *k == self.inv_sort).map(|(_, l)| *l).unwrap_or("Money");
        let sort_label = tr!("Sort: %s", tr(sort_name));
        self.button(Rect::new(x, y, sort_w, h), &sort_label, &font_for(&sort_label, sort_w), mouse_pos, panel_light(), panel_lighter(), WHITE, cb(|g| g.cycle_inv_sort()), Bo::r(8));
        x += sort_w + gap;
        let order_label = if self.inv_high_first { tr("Highest first") } else { tr("Lowest first") };
        self.button(Rect::new(x, y, order_w, h), &order_label, &font_for(&order_label, order_w), mouse_pos, panel_light(), panel_lighter(), WHITE, cb(|g| g.toggle_inv_order()), Bo::r(8));
        x += order_w + gap;
        let arrow_w = 30;
        // mutation picker
        let mut_name = INV_MUT_FILTERS.iter().find(|(k, _)| *k == self.inv_mut).map(|(_, l)| *l).unwrap_or("All");
        let label = tr!("Mutation: %s", tr(mut_name));
        let active = self.inv_mut != "all";
        self.button(Rect::new(x, y, arrow_w, h), "<", &sb, mouse_pos, panel_light(), panel_lighter(), WHITE, cb(|g| g.step_inv_mut(-1)), Bo::r(8));
        let mid = Rect::new(x + arrow_w + 4, y, pick_w - 2 * (arrow_w + 4), h);
        self.button(
            mid,
            &label,
            &font_for(&label, mid.w),
            mouse_pos,
            if active { accent() } else { panel_light() },
            if active { accent_hover() } else { panel_lighter() },
            if active { BLACK } else { WHITE },
            cb(|g| g.step_inv_mut(1)),
            Bo::r(8),
        );
        if self.inv_mut == "rainbow" {
            draw_rainbow_border(&mut self.canvas, mid.inflate(-6, -6), 6, 2);
        }
        self.button(Rect::new(x + pick_w - arrow_w, y, arrow_w, h), ">", &sb, mouse_pos, panel_light(), panel_lighter(), WHITE, cb(|g| g.step_inv_mut(1)), Bo::r(8));
        x += pick_w + gap;
        // rarity picker
        let (label, base, text_color) = match self.inv_tier {
            None => (tr("Rarity: All"), panel_light(), WHITE),
            Some(t) => {
                let tier = &RARITY_TIERS[t];
                (tr!("Rarity: %s", tr(tier.name)), tier.color, tier.text)
            }
        };
        let hover = if self.inv_tier.is_none() { panel_lighter() } else { base };
        self.button(Rect::new(x, y, arrow_w, h), "<", &sb, mouse_pos, panel_light(), panel_lighter(), WHITE, cb(|g| g.step_inv_tier(-1)), Bo::r(8));
        let mid = Rect::new(x + arrow_w + 4, y, pick_w - 2 * (arrow_w + 4), h);
        self.button(mid, &label, &font_for(&label, mid.w), mouse_pos, base, hover, text_color, cb(|g| g.step_inv_tier(1)), Bo::r(8));
        self.button(Rect::new(x + pick_w - arrow_w, y, arrow_w, h), ">", &sb, mouse_pos, panel_light(), panel_lighter(), WHITE, cb(|g| g.step_inv_tier(1)), Bo::r(8));
        y + h
    }

    fn draw_bag_equipped(&mut self, rect: Rect, content: Rect, scroll: f64, mouse_pos: (f64, f64), pad: i32) -> f64 {
        let gap = 14;
        let n_slots = self.state.max_slots() as i32;
        let fit = 2.max((rect.w - pad * 2 - 6 + gap) / (170 + gap));
        let cols = fit.min(n_slots.max(1));
        let card = ((rect.w - pad * 2 - gap * (fit - 1) - 6) / fit).max(150).min(if rect.w >= 760 { 230 } else { i32::MAX });
        // the cards in the middle of the page (with few slots they used to sit on the left, the rest empty)
        let row_w = cols * card + (cols - 1) * gap;
        let x0 = if rect.w >= 760 { rect.x + (rect.w - row_w) / 2 } else { rect.x + pad };
        let top_y = content.top() as f64 + 10.0 - scroll;
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
            let crect = Rect::new(x0 + col * (card + gap), ti(top_y + (row * (card + gap)) as f64), card, card);
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
                // an empty slot: click it to pick a pet in the Inventory
                let hover = crect.collidepoint(mouse_pos) && content.collidepoint(mouse_pos);
                draw_panel(&mut self.canvas, crect, Some(if hover { panel_lighter() } else { panel_light() }), 12, false, None);
                let plus = self.f.big.render("+", if hover { WHITE } else { grey_dim() });
                let (cx, cy) = crect.center();
                blit_center(&mut self.canvas, &plus, (cx, cy - 10));
                let t = self.f.small.render(&tr("empty"), grey_dim());
                blit_center(&mut self.canvas, &t, (cx, cy + 22));
                self.register_button(crect, Rc::new(|g: &mut Game| g.set_bag_view("inventory")), Some("click"));
            }
        }
        let rows = (n_slots + cols - 1) / cols;
        let mut info_y = top_y + (rows * (card + gap)) as f64 + 8.0;
        let small = self.f.small.clone();
        let mut hint = tr("Click a pet to remove it. Open the Inventory to equip more.");
        if !self.state.auto_equip_unlocked() {
            hint = format!("{}  {}", hint, tr("Auto Equip Best is unlocked in Upgrades (Misc)."));
        }
        for line in wrap_text(&hint, &small, rect.w - pad * 2) {
            let t = small.render(&line, grey_dim());
            if rect.w >= 760 {
                blit_center(&mut self.canvas, &t, (rect.centerx(), ti(info_y) + t.h / 2));
            } else {
                self.canvas.blit(&t, rect.x + pad, ti(info_y));
            }
            info_y += 20.0;
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

    pub fn step_inv_mut(&mut self, step: i64) {
        let pos = INV_MUT_FILTERS.iter().position(|(k, _)| *k == self.inv_mut).unwrap_or(0) as i64;
        let k = INV_MUT_FILTERS[(pos + step).rem_euclid(INV_MUT_FILTERS.len() as i64) as usize].0;
        self.set_inv_mut(k);
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
        // row 2: mutation filter (< Mutation: All >) - with 5 options the side-by-side buttons didn't fit
        y += h + gap;
        let arrow_w = 34;
        self.button(Rect::new(rect.x + pad, y, arrow_w, h), "<", &sb, mouse_pos, panel_light(), panel_lighter(), WHITE, cb(|g| g.step_inv_mut(-1)), Bo::r(8));
        self.button(Rect::new(rect.right() - pad - arrow_w, y, arrow_w, h), ">", &sb, mouse_pos, panel_light(), panel_lighter(), WHITE, cb(|g| g.step_inv_mut(1)), Bo::r(8));
        let mid = Rect::new(rect.x + pad + arrow_w + gap, y, w - 2 * (arrow_w + gap), h);
        let mut_name = INV_MUT_FILTERS.iter().find(|(k, _)| *k == self.inv_mut).map(|(_, l)| *l).unwrap_or("All");
        let label = tr!("Mutation: %s", tr(mut_name));
        let active = self.inv_mut != "all";
        self.button(
            mid,
            &label,
            &font_for(&label, mid.w),
            mouse_pos,
            if active { accent() } else { panel_light() },
            if active { accent_hover() } else { panel_lighter() },
            if active { BLACK } else { WHITE },
            cb(|g| g.step_inv_mut(1)),
            Bo::r(8),
        );
        if self.inv_mut == "rainbow" {
            draw_rainbow_border(&mut self.canvas, mid.inflate(-6, -6), 6, 2);
        }
        y += h + gap;
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
        // square cards like the Index (a strip at the bottom for the - / + buttons)
        let cols = 3.max((rect.w - ipad * 2 - 6 + gap) / (180 + gap));
        let card_w = (rect.w - ipad * 2 - gap * (cols - 1) - 6) / cols;
        let card_h = card_w + 16;
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
            let plates = vec![vec![cell(tr("Income"), tr!("+%s/sec", format_number(income)))], vec![cell(tr("Have"), format_number(owned as f64)), cell(tr("Equip"), eq.to_string())]];
            let s = render_pet_card(&rarities()[idx], m, card_w, card_h, &plates, 30, false);
            self.canvas.blit(&s, crect.x, crect.y);
            let bw = (card_w - 18) / 2;
            let minus = Rect::new(crect.x + 6, crect.bottom() - 28, bw, 22);
            let plus = Rect::new(crect.right() - 6 - bw, crect.bottom() - 28, bw, 22);
            let can_minus = eq > 0;
            let can_plus = eq < owned && (self.state.equipped.len() as i64) < self.state.max_slots();
            self.button(minus, "-", &sb, mouse_pos, Color::rgb(38, 40, 52), BAD, WHITE, if can_minus { cb(move |g| { g.state.equip_remove_one(idx, m); }) } else { None }, Bo::r(7).enabled(can_minus).sfx(Some("equip")));
            self.button(plus, "+", &sb, mouse_pos, Color::rgb(38, 40, 52), GOOD, WHITE, if can_plus { cb(move |g| { g.state.equip_add(idx, m); }) } else { None }, Bo::r(7).enabled(can_plus).sfx(Some("equip")));

            // the sell button ($): on the right, under the rarity, next to the verity's image
            let sell_size = 28;
            let sell_rect = Rect::new(crect.right() - 6 - sell_size, crect.y + 34, sell_size, sell_size);
            let label = if load_icon("sell", 16).is_some() { "" } else { "$" };
            self.button(sell_rect, label, &sb, mouse_pos, Color::rgb(38, 40, 52), Color::rgb(62, 66, 84), GOOD, cb(move |g| g.open_sell(idx, m)), Bo::r(7));
            if let Some(icon) = load_icon("sell", sell_size - 6) {
                if self.clip_allows(&sell_rect) {
                    blit_center(&mut self.canvas, &icon, sell_rect.center());
                }
            }
        }
        let rows = (entries.len() as i32 + cols - 1) / cols;
        (top_y + scroll - content.top() as f64) + (rows * (card_h + gap)) as f64 + 10.0
    }
}
