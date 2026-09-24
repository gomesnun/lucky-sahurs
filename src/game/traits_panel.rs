//! Traits page (ui/traits_panel.py).

use super::base::Bo;
use super::{Game, cb};
use crate::config::{TOPBAR_H, VIRTUAL_H};
use crate::core::data::TRAITS;
use crate::core::formatting::format_one_in;
use crate::gfx::{Color, Rect, Surf, draw, ti};
use crate::i18n::tr;
use crate::pyfmt::format as pyformat;
use crate::theme::*;
use crate::ui::cards::render_trait_card;
use crate::ui::drawing::{dim_overlay, draw_panel};
use crate::ui::fonts::{fit_text, wrap_text};
use crate::{args, tr};
use indexmap::IndexMap;
use std::rc::Rc;

const BATCH_ROW_H: i32 = 20;

impl Game {
    pub fn close_traits(&mut self) {
        self.traits_open = false;
    }

    pub fn toggle_traits(&mut self) {
        if self.traits_open {
            self.traits_open = false;
        } else {
            self.close_overlays();
            self.traits_open = true;
            self.right_panel.close();
            self.left_panel.close();
        }
    }

    pub fn notify_trait_charges(&mut self, n: i64) {
        if n <= 0 || !self.settings.get_bool("trait_notifications", true) {
            return;
        }
        if self.toast_kind == Some("trait") && self.toast_timer > 0.0 {
            self.trait_toast_count += n;
        } else {
            self.trait_toast_count = n;
        }
        self.toast_kind = Some("trait");
        self.toast_text = Some(if self.trait_toast_count == 1 { tr("You gained a Trait Roll!") } else { tr!("You gained a Trait Roll!  x%d", self.trait_toast_count) });
        self.toast_timer = 1.8;
    }

    /// Game.render_trait_card: the card with the chance plate filled in.
    pub fn trait_card(&self, trait_index: usize, w: i32, h: i32, owned: bool, equipped: bool) -> Surf {
        let chance_text = format_one_in(self.state.trait_chance(trait_index));
        render_trait_card(trait_index, w, h, owned, equipped, &chance_text)
    }

    pub fn draw_traits_page(&mut self, mouse_pos: (f64, f64)) {
        let ov = dim_overlay(self.vw, VIRTUAL_H, 170);
        self.canvas.blit(&ov, 0, 0);

        let panel_w = 640.max((self.vw - 400).min(1080));
        let top = TOPBAR_H + 12;
        let panel_h = VIRTUAL_H - top - 14;
        let rect = Rect::new(self.vw / 2 - panel_w / 2, top, panel_w, panel_h);
        draw_panel(&mut self.canvas, rect, Some(panel()), 16, true, None);
        self.register_button(rect, Rc::new(|_: &mut Game| {}), None);

        let title = self.f.big.render(&tr("Traits"), WHITE);
        self.canvas.blit(&title, rect.x + 26, rect.y + 16);
        let close_rect = Rect::new(rect.right() - 48, rect.y + 18, 30, 30);
        let sb = self.f.small_b.clone();
        self.button(close_rect, "X", &sb, mouse_pos, panel_light(), BAD, WHITE, cb(|g| g.close_traits()), Bo::r(8));

        let sub_y = rect.y + 16 + title.h + 2;
        let small = self.f.small.clone();
        let sub_text = fit_text(&small, &tr("Your collection — click a trait you own to equip it."), panel_w - 52 - 60);
        let s = small.render(&sub_text, grey());
        self.canvas.blit(&s, rect.x + 26, sub_y);

        let content_top = sub_y + small.get_height() + 14;
        let content_h = rect.bottom() - 22 - content_top;
        let list_w = (panel_w as f64 * 0.36) as i32;
        let list_rect = Rect::new(rect.x + 22, content_top, list_w, content_h);
        let roll_rect = Rect::new(list_rect.right() + 22, content_top, panel_w - list_w - 66, content_h);

        self.draw_traits_list(list_rect, mouse_pos);
        self.draw_traits_roll_area(roll_rect, mouse_pos);
    }

    fn draw_traits_list(&mut self, rect: Rect, mouse_pos: (f64, f64)) {
        self.traits_list_rect = rect;
        self.push_clip(rect);
        let scroll = self.traits_scroll;
        let card_w = rect.w - 16;
        let card_h = 92;
        let gap = 12;
        let mut y = rect.top() as f64 - scroll;
        for i in 0..TRAITS.len() {
            let crect = Rect::new(rect.x, ti(y), card_w, card_h);
            if crect.bottom() >= rect.top() - 4 && crect.top() <= rect.bottom() + 4 {
                let owned = self.state.owned_traits.contains(&i);
                let equipped = self.state.equipped_trait == Some(i);
                let surf = self.trait_card(i, card_w, card_h, owned, equipped);
                self.canvas.blit(&surf, crect.x, crect.y);
                if owned {
                    if crect.collidepoint(mouse_pos) && rect.collidepoint(mouse_pos) {
                        draw::rect(&mut self.canvas, WHITE, crect, 2, 12);
                    }
                    self.register_button(
                        crect,
                        Rc::new(move |g: &mut Game| {
                            g.state.equip_trait(i);
                        }),
                        Some("equip"),
                    );
                }
            }
            y += (card_h + gap) as f64;
        }
        let content_h = (TRAITS.len() as i32 * (card_h + gap) - gap) as f64;
        self.traits_max_scroll = (content_h - rect.h as f64).max(0.0);
        self.traits_scroll = self.traits_scroll.min(self.traits_max_scroll).max(0.0);
        self.pop_clip();
        self.draw_scrollbar(rect, scroll, content_h, Some("traits"), Some(mouse_pos));
    }

    fn traits_batch_height(&self) -> i32 {
        36 + self.state.last_trait_batch.as_ref().map(|b| b.len() as i32).unwrap_or(0) * BATCH_ROW_H + 10
    }

    fn draw_traits_batch_summary(&mut self, rect: Rect) {
        let batch = self.state.last_trait_batch.clone().unwrap_or_default();
        let total: i64 = batch.values().sum();
        draw_panel(&mut self.canvas, rect, Some(panel_light()), 12, true, None);
        let sb = self.f.small_b.clone();
        let small = self.f.small.clone();
        let title_txt = fit_text(&sb, &tr!("Batch summary (%d charges used):", total), rect.w - 28);
        let title = sb.render(&title_txt, WHITE);
        self.canvas.blit(&title, rect.x + 14, rect.y + 10);
        let mut ty = rect.y + 36;
        let mut keys: Vec<usize> = batch.keys().copied().collect();
        keys.sort_unstable_by(|a, b| b.cmp(a));
        for idx in keys {
            let t = &TRAITS[idx];
            let count = batch[&idx];
            let swatch = Rect::new(rect.x + 14, ty + 3, 14, 14);
            draw::rect(&mut self.canvas, t.color, swatch, 0, 3);
            let line = fit_text(&small, &pyformat("x%d  %s", &args![count, tr(t.name)]), rect.right() - 14 - (swatch.right() + 10));
            let s = small.render(&line, WHITE);
            self.canvas.blit(&s, swatch.right() + 10, ty);
            ty += BATCH_ROW_H;
        }
    }

    fn draw_traits_roll_area(&mut self, rect: Rect, mouse_pos: (f64, f64)) {
        let (card_w, card_h) = (260, 172);
        let card_rect = Rect::new(rect.centerx() - card_w / 2, rect.top() + 10, card_w, card_h);
        let small = self.f.small.clone();
        let med = self.f.med.clone();
        let tiny = self.f.tiny.clone();
        let sb = self.f.small_b.clone();

        let has_batch = self.state.last_trait_batch.as_ref().is_some_and(|b| !b.is_empty());
        let mut y;
        if has_batch {
            let sw = card_w.max((rect.w - 20).min(380));
            let summary_rect = Rect::new(rect.centerx() - sw / 2, rect.top() + 10, sw, self.traits_batch_height());
            self.draw_traits_batch_summary(summary_rect);
            y = summary_rect.bottom() + 16;
        } else if let Some(last) = self.state.last_trait_roll {
            let surf = self.trait_card(last, card_w, card_h, true, self.state.equipped_trait == Some(last));
            self.canvas.blit(&surf, card_rect.x, card_rect.y);
            y = card_rect.bottom() + 22;
        } else {
            draw_panel(&mut self.canvas, card_rect, Some(panel_light()), 12, true, None);
            let txt = small.render(&tr("You haven't rolled any traits yet."), grey());
            let r = Rect::with_center(txt.w, txt.h, card_rect.center());
            self.canvas.blit(&txt, r.x, r.y);
            y = card_rect.bottom() + 22;
        }
        let charges = self.state.trait_charges;
        let charges_txt = med.render(&tr!("Charges available: %d", charges), if charges != 0 { GOOD } else { grey() });
        let r = Rect::with_center(charges_txt.w, charges_txt.h, (rect.centerx(), y + charges_txt.h / 2));
        self.canvas.blit(&charges_txt, r.x, r.y);
        y += 34;

        let chance_str = tr!("Every pet roll (manual or Auto Roller) has a %.3f%% chance of giving 1 charge.", self.state.trait_charge_chance() * 100.0);
        for line in wrap_text(&chance_str, &tiny, rect.w - 20) {
            let t = tiny.render(&line, grey());
            let r = Rect::with_center(t.w, t.h, (rect.centerx(), y + t.h / 2));
            self.canvas.blit(&t, r.x, r.y);
            y += 16;
        }
        y += 12;

        let btn_w = 260.min(rect.w - 20);
        let can = charges > 0;
        self.button(
            Rect::new(rect.centerx() - btn_w / 2, y, btn_w, 46),
            &tr("Roll Trait  (1 charge)"),
            &med,
            mouse_pos,
            if can { accent() } else { Color::rgb(70, 73, 88) },
            accent_hover(),
            if can { BLACK } else { grey() },
            if can {
                cb(|g| {
                    g.state.roll_trait();
                    g.state.last_trait_batch = None;
                    g.play("trait_roll", 0.0);
                })
            } else {
                None
            },
            Bo::r(10).enabled(can).sfx(None),
        );
        y += 56;

        self.button(
            Rect::new(rect.centerx() - btn_w / 2, y, btn_w, 40),
            &tr!("Use All Charges (%d)", charges),
            &sb,
            mouse_pos,
            if can { Color::rgb(52, 120, 80) } else { Color::rgb(70, 73, 88) },
            panel_lighter(),
            if can { WHITE } else { grey() },
            if can {
                cb(|g| {
                    let n = g.state.trait_charges;
                    let mut results: IndexMap<usize, i64> = IndexMap::new();
                    while g.state.trait_charges > 0 {
                        if let Some(idx) = g.state.roll_trait() {
                            *results.entry(idx).or_insert(0) += 1;
                        }
                    }
                    if n > 0 {
                        g.state.last_trait_batch = Some(results);
                        g.play("trait_roll", 0.0);
                        g.show_toast(&tr!("Used %d trait charges!", n), 1.8);
                    }
                })
            } else {
                None
            },
            Bo::r(10).enabled(can).sfx(None),
        );
        y += 54;

        let eq = self.state.equipped_trait;
        let box_rect = Rect::new(rect.x + 10, y, rect.w - 20, rect.bottom() - y - 4);
        if box_rect.h > 40 {
            draw_panel(&mut self.canvas, box_rect, Some(panel_light()), 10, false, None);
            self.push_clip(box_rect);
            let inner_w = box_rect.w - 24;
            let line_h = 20;
            match eq {
                None => {
                    let mut ty = box_rect.y + 12;
                    for text in [tr("No trait equipped."), tr("Equip a trait from the list on the side to get its buffs.")] {
                        for wline in wrap_text(&text, &small, inner_w) {
                            let s = small.render(&wline, if ty == box_rect.y + 12 { WHITE } else { grey() });
                            self.canvas.blit(&s, box_rect.x + 12, ty);
                            ty += line_h;
                        }
                    }
                }
                Some(eq) => {
                    let t = &TRAITS[eq];
                    let labels = [
                        ("money", tr("Money")),
                        ("luck", tr("Luck")),
                        ("secret_luck", tr("Secret+ Luck")),
                        ("mutation", tr("Mutation Chance")),
                        ("auto_speed", tr("Auto Speed")),
                        ("charge_chance", tr("Trait Charge Chance")),
                    ];
                    let items: Vec<String> = labels
                        .iter()
                        .filter(|(k, _)| t.buff(k) != 0.0)
                        .map(|(k, label)| pyformat("+%.0f%% %s", &args![t.buff(k) * 100.0, label]))
                        .collect();
                    let head = small.render(&fit_text(&small, &tr!("Equipped: %s", tr(t.name)), inner_w), WHITE);
                    self.canvas.blit(&head, box_rect.x + 12, box_rect.y + 12);
                    let items_top = box_rect.y + 12 + line_h;
                    let one_col_h = items.len() as i32 * line_h + 12;
                    let cols = if (items_top - box_rect.y) + one_col_h <= box_rect.h { 1 } else { 2 };
                    let col_w = inner_w / cols;
                    let rows = (items.len() as i32 + cols - 1) / cols;
                    for (n_i, text) in items.iter().enumerate() {
                        let n_i = n_i as i32;
                        let (col, row) = (n_i / rows, n_i % rows);
                        let text = fit_text(&small, text, col_w - 8);
                        let s = small.render(&text, grey());
                        self.canvas.blit(&s, box_rect.x + 12 + col * col_w, items_top + row * line_h);
                    }
                }
            }
            self.pop_clip();
        }
    }
}
