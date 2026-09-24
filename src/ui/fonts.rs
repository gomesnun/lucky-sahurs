//! Fonts and text: outlined font, line wrapping, text with a shadow (ui/fonts.py).

use crate::config::{asset_roots, game_dir};
use crate::gfx::text::RawFont;
use crate::gfx::{Color, Rect, Surf, Surface};
use crate::theme::{BLACK, WHITE, outline};
use std::cell::{Cell, RefCell};
use std::collections::HashMap;
use std::rc::Rc;

pub fn wrap_text(text: &str, font: &StyledFont, max_width: i32) -> Vec<String> {
    let mut lines = Vec::new();
    let mut cur = String::new();
    for w in text.split(' ') {
        let test = format!("{} {}", cur, w).trim().to_string();
        if font.size(&test).0 <= max_width || cur.is_empty() {
            cur = test;
        } else {
            lines.push(std::mem::take(&mut cur));
            cur = w.to_string();
        }
    }
    if !cur.is_empty() {
        lines.push(cur);
    }
    lines
}

/// Shortens the text with '...' if it does not fit in max_width.
pub fn fit_text(font: &StyledFont, text: &str, max_width: i32) -> String {
    if font.size(text).0 <= max_width {
        return text.to_string();
    }
    let mut t: Vec<char> = text.chars().collect();
    loop {
        if t.is_empty() {
            break;
        }
        let s: String = t.iter().collect();
        if font.size(&(s + "...")).0 > max_width {
            t.pop();
        } else {
            break;
        }
    }
    let s: String = t.iter().collect();
    s.trim_end().to_string() + "..."
}

pub fn outline_color_for(c: Color) -> Color {
    let lum = 0.299 * c.r as f64 + 0.587 * c.g as f64 + 0.114 * c.b as f64;
    if lum < 128.0 { WHITE } else { BLACK }
}

pub fn is_light(c: Color) -> bool {
    0.299 * c.r as f64 + 0.587 * c.g as f64 + 0.114 * c.b as f64 >= 110.0
}

#[derive(Clone)]
enum FontSrc {
    Path(String),
    Embedded(&'static [u8]),
}

impl FontSrc {
    fn open(&self, size: i32) -> Option<RawFont> {
        match self {
            FontSrc::Path(p) => RawFont::open(p, size),
            FontSrc::Embedded(b) => RawFont::open_static(b, size),
        }
    }
}

/// pygame.font.Font(None, size): pygame's own freesansbold.ttf.
pub const PYGAME_DEFAULT_FONT: &[u8] = include_bytes!("../../vendor/pygame/freesansbold.ttf");

/// First .ttf/.otf in a fonts/ folder next to the game, else the one built into the executable.
fn find_custom_font() -> Option<FontSrc> {
    thread_local! { static F: RefCell<Option<Option<FontSrc>>> = const { RefCell::new(None) }; }
    F.with(|f| {
        if let Some(v) = &*f.borrow() {
            return v.clone();
        }
        let mut folders = vec![game_dir().join("fonts")];
        folders.extend(asset_roots().iter().map(|r| r.join("fonts")));
        let is_font = |n: &str| {
            let l = n.to_lowercase();
            l.ends_with(".ttf") || l.ends_with(".otf")
        };
        let mut found = None;
        for folder in folders {
            if let Ok(rd) = std::fs::read_dir(&folder) {
                let mut names: Vec<String> = rd.flatten().map(|e| e.file_name().to_string_lossy().to_string()).collect();
                names.sort();
                if let Some(n) = names.iter().find(|n| is_font(n)) {
                    found = Some(FontSrc::Path(folder.join(n).to_string_lossy().to_string()));
                    break;
                }
            }
        }
        if found.is_none() {
            found = crate::assets::embedded_names("fonts")
                .into_iter()
                .find(|n| is_font(n))
                .and_then(|n| crate::assets::embedded(&format!("fonts/{n}")))
                .map(FontSrc::Embedded);
        }
        *f.borrow_mut() = Some(found.clone());
        found
    })
}

/// A font whose light text gets a thick dark outline (cartoon style). Dark text has none.
pub struct StyledFont {
    pub raw: RawFont,
    pub outline: i32,
    offsets: Vec<(i32, i32)>,
    id: usize,
}

pub type Font = Rc<StyledFont>;

struct RenderCache {
    map: HashMap<(usize, String, u32), (Surf, u64)>,
    tick: u64,
}

const CACHE_MAX: usize = 1500;

thread_local! {
    static RCACHE: RefCell<RenderCache> = RefCell::new(RenderCache { map: HashMap::new(), tick: 0 });
    static NEXT_ID: Cell<usize> = const { Cell::new(1) };
    static FONT_AT: RefCell<HashMap<(i32, bool, i32), Font>> = RefCell::new(HashMap::new());
}

impl StyledFont {
    fn new(raw: RawFont, outline: i32) -> StyledFont {
        let t = outline;
        let mut offsets = Vec::new();
        for dx in -t..=t {
            for dy in -t..=t {
                if (dx != 0 || dy != 0) && dx * dx + dy * dy <= t * t + 1 {
                    offsets.push((dx, dy));
                }
            }
        }
        let id = NEXT_ID.with(|n| {
            let v = n.get();
            n.set(v + 1);
            v
        });
        StyledFont { raw, outline, offsets, id }
    }

    pub fn size(&self, text: &str) -> (i32, i32) {
        self.raw.size(text)
    }
    pub fn get_height(&self) -> i32 {
        self.raw.get_height()
    }
    pub fn get_linesize(&self) -> i32 {
        self.raw.get_linesize()
    }

    pub fn render(&self, text: &str, color: Color) -> Surf {
        let color = Color::rgb(color.r, color.g, color.b);
        let key = (self.id, text.to_string(), color.argb());
        let hit = RCACHE.with(|c| {
            let mut c = c.borrow_mut();
            c.tick += 1;
            let t = c.tick;
            c.map.get_mut(&key).map(|e| {
                e.1 = t;
                e.0.clone()
            })
        });
        if let Some(s) = hit {
            return s;
        }
        let base = self.raw.render(text, color);
        let surf = if self.outline <= 0 || !is_light(color) {
            base
        } else {
            let t = self.outline;
            let (w, h) = base.get_size();
            let mut s = Surface::new_alpha(w + 2 * t, h + 2 * t);
            let shade = self.raw.render(text, outline());
            for (dx, dy) in &self.offsets {
                s.blit(&shade, t + dx, t + dy);
            }
            s.blit(&base, t, t);
            s
        };
        let surf = Rc::new(surf);
        RCACHE.with(|c| {
            let mut c = c.borrow_mut();
            let t = c.tick;
            c.map.insert(key, (surf.clone(), t));
            if c.map.len() > CACHE_MAX + CACHE_MAX / 4 {
                // drop the least recently used entries down to the limit
                let mut ticks: Vec<u64> = c.map.values().map(|v| v.1).collect();
                ticks.sort_unstable();
                let cut = ticks[c.map.len() - CACHE_MAX];
                c.map.retain(|_, v| v.1 >= cut);
            }
        });
        surf
    }
}

pub fn make_font(size: i32, outline: i32, _heavy: bool) -> Font {
    let raw = find_custom_font()
        .and_then(|p| p.open(size))
        .or_else(|| RawFont::open_static(PYGAME_DEFAULT_FONT, size))
        .expect("no usable font found");
    Rc::new(StyledFont::new(raw, outline))
}

fn default_outline(size: i32, heavy: bool) -> i32 {
    if !heavy || size <= 14 {
        1
    } else if size <= 22 {
        2
    } else if size <= 34 {
        3
    } else if size <= 54 {
        4
    } else {
        5
    }
}

/// Font with the exact size asked (cached).
pub fn font_at(size: i32, heavy: bool, outline: Option<i32>) -> Font {
    let size = size.max(1);
    let outline = outline.unwrap_or_else(|| default_outline(size, heavy));
    let key = (size, heavy, outline);
    if let Some(f) = FONT_AT.with(|c| c.borrow().get(&key).cloned()) {
        return f;
    }
    let f = make_font(size, outline, heavy);
    FONT_AT.with(|c| {
        let mut c = c.borrow_mut();
        if c.len() > 400 {
            c.clear();
        }
        c.insert(key, f.clone());
    });
    f
}

pub fn clear_font_caches() {
    FONT_AT.with(|c| c.borrow_mut().clear());
    RCACHE.with(|c| c.borrow_mut().map.clear());
}

/// Text with an outline all around (8 directions).
pub fn render_outlined(font: &StyledFont, text: &str, color: Color, outline_color: Option<Color>, thickness: i32) -> Surface {
    let oc = outline_color.unwrap_or_else(|| outline_color_for(color));
    let base = font.raw.render(text, color);
    let (w, h) = base.get_size();
    let pad = thickness + 1;
    let mut s = Surface::new_alpha(w + pad * 2, h + pad * 2);
    let o = font.raw.render(text, oc);
    for dx in -thickness..=thickness {
        for dy in -thickness..=thickness {
            if dx == 0 && dy == 0 {
                continue;
            }
            s.blit(&o, pad + dx, pad + dy);
        }
    }
    s.blit(&base, pad, pad);
    s
}

pub enum Anchor {
    Center((i32, i32)),
    TopLeft((i32, i32)),
}

/// Writes text with a 1px contrast shadow (dark text only; light text already has its outline).
pub fn blit_text(surf: &mut Surface, font: &StyledFont, text: &str, color: Color, at: Anchor, shadow: bool) -> Rect {
    let t = font.render(text, color);
    let (w, h) = t.get_size();
    let r = match at {
        Anchor::Center(c) => Rect::with_center(w, h, c),
        Anchor::TopLeft((x, y)) => Rect::new(x, y, w, h),
    };
    if shadow && !is_light(color) {
        let s = font.raw.render(text, outline_color_for(color));
        surf.blit_with_alpha(&s, r.x + 1, r.y + 1, 150);
    }
    surf.blit(&t, r.x, r.y);
    r
}

pub fn blit_text_c(surf: &mut Surface, font: &StyledFont, text: &str, color: Color, center: (i32, i32)) -> Rect {
    blit_text(surf, font, text, color, Anchor::Center(center), true)
}

pub fn blit_text_tl(surf: &mut Surface, font: &StyledFont, text: &str, color: Color, topleft: (i32, i32)) -> Rect {
    blit_text(surf, font, text, color, Anchor::TopLeft(topleft), true)
}
