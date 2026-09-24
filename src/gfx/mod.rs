//! Software rendering layer that mirrors pygame-ce 2.5.8.

pub mod draw;
pub mod glow;
pub mod r3d;
pub mod rect;
pub mod surface;
pub mod text;
pub mod transform;

pub use rect::{Rect, ti};
pub use surface::{BLEND_RGB_MULT, BLEND_RGBA_MIN, BLEND_RGBA_MULT, Color, Surf, Surface};

/// pygame.image.load(path).convert_alpha()
pub fn load_png(path: &std::path::Path) -> Option<Surface> {
    load_png_bytes(&std::fs::read(path).ok()?)
}

/// load_png() for a file already in memory.
pub fn load_png_bytes(data: &[u8]) -> Option<Surface> {
    let img = image::load_from_memory_with_format(data, image::ImageFormat::Png).ok()?.to_rgba8();
    let (w, h) = img.dimensions();
    let mut s = Surface::new_alpha(w as i32, h as i32);
    for (i, p) in img.pixels().enumerate() {
        let [r, g, b, a] = p.0;
        s.px[i] = Color::rgba(r, g, b, a).argb();
    }
    Some(s)
}
