//! Verity and trait cards (ui/cards.py): the verity image, the big outlined name, the rarity in a pill
//! and the income / chance in dark plates.

use crate::core::data::{PHASES, Rarity, TRAITS, mutation};
use crate::gfx::{BLEND_RGBA_MIN, Color, Rect, Surf, Surface, draw};
use crate::i18n::tr;
use crate::theme::{BORDER_W_SMALL, GOLD_BORDER, WHITE, outline, panel_light};
use crate::ui::drawing::{draw_rarity_bg, mix, rounded_gradient, shade};
use crate::ui::fonts::{Font, fit_text, font_at, wrap_text};
use crate::ui::icons::{load_pet_image, load_pet_phase_image};
use std::cell::RefCell;
use std::collections::HashMap;
use std::hash::Hash;
use std::rc::Rc;

pub fn rarity_glow_color(key: &str) -> Color {
    let (r, g, b) = match key {
        "comum" => (232, 234, 245),
        "incomum" => (96, 232, 126),
        "raro" => (92, 168, 255),
        "epico" => (196, 118, 255),
        "lendario" => (255, 216, 64),
        "mitico" => (255, 88, 88),
        "exotico" => (255, 168, 56),
        "secreto" => (245, 245, 250),
        "divino" => (255, 156, 210),
        "cosmico" => (160, 130, 255),
        "transcendente" => (130, 240, 255),
        "etereo" => (150, 225, 255),
        "celestial" => (255, 214, 110),
        "absoluto" => (255, 190, 50),
        "primordial" => (255, 110, 40),
        "paradoxo" => (190, 150, 255),
        _ => (255, 255, 255),
    };
    Color::rgb(r, g, b)
}

pub const PLATE_FILL: Color = Color::rgba(10, 11, 20, 196);
pub const PLATE_LABEL: Color = Color::rgb(196, 202, 226);
pub const PLATE_SUB: Color = Color::rgb(150, 156, 184);
pub const LOCKED_MARK: Color = Color::rgb(150, 156, 184);

pub const BALL_FRAC: f64 = 0.56;
pub const BALL_CY: f64 = 0.542;

/// Small LRU cache (evicts the least recently used entry, one at a time).
pub struct Lru<K, V> {
    map: HashMap<K, (V, u64)>,
    tick: u64,
    limit: usize,
}

impl<K: Hash + Eq + Clone, V: Clone> Lru<K, V> {
    pub fn new(limit: usize) -> Self {
        Lru { map: HashMap::new(), tick: 0, limit }
    }
    pub fn get(&mut self, k: &K) -> Option<V> {
        self.tick += 1;
        let t = self.tick;
        self.map.get_mut(k).map(|e| {
            e.1 = t;
            e.0.clone()
        })
    }
    pub fn put(&mut self, k: K, v: V) {
        self.tick += 1;
        self.map.insert(k, (v, self.tick));
        while self.map.len() > self.limit {
            let oldest = self.map.iter().min_by_key(|e| e.1.1).map(|e| e.0.clone());
            match oldest {
                Some(o) => {
                    self.map.remove(&o);
                }
                None => break,
            }
        }
    }
    pub fn clear(&mut self) {
        self.map.clear();
    }
}

thread_local! {
    static CARDS: RefCell<Lru<String, Surf>> = RefCell::new(Lru::new(320));
    static OVERLAY: RefCell<Lru<(i32, i32, i32, i32), Surf>> = RefCell::new(Lru::new(320));
}

pub fn clear_card_cache() {
    CARDS.with(|c| c.borrow_mut().clear());
    OVERLAY.with(|c| c.borrow_mut().clear());
}

fn shade_overlay(w: i32, h: i32, radius: i32) -> Surf {
    let key = (w, h, radius, -1);
    if let Some(s) = OVERLAY.with(|c| c.borrow_mut().get(&key)) {
        return s;
    }
    let mut s = Surface::new_alpha(w, h);
    for y in 0..h {
        let t = y as f64 / (h - 1).max(1) as f64;
        if t < 0.34 {
            let a = (46.0 * (1.0 - t / 0.34)) as i32;
            draw::line(&mut s, Color::rgba(255, 255, 255, a as u8), (0, y), (w, y), 1);
        } else if t > 0.42 {
            let a = (118.0 * ((t - 0.42) / 0.58).powf(1.3)) as i32;
            draw::line(&mut s, Color::rgba(0, 0, 0, a as u8), (0, y), (w, y), 1);
        }
    }
    let mut mask = Surface::new_alpha(w, h);
    let mr = mask.get_rect();
    draw::rect(&mut mask, Color::rgba(255, 255, 255, 255), mr, 0, radius);
    s.blit_ex(&mask, 0, 0, None, BLEND_RGBA_MIN);
    let s = Rc::new(s);
    OVERLAY.with(|c| c.borrow_mut().put(key, s.clone()));
    s
}

fn round_mask(w: i32, h: i32, radius: i32, inset: i32) -> Surf {
    // keyed apart from the overlays by a negative first field
    let key = (-w - 1, h, radius, inset);
    if let Some(s) = OVERLAY.with(|c| c.borrow_mut().get(&key)) {
        return s;
    }
    let mut s = Surface::new_alpha(w, h);
    draw::rect(&mut s, Color::rgba(255, 255, 255, 255), Rect::new(inset, inset, w - 2 * inset, h - 2 * inset), 0, 2.max(radius - inset));
    let s = Rc::new(s);
    OVERLAY.with(|c| c.borrow_mut().put(key, s.clone()));
    s
}

/// Dark pill around an already-rendered text.
pub fn pill(text: &Surface, edge: Option<Color>, pad_x: i32, pad_y: i32) -> Surface {
    let (w, h) = (text.w + 2 * pad_x, text.h + 2 * pad_y);
    let mut s = Surface::new_alpha(w, h);
    let r = s.get_rect();
    draw::rect(&mut s, Color::rgba(6, 7, 16, 170), r, 0, h / 2);
    if let Some(e) = edge {
        draw::rect(&mut s, Color::rgba(e.r, e.g, e.b, 190), r, 2, h / 2);
    }
    let tr_ = Rect::with_center(text.w, text.h, (w / 2, h / 2));
    s.blit(text, tr_.x, tr_.y);
    s
}

/// One plate cell: (label, value, optional small line).
pub type Cell = (String, String, Option<String>);

pub fn cell(label: impl Into<String>, value: impl Into<String>) -> Cell {
    (label.into(), value.into(), None)
}
pub fn cell3(label: impl Into<String>, value: impl Into<String>, sub: impl Into<String>) -> Cell {
    (label.into(), value.into(), Some(sub.into()))
}

fn has_sub(c: &Cell) -> bool {
    c.2.as_ref().map(|s| !s.is_empty()).unwrap_or(false)
}

fn card_scale(w: i32) -> f64 {
    (w as f64 / 236.0).clamp(0.55, 1.3)
}

fn rnd(x: f64) -> i32 {
    crate::core::formatting::py_round(x) as i32
}

pub struct PlateFonts {
    pub label: Font,
    pub value: Font,
    pub sub: Font,
}

pub fn plate_fonts(s: f64) -> PlateFonts {
    PlateFonts {
        label: font_at(11.max(rnd(12.0 * s)), false, Some(0)),
        value: font_at(13.max(rnd(19.0 * s)), true, None),
        sub: font_at(10.max(rnd(12.0 * s)), false, Some(0)),
    }
}

fn plate_pad(s: f64) -> i32 {
    3.max(rnd(4.0 * s))
}

pub fn plate_height(cells: &[Cell], f: &PlateFonts, s: f64) -> i32 {
    let mut h = f.label.get_height() + f.value.get_height() - 2;
    if cells.iter().any(has_sub) {
        h += f.sub.get_height() - 2;
    }
    h + 2 * plate_pad(s)
}

fn fit_value(text: &str, size: i32, max_w: i32) -> Surf {
    let mut size = size;
    while size > 11 {
        let f = font_at(size, true, None);
        if f.size(text).0 + 2 * f.outline <= max_w {
            return f.render(text, WHITE);
        }
        size -= 1;
    }
    let f = font_at(11, true, None);
    f.render(&fit_text(&f, text, max_w - 2 * f.outline), WHITE)
}

pub fn draw_plate(surf: &mut Surface, rect: Rect, cells: &[Cell], f: &PlateFonts, s: f64) {
    draw::rect(surf, PLATE_FILL, rect, 0, 6.max(rnd(9.0 * s)));
    let cell_w = rect.w / cells.len() as i32;
    let value_size = 13.max(rnd(19.0 * s));
    for (i, c) in cells.iter().enumerate() {
        let i = i as i32;
        let cx = rect.x + i * cell_w + cell_w / 2;
        let mut y = rect.y + plate_pad(s);
        let lab = f.label.render(&fit_text(&f.label, &c.0, cell_w - 8), PLATE_LABEL);
        let r = Rect::with_midtop(lab.w, lab.h, (cx, y));
        surf.blit(&lab, r.x, r.y);
        y += f.label.get_height() - 2;
        let val = fit_value(&c.1, value_size, cell_w - 8);
        let r = Rect::with_midtop(val.w, val.h, (cx, y - 1));
        surf.blit(&val, r.x, r.y);
        if has_sub(c) {
            y += f.value.get_height() - 2;
            let st = f.sub.render(&fit_text(&f.sub, c.2.as_deref().unwrap(), cell_w - 8), PLATE_SUB);
            let r = Rect::with_midtop(st.w, st.h, (cx, y));
            surf.blit(&st, r.x, r.y);
        }
        if i > 0 {
            draw::line(surf, Color::rgba(255, 255, 255, 34), (rect.x + i * cell_w, rect.y + 6), (rect.x + i * cell_w, rect.bottom() - 7), 1);
        }
    }
}

/// Biggest font (from base_size down) so the name fits max_w x max_h. Returns (lines, pitch, total).
fn fit_name(name: &str, base_size: i32, max_w: i32, max_h: i32, extra_h: i32) -> (Vec<Surf>, i32, i32) {
    let mut size = base_size;
    loop {
        let font = font_at(size, true, None);
        let lines = wrap_text(name, &font, max_w);
        let pitch = font.get_height() - 2.max(size.div_euclid(8));
        let widest = lines.iter().map(|t| font.size(t).0 + 2 * font.outline).max().unwrap_or(0);
        let total = lines.len() as i32 * pitch + 2 * font.outline + extra_h;
        if (widest <= max_w && total <= max_h) || size <= 12 {
            return (lines.iter().map(|t| font.render(t, WHITE)).collect(), pitch, total);
        }
        size -= 1;
    }
}

#[allow(clippy::too_many_arguments)]
fn draw_pet_art(surf: &mut Surface, rarity: &Rarity, phase: usize, locked: bool, area_top: i32, area_bottom: i32, inner_w: i32, s: f64, radius: i32, inset: i32, pill_s: &Surface) {
    let (w, h) = surf.get_size();
    let (lines, pitch, total) = if locked {
        let font = font_at(16.max(rnd(30.0 * s)), true, None);
        let p = font.get_height() - 4;
        (vec![font.render("?", LOCKED_MARK)], p, p)
    } else {
        fit_name(rarity.pet, 13.max(rnd(28.0 * s)), inner_w - 4, ((area_bottom - area_top) as f64 * 0.45) as i32, 0)
    };
    let ball_top = area_top + (pill_s.h as f64 * 0.7) as i32;
    let ball_bottom = area_bottom - (total as f64 * 0.5) as i32;
    let diameter = ball_bottom - ball_top;
    if diameter >= 18 {
        let side = ((diameter as f64 / BALL_FRAC) as i32).min((w as f64 * 1.15) as i32);
        if let Some(art) = load_pet_phase_image(rarity.pet, phase, side, locked) {
            let mut layer = Surface::new_alpha(w, h);
            let cy = (ball_top + ball_bottom).div_euclid(2);
            layer.blit(&art, w / 2 - side.div_euclid(2), (cy as f64 - BALL_CY * side as f64) as i32);
            layer.blit_ex(&round_mask(w, h, radius, inset), 0, 0, None, BLEND_RGBA_MIN);
            surf.blit(&layer, 0, 0);
        }
    }
    let r = Rect::with_midtop(pill_s.w, pill_s.h, (w / 2, area_top));
    surf.blit(pill_s, r.x, r.y);
    let mut y = area_bottom - total;
    if phase > 0 && !locked {
        // the phase just above the name ("Phase 2", "Phase 3", "Monster"): clear of the face and the monster's head
        let ph = &PHASES[phase.min(PHASES.len() - 1)];
        let size = 9.max(rnd(13.0 * s));
        let p = pill(&font_at(size, true, None).render(&tr(ph.name).to_uppercase(), ph.color), Some(ph.color), 8, 2);
        let pr = Rect::with_midbottom(p.w, p.h, (w / 2, y + 2));
        surf.blit(&p, pr.x, pr.y);
    }
    for t in &lines {
        let r = Rect::with_midtop(t.w, t.h, (w / 2, y));
        surf.blit(t, r.x, r.y);
        y += pitch;
    }
}

/// The verity card (cached; the result must not be modified).
pub fn render_pet_card(rarity: &Rarity, mutation_key: &str, w: i32, h: i32, plates: &[Vec<Cell>], footer_h: i32, locked: bool) -> Surf {
    render_pet_card_phase(rarity, mutation_key, 0, w, h, plates, footer_h, locked)
}

/// The verity card in a phase (0 = as rolled ... 3 = Monster): the phase's art and a badge under the rarity.
#[allow(clippy::too_many_arguments)]
pub fn render_pet_card_phase(rarity: &Rarity, mutation_key: &str, phase: usize, w: i32, h: i32, plates: &[Vec<Cell>], footer_h: i32, locked: bool) -> Surf {
    let key = format!("{}|{}|{}|{}|{}|{}|{:?}|{}|{}", rarity.pet, mutation_key, phase, tr(rarity.name), w, h, plates, footer_h, locked);
    if let Some(s) = CARDS.with(|c| c.borrow_mut().get(&key)) {
        return s;
    }
    let s = card_scale(w);
    let radius = 9.max(rnd(16.0 * s));
    let pad = 6.max(rnd(10.0 * s));
    let gap = 3.max(rnd(5.0 * s));
    let mut surf = Surface::new_alpha(w, h);
    let rect = Rect::new(0, 0, w, h);

    if locked {
        let dark = mix(rarity.color, Color::rgb(16, 18, 30), 0.80);
        let g = rounded_gradient(w, h, shade(dark, 1.25), shade(dark, 0.80), radius);
        surf.blit(&g, 0, 0);
    } else {
        draw_rarity_bg(&mut surf, rect, rarity, radius);
        let o = shade_overlay(w, h, radius);
        surf.blit(&o, 0, 0);
    }

    let border = mutation(mutation_key).and_then(|m| m.border);
    match border {
        Some(_) if mutation_key == "rainbow" => {
            draw::rect(&mut surf, outline(), rect, 2, radius);
            crate::ui::drawing::draw_rainbow_border(&mut surf, rect.inflate(-4, -4), 4.max(radius - 2), 3.max(rnd(4.0 * s)));
        }
        Some(b) => {
            draw::rect(&mut surf, outline(), rect, 2, radius);
            draw::rect(&mut surf, b, rect.inflate(-4, -4), 3.max(rnd(4.0 * s)), 4.max(radius - 2));
        }
        None => draw::rect(&mut surf, outline(), rect, BORDER_W_SMALL, radius),
    }

    let fonts = plate_fonts(s);
    let heights: Vec<i32> = plates.iter().map(|p| plate_height(p, &fonts, s)).collect();
    let plates_h: i32 = heights.iter().sum::<i32>() + gap * 0.max(plates.len() as i32 - 1);
    let bottom = h - pad - footer_h - if footer_h != 0 { gap } else { 0 };
    let plates_top = bottom - plates_h;
    let inner_w = w - 2 * pad;
    let (area_top, area_bottom) = (pad, plates_top - gap);

    let accent = rarity_glow_color(rarity.key);
    let pill_size = 11.max(rnd(16.0 * s));
    let pill_s = pill(&font_at(pill_size, true, None).render(&tr(rarity.name), accent), Some(accent), 10, 2);
    if load_pet_image(rarity.pet, 16, false).is_some() {
        draw_pet_art(&mut surf, rarity, phase, locked, area_top, area_bottom, inner_w, s, radius, if border.is_some() { 6 } else { 3 }, &pill_s);
    } else if locked {
        let q = font_at(20.max(rnd(58.0 * s)), true, None).render("?", LOCKED_MARK);
        let block_h = q.h - 8 + gap + pill_s.h;
        let y = area_top + 0.max((area_bottom - area_top - block_h).div_euclid(2));
        let r = Rect::with_midtop(q.w, q.h, (w / 2, y - 6));
        surf.blit(&q, r.x, r.y);
        let r = Rect::with_midtop(pill_s.w, pill_s.h, (w / 2, y + q.h - 8 + gap));
        surf.blit(&pill_s, r.x, r.y);
    } else {
        let base_size = 13.max(rnd(33.0 * s));
        let (lines, pitch, total) = fit_name(rarity.pet, base_size, inner_w - 4, area_bottom - area_top, gap + 2 + pill_s.h);
        let mut y = area_top + 0.max((area_bottom - area_top - total).div_euclid(2));
        for t in &lines {
            let r = Rect::with_midtop(t.w, t.h, (w / 2, y));
            surf.blit(t, r.x, r.y);
            y += pitch;
        }
        let r = Rect::with_midtop(pill_s.w, pill_s.h, (w / 2, y + gap + 2));
        surf.blit(&pill_s, r.x, r.y);
    }

    let mut y = plates_top;
    for (cells, ph) in plates.iter().zip(heights.iter()) {
        draw_plate(&mut surf, Rect::new(pad, y, inner_w, *ph), cells, &fonts, s);
        y += ph + gap;
    }
    let surf = Rc::new(surf);
    CARDS.with(|c| c.borrow_mut().put(key, surf.clone()));
    surf
}

/// A trait card (same style as the pets: outlined name + plate with the chance).
pub fn render_trait_card(trait_index: usize, w: i32, h: i32, owned: bool, equipped: bool, chance_text: &str) -> Surf {
    let key = format!("trait|{}|{}|{}|{}|{}|{}|{}|{}", trait_index, w, h, owned, equipped, chance_text, tr("Chance"), tr("EQUIPPED"));
    if let Some(s) = CARDS.with(|c| c.borrow_mut().get(&key)) {
        return s;
    }
    let tdef = &TRAITS[trait_index];
    let s = ((w as f64 / 250.0).min(h as f64 / 150.0) * 1.05).clamp(0.6, 1.2);
    let radius = 9.max(rnd(14.0 * s));
    let pad = 6.max(rnd(9.0 * s));
    let mut surf = Surface::new_alpha(w, h);
    let rect = Rect::new(0, 0, w, h);
    let fonts = plate_fonts(s);
    let cells = vec![cell(tr("Chance"), chance_text)];
    let plate_h = plate_height(&cells, &fonts, s);
    let plate_rect = Rect::new(pad, h - pad - plate_h, w - 2 * pad, plate_h);
    let area_h = plate_rect.top() - 2 * pad;

    if !owned {
        let g = rounded_gradient(w, h, shade(panel_light(), 1.10), shade(panel_light(), 0.84), radius);
        surf.blit(&g, 0, 0);
        draw::rect(&mut surf, outline(), rect, BORDER_W_SMALL, radius);
        let q = font_at(24.max(rnd(52.0 * s)), true, None).render("?", LOCKED_MARK);
        let r = Rect::with_center(q.w, q.h, (w / 2, pad + area_h.div_euclid(2) - 2));
        surf.blit(&q, r.x, r.y);
        draw_plate(&mut surf, plate_rect, &cells, &fonts, s);
        let surf = Rc::new(surf);
        CARDS.with(|c| c.borrow_mut().put(key, surf.clone()));
        return surf;
    }

    let base = tdef.color;
    let g = rounded_gradient(w, h, shade(base, 1.12), shade(base, 0.84), radius);
    surf.blit(&g, 0, 0);
    let o = shade_overlay(w, h, radius);
    surf.blit(&o, 0, 0);
    if equipped {
        draw::rect(&mut surf, outline(), rect, 2, radius);
        draw::rect(&mut surf, GOLD_BORDER, rect.inflate(-4, -4), 4, 4.max(radius - 2));
    } else {
        draw::rect(&mut surf, outline(), rect, BORDER_W_SMALL, radius);
    }

    let (lines, pitch, total) = fit_name(&tr(tdef.name), 14.max(rnd(32.0 * s)), w - 2 * pad - 4, area_h, 0);
    let mut y = pad + 0.max((area_h - total).div_euclid(2));
    for t in &lines {
        let r = Rect::with_midtop(t.w, t.h, (w / 2, y));
        surf.blit(t, r.x, r.y);
        y += pitch;
    }

    if equipped {
        let p = pill(&font_at(11.max(rnd(13.0 * s)), true, None).render(&tr("EQUIPPED"), GOLD_BORDER), Some(GOLD_BORDER), 10, 2);
        let corner_pad = 6.max(rnd(8.0 * s));
        let r = Rect::with_topright(p.w, p.h, (w - corner_pad, corner_pad));
        surf.blit(&p, r.x, r.y);
    }
    draw_plate(&mut surf, plate_rect, &cells, &fonts, s);
    let surf = Rc::new(surf);
    CARDS.with(|c| c.borrow_mut().put(key, surf.clone()));
    surf
}
