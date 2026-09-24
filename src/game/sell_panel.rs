//! Selling pets (ui/sell_panel.py): each pet's "$" button in the Inventory opens this page. Type how many to sell
//! (more than you have sells them all) or press "Sell All", then Sell -> "Are you sure?" -> sold. The logic
//! (price, unequipping, online wallet) is in core/state.rs (sell_pets).

use super::base::{Bo, FieldRef};
use super::{Game, KeyEv, cb};
use crate::config::VIRTUAL_H;
use crate::core::data::{mutation, rarities};
use crate::core::formatting::format_number;
use crate::gfx::{Color, Rect, draw};
use crate::i18n::tr;
use crate::theme::*;
use crate::tr;
use crate::ui::cards::render_pet_card;
use crate::ui::drawing::{dim_overlay, draw_panel};
use crate::ui::fonts::fit_text;
use crate::ui::icons::load_icon;
use crate::ui::widgets::TextField;
use std::rc::Rc;

/// the "Are you sure?" goes back to "Sell" if not pressed again in time
pub const SELL_CONFIRM_SECONDS: f64 = 4.0;

pub struct SellUi {
    /// (pet index, mutation) being sold, or None = page closed
    pub target: Option<(usize, &'static str)>,
    pub field: TextField,
    pub focus: bool,
    /// "Sell All" on: sells them all, no typing needed
    pub all: bool,
    /// true = the button is at "Are you sure?"
    pub confirm: bool,
    pub confirm_timer: f64,
}

impl SellUi {
    pub fn new() -> SellUi {
        SellUi { target: None, field: TextField::new("digits"), focus: false, all: false, confirm: false, confirm_timer: 0.0 }
    }
}

/// "Golden Eternity" (the verity's name isn't translated; the mutation is).
pub fn sell_pet_label(rarity_index: usize, m: &str) -> String {
    let label = mutation(m).map(|x| x.label).unwrap_or("");
    let prefix = if label.is_empty() { String::new() } else { tr(label) };
    format!("{} {}", prefix, rarities()[rarity_index].pet).trim().to_string()
}

impl Game {
    // ---------------------------------------------------------------- open / close
    pub fn open_sell(&mut self, rarity_index: usize, m: &'static str) {
        self.sell.target = Some((rarity_index, m));
        self.sell.field.set_text("");
        self.sell.all = false;
        self.sell.confirm = false;
        self.set_sell_focus(true);
    }

    pub fn close_sell(&mut self) {
        self.sell.target = None;
        self.sell.confirm = false;
        self.set_sell_focus(false);
    }

    pub fn set_sell_focus(&mut self, on: bool) {
        if on == self.sell.focus {
            return;
        }
        self.sell.focus = on;
        if on {
            self.start_text_input();
        } else {
            self.stop_text_input();
        }
    }

    pub fn tick_sell(&mut self, dt: f64) {
        if self.sell.confirm {
            self.sell.confirm_timer -= dt;
            if self.sell.confirm_timer <= 0.0 {
                self.sell.confirm = false;
            }
        }
    }

    // ---------------------------------------------------------------- amount
    /// How many pets will be sold now (never more than you have).
    pub fn sell_amount(&self) -> i64 {
        let Some((r, m)) = self.sell.target else { return 0 };
        let owned = self.state.count_owned(r, m);
        if self.sell.all {
            return owned;
        }
        let text = self.sell.field.text.trim();
        if text.is_empty() { 0 } else { owned.min(text.parse::<i64>().unwrap_or(i64::MAX)) }
    }

    /// Typing in the field turns off "Sell All" and "Are you sure?" (the amount changed).
    pub fn sell_type(&mut self, text: &str) {
        let before = self.sell.field.text.clone();
        self.sell.field.add(text);
        if self.sell.field.text != before {
            self.sell.all = false;
            self.sell.confirm = false;
        }
    }

    pub fn toggle_sell_all(&mut self) {
        self.sell.all = !self.sell.all;
        self.sell.confirm = false;
        if self.sell.all {
            self.sell.field.set_text("");
            self.set_sell_focus(false);
        }
    }

    /// 1st click: asks "Are you sure?". 2nd click (in time): sells.
    pub fn press_sell(&mut self) {
        let amount = self.sell_amount();
        if amount <= 0 {
            return;
        }
        if !self.sell.confirm {
            self.sell.confirm = true;
            self.sell.confirm_timer = SELL_CONFIRM_SECONDS;
            return;
        }
        let Some((r, m)) = self.sell.target else { return };
        let (sold, gain) = self.state.sell_pets(r, m, amount);
        if sold > 0 && self.state.auto_equip_unlocked() && self.state.auto_equip_best_on {
            self.state.equip_best();
        }
        self.close_sell();
        if sold > 0 {
            let t = tr!("Sold %s %s for $%s!", format_number(sold as f64), sell_pet_label(r, m), format_number(gain));
            self.show_toast(&t, 2.2);
        }
    }

    // ---------------------------------------------------------------- keys
    /// Keys with the sell page open. Returns true if the key was used.
    pub fn handle_sell_key(&mut self, ev: KeyEv) -> bool {
        use sdl2::keyboard::Keycode as K;
        if self.sell.target.is_none() {
            return false;
        }
        match ev.key {
            K::Escape => {
                self.close_sell();
                return true;
            }
            K::Return | K::KpEnter => {
                self.press_sell();
                return true;
            }
            _ => {}
        }
        if !self.sell.focus {
            return false;
        }
        if ev.key == K::V && ev.ctrl {
            let t = self.clipboard_text();
            self.sell_type(&t);
            return true;
        }
        if !self.edit_field_key(FieldRef::SellAmount, ev, true) {
            return false;
        }
        self.sell.all = false;
        self.sell.confirm = false;
        true
    }

    // ---------------------------------------------------------------- drawing
    pub fn draw_sell_page(&mut self, mouse_pos: (f64, f64)) {
        let Some((r_idx, m)) = self.sell.target else { return };
        let owned = self.state.count_owned(r_idx, m);
        if owned <= 0 {
            // none left (a trade, another sale...): nothing to sell
            self.close_sell();
            return;
        }
        let ov = dim_overlay(self.vw, VIRTUAL_H, 170);
        self.canvas.blit(&ov, 0, 0);
        let panel_w = 520.min(self.vw - 40);
        let panel_h = 430;
        let rect = Rect::new(self.vw / 2 - panel_w / 2, VIRTUAL_H / 2 - panel_h / 2, panel_w, panel_h);
        draw_panel(&mut self.canvas, rect, Some(panel()), 16, true, None);
        self.register_button(rect, Rc::new(|g: &mut Game| g.set_sell_focus(false)), None); // clicking inside doesn't close

        // ---- title ----
        let x0 = rect.x + 24;
        let w = panel_w - 48;
        let mut tx = x0;
        if let Some(icon) = load_icon("sell", 36) {
            self.canvas.blit(&icon, x0, rect.y + 18);
            tx += 44;
        }
        let big = self.f.big.clone();
        let sb = self.f.small_b.clone();
        let small = self.f.small.clone();
        let med = self.f.med.clone();
        let title = big.render(&fit_text(&big, &tr("Sell Pets"), w - 60), WHITE);
        self.canvas.blit(&title, tx, rect.y + 20);
        self.button(Rect::new(rect.right() - 46, rect.y + 20, 28, 28), "X", &sb, mouse_pos, panel_light(), BAD, WHITE, cb(|g| g.close_sell()), Bo::r(8));

        // ---- the pet's card (left) + details (right) ----
        let top = rect.y + 72;
        let (card_w, card_h) = (140, 160);
        let card = render_pet_card(&rarities()[r_idx], m, card_w, card_h, &[], 0, false);
        self.canvas.blit(&card, x0, top);

        let info_x = x0 + card_w + 20;
        let info_w = rect.right() - 24 - info_x;
        let price = self.state.sell_price(r_idx, m);
        let rows = [
            (tr("Pet"), sell_pet_label(r_idx, m)),
            (tr("Have"), format_number(owned as f64)),
            (tr("Equipped"), format_number(self.state.equipped_count(r_idx, m) as f64)),
            (tr("Price each"), format!("${}", format_number(price))),
        ];
        let mut y = top + 4;
        for (label, value) in &rows {
            let lt = small.render(label, grey());
            let vt = sb.render(&fit_text(&sb, value, info_w - lt.w - 12), WHITE);
            self.canvas.blit(&lt, info_x, y);
            self.canvas.blit(&vt, rect.right() - 24 - vt.w, y);
            draw::line(&mut self.canvas, panel_light(), (info_x, y + 30), (rect.right() - 24, y + 30), 1);
            y += 38;
        }

        // ---- amount: text field + Sell All ----
        let mut y = top + card_h + 16;
        let t = sb.render(&tr("Amount to sell"), grey_dim());
        self.canvas.blit(&t, x0, y);
        y += t.h + 6;
        let all_w = 150;
        let field_rect = Rect::new(x0, y, w - all_w - 10, 44);
        let placeholder = if self.sell.all { tr!("All (%s)", format_number(owned as f64)) } else { tr("Type a number...") };
        let display = self.sell.field.text.clone();
        let focus = self.sell.focus;
        self.draw_text_field(field_rect, FieldRef::SellAmount, &display, &placeholder, focus, Rc::new(|g: &mut Game| g.set_sell_focus(true)));
        let all = self.sell.all;
        self.button(
            Rect::new(field_rect.right() + 10, y, all_w, 44),
            &tr("Sell All"),
            &sb,
            mouse_pos,
            if all { accent() } else { panel_light() },
            if all { accent() } else { panel_lighter() },
            if all { BLACK } else { WHITE },
            cb(|g| g.toggle_sell_all()),
            Bo::r(10),
        );
        y += 44 + 8;

        let amount = self.sell_amount();
        let typed = self.sell.field.text.trim().to_string();
        let hint = if !typed.is_empty() && !all && typed.parse::<i64>().map(|v| v > owned).unwrap_or(true) {
            tr!("You only have %s - all of them will be sold.", format_number(owned as f64))
        } else {
            tr!("You get: $%s", format_number(price * amount as f64))
        };
        let ht = small.render(&fit_text(&small, &hint, w), if amount > 0 { GOOD } else { grey() });
        self.canvas.blit(&ht, x0, y);

        // ---- Sell -> Are you sure? ----
        let btn = Rect::new(x0, rect.bottom() - 24 - 50, w, 50);
        if self.sell.confirm {
            self.button(btn, &tr!("Are you sure? Sell %s", format_number(amount as f64)), &med, mouse_pos, BAD, Color::rgb(235, 90, 90), WHITE, cb(|g| g.press_sell()), Bo::r(12).icon("sell"));
        } else {
            let label = if amount > 0 { tr!("Sell %s", format_number(amount as f64)) } else { tr("Sell") };
            self.button(btn, &label, &med, mouse_pos, GOOD, accent_hover(), BLACK, cb(|g| g.press_sell()), Bo::r(12).enabled(amount > 0).icon("sell"));
        }
    }
}
