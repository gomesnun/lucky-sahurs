//! Main screen: top bar, side buttons, main card and Stats (ui/game_screen.py).

use super::base::{Bo, blit_center, blit_midtop};
use super::{Game, cb};
use crate::config::{TOPBAR_H, VIRTUAL_H};
use crate::core::data::{INDEX_ENTRIES, MILESTONES, mutation, rarities};
use crate::core::formatting::{format_number, format_one_in, format_playtime};
use crate::core::state::now_ts;
use crate::gfx::{Color, Rect, Surface, draw, ti, transform};
use crate::i18n::tr;
use crate::pyfmt::format as pyformat;
use crate::theme::*;
use crate::tr;
use crate::ui::cards::{cell, rarity_glow_color, render_pet_card};
use crate::ui::drawing::{
    bar_fill_surface, dim_overlay, draw_panel, draw_rainbow_border, draw_state_border, ease_out_back, rainbow_glow_surface, rarity_glow,
};
use crate::ui::fonts::{fit_text, is_light};
use crate::ui::icons::{load_icon, load_pet_image};
use std::rc::Rc;

const HINT_H: i32 = 0;

impl Game {
    pub fn open_stats(&mut self) {
        if self.options_open {
            self.close_options();
        }
        self.traits_open = false;
        self.rebirth_open = false;
        self.rebirth_confirm = false;
        self.leaderboard_open = false;
        self.credits_open = false;
        self.stats_open = true;
    }
    pub fn close_stats(&mut self) {
        self.stats_open = false;
    }
    pub fn toggle_stats(&mut self) {
        if self.stats_open {
            self.close_stats();
        } else {
            self.open_stats();
        }
    }

    pub fn main_center_x(&self) -> i32 {
        let left = if self.left_panel.visible() { self.left_panel.shown_width(self.left_w) } else { 0 };
        let right = self.vw - if self.right_panel.visible() { self.right_panel.shown_width(self.right_w) } else { 0 };
        (left + right).div_euclid(2)
    }

    pub fn draw_game_screen(&mut self, mouse_pos: (f64, f64)) {
        self.draw_verity_rain(); // v3.0: your verities drifting down behind everything
        self.draw_topbar(mouse_pos);
        self.draw_main(mouse_pos);
        if self.animations() {
            let ps = std::mem::take(&mut self.particles);
            for p in &ps {
                p.draw(&mut self.canvas);
            }
            self.particles = ps;
            self.draw_roll_pops();
        }
        self.right_rect = Rect::ZERO;
        self.left_rect = Rect::ZERO;
        let panel_top = TOPBAR_H;
        let panel_h = VIRTUAL_H - TOPBAR_H - HINT_H;
        if self.left_panel.visible() {
            let shown = self.left_panel.shown_width(self.left_w);
            let rect = Rect::new(shown - self.left_w, panel_top, self.left_w, panel_h);
            self.left_rect = rect;
            self.draw_bag_panel(rect, mouse_pos);
        }
        if self.right_panel.visible() {
            let shown = self.right_panel.shown_width(self.right_w);
            let rect = Rect::new(self.vw - shown, panel_top, self.right_w, panel_h);
            self.right_rect = rect;
            match self.right_panel.content {
                Some("tree") => self.draw_tree_panel(rect, mouse_pos),
                Some("milestones") => self.draw_milestones_panel(rect, mouse_pos),
                Some("daily") => self.draw_daily_panel(rect, mouse_pos),
                _ => self.draw_index_panel(rect, mouse_pos),
            }
        }
        self.draw_side_buttons(mouse_pos);
        if self.traits_open {
            self.begin_modal();
            self.draw_traits_page(mouse_pos);
        }
        if self.index_open {
            self.begin_modal();
            self.draw_index_page(mouse_pos);
        }
        if self.shop.open {
            self.begin_modal();
            self.draw_shop_page(mouse_pos);
        }
        if self.rebirth_open {
            self.begin_modal();
            self.draw_rebirth_page(mouse_pos);
        }
        self.draw_toast();
    }

    pub fn draw_topbar(&mut self, mouse_pos: (f64, f64)) {
        let rect = Rect::new(0, 0, self.vw, TOPBAR_H);
        draw::rect(&mut self.canvas, panel(), rect, 0, 0);
        draw::line(&mut self.canvas, outline(), (0, TOPBAR_H), (self.vw, TOPBAR_H), 2 * BORDER_W - 3);
        let mut off = 0;
        if let Some(cash) = load_icon("cash", 54) {
            self.canvas.blit(&cash, 12, (TOPBAR_H - 54) / 2);
            off = 58;
        }
        let shown = self.coins_display.unwrap_or(self.state.coins);
        let coins = self.f.big.render(&format!("$ {}", format_number(shown)), GOOD);
        self.canvas.blit(&coins, 22 + off, 11);
        let dps = self.f.small.render(&tr!("%s / sec", format_number(self.state.income_per_second())), grey());
        self.canvas.blit(&dps, 25 + off, 44);
        let pets = self.f.small.render(&tr!("·   %d / %d pets equipped", self.state.equipped.len() as i64, self.state.max_slots()), grey());
        self.canvas.blit(&pets, 25 + off + dps.w + 14, 44);

        self.nav_mode = true;
        let med = self.f.med.clone();
        let friends_rect = Rect::new(self.vw - 472, 15, 146, 40);
        self.button(friends_rect, &tr("Friends"), &med, mouse_pos, panel_light(), panel_lighter(), WHITE, cb(|g| g.toggle_friends()), Bo::default().icon("friends"));
        // red dot: unanswered friend requests + conversations with unread messages
        let pedidos = self.friends_pending_count() + self.chat_unread_count() as i64;
        if pedidos > 0 {
            self.draw_friends_badge(friends_rect.topright(), pedidos);
        }
        let stats_rect = Rect::new(self.vw - 320, 15, 146, 40);
        self.button(stats_rect, &tr("Stats"), &med, mouse_pos, panel_light(), panel_lighter(), WHITE, cb(|g| g.toggle_stats()), Bo::default().icon("stats"));
        let opt_rect = Rect::new(self.vw - 168, 15, 146, 40);
        self.button(opt_rect, &tr("Options"), &med, mouse_pos, panel_light(), panel_lighter(), WHITE, cb(|g| g.toggle_options()), Bo::default().icon("options"));
        self.nav_mode = false;
        self.draw_event_banner();
    }

    pub fn draw_stats(&mut self, mouse_pos: (f64, f64)) {
        let d = dim_overlay(self.vw, VIRTUAL_H, 170);
        self.canvas.blit(&d, 0, 0);
        let st = &self.state;
        let total_ms: usize = MILESTONES.iter().map(|m| m.tiers.len()).sum();
        let lines: Vec<(String, String)> = vec![
            (tr("Save"), tr!("Slot %d", st.slot.unwrap_or(0))),
            (tr("Coins"), format!("${}", format_number(st.coins))),
            (tr("Income"), tr!("%s / sec", format_number(st.income_per_second()))),
            (tr("Total Coins Earned"), format!("${}", format_number(st.total_coins_earned))),
            (tr("Total Rolls"), format_number(st.total_rolls as f64)),
            (tr("Equipped Slots"), format!("{}/{}", st.equipped.len(), st.max_slots())),
            (tr("Playtime"), format_playtime(st.playtime)),
            (tr("Indexed Pets"), format!("{}/{}", st.indexed_pets_count(), INDEX_ENTRIES)),
            (tr("Traits Rolled"), format_number(st.total_traits_rolled as f64)),
            (tr("Rebirths"), format_number(st.rebirths as f64)),
            (tr("Milestones Claimed"), format!("{}/{}", st.milestones_claimed.len(), total_ms)),
        ];
        let row_h = 34;
        let panel_w = 440;
        let panel_h = 84 + row_h * lines.len() as i32 + 16;
        let rect = Rect::new(self.vw / 2 - panel_w / 2, VIRTUAL_H / 2 - panel_h / 2, panel_w, panel_h);
        draw_panel(&mut self.canvas, rect, Some(panel()), 16, true, None);
        self.register_blocker(rect);
        let title = self.f.big.render(&tr("Stats"), WHITE);
        self.canvas.blit(&title, rect.x + 24, rect.y + 20);
        let close = Rect::new(rect.right() - 46, rect.y + 20, 28, 28);
        let sb = self.f.small_b.clone();
        self.button(close, "X", &sb, mouse_pos, panel_light(), BAD, WHITE, cb(|g| g.close_stats()), Bo::r(8));
        let mut y = rect.y + 72;
        for (label, value) in &lines {
            let l = self.f.med.render(label, grey());
            let v = self.f.med.render(value, WHITE);
            self.canvas.blit(&l, rect.x + 24, y);
            self.canvas.blit(&v, rect.right() - 24 - v.w, y);
            draw::line(&mut self.canvas, panel_light(), (rect.x + 24, y + row_h - 6), (rect.right() - 24, y + row_h - 6), 1);
            y += row_h;
        }
    }

    pub fn open_right_panel(&mut self, content: &'static str) {
        self.close_overlays();
        if content == "milestones" {
            self.milestones_selected_category = None;
            self.milestones_selected_group = None;
        } else if content == "tree" {
            self.tree_selected_category = None;
        }
        self.right_panel.toggle(content);
    }

    pub fn open_left_panel(&mut self, content: &'static str) {
        self.close_overlays();
        self.left_panel.toggle(content);
    }

    pub fn draw_side_buttons(&mut self, mouse_pos: (f64, f64)) {
        let size = 62;
        let step = size + 38;
        let labels = [tr("INDEX"), tr("UPGRADES"), tr("MILESTONES"), tr("QUESTS"), tr("BAG"), tr("REBIRTH"), tr("TRAITS"), tr("SHOP")];
        let mut lf = self.f.small_b.clone();
        if labels.iter().map(|t| lf.render(t, WHITE).w).max().unwrap_or(0) > size + 24 {
            lf = self.f.tiny_b.clone();
        }
        self.nav_mode = true;
        let shown_r = if self.right_panel.visible() { self.right_panel.shown_width(self.right_w) } else { 0 };
        let x = self.vw - shown_r - size - 18;
        let label_h = 26;
        let column_top = |n: i32| VIRTUAL_H / 2 - ((n - 1) * step + size + label_h) / 2;
        let rp_open = self.right_panel.is_open();
        let rc = self.right_panel.content;
        let y = column_top(4);
        let idx_open = self.index_open;
        self.side_button(Rect::new(x, y, size, size), &labels[0], "index", mouse_pos, idx_open, Rc::new(|g: &mut Game| g.toggle_index_page()), Some(lf.clone()), 0, false);
        let afford = self.state.affordable_upgrades_count() as i64;
        self.side_button(Rect::new(x, y + step, size, size), &labels[1], "tree", mouse_pos, rp_open && rc == Some("tree"), Rc::new(|g: &mut Game| g.open_right_panel("tree")), Some(lf.clone()), afford, false);
        self.side_button(Rect::new(x, y + 2 * step, size, size), &labels[2], "milestones", mouse_pos, rp_open && rc == Some("milestones"), Rc::new(|g: &mut Game| g.open_right_panel("milestones")), Some(lf.clone()), 0, false);
        let quests_ready = self.quests_claimable();
        self.side_button(Rect::new(x, y + 3 * step, size, size), &labels[3], "daily", mouse_pos, rp_open && rc == Some("daily"), Rc::new(|g: &mut Game| g.open_right_panel("daily")), Some(lf.clone()), 0, quests_ready);

        let shown_l = if self.left_panel.visible() { self.left_panel.shown_width(self.left_w) } else { 0 };
        let lx = shown_l + 18;
        let y = column_top(4); // the left column has 4 buttons (Bag, Rebirth, Traits, Shop)
        let lp_open = self.left_panel.is_open();
        self.side_button(Rect::new(lx, y, size, size), &labels[4], "bag", mouse_pos, lp_open, Rc::new(|g: &mut Game| g.open_left_panel("bag")), Some(lf.clone()), 0, false);
        let rb_open = self.rebirth_open;
        let rb_avail = self.state.rebirth_available() || self.state.prestige_available();
        self.side_button(Rect::new(lx, y + step, size, size), &labels[5], "rebirth", mouse_pos, rb_open, Rc::new(|g: &mut Game| g.toggle_rebirth()), Some(lf.clone()), 0, rb_avail);
        let tr_open = self.traits_open;
        self.side_button(Rect::new(lx, y + 2 * step, size, size), &labels[6], "trait", mouse_pos, tr_open, Rc::new(|g: &mut Game| g.toggle_traits()), Some(lf.clone()), 0, false);
        // Shop: locked until Rebirth 1 (the button shows anyway, dimmed)
        let shop_rect = Rect::new(lx, y + 3 * step, size, size);
        let sh_open = self.shop.open;
        self.side_button(shop_rect, &labels[7], "shop", mouse_pos, sh_open, Rc::new(|g: &mut Game| g.toggle_shop()), Some(lf.clone()), 0, false);
        if !self.state.shop_unlocked() {
            let mut veil = Surface::new_alpha(shop_rect.w, shop_rect.h);
            let vr = veil.get_rect();
            draw::rect(&mut veil, Color::rgba(10, 12, 20, 150), vr, 0, 12);
            self.canvas.blit(&veil, shop_rect.x, shop_rect.y);
        }
        self.nav_mode = false;
    }

    pub fn main_card_rect(&self) -> Rect {
        let (card_w, card_h) = (236, 236);
        let cx = self.main_center_x();
        let cy = VIRTUAL_H / 2;
        Rect::new(cx - card_w / 2, cy - card_h / 2, card_w, card_h)
    }

    pub fn active_roll_luck(&self) -> (f64, Option<&'static str>) {
        let st = &self.state;
        let (mut total, mut top) = (0.0, None);
        for (key, unlocked, ready, mult) in [
            ("golden", true, st.cyclic_bonus_ready, st.golden_roll_mult()),
            ("diamond", st.diamond_roll_unlocked(), st.diamond_bonus_ready, st.diamond_roll_mult()),
            ("rainbow", st.rainbow_roll_unlocked(), st.rainbow_bonus_ready, st.rainbow_roll_mult()),
        ] {
            if !unlocked || st.is_cycle_paused(key) {
                continue;
            }
            if ready || self.bar_continuous.get(key).copied().unwrap_or(false) {
                total += mult;
                top = Some(key);
            }
        }
        (total, top)
    }

    /// With the Auto Roller doing more rolls than can be shown (e.g. a x1M speed event), the middle card says so:
    /// the rolls per second and the best pet since it got that fast.
    fn draw_too_fast_card(&mut self, rect: Rect) {
        let t = now_ts();
        draw_panel(&mut self.canvas, rect, Some(panel_light()), 12, true, None);
        let pulse = 0.5 + 0.5 * (t * 6.0).sin();
        let a = accent();
        let mixc = |c1: u8| (c1 as f64 + (255.0 - c1 as f64) * pulse) as i64 as u8;
        draw_state_border(&mut self.canvas, rect, Color::rgb(mixc(a.r), mixc(a.g), mixc(a.b)), 12, 3);
        let mut y = rect.y + 18;
        let big = self.f.big.clone();
        for line in [tr("TOO FAST"), tr("TO SHOW!")] {
            let txt = big.render(&line, accent());
            blit_midtop(&mut self.canvas, &txt, (rect.centerx(), y));
            y += txt.h - 4;
        }
        let rps = self.f.small_b.render(&tr!("%s rolls / sec", format_number(self.state.auto_rolls_per_second())), WHITE);
        blit_midtop(&mut self.canvas, &rps, (rect.centerx(), y + 6));
        if let Some((r_idx, m)) = self.too_fast_best {
            let rarity = &rarities()[r_idx];
            let label = self.f.tiny_b.render(&tr("Best so far:"), grey());
            blit_midtop(&mut self.canvas, &label, (rect.centerx(), rect.bottom() - 86));
            let img = load_pet_image(rarity.pet, 44, false);
            let ml = mutation(m).map(|x| x.label).unwrap_or("");
            let name = if ml.is_empty() { rarity.pet.to_string() } else { format!("{} {}", tr(ml), rarity.pet) };
            let sb = self.f.small_b.clone();
            let nt = sb.render(&fit_text(&sb, &name, rect.w - 70), rarity_glow_color(rarity.key));
            let total_w = if img.is_some() { 44 + 6 } else { 0 } + nt.w;
            let mut x = rect.centerx() - total_w / 2;
            if let Some(img) = img {
                self.canvas.blit(&img, x, rect.bottom() - 64);
                x += 50;
            }
            self.canvas.blit(&nt, x, rect.bottom() - 64 + 22 - nt.h / 2);
            let rt = self.f.tiny.render(&tr(rarity.name), grey());
            blit_midtop(&mut self.canvas, &rt, (rect.centerx(), rect.bottom() - 18));
        }
    }

    pub fn draw_main(&mut self, mouse_pos: (f64, f64)) {
        let center_x = self.main_center_x();
        let card_rect = self.main_card_rect();
        let (card_w, card_h) = card_rect.size();

        if now_ts() < self.too_fast_until {
            self.draw_too_fast_card(card_rect); // the Auto Roller is too fast to show each pet
        } else if let Some((r_idx, m)) = self.state.last_roll {
            let rarity = &rarities()[r_idx];
            let income = self.state.pet_income(r_idx, m);
            let chance = self.state.combined_chance(r_idx, m, None, None);
            let plates = vec![vec![cell(tr("Income"), tr!("+%s/sec", format_number(income)))], vec![cell(tr("Roll Chance"), format_one_in(chance))]];
            let card = render_pet_card(rarity, m, card_w, card_h, &plates, 0, false);
            let c = rarity.color;
            let glow_color = if c.b > 60 || is_light(c) { c } else { Color::rgb(90, 90, 110) };
            let glow = rarity_glow((card_w, card_h), glow_color, 30);
            let glow_rect = Rect::with_center(glow.w, glow.h, card_rect.center());
            let mut zoom = None;
            let mut anim_elapsed = None;
            if self.animations() {
                let elapsed = now_ts() - self.roll_anim_start;
                // the better the rarity, the bigger the "pop" (and it lasts a little longer)
                let dur = if rarity.tier < 3 { 0.18 } else { 0.32 };
                if elapsed < dur {
                    let t = elapsed / dur;
                    zoom = Some(if rarity.tier < 3 { 0.72 + 0.28 * t } else { 0.6 + 0.4 * ease_out_back(t) });
                }
                if elapsed < 0.6 {
                    anim_elapsed = Some(elapsed);
                }
            }
            self.canvas.blit(&glow, glow_rect.x, glow_rect.y);
            let card: Rc<Surface> = match zoom {
                Some(z) => Rc::new(transform::smoothscale(&card, 1.max((card_w as f64 * z) as i32), 1.max((card_h as f64 * z) as i32))),
                None => card,
            };
            blit_center(&mut self.canvas, &card, card_rect.center());
            if self.animations() && zoom.is_none() {
                // v3.0: a subtle glare sweeps across the card now and then
                if let Some(phase) = crate::ui::fx::glare_phase(now_ts(), card_rect, 4.5, 0.9) {
                    if let Some(g) = crate::ui::fx::glare_band(card_rect.w, card_rect.h, 14, phase, 48) {
                        self.canvas.blit(&g, card_rect.x, card_rect.y);
                    }
                }
            }
            if let Some(ae) = anim_elapsed.filter(|_| rarity.tier >= 4) {
                // a quick white flash on the card (the big ring around it was removed in v3.0)
                if ae < 0.12 {
                    let mut flash = Surface::new_alpha(card_rect.w, card_rect.h);
                    let fr = flash.get_rect();
                    draw::rect(&mut flash, Color::rgba(255, 255, 255, (170.0 * (1.0 - ae / 0.12)) as i32 as u8), fr, 0, 14);
                    self.canvas.blit(&flash, card_rect.x, card_rect.y);
                }
            }
        } else {
            draw_panel(&mut self.canvas, card_rect, Some(panel_light()), 12, true, None);
            let t = self.f.med.render(&tr("Click ROLL to start!"), grey());
            blit_center(&mut self.canvas, &t, card_rect.center());
        }

        if self.state.auto_unlocked() {
            let rps = self.state.auto_rolls_per_second();
            let on = self.state.auto_on;
            let auto_rect = Rect::new(center_x - 130, card_rect.top() - 24 - 40, 260, 40);
            let label = tr!("Auto Roller: %s", if on { tr("ON") } else { tr("OFF") });
            let sb = self.f.small_b.clone();
            self.button(
                auto_rect,
                &label,
                &sb,
                mouse_pos,
                if on { Color::rgb(52, 120, 80) } else { panel_light() },
                panel_lighter(),
                WHITE,
                cb(|g| {
                    g.state.auto_on = !g.state.auto_on;
                    g.auto_accum = 0.0;
                }),
                Bo::r(10),
            );
            let sub = self.f.small.render(&tr!("%.2f rolls / sec", rps), grey());
            blit_center(&mut self.canvas, &sub, (center_x, auto_rect.top() - 14));
        }

        let st = &self.state;
        let bonus_kind = if st.rainbow_roll_unlocked() && st.rainbow_bonus_ready && !st.is_cycle_paused("rainbow") {
            Some("rainbow")
        } else if st.diamond_roll_unlocked() && st.diamond_bonus_ready && !st.is_cycle_paused("diamond") {
            Some("diamond")
        } else if st.cyclic_bonus_ready && !st.is_cycle_paused("golden") {
            Some("golden")
        } else {
            None
        };

        let roll_rect = Rect::new(center_x - 130, card_rect.bottom() + 24, 260, 60);
        if let Some(kind) = bonus_kind {
            let pulse = 0.6 + 0.4 * (now_ts() * 6.0).sin();
            let step = 1.max(8.min(crate::core::formatting::py_round(pulse * 8.0) as i32));
            let pulse = step as f64 / 8.0;
            let (gw, gh) = (roll_rect.w + 20, roll_rect.h + 20);
            let gpos = (roll_rect.x - 10, roll_rect.y - 10);
            if kind == "rainbow" {
                let glow = rainbow_glow_surface(gw, gh, 20);
                self.canvas.blit_with_alpha(&glow, gpos.0, gpos.1, (120.0 * pulse) as i32);
            } else {
                let (gc, ga) = if kind == "diamond" { (DIAMOND_BORDER, 130.0) } else { (GOLD_BORDER, 90.0) };
                let mut glow = Surface::new_alpha(gw, gh);
                let r = glow.get_rect();
                draw::rect(&mut glow, Color::rgba(gc.r, gc.g, gc.b, (ga * pulse) as i32 as u8), r, 0, 20);
                self.canvas.blit(&glow, gpos.0, gpos.1);
            }
        }
        let (luck_total, luck_top) = self.active_roll_luck();
        let mut luck_parts: Option<(Rc<Surface>, Rc<Surface>, i32)> = None;
        if luck_total > 0.0 {
            let luck_color = match luck_top {
                Some("rainbow") => Color::rgb(255, 140, 220),
                Some("diamond") => DIAMOND_BORDER,
                _ => GOLD_BORDER,
            };
            let s = format!("\u{d7}{}", format_number(luck_total));
            let mut num = self.f.med.render(&s, luck_color);
            if num.w > 60 {
                num = self.f.small_b.render(&s, luck_color);
            }
            let lab = self.f.tiny.render(&tr("LUCK"), WHITE);
            luck_parts = Some((lab, num, roll_rect.right() - 42));
        }
        let mut roll_txt = self.f.huge.render(&tr("ROLL"), WHITE);
        let mut roll_cx = roll_rect.centerx();
        if let Some((lab, num, bx)) = &luck_parts {
            let block_left = bx - lab.w.max(num.w) / 2;
            let limit = block_left - 18;
            if roll_cx + roll_txt.w / 2 > limit {
                roll_cx = limit - roll_txt.w / 2;
                if roll_cx - roll_txt.w / 2 < roll_rect.x + 10 {
                    roll_txt = self.f.big.render(&tr("ROLL"), WHITE);
                    roll_cx = roll_rect.centerx().min(limit - roll_txt.w / 2);
                }
            }
        }
        // equipped die (Shop): the button takes its colours, border and the little die on the left
        let dice_style = self.roll_button_colors();
        let (roll_base, roll_hover) = match dice_style {
            Some(ds) => (ds.base, ds.hover),
            None => (panel_light(), panel_lighter()),
        };
        if let Some(ds) = dice_style {
            let roll_font = if roll_txt.h < self.f.huge.get_height() { self.f.big.clone() } else { self.f.huge.clone() };
            roll_txt = roll_font.render(&tr("ROLL"), ds.text);
            if roll_cx - roll_txt.w / 2 < roll_rect.x + roll_rect.h - 4 {
                // doesn't cover the die
                roll_cx = roll_rect.x + roll_rect.h - 4 + roll_txt.w / 2;
            }
        }
        let huge = self.f.huge.clone();
        self.button(roll_rect, "", &huge, mouse_pos, roll_base, roll_hover, WHITE, cb(|g| g.do_roll()), Bo::r(14).sfx(None));
        blit_center(&mut self.canvas, &roll_txt, (roll_cx, roll_rect.centery()));
        if let Some(ds) = dice_style {
            self.draw_roll_style_extras(roll_rect, ds);
        }
        match bonus_kind {
            Some("rainbow") => draw_rainbow_border(&mut self.canvas, roll_rect.inflate(-9, -9), 10, 3),
            Some("diamond") => draw_state_border(&mut self.canvas, roll_rect, DIAMOND_BORDER, 14, 3),
            Some("golden") => draw_state_border(&mut self.canvas, roll_rect, GOLD_BORDER, 14, 3),
            _ => {}
        }
        if let Some((lab, num, bx)) = &luck_parts {
            let gap = -2;
            let block_h = lab.h + gap + num.h;
            let by = roll_rect.centery() - block_h.div_euclid(2);
            blit_midtop(&mut self.canvas, lab, (*bx, by));
            blit_midtop(&mut self.canvas, num, (*bx, by + lab.h + gap));
        }

        let mut y = roll_rect.bottom() + 16;
        let st = &self.state;
        let golden = (st.cyclic_bonus_ready, st.golden_roll_mult(), st.rolls_until_golden_roll(), st.golden_roll_every());
        y = self.draw_cycle_line("golden", y, golden.0, "Golden Roll", golden.1, golden.2, golden.3, GOLD_BORDER, center_x, mouse_pos);
        if self.state.diamond_roll_unlocked() {
            let st = &self.state;
            let d = (st.diamond_bonus_ready, st.diamond_roll_mult(), st.rolls_until_diamond_roll().unwrap_or(0), st.diamond_roll_every());
            y = self.draw_cycle_line("diamond", y, d.0, "Diamond Roll", d.1, d.2, d.3, DIAMOND_BORDER, center_x, mouse_pos);
        }
        if self.state.rainbow_roll_unlocked() {
            let st = &self.state;
            let d = (st.rainbow_bonus_ready, st.rainbow_roll_mult(), st.rolls_until_rainbow_roll().unwrap_or(0), st.rainbow_roll_every());
            y = self.draw_cycle_line("rainbow", y, d.0, "Rainbow Roll", d.1, d.2, d.3, Color::rgb(255, 140, 220), center_x, mouse_pos);
        }

        // active potions (Shop): a "pill" per potion with its level and the time left
        self.draw_active_potions(center_x, y + 4);
    }

    #[allow(clippy::too_many_arguments)]
    fn draw_cycle_line(&mut self, key: &'static str, y: i32, ready: bool, label: &str, mult: f64, left: i64, every: i64, color: Color, center_x: i32, mouse_pos: (f64, f64)) -> i32 {
        let pill = Rect::new(center_x - 150, y, 300, 34);
        let paused = self.state.is_cycle_paused(key);
        let cycles_per_sec = self.roll_rate / every.max(1) as f64;
        let mut cont = self.bar_continuous.get(key).copied().unwrap_or(false);
        if paused {
            cont = false;
        } else if cont && cycles_per_sec < 1.2 {
            cont = false;
        } else if !cont && cycles_per_sec >= 2.0 {
            cont = true;
        }
        self.bar_continuous.insert(key, cont);
        let target = if ready || cont { 1.0 } else { (1.0 - left as f64 / every.max(1) as f64).max(0.0) };
        let mut shown = self.bar_display.get(key).copied().unwrap_or(target);
        if target < shown - 0.02 {
            shown = target;
        } else {
            shown += (target - shown) * (self.frame_dt * 16.0).min(1.0);
        }
        self.bar_display.insert(key, shown);

        draw::rect(&mut self.canvas, panel(), pill, 0, 17);
        let inner = pill.inflate(-10, -10);
        draw::rect(&mut self.canvas, Color::rgb(20, 20, 23), inner, 0, inner.h / 2);
        let fill_w = (inner.w as f64 * shown.clamp(0.0, 1.0)) as i32;
        if fill_w > 0 {
            let fill = bar_fill_surface(key, Some(color), inner.w, inner.h);
            self.canvas.blit_ex(&fill, inner.x, inner.y, Some(Rect::new(0, 0, fill_w, inner.h)), 0);
        }
        let oc = if paused {
            PAUSED_RED
        } else if pill.collidepoint(mouse_pos) {
            grey()
        } else {
            outline()
        };
        draw::rect(&mut self.canvas, oc, pill, BORDER_W_SMALL, 17);
        self.register_button(pill, Rc::new(move |g: &mut Game| g.state.toggle_cycle_pause(key)), Some("click"));
        let name = tr(label);
        let text = if ready {
            tr!("%s READY!  x%g", name.to_uppercase(), mult)
        } else if cont {
            pyformat("%s  x%g", &crate::args![name.to_uppercase(), mult])
        } else if left == 1 {
            tr!("%s in %d roll", name, left)
        } else {
            tr!("%s in %d rolls", name, left)
        };
        let t = self.f.small_b.render(&text, WHITE);
        blit_center(&mut self.canvas, &t, pill.center());
        let _ = ti;
        y + pill.h + 8
    }
}
