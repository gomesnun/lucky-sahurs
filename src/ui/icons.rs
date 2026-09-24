//! PNG icons (icons/ folder), loaded once and cached; a missing file falls back to the drawn icon.

use crate::gfx::{BLEND_RGB_MULT, Color, Surf, load_png_bytes, transform};
use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;

thread_local! {
    static ICONS: RefCell<HashMap<(String, i32), Option<Surf>>> = RefCell::new(HashMap::new());
    static PETS: RefCell<HashMap<(String, usize, i32, bool), Option<Surf>>> = RefCell::new(HashMap::new());
}

/// Square (size x size) surface from icons/<name>.png, or None.
pub fn load_icon(name: &str, size: i32) -> Option<Surf> {
    let key = (name.to_string(), size);
    if let Some(v) = ICONS.with(|c| c.borrow().get(&key).cloned()) {
        return v;
    }
    let surf = crate::assets::read(&format!("icons/{}.png", name))
        .and_then(|b| load_png_bytes(&b))
        .map(|img| Rc::new(transform::smoothscale(&img, size, size)));
    ICONS.with(|c| c.borrow_mut().insert(key, surf.clone()));
    surf
}

pub fn pet_slug(pet_name: &str) -> String {
    pet_name.to_lowercase().chars().filter(|c| c.is_alphanumeric()).collect()
}

/// The verity image (icons/pets/<name>.png) at size x size; silhouette = darkened.
pub fn load_pet_image(pet_name: &str, size: i32, silhouette: bool) -> Option<Surf> {
    load_pet_phase_image(pet_name, 0, size, silhouette)
}

/// The verity image in a phase (0 = as rolled, icons/pets/<name>.png; 1..3 = icons/pets/phases/<name>_p2..p4.png,
/// p4 being the Monster). A missing phase image falls back to the normal one.
pub fn load_pet_phase_image(pet_name: &str, phase: usize, size: i32, silhouette: bool) -> Option<Surf> {
    let key = (pet_name.to_string(), phase, size, silhouette);
    if let Some(v) = PETS.with(|c| c.borrow().get(&key).cloned()) {
        return v;
    }
    let slug = pet_slug(pet_name);
    let bytes = if phase > 0 { crate::assets::read(&format!("icons/pets/phases/{}_p{}.png", slug, phase + 1)) } else { None };
    let surf = bytes.or_else(|| crate::assets::read(&format!("icons/pets/{}.png", slug))).and_then(|b| load_png_bytes(&b)).map(|img| {
        let mut s = transform::smoothscale(&img, size, size);
        if silhouette {
            s.fill_blend(Color::rgba(24, 26, 44, 255), BLEND_RGB_MULT);
        }
        Rc::new(s)
    });
    PETS.with(|c| {
        let mut c = c.borrow_mut();
        if c.len() > 400 {
            c.clear();
        }
        c.insert(key, surf.clone());
    });
    surf
}
