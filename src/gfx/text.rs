//! Fonts through the vendored SDL_ttf 2.24 (HarfBuzz + FreeType), exactly like pygame-ce's
//! pygame.font.Font: size() = TTF_SizeUTF8, render() = TTF_RenderUTF8_Blended_Wrapped.

use super::surface::{Color, Surface};
use std::cell::RefCell;
use std::collections::HashMap;
use std::ffi::CString;
use std::os::raw::{c_char, c_int, c_void};

#[repr(C)]
#[derive(Clone, Copy)]
struct SdlColor {
    r: u8,
    g: u8,
    b: u8,
    a: u8,
}

#[allow(non_camel_case_types)]
type TTF_Font = c_void;

unsafe extern "C" {
    fn TTF_Init() -> c_int;
    fn TTF_OpenFont(file: *const c_char, ptsize: c_int) -> *mut TTF_Font;
    fn TTF_OpenFontRW(src: *mut sdl2::sys::SDL_RWops, freesrc: c_int, ptsize: c_int) -> *mut TTF_Font;
    fn TTF_SizeUTF8(font: *mut TTF_Font, text: *const c_char, w: *mut c_int, h: *mut c_int) -> c_int;
    fn TTF_FontHeight(font: *const TTF_Font) -> c_int;
    fn TTF_FontLineSkip(font: *const TTF_Font) -> c_int;
    fn TTF_FontAscent(font: *const TTF_Font) -> c_int;
    fn TTF_RenderUTF8_Blended_Wrapped(
        font: *mut TTF_Font,
        text: *const c_char,
        fg: SdlColor,
        wrap: u32,
    ) -> *mut sdl2::sys::SDL_Surface;
}

thread_local! {
    static TTF_READY: RefCell<bool> = const { RefCell::new(false) };
}

fn ensure_init() {
    TTF_READY.with(|r| {
        if !*r.borrow() {
            unsafe {
                TTF_Init();
            }
            *r.borrow_mut() = true;
        }
    });
}

/// A raw SDL_ttf font (pygame.font.Font).
pub struct RawFont {
    ptr: *mut TTF_Font,
    size_cache: RefCell<HashMap<String, (i32, i32)>>,
}

impl RawFont {
    pub fn open(path: &str, size: i32) -> Option<RawFont> {
        ensure_init();
        let c = CString::new(path).ok()?;
        let ptr = unsafe { TTF_OpenFont(c.as_ptr(), size.max(1)) };
        if ptr.is_null() {
            return None;
        }
        Some(RawFont { ptr, size_cache: RefCell::new(HashMap::new()) })
    }

    /// Font from a file built into the executable (crate::assets).
    pub fn open_static(data: &'static [u8], size: i32) -> Option<RawFont> {
        ensure_init();
        let ptr = unsafe {
            let rw = sdl2::sys::SDL_RWFromConstMem(data.as_ptr() as *const c_void, data.len() as c_int);
            if rw.is_null() {
                return None;
            }
            TTF_OpenFontRW(rw, 1, size.max(1))
        };
        if ptr.is_null() {
            return None;
        }
        Some(RawFont { ptr, size_cache: RefCell::new(HashMap::new()) })
    }

    fn cstr(text: &str) -> CString {
        // pygame refuses embedded NULs; the game never produces them, strip defensively
        CString::new(text.replace('\0', "")).unwrap()
    }

    /// font.size(text)
    pub fn size(&self, text: &str) -> (i32, i32) {
        if let Some(v) = self.size_cache.borrow().get(text) {
            return *v;
        }
        let c = Self::cstr(text);
        let (mut w, mut h) = (0, 0);
        unsafe {
            TTF_SizeUTF8(self.ptr, c.as_ptr(), &mut w, &mut h);
        }
        let mut cache = self.size_cache.borrow_mut();
        if cache.len() > 4000 {
            cache.clear();
        }
        cache.insert(text.to_string(), (w, h));
        (w, h)
    }

    pub fn get_height(&self) -> i32 {
        unsafe { TTF_FontHeight(self.ptr) }
    }
    pub fn get_linesize(&self) -> i32 {
        unsafe { TTF_FontLineSkip(self.ptr) }
    }
    pub fn get_ascent(&self) -> i32 {
        unsafe { TTF_FontAscent(self.ptr) }
    }

    /// font.render(text, True, color)
    pub fn render(&self, text: &str, color: Color) -> Surface {
        if text.is_empty() {
            // pygame: special 0-width surface with the font height (no alpha channel)
            return Surface::new(0, self.get_height());
        }
        let c = Self::cstr(text);
        let fg = SdlColor { r: color.r, g: color.g, b: color.b, a: 255 };
        unsafe {
            let s = TTF_RenderUTF8_Blended_Wrapped(self.ptr, c.as_ptr(), fg, 0);
            if s.is_null() {
                return Surface::new_alpha(0, self.get_height());
            }
            let w = (*s).w;
            let h = (*s).h;
            let pitch = (*s).pitch as usize;
            let mut out = Surface::new_alpha(w, h);
            let base = (*s).pixels as *const u8;
            for y in 0..h as usize {
                let row = base.add(y * pitch) as *const u32;
                for x in 0..w as usize {
                    out.px[y * w as usize + x] = *row.add(x);
                }
            }
            sdl2::sys::SDL_FreeSurface(s);
            out
        }
    }
}
