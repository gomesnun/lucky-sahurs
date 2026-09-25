//! The 4.0 teaser: the first time a player who already played opens the game on 4.0, a short cinematic plays -
//! a black screen, the OG Verity's Monster walks in under a spotlight, stops, stares at you, waves, then the new
//! OG verities show up and it fades into the game. Only once (settings: "teaser_v4_seen"), never for new players.
//! Click or Esc skips it.

use super::monster3d::{MonsterDraw, draw_monster};
use super::{Game, KeyEv};
use crate::config::VIRTUAL_H;
use crate::core::data::rarities;
use crate::gfx::r3d::{Camera, Fog, Frame, View, v3};
use crate::gfx::{Color, Rect, Surface, transform};
use crate::i18n::tr;
use crate::storage::save_settings;
use crate::theme::*;
use crate::ui::icons::load_pet_image;
use std::rc::Rc;

pub const TEASER_SETTING: &str = "teaser_v4_seen";
/// the whole thing lasts this long
const LENGTH: f64 = 14.0;
// the beats
const WALK_END: f64 = 5.2;
const TURN_END: f64 = 5.9;
const STARE_END: f64 = 7.8;
const WAVE_END: f64 = 10.4;
const FADE_OUT: f64 = 1.4;

/// 0 while the teaser shows; then 0..1 as the game comes back from black (the last half of the fade)
pub fn game_from(t: f64) -> f64 {
    ((t - (LENGTH - FADE_OUT * 0.5)) / (FADE_OUT * 0.5)).clamp(0.0, 1.0)
}

fn ease(t: f64) -> f64 {
    let t = t.clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

impl Game {
    pub fn teaser_active(&self) -> bool {
        self.teaser_t.is_some()
    }

    /// Played once, for a save that already existed before 4.0.
    pub fn maybe_start_teaser(&mut self) -> bool {
        if self.settings.get_bool(TEASER_SETTING, false) {
            return false;
        }
        self.settings.set_bool(TEASER_SETTING, true);
        save_settings(&self.settings);
        self.teaser_t = Some(0.0);
        // "What's new" waits for the teaser to end
        self.whats_new_open = false;
        true
    }

    pub fn end_teaser(&mut self) {
        self.teaser_t = None;
        if !self.unseen_updates().is_empty() {
            self.whats_new_open = true;
        }
    }

    pub fn tick_teaser(&mut self, dt: f64) {
        if let Some(t) = self.teaser_t.as_mut() {
            *t += dt;
            if *t >= LENGTH {
                self.end_teaser();
            }
        }
    }

    pub fn handle_teaser_key(&mut self, ev: KeyEv) -> bool {
        use sdl2::keyboard::Keycode as K;
        if !self.teaser_active() {
            return false;
        }
        if matches!(ev.key, K::Escape | K::Return | K::KpEnter | K::Space) {
            self.skip_teaser();
        }
        true
    }

    /// Skipping jumps to the fade out.
    pub fn skip_teaser(&mut self) {
        if let Some(t) = self.teaser_t.as_mut() {
            *t = t.max(LENGTH - FADE_OUT);
        }
    }

    pub fn draw_teaser(&mut self) {
        let Some(t) = self.teaser_t else { return };
        let (w, h) = (self.vw, VIRTUAL_H);
        self.buttons.clear();
        self.register_button(Rect::new(0, 0, w, h), Rc::new(|g: &mut Game| g.skip_teaser()), None);

        // ---- the Monster, walking in the dark
        let verity = rarities().iter().position(|r| r.pet == "Verity").unwrap_or(0);
        let (fw, fh) = ((w / 2).max(8) as usize, (h / 2).max(8) as usize);
        let mut fr = Frame::new(fw, fh);
        fr.clear_sky(0xff000000, 0xff050508);
        let fog = Fog { color: 0x000000, start: 60.0, end: 120.0 };
        // where it is, where it looks, what it does
        let walk = ease(t / WALK_END);
        let x = -7.0 + 7.0 * walk;
        let turn = ease((t - WALK_END) / (TURN_END - WALK_END));
        let facing = v3(1.0 - turn, 0.0, turn).norm();
        let (mv, mt) = if t < WALK_END {
            ("Walk", t)
        } else if t < STARE_END {
            ("Idle", t - WALK_END)
        } else if t < WAVE_END {
            ("Wave", t - STARE_END)
        } else {
            ("Idle", t)
        };
        // the camera: low and far while it walks, then slowly closer to its face while it stares
        let push = ease((t - WALK_END) / (STARE_END - WALK_END + 1.5));
        let cam = Camera { pos: v3(x * 0.35, 1.6 + push * 0.8, 9.0 - push * 3.2), target: v3(x * 0.6, 1.8 + push * 0.5, 0.0), fov: 42.0 };
        let view = View::new(&cam, fw, fh);
        // a spotlight on the floor, following it
        for i in 0..40 {
            let a = i as f64 / 40.0 * std::f64::consts::TAU;
            for r in [0.4, 0.9, 1.4] {
                fr.glow(&view, v3(x + a.cos() * r, 0.02, a.sin() * r * 0.6), 0.5, (70.0, 64.0, 50.0), 0.12);
            }
        }
        let d = MonsterDraw { pet: verity, at: v3(x, 0.0, 0.0), facing, height: 3.4, move_name: mv, move_t: mt, alpha: 1.0, flash: 0.0 };
        draw_monster(&mut fr, &view, &fog, &d);
        let img = transform::scale(&fr.to_surface(), w, h);
        self.canvas.blit(&img, 0, 0);

        // ---- the new verities and the title, after the wave
        let reveal = ease((t - (WAVE_END - 0.6)) / 1.0);
        if reveal > 0.0 {
            let mut veil = Surface::new_alpha(w, h);
            veil.fill(Color::rgba(0, 0, 0, (160.0 * reveal) as u8), None);
            self.canvas.blit(&veil, 0, 0);
            let big = self.f.big.clone();
            let title = big.render("LUCKY VERITIES 4.0", accent());
            let title = transform::smoothscale(&title, title.w * 2, title.h * 2);
            let mut title = title;
            title.set_alpha((255.0 * reveal) as i32);
            self.canvas.blit(&title, w / 2 - title.w / 2, h / 2 - 210);
            let sub = self.f.med.render(&tr("New: the OG Verities"), WHITE);
            let mut sub = (*sub).clone();
            sub.set_alpha((255.0 * reveal) as i32);
            self.canvas.blit(&sub, w / 2 - sub.w / 2, h / 2 - 210 + title.h + 6);
            for (i, name) in ["Verity", "Lovity", "Falsity"].iter().enumerate() {
                // each one pops in a little after the one before
                let k = ease((t - (WAVE_END - 0.2) - i as f64 * 0.3) / 0.45);
                if k <= 0.0 {
                    continue;
                }
                let size = (150.0 * (0.4 + 0.6 * k) + 20.0 * (1.0 - k) * (k * 9.0).sin()) as i32;
                if let Some(ball) = load_pet_image(name, size.max(8), false) {
                    let cx = w / 2 + (i as i32 - 1) * 220;
                    let mut ball = (*ball).clone();
                    ball.set_alpha((255.0 * k) as i32);
                    self.canvas.blit(&ball, cx - ball.w / 2, h / 2 + 30 - ball.h / 2);
                    let n = self.f.med.render(name, WHITE);
                    let mut n = (*n).clone();
                    n.set_alpha((255.0 * k) as i32);
                    self.canvas.blit(&n, cx - n.w / 2, h / 2 + 120);
                }
            }
        }
        // ---- fades: in from black at the start, out to the game at the end
        let fade_in = 1.0 - ease(t / 0.9);
        let fade_out = ease((t - (LENGTH - FADE_OUT)) / (FADE_OUT * 0.5));
        let a = fade_in.max(fade_out);
        if a > 0.0 {
            let mut veil = Surface::new_alpha(w, h);
            veil.fill(Color::rgba(0, 0, 0, (255.0 * a) as u8), None);
            self.canvas.blit(&veil, 0, 0);
        }
        let hint = self.f.tiny.render(&tr("click to skip"), grey_dim());
        self.canvas.blit(&hint, w - hint.w - 20, h - hint.h - 14);
    }
}
