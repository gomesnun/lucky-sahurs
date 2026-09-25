//! Shop (left side button, from Rebirth 1): a page with the Dice and Potions tabs, the potions part of the Bag
//! and each die's ROLL button style (ui/shop_panel.py). The logic is in core/shop.rs.

use super::base::Bo;
use super::{Game, cb};
use crate::config::{TOPBAR_H, VIRTUAL_H};
use crate::core::formatting::{format_number, py_round};
use crate::core::shop::{DICE, DiceStyle, POTION_COMBINE, POTION_LEVELS, POTION_TYPES, ROMAN, SHOP_UNLOCK_REBIRTHS, dice_by_key, potion_by_key, potion_id, shop_seconds_left};
use crate::gfx::{Color, Rect, draw, ti};
use crate::i18n::tr;
use crate::online::firebase::server_now;
use crate::theme::*;
use crate::tr;
use crate::ui::drawing::{dim_overlay, draw_panel, draw_rainbow_border, draw_state_border};
use crate::ui::fonts::fit_text;
use crate::ui::icons::load_icon;

pub fn fmt_clock(seconds: f64) -> String {
    let s = seconds.max(0.0) as i64;
    format!("{}:{:02}", s / 60, s % 60)
}

const LUCK_GREEN: Color = Color::rgb(120, 230, 140);
const BUY_HOVER: Color = Color::rgb(120, 235, 150);

pub struct ShopUi {
    pub open: bool,
    pub tab: &'static str,
    pub scroll: f64,
    pub max_scroll: f64,
    pub list_rect: Rect,
}

impl ShopUi {
    pub fn new() -> ShopUi {
        ShopUi { open: false, tab: "dice", scroll: 0.0, max_scroll: 0.0, list_rect: Rect::ZERO }
    }
}

/// The highlight of a shop card: a plain colour or the striped rainbow.
enum Highlight {
    None,
    Color(Color),
    Rainbow,
}

impl Game {
    /// The server's time: the stock changes at the same moment for everyone.
    pub fn shop_now(&self) -> f64 {
        server_now()
    }

    // ---------------------------------------------------------------- open / close
    pub fn toggle_shop(&mut self) {
        if self.shop.open {
            self.close_shop();
            return;
        }
        if !self.state.shop_unlocked() {
            self.show_toast(&tr!("The Shop unlocks at Rebirth %d.", SHOP_UNLOCK_REBIRTHS), 1.8);
            return;
        }
        self.close_overlays();
        self.shop.open = true;
        self.shop.scroll = 0.0;
        self.right_panel.close();
        self.left_panel.close();
    }

    pub fn close_shop(&mut self) {
        self.shop.open = false;
    }

    // ---------------------------------------------------------------- page
    pub fn draw_shop_page(&mut self, mouse_pos: (f64, f64)) {
        let ov = dim_overlay(self.vw, VIRTUAL_H, 170);
        self.canvas.blit(&ov, 0, 0);
        let panel_w = 640.max((self.vw - 240).min(1100));
        let top = TOPBAR_H + 12;
        let rect = Rect::new(self.vw / 2 - panel_w / 2, top, panel_w, VIRTUAL_H - top - 14);
        draw_panel(&mut self.canvas, rect, Some(panel()), 16, true, None);
        self.register_blocker(rect);

        let sb = self.f.small_b.clone();
        let small = self.f.small.clone();
        let title = self.f.big.render(&tr("Shop"), WHITE);
        self.canvas.blit(&title, rect.x + 26, rect.y + 16);
        self.button(Rect::new(rect.right() - 48, rect.y + 18, 30, 30), "X", &sb, mouse_pos, panel_light(), BAD, WHITE, cb(|g| g.close_shop()), Bo::r(8));
        let now = self.shop_now();
        let restock = tr!("New stock in %s  ·  the same stock for every player", fmt_clock(shop_seconds_left(now)));
        let sub = small.render(&fit_text(&small, &restock, panel_w - 60), grey());
        self.canvas.blit(&sub, rect.x + 26, rect.y + 18 + title.h);

        let ty = rect.y + 26 + title.h + sub.h;
        let tab_w = 180;
        for (n, (key, label, icon)) in [("dice", tr("Dice"), "shop/dice_gold"), ("potions", tr("Potions"), "shop/potion_luck")].into_iter().enumerate() {
            let trect = Rect::new(rect.x + 24 + n as i32 * (tab_w + 10), ty, tab_w, 38);
            let active = self.shop.tab == key;
            self.button(
                trect,
                &label,
                &sb,
                mouse_pos,
                if active { accent() } else { panel_light() },
                if !active { accent_hover() } else { accent() },
                if active { BLACK } else { WHITE },
                cb(move |g| {
                    g.shop.tab = key;
                    g.shop.scroll = 0.0;
                }),
                Bo::r(9).icon(icon),
            );
        }
        let coins = sb.render(&format!("${}", format_number(self.state.coins)), GOOD);
        let r = Rect::with_midright(coins.w, coins.h, (rect.right() - 28, ty + 19));
        self.canvas.blit(&coins, r.x, r.y);

        let content = Rect::new(rect.x + 12, ty + 50, rect.w - 24, rect.bottom() - 16 - (ty + 50));
        self.shop.list_rect = content;
        self.push_clip(content);
        let content_h = if self.shop.tab == "dice" { self.draw_shop_dice(content, mouse_pos, now) } else { self.draw_shop_potions(content, mouse_pos, now) };
        self.pop_clip();
        self.shop.max_scroll = (content_h as f64 - content.h as f64).max(0.0);
        self.shop.scroll = self.shop.scroll.clamp(0.0, self.shop.max_scroll);
        let scroll = self.shop.scroll;
        self.draw_scrollbar(content, scroll, content_h as f64, Some("shop"), Some(mouse_pos));
    }

    /// A generic card: image, name, lines of text. Returns the free rect at the bottom (for the button).
    fn shop_card(&mut self, crect: Rect, icon: &str, name: &str, lines: &[(String, Color)], status: Option<(String, Color)>, highlight: Highlight) -> Rect {
        draw_panel(&mut self.canvas, crect, Some(panel_light()), 12, false, None);
        match highlight {
            Highlight::Rainbow => draw_rainbow_border(&mut self.canvas, crect.inflate(-6, -6), 10, 3),
            Highlight::Color(c) => draw_state_border(&mut self.canvas, crect, c, 12, 3),
            Highlight::None => {}
        }
        if let Some(img) = load_icon(icon, 84) {
            let r = Rect::with_midtop(img.w, img.h, (crect.centerx(), crect.y + 8));
            self.canvas.blit(&img, r.x, r.y);
        }
        let mut y = crect.y + 96;
        let sb = self.f.small_b.clone();
        let tiny = self.f.tiny.clone();
        let nt = sb.render(&fit_text(&sb, name, crect.w - 12), WHITE);
        let r = Rect::with_midtop(nt.w, nt.h, (crect.centerx(), y));
        self.canvas.blit(&nt, r.x, r.y);
        y += nt.h + 2;
        for (text, color) in lines {
            let t = tiny.render(&fit_text(&tiny, text, crect.w - 12), *color);
            let r = Rect::with_midtop(t.w, t.h, (crect.centerx(), y));
            self.canvas.blit(&t, r.x, r.y);
            y += t.h + 1;
        }
        if let Some((text, color)) = status {
            let t = self.f.tiny_b.render(&text, color);
            let r = Rect::with_midtop(t.w, t.h, (crect.centerx(), y + 2));
            self.canvas.blit(&t, r.x, r.y);
        }
        Rect::new(crect.x + 8, crect.bottom() - 40, crect.w - 16, 32)
    }

    fn draw_shop_dice(&mut self, content: Rect, mouse_pos: (f64, f64), now: f64) -> i32 {
        let gap = 12;
        let cols = 3.max(5.min((content.w - 20 + gap).div_euclid(170 + gap)));
        let card_w = (content.w - 20 - gap * (cols - 1)) / cols;
        let card_h = 232;
        let top = content.y as f64 + 4.0 - self.shop.scroll;
        let sb = self.f.small_b.clone();
        for (n, d) in DICE.iter().enumerate() {
            let (col, row) = (n as i32 % cols, n as i32 / cols);
            let crect = Rect::new(content.x + 4 + col * (card_w + gap), ti(top + (row * (card_h + gap)) as f64), card_w, card_h);
            if crect.bottom() < content.top() || crect.top() > content.bottom() {
                continue;
            }
            let owned = self.state.shop.dice_owned.contains(&d.key);
            let equipped = self.state.shop.dice_equipped == Some(d.key);
            let in_stock = self.state.shop_in_stock(&format!("dice:{}", d.key), now);
            let status = if owned {
                (if equipped { tr("EQUIPPED") } else { tr("OWNED") }, GOOD)
            } else if in_stock {
                (tr("IN STOCK: 1"), accent())
            } else {
                (tr("NO STOCK"), grey_dim())
            };
            let lines = [
                (tr!("+%d%% Luck", py_round(d.luck * 100.0) as i64), LUCK_GREEN),
                (tr!("%d%% Double Roll", py_round(d.double * 100.0) as i64), accent()),
                (tr!("%d%% stock chance", py_round(d.chance * 100.0) as i64), grey()),
            ];
            let highlight = if !equipped {
                Highlight::None
            } else if d.style.effect == Some("rainbow") {
                Highlight::Rainbow
            } else {
                Highlight::Color(d.style.border)
            };
            let brect = self.shop_card(crect, &format!("shop/dice_{}", d.key), &tr(d.name), &lines, Some(status), highlight);
            let key = d.key;
            if owned {
                self.button(
                    brect,
                    &if equipped { tr("Unequip") } else { tr("Equip") },
                    &sb,
                    mouse_pos,
                    if equipped { panel_lighter() } else { accent() },
                    if equipped { BAD } else { accent_hover() },
                    if equipped { WHITE } else { BLACK },
                    cb(move |g| g.state.equip_dice(key)),
                    Bo::r(8).sfx(Some("equip")),
                );
            } else {
                let can = in_stock && self.state.coins >= d.price;
                self.button(
                    brect,
                    &format!("${}", format_number(d.price)),
                    &sb,
                    mouse_pos,
                    if can { GOOD } else { panel() },
                    BUY_HOVER,
                    if can { BLACK } else { grey_dim() },
                    if can {
                        cb(move |g| {
                            let now = g.shop_now();
                            if g.state.buy_dice(key, now) {
                                g.play("buy", 0.0);
                                let name = dice_by_key(key).map(|d| tr(d.name)).unwrap_or_default();
                                g.show_toast(&tr!("Bought %s! It's yours forever.", name), 1.8);
                            }
                        })
                    } else {
                        None
                    },
                    Bo::r(8).enabled(can).sfx(None),
                );
            }
        }
        let rows = (DICE.len() as i32 + cols - 1) / cols;
        rows * (card_h + gap) + 8
    }

    fn draw_shop_potions(&mut self, content: Rect, mouse_pos: (f64, f64), now: f64) -> i32 {
        let gap = 10;
        let card_w = (content.w - 20 - gap * (POTION_LEVELS as i32 - 1)) / POTION_LEVELS as i32;
        let card_h = 226;
        let mut y = content.y as f64 + 4.0 - self.shop.scroll;
        let med = self.f.med.clone();
        let sb = self.f.small_b.clone();
        for p in POTION_TYPES.iter() {
            let head = med.render(&tr(p.name), WHITE);
            if y + head.h as f64 >= content.top() as f64 && y <= content.bottom() as f64 {
                self.canvas.blit(&head, content.x + 6, ti(y));
            }
            y += (head.h + 6) as f64;
            for lvl in 1..=POTION_LEVELS {
                let crect = Rect::new(content.x + 4 + (lvl as i32 - 1) * (card_w + gap), ti(y), card_w, card_h);
                if crect.bottom() < content.top() || crect.top() > content.bottom() {
                    continue;
                }
                let pid = potion_id(p.key, lvl);
                let in_stock = self.state.shop_in_stock(&format!("potion:{}", pid), now);
                let have = self.state.shop.potions.get(&pid).copied().unwrap_or(0);
                let lines = [
                    (tr!("x%g %s", p.effect[(lvl - 1) as usize], tr(p.stat)), LUCK_GREEN),
                    (tr!("You have: %d", have), grey()),
                ];
                let status = if in_stock { (tr("IN STOCK: 1"), accent()) } else { (tr("NO STOCK"), grey_dim()) };
                let name = format!("{} {}", tr(p.name), ROMAN[(lvl - 1) as usize]);
                let brect = self.shop_card(crect, &format!("shop/potion_{}", p.key), &name, &lines, Some(status), Highlight::None);
                let price = self.state.potion_price(p.key, lvl);
                let can = in_stock && self.state.coins >= price;
                let key = p.key;
                self.button(
                    brect,
                    &format!("${}", format_number(price)),
                    &sb,
                    mouse_pos,
                    if can { GOOD } else { panel() },
                    BUY_HOVER,
                    if can { BLACK } else { grey_dim() },
                    if can {
                        cb(move |g| {
                            let now = g.shop_now();
                            if g.state.buy_potion(key, lvl, now) {
                                g.play("buy", 0.0);
                            }
                        })
                    } else {
                        None
                    },
                    Bo::r(8).enabled(can).sfx(None),
                );
            }
            y += (card_h + 16) as f64;
        }
        let tiny = self.f.tiny.clone();
        let t = tiny.render(&tr!("Potions go to your Bag (Potions). Each lasts %d min of play; %d of one level combine into 1 of the next.", 10i64, POTION_COMBINE), grey());
        self.canvas.blit(&t, content.x + 6, ti(y));
        ti(y + self.shop.scroll - content.y as f64) + 30
    }

    // ---------------------------------------------------------------- Bag: potions
    /// The Bag's Potions view: the active ones (with their time) and the ones you have, with Use and Combine.
    /// Returns the height.
    pub fn draw_bag_potions(&mut self, rect: Rect, content_rect: Rect, scroll: f64, mouse_pos: (f64, f64)) -> i32 {
        let pad = 14;
        let mut y = content_rect.top() as f64 + 4.0 - scroll;
        let w = rect.w - pad * 2 - 6;
        let sb = self.f.small_b.clone();
        let small = self.f.small.clone();
        let tiny = self.f.tiny.clone();
        if !self.state.shop.active_potions.is_empty() {
            let t = sb.render(&tr("Active"), grey());
            self.canvas.blit(&t, rect.x + pad, ti(y));
            y += (t.h + 4) as f64;
            for (kind, (lvl, left)) in self.state.shop.active_potions.clone() {
                let Some(p) = potion_by_key(kind) else { continue };
                let line = format!(
                    "{} {}  ·  x{} {}  ·  {}",
                    tr(p.name),
                    ROMAN[(lvl - 1) as usize],
                    crate::pyfmt::format("%g", &crate::args![p.effect[(lvl - 1) as usize]]),
                    tr(p.stat),
                    fmt_clock(left)
                );
                let t = small.render(&fit_text(&small, &line, w), LUCK_GREEN);
                self.canvas.blit(&t, rect.x + pad, ti(y));
                y += (t.h + 3) as f64;
            }
            y += 8.0;
        }
        let owned: Vec<(&'static crate::core::shop::PotionType, i64)> = POTION_TYPES
            .iter()
            .flat_map(|p| (1..=POTION_LEVELS).map(move |l| (p, l)))
            .filter(|(p, l)| self.state.shop.potions.get(&potion_id(p.key, *l)).copied().unwrap_or(0) > 0)
            .collect();
        if owned.is_empty() {
            let t = small.render(&tr("No potions yet. Buy them in the Shop."), grey());
            self.canvas.blit(&t, rect.x + pad, ti(y));
            return ti(y + scroll - content_rect.top() as f64) + 40;
        }
        let row_h = 96;
        for (p, lvl) in owned {
            let row = Rect::new(rect.x + pad, ti(y), w, row_h - 6);
            y += row_h as f64;
            if row.bottom() < content_rect.top() || row.top() > content_rect.bottom() {
                continue;
            }
            draw_panel(&mut self.canvas, row, Some(panel_light()), 10, false, None);
            if let Some(img) = load_icon(&format!("shop/potion_{}", p.key), 50) {
                self.canvas.blit(&img, row.x + 6, row.y + 6);
            }
            let n = self.state.shop.potions.get(&potion_id(p.key, lvl)).copied().unwrap_or(0);
            let text_w = row.w - 64 - 8;
            let name = self.texto_que_cabe(&format!("{} {}  x{}", tr(p.name), ROMAN[(lvl - 1) as usize], n), &sb, WHITE, text_w);
            self.canvas.blit(&name, row.x + 62, row.y + 8);
            let eff_txt = tr!("x%g %s for %d min", p.effect[(lvl - 1) as usize], tr(p.stat), 10i64);
            let eff = tiny.render(&fit_text(&tiny, &eff_txt, text_w), grey());
            self.canvas.blit(&eff, row.x + 62, row.y + 10 + name.h);
            // buttons underneath, side by side: Use | Combine 5 > 1
            let bw = (row.w - 18) / 2;
            let use_rect = Rect::new(row.x + 6, row.bottom() - 32, bw, 26);
            let can_use = !self.state.shop.active_potions.get(p.key).is_some_and(|c| c.0 > lvl);
            let key = p.key;
            self.button(
                use_rect,
                &tr("Use"),
                &sb,
                mouse_pos,
                GOOD,
                BUY_HOVER,
                BLACK,
                if can_use {
                    cb(move |g| {
                        if g.state.use_potion(key, lvl) {
                            g.play("buy", 0.0);
                        }
                    })
                } else {
                    None
                },
                Bo::r(7).enabled(can_use).sfx(None),
            );
            if lvl < POTION_LEVELS {
                let can_comb = n >= POTION_COMBINE;
                let comb_rect = Rect::new(use_rect.right() + 6, use_rect.y, row.w - 18 - bw, 26);
                let pname = p.name;
                self.button(
                    comb_rect,
                    &tr!("Combine %d > 1", POTION_COMBINE),
                    &sb,
                    mouse_pos,
                    if can_comb { accent() } else { panel() },
                    accent_hover(),
                    if can_comb { BLACK } else { grey_dim() },
                    if can_comb {
                        cb(move |g| {
                            if g.state.combine_potions(key, lvl) {
                                g.play("buy", 0.0);
                                g.show_toast(&tr!("Combined %d into 1 %s %s!", POTION_COMBINE, tr(pname), ROMAN[lvl as usize]), 1.8);
                            }
                        })
                    } else {
                        None
                    },
                    Bo::r(7).enabled(can_comb).sfx(None),
                );
            }
        }
        ti(y + scroll - content_rect.top() as f64) + 10
    }

    // ---------------------------------------------------------------- active potions (main screen)
    /// A small row with the active potions and the time left (under the bonus bars).
    pub fn draw_active_potions(&mut self, x_center: i32, y: i32) -> i32 {
        if self.state.shop.active_potions.is_empty() {
            return y;
        }
        let tb = self.f.tiny_b.clone();
        let rendered: Vec<(&'static str, _)> =
            self.state.shop.active_potions.iter().map(|(kind, (lvl, left))| (*kind, tb.render(&format!("{}  {}", ROMAN[(*lvl - 1) as usize], fmt_clock(*left)), WHITE))).collect();
        let widths: Vec<i32> = rendered.iter().map(|(_, t)| 22 + 4 + t.w).collect();
        let total = widths.iter().sum::<i32>() + 14 * (widths.len() as i32 - 1);
        let mut x = x_center - total / 2;
        for ((kind, t), wdt) in rendered.iter().zip(widths) {
            let pill = Rect::new(x - 4, y, wdt + 8, 24);
            draw::rect(&mut self.canvas, panel_light(), pill, 0, 12);
            draw::rect(&mut self.canvas, outline(), pill, 2, 12);
            if let Some(img) = load_icon(&format!("shop/potion_{}", kind), 22) {
                self.canvas.blit(&img, x, y + 1);
            }
            self.canvas.blit(t, x + 26, y + 12 - t.h / 2);
            x += wdt + 14;
        }
        y + 30
    }

    // ---------------------------------------------------------------- the ROLL button with the die's style
    /// The equipped die's ROLL button style, or None without a die.
    pub fn roll_button_colors(&self) -> Option<&'static DiceStyle> {
        self.state.roll_button_style()
    }

    /// On top of the ROLL button: the die's border, the special effect and the little die on the left.
    pub fn draw_roll_style_extras(&mut self, rect: Rect, style: &DiceStyle) {
        if style.effect == Some("rainbow") {
            draw_rainbow_border(&mut self.canvas, rect.inflate(-9, -9), 10, 4);
        } else {
            draw_state_border(&mut self.canvas, rect, style.border, 14, 3);
        }
        if style.effect == Some("stars") && self.animations() {
            let t = crate::core::state::now_ts();
            for i in 0..7 {
                let a = t * 0.8 + i as f64 * 0.9;
                let sx = rect.x + 14 + ((i * 37) as i64 + (t * 18.0) as i64).rem_euclid(1.max(rect.w - 28) as i64) as i32;
                let sy = rect.y + 10 + ((a.sin() * 0.5 + 0.5) * (rect.h - 20) as f64) as i32;
                let r = if i % 2 != 0 { 2 } else { 3 };
                draw::circle(&mut self.canvas, Color::rgb(230, 220, 255), (sx, sy), r, 0);
            }
        }
        if let Some(key) = self.state.shop.dice_equipped {
            if let Some(img) = load_icon(&format!("shop/dice_{}", key), rect.h - 18) {
                let r = Rect::with_midleft(img.w, img.h, (rect.x + 10, rect.centery()));
                self.canvas.blit(&img, r.x, r.y);
            }
        }
    }
}
