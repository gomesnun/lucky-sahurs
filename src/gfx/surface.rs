//! Software surfaces that reproduce pygame-ce's pixel behaviour: raw writes for fill / draw,
//! pygame's own alpha blender (the AVX2 path used on desktop) for per-pixel-alpha blits,
//! and the BLEND_* special flags that the game uses.

use super::rect::Rect;
use std::rc::Rc;

/// RGBA colour.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Default)]
pub struct Color {
    pub r: u8,
    pub g: u8,
    pub b: u8,
    pub a: u8,
}

impl Color {
    #[inline]
    pub const fn rgb(r: u8, g: u8, b: u8) -> Color {
        Color { r, g, b, a: 255 }
    }
    #[inline]
    pub const fn rgba(r: u8, g: u8, b: u8, a: u8) -> Color {
        Color { r, g, b, a }
    }
    #[inline]
    pub fn with_alpha(self, a: u8) -> Color {
        Color { a, ..self }
    }
    #[inline]
    pub fn rgb_tuple(self) -> (u8, u8, u8) {
        (self.r, self.g, self.b)
    }
    #[inline]
    pub fn argb(self) -> u32 {
        ((self.a as u32) << 24) | ((self.r as u32) << 16) | ((self.g as u32) << 8) | self.b as u32
    }
    #[inline]
    pub fn from_argb(p: u32) -> Color {
        Color { a: (p >> 24) as u8, r: (p >> 16) as u8, g: (p >> 8) as u8, b: p as u8 }
    }
    /// Colour from a tuple of ints (clamped the way pygame would reject/accept them).
    pub fn from_i32(r: i32, g: i32, b: i32) -> Color {
        Color::rgb(r.clamp(0, 255) as u8, g.clamp(0, 255) as u8, b.clamp(0, 255) as u8)
    }
}

pub const BLEND_NONE: u32 = 0;
pub const BLEND_RGB_MULT: u32 = 3;
pub const BLEND_RGBA_MULT: u32 = 0x12;
pub const BLEND_RGBA_MIN: u32 = 0x13;

/// A pixel buffer. Pixels are 0xAARRGGBB. Surfaces without per-pixel alpha keep A = 255.
#[derive(Clone)]
pub struct Surface {
    pub w: i32,
    pub h: i32,
    pub px: Vec<u32>,
    /// SRCALPHA (per-pixel alpha)
    pub alpha: bool,
    /// set_alpha() value (255 = none)
    pub blanket: u8,
    pub clip: Rect,
}

pub type Surf = Rc<Surface>;

#[inline]
fn div255(x: u32) -> u32 {
    // exact floor(x / 255) for 16-bit inputs (the 0x8081 trick used by the SIMD blitters)
    (x * 0x8081) >> 23
}

/// Per channel ((d << 8) + (s - d) * a + s) >> 8 (pygame-ce's AVX2 alpha blend), for the three RGB
/// channels at once: it equals (d * (256 - a) + s * (a + 1)) >> 8, whose terms are never negative
/// and never pass 16 bits, so red and blue share one u32 without carrying into each other.
#[inline(always)]
fn blend_rgb(s: u32, d: u32, a: u32) -> u32 {
    let (ia, sa1) = (256 - a, a + 1);
    let rb = (((d & 0x00FF_00FF) * ia + (s & 0x00FF_00FF) * sa1) >> 8) & 0x00FF_00FF;
    let g = ((((d >> 8) & 0xFF) * ia + ((s >> 8) & 0xFF) * sa1) >> 8) & 0xFF;
    rb | (g << 8)
}

/// One row of an alpha blit (see Surface::blit_alpha).
#[inline(always)]
fn blend_row_body(drow: &mut [u32], srow: &[u32], dst_has_alpha: bool, modulate: u32) {
    match (dst_has_alpha, modulate == 255) {
        (false, true) => {
            for (d, &s) in drow.iter_mut().zip(srow.iter()) {
                let sa = s >> 24;
                let rgb = blend_rgb(s, *d, sa);
                *d = if sa == 0 { *d } else { 0xFF00_0000 | rgb };
            }
        }
        (false, false) => {
            for (d, &s) in drow.iter_mut().zip(srow.iter()) {
                let sa = div255((s >> 24) * modulate);
                let rgb = blend_rgb(s, *d, sa);
                *d = if sa == 0 { *d } else { 0xFF00_0000 | rgb };
            }
        }
        (true, _) => {
            for (d, &s) in drow.iter_mut().zip(srow.iter()) {
                let mut sa = s >> 24;
                if modulate != 255 {
                    sa = div255(sa * modulate);
                }
                let dv = *d;
                let da = dv >> 24;
                let new_a = sa + da - div255(sa * da);
                let k = if da == 0 { 255 } else { sa };
                *d = (new_a << 24) | blend_rgb(s, dv, k);
            }
        }
    }
}

fn blend_row(drow: &mut [u32], srow: &[u32], dst_has_alpha: bool, modulate: u32) {
    blend_row_body(drow, srow, dst_has_alpha, modulate);
}

#[cfg(target_arch = "x86_64")]
#[target_feature(enable = "avx2")]
unsafe fn blend_row_avx2(drow: &mut [u32], srow: &[u32], dst_has_alpha: bool, modulate: u32) {
    blend_row_body(drow, srow, dst_has_alpha, modulate);
}

impl Surface {
    /// pygame.Surface((w, h)) - opaque, filled with black.
    pub fn new(w: i32, h: i32) -> Surface {
        let (w, h) = (w.max(0), h.max(0));
        Surface {
            w,
            h,
            px: vec![0xFF00_0000; (w as usize) * (h as usize)],
            alpha: false,
            blanket: 255,
            clip: Rect::new(0, 0, w, h),
        }
    }
    /// pygame.Surface((w, h), SRCALPHA) - fully transparent.
    pub fn new_alpha(w: i32, h: i32) -> Surface {
        let (w, h) = (w.max(0), h.max(0));
        Surface {
            w,
            h,
            px: vec![0; (w as usize) * (h as usize)],
            alpha: true,
            blanket: 255,
            clip: Rect::new(0, 0, w, h),
        }
    }
    #[inline]
    pub fn get_width(&self) -> i32 {
        self.w
    }
    #[inline]
    pub fn get_height(&self) -> i32 {
        self.h
    }
    #[inline]
    pub fn get_size(&self) -> (i32, i32) {
        (self.w, self.h)
    }
    #[inline]
    pub fn get_rect(&self) -> Rect {
        Rect::new(0, 0, self.w, self.h)
    }
    #[inline]
    pub fn map(&self, c: Color) -> u32 {
        if self.alpha { c.argb() } else { c.argb() | 0xFF00_0000 }
    }
    #[inline]
    pub fn get_at(&self, x: i32, y: i32) -> Color {
        Color::from_argb(self.px[(y * self.w + x) as usize])
    }
    #[inline]
    pub fn set_at_raw(&mut self, x: i32, y: i32, p: u32) {
        let i = (y * self.w + x) as usize;
        self.px[i] = p;
    }

    pub fn set_clip(&mut self, r: Option<Rect>) {
        let full = Rect::new(0, 0, self.w, self.h);
        self.clip = match r {
            None => full,
            Some(r) => r.sdl_intersect(&full).unwrap_or(Rect::new(0, 0, 0, 0)),
        };
    }

    pub fn set_alpha(&mut self, a: i32) {
        self.blanket = a.clamp(0, 255) as u8;
    }

    /// surf.copy()
    pub fn copy(&self) -> Surface {
        self.clone()
    }

    /// surf.subsurface(rect).copy()
    pub fn sub_copy(&self, r: Rect) -> Surface {
        let mut out = if self.alpha { Surface::new_alpha(r.w, r.h) } else { Surface::new(r.w, r.h) };
        for y in 0..r.h {
            let sy = r.y + y;
            if sy < 0 || sy >= self.h {
                continue;
            }
            for x in 0..r.w {
                let sx = r.x + x;
                if sx < 0 || sx >= self.w {
                    continue;
                }
                out.px[(y * r.w + x) as usize] = self.px[(sy * self.w + sx) as usize];
            }
        }
        out.blanket = self.blanket;
        out
    }

    /// surf.fill(color) / surf.fill(color, rect): raw write, respecting the clip rect.
    pub fn fill(&mut self, c: Color, rect: Option<Rect>) {
        let area = match rect {
            None => self.clip,
            Some(r) => match r.sdl_intersect(&self.clip) {
                Some(a) => a,
                None => return,
            },
        };
        let p = self.map(c);
        for y in area.y..area.y + area.h {
            let row = (y * self.w) as usize;
            self.px[row + area.x as usize..row + (area.x + area.w) as usize].fill(p);
        }
    }

    /// surf.fill(color, special_flags=...) for BLEND_RGB_MULT / BLEND_RGBA_MULT.
    pub fn fill_blend(&mut self, c: Color, flags: u32) {
        let area = self.clip;
        let has_alpha = self.alpha;
        for y in area.y..area.y + area.h {
            for x in area.x..area.x + area.w {
                let i = (y * self.w + x) as usize;
                let d = Color::from_argb(self.px[i]);
                let mul = |dc: u8, sc: u8| -> u8 {
                    if dc != 0 && sc != 0 { (((dc as u32 * sc as u32) + 255) >> 8) as u8 } else { 0 }
                };
                let mut o = Color::rgba(mul(d.r, c.r), mul(d.g, c.g), mul(d.b, c.b), d.a);
                if flags == BLEND_RGBA_MULT && has_alpha {
                    o.a = mul(d.a, c.a);
                }
                self.px[i] = if has_alpha { o.argb() } else { o.argb() | 0xFF00_0000 };
            }
        }
    }

    /// Clip a blit the way SDL_BlitSurface / pygame do. Returns (src_x, src_y, dst_x, dst_y, w, h).
    fn clip_blit(&self, src: &Surface, dx: i32, dy: i32, area: Option<Rect>) -> Option<(i32, i32, i32, i32, i32, i32)> {
        let (mut sx, mut sy, mut w, mut h) = match area {
            None => (0, 0, src.w, src.h),
            Some(a) => (a.x, a.y, a.w, a.h),
        };
        let (mut dx, mut dy) = (dx, dy);
        // clip the source rect to the source surface
        if sx < 0 {
            w += sx;
            dx -= sx;
            sx = 0;
        }
        let maxw = src.w - sx;
        if maxw < w {
            w = maxw;
        }
        if sy < 0 {
            h += sy;
            dy -= sy;
            sy = 0;
        }
        let maxh = src.h - sy;
        if maxh < h {
            h = maxh;
        }
        // clip to the destination clip rect
        let clip = self.clip;
        let mut dx2 = clip.x - dx;
        if dx2 > 0 {
            sx += dx2;
            w -= dx2;
            dx += dx2;
        }
        dx2 = dx + w - clip.x - clip.w;
        if dx2 > 0 {
            w -= dx2;
        }
        let mut dy2 = clip.y - dy;
        if dy2 > 0 {
            sy += dy2;
            h -= dy2;
            dy += dy2;
        }
        dy2 = dy + h - clip.y - clip.h;
        if dy2 > 0 {
            h -= dy2;
        }
        if w <= 0 || h <= 0 {
            return None;
        }
        Some((sx, sy, dx, dy, w, h))
    }

    /// dst.blit(src, (x, y))
    #[inline]
    pub fn blit(&mut self, src: &Surface, x: i32, y: i32) {
        self.blit_ex(src, x, y, None, BLEND_NONE);
    }

    /// dst.blit(src, (x, y), area, special_flags)
    pub fn blit_ex(&mut self, src: &Surface, x: i32, y: i32, area: Option<Rect>, flags: u32) {
        self.blit_full(src, x, y, area, flags, src.blanket);
    }

    /// Blit as if `src.set_alpha(alpha)` had been called first (without touching a shared surface).
    pub fn blit_with_alpha(&mut self, src: &Surface, x: i32, y: i32, alpha: i32) {
        self.blit_full(src, x, y, None, BLEND_NONE, alpha.clamp(0, 255) as u8);
    }

    fn blit_full(&mut self, src: &Surface, x: i32, y: i32, area: Option<Rect>, flags: u32, blanket: u8) {
        let Some((sx, sy, dx, dy, w, h)) = self.clip_blit(src, x, y, area) else {
            return;
        };
        let dst_alpha = self.alpha;
        match flags {
            BLEND_RGBA_MIN if src.alpha && dst_alpha => {
                for row in 0..h {
                    let si = ((sy + row) * src.w + sx) as usize;
                    let di = ((dy + row) * self.w + dx) as usize;
                    for k in 0..w as usize {
                        let s = src.px[si + k];
                        let d = self.px[di + k];
                        let mut o = 0u32;
                        for sh in [0u32, 8, 16, 24] {
                            let sc = (s >> sh) & 0xFF;
                            let dc = (d >> sh) & 0xFF;
                            o |= sc.min(dc) << sh;
                        }
                        self.px[di + k] = o;
                    }
                }
            }
            BLEND_RGBA_MIN => {
                // RGB min only (one side has no alpha channel)
                for row in 0..h {
                    let si = ((sy + row) * src.w + sx) as usize;
                    let di = ((dy + row) * self.w + dx) as usize;
                    for k in 0..w as usize {
                        let s = src.px[si + k];
                        let d = self.px[di + k];
                        let mut o = d & 0xFF00_0000;
                        for sh in [0u32, 8, 16] {
                            o |= ((s >> sh) & 0xFF).min((d >> sh) & 0xFF) << sh;
                        }
                        self.px[di + k] = o;
                    }
                }
            }
            BLEND_RGBA_MULT | BLEND_RGB_MULT => {
                let rgba = flags == BLEND_RGBA_MULT && src.alpha && dst_alpha;
                for row in 0..h {
                    let si = ((sy + row) * src.w + sx) as usize;
                    let di = ((dy + row) * self.w + dx) as usize;
                    for k in 0..w as usize {
                        let s = src.px[si + k];
                        let d = self.px[di + k];
                        let mul = |sh: u32| -> u32 {
                            let sc = (s >> sh) & 0xFF;
                            let dc = (d >> sh) & 0xFF;
                            if sc != 0 && dc != 0 { ((sc * dc + 255) >> 8).min(255) } else { 0 }
                        };
                        let a = if rgba { mul(24) } else { d >> 24 };
                        self.px[di + k] = (a << 24) | (mul(16) << 16) | (mul(8) << 8) | mul(0);
                    }
                }
            }
            _ => {
                if src.alpha {
                    self.blit_alpha(src, sx, sy, dx, dy, w, h, blanket);
                } else {
                    self.blit_opaque(src, sx, sy, dx, dy, w, h, blanket);
                }
            }
        }
    }

    /// Per-pixel alpha source: pygame-ce's alphablit_alpha (AVX2 variant, which is what runs on desktop).
    #[allow(clippy::too_many_arguments)]
    fn blit_alpha(&mut self, src: &Surface, sx: i32, sy: i32, dx: i32, dy: i32, w: i32, h: i32, blanket: u8) {
        let modulate = blanket as u32;
        let dst_has_alpha = self.alpha;
        #[cfg(target_arch = "x86_64")]
        let avx2 = std::is_x86_feature_detected!("avx2");
        for row in 0..h {
            let si = ((sy + row) * src.w + sx) as usize;
            let di = ((dy + row) * self.w + dx) as usize;
            let srow = &src.px[si..si + w as usize];
            let drow = &mut self.px[di..di + w as usize];
            #[cfg(target_arch = "x86_64")]
            if avx2 {
                // SAFETY: the CPU supports AVX2 (checked above)
                unsafe { blend_row_avx2(drow, srow, dst_has_alpha, modulate) };
                continue;
            }
            blend_row(drow, srow, dst_has_alpha, modulate);
        }
    }

    /// Source without per-pixel alpha (SDL_BlitSurface path).
    #[allow(clippy::too_many_arguments)]
    fn blit_opaque(&mut self, src: &Surface, sx: i32, sy: i32, dx: i32, dy: i32, w: i32, h: i32, blanket: u8) {
        let a = blanket as u32;
        for row in 0..h {
            let si = ((sy + row) * src.w + sx) as usize;
            let di = ((dy + row) * self.w + dx) as usize;
            if a == 255 {
                let srow = &src.px[si..si + w as usize];
                for (d, &s) in self.px[di..di + w as usize].iter_mut().zip(srow.iter()) {
                    *d = s | 0xFF00_0000;
                }
            } else {
                for k in 0..w as usize {
                    let s = src.px[si + k];
                    let dv = self.px[di + k];
                    let blend = |sh: u32| -> u32 {
                        let sc = ((s >> sh) & 0xFF) as i32;
                        let dc = ((dv >> sh) & 0xFF) as i32;
                        (((sc - dc) * a as i32) / 255 + dc) as u32 & 0xFF
                    };
                    let out_a = if self.alpha { dv >> 24 } else { 0xFF };
                    self.px[di + k] = (out_a << 24) | (blend(16) << 16) | (blend(8) << 8) | blend(0);
                }
            }
        }
    }
}

impl Surface {
    /// surf.get_bounding_rect(): smallest rect with alpha >= 1 (the whole surface without SRCALPHA).
    pub fn bounding_rect(&self) -> Rect {
        if !self.alpha {
            return Rect::new(0, 0, self.w, self.h);
        }
        let (mut minx, mut miny, mut maxx, mut maxy) = (self.w, self.h, -1, -1);
        for y in 0..self.h {
            for x in 0..self.w {
                if self.px[(y * self.w + x) as usize] >> 24 != 0 {
                    minx = minx.min(x);
                    maxx = maxx.max(x);
                    miny = miny.min(y);
                    maxy = maxy.max(y);
                }
            }
        }
        if maxx < 0 {
            return Rect::new(0, 0, 0, 0);
        }
        Rect::new(minx, miny, maxx - minx + 1, maxy - miny + 1)
    }
}
