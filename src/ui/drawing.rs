//! Drawing: panels, gradients, bars and the rarity backgrounds (ui/drawing.py).
//! The ALPHA_IS_SLOW (Android) paths are not needed on desktop: bake* are the identity there.

use crate::core::data::Rarity;
use crate::gfx::{BLEND_RGBA_MIN, Color, Rect, Surf, Surface, draw, transform};
use crate::pyrand::PyRandom;
use crate::theme::{BORDER_W, BORDER_W_SMALL, dark_mode, outline, panel};
use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;

/// colorsys.hsv_to_rgb
pub fn hsv_to_rgb(h: f64, s: f64, v: f64) -> (f64, f64, f64) {
    if s == 0.0 {
        return (v, v, v);
    }
    let i = (h * 6.0).trunc();
    let f = h * 6.0 - i;
    let p = v * (1.0 - s);
    let q = v * (1.0 - s * f);
    let t = v * (1.0 - s * (1.0 - f));
    match (i as i64).rem_euclid(6) {
        0 => (v, t, p),
        1 => (q, v, p),
        2 => (p, v, t),
        3 => (p, q, v),
        4 => (t, p, v),
        _ => (v, p, q),
    }
}

pub fn hsv_color(h: f64, s: f64, v: f64) -> Color {
    let (r, g, b) = hsv_to_rgb(h, s, v);
    Color::rgb((r * 255.0) as u8, (g * 255.0) as u8, (b * 255.0) as u8)
}

fn lerp_i(a: u8, b: u8, t: f64) -> u8 {
    (a as f64 + (b as f64 - a as f64) * t) as i64 as u8
}

pub fn make_vertical_gradient(w: i32, h: i32, top: Color, bottom: Color) -> Surface {
    let mut s = Surface::new(w, h);
    for y in 0..h {
        let t = y as f64 / (h - 1).max(1) as f64;
        let c = Color::rgb(lerp_i(top.r, bottom.r, t), lerp_i(top.g, bottom.g, t), lerp_i(top.b, bottom.b, t));
        draw::line(&mut s, c, (0, y), (w, y), 1);
    }
    s
}

pub fn make_game_background(w: i32, h: i32, top: Color, bottom: Color, dark: bool) -> Surface {
    let mut s = make_vertical_gradient(w, h, top, bottom);
    let mut rng = PyRandom::from_int(1789980865);
    let (target, low, high) = if dark { (Color::rgb(255, 255, 255), 0.12, 0.42) } else { (Color::rgb(0, 0, 0), 0.16, 0.52) };
    let n = 50.max((w as i64 * h as i64) / 4600);
    for _ in 0..n {
        let x = rng.randrange(w as i64) as i32;
        let y = rng.randrange(h as i64) as i32;
        let t = y as f64 / (h - 1).max(1) as f64;
        let base = mix(top, bottom, t);
        let dot = mix(base, target, rng.uniform(low, high));
        let r = if rng.random() < 0.82 { 1 } else { 2 };
        draw::circle(&mut s, dot, (x, y), r, 0);
    }
    s
}

/// Mix two colours (t=0 -> c1, t=1 -> c2).
pub fn mix(c1: Color, c2: Color, t: f64) -> Color {
    Color::rgb(lerp_i(c1.r, c2.r, t), lerp_i(c1.g, c2.g, t), lerp_i(c1.b, c2.b, t))
}

pub fn shade(c: Color, factor: f64) -> Color {
    let f = |v: u8| ((v as f64 * factor) as i64).clamp(0, 255) as u8;
    Color::rgb(f(c.r), f(c.g), f(c.b))
}

pub fn ease_out_cubic(t: f64) -> f64 {
    let t = t.clamp(0.0, 1.0);
    1.0 - (1.0 - t).powi(3)
}

type Cache<K> = RefCell<HashMap<K, Surf>>;

thread_local! {
    static GRADIENT: Cache<(i32, i32, u32, u32, i32)> = RefCell::new(HashMap::new());
    static SHADOW: Cache<(i32, i32, i32)> = RefCell::new(HashMap::new());
    static PANEL: Cache<(i32, i32, i32, u32, i32, u32)> = RefCell::new(HashMap::new());
    static DIM: Cache<(i32, i32, i32, bool)> = RefCell::new(HashMap::new());
    static VSHIFT: RefCell<HashMap<String, Rc<Vec<Surface>>>> = RefCell::new(HashMap::new());
    static BAR_FILL: Cache<(&'static str, u32, i32, i32)> = RefCell::new(HashMap::new());
    static RAINBOW: Cache<(&'static str, i32, i32, i32, i32)> = RefCell::new(HashMap::new());
    static GLOW: Cache<(i32, i32, u32, i32)> = RefCell::new(HashMap::new());
    static RARITY_BG: Cache<(&'static str, i32, i32, i32)> = RefCell::new(HashMap::new());
}

pub fn rounded_mask(w: i32, h: i32, radius: i32) -> Surface {
    let mut m = Surface::new_alpha(w, h);
    let mr = m.get_rect();
    draw::rect(&mut m, Color::rgba(255, 255, 255, 255), mr, 0, radius);
    m
}

/// Vertical gradient clipped to a rounded rectangle.
pub fn rounded_gradient(w: i32, h: i32, top: Color, bottom: Color, radius: i32) -> Surf {
    let key = (w, h, top.argb(), bottom.argb(), radius);
    if let Some(s) = GRADIENT.with(|c| c.borrow().get(&key).cloned()) {
        return s;
    }
    let mut s = Surface::new_alpha(w, h);
    for y in 0..h {
        let t = y as f64 / (h - 1).max(1) as f64;
        draw::line(&mut s, mix(top, bottom, t), (0, y), (w, y), 1);
    }
    s.blit_ex(&rounded_mask(w, h, radius), 0, 0, None, BLEND_RGBA_MIN);
    let s = Rc::new(s);
    GRADIENT.with(|c| {
        let mut c = c.borrow_mut();
        if c.len() > 200 {
            c.clear();
        }
        c.insert(key, s.clone());
    });
    s
}

/// The dark veil behind the pages.
pub fn dim_overlay(w: i32, h: i32, alpha: i32) -> Surf {
    let key = (w, h, alpha, dark_mode());
    if let Some(s) = DIM.with(|c| c.borrow().get(&key).cloned()) {
        return s;
    }
    let mut s = Surface::new_alpha(w, h);
    s.fill(Color::rgba(0, 0, 0, alpha.clamp(0, 255) as u8), None);
    let s = Rc::new(s);
    DIM.with(|c| {
        let mut c = c.borrow_mut();
        c.clear();
        c.insert(key, s.clone());
    });
    s
}

/// Rounded rectangle (fill + outline), cached.
pub fn rounded_box(w: i32, h: i32, color: Color, radius: i32, border_w: i32) -> Surf {
    let ol = outline();
    let key = (w, h, radius, Color::rgb(color.r, color.g, color.b).argb(), border_w, ol.argb());
    if let Some(s) = PANEL.with(|c| c.borrow().get(&key).cloned()) {
        return s;
    }
    let mut b = Surface::new_alpha(w, h);
    let r = b.get_rect();
    draw::rect(&mut b, color, r, 0, radius);
    if border_w > 0 {
        draw::rect(&mut b, ol, r, border_w, radius);
    }
    let b = Rc::new(b);
    PANEL.with(|c| {
        let mut c = c.borrow_mut();
        if c.len() > 600 {
            c.clear();
        }
        c.insert(key, b.clone());
    });
    b
}

/// Panel with a thick outline. border None -> 4px with shadow, 3px without.
pub fn draw_panel(canvas: &mut Surface, rect: Rect, color: Option<Color>, radius: i32, shadow: bool, border: Option<i32>) {
    let color = color.unwrap_or_else(panel);
    if shadow {
        let key = (rect.w, rect.h, radius);
        let s = SHADOW.with(|c| c.borrow().get(&key).cloned());
        let s = match s {
            Some(s) => s,
            None => {
                let mut s = Surface::new_alpha(rect.w + 14, rect.h + 14);
                draw::rect(&mut s, Color::rgba(0, 0, 0, 80), Rect::new(7, 11, rect.w, rect.h), 0, radius);
                let s = Rc::new(s);
                SHADOW.with(|c| {
                    let mut c = c.borrow_mut();
                    if c.len() > 400 {
                        c.clear();
                    }
                    c.insert(key, s.clone());
                });
                s
            }
        };
        canvas.blit(&s, rect.x - 7, rect.y - 7);
    }
    let bw = border.unwrap_or(if shadow { BORDER_W } else { BORDER_W_SMALL });
    let b = rounded_box(rect.w, rect.h, color, radius, bw);
    canvas.blit(&b, rect.x, rect.y);
}

/// draw_panel with the defaults (PANEL colour, radius 14, shadow).
pub fn panel_box(canvas: &mut Surface, rect: Rect) {
    draw_panel(canvas, rect, None, 14, true, None);
}

pub const VSHIFT_STEPS: i32 = 8;

fn vshift_variants(surf: &Surface, steps: i32) -> Vec<Surface> {
    let (w, h) = surf.get_size();
    let mut hi = Surface::new_alpha(w * steps, (h + 4) * steps);
    hi.blit(&transform::scale(surf, w * steps, h * steps), 0, 2 * steps);
    (0..steps)
        .map(|k| {
            let win = Rect::new(0, steps - k, w * steps, (h + 2) * steps);
            transform::smoothscale(&hi.sub_copy(win), w, h + 2)
        })
        .collect()
}

pub fn clear_drawing_caches() {
    VSHIFT.with(|c| c.borrow_mut().clear());
    SHADOW.with(|c| c.borrow_mut().clear());
    DIM.with(|c| c.borrow_mut().clear());
    PANEL.with(|c| c.borrow_mut().clear());
}

/// Draws a STATIC sprite at a fractional y (8 sub-pixel variants, cached by key).
pub fn blit_smooth_y(canvas: &mut Surface, key: &str, build: impl FnOnce() -> Surface, x: f64, y: f64) {
    let v = VSHIFT.with(|c| c.borrow().get(key).cloned());
    let v = match v {
        Some(v) => v,
        None => {
            let v = Rc::new(vshift_variants(&build(), VSHIFT_STEPS));
            VSHIFT.with(|c| c.borrow_mut().insert(key.to_string(), v.clone()));
            v
        }
    };
    let iy = y.floor() as i32;
    let k = ((VSHIFT_STEPS - 1) as i64).min(((y - iy as f64) * VSHIFT_STEPS as f64) as i64) as usize;
    canvas.blit(&v[k], x as i32, iy - 1);
}

/// Coloured border INSIDE the black outline (state: max, hover, active...).
pub fn draw_state_border(canvas: &mut Surface, rect: Rect, color: Color, radius: i32, width: i32) {
    draw::rect(canvas, color, rect.inflate(-9, -9), width, 2.max(radius - 4));
}

/// Fill of the Golden / Diamond / Rainbow bars (rounded, cached). kind "rainbow" ignores color.
pub fn bar_fill_surface(kind: &'static str, color: Option<Color>, w: i32, h: i32) -> Surf {
    let key = (kind, color.map(|c| c.argb()).unwrap_or(0), w, h);
    if let Some(s) = BAR_FILL.with(|c| c.borrow().get(&key).cloned()) {
        return s;
    }
    let mut s = Surface::new_alpha(w, h);
    if kind == "rainbow" {
        for x in 0..w {
            let c = hsv_color(0.83 * x as f64 / w.max(1) as f64, 0.65, 1.0);
            draw::line(&mut s, c, (x, 0), (x, h), 1);
        }
    } else {
        let color = color.unwrap_or(Color::rgb(0, 0, 0));
        for yy in 0..h {
            let t = yy as f64 / (h - 1).max(1) as f64;
            let f = |v: u8| (v as f64 * (1.0 - 0.30 * t)) as i64 as u8;
            draw::line(&mut s, Color::rgb(f(color.r), f(color.g), f(color.b)), (0, yy), (w, yy), 1);
        }
    }
    s.blit_ex(&rounded_mask(w, h, h / 2), 0, 0, None, BLEND_RGBA_MIN);
    let s = Rc::new(s);
    BAR_FILL.with(|c| c.borrow_mut().insert(key, s.clone()));
    s
}

fn rainbow_gradient(w: i32, h: i32, sat: f64) -> Surface {
    let mut s = Surface::new_alpha(w, h);
    for x in 0..w {
        draw::line(&mut s, hsv_color(0.83 * x as f64 / w.max(1) as f64, sat, 1.0), (x, 0), (x, h), 1);
    }
    s
}

/// Rainbow glow behind the ROLL button (callers blit it with their own alpha).
pub fn rainbow_glow_surface(w: i32, h: i32, radius: i32) -> Surf {
    let key = ("glow", w, h, radius, 0);
    if let Some(s) = RAINBOW.with(|c| c.borrow().get(&key).cloned()) {
        return s;
    }
    let mut s = rainbow_gradient(w, h, 0.8);
    s.blit_ex(&rounded_mask(w, h, radius), 0, 0, None, BLEND_RGBA_MIN);
    let s = Rc::new(s);
    RAINBOW.with(|c| c.borrow_mut().insert(key, s.clone()));
    s
}

/// Rainbow outline (like draw_state_border); `rect` is the already-shrunk rectangle.
pub fn draw_rainbow_border(canvas: &mut Surface, rect: Rect, radius: i32, width: i32) {
    let key = ("border", rect.w, rect.h, radius, width);
    let s = RAINBOW.with(|c| c.borrow().get(&key).cloned());
    let s = match s {
        Some(s) => s,
        None => {
            let mut s = rainbow_gradient(rect.w, rect.h, 0.85);
            let mut ring = Surface::new_alpha(rect.w, rect.h);
            let rr = ring.get_rect();
            draw::rect(&mut ring, Color::rgba(255, 255, 255, 255), rr, width, radius);
            s.blit_ex(&ring, 0, 0, None, BLEND_RGBA_MIN);
            let s = Rc::new(s);
            RAINBOW.with(|c| c.borrow_mut().insert(key, s.clone()));
            s
        }
    };
    canvas.blit(&s, rect.x, rect.y);
}

/// Soft glow (rarity colour) behind the last-pet card.
pub fn rarity_glow(size: (i32, i32), color: Color, spread: i32) -> Surf {
    let key = (size.0, size.1, Color::rgb(color.r, color.g, color.b).argb(), spread);
    if let Some(s) = GLOW.with(|c| c.borrow().get(&key).cloned()) {
        return s;
    }
    let (w, h) = size;
    let mut g = Surface::new_alpha(w + 2 * spread, h + 2 * spread);
    let mut i = spread;
    while i > 0 {
        let a = (140.0 * ((spread - i) as f64 / spread as f64).powi(2)) as i32;
        draw::rect(
            &mut g,
            Color::rgba(color.r, color.g, color.b, a as u8),
            Rect::new(spread - i, spread - i, w + 2 * i, h + 2 * i),
            0,
            12 + i,
        );
        i -= 2;
    }
    let g = Rc::new(g);
    GLOW.with(|c| c.borrow_mut().insert(key, g.clone()));
    g
}

fn paint_cosmic_bg(bg: &mut Surface, c1: Color, c2: Color, w: i32, h: i32) {
    for y in 0..h {
        let t = y as f64 / (h - 1).max(1) as f64;
        let l = |a: u8, b: u8| (a as f64 + (b as f64 - a as f64) * t) as i64 as u8;
        draw::line(bg, Color::rgba(l(c2.r, c1.r), l(c2.g, c1.g), l(c2.b, c1.b), 255), (0, y), (w, y), 1);
    }
    let mut rng = PyRandom::from_int(7);
    const COLS: [Color; 3] = [Color::rgba(255, 255, 255, 255), Color::rgba(170, 230, 255, 255), Color::rgba(255, 190, 240, 255)];
    for _ in 0..10.max((w as i64 * h as i64) / 700) {
        let x = rng.randrange(w as i64) as i32;
        let y = rng.randrange(h as i64) as i32;
        let col = COLS[rng.choice_index(3)];
        if rng.random() < 0.2 {
            draw::line(bg, col, (x - 3, y), (x + 3, y), 1);
            draw::line(bg, col, (x, y - 3), (x, y + 3), 1);
        } else {
            draw::circle(bg, col, (x, y), 1, 0);
        }
    }
}

fn paint_transcendent_bg(bg: &mut Surface, w: i32, h: i32) {
    let total = w + h;
    let mut k = 0;
    while k < total + 6 {
        let c = hsv_color((k as f64 / total as f64) % 1.0, 0.38, 1.0);
        draw::line(bg, c, (k, 0), (k - h, h), 4);
        k += 3;
    }
    let mut rng = PyRandom::from_int(11);
    let white = Color::rgba(255, 255, 255, 255);
    for _ in 0..4.max((w as i64 * h as i64) / 2500) {
        let x = rng.randrange(w as i64) as i32;
        let y = rng.randrange(h as i64) as i32;
        draw::line(bg, white, (x - 3, y), (x + 3, y), 1);
        draw::line(bg, white, (x, y - 3), (x, y + 3), 1);
    }
}

fn lerp_line(bg: &mut Surface, a: Color, b: Color, t: f64, y: i32, w: i32) {
    let l = |x: u8, z: u8| (x as f64 + (z as f64 - x as f64) * t) as i64 as u8;
    draw::line(bg, Color::rgba(l(a.r, b.r), l(a.g, b.g), l(a.b, b.b), 255), (0, y), (w, y), 1);
}

/// Frosted glass: ice-blue -> almost white gradient, with diagonal shine bands and specks of light.
fn paint_ethereal_bg(bg: &mut Surface, c1: Color, c2: Color, w: i32, h: i32) {
    for y in 0..h {
        lerp_line(bg, c2, c1, y as f64 / (h - 1).max(1) as f64, y, w);
    }
    let mut shine = Surface::new_alpha(w, h);
    for (off, width, alpha) in [(0.15, 0.10, 90u8), (0.38, 0.05, 70), (0.70, 0.14, 55)] {
        let x = (off * (w + h) as f64) as i32 - h;
        let ww = (width * w as f64) as i32;
        draw::polygon(&mut shine, Color::rgba(255, 255, 255, alpha), &[(x, h), (x + ww, h), (x + ww + h, 0), (x + h, 0)], 0);
    }
    bg.blit(&shine, 0, 0);
    let mut rng = PyRandom::from_int(23);
    for _ in 0..6.max((w as i64 * h as i64) / 1400) {
        let x = rng.randrange(w as i64) as i32;
        let y = rng.randrange(h as i64) as i32;
        let r = [1, 1, 2][rng.choice_index(3)];
        draw::circle(bg, Color::rgba(255, 255, 255, 255), (x, y), r, 0);
    }
}

/// White-gold with sun rays from the top and constellations (stars joined by thin lines).
fn paint_celestial_bg(bg: &mut Surface, c1: Color, c2: Color, w: i32, h: i32) {
    for y in 0..h {
        lerp_line(bg, c1, c2, y as f64 / (h - 1).max(1) as f64 * 0.55, y, w);
    }
    let mut rays = Surface::new_alpha(w, h);
    let cx = w / 2;
    let big = (w + h) as f64;
    for i in 0..9 {
        let a = std::f64::consts::PI * (0.1 + 0.8 * i as f64 / 8.0);
        let tip = ((cx as f64 + a.cos() * big) as i32, (a.sin() * big) as i32);
        let tip2 = ((cx as f64 + (a + 0.07).cos() * big) as i32, ((a + 0.07).sin() * big) as i32);
        draw::polygon(&mut rays, Color::rgba(255, 255, 255, 60), &[(cx, -4), tip, tip2], 0);
    }
    bg.blit(&rays, 0, 0);
    let mut rng = PyRandom::from_int(31);
    let line_col = Color::rgba(180, 130, 30, 255);
    for _ in 0..2.max((w as i64 * h as i64) / 9000) {
        let mut pts = vec![(rng.randrange(w as i64) as i32, rng.randrange(h as i64) as i32)];
        for _ in 0..3 {
            let (px, py) = *pts.last().unwrap();
            let nx = (px as i64 + rng.randint(-(w as i64) / 5, w as i64 / 5)).clamp(0, w as i64 - 1) as i32;
            let ny = (py as i64 + rng.randint(-(h as i64) / 6, h as i64 / 6)).clamp(0, h as i64 - 1) as i32;
            pts.push((nx, ny));
        }
        draw::lines(bg, line_col, false, &pts, 1);
        for &(px, py) in &pts {
            draw::circle(bg, Color::rgba(255, 255, 255, 255), (px, py), 2, 0);
            draw::circle(bg, line_col, (px, py), 2, 1);
        }
    }
}

/// Black with golden rays from the centre and a golden ring: the final rarity.
fn paint_absolute_bg(bg: &mut Surface, c1: Color, c2: Color, w: i32, h: i32) {
    bg.fill(Color::rgba(c1.r, c1.g, c1.b, 255), None);
    let (cx, cy) = (w as f64 / 2.0, h as f64 * 0.45);
    let mut rays = Surface::new_alpha(w, h);
    for i in 0..16 {
        let a = std::f64::consts::TAU * i as f64 / 16.0;
        let r = (w + h) as f64;
        let p2 = ((cx + (a - 0.06).cos() * r) as i32, (cy + (a - 0.06).sin() * r) as i32);
        let p3 = ((cx + (a + 0.06).cos() * r) as i32, (cy + (a + 0.06).sin() * r) as i32);
        draw::polygon(&mut rays, Color::rgba(c2.r, c2.g, c2.b, if i % 2 == 0 { 70 } else { 35 }), &[(cx as i32, cy as i32), p2, p3], 0);
    }
    bg.blit(&rays, 0, 0);
    let ring_r = (w.min(h) as f64 * 0.40) as i32;
    draw::circle(bg, Color::rgba(c2.r, c2.g, c2.b, 255), (cx as i32, cy as i32), ring_r, 2.max(w / 80));
    let mut rng = PyRandom::from_int(41);
    for _ in 0..6.max((w as i64 * h as i64) / 1600) {
        let x = rng.randrange(w as i64) as i32;
        let y = rng.randrange(h as i64) as i32;
        draw::circle(bg, Color::rgba(255, 226, 140, 255), (x, y), 1, 0);
    }
}

/// Black rock with lava cracks (glowing orange) and embers.
fn paint_primordial_bg(bg: &mut Surface, c1: Color, c2: Color, w: i32, h: i32) {
    bg.fill(Color::rgba(c1.r, c1.g, c1.b, 255), None);
    let mut glow = Surface::new_alpha(w, h);
    let mut rng = PyRandom::from_int(53);
    for _ in 0..4.max((w + h) / 45) {
        let (mut x, mut y) = (rng.randrange(w as i64), rng.randrange(h as i64));
        let mut pts = vec![(x as i32, y as i32)];
        for _ in 0..rng.randint(3, 6) {
            x += rng.randint(-(w as i64) / 6, w as i64 / 6);
            y += rng.randint(-(h as i64) / 8, h as i64 / 5);
            pts.push((x as i32, y as i32));
        }
        draw::lines(&mut glow, Color::rgba(c2.r, c2.g, c2.b, 90), false, &pts, 4.max(w / 30));
        draw::lines(bg, Color::rgba(c2.r, c2.g, c2.b, 255), false, &pts, 1.max(w / 110));
        draw::lines(bg, Color::rgba(255, 220, 120, 255), false, &pts, 1);
    }
    bg.blit(&glow, 0, 0);
    for _ in 0..6.max((w as i64 * h as i64) / 1800) {
        let x = rng.randrange(w as i64) as i32;
        let y = rng.randrange(h as i64) as i32;
        let g = rng.randint(120, 200) as u8;
        let r = [1, 1, 2][rng.choice_index(3)];
        draw::circle(bg, Color::rgba(255, g, 40, 255), (x, y), r, 0);
    }
}

/// Half white / half black on the diagonal, with stars of the opposite colour on each side.
fn paint_paradox_bg(bg: &mut Surface, c1: Color, c2: Color, w: i32, h: i32) {
    bg.fill(Color::rgba(c1.r, c1.g, c1.b, 255), None);
    draw::polygon(bg, Color::rgba(c2.r, c2.g, c2.b, 255), &[(w, 0), (w, h), (0, h)], 0);
    let mut rng = PyRandom::from_int(61);
    for _ in 0..8.max((w as i64 * h as i64) / 1500) {
        let x = rng.randrange(w as i64) as i32;
        let y = rng.randrange(h as i64) as i32;
        let dark_side = x as i64 * h as i64 + y as i64 * w as i64 > w as i64 * h as i64; // below the diagonal
        let col = if dark_side { Color::rgba(c1.r, c1.g, c1.b, 255) } else { Color::rgba(c2.r, c2.g, c2.b, 255) };
        if rng.random() < 0.3 {
            draw::line(bg, col, (x - 3, y), (x + 3, y), 1);
            draw::line(bg, col, (x, y - 3), (x, y + 3), 1);
        } else {
            draw::circle(bg, col, (x, y), 1, 0);
        }
    }
    draw::line(bg, Color::rgba(150, 100, 255, 255), (w, 0), (0, h), 2.max(w / 70));
}

/// Grows, overshoots the final size a little and comes back (an elastic "pop").
pub fn ease_out_back(t: f64) -> f64 {
    let overshoot = 1.9;
    let t = t.clamp(0.0, 1.0) - 1.0;
    1.0 + (overshoot + 1.0) * t.powi(3) + overshoot * t.powi(2)
}

/// An expanding ring (shockwave) with transparency.
pub fn draw_shockwave(canvas: &mut Surface, center: (i32, i32), radius: f64, color: Color, alpha: f64, width: f64) {
    if alpha <= 0.0 || radius <= 1.0 {
        return;
    }
    let r = radius as i32;
    let mut surf = Surface::new_alpha(r * 2 + 4, r * 2 + 4);
    draw::circle(&mut surf, Color::rgba(color.r, color.g, color.b, alpha.clamp(0.0, 255.0) as u8), (r + 2, r + 2), r, 1.max(width as i32));
    canvas.blit(&surf, center.0 - r - 2, center.1 - r - 2);
}

pub fn draw_rarity_bg(surface: &mut Surface, rect_local: Rect, rarity: &Rarity, radius: i32) {
    let c1 = rarity.color;
    let Some(c2) = rarity.color2 else {
        draw::rect(surface, c1, rect_local, 0, radius);
        return;
    };
    let (w, h) = rect_local.size();
    let key = (rarity.key, w, h, radius);
    let bg = RARITY_BG.with(|c| c.borrow().get(&key).cloned());
    let bg = match bg {
        Some(b) => b,
        None => {
            let mut bg = Surface::new_alpha(w, h);
            bg.fill(Color::rgba(c1.r, c1.g, c1.b, 255), None);
            let full = Rect::new(0, 0, w, h);
            match rarity.key {
                "cosmico" => paint_cosmic_bg(&mut bg, c1, c2, w, h),
                "transcendente" => paint_transcendent_bg(&mut bg, w, h),
                "etereo" => paint_ethereal_bg(&mut bg, c1, c2, w, h),
                "celestial" => paint_celestial_bg(&mut bg, c1, c2, w, h),
                "absoluto" => paint_absolute_bg(&mut bg, c1, c2, w, h),
                "primordial" => paint_primordial_bg(&mut bg, c1, c2, w, h),
                "paradoxo" => paint_paradox_bg(&mut bg, c1, c2, w, h),
                "secreto" => {
                    let stripe_w = 8.max(w / 14);
                    let mut x = full.left() - full.h;
                    while x < full.right() {
                        draw::polygon(
                            &mut bg,
                            c2,
                            &[(x, full.bottom()), (x + stripe_w, full.bottom()), (x + stripe_w + full.h, full.top()), (x + full.h, full.top())],
                            0,
                        );
                        x += stripe_w * 2;
                    }
                }
                _ => {
                    draw::polygon(&mut bg, c2, &[(full.right(), full.top()), (full.right(), full.bottom()), (full.left(), full.bottom())], 0);
                }
            }
            bg.blit_ex(&rounded_mask(w, h, radius), 0, 0, None, BLEND_RGBA_MIN);
            let bg = Rc::new(bg);
            RARITY_BG.with(|c| c.borrow_mut().insert(key, bg.clone()));
            bg
        }
    };
    surface.blit(&bg, rect_local.x, rect_local.y);
}
