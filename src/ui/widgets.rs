//! Reusable pieces: particles, sliding side panels and the text field (ui/widgets.py).

use crate::core::state::rand_uniform;
use crate::gfx::{Color, Surface, draw, ti};
use crate::ui::drawing::ease_out_cubic;
use std::collections::HashMap;

/// Text of the account screen / friend search / chat / feedback (text + caret, in characters).
#[derive(Clone, Debug)]
pub struct TextField {
    pub kind: &'static str,
    pub text: String,
    pub caret: usize,
    pub max_len: usize,
}

fn py_isprintable(c: char) -> bool {
    if c == ' ' {
        return true;
    }
    !(c.is_control() || c.is_whitespace() || ('\u{200b}'..='\u{200f}').contains(&c) || ('\u{2028}'..='\u{202e}').contains(&c) || c == '\u{feff}')
}

impl TextField {
    pub fn new(kind: &'static str) -> TextField {
        let max_len = match kind {
            "username" => 16,
            "email" => 80,
            "code" => 6,
            "text" => 280,
            "digits" => 9,
            _ => 64,
        };
        TextField { kind, text: String::new(), caret: 0, max_len }
    }

    pub fn len(&self) -> usize {
        self.text.chars().count()
    }

    pub fn set_text(&mut self, text: &str) {
        self.text = text.to_string();
        self.caret = self.len();
    }

    fn clamp(&mut self) {
        self.caret = self.caret.min(self.len());
    }

    pub fn add(&mut self, s: &str) {
        for ch in s.chars() {
            let mut ch = ch;
            if self.kind == "username" {
                let low: Vec<char> = ch.to_lowercase().collect();
                if low.len() != 1 {
                    continue;
                }
                ch = low[0];
                if !(ch.is_ascii() && (ch.is_ascii_alphanumeric() || ch == '_')) {
                    continue;
                }
            } else if self.kind == "code" || self.kind == "digits" {
                if !ch.is_ascii_digit() {
                    continue;
                }
            } else if !py_isprintable(ch) {
                continue;
            }
            if ch == ' ' && (self.kind != "text" || self.text.trim().is_empty() || self.text.ends_with("  ")) {
                continue;
            }
            if self.len() >= self.max_len {
                break;
            }
            self.clamp();
            let mut chars: Vec<char> = self.text.chars().collect();
            chars.insert(self.caret, ch);
            self.text = chars.into_iter().collect();
            self.caret += 1;
        }
    }

    pub fn backspace(&mut self) {
        self.clamp();
        if self.caret > 0 {
            let mut chars: Vec<char> = self.text.chars().collect();
            chars.remove(self.caret - 1);
            self.text = chars.into_iter().collect();
            self.caret -= 1;
        }
    }

    pub fn delete(&mut self) {
        self.clamp();
        let mut chars: Vec<char> = self.text.chars().collect();
        if self.caret < chars.len() {
            chars.remove(self.caret);
        }
        self.text = chars.into_iter().collect();
    }

    pub fn move_by(&mut self, step: i64) {
        self.caret = (self.caret as i64 + step).clamp(0, self.len() as i64) as usize;
    }
    pub fn home(&mut self) {
        self.caret = 0;
    }
    pub fn end(&mut self) {
        self.caret = self.len();
    }
}

thread_local! {
    static STAR_CACHE: std::cell::RefCell<HashMap<(u32, i32), std::rc::Rc<Surface>>> = std::cell::RefCell::new(HashMap::new());
}

/// A 4-pointed star (a sparkle) with a white centre, cached by colour and size.
fn star_sprite(color: Color, r: i32) -> std::rc::Rc<Surface> {
    let key = (Color::rgb(color.r, color.g, color.b).argb(), r);
    if let Some(s) = STAR_CACHE.with(|c| c.borrow().get(&key).cloned()) {
        return s;
    }
    let size = r * 2 + 2;
    let mut surf = Surface::new_alpha(size, size);
    let c = size as f64 / 2.0;
    let pts: Vec<(i32, i32)> = (0..8)
        .map(|i| {
            let a = i as f64 * std::f64::consts::PI / 4.0;
            let rr = if i % 2 == 0 { r as f64 } else { r as f64 * 0.32 };
            ((c + a.cos() * rr) as i32, (c + a.sin() * rr) as i32)
        })
        .collect();
    draw::polygon(&mut surf, Color::rgba(color.r, color.g, color.b, 255), &pts, 0);
    draw::circle(&mut surf, Color::rgba(255, 255, 255, 230), (c as i32, c as i32), 1.max(r / 3), 0);
    let surf = std::rc::Rc::new(surf);
    STAR_CACHE.with(|cache| {
        let mut cache = cache.borrow_mut();
        if cache.len() > 300 {
            cache.clear();
        }
        cache.insert(key, surf.clone());
    });
    surf
}

/// A spark that jumps off the card: 2/3 are little spinning, twinkling stars, the rest dots.
pub struct Particle {
    pub x: f64,
    pub y: f64,
    pub vx: f64,
    pub vy: f64,
    pub life: f64,
    pub max_life: f64,
    pub color: Color,
    pub radius: f64,
    pub star: bool,
    pub spin: f64,
    pub twinkle: f64,
}

impl Particle {
    pub fn new(x: f64, y: f64, color: Color) -> Particle {
        Particle::with_speed(x, y, color, 1.0)
    }

    pub fn with_speed(x: f64, y: f64, color: Color, speed_mult: f64) -> Particle {
        let angle = rand_uniform(0.0, 2.0 * std::f64::consts::PI);
        let speed = rand_uniform(60.0, 190.0) * speed_mult;
        let life = rand_uniform(0.5, 0.95);
        let radius = rand_uniform(2.5, 4.5);
        let star = crate::core::state::rand_random() < 0.66;
        let spin = rand_uniform(0.0, 6.28);
        let twinkle = rand_uniform(8.0, 16.0);
        Particle {
            x,
            y,
            vx: angle.cos() * speed,
            vy: angle.sin() * speed - 60.0,
            life,
            max_life: life,
            color: Color::rgb(color.r, color.g, color.b),
            radius,
            star,
            spin,
            twinkle,
        }
    }

    pub fn update(&mut self, dt: f64) {
        self.x += self.vx * dt;
        self.y += self.vy * dt;
        self.vx *= 1.0 - 0.9 * dt; // slows down a little in the air (feels "lighter")
        self.vy += 140.0 * dt;
        self.spin += dt * 5.0;
        self.life -= dt;
    }

    pub fn draw(&self, canvas: &mut Surface) {
        if self.life <= 0.0 {
            return;
        }
        let t = (self.life / self.max_life).max(0.0);
        if self.star {
            let flick = 0.65 + 0.35 * (self.spin * self.twinkle * 0.3).sin();
            let r = 2.max((self.radius * 1.9 * (0.4 + 0.6 * t) * flick) as i32);
            let spr = star_sprite(self.color, r);
            let mut spr = crate::gfx::transform::rotozoom(&spr, self.spin.to_degrees() % 90.0, 1.0);
            spr.set_alpha((255.0 * (t * 1.4).min(1.0)) as i32);
            canvas.blit(&spr, ti(self.x - spr.w as f64 / 2.0), ti(self.y - spr.h as f64 / 2.0));
            return;
        }
        let r = 1.max((self.radius * t + 1.0) as i32);
        let mut s = Surface::new_alpha(r * 2, r * 2);
        draw::circle(&mut s, Color::rgba(self.color.r, self.color.g, self.color.b, (255.0 * t) as i32 as u8), (r, r), r, 0);
        canvas.blit(&s, ti(self.x - r as f64), ti(self.y - r as f64));
    }
}

/// Panel that slides in from a side.
pub struct SlidePanel {
    #[allow(dead_code)]
    pub side: &'static str,
    pub content: Option<&'static str>,
    pub pending: Option<&'static str>,
    pub progress: f64,
    pub target: f64,
    pub scroll: HashMap<Option<&'static str>, f64>,
    pub max_scroll: HashMap<Option<&'static str>, f64>,
}

impl SlidePanel {
    pub const SPEED: f64 = 5.5;

    pub fn new(side: &'static str) -> SlidePanel {
        SlidePanel { side, content: None, pending: None, progress: 0.0, target: 0.0, scroll: HashMap::new(), max_scroll: HashMap::new() }
    }

    pub fn toggle(&mut self, content: &'static str) {
        if self.content == Some(content) && self.target == 1.0 {
            self.close();
        } else {
            self.open(content);
        }
    }

    pub fn open(&mut self, content: &'static str) {
        if self.content.is_none() || self.progress <= 0.0 || self.content == Some(content) {
            self.content = Some(content);
            self.pending = None;
            self.target = 1.0;
        } else {
            self.pending = Some(content);
            self.target = 0.0;
        }
    }

    pub fn close(&mut self) {
        self.pending = None;
        self.target = 0.0;
    }

    pub fn update(&mut self, dt: f64) {
        if self.progress < self.target {
            self.progress = self.target.min(self.progress + dt * Self::SPEED);
        } else if self.progress > self.target {
            self.progress = self.target.max(self.progress - dt * Self::SPEED);
        }
        if self.progress <= 0.0 && self.pending.is_some() {
            self.content = self.pending.take();
            self.target = 1.0;
        }
    }

    pub fn visible(&self) -> bool {
        self.content.is_some() && self.progress > 0.002
    }

    pub fn is_open(&self) -> bool {
        self.target == 1.0 && self.content.is_some()
    }

    pub fn shown_width(&self, width: i32) -> i32 {
        (width as f64 * ease_out_cubic(self.progress)) as i32
    }

    pub fn get_scroll(&self) -> f64 {
        self.scroll.get(&self.content).copied().unwrap_or(0.0)
    }

    pub fn add_scroll(&mut self, amount: f64) {
        let cur = self.get_scroll();
        let limit = self.max_scroll.get(&self.content).copied().unwrap_or(0.0);
        self.scroll.insert(self.content, (cur + amount).min(limit).max(0.0));
    }

    pub fn set_scroll_abs(&mut self, value: f64) {
        let limit = self.max_scroll.get(&self.content).copied().unwrap_or(0.0);
        self.scroll.insert(self.content, value.min(limit).max(0.0));
    }

    pub fn set_max_scroll(&mut self, value: f64) {
        let m = value.max(0.0);
        self.max_scroll.insert(self.content, m);
        if self.get_scroll() > m {
            self.scroll.insert(self.content, m);
        }
    }
}
