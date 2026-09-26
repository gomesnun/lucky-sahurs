//! Evolving pets (stacking): each pet's arrow button in the Inventory opens this page. Stacking enough copies of one
//! verity into it raises every copy of it to the next phase: Phase 1 -> Phase 2 -> Phase 3 -> Monster.
//! The logic (costs, used-up copies, unequipping, online wallet) is in core/state.rs (evolve).

use super::base::Bo;
use super::sell_panel::sell_pet_label;
use super::{Game, KeyEv, cb};
use crate::config::VIRTUAL_H;
use crate::core::data::{MAX_PHASE, PHASES, rarities};
use crate::core::formatting::format_number;
use crate::gfx::{Color, Rect, draw};
use crate::i18n::tr;
use crate::theme::*;
use crate::tr;
use crate::ui::cards::render_pet_card_phase;
use crate::ui::drawing::{dim_overlay, draw_panel};
use crate::ui::fonts::{fit_text, wrap_text};
use crate::ui::icons::load_icon;
use std::rc::Rc;

pub struct EvolveUi {
    /// (pet index, mutation, phase) being evolved, or None = page closed
    pub target: Option<(usize, &'static str, usize)>,
}

impl EvolveUi {
    pub fn new() -> EvolveUi {
        EvolveUi { target: None }
    }
}

impl Game {
    pub fn open_evolve(&mut self, rarity_index: usize, m: &'static str, phase: usize) {
        self.evolve.target = Some((rarity_index, m, phase));
    }

    pub fn close_evolve(&mut self) {
        self.evolve.target = None;
    }

    /// Stacks the copies and evolves; the page stays open (following the same stack to its new phase) so you can
    /// keep going.
    pub fn press_evolve(&mut self) {
        let Some((r, m, phase)) = self.evolve.target else { return };
        let Some(new_phase) = self.state.evolve(r, m, phase) else { return };
        // stay on this stack while it still has copies (to keep fusing them), else follow the new one
        if self.state.count_owned_at(r, m, phase) <= 0 {
            self.evolve.target = Some((r, m, new_phase));
        }
        let phase = new_phase;
        if self.state.auto_equip_unlocked() && self.state.auto_equip_best_on {
            self.state.equip_best();
        }
        let t = if phase == MAX_PHASE {
            tr!("%s became a MONSTER!", sell_pet_label(r, m))
        } else {
            tr!("%s evolved to %s!", sell_pet_label(r, m), tr(PHASES[phase].name))
        };
        self.show_toast(&t, 2.4);
    }

    /// Evolves every pet you can afford to, as many times as each can afford. Bag > Inventory > "Fuse All".
    pub fn press_fuse_all(&mut self) {
        let n = self.state.evolve_all();
        if n <= 0 {
            self.show_toast(&tr("Nothing to fuse - you need more copies of a pet first."), 1.8);
            return;
        }
        if self.state.auto_equip_unlocked() && self.state.auto_equip_best_on {
            self.state.equip_best();
        }
        self.show_toast(&tr!("Fused %d time(s)!", n), 2.2);
    }

    /// Keys with the evolve page open. Returns true if the key was used.
    pub fn handle_evolve_key(&mut self, ev: KeyEv) -> bool {
        use sdl2::keyboard::Keycode as K;
        if self.evolve.target.is_none() {
            return false;
        }
        match ev.key {
            K::Escape => self.close_evolve(),
            K::Return | K::KpEnter => self.press_evolve(),
            _ => return false,
        }
        true
    }

    pub fn draw_evolve_page(&mut self, mouse_pos: (f64, f64)) {
        let Some((r_idx, m, phase)) = self.evolve.target else { return };
        let owned = self.state.count_owned_at(r_idx, m, phase);
        if owned <= 0 {
            self.close_evolve();
            return;
        }
        let ov = dim_overlay(self.vw, VIRTUAL_H, 170);
        self.canvas.blit(&ov, 0, 0);
        let panel_w = 560.min(self.vw - 40);
        let panel_h = 520;
        let rect = Rect::new(self.vw / 2 - panel_w / 2, VIRTUAL_H / 2 - panel_h / 2, panel_w, panel_h);
        draw_panel(&mut self.canvas, rect, Some(panel()), 16, true, None);
        self.register_button(rect, Rc::new(|_: &mut Game| {}), None); // clicking inside doesn't close

        let x0 = rect.x + 24;
        let w = panel_w - 48;
        let mut tx = x0;
        if let Some(icon) = load_icon("evolve", 36) {
            self.canvas.blit(&icon, x0, rect.y + 18);
            tx += 44;
        }
        let big = self.f.big.clone();
        let sb = self.f.small_b.clone();
        let small = self.f.small.clone();
        let tiny = self.f.tiny.clone();
        let med = self.f.med.clone();
        let title = big.render(&fit_text(&big, &tr("Evolve"), w - 60), WHITE);
        self.canvas.blit(&title, tx, rect.y + 20);
        self.button(Rect::new(rect.right() - 46, rect.y + 20, 28, 28), "X", &sb, mouse_pos, panel_light(), BAD, WHITE, cb(|g| g.close_evolve()), Bo::r(8));

        // ---- now -> next ----
        let cost = self.state.evolve_cost(r_idx, m, phase);
        let top = rect.y + 72;
        let (card_w, card_h) = (150, 170);
        let rarity = &rarities()[r_idx];
        if cost.is_some() {
            let gap = 70;
            let left = rect.centerx() - card_w - gap / 2;
            let now = render_pet_card_phase(rarity, m, phase, card_w, card_h, &[], 0, false);
            self.canvas.blit(&now, left, top);
            let next = render_pet_card_phase(rarity, m, phase + 1, card_w, card_h, &[], 0, false);
            self.canvas.blit(&next, left + card_w + gap, top);
            let arrow = med.render(">>", PHASES[phase + 1].color);
            self.canvas.blit(&arrow, rect.centerx() - arrow.w / 2, top + card_h / 2 - arrow.h / 2);
        } else {
            let now = render_pet_card_phase(rarity, m, phase, card_w, card_h, &[], 0, false);
            self.canvas.blit(&now, rect.centerx() - card_w / 2, top);
        }

        // ---- details ----
        let mut y = top + card_h + 14;
        let income_now = self.state.pet_income(r_idx, m, phase);
        let mut rows: Vec<(String, String, Color)> = vec![(tr("Pet"), sell_pet_label(r_idx, m), WHITE)];
        match cost {
            Some(c) => {
                let next_income = income_now * PHASES[phase + 1].mult / PHASES[phase].mult;
                rows.push((tr("Phase"), format!("{}  >  {}", tr(PHASES[phase].name), tr(PHASES[phase + 1].name)), PHASES[phase + 1].color));
                rows.push((tr("Copies to stack"), tr!("%s (you have %s)", format_number(c as f64), format_number(owned as f64)), if owned >= c { GOOD } else { BAD }));
                rows.push((tr("Income each"), format!("+{}/s  >  +{}/s", format_number(income_now), format_number(next_income)), GOOD));
            }
            None => {
                rows.push((tr("Phase"), tr(PHASES[phase].name), PHASES[phase].color));
                rows.push((tr("Income each"), format!("+{}/s", format_number(income_now)), GOOD));
            }
        }
        for (label, value, color) in &rows {
            let lt = small.render(label, grey());
            let vt = sb.render(&fit_text(&sb, value, w - lt.w - 12), *color);
            self.canvas.blit(&lt, x0, y);
            self.canvas.blit(&vt, rect.right() - 24 - vt.w, y);
            draw::line(&mut self.canvas, panel_light(), (x0, y + 28), (rect.right() - 24, y + 28), 1);
            y += 34;
        }
        let hint = match cost {
            Some(c) => tr!("Uses up %d copies of this phase to make 1 at the next. New copies you roll start at Phase 1.", c as i64),
            None => tr("Fully evolved: this is its Monster form."),
        };
        for line in wrap_text(&hint, &tiny, w) {
            let t = tiny.render(&line, grey_dim());
            self.canvas.blit(&t, x0, y);
            y += 16;
        }

        // ---- Evolve ----
        let btn = Rect::new(x0, rect.bottom() - 24 - 50, w, 50);
        match cost {
            Some(c) if owned >= c => {
                let label = if phase + 1 == MAX_PHASE { tr!("Become a Monster (uses %s)", format_number(c as f64)) } else { tr!("Evolve (uses %s)", format_number(c as f64)) };
                self.button(btn, &label, &med, mouse_pos, PHASES[phase + 1].color, accent_hover(), BLACK, cb(|g| g.press_evolve()), Bo::r(12).icon("evolve").sfx(Some("milestone")));
            }
            Some(c) => {
                let label = tr!("Need %s more", format_number((c - owned) as f64));
                self.button(btn, &label, &med, mouse_pos, panel_light(), panel_light(), grey(), None, Bo::r(12).enabled(false));
            }
            None => {
                self.button(btn, &tr("Close"), &med, mouse_pos, panel_light(), panel_lighter(), WHITE, cb(|g| g.close_evolve()), Bo::r(12));
            }
        }
    }
}
