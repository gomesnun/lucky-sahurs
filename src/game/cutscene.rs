//! Full-screen cutscene when catching a Secret+ pet (ui/cutscene_panel.py).

use super::base::Bo;
use super::{Game, cb};
use crate::theme::WHITE;
use crate::config::VIRTUAL_H;
use crate::core::data::{TIER_SECRET, mutation, rarities};
use crate::core::formatting::format_number;
use crate::gfx::{Color, Rect, Surface, draw, ti, transform};
use crate::i18n::tr;
use crate::tr;
use crate::ui::cards::{cell, rarity_glow_color, render_pet_card_phase};
use crate::ui::drawing::{draw_rainbow_border, draw_shockwave, ease_out_back, ease_out_cubic, rarity_glow};
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
        "etereo" => CutsceneStyle { duration: 3.6, rays: 26, ambient: 40.0, burst: 90, shake: 4.0, label: "ETHEREAL PET!" },
        "celestial" => CutsceneStyle { duration: 4.0, rays: 30, ambient: 48.0, burst: 110, shake: 7.0, label: "CELESTIAL PET!" },
        "absoluto" => CutsceneStyle { duration: 4.8, rays: 36, ambient: 60.0, burst: 140, shake: 10.0, label: "ABSOLUTE PET!" },
        "primordial" => CutsceneStyle { duration: 5.4, rays: 40, ambient: 70.0, burst: 170, shake: 13.0, label: "PRIMORDIAL PET!" },
        "paradoxo" => CutsceneStyle { duration: 6.0, rays: 48, ambient: 80.0, burst: 200, shake: 16.0, label: "PARADOX PET!" },
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
        if self.animations() {
            let rarity = &rarities()[rarity_index];
            let style = cutscene_style(rarity.key);
            let (cx, cy) = (self.vw / 2, VIRTUAL_H / 2);
            let col = rarity_glow_color(rarity.key);
            let speed = 1.6 + 0.15 * (rarity.tier as f64 - TIER_SECRET as f64);
            for k in 0..style.burst {
                let c = if k % 3 != 0 { col } else { Color::rgb(255, 255, 255) };
                self.cutscene_particles.push(Particle::with_speed(cx as f64, cy as f64, c, speed));
            }
        }
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

    /// The button inside a cutscene: no more cutscenes for this rarity (Options > Game turns them back on).
    pub fn turn_off_cutscenes(&mut self, key: &'static str) {
        self.settings.set_bool(&format!("cutscenes_{}", key), false);
        crate::storage::save_settings(&self.settings);
        self.cutscene_queue.retain(|q| rarities()[q.0].key != key);
        self.skip_cutscene();
        let name = rarities().iter().find(|r| r.key == key).map(|r| tr(r.name)).unwrap_or_default();
        self.show_toast(&tr!("%s cutscenes turned off. Turn them back on in Options > Game.", name), 3.0);
    }

    pub fn draw_cutscene(&mut self, mouse_pos: (f64, f64)) {
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
        let mut color = Color::rgb(rarity.color.r, rarity.color.g, rarity.color.b);
        if (color.r as i32 + color.g as i32 + color.b as i32) < 120 {
            // dark rarities (Secret, Absolute): the rays use the bright colour
            color = rarity_glow_color(rarity.key);
        }

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

        // shockwaves from the card (more rings the better the rarity)
        if anim {
            let n_waves = 2 + (rarity.tier as i32 - TIER_SECRET as i32).max(0) / 2;
            let wave_col = rarity_glow_color(rarity.key);
            for k in 0..n_waves {
                let t = (elapsed - k as f64 * 0.18) / 0.9;
                if (0.0..=1.0).contains(&t) {
                    let c = if k % 2 == 0 { wave_col } else { Color::rgb(255, 255, 255) };
                    let r = 60.0 + t * self.vw.min(VIRTUAL_H) as f64 * 0.55;
                    draw_shockwave(&mut self.canvas, (ti(cx), ti(cy)), r, c, 200.0 * (1.0 - t) * alpha_mul, 8.0 * (1.0 - t) + 2.0);
                }
            }
        }

        // the pet card: an elastic "pop" spinning into place
        let pop_t = if anim { ease_out_back((elapsed / 0.55).min(1.0)) } else { 1.0 };
        let scale = 0.35 + 0.65 * pop_t;
        let spin_deg = if anim { (1.0 - ease_out_cubic((elapsed / 0.55).min(1.0))) * -14.0 } else { 0.0 };
        let base = 220.max(380.min((self.vw.min(VIRTUAL_H) as f64 * 0.46) as i32));
        let income = self.state.pet_income(r_idx, mutation_key);
        let plates = vec![vec![cell(tr("Income"), tr!("+%s/sec", format_number(income)))]];
        let mut card_surf = render_pet_card_phase(rarity, mutation_key, self.state.phase(r_idx, mutation_key), base, base, &plates, 0, false);
        if (scale - 1.0).abs() > 0.005 {
            let s = 1.max((base as f64 * scale) as i32);
            card_surf = Rc::new(transform::smoothscale(&card_surf, s, s));
        }
        if spin_deg.abs() > 0.3 {
            card_surf = Rc::new(transform::rotozoom(&card_surf, spin_deg, 1.0));
        }
        let mut card_rect = Rect::with_center(card_surf.w, card_surf.h, (ti(cx), ti(cy)));
        if alpha_mul < 0.999 {
            self.canvas.blit_with_alpha(&card_surf, card_rect.x, card_rect.y, (255.0 * alpha_mul) as i32);
        } else {
            self.canvas.blit(&card_surf, card_rect.x, card_rect.y);
        }
        if spin_deg.abs() > 0.3 {
            // the borders on top (rainbow / mutation) use the upright card's size
            let s = (base as f64 * scale) as i32;
            card_rect = Rect::with_center(s, s, (ti(cx), ti(cy)));
        }
        if rarity.key == "transcendente" && anim {
            draw_rainbow_border(&mut self.canvas, card_rect.inflate(-9, -9), 14, 3);
        }
        if let Some(border) = mutation(mutation_key).and_then(|m| m.border).filter(|_| mutation_key != "rainbow") {
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

        // a white flash on opening (stronger on the better rarities)
        if anim && elapsed < 0.3 {
            let mut flash = Surface::new_alpha(self.vw, VIRTUAL_H);
            let strength = 230.min(120 + 18 * (rarity.tier as i32 - TIER_SECRET as i32)) as f64;
            flash.fill(Color::rgba(255, 255, 255, (strength * (1.0 - elapsed / 0.3)) as i32 as u8), None);
            self.canvas.blit(&flash, 0, 0);
        }

        // skip hint
        let hint_alpha = (160.0 * alpha_mul * (0.7 + 0.3 * (elapsed * 3.0).sin())) as i32;
        let hint = self.f.small.render(&tr("Click or press any key to continue"), Color::rgb(255, 255, 255));
        let hr = Rect::with_center(hint.w, hint.h, (ti(self.vw as f64 / 2.0), VIRTUAL_H - 44));
        self.canvas.blit_with_alpha(&hint, hr.x, hr.y, hint_alpha.max(0));

        self.register_button(Rect::new(0, 0, self.vw, VIRTUAL_H), Rc::new(|g: &mut Game| g.skip_cutscene()), None);

        // v3.0.1: turn this rarity's cutscenes off right here (registered after the skip area, so it wins the click)
        let key = rarity.key;
        let sb = self.f.small_b.clone();
        let label = tr!("Turn off %s cutscenes", tr(rarity.name));
        let w = sb.render(&label, WHITE).w + 36;
        let r = Rect::new(self.vw / 2 - w / 2, VIRTUAL_H - 104, w, 40);
        self.button(r, &label, &sb, mouse_pos, Color::rgba(20, 20, 30, 200), Color::rgb(170, 60, 60), WHITE, cb(move |g| g.turn_off_cutscenes(key)), Bo::r(10).border(Some(Color::rgb(255, 255, 255))));
    }
}
