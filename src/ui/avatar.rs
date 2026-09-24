//! Profile picture: a verity image inside a circle, with a ring in the mutation's colour (ui/avatar.py).

use crate::core::data::{mutation, rarities};
use crate::gfx::text::RawFont;
use crate::gfx::{BLEND_RGBA_MULT, Color, Rect, Surf, Surface, draw};
use crate::theme::{grey_dim, outline, panel_light, panel_lighter};
use crate::ui::cards::{BALL_CY, BALL_FRAC, Lru};
use crate::ui::icons::load_pet_image;
use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;

pub fn avatar_ring_color(mutation_key: &str) -> Color {
    mutation(mutation_key).and_then(|m| m.border).unwrap_or_else(panel_lighter)
}

thread_local! {
    static CACHE: RefCell<Lru<(i64, String, i32, u32, u32, u32), Surf>> = RefCell::new(Lru::new(80));
    static LETTER: RefCell<HashMap<i32, Rc<RawFont>>> = RefCell::new(HashMap::new());
}

/// pygame.font.SysFont("arial", size, bold=True) -> fontconfig picks FreeSans Bold here.
fn letter_font(size: i32) -> Option<Rc<RawFont>> {
    let size = 10.max((size as f64 * 0.5) as i32);
    if let Some(f) = LETTER.with(|c| c.borrow().get(&size).cloned()) {
        return Some(f);
    }
    let windows_arial = std::env::var("WINDIR").unwrap_or_else(|_| "C:\\Windows".into()) + "\\Fonts\\arialbd.ttf";
    let candidates = [
        windows_arial.as_str(),
        "/System/Library/Fonts/Supplemental/Arial Bold.ttf",
        "/Library/Fonts/Arial Bold.ttf",
        "/usr/share/fonts/gnu-free/FreeSansBold.otf",
        "/usr/share/fonts/truetype/freefont/FreeSansBold.ttf",
        "/usr/share/fonts/TTF/DejaVuSans-Bold.ttf",
        "/usr/share/fonts/truetype/dejavu/DejaVuSans-Bold.ttf",
    ];
    // SysFont falls back to pygame's default font when "arial" is not installed
    let f = candidates
        .iter()
        .find_map(|p| RawFont::open(p, size))
        .or_else(|| RawFont::open_static(crate::ui::fonts::PYGAME_DEFAULT_FONT, size))
        .map(Rc::new)?;
    LETTER.with(|c| c.borrow_mut().insert(size, f.clone()));
    Some(f)
}

/// Square (size x size) surface with the round photo and the ring. pet_index None / invalid gives the
/// empty photo (circle with a question mark). Cached: do not modify it.
pub fn avatar_surface(pet_index: Option<i64>, mutation_key: &str, size: i32) -> Surf {
    let ring = avatar_ring_color(mutation_key);
    let key = (pet_index.unwrap_or(-1_000_000), mutation_key.to_string(), size, ring.argb(), panel_light().argb(), outline().argb());
    if let Some(s) = CACHE.with(|c| c.borrow_mut().get(&key)) {
        return s;
    }
    let size = 16.max(size);
    let border = 3.max(size / 14);
    let center = (size / 2, size / 2);
    let radius = size / 2 - 1;
    let mut surf = Surface::new_alpha(size, size);

    let valid = matches!(pet_index, Some(i) if i >= 0 && (i as usize) < rarities().len());
    let inner = size - 2 * border;
    let mut img = None;
    let mut img_w = 0;
    if valid {
        img_w = 8.max(crate::core::formatting::py_round(inner as f64 / BALL_FRAC) as i32);
        img = load_pet_image(rarities()[pet_index.unwrap() as usize].pet, img_w, false);
    }
    if let Some(img) = img {
        surf.blit(&img, center.0 - img_w / 2, center.1 - (img_w as f64 * BALL_CY) as i32);
        let mut mask = Surface::new_alpha(size, size);
        draw::circle(&mut mask, Color::rgba(255, 255, 255, 255), center, radius - border + 1, 0);
        surf.blit_ex(&mask, 0, 0, None, BLEND_RGBA_MULT);
    } else {
        let fill = if valid { rarities()[pet_index.unwrap() as usize].color } else { panel_light() };
        draw::circle(&mut surf, fill, center, radius - border + 1, 0);
        if !valid {
            if let Some(font) = letter_font(size) {
                let t = font.render("?", grey_dim());
                let r = Rect::with_center(t.w, t.h, center);
                surf.blit(&t, r.x, r.y);
            }
        }
    }
    draw::circle(&mut surf, ring, center, radius, border);
    draw::circle(&mut surf, outline(), center, radius, 1);
    draw::circle(&mut surf, outline(), center, 1.max(radius - border), 1);
    let s = Rc::new(surf);
    CACHE.with(|c| c.borrow_mut().put(key, s.clone()));
    s
}
