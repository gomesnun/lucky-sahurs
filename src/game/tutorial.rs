//! First-time tutorial (a quick, skippable tour of the main screen) and the "What's new" pop-up that shows once
//! after an update. Both are remembered in the settings: "tutorial_done" and "last_update_seen" (the newest
//! UPDATE_LOG version the player has seen).

use super::base::Bo;
use super::updatelog::UPDATE_LOG;
use super::{Game, KeyEv, cb};
use crate::config::{TOPBAR_H, VIRTUAL_H};
use crate::gfx::{Color, Rect, Surface, draw};
use crate::i18n::tr;
use crate::online::updater::parse_version;
use crate::storage::save_settings;
use crate::theme::*;
use crate::tr;
use crate::ui::drawing::{dim_overlay, draw_panel};
use crate::ui::fonts::wrap_text;
use std::rc::Rc;

/// What the tutorial points at on each step.
#[derive(Clone, Copy)]
enum Spot {
    Nothing,
    Roll,
    Card,
    Money,
    Side(usize),
    Sides(usize, usize),
    Topbar,
}

const STEPS: &[(&str, &str, Spot)] = &[
    ("Welcome to Lucky Verities!", "A quick tour of everything. You can skip it at any time, and replay it later from Options > Game.", Spot::Nothing),
    ("Roll", "Press ROLL to get a Verity. Rarer ones earn much more money. The Auto Roller (bought in Upgrades) rolls for you.", Spot::Roll),
    ("Your Verity", "The card shows what you got: its rarity, the money it makes each second and how rare it is. Golden, Diamond and Rainbow mutations earn x3, x9 and x27.", Spot::Card),
    ("Money", "Your coins and how much you earn per second. Equipped Verities make money all the time - even while the game is closed.", Spot::Money),
    ("Bag", "Equip your best Verities here (or press Equip Best), sell extras with the $ button, and use your potions.", Spot::Side(4)),
    ("Evolve", "Got many copies of one Verity? Its arrow button in the Inventory stacks them to evolve it: Phase 2, Phase 3 and then its Monster form, each earning more.", Spot::Side(4)),
    ("Index", "Every Verity and mutation you ever found, and the chance of each one.", Spot::Side(0)),
    ("Upgrades", "Spend coins on luck, money, mutations, the Auto Roller, more equip slots and much more.", Spot::Side(1)),
    ("Traits", "Rolling sometimes gives trait charges. Roll traits with them and equip one for extra money, luck and speed.", Spot::Side(6)),
    ("Rebirth & Prestige", "Reset your coins for permanent Money and Luck boosts. After enough Rebirths, Prestige gives even bigger ones.", Spot::Side(5)),
    ("Shop", "Unlocks at Rebirth 1: dice for permanent luck and potions, with new stock every 10 minutes.", Spot::Side(7)),
    ("Milestones & Quests", "Long-term goals with permanent bonuses, plus daily and weekly quests with rewards that grow with you.", Spot::Sides(2, 3)),
    ("Friends, Stats & Options", "Add friends, chat and trade, check your stats and change settings. You're all set - good luck!", Spot::Topbar),
];

fn union(a: Rect, b: Rect) -> Rect {
    let (l, t) = (a.left().min(b.left()), a.top().min(b.top()));
    Rect::new(l, t, a.right().max(b.right()) - l, a.bottom().max(b.bottom()) - t)
}

/// The newest version in the Update Log.
pub fn latest_update() -> &'static str {
    UPDATE_LOG.first().map(|u| u.0).unwrap_or("")
}

impl Game {
    // ---------------------------------------------------------------- entering the game
    /// Called when a save is opened: a brand-new player gets the tutorial, a returning one sees what's new once.
    pub fn on_enter_game(&mut self) {
        let fresh = self.state.total_rolls == 0 && self.state.owned.is_empty();
        if fresh && !self.settings.get_bool("tutorial_done", false) {
            self.start_tutorial();
            self.mark_updates_seen(); // the tutorial covers what's new
            return;
        }
        if !self.unseen_updates().is_empty() {
            self.whats_new_open = true;
        }
    }

    // ---------------------------------------------------------------- tutorial
    pub fn tutorial_active(&self) -> bool {
        self.tutorial_step.is_some()
    }

    pub fn start_tutorial(&mut self) {
        self.close_overlays();
        self.options_open = false;
        self.stats_open = false;
        self.whats_new_open = false;
        self.left_panel.close();
        self.right_panel.close();
        self.tutorial_step = Some(0);
    }

    pub fn tutorial_next(&mut self) {
        match self.tutorial_step {
            Some(i) if i + 1 < STEPS.len() => self.tutorial_step = Some(i + 1),
            Some(_) => self.end_tutorial(),
            None => {}
        }
    }

    pub fn tutorial_back(&mut self) {
        if let Some(i) = self.tutorial_step {
            self.tutorial_step = Some(i.saturating_sub(1));
        }
    }

    pub fn end_tutorial(&mut self) {
        self.tutorial_step = None;
        self.settings.set_bool("tutorial_done", true);
        save_settings(&self.settings);
    }

    fn spot_rect(&self, spot: Spot) -> Option<Rect> {
        let side = self.side_button_rects();
        let label = 28; // the side buttons' labels sit under them
        Some(match spot {
            Spot::Nothing => return None,
            Spot::Roll => self.roll_button_rect(),
            Spot::Card => self.main_card_rect(),
            Spot::Money => Rect::new(8, 4, 430.min(self.vw / 2), TOPBAR_H - 8),
            Spot::Side(i) => Rect::new(side[i].x, side[i].y, side[i].w, side[i].h + label),
            Spot::Sides(a, b) => union(side[a], Rect::new(side[b].x, side[b].y, side[b].w, side[b].h + label)),
            Spot::Topbar => {
                let t = self.topbar_button_rects();
                union(t[0], t[2])
            }
        })
    }

    /// Keys while the tutorial is up: Enter / Right = next, Left = back, Esc = skip. It takes every key.
    pub fn handle_tutorial_key(&mut self, ev: KeyEv) -> bool {
        use sdl2::keyboard::Keycode as K;
        if !self.tutorial_active() {
            return false;
        }
        match ev.key {
            K::Escape => self.end_tutorial(),
            K::Return | K::KpEnter | K::Right | K::Space => self.tutorial_next(),
            K::Left => self.tutorial_back(),
            _ => {}
        }
        true
    }

    pub fn draw_tutorial(&mut self, mouse_pos: (f64, f64)) {
        let Some(i) = self.tutorial_step else { return };
        let (title, text, spot) = STEPS[i.min(STEPS.len() - 1)];
        // nothing under the tutorial can be clicked
        self.buttons.clear();
        self.register_button(Rect::new(0, 0, self.vw, VIRTUAL_H), Rc::new(|_: &mut Game| {}), None);

        // dim everything but the spotlight
        let target = self.spot_rect(spot).map(|r| r.inflate(16, 16));
        match target {
            None => {
                let ov = dim_overlay(self.vw, VIRTUAL_H, 170);
                self.canvas.blit(&ov, 0, 0);
            }
            Some(t) => {
                let dim = Color::rgba(4, 6, 14, 175);
                for r in [
                    Rect::new(0, 0, self.vw, t.top().max(0)),
                    Rect::new(0, t.bottom(), self.vw, VIRTUAL_H - t.bottom()),
                    Rect::new(0, t.top(), t.left().max(0), t.h),
                    Rect::new(t.right(), t.top(), self.vw - t.right(), t.h),
                ] {
                    if r.w > 0 && r.h > 0 {
                        let mut s = Surface::new_alpha(r.w, r.h);
                        s.fill(dim, None);
                        self.canvas.blit(&s, r.x, r.y);
                    }
                }
                let pulse = 0.5 + 0.5 * (crate::core::state::now_ts() * 4.0).sin();
                draw::rect(&mut self.canvas, accent(), t.inflate((4.0 * pulse) as i32, (4.0 * pulse) as i32), 3, 14);
            }
        }

        // the explanation box: beside the spotlight, away from the screen edge it's on
        let w = 430.min(self.vw - 40);
        let pad = 22;
        let small = self.f.small.clone();
        let sb = self.f.small_b.clone();
        let lines = wrap_text(&tr(text), &small, w - pad * 2);
        let head = self.f.big.render(&tr(title), WHITE);
        let h = pad + head.h + 10 + lines.len() as i32 * 22 + 18 + 40 + pad;
        let rect = match target {
            None => Rect::with_center(w, h, (self.vw / 2, VIRTUAL_H / 2)),
            Some(t) => {
                let cx = t.centerx();
                let mut r = if cx < self.vw / 3 {
                    Rect::with_midleft(w, h, (t.right() + 24, t.centery()))
                } else if cx > self.vw * 2 / 3 && t.top() > TOPBAR_H {
                    Rect::with_midright(w, h, (t.left() - 24, t.centery()))
                } else if t.bottom() + 24 + h < VIRTUAL_H {
                    Rect::with_midtop(w, h, (cx, t.bottom() + 24))
                } else {
                    Rect::with_midbottom(w, h, (cx, t.top() - 24))
                };
                r.x = r.x.clamp(12, self.vw - w - 12);
                r.y = r.y.clamp(TOPBAR_H + 8, VIRTUAL_H - h - 12);
                r
            }
        };
        draw_panel(&mut self.canvas, rect, Some(panel()), 16, true, None);
        self.register_blocker(rect);
        self.canvas.blit(&head, rect.x + pad, rect.y + pad);
        let count = self.f.tiny.render(&format!("{} / {}", i + 1, STEPS.len()), grey_dim());
        self.canvas.blit(&count, rect.right() - pad - count.w, rect.y + pad + 4);
        let mut y = rect.y + pad + head.h + 10;
        for line in &lines {
            let t = small.render(line, grey());
            self.canvas.blit(&t, rect.x + pad, y);
            y += 22;
        }
        let by = rect.bottom() - pad - 40;
        let last = i + 1 >= STEPS.len();
        self.button(Rect::new(rect.x + pad, by, 130, 40), &tr("Skip tutorial"), &sb, mouse_pos, panel_light(), panel_lighter(), grey(), cb(|g| g.end_tutorial()), Bo::r(10));
        let next_w = 130;
        let next = Rect::new(rect.right() - pad - next_w, by, next_w, 40);
        let label = if last { tr("Let's go!") } else { tr("Next") };
        self.button(next, &label, &sb, mouse_pos, accent(), accent_hover(), BLACK, cb(|g| g.tutorial_next()), Bo::r(10));
        if i > 0 {
            self.button(Rect::new(next.x - 10 - 90, by, 90, 40), &tr("Back"), &sb, mouse_pos, panel_light(), panel_lighter(), WHITE, cb(|g| g.tutorial_back()), Bo::r(10));
        }
    }

    // ---------------------------------------------------------------- what's new
    /// Update Log entries newer than the last one this player saw (all of them never counts: a player with no
    /// record gets only the newest one).
    pub fn unseen_updates(&self) -> Vec<usize> {
        let seen = self.settings.get_str("last_update_seen", "");
        if seen == latest_update() || UPDATE_LOG.is_empty() {
            return Vec::new();
        }
        match parse_version(&seen) {
            None => vec![0],
            Some(s) => (0..UPDATE_LOG.len()).filter(|&i| parse_version(UPDATE_LOG[i].0).is_some_and(|v| v > s)).take(3).collect(),
        }
    }

    pub fn mark_updates_seen(&mut self) {
        self.settings.set_str("last_update_seen", latest_update());
        save_settings(&self.settings);
    }

    pub fn close_whats_new(&mut self) {
        self.whats_new_open = false;
        self.mark_updates_seen();
    }

    pub fn handle_whats_new_key(&mut self, ev: KeyEv) -> bool {
        use sdl2::keyboard::Keycode as K;
        if !self.whats_new_open {
            return false;
        }
        if matches!(ev.key, K::Escape | K::Return | K::KpEnter | K::Space) {
            self.close_whats_new();
        }
        true
    }

    pub fn draw_whats_new(&mut self, mouse_pos: (f64, f64)) {
        let entries = self.unseen_updates();
        if entries.is_empty() {
            self.whats_new_open = false;
            return;
        }
        let ov = dim_overlay(self.vw, VIRTUAL_H, 170);
        self.canvas.blit(&ov, 0, 0);
        self.buttons.clear();
        self.register_button(Rect::new(0, 0, self.vw, VIRTUAL_H), Rc::new(|_: &mut Game| {}), None);
        let w = 600.min(self.vw - 40);
        let pad = 26;
        let small = self.f.small.clone();
        let med = self.f.med.clone();
        let sb = self.f.small_b.clone();
        // measure first, then center the box
        let mut blocks: Vec<(String, Vec<String>)> = Vec::new();
        let mut body_h = 0;
        for &i in &entries {
            let (version, _, lines) = UPDATE_LOG[i];
            let mut wrapped = Vec::new();
            for l in lines.iter() {
                wrapped.extend(wrap_text(&format!("- {}", tr(l)), &small, w - pad * 2));
            }
            body_h += med.get_height() + 8 + wrapped.len() as i32 * 22 + 14;
            blocks.push((version.to_string(), wrapped));
        }
        let head = self.f.big.render(&tr("What's new"), WHITE);
        let max_body = VIRTUAL_H - 80 - (pad + head.h + 16) - (40 + pad * 2);
        let body_h = body_h.min(max_body);
        let h = pad + head.h + 16 + body_h + 16 + 40 + pad;
        let rect = Rect::with_center(w, h, (self.vw / 2, VIRTUAL_H / 2));
        draw_panel(&mut self.canvas, rect, Some(panel()), 16, true, None);
        self.register_blocker(rect);
        self.canvas.blit(&head, rect.x + pad, rect.y + pad);
        let body = Rect::new(rect.x + pad, rect.y + pad + head.h + 16, w - pad * 2, body_h);
        self.push_clip(body);
        let mut y = body.y;
        for (version, wrapped) in &blocks {
            let t = med.render(&tr!("Version %s", version), accent());
            self.canvas.blit(&t, body.x, y);
            y += t.h + 8;
            for l in wrapped {
                let t = small.render(l, grey());
                self.canvas.blit(&t, body.x, y);
                y += 22;
            }
            y += 14;
        }
        self.pop_clip();
        let by = rect.bottom() - pad - 40;
        let half = (w - pad * 2 - 12) / 2;
        self.button(
            Rect::new(rect.x + pad, by, half, 40),
            &tr("All updates"),
            &sb,
            mouse_pos,
            panel_light(),
            panel_lighter(),
            WHITE,
            cb(|g| {
                g.close_whats_new();
                g.open_update_log();
            }),
            Bo::r(10).icon("updatelog"),
        );
        self.button(Rect::new(rect.right() - pad - half, by, half, 40), &tr("Got it!"), &sb, mouse_pos, accent(), accent_hover(), BLACK, cb(|g| g.close_whats_new()), Bo::r(10));
    }
}
