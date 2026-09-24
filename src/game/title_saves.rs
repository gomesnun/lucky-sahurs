//! Title and save-slot screens (ui/title_saves.py).

use super::base::{Bo, blit_center};
use super::{Game, cb};
use crate::config::{FULLSCREEN_HINT, GAME_TITLE, SAVE_SLOTS, VERSION, VIRTUAL_H};
use crate::core::data::{RARITY_TIERS, rarities, tier_first_pet};
use crate::core::formatting::{format_number, format_playtime, py_round};
use crate::core::state::{GameState, now_ts};
use crate::gfx::{BLEND_RGBA_MULT, Color, Rect, Surface, draw, ti, transform};
use crate::i18n::tr;
use crate::storage::{delete_slot, peek_slot};
use crate::theme::*;
use crate::tr;
use crate::ui::drawing::{blit_smooth_y, draw_panel, draw_rarity_bg, draw_state_border};
use crate::ui::fonts::{fit_text, wrap_text};
use crate::ui::icons::load_icon;
use std::rc::Rc;

pub struct SlotInfo {
    pub coins: f64,
    pub total_rolls: i64,
    pub playtime: f64,
}

pub fn offline_message(gain: f64, seconds: f64) -> String {
    tr!("Welcome back! +$%s earned while you were away (%s).", format_number(gain), format_playtime(seconds))
}

impl Game {
    pub fn refresh_slot_info(&mut self) {
        self.slot_info = (1..=SAVE_SLOTS).map(|i| (i, peek_slot(i))).collect();
    }

    pub fn open_saves(&mut self) {
        self.refresh_slot_info();
        self.delete_confirm_slot = None;
        self.screen_mode = "saves";
        if self.account.is_some() && !self.cloud_slots_loaded {
            self.load_cloud_slots();
        }
    }

    pub fn back_to_title(&mut self) {
        self.delete_confirm_slot = None;
        self.screen_mode = "title";
    }

    pub fn request_delete(&mut self, slot: i64) {
        if self.delete_confirm_slot == Some(slot) {
            delete_slot(slot);
            self.delete_confirm_slot = None;
            self.refresh_slot_info();
            self.play("click", 0.0);
            self.show_toast(&tr!("Slot %d deleted.", slot), 1.8);
        } else {
            self.delete_confirm_slot = Some(slot);
            self.delete_confirm_timer = 3.0;
        }
    }

    pub fn request_delete_account(&mut self, slot: i64) {
        if self.delete_confirm_slot == Some(slot) {
            self.delete_confirm_slot = None;
            self.play("click", 0.0);
            self.delete_account_slot(slot);
        } else {
            self.delete_confirm_slot = Some(slot);
            self.delete_confirm_timer = 3.0;
        }
    }

    pub fn start_slot(&mut self, slot: i64) {
        let mut st = GameState::new();
        st.slot = Some(slot);
        st.load();
        let (gain, away) = st.claim_offline_earnings();
        st.save();
        self.replace_state(st);
        self.reset_ui();
        self.options_open = false;
        self.stats_open = false;
        self.autosave_timer = 0.0;
        self.screen_mode = "game";
        if gain > 0.0 {
            let t = format!("{}\n{}", tr!("Slot %d loaded", slot), offline_message(gain, away));
            self.show_toast(&t, 6.0);
        } else {
            self.show_toast(&tr!("Slot %d loaded", slot), 1.8);
        }
    }

    pub fn go_to_menu(&mut self) {
        self.state.save();
        self.release_session();
        self.replace_state(GameState::new());
        self.reset_ui();
        self.options_open = false;
        self.stats_open = false;
        self.screen_mode = "title";
    }

    pub fn draw_title(&mut self, mouse_pos: (f64, f64)) {
        let cx = self.vw / 2;
        let t = now_ts();
        let n = RARITY_TIERS.len() as i32;
        let (size, gap) = (64, 16);
        let total = n * size + (n - 1) * gap;
        let x0 = cx - total / 2;
        for i in 0..n {
            let r = &rarities()[tier_first_pet(i as usize)];
            let bob = if self.animations() { (t * 1.6 + i as f64 * 0.7).sin() * 8.0 } else { 0.0 };
            let x = x0 + i * (size + gap);
            let key = format!("title_card|{}|{}", r.key, size);
            blit_smooth_y(
                &mut self.canvas,
                &key,
                || {
                    let mut card = Surface::new_alpha(size, size);
                    draw_rarity_bg(&mut card, Rect::new(0, 0, size, size), r, 10);
                    let cr = card.get_rect();
                    draw::rect(&mut card, outline(), cr, BORDER_W_SMALL, 10);
                    card
                },
                x as f64,
                250.0 + bob,
            );
        }

        let title = self.f.title.render(GAME_TITLE, WHITE);
        let tid = Rc::as_ptr(&title) as usize;
        let shadow = match &self.title_shadow {
            Some((id, s)) if *id == tid => s.clone(),
            _ => {
                let mut s = title.copy();
                s.fill_blend(Color::rgba(0, 0, 0, 110), BLEND_RGBA_MULT);
                let s = Rc::new(s);
                self.title_shadow = Some((tid, s.clone()));
                s
            }
        };
        blit_center(&mut self.canvas, &shadow, (cx + 3, 128 + 9));
        blit_center(&mut self.canvas, &title, (cx, 128));
        let sub = self.f.med.render(&tr("Roll pets, equip them and fill your pockets"), grey());
        blit_center(&mut self.canvas, &sub, (cx, 212));

        self.draw_account_chip(mouse_pos);

        let (bw, bh) = (340, 62);
        let mut y = 360;
        let stack_top = y;
        let big = self.f.big.clone();
        self.button(Rect::new(cx - bw / 2, y, bw, bh), &tr("Saves"), &big, mouse_pos, accent(), accent_hover(), BLACK, cb(|g| g.open_saves()), Bo::r(14).icon("saves"));
        y += bh + 14;
        self.nav_mode = true;
        self.button(Rect::new(cx - bw / 2, y, bw, bh), &tr("Leaderboard"), &big, mouse_pos, panel_light(), panel_lighter(), accent(), cb(|g| g.toggle_leaderboard()), Bo::r(14).icon("leaderboard"));
        y += bh + 14;
        self.button(Rect::new(cx - bw / 2, y, bw, bh), &tr("Options"), &big, mouse_pos, panel_light(), panel_lighter(), WHITE, cb(|g| g.toggle_options()), Bo::r(14).icon("options"));
        y += bh + 14;
        let half = (bw - 14) / 2;
        self.button(Rect::new(cx - bw / 2, y, half, bh), &tr("Credits"), &big, mouse_pos, panel_light(), panel_lighter(), WHITE, cb(|g| g.toggle_credits()), Bo::r(14));
        self.nav_mode = false;
        self.button(Rect::new(cx - bw / 2 + half + 14, y, half, bh), &tr("Quit"), &big, mouse_pos, Color::rgb(150, 60, 60), BAD, WHITE, cb(|g| g.quit_game()), Bo::r(14));

        let stack_cy = (stack_top + y + bh) / 2;
        self.draw_mascot((cx + bw / 2 + self.vw) / 2, stack_cy + 115, 230);

        let log_rect = Rect::new(20, VIRTUAL_H - 20 - 44, 190, 44);
        self.nav_mode = true;
        let sb = self.f.small_b.clone();
        self.button(log_rect, &tr("Update Log"), &sb, mouse_pos, panel_light(), panel_lighter(), WHITE, cb(|g| g.toggle_update_log()), Bo::r(10).icon("updatelog"));
        let fb_rect = Rect::new(log_rect.right() + 12, log_rect.y, 160, 44);
        self.button(fb_rect, &tr("Feedback"), &sb, mouse_pos, panel_light(), panel_lighter(), WHITE, cb(|g| g.toggle_feedback()), Bo::r(10).icon("feedback"));
        self.nav_mode = false;

        let rodape = tr!("%s   ·   %s: fullscreen / windowed", VERSION, FULLSCREEN_HINT);
        let foot = self.f.small.render(&rodape, grey_dim());
        blit_center(&mut self.canvas, &foot, (cx, VIRTUAL_H - 20));
    }

    fn mascot_ready(surf: &Surface) -> (Rc<Surface>, (i32, i32)) {
        let full = surf.get_rect();
        let mut bx = surf.bounding_rect();
        if bx.w <= 0 || bx.h <= 0 {
            bx = full;
        }
        let cropped = surf.sub_copy(bx);
        (Rc::new(cropped), (bx.centerx() - full.centerx(), bx.centery() - full.centery()))
    }

    pub fn draw_mascot(&mut self, cx: i32, ground_y: i32, size: i32) {
        let Some(img) = load_icon("verity", size) else { return };
        let iid = Rc::as_ptr(&img) as usize;
        let padded = match &self.mascot_pad {
            Some((id, p)) if *id == iid => p.clone(),
            _ => {
                self.mascot_frames.clear();
                self.mascot_shadows.clear();
                self.mascot_still = None;
                let mut p = Surface::new_alpha(size, size * 2);
                p.blit(&img, 0, 0);
                let p = Rc::new(p);
                self.mascot_pad = Some((iid, p.clone()));
                p
            }
        };
        let t = now_ts();
        let anim = self.animations();
        let angle = if anim { (t * 1.3).sin() * 3.5 } else { 0.0 };
        let scale = if anim { 1.0 + 0.025 * (t * 2.0).sin() } else { 1.0 };
        let shadow_w = (size as f64 * 0.66 * scale) as i32;
        let shadow = match self.mascot_shadows.get(&shadow_w) {
            Some(s) => s.clone(),
            None => {
                let mut s = Surface::new_alpha(shadow_w, 26);
                let r = s.get_rect();
                draw::ellipse(&mut s, Color::rgba(0, 0, 0, 55), r, 0);
                let s = Rc::new(s);
                self.mascot_shadows.insert(shadow_w, s.clone());
                s
            }
        };
        blit_center(&mut self.canvas, &shadow, (cx, ground_y - 2));
        let (surf, offset) = if anim {
            let ka = py_round(angle * 2.0) as i64;
            let ks = py_round(scale * 100.0) as i64;
            match self.mascot_frames.get(&(ka, ks)) {
                Some(f) => f.clone(),
                None => {
                    let rz = transform::rotozoom(&padded, ka as f64 / 2.0, ks as f64 / 100.0);
                    let f = Self::mascot_ready(&rz);
                    self.mascot_frames.insert((ka, ks), f.clone());
                    f
                }
            }
        } else {
            match &self.mascot_still {
                Some(f) => f.clone(),
                None => {
                    let f = Self::mascot_ready(&padded);
                    self.mascot_still = Some(f.clone());
                    f
                }
            }
        };
        let r = Rect::with_center(surf.w, surf.h, (cx + offset.0, ground_y + offset.1));
        self.canvas.blit(&surf, r.x, r.y);
    }

    pub fn draw_save_card_body(&mut self, rect: Rect, info: &SlotInfo) {
        let coins = self.f.big.render(&format!("$ {}", format_number(info.coins)), GOOD);
        blit_center(&mut self.canvas, &coins, (rect.centerx(), rect.y + 98));
        let rows = [(tr("Rolls"), format_number(info.total_rolls as f64)), (tr("Playtime"), format_playtime(info.playtime))];
        let mut ry = rect.y + 152;
        for (label, value) in rows {
            let l = self.f.small.render(&label, grey());
            let v = self.f.small_b.render(&value, WHITE);
            self.canvas.blit(&l, rect.x + 28, ry);
            self.canvas.blit(&v, rect.right() - 28 - v.w, ry);
            ry += 30;
        }
    }

    pub fn draw_saves(&mut self, mouse_pos: (f64, f64)) {
        let cx = self.vw / 2;
        let title = self.f.huge.render(&tr("Saves"), WHITE);
        blit_center(&mut self.canvas, &title, (cx, 70));
        let sub_text = match &self.account {
            Some(a) => tr!("%s's saves - synced to your account (up to %d slots).", a.username.clone(), SAVE_SLOTS),
            None => tr!("Pick a slot to play (up to %d saves).", SAVE_SLOTS),
        };
        let small = self.f.small.clone();
        let sub = small.render(&fit_text(&small, &sub_text, self.vw - 340), grey());
        blit_center(&mut self.canvas, &sub, (cx, 114));
        self.draw_account_chip(mouse_pos);

        let gap = 32;
        let card_w = 320.min((self.vw - 160 - gap * 2) / SAVE_SLOTS as i32);
        let card_h = 380;
        let total_w = card_w * SAVE_SLOTS as i32 + gap * (SAVE_SLOTS as i32 - 1);
        let x0 = cx - total_w / 2;
        let y0 = 160;
        let busy = self.slot_starting || self.import_busy;
        let med = self.f.med.clone();
        let sb = self.f.small_b.clone();

        for i in 0..SAVE_SLOTS as i32 {
            let slot = (i + 1) as i64;
            let rect = Rect::new(x0 + i * (card_w + gap), y0, card_w, card_h);
            draw_panel(&mut self.canvas, rect, Some(panel()), 16, true, None);
            let head = self.f.big.render(&tr!("Slot %d", slot), WHITE);
            blit_center(&mut self.canvas, &head, (rect.centerx(), rect.y + 38));
            let play_rect = Rect::new(rect.x + 24, rect.bottom() - 118, rect.w - 48, 50);

            if self.account.is_none() {
                let info = self.slot_info.get(&slot).cloned().flatten();
                if let Some(info) = info {
                    draw_state_border(&mut self.canvas, rect, accent(), 16, 2);
                    self.draw_save_card_body(rect, &SlotInfo { coins: info.coins, total_rolls: info.total_rolls, playtime: info.playtime });
                    self.button(play_rect, &tr("Play"), &med, mouse_pos, accent(), accent_hover(), BLACK, cb(move |g| g.start_slot(slot)), Bo::r(12));
                    let confirming = self.delete_confirm_slot == Some(slot);
                    self.button(
                        Rect::new(rect.x + 24, rect.bottom() - 56, rect.w - 48, 38),
                        &if confirming { tr("Click again to delete") } else { tr("Delete save") },
                        &sb,
                        mouse_pos,
                        if confirming { Color::rgb(170, 60, 60) } else { panel_light() },
                        BAD,
                        WHITE,
                        cb(move |g| g.request_delete(slot)),
                        Bo::r(10).sfx(None),
                    );
                } else {
                    let empty = self.f.med.render(&tr("Empty"), grey_dim());
                    blit_center(&mut self.canvas, &empty, (rect.centerx(), rect.centery() - 20));
                    self.button(play_rect, &tr("New Game"), &med, mouse_pos, accent(), accent_hover(), BLACK, cb(move |g| g.start_slot(slot)), Bo::r(12));
                }
                continue;
            }

            let (kind, info, label) = self.slot_view(slot);
            if !label.is_empty() {
                let badge_color = if label == "Cloud" { GOOD } else { grey_dim() };
                let badge = self.f.tiny.render(&tr(label), badge_color);
                self.canvas.blit(&badge, rect.right() - 14 - badge.w, rect.y + 15);
            }
            match kind {
                "unknown" => {
                    let failed = self.cloud_slots_error.is_some() && !self.cloud_slots_loading;
                    let msg = self.f.med.render(&if failed { tr("Couldn't load") } else { tr("Loading...") }, if failed { BAD } else { grey_dim() });
                    blit_center(&mut self.canvas, &msg, rect.center());
                }
                "empty" => {
                    let empty = self.f.med.render(&tr("Empty"), grey_dim());
                    blit_center(&mut self.canvas, &empty, (rect.centerx(), rect.centery() - 20));
                    self.button(play_rect, &tr("New Game"), &med, mouse_pos, accent(), accent_hover(), BLACK, cb(move |g| g.start_cloud_slot(slot)), Bo::r(12).enabled(!busy));
                }
                "importable" => {
                    draw_state_border(&mut self.canvas, rect, panel_lighter(), 16, 2);
                    self.draw_save_card_body(rect, info.as_ref().unwrap());
                    let ib = self.import_busy;
                    self.button(play_rect, &tr("Import to account"), &sb, mouse_pos, accent(), accent_hover(), BLACK, cb(move |g| g.import_local_slot(slot)), Bo::r(12).enabled(!ib));
                    let mut ny = rect.bottom() - 60;
                    let tiny = self.f.tiny.clone();
                    for line in wrap_text(&tr("Local save on this PC. Importing copies it here - the local file is kept."), &tiny, rect.w - 32).iter().take(2) {
                        let t = tiny.render(line, grey_dim());
                        self.canvas.blit(&t, rect.x + 16, ny);
                        ny += 15;
                    }
                }
                _ => {
                    draw_state_border(&mut self.canvas, rect, accent(), 16, 2);
                    self.draw_save_card_body(rect, info.as_ref().unwrap());
                    self.button(play_rect, &tr("Play"), &med, mouse_pos, accent(), accent_hover(), BLACK, cb(move |g| g.start_cloud_slot(slot)), Bo::r(12).enabled(!busy));
                    let confirming = self.delete_confirm_slot == Some(slot);
                    self.button(
                        Rect::new(rect.x + 24, rect.bottom() - 56, rect.w - 48, 38),
                        &if confirming { tr("Click again to delete") } else { tr("Delete save") },
                        &sb,
                        mouse_pos,
                        if confirming { Color::rgb(170, 60, 60) } else { panel_light() },
                        BAD,
                        WHITE,
                        cb(move |g| g.request_delete_account(slot)),
                        Bo::r(10).sfx(None),
                    );
                }
            }
        }

        let foot_y = y0 + card_h + 40;
        self.button(Rect::new(cx - 110, foot_y, 220, 48), &tr("Back"), &med, mouse_pos, panel_light(), panel_lighter(), WHITE, cb(|g| g.back_to_title()), Bo::r(12));

        if self.account.is_some() && self.cloud_slots_error.is_some() && !self.cloud_slots_loading {
            let note = if self.cloud_slots_loaded {
                tr("Couldn't refresh your cloud saves - showing the last known copy.")
            } else if !self.cloud_slots_error_msg.is_empty() {
                self.cloud_slots_error_msg.clone()
            } else {
                tr("Couldn't reach your cloud saves.")
            };
            let tiny = self.f.tiny.clone();
            let note = fit_text(&tiny, &note, 200.max(self.vw - (cx + 130) - 20));
            let t = tiny.render(&note, BAD);
            self.canvas.blit(&t, cx + 130, foot_y + 8);
            self.button(Rect::new(cx + 130, foot_y + 26, 90, 32), &tr("Retry"), &sb, mouse_pos, panel_light(), panel_lighter(), WHITE, cb(|g| g.load_cloud_slots()), Bo::r(8));
        }
        let _ = ti;
    }
}
