//! v3.0, Cookie Clicker style: your verities drift down behind the roll screen (more of them the more you earn),
//! and every manual ROLL pops the verity you got up from where you clicked, with its name.

use super::Game;
use crate::config::VIRTUAL_H;
use crate::core::data::{mutation, rarities};
use crate::core::state::rand_uniform;
use crate::gfx::{Color, Surf, ti, transform};
use crate::ui::cards::rarity_glow_color;
use crate::ui::icons::load_pet_image;
use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;

/// most verities falling at once
const MAX_DROPS: usize = 70;
/// most pops on screen at once (fast clicking)
const MAX_POPS: usize = 8;
/// rotation steps of the cached sprites (15 degrees each)
const ANGLE_STEPS: i32 = 24;

pub struct Drop {
    x: f64,
    y: f64,
    vy: f64,
    sway: f64,
    phase: f64,
    angle: f64,
    spin: f64,
    pet: usize,
    size: i32,
    alpha: i32,
}

pub struct Pop {
    x: f64,
    y: f64,
    vy: f64,
    life: f64,
    pet: usize,
    m: &'static str,
    /// the extra verity of a Double Roll
    double: bool,
}

#[derive(Default)]
pub struct VerityFx {
    drops: Vec<Drop>,
    accum: f64,
    pops: Vec<Pop>,
}

const POP_LIFE: f64 = 1.0;

thread_local! {
    static SPRITES: RefCell<HashMap<(usize, i32, i32), Option<Surf>>> = RefCell::new(HashMap::new());
}

/// The verity's image at this size, turned to the nearest 15 degrees (cached).
fn sprite(pet: usize, size: i32, angle: f64) -> Option<Surf> {
    let step = (angle / (360.0 / ANGLE_STEPS as f64)).round() as i32 % ANGLE_STEPS;
    let key = (pet, size, step);
    if let Some(s) = SPRITES.with(|c| c.borrow().get(&key).cloned()) {
        return s;
    }
    let img = load_pet_image(rarities()[pet].pet, size, false).map(|img| {
        if step == 0 { img } else { Rc::new(transform::rotozoom(&img, step as f64 * 360.0 / ANGLE_STEPS as f64, 1.0)) }
    });
    SPRITES.with(|c| {
        let mut c = c.borrow_mut();
        if c.len() > 900 {
            c.clear();
        }
        c.insert(key, img.clone());
    });
    img
}

impl Game {
    /// The verities that rain: the ones you have equipped, or else any you own.
    fn rain_pets(&self) -> Vec<usize> {
        let mut pets: Vec<usize> = Vec::new();
        for (p, _, _) in &self.state.equipped {
            if !pets.contains(p) {
                pets.push(*p);
            }
        }
        if pets.is_empty() {
            for (k, n) in &self.state.owned {
                if *n <= 0 {
                    continue;
                }
                if let Some(Ok(i)) = k.split_once('_').map(|(i, _)| i.parse::<usize>()) {
                    if i < rarities().len() && !pets.contains(&i) {
                        pets.push(i);
                    }
                }
                if pets.len() >= 20 {
                    break;
                }
            }
        }
        pets
    }

    pub fn vfx_drop_count(&self) -> usize {
        self.vfx.drops.len()
    }

    pub fn update_verity_fx(&mut self, dt: f64) {
        if !self.animations() || self.screen_mode != "game" {
            self.vfx.drops.clear();
            self.vfx.pops.clear();
            return;
        }
        // more rain the more you earn (log scale): a light drizzle early on, a steady shower later
        let ips = self.state.income_per_second().max(0.0);
        let rate = ((ips + 1.0).log10() * 0.8).clamp(0.4, 7.0);
        self.vfx.accum += rate * dt;
        let pets = if self.vfx.accum >= 1.0 { self.rain_pets() } else { Vec::new() };
        while self.vfx.accum >= 1.0 {
            self.vfx.accum -= 1.0;
            if pets.is_empty() || self.vfx.drops.len() >= MAX_DROPS {
                continue;
            }
            let pet = pets[(rand_uniform(0.0, pets.len() as f64) as usize).min(pets.len() - 1)];
            let size = rand_uniform(30.0, 50.0) as i32 / 2 * 2;
            self.vfx.drops.push(Drop {
                x: rand_uniform(0.0, self.vw as f64),
                y: -(size as f64) - 10.0,
                vy: rand_uniform(30.0, 75.0),
                sway: rand_uniform(6.0, 22.0),
                phase: rand_uniform(0.0, 6.28),
                angle: rand_uniform(0.0, 360.0),
                spin: rand_uniform(-45.0, 45.0),
                pet,
                size,
                alpha: rand_uniform(60.0, 105.0) as i32,
            });
        }
        for d in self.vfx.drops.iter_mut() {
            d.y += d.vy * dt;
            d.phase += dt * 1.3;
            d.angle = (d.angle + d.spin * dt).rem_euclid(360.0);
        }
        self.vfx.drops.retain(|d| d.y < VIRTUAL_H as f64 + 60.0);
        for p in self.vfx.pops.iter_mut() {
            p.y += p.vy * dt;
            p.vy *= 1.0 - 2.2 * dt; // slows down as it rises
            p.life -= dt;
        }
        self.vfx.pops.retain(|p| p.life > 0.0);
    }

    /// Behind everything on the roll screen.
    pub fn draw_verity_rain(&mut self) {
        if !self.animations() {
            return;
        }
        for d in &self.vfx.drops {
            let Some(s) = sprite(d.pet, d.size, d.angle) else { continue };
            let x = d.x + d.phase.sin() * d.sway - s.w as f64 / 2.0;
            let y = d.y - s.h as f64 / 2.0;
            self.canvas.blit_with_alpha(&s, ti(x), ti(y), d.alpha);
        }
    }

    /// Like a click in Cookie Clicker: the verity you got pops up from the click, with its name.
    pub fn spawn_roll_pop(&mut self, pet: usize, m: &'static str) {
        self.spawn_roll_pop_ex(pet, m, false);
    }

    pub fn spawn_roll_pop_ex(&mut self, pet: usize, m: &'static str, double: bool) {
        if !self.animations() {
            return;
        }
        let at = self.last_click_pos.unwrap_or_else(|| {
            let c = self.main_card_rect();
            (c.centerx() as f64, (c.bottom() + 54) as f64)
        });
        if self.vfx.pops.len() >= MAX_POPS {
            self.vfx.pops.remove(0);
        }
        self.vfx.pops.push(Pop { x: at.0 + rand_uniform(-14.0, 14.0), y: at.1 - 10.0 - if double { 38.0 } else { 0.0 }, vy: -150.0, life: POP_LIFE, pet, m, double });
    }

    pub fn draw_roll_pops(&mut self) {
        let font = self.f.med.clone();
        for p in &self.vfx.pops {
            let t = (p.life / POP_LIFE).clamp(0.0, 1.0);
            let alpha = (255.0 * (t * 1.6).min(1.0)) as i32;
            let r = &rarities()[p.pet];
            let label = mutation(p.m).map(|x| x.label).unwrap_or("");
            let name = if label.is_empty() { r.pet.to_string() } else { format!("{} {}", crate::i18n::tr(label), r.pet) };
            let text = if p.double { format!("{}  +1 {}", crate::i18n::tr("DOUBLE ROLL!"), name) } else { format!("+1 {}", name) };
            let txt = font.render(&text, rarity_glow_color(r.key));
            let shadow = font.render(&text, Color::rgb(0, 0, 0));
            let img = sprite(p.pet, 40, 0.0);
            let total = txt.w + if img.is_some() { 44 } else { 0 };
            let mut x = ti(p.x) - total / 2;
            let y = ti(p.y);
            if let Some(img) = img {
                self.canvas.blit_with_alpha(&img, x, y - 20, alpha);
                x += 44;
            }
            // dark shadow so it reads over the card and the rain
            self.canvas.blit_with_alpha(&shadow, x + 2, y - txt.h / 2 + 2, alpha * 2 / 3);
            self.canvas.blit_with_alpha(&txt, x, y - txt.h / 2, alpha);
        }
    }
}
