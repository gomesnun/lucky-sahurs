//! v3.0 Prestige: the second tab of the Rebirth page. Needs 10 / 15 / 20 / 30 / 40 Rebirths. Resets coins, Rebirths,
//! upgrades (not the Auto Upgrader / Auto Trait Roller) and pets, except ONE verity you pick; gives big permanent
//! Money and Luck multipliers and a perk. The rules are in core/state.rs (do_prestige), the table in core/data.rs.

use super::base::Bo;
use super::{Game, cb};
use crate::core::data::{MUT_ORDER, PRESTIGES, rarities};
use crate::core::formatting::format_number;
use crate::core::state::Pet;
use crate::gfx::{Color, Rect, draw, ti};
use crate::i18n::tr;
use crate::theme::*;
use crate::tr;
use crate::ui::cards::rarity_glow_color;
use crate::ui::drawing::{draw_panel, draw_state_border};
use crate::ui::fonts::{Font, fit_text, wrap_text};
use crate::ui::icons::load_pet_image;
use super::sell_panel::sell_pet_label as pet_label;

pub const PRESTIGE_COLOR: Color = Color::rgb(186, 104, 255);
const ROW_H: i32 = 64;
const ROW_GAP: i32 = 8;
const PICK_ROW_H: i32 = 56;

pub fn roman(n: i64) -> &'static str {
    ["0", "I", "II", "III", "IV", "V"].get(n.clamp(0, 5) as usize).copied().unwrap_or("V")
}

/// "x3 Money, x3 Luck, +1 Equip Slot"
pub fn prestige_gives(i: usize) -> String {
    let p = &PRESTIGES[i];
    tr!("x%s Money, x%s Luck", format_number(p.money), format_number(p.luck)) + ", " + &tr(p.perk)
}

impl Game {
    /// The verity the Prestige will keep: the one you picked (if you still have it), else your best one.
    pub fn prestige_keep_pet(&self) -> Option<Pet> {
        self.prestige_keep.filter(|(r, m)| self.state.count_owned(*r, m) > 0).or_else(|| self.state.best_owned_pet())
    }

    pub fn set_rebirth_tab(&mut self, tab: &'static str) {
        self.rebirth_tab = tab;
        self.rebirth_confirm = false;
        self.prestige_picking = false;
        self.rebirth_scroll = 0.0;
    }

    pub fn do_prestige_clicked(&mut self) {
        if !self.state.prestige_available() {
            return;
        }
        if !self.rebirth_confirm {
            self.rebirth_confirm = true;
            self.rebirth_confirm_timer = 4.0;
            return;
        }
        let keep = self.prestige_keep_pet();
        if self.state.do_prestige(keep) {
            self.rebirth_confirm = false;
            self.prestige_keep = None;
            if self.state.auto_equip_unlocked() && self.state.auto_equip_best_on {
                self.state.equip_best();
            }
            self.play("rebirth", 0.0);
            let n = self.state.prestige;
            let msg = tr!("Prestige %s! Permanent %s.", roman(n), prestige_gives(n as usize - 1));
            self.show_toast(&msg, 4.5);
            self.state.save();
        }
    }

    /// A small "Prestige III" pill whose RIGHT edge is at `right`. Returns its width.
    pub fn draw_prestige_pill(&mut self, right: i32, cy: i32, prestige: i64) -> i32 {
        let font = self.f.tiny_b.clone();
        let txt = font.render(&tr!("Prestige %s", roman(prestige)), WHITE);
        let r = Rect::new(right - txt.w - 16, cy - (txt.h + 6) / 2, txt.w + 16, txt.h + 6);
        draw::rect(&mut self.canvas, Color::rgb(70, 36, 110), r, 0, r.h / 2);
        draw::rect(&mut self.canvas, PRESTIGE_COLOR, r, 2, r.h / 2);
        self.canvas.blit(&txt, r.x + 8, r.y + 3);
        r.w
    }

    /// The Prestige tab's content, below the page title.
    pub fn draw_prestige_body(&mut self, rect: Rect, top: i32, mouse_pos: (f64, f64)) {
        let small = self.f.small.clone();
        let sb = self.f.small_b.clone();
        let med = self.f.med.clone();
        let x0 = rect.x + 26;
        let w = rect.w - 52;
        let mut y = top;
        let sub = tr("A huge PERMANENT boost. Resets your coins, Rebirths, upgrades and pets - you keep 1 verity of your choice. Dice, potions, traits, milestones, the Index, the Auto Upgrader and the Auto Trait Roller all stay.");
        for line in wrap_text(&sub, &small, w) {
            let s = small.render(&line, grey());
            self.canvas.blit(&s, x0, y);
            y += small.get_height() + 2;
        }
        y += 12;

        // ---- what you have now ----
        let card = Rect::new(x0, y, w, 88);
        draw_panel(&mut self.canvas, card, Some(panel_light()), 12, false, None);
        let pr = self.state.prestige;
        let def = self.state.prestige_def();
        let lines = [
            (tr("Prestige"), if pr > 0 { format!("{} / V", roman(pr)) } else { tr("None yet") }),
            (tr("Money / Luck (Prestige)"), def.map_or("x1 / x1".to_string(), |d| format!("x{} / x{}", format_number(d.money), format_number(d.luck)))),
        ];
        let mut ly = card.y + 12;
        for (label, value) in &lines {
            let l = med.render(label, grey());
            let v = med.render(value, PRESTIGE_COLOR);
            self.canvas.blit(&l, card.x + 18, ly);
            self.canvas.blit(&v, card.right() - 18 - v.w, ly);
            ly += 36;
        }
        y = card.bottom() + 16;

        let Some(next) = self.state.next_prestige() else {
            let t = med.render(&tr("You reached the final Prestige!"), PRESTIGE_COLOR);
            self.canvas.blit(&t, rect.centerx() - t.w / 2, y);
            y += t.h + 16;
            self.draw_prestige_list(Rect::new(rect.x + 22, y, rect.w - 44, rect.bottom() - 16 - y), mouse_pos);
            return;
        };
        let can = self.state.prestige_available();
        let need = med.render(
            &tr!("Prestige %s needs %d Rebirths  (you have %d)", roman(pr + 1), next.need, self.state.rebirths),
            if can { WHITE } else { grey() },
        );
        self.canvas.blit(&need, rect.centerx() - need.w / 2, y);
        y += need.h + 10;

        // ---- the verity you keep ----
        let keep_rect = Rect::new(x0, y, w, 52);
        draw::rect(&mut self.canvas, panel_light(), keep_rect, 0, 10);
        let label = sb.render(&tr("Keep:"), grey());
        self.canvas.blit(&label, keep_rect.x + 12, keep_rect.centery() - label.h / 2);
        let mut kx = keep_rect.x + 20 + label.w;
        let change_w = 120;
        match self.prestige_keep_pet() {
            Some((r, m)) => {
                if let Some(img) = load_pet_image(rarities()[r].pet, 40, false) {
                    self.canvas.blit(&img, kx, keep_rect.centery() - img.h / 2);
                    kx += img.w + 8;
                }
                let name = fit_text(&sb, &pet_label(r, m), keep_rect.right() - change_w - 20 - kx);
                let t = sb.render(&name, rarity_glow_color(rarities()[r].key));
                self.canvas.blit(&t, kx, keep_rect.centery() - t.h / 2);
            }
            None => {
                let t = small.render(&tr("You have no verities to keep."), grey());
                self.canvas.blit(&t, kx, keep_rect.centery() - t.h / 2);
            }
        }
        let picking = self.prestige_picking;
        self.button(
            Rect::new(keep_rect.right() - change_w - 8, keep_rect.y + 8, change_w, keep_rect.h - 16),
            &if picking { tr("Done") } else { tr("Change") },
            &sb,
            mouse_pos,
            if picking { accent() } else { panel_lighter() },
            accent_hover(),
            if picking { BLACK } else { WHITE },
            cb(|g| {
                g.prestige_picking = !g.prestige_picking;
                g.rebirth_scroll = 0.0;
            }),
            Bo::r(8),
        );
        y = keep_rect.bottom() + 12;

        // ---- the button ----
        let btn_w = 460.min(w);
        let (label, color) = if self.rebirth_confirm {
            (tr("Click again to confirm - resets coins, Rebirths, upgrades & pets!"), BAD)
        } else {
            (tr!("Prestige %s  (%s)", roman(pr + 1), tr!("x%s Money, x%s Luck", format_number(next.money), format_number(next.luck))), if can { PRESTIGE_COLOR } else { Color::rgb(70, 73, 88) })
        };
        self.button(
            Rect::new(rect.centerx() - btn_w / 2, y, btn_w, 50),
            &label,
            &sb,
            mouse_pos,
            color,
            Color::rgb(206, 140, 255),
            if can { BLACK } else { grey() },
            if can { cb(|g| g.do_prestige_clicked()) } else { None },
            Bo::r(10).enabled(can).sfx(None).icon("prestige"),
        );
        y += 50 + 16;

        let list = Rect::new(rect.x + 22, y, rect.w - 44, rect.bottom() - 16 - y);
        if self.prestige_picking {
            self.draw_prestige_picker(list, mouse_pos);
        } else {
            self.draw_prestige_list(list, mouse_pos);
        }
    }

    /// The 5 Prestiges, what each needs and gives.
    fn draw_prestige_list(&mut self, rect: Rect, mouse_pos: (f64, f64)) {
        let sb = self.f.small_b.clone();
        let small = self.f.small.clone();
        let head = self.f.med.render(&tr!("Prestiges  (%d/%d)", self.state.prestige, PRESTIGES.len() as i64), PRESTIGE_COLOR);
        self.canvas.blit(&head, rect.x + 4, rect.y);
        let list = Rect::new(rect.x, rect.y + head.h + 8, rect.w, rect.h - head.h - 8);
        self.rebirth_list_rect = list;
        let content_h = (PRESTIGES.len() as i32 * (ROW_H + ROW_GAP) - ROW_GAP) as f64;
        self.rebirth_max_scroll = (content_h - list.h as f64).max(0.0);
        self.rebirth_scroll = self.rebirth_scroll.clamp(0.0, self.rebirth_max_scroll);
        let scroll = self.rebirth_scroll;
        self.push_clip(list);
        for (i, p) in PRESTIGES.iter().enumerate() {
            let row = Rect::new(list.x, ti((list.y + i as i32 * (ROW_H + ROW_GAP)) as f64 - scroll), list.w - 20, ROW_H);
            if row.bottom() < list.top() || row.top() > list.bottom() {
                continue;
            }
            let done = self.state.prestige > i as i64;
            let is_next = self.state.prestige == i as i64;
            draw_panel(&mut self.canvas, row, Some(panel_light()), 10, false, None);
            if done {
                draw_state_border(&mut self.canvas, row, PRESTIGE_COLOR, 10, 2);
            } else if is_next {
                draw_state_border(&mut self.canvas, row, accent(), 10, 2);
            }
            let ready = is_next && self.state.rebirths >= p.need;
            let status = if done {
                tr("Done")
            } else if ready {
                tr("Ready!")
            } else {
                format!("{} / {}", if is_next { self.state.rebirths } else { 0 }, p.need)
            };
            let st = sb.render(&status, if done { PRESTIGE_COLOR } else if ready { GOOD } else { grey() });
            self.canvas.blit(&st, row.right() - 14 - st.w, row.y + 10);
            let title = sb.render(&fit_text(&sb, &tr!("%s  -  %d Rebirths", tr(p.name), p.need), row.w - 40 - st.w), if done || is_next { WHITE } else { grey() });
            self.canvas.blit(&title, row.x + 14, row.y + 10);
            let gives = small.render(&fit_text(&small, &prestige_gives(i), row.w - 28), if done { PRESTIGE_COLOR } else { grey() });
            self.canvas.blit(&gives, row.x + 14, row.y + 12 + title.h);
        }
        self.pop_clip();
        self.draw_scrollbar(list, scroll, content_h, Some("rebirth"), Some(mouse_pos));
    }

    /// Pick the verity to keep: every one you own, best first.
    fn draw_prestige_picker(&mut self, rect: Rect, mouse_pos: (f64, f64)) {
        let head = self.f.med.render(&tr("Choose the verity to keep"), PRESTIGE_COLOR);
        self.canvas.blit(&head, rect.x + 4, rect.y);
        let list = Rect::new(rect.x, rect.y + head.h + 8, rect.w, rect.h - head.h - 8);
        let mut pets: Vec<(f64, Pet, i64)> = Vec::new();
        for r in 0..rarities().len() {
            for m in MUT_ORDER {
                let n = self.state.count_owned(r, m);
                if n > 0 {
                    pets.push((self.state.pet_income(r, m), (r, m), n));
                }
            }
        }
        pets.sort_by(|a, b| b.0.partial_cmp(&a.0).unwrap_or(std::cmp::Ordering::Equal));
        self.rebirth_list_rect = list;
        let content_h = (pets.len() as i32 * (PICK_ROW_H + 6) - 6).max(0) as f64;
        self.rebirth_max_scroll = (content_h - list.h as f64).max(0.0);
        self.rebirth_scroll = self.rebirth_scroll.clamp(0.0, self.rebirth_max_scroll);
        let scroll = self.rebirth_scroll;
        let chosen = self.prestige_keep_pet();
        let sb = self.f.small_b.clone();
        let tiny = self.f.tiny.clone();
        self.push_clip(list);
        for (i, (income, (r, m), n)) in pets.iter().enumerate() {
            let row = Rect::new(list.x, ti((list.y + i as i32 * (PICK_ROW_H + 6)) as f64 - scroll), list.w - 20, PICK_ROW_H);
            if row.bottom() < list.top() || row.top() > list.bottom() {
                continue;
            }
            let sel = chosen == Some((*r, *m));
            let (r, m) = (*r, *m);
            self.button(row, "", &sb, mouse_pos, if sel { panel_lighter() } else { panel_light() }, panel_lighter(), WHITE, cb(move |g| g.prestige_keep = Some((r, m))), Bo::r(10).border(if sel { Some(PRESTIGE_COLOR) } else { None }));
            self.draw_pick_row(row, r, m, *n, *income, &sb, &tiny);
        }
        self.pop_clip();
        self.draw_scrollbar(list, scroll, content_h, Some("rebirth"), Some(mouse_pos));
    }

    fn draw_pick_row(&mut self, row: Rect, r: usize, m: &str, n: i64, income: f64, sb: &Font, tiny: &Font) {
        let mut x = row.x + 10;
        if let Some(img) = load_pet_image(rarities()[r].pet, 40, false) {
            self.canvas.blit(&img, x, row.centery() - img.h / 2);
            x += img.w + 10;
        }
        let right = tiny.render(&tr!("have %s  -  $%s/sec each", format_number(n as f64), format_number(income)), grey());
        self.canvas.blit(&right, row.right() - 12 - right.w, row.centery() - right.h / 2);
        let name = sb.render(&fit_text(sb, &pet_label(r, m), row.right() - 24 - right.w - x), rarity_glow_color(rarities()[r].key));
        self.canvas.blit(&name, x, row.y + 8);
        let rar = tiny.render(&tr(rarities()[r].name), grey_dim());
        self.canvas.blit(&rar, x, row.y + 10 + name.h);
    }
}
