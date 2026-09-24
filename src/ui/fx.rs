//! v3.0 effects: the subtle glare that sweeps across buttons and cards, the ripple where you click, and the
//! soft glow around a hovered button. Everything is cached by size, so it stays cheap even with long lists.

use crate::gfx::{BLEND_RGBA_MIN, Color, Rect, Surf, Surface, draw};
use crate::ui::drawing::rounded_mask;
use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;

/// the glare's position is rounded to this many steps (so the surfaces can be cached)
const GLARE_STEPS: i32 = 24;

thread_local! {
    static GLARE: RefCell<HashMap<(i32, i32, i32, i32, u8), Surf>> = RefCell::new(HashMap::new());
    static GLOW: RefCell<HashMap<(i32, i32, i32, u32), Surf>> = RefCell::new(HashMap::new());
}

fn cached<K: std::hash::Hash + Eq>(map: &'static std::thread::LocalKey<RefCell<HashMap<K, Surf>>>, key: K, build: impl FnOnce() -> Surface) -> Surf {
    if let Some(s) = map.with(|c| c.borrow().get(&key).cloned()) {
        return s;
    }
    let s = Rc::new(build());
    map.with(|c| {
        let mut c = c.borrow_mut();
        if c.len() > 600 {
            c.clear();
        }
        c.insert(key, s.clone());
    });
    s
}

/// A soft diagonal band of light at `phase` (0 = just left of the shape, 1 = just past its right edge),
/// clipped to the rounded rectangle.
pub fn glare_band(w: i32, h: i32, radius: i32, phase: f64, alpha: u8) -> Option<Surf> {
    if w < 8 || h < 8 || !(0.0..=1.0).contains(&phase) {
        return None;
    }
    let step = (phase * GLARE_STEPS as f64).round() as i32;
    Some(cached(&GLARE, (w, h, radius, step, alpha), || {
        let mut s = Surface::new_alpha(w, h);
        let band = (w.min(h * 2) / 3).max(10);
        let travel = w + h + band * 2;
        let x = -h - band + (travel as f64 * step as f64 / GLARE_STEPS as f64) as i32;
        // a wide faint band with a thin brighter core
        for (width, a) in [(band, alpha as i32 / 2), (band / 3, alpha as i32)] {
            let off = (band - width) / 2;
            let x0 = x + off;
            draw::polygon(&mut s, Color::rgba(255, 255, 255, a.clamp(0, 255) as u8), &[(x0, h), (x0 + width, h), (x0 + width + h, 0), (x0 + h, 0)], 0);
        }
        s.blit_ex(&rounded_mask(w, h, radius), 0, 0, None, BLEND_RGBA_MIN);
        s
    }))
}

/// Where the periodic glare of a shape is right now: Some(phase) while it sweeps, None between sweeps.
/// Shapes at different places sweep at different moments, so a list doesn't flash all at once.
pub fn glare_phase(t: f64, rect: Rect, period: f64, sweep: f64) -> Option<f64> {
    let offset = (rect.x as f64 * 0.0023 + rect.y as f64 * 0.0041).fract() * period;
    let k = (t + offset).rem_euclid(period);
    (k < sweep).then(|| k / sweep)
}

/// A soft glow around a rectangle (hovered buttons).
pub fn hover_glow(w: i32, h: i32, radius: i32, color: Color) -> Surf {
    cached(&GLOW, (w, h, radius, Color::rgb(color.r, color.g, color.b).argb()), || {
        let spread = 7;
        let mut g = Surface::new_alpha(w + 2 * spread, h + 2 * spread);
        let mut i = spread;
        while i > 0 {
            let a = (70.0 * ((spread - i + 1) as f64 / spread as f64).powi(2)) as u8;
            draw::rect(&mut g, Color::rgba(color.r, color.g, color.b, a), Rect::new(spread - i, spread - i, w + 2 * i, h + 2 * i), 0, radius + i);
            i -= 1;
        }
        g
    })
}

/// The ripple of a click: a circle growing from the click point and fading, clipped to the button.
/// `t` goes from 0 to 1.
pub fn ripple(w: i32, h: i32, radius: i32, at: (i32, i32), t: f64) -> Surface {
    let mut s = Surface::new_alpha(w, h);
    let max_r = ((w * w + h * h) as f64).sqrt();
    let r = (max_r * (0.15 + 0.85 * t)) as i32;
    let a = (95.0 * (1.0 - t)).clamp(0.0, 255.0) as u8;
    draw::circle(&mut s, Color::rgba(255, 255, 255, a), at, r.max(1), 0);
    s.blit_ex(&rounded_mask(w, h, radius), 0, 0, None, BLEND_RGBA_MIN);
    s
}
