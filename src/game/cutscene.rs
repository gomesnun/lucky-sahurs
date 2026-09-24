//! Full-screen cutscene when catching a Secret+ pet (ui/cutscene_panel.py).

use super::Game;
use crate::config::VIRTUAL_H;
use crate::core::data::{TIER_SECRET, mutation, rarities};
use crate::core::formatting::format_number;
use crate::gfx::{Color, Rect, Surface, draw, ti, transform};
use crate::i18n::tr;
use crate::tr;
use crate::ui::cards::{cell, rarity_glow_color, render_pet_card};
use crate::ui::drawing::{draw_rainbow_border, ease_out_cubic, rarity_glow};
use crate::ui::widgets::Particle;
use std::f64::consts::PI;
use std::rc::Rc;

pub struct CutsceneStyle {
    pub duration: f64,
    pub rays: i32,
    pub ambient: f64,
    pub burst: i32,
    pub shake: f64,
    pub label: &'static str,
}

pub fn cutscene_style(key: &str) -> CutsceneStyle {
    match key {
        "secreto" => CutsceneStyle { duration: 1.8, rays: 8, ambient: 10.0, burst: 26, shake: 0.0, label: "SECRET PET!" },
        "divino" => CutsceneStyle { duration: 2.2, rays: 12, ambient: 16.0, burst: 38, shake: 0.0, label: "DIVINE PET!" },
        "cosmico" => CutsceneStyle { duration: 2.6, rays: 16, ambient: 24.0, burst: 54, shake: 3.0, label: "COSMIC PET!" },
        _ => CutsceneStyle { duration: 3.2, rays: 22, ambient: 34.0, burst: 76, shake: 6.0, label: "TRANSCENDENT PET!" },
    }
}

const FADE_IN: f64 = 0.25;
const FADE_OUT: f64 = 0.35;

impl Game {
    pub fn trigger_cutscene(&mut self, rarity_index: usize, mutation: &'static str) {
        let rarity = &rarities()[rarity_index];
        if rarity.tier < TIER_SECRET {
            return;
        }
        if !self.settings.get_bool(&format!("cutscenes_{}", rarity.key), true) {
            return;
        }
        if self.cutscene_active.is_none() && self.cutscene_queue.is_empty() {
            self.start_cutscene(rarity_index, mutation);
        } else if self.cutscene_queue.len() < 3 {
            self.cutscene_queue.push((rarity_index, mutation));
        }
    }

    pub fn start_cutscene(&mut self, rarity_index: usize, mutation: &'static str) {
        self.cutscene_active = Some((rarity_index, mutation));
        self.cutscene_elapsed = 0.0;
        self.cutscene_particles.clear();
        self.cutscene_ambient_accum = 0.0;
        self.play_roll_sfx(rarity_index, mutation, true, false);
    }

    pub fn end_cutscene(&mut self) {
        self.cutscene_active = None;
        self.cutscene_elapsed = 0.0;
        self.cutscene_particles.clear();
    }

    pub fn skip_cutscene(&mut self) {
        if self.cutscene_active.is_some() {
            self.play("click", 0.0);
            self.end_cutscene();
        }
    }

    pub fn update_cutscenes(&mut self, dt: f64) {
        let Some((r_idx, _)) = self.cutscene_active else {
            if !self.cutscene_queue.is_empty() {
                let (r, m) = self.cutscene_queue.remove(0);
                self.start_cutscene(r, m);
            }
            return;
        };
        let style = cutscene_style(rarities()[r_idx].key);
        self.cutscene_elapsed += dt;
        for p in self.cutscene_particles.iter_mut() {
            p.update(dt);
        }
        self.cutscene_particles.retain(|p| p.life > 0.0);
        if self.animations() {
            self.cutscene_ambient_accum += dt * style.ambient;
            let n = self.cutscene_ambient_accum as i64;
            if n > 0 {
                self.cutscene_ambient_accum -= n as f64;
                let color = rarities()[r_idx].color;
                let (cx, cy) = (self.vw / 2, VIRTUAL_H / 2);
                for _ in 0..n.min(6) {
                    self.cutscene_particles.push(Particle::new(cx as f64, cy as f64, color));
                }
            }
        }
        if self.cutscene_elapsed >= style.duration {
            self.end_cutscene();
        }
    }

    pub fn draw_cutscene(&mut self, _mouse_pos: (f64, f64)) {
        self.buttons.clear();
        self.scrollbar_hits.clear();

        let Some((r_idx, mutation_key)) = self.cutscene_active else { return };
        let rarity = &rarities()[r_idx];
        let style = cutscene_style(rarity.key);
        let dur = style.duration;
        let elapsed = self.cutscene_elapsed;
        let anim = self.animations();

        let fade_in = (elapsed / FADE_IN).min(1.0);
        let fade_out = if dur - elapsed < FADE_OUT { ((dur - elapsed) / FADE_OUT).min(1.0) } else { 1.0 };
        let alpha_mul = fade_in.min(fade_out).clamp(0.0, 1.0);

        let (mut shake_x, mut shake_y) = (0.0, 0.0);
        if style.shake != 0.0 && anim {
            shake_x = (elapsed * 29.0).sin() * style.shake;
            shake_y = (elapsed * 23.0).cos() * style.shake;
        }
        let cx = self.vw as f64 / 2.0 + shake_x;
        let cy = VIRTUAL_H as f64 / 2.0 + shake_y;
        let color = Color::rgb(rarity.color.r, rarity.color.g, rarity.color.b);

        // dark backdrop + spinning light rays
        let mut overlay = Surface::new_alpha(self.vw, VIRTUAL_H);
        overlay.fill(Color::rgba(0, 0, 0, (225.0 * alpha_mul) as i32 as u8), None);
        if anim {
            let n_rays = style.rays;
            let spin = elapsed * 0.5;
            let ray_len = self.vw.max(VIRTUAL_H) as f64 * 0.75;
            let half_w = PI / 10.max(n_rays * 2) as f64;
            let ray_color = Color::rgba(color.r, color.g, color.b, (46.0 * alpha_mul) as i32 as u8);
            for i in 0..n_rays {
                let ang = spin + i as f64 * (2.0 * PI / n_rays as f64);
                let p2 = (ti(cx + (ang - half_w).cos() * ray_len), ti(cy + (ang - half_w).sin() * ray_len));
                let p3 = (ti(cx + (ang + half_w).cos() * ray_len), ti(cy + (ang + half_w).sin() * ray_len));
                draw::polygon(&mut overlay, ray_color, &[(ti(cx), ti(cy)), p2, p3], 0);
            }
        }
        self.canvas.blit(&overlay, 0, 0);

        // glow + sparks behind the card
        let glow_size = (self.vw.min(VIRTUAL_H) as f64 * 0.62) as i32;
        let pulse = 0.75 + 0.25 * (elapsed * 5.0).sin();
        let glow = rarity_glow((glow_size, glow_size), rarity_glow_color(rarity.key), 46);
        let gr = Rect::with_center(glow.w, glow.h, (ti(cx), ti(cy)));
        self.canvas.blit_with_alpha(&glow, gr.x, gr.y, (255.0 * alpha_mul * pulse) as i32);
        if anim {
            for p in &self.cutscene_particles {
                p.draw(&mut self.canvas);
            }
        }

        // the pet card, popping in
        let pop_t = ease_out_cubic((elapsed / 0.4).min(1.0));
        let scale = 0.35 + 0.65 * pop_t;
        let base = 220.max(380.min((self.vw.min(VIRTUAL_H) as f64 * 0.46) as i32));
        let income = self.state.pet_income(r_idx, mutation_key);
        let plates = vec![vec![cell(tr("Income"), tr!("+%s/sec", format_number(income)))]];
        let mut card_surf = render_pet_card(rarity, mutation_key, base, base, &plates, 0, false);
        if scale < 0.995 {
            let s = 1.max((base as f64 * scale) as i32);
            card_surf = Rc::new(transform::smoothscale(&card_surf, s, s));
        }
        let card_rect = Rect::with_center(card_surf.w, card_surf.h, (ti(cx), ti(cy)));
        if alpha_mul < 0.999 {
            self.canvas.blit_with_alpha(&card_surf, card_rect.x, card_rect.y, (255.0 * alpha_mul) as i32);
        } else {
            self.canvas.blit(&card_surf, card_rect.x, card_rect.y);
        }
        if rarity.key == "transcendente" && anim {
            draw_rainbow_border(&mut self.canvas, card_rect.inflate(-9, -9), 14, 3);
        }
        if let Some(border) = mutation(mutation_key).and_then(|m| m.border) {
            draw::rect(&mut self.canvas, border, card_rect.inflate(2, 2), 2, 14);
        }

        // big title
        let title_scale = 0.6 + 0.4 * ease_out_cubic((elapsed / 0.3).min(1.0));
        let sum = color.r as i32 + color.g as i32 + color.b as i32;
        let mut title = self.f.huge.render(&tr(style.label), if sum > 260 { color } else { Color::rgb(255, 255, 255) });
        if title_scale < 0.995 {
            let w = 1.max((title.w as f64 * title_scale) as i32);
            let h = 1.max((title.h as f64 * title_scale) as i32);
            title = Rc::new(transform::smoothscale(&title, w, h));
        }
        let tr_rect = Rect::with_center(title.w, title.h, (ti(cx), ti(card_rect.top() as f64 - 46.0)));
        if alpha_mul < 0.999 {
            self.canvas.blit_with_alpha(&title, tr_rect.x, tr_rect.y, (255.0 * alpha_mul) as i32);
        } else {
            self.canvas.blit(&title, tr_rect.x, tr_rect.y);
        }

        // skip hint
        let hint_alpha = (160.0 * alpha_mul * (0.7 + 0.3 * (elapsed * 3.0).sin())) as i32;
        let hint = self.f.small.render(&tr("Click or press any key to continue"), Color::rgb(255, 255, 255));
        let hr = Rect::with_center(hint.w, hint.h, (ti(self.vw as f64 / 2.0), VIRTUAL_H - 44));
        self.canvas.blit_with_alpha(&hint, hr.x, hr.y, hint_alpha.max(0));

        self.register_button(Rect::new(0, 0, self.vw, VIRTUAL_H), Rc::new(|g: &mut Game| g.skip_cutscene()), None);
    }
}
