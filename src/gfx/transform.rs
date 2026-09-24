//! Port of pygame-ce's transform.smoothscale / transform.scale / transform.rotozoom.

use super::surface::Surface;

#[inline]
fn ch(p: u32, i: usize) -> u32 {
    (p >> (i * 8)) & 0xFF
}

// pygame-ce picks its SSE2 filters on x86-64 (smoothscale backend "SSE2"); these are exact
// scalar translations of simd_transform_sse2.c.

#[inline]
fn mulhi(a: u32, b: u32) -> u32 {
    ((a & 0xFFFF) * (b & 0xFFFF)) >> 16
}

fn filter_shrink_x(src: &[u32], sw: usize, dst: &mut [u32], dw: usize, height: usize) {
    let xspace: i32 = (0x04000 * sw / dw) as i32;
    let xrecip: u32 = ((0x4000_0000u32 / xspace as u32) & 0xFFFF) as u32;
    for y in 0..height {
        let srow = &src[y * sw..y * sw + sw];
        let drow = &mut dst[y * dw..y * dw + dw];
        let mut acc = [0u32; 4];
        let mut xcounter: i32 = xspace;
        let mut di = 0;
        for &sp in srow.iter() {
            if xcounter > 0x04000 {
                for (i, a) in acc.iter_mut().enumerate() {
                    *a = (*a + ch(sp, i)) & 0xFFFF;
                }
                xcounter -= 0x04000;
            } else {
                let xfrac = (0x04000 - xcounter) as u32;
                let mut out = 0u32;
                for (i, a) in acc.iter_mut().enumerate() {
                    let s4 = (ch(sp, i) << 2) & 0xFFFF;
                    let d = (mulhi(s4, xcounter as u32) + *a) & 0xFFFF;
                    *a = mulhi(s4, xfrac);
                    let v = mulhi(d, xrecip).min(255);
                    out |= v << (i * 8);
                }
                if di < dw {
                    drow[di] = out;
                }
                di += 1;
                xcounter = xspace - xfrac as i32;
            }
        }
    }
}

fn filter_shrink_y(src: &[u32], width: usize, dst: &mut [u32], sh: usize, dh: usize) {
    let yspace: i32 = (0x04000 * sh / dh) as i32;
    let yrecip: u32 = (0x4000_0000u32 / yspace as u32) & 0xFFFF;
    let mut ycounter = yspace;
    let mut templine = vec![0u32; width * 4];
    let mut dy = 0;
    for y in 0..sh {
        let srow = &src[y * width..y * width + width];
        if ycounter > 0x04000 {
            for (x, &sp) in srow.iter().enumerate() {
                for i in 0..4 {
                    templine[x * 4 + i] = (templine[x * 4 + i] + ch(sp, i)) & 0xFFFF;
                }
            }
            ycounter -= 0x04000;
        } else {
            let yfrac = (0x04000 - ycounter) as u32;
            let write = dy < dh;
            for (x, &sp) in srow.iter().enumerate() {
                let mut out = 0u32;
                for i in 0..4 {
                    let s4 = (ch(sp, i) << 2) & 0xFFFF;
                    let acc = templine[x * 4 + i];
                    templine[x * 4 + i] = mulhi(s4, yfrac);
                    let d = (mulhi(s4, ycounter as u32) + acc) & 0xFFFF;
                    out |= mulhi(d, yrecip).min(255) << (i * 8);
                }
                if write {
                    dst[dy * width + x] = out;
                }
            }
            dy += 1;
            ycounter = yspace - yfrac as i32;
        }
    }
}

fn filter_expand_x(src: &[u32], sw: usize, dst: &mut [u32], dw: usize, height: usize) {
    let mut xidx0 = vec![0usize; dw];
    let mut xm0 = vec![0u32; dw];
    let mut xm1 = vec![0u32; dw];
    for x in 0..dw {
        xm1[x] = (0x100 * ((x * (sw - 1)) % dw) / dw) as u32;
        xm0[x] = 0x100 - xm1[x];
        xidx0[x] = x * (sw - 1) / dw;
    }
    for y in 0..height {
        let srow = &src[y * sw..];
        let drow = &mut dst[y * dw..y * dw + dw];
        for x in 0..dw {
            let i0 = xidx0[x];
            let p0 = srow[i0];
            let p1 = if y * sw + i0 + 1 < src.len() { srow[i0 + 1] } else { 0 };
            let mut out = 0u32;
            for i in 0..4 {
                let v = (((ch(p0, i) * xm0[x]) & 0xFFFF) + ((ch(p1, i) * xm1[x]) & 0xFFFF)) & 0xFFFF;
                out |= (v >> 8).min(255) << (i * 8);
            }
            drow[x] = out;
        }
    }
}

fn filter_expand_y(src: &[u32], width: usize, dst: &mut [u32], sh: usize, dh: usize) {
    for y in 0..dh {
        let yidx0 = y * (sh - 1) / dh;
        let ym1 = (0x100 * ((y * (sh - 1)) % dh) / dh) as u32;
        let ym0 = 0x100 - ym1;
        let r0 = &src[yidx0 * width..yidx0 * width + width];
        let r1 = if yidx0 + 1 < sh { &src[(yidx0 + 1) * width..(yidx0 + 2) * width] } else { r0 };
        let drow = &mut dst[y * width..y * width + width];
        for x in 0..width {
            let (p0, p1) = (r0[x], r1[x]);
            let mut out = 0u32;
            for i in 0..4 {
                let v = (((ch(p0, i) * ym0) & 0xFFFF) + ((ch(p1, i) * ym1) & 0xFFFF)) & 0xFFFF;
                out |= (v >> 8).min(255) << (i * 8);
            }
            drow[x] = out;
        }
    }
}

/// pygame.transform.smoothscale(surf, (w, h))
pub fn smoothscale(src: &Surface, w: i32, h: i32) -> Surface {
    let (w, h) = (w.max(0), h.max(0));
    let mut out = if src.alpha { Surface::new_alpha(w, h) } else { Surface::new(w, h) };
    if w == 0 || h == 0 || src.w == 0 || src.h == 0 {
        return out;
    }
    if src.w == w && src.h == h {
        out.px.copy_from_slice(&src.px);
        return out;
    }
    let (sw, sh, dw, dh) = (src.w as usize, src.h as usize, w as usize, h as usize);
    // X pass
    let tmp: Vec<u32> = if dw < sw {
        let mut t = vec![0u32; dw * sh];
        filter_shrink_x(&src.px, sw, &mut t, dw, sh);
        t
    } else if dw > sw {
        let mut t = vec![0u32; dw * sh];
        filter_expand_x(&src.px, sw, &mut t, dw, sh);
        t
    } else {
        src.px.clone()
    };
    // Y pass
    if dh < sh {
        filter_shrink_y(&tmp, dw, &mut out.px, sh, dh);
    } else if dh > sh {
        filter_expand_y(&tmp, dw, &mut out.px, sh, dh);
    } else {
        out.px.copy_from_slice(&tmp);
    }
    if !out.alpha {
        for p in out.px.iter_mut() {
            *p |= 0xFF00_0000;
        }
    }
    out
}

/// pygame.transform.scale(surf, (w, h)) - SDL_SoftStretchNearest
pub fn scale(src: &Surface, w: i32, h: i32) -> Surface {
    let (w, h) = (w.max(0), h.max(0));
    let mut out = if src.alpha { Surface::new_alpha(w, h) } else { Surface::new(w, h) };
    if w == 0 || h == 0 || src.w == 0 || src.h == 0 {
        return out;
    }
    let incx: u32 = ((src.w as u32) << 16) / w as u32;
    let incy: u32 = ((src.h as u32) << 16) / h as u32;
    let mut posy: u32 = incy / 2;
    for y in 0..h as usize {
        let sy = ((posy >> 16) as usize).min(src.h as usize - 1);
        let mut posx: u32 = incx / 2;
        for x in 0..w as usize {
            let sx = ((posx >> 16) as usize).min(src.w as usize - 1);
            out.px[y * w as usize + x] = src.px[sy * src.w as usize + sx];
            posx = posx.wrapping_add(incx);
        }
        posy = posy.wrapping_add(incy);
    }
    out
}

#[derive(Clone, Copy)]
struct C4 {
    c: [i32; 4],
}

#[inline]
fn c4(p: u32) -> C4 {
    C4 { c: [ch(p, 0) as i32, ch(p, 1) as i32, ch(p, 2) as i32, ch(p, 3) as i32] }
}

#[inline]
fn interp(c00: C4, c01: C4, c10: C4, c11: C4, ex: i32, ey: i32) -> u32 {
    let mut out = 0u32;
    for i in 0..4 {
        let t1 = ((((c01.c[i] - c00.c[i]) * ex) >> 16) + c00.c[i]) & 0xff;
        let t2 = ((((c11.c[i] - c10.c[i]) * ex) >> 16) + c10.c[i]) & 0xff;
        let v = (((t2 - t1) * ey) >> 16) + t1;
        out |= ((v as u32) & 0xFF) << (i * 8);
    }
    out
}

fn zoom_surface_rgba(src: &Surface, dst: &mut Surface) {
    let sx = (65536.0 * ((src.w - 1) as f32) as f64 / (dst.w as f32) as f64) as i32;
    let sy = (65536.0 * ((src.h - 1) as f32) as f64 / (dst.h as f32) as f64) as i32;
    let mut sax = vec![0i32; dst.w as usize + 1];
    let mut say = vec![0i32; dst.h as usize + 1];
    let mut csx = 0i32;
    for v in sax.iter_mut() {
        *v = csx;
        csx &= 0xffff;
        csx += sx;
    }
    let mut csy = 0i32;
    for v in say.iter_mut() {
        *v = csy;
        csy &= 0xffff;
        csy += sy;
    }
    let sw = src.w as isize;
    let get = |idx: isize| -> u32 {
        if idx >= 0 && (idx as usize) < src.px.len() { src.px[idx as usize] } else { 0 }
    };
    let mut csp: isize = 0;
    for y in 0..dst.h as usize {
        let mut c00 = csp;
        let mut csax = 0usize;
        for x in 0..dst.w as usize {
            let ex = sax[csax] & 0xffff;
            let ey = say[y] & 0xffff;
            let v = interp(c4(get(c00)), c4(get(c00 + 1)), c4(get(c00 + sw)), c4(get(c00 + sw + 1)), ex, ey);
            dst.px[y * dst.w as usize + x] = v;
            csax += 1;
            c00 += (sax[csax] >> 16) as isize;
        }
        csp += (say[y + 1] >> 16) as isize * sw;
    }
}

fn transform_surface_rgba(src: &Surface, dst: &mut Surface, cx: i32, cy: i32, isin: i32, icos: i32) {
    let xd = ((src.w - dst.w) as i64) << 15;
    let yd = ((src.h - dst.h) as i64) << 15;
    let xd = xd as i32;
    let yd = yd as i32;
    let ax = (cx << 16).wrapping_sub(icos.wrapping_mul(cx));
    let ay = (cy << 16).wrapping_sub(isin.wrapping_mul(cx));
    let sw = src.w - 1;
    let sh = src.h - 1;
    let at = |x: i32, y: i32| -> C4 { c4(src.px[(y * src.w + x) as usize]) };
    for y in 0..dst.h {
        let dyv = cy - y;
        let mut sdx = ax.wrapping_add(isin.wrapping_mul(dyv)).wrapping_add(xd);
        let mut sdy = ay.wrapping_sub(icos.wrapping_mul(dyv)).wrapping_add(yd);
        for x in 0..dst.w {
            let dx = sdx >> 16;
            let dy = sdy >> 16;
            if dx >= -1 && dy >= -1 && dx < src.w && dy < src.h {
                let (c00, c01, c10, c11);
                if dx >= 0 && dy >= 0 && dx < sw && dy < sh {
                    c00 = at(dx, dy);
                    c01 = at(dx + 1, dy);
                    c10 = at(dx, dy + 1);
                    c11 = at(dx + 1, dy + 1);
                } else if dx == sw && dy == sh {
                    let c = at(dx, dy);
                    (c00, c01, c10, c11) = (c, c, c, c);
                } else if dx == -1 && dy == -1 {
                    let c = at(0, 0);
                    (c00, c01, c10, c11) = (c, c, c, c);
                } else if dx == -1 && dy == sh {
                    let c = at(0, dy);
                    (c00, c01, c10, c11) = (c, c, c, c);
                } else if dx == sw && dy == -1 {
                    let c = at(dx, 0);
                    (c00, c01, c10, c11) = (c, c, c, c);
                } else if dx == -1 {
                    let c = at(0, dy);
                    c00 = c;
                    c01 = c;
                    c10 = c;
                    c11 = at(0, dy + 1);
                } else if dy == -1 {
                    let c = at(dx, 0);
                    c00 = c;
                    c01 = c;
                    c10 = c;
                    c11 = at(dx + 1, 0);
                } else if dx == sw {
                    c00 = at(dx, dy);
                    c01 = c00;
                    c10 = at(dx, dy + 1);
                    c11 = c10;
                } else if dy == sh {
                    c00 = at(dx, dy);
                    c01 = at(dx + 1, dy);
                    c10 = c01;
                    c11 = c01;
                } else {
                    let c = at(0, 0);
                    (c00, c01, c10, c11) = (c, c, c, c);
                }
                let ex = sdx & 0xffff;
                let ey = sdy & 0xffff;
                dst.px[(y * dst.w + x) as usize] = interp(c00, c01, c10, c11, ex, ey);
            }
            sdx = sdx.wrapping_add(icos);
            sdy = sdy.wrapping_add(isin);
        }
    }
}

/// pygame.transform.rotozoom(surf, angle, scale) (always smooth).
pub fn rotozoom(src: &Surface, angle: f64, zoom: f64) -> Surface {
    // pygame parses both arguments as C floats
    let angle = angle as f32 as f64;
    let mut zoom = zoom as f32 as f64;
    if zoom == 0.0 || src.w == 0 || src.h == 0 {
        return Surface::new_alpha(0, 0);
    }
    if zoom < 0.001 {
        zoom = 0.001;
    }
    let zoominv = 65536.0 / (zoom * zoom);
    if angle.abs() > 0.001 {
        let radangle = angle * (std::f64::consts::PI / 180.0);
        let sanglezoom = radangle.sin() * zoom;
        let canglezoom = radangle.cos() * zoom;
        let x = (src.w / 2) as f64;
        let y = (src.h / 2) as f64;
        let cx = canglezoom * x;
        let cy = canglezoom * y;
        let sx = sanglezoom * x;
        let sy = sanglezoom * y;
        let dwh = ((cx + sy).abs().max((cx - sy).abs()).max((-cx + sy).abs()).max((-cx - sy).abs()).ceil() as i32).max(1);
        let dhh = ((sx + cy).abs().max((sx - cy).abs()).max((-sx + cy).abs()).max((-sx - cy).abs()).ceil() as i32).max(1);
        let (dw, dh) = (2 * dwh, 2 * dhh);
        let mut dst = Surface::new_alpha(dw, dh);
        dst.alpha = src.alpha;
        if !src.alpha {
            dst.px.fill(0);
        }
        transform_surface_rgba(src, &mut dst, dw / 2, dh / 2, (sanglezoom * zoominv) as i32, (canglezoom * zoominv) as i32);
        if !src.alpha {
            for p in dst.px.iter_mut() {
                *p |= 0xFF00_0000;
            }
        }
        dst
    } else {
        let dw = ((src.w as f64 * zoom) as i32).max(1);
        let dh = ((src.h as f64 * zoom) as i32).max(1);
        let mut dst = Surface::new_alpha(dw, dh);
        dst.alpha = src.alpha;
        zoom_surface_rgba(src, &mut dst);
        dst
    }
}
