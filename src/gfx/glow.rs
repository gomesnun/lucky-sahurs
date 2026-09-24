//! Glow (bloom): bright parts of the frame - rare cards, the ROLL button, sparkles, gold text, auras - bleed a soft
//! light around them. Done on a quarter-size copy (find what's bright, blur it, add it back), so it costs little.

use super::Surface;

const SCALE: usize = 4;
/// how bright (its strongest channel, 0-1) a pixel has to be before it starts to glow
const THRESHOLD: f32 = 0.5;

#[derive(Default)]
pub struct GlowBuf {
    small: Vec<[f32; 3]>,
    tmp: Vec<[f32; 3]>,
}

/// Box blur of radius r along rows (horizontal) or columns, via a running sum.
fn blur_pass(src: &[[f32; 3]], dst: &mut [[f32; 3]], w: usize, h: usize, r: usize, horizontal: bool) {
    let (len, lines) = if horizontal { (w, h) } else { (h, w) };
    let at = |line: usize, i: usize| if horizontal { line * w + i } else { i * w + line };
    let norm = 1.0 / (2 * r + 1) as f32;
    for line in 0..lines {
        let mut acc = [0.0f32; 3];
        for k in 0..=r.min(len - 1) {
            let p = src[at(line, k)];
            for c in 0..3 {
                acc[c] += p[c];
            }
        }
        for i in 0..len {
            dst[at(line, i)] = [acc[0] * norm, acc[1] * norm, acc[2] * norm];
            if i + r + 1 < len {
                let p = src[at(line, i + r + 1)];
                for c in 0..3 {
                    acc[c] += p[c];
                }
            }
            if i >= r {
                let p = src[at(line, i - r)];
                for c in 0..3 {
                    acc[c] -= p[c];
                }
            }
        }
    }
}

/// Adds the glow to an opaque frame (0xAARRGGBB pixels). strength ~0.5-1.
pub fn apply(frame: &mut Surface, buf: &mut GlowBuf, strength: f32) {
    let (w, h) = (frame.w as usize, frame.h as usize);
    let (sw, sh) = (w / SCALE, h / SCALE);
    if sw < 4 || sh < 4 {
        return;
    }
    buf.small.resize(sw * sh, [0.0; 3]);
    buf.tmp.resize(sw * sh, [0.0; 3]);
    // 1. what's bright, at quarter size (2x2 samples of each 4x4 block)
    for sy in 0..sh {
        for sx in 0..sw {
            let mut sum = [0.0f32; 3];
            for (dy, dx) in [(0, 0), (0, 2), (2, 0), (2, 2)] {
                let p = frame.px[(sy * SCALE + dy + 1) * w + sx * SCALE + dx + 1];
                sum[0] += ((p >> 16) & 255) as f32;
                sum[1] += ((p >> 8) & 255) as f32;
                sum[2] += (p & 255) as f32;
            }
            let c = [sum[0] / 1020.0, sum[1] / 1020.0, sum[2] / 1020.0];
            let key = c[0].max(c[1]).max(c[2]);
            let sat = if key > 0.0 { (key - c[0].min(c[1]).min(c[2])) / key } else { 0.0 };
            let k = ((key - THRESHOLD) / (1.0 - THRESHOLD)).clamp(0.0, 1.0);
            // colourful things (gold, rarity colours, sparkles) glow the most; big white areas only a little
            let k = k * k * (0.3 + 0.7 * sat);
            buf.small[sy * sw + sx] = [c[0] * k, c[1] * k, c[2] * k];
        }
    }
    // 2. blur it (3 box passes each way ~ a gaussian)
    for _ in 0..3 {
        blur_pass(&buf.small, &mut buf.tmp, sw, sh, 5, true);
        blur_pass(&buf.tmp, &mut buf.small, sw, sh, 5, false);
    }
    // 3. add it back (bilinear), screen-blended so whites don't blow out. Only 4x4 blocks near some glow are
    //    touched (most of the screen is dark), and the rows are shared between the CPU cores.
    let lit_min = 0.004 / strength;
    let mut lit = vec![false; sw * sh];
    for by in 0..sh {
        for bx in 0..sw {
            let p = buf.small[by * sw + bx];
            if p[0] + p[1] + p[2] > lit_min {
                for yy in by.saturating_sub(1)..=(by + 1).min(sh - 1) {
                    for xx in bx.saturating_sub(1)..=(bx + 1).min(sw - 1) {
                        lit[yy * sw + xx] = true;
                    }
                }
            }
        }
    }
    let small = &buf.small;
    let lit = &lit;
    let s = strength * 255.0;
    let threads = std::thread::available_parallelism().map(|n| n.get()).unwrap_or(1).clamp(1, 8);
    let rows_per = h.div_ceil(threads).div_ceil(SCALE) * SCALE;
    std::thread::scope(|scope| {
        for (chunk_i, chunk) in frame.px.chunks_mut(rows_per * w).enumerate() {
            scope.spawn(move || {
                let mut row = vec![[0.0f32; 3]; sw];
                for (ry, line) in chunk.chunks_mut(w).enumerate() {
                    let y = chunk_i * rows_per + ry;
                    let by = (y / SCALE).min(sh - 1);
                    let fy = ((y as f32 + 0.5) / SCALE as f32 - 0.5).clamp(0.0, (sh - 1) as f32);
                    let ya = fy as usize;
                    let yb = (ya + 1).min(sh - 1);
                    let ty = fy - ya as f32;
                    for sx in 0..sw {
                        let (a, b) = (small[ya * sw + sx], small[yb * sw + sx]);
                        row[sx] = [a[0] + (b[0] - a[0]) * ty, a[1] + (b[1] - a[1]) * ty, a[2] + (b[2] - a[2]) * ty];
                    }
                    for bx in 0..sw {
                        if !lit[by * sw + bx] {
                            continue;
                        }
                        for x in bx * SCALE..((bx + 1) * SCALE).min(w) {
                            let fx = ((x as f32 + 0.5) / SCALE as f32 - 0.5).clamp(0.0, (sw - 1) as f32);
                            let xa = fx as usize;
                            let xb = (xa + 1).min(sw - 1);
                            let tx = fx - xa as f32;
                            let (a, b) = (row[xa], row[xb]);
                            let p = &mut line[x];
                            let mut out = *p & 0xff00_0000;
                            for (ch, shift) in [(0usize, 16u32), (1, 8), (2, 0)] {
                                let g = a[ch] + (b[ch] - a[ch]) * tx;
                                let v = ((*p >> shift) & 255) as f32;
                                let nv = v + g * s * (1.0 - v * (1.0 / 255.0));
                                out |= (nv.min(255.0) as u32) << shift;
                            }
                            *p = out;
                        }
                    }
                }
            });
        }
    });
}
