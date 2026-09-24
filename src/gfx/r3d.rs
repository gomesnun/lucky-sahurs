//! A small software 3D renderer (for the battle arena): a camera, textured quads with a depth buffer, ray-traced
//! spheres, camera-facing sprites and glowing particles. Everything draws into its own colour + depth buffers
//! (0xAARRGGBB, like Surface), which the caller scales onto the canvas.

use super::Surface;
use std::ops::{Add, Mul, Neg, Sub};

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct V3 {
    pub x: f64,
    pub y: f64,
    pub z: f64,
}

pub const fn v3(x: f64, y: f64, z: f64) -> V3 {
    V3 { x, y, z }
}

impl V3 {
    pub fn dot(self, o: V3) -> f64 {
        self.x * o.x + self.y * o.y + self.z * o.z
    }
    pub fn cross(self, o: V3) -> V3 {
        v3(self.y * o.z - self.z * o.y, self.z * o.x - self.x * o.z, self.x * o.y - self.y * o.x)
    }
    pub fn len(self) -> f64 {
        self.dot(self).sqrt()
    }
    pub fn norm(self) -> V3 {
        let l = self.len();
        if l > 1e-12 { self * (1.0 / l) } else { self }
    }
    pub fn lerp(self, o: V3, t: f64) -> V3 {
        self + (o - self) * t
    }
}

impl Add for V3 {
    type Output = V3;
    fn add(self, o: V3) -> V3 {
        v3(self.x + o.x, self.y + o.y, self.z + o.z)
    }
}
impl Sub for V3 {
    type Output = V3;
    fn sub(self, o: V3) -> V3 {
        v3(self.x - o.x, self.y - o.y, self.z - o.z)
    }
}
impl Mul<f64> for V3 {
    type Output = V3;
    fn mul(self, k: f64) -> V3 {
        v3(self.x * k, self.y * k, self.z * k)
    }
}
impl Neg for V3 {
    type Output = V3;
    fn neg(self) -> V3 {
        v3(-self.x, -self.y, -self.z)
    }
}

pub const UP: V3 = v3(0.0, 1.0, 0.0);
const NEAR: f64 = 0.08;

/// A 16x16 (or any size) texture of 0xRRGGBB colours.
#[derive(Clone)]
pub struct Tex {
    pub w: usize,
    pub h: usize,
    pub px: Vec<u32>,
}

#[derive(Clone, Copy)]
pub struct Camera {
    pub pos: V3,
    pub target: V3,
    /// vertical field of view, degrees
    pub fov: f64,
}

/// A camera ready for drawing: its axes and the focal length in pixels.
#[derive(Clone, Copy)]
pub struct View {
    pub pos: V3,
    pub r: V3,
    pub u: V3,
    pub f: V3,
    pub focal: f64,
    pub cx: f64,
    pub cy: f64,
}

impl View {
    pub fn new(cam: &Camera, w: usize, h: usize) -> View {
        let f = (cam.target - cam.pos).norm();
        let r = f.cross(UP).norm();
        let u = r.cross(f);
        let focal = (h as f64 / 2.0) / (cam.fov.to_radians() / 2.0).tan();
        View { pos: cam.pos, r, u, f, focal, cx: w as f64 / 2.0, cy: h as f64 / 2.0 }
    }
    /// world -> camera space (x right, y up, z forward)
    pub fn to_cam(&self, p: V3) -> V3 {
        let d = p - self.pos;
        v3(d.dot(self.r), d.dot(self.u), d.dot(self.f))
    }
    /// world -> (screen x, screen y, depth), or None behind the camera
    pub fn project(&self, p: V3) -> Option<(f64, f64, f64)> {
        let c = self.to_cam(p);
        if c.z <= NEAR {
            return None;
        }
        Some((self.cx + c.x / c.z * self.focal, self.cy - c.y / c.z * self.focal, c.z))
    }
    /// the world direction through a screen pixel
    pub fn ray(&self, sx: f64, sy: f64) -> V3 {
        (self.r * ((sx - self.cx) / self.focal) + self.u * ((self.cy - sy) / self.focal) + self.f).norm()
    }
}

pub struct Frame {
    pub w: usize,
    pub h: usize,
    pub color: Vec<u32>,
    pub depth: Vec<f32>,
}

#[inline]
fn rgb(c: u32) -> (f64, f64, f64) {
    (((c >> 16) & 255) as f64, ((c >> 8) & 255) as f64, (c & 255) as f64)
}

#[inline]
fn pack(r: f64, g: f64, b: f64) -> u32 {
    0xff00_0000 | ((r.clamp(0.0, 255.0) as u32) << 16) | ((g.clamp(0.0, 255.0) as u32) << 8) | (b.clamp(0.0, 255.0) as u32)
}

/// Mixes a colour toward the fog colour by t (0-1).
#[inline]
pub fn fog_mix(c: u32, fog: u32, t: f64) -> u32 {
    if t <= 0.0 {
        return c;
    }
    let (r, g, b) = rgb(c);
    let (fr, fg, fb) = rgb(fog);
    pack(r + (fr - r) * t, g + (fg - g) * t, b + (fb - b) * t)
}

/// One corner of a quad / triangle: position (camera space) and texture coordinates.
#[derive(Clone, Copy)]
struct Vert {
    c: V3,
    u: f64,
    v: f64,
}

pub struct Fog {
    pub color: u32,
    pub start: f64,
    pub end: f64,
}

impl Frame {
    pub fn new(w: usize, h: usize) -> Frame {
        Frame { w, h, color: vec![0xff00_0000; w * h], depth: vec![f32::INFINITY; w * h] }
    }

    /// Sky: a vertical gradient (top to horizon colour), and a clear depth buffer.
    pub fn clear_sky(&mut self, top: u32, bottom: u32) {
        let (tr, tg, tb) = rgb(top);
        let (br, bg, bb) = rgb(bottom);
        for y in 0..self.h {
            let t = y as f64 / self.h.max(1) as f64;
            let c = pack(tr + (br - tr) * t, tg + (bg - tg) * t, tb + (bb - tb) * t);
            self.color[y * self.w..(y + 1) * self.w].fill(c);
        }
        self.depth.fill(f32::INFINITY);
    }

    /// A textured quad (see Band::quad), on the whole frame.
    pub fn quad(&mut self, view: &View, corners: [V3; 4], tex: &Tex, light: f64, fog: &Fog) {
        let (w, h) = (self.w, self.h);
        Band { w, y0: 0, y1: h, color: &mut self.color, depth: &mut self.depth }.quad(view, corners, tex, light, fog);
    }

    /// The frame cut into horizontal bands of `rows` rows (to draw them on several threads).
    pub fn bands(&mut self, rows: usize) -> Vec<Band<'_>> {
        let w = self.w;
        let h = self.h;
        self.color
            .chunks_mut(rows * w)
            .zip(self.depth.chunks_mut(rows * w))
            .enumerate()
            .map(|(i, (color, depth))| Band { w, y0: i * rows, y1: (i * rows + rows).min(h), color, depth })
            .collect()
    }

    /// A sphere, ray traced. `shade(normal, local)` gives its colour at a point: `local` is the normal in the
    /// sphere's own frame (x right, y up, z forward = where it faces).
    pub fn sphere(&mut self, view: &View, center: V3, radius: f64, facing: V3, shade: &dyn Fn(V3, V3) -> u32, fog: &Fog) {
        let Some((sx, sy, z)) = view.project(center) else { return };
        let rpx = radius / z * view.focal * 1.25 + 2.0;
        let (x0, x1) = ((sx - rpx).floor().max(0.0) as i64, (sx + rpx).ceil().min(self.w as f64 - 1.0) as i64);
        let (y0, y1) = ((sy - rpx).floor().max(0.0) as i64, (sy + rpx).ceil().min(self.h as f64 - 1.0) as i64);
        let fwd = v3(facing.x, 0.0, facing.z).norm();
        let right = UP.cross(fwd);
        for y in y0..=y1 {
            for x in x0..=x1 {
                let d = view.ray(x as f64 + 0.5, y as f64 + 0.5);
                let oc = view.pos - center;
                let b = oc.dot(d);
                let c = oc.dot(oc) - radius * radius;
                let disc = b * b - c;
                if disc < 0.0 {
                    continue;
                }
                let t = -b - disc.sqrt();
                if t <= NEAR {
                    continue;
                }
                let depth = t * d.dot(view.f);
                let i = y as usize * self.w + x as usize;
                if depth as f32 >= self.depth[i] {
                    continue;
                }
                let n = (view.pos + d * t - center) * (1.0 / radius);
                let local = v3(n.dot(right), n.y, n.dot(fwd));
                let col = shade(n, local);
                let ft = ((depth - fog.start) / (fog.end - fog.start)).clamp(0.0, 1.0);
                self.color[i] = fog_mix(col, fog.color, ft);
                self.depth[i] = depth as f32;
            }
        }
    }

    /// A camera-facing picture: `anchor` (a point of the image, as fractions 0-1) is placed at `at`, the picture is
    /// `world_h` tall. alpha multiplies its opacity; flash (0-1) whitens it.
    #[allow(clippy::too_many_arguments)]
    pub fn sprite(&mut self, view: &View, img: &Surface, at: V3, anchor: (f64, f64), world_h: f64, alpha: f64, flash: f64, mirror: bool, fog: &Fog) {
        let Some((sx, sy, z)) = view.project(at) else { return };
        let hpx = world_h / z * view.focal;
        if hpx < 1.0 {
            return;
        }
        let wpx = hpx * img.w as f64 / img.h as f64;
        let left = sx - anchor.0 * wpx;
        let top = sy - anchor.1 * hpx;
        let (x0, x1) = (left.floor().max(0.0) as i64, (left + wpx).ceil().min(self.w as f64) as i64);
        let (y0, y1) = (top.floor().max(0.0) as i64, (top + hpx).ceil().min(self.h as f64) as i64);
        let ft = ((z - fog.start) / (fog.end - fog.start)).clamp(0.0, 1.0);
        for y in y0..y1 {
            let v = ((y as f64 + 0.5 - top) / hpx * img.h as f64) as i64;
            if v < 0 || v >= img.h as i64 {
                continue;
            }
            for x in x0..x1 {
                let mut u = ((x as f64 + 0.5 - left) / wpx * img.w as f64) as i64;
                if u < 0 || u >= img.w as i64 {
                    continue;
                }
                if mirror {
                    u = img.w as i64 - 1 - u;
                }
                let i = y as usize * self.w + x as usize;
                if z as f32 >= self.depth[i] {
                    continue;
                }
                let p = img.px[v as usize * img.w as usize + u as usize];
                let a = ((p >> 24) & 255) as f64 / 255.0 * alpha;
                if a < 0.02 {
                    continue;
                }
                let (mut r, mut g, mut b) = rgb(p);
                if flash > 0.0 {
                    r += (255.0 - r) * flash;
                    g += (255.0 - g) * flash;
                    b += (255.0 - b) * flash;
                }
                let src = fog_mix(pack(r, g, b), fog.color, ft);
                let (sr, sg, sb) = rgb(src);
                let (dr, dg, db) = rgb(self.color[i]);
                self.color[i] = pack(dr + (sr - dr) * a, dg + (sg - dg) * a, db + (sb - db) * a);
                if a > 0.6 {
                    self.depth[i] = z as f32;
                }
            }
        }
    }

    /// A glowing dot (added light), hidden behind nearer things. size is its radius in world units.
    pub fn glow(&mut self, view: &View, at: V3, size: f64, color: (f64, f64, f64), strength: f64) {
        let Some((sx, sy, z)) = view.project(at) else { return };
        let r = (size / z * view.focal).clamp(0.6, 60.0);
        let (x0, x1) = ((sx - r * 2.0).floor().max(0.0) as i64, (sx + r * 2.0).ceil().min(self.w as f64 - 1.0) as i64);
        let (y0, y1) = ((sy - r * 2.0).floor().max(0.0) as i64, (sy + r * 2.0).ceil().min(self.h as f64 - 1.0) as i64);
        for y in y0..=y1 {
            for x in x0..=x1 {
                let i = y as usize * self.w + x as usize;
                if z as f32 > self.depth[i] + 0.3 {
                    continue;
                }
                let d2 = ((x as f64 + 0.5 - sx).powi(2) + (y as f64 + 0.5 - sy).powi(2)) / (r * r);
                if d2 > 4.0 {
                    continue;
                }
                // a bright core and a soft halo
                let k = strength * ((1.0 - d2).max(0.0) * 0.9 + (-d2 * 1.2).exp() * 0.45);
                if k <= 0.003 {
                    continue;
                }
                let (dr, dg, db) = rgb(self.color[i]);
                self.color[i] = pack(dr + color.0 * k, dg + color.1 * k, db + color.2 * k);
            }
        }
    }

    /// Copies the frame into a Surface (opaque).
    pub fn to_surface(&self) -> Surface {
        let mut s = Surface::new(self.w as i32, self.h as i32);
        s.px.copy_from_slice(&self.color);
        s
    }
}

/// A horizontal slice of a Frame (rows y0..y1): textured quads can be drawn into it on its own thread.
pub struct Band<'a> {
    pub w: usize,
    pub y0: usize,
    pub y1: usize,
    pub color: &'a mut [u32],
    pub depth: &'a mut [f32],
}

impl Band<'_> {
    /// A textured quad, lit by `light` (0-1) and fogged by distance. Corners: top-left, bottom-left, bottom-right,
    /// top-right of the texture, going round the face so that (b-a)x(c-a) points out of it.
    pub fn quad(&mut self, view: &View, corners: [V3; 4], tex: &Tex, light: f64, fog: &Fog) {
        // back faces are skipped: corners go around the face so that (b-a)x(c-a) points out of it
        let n = (corners[1] - corners[0]).cross(corners[2] - corners[0]);
        if n.dot(view.pos - corners[0]) <= 0.0 {
            return;
        }
        let uv = [(0.0, 0.0), (0.0, 1.0), (1.0, 1.0), (1.0, 0.0)];
        let quad: [Vert; 4] = std::array::from_fn(|i| Vert { c: view.to_cam(corners[i]), u: uv[i].0, v: uv[i].1 });
        if quad.iter().all(|p| p.c.z <= NEAR) {
            return;
        }
        // off to a side of the view, or past the band's rows: nothing to draw
        let (hw, hh) = (view.cx / view.focal, view.cy / view.focal);
        if quad.iter().all(|p| p.c.x > p.c.z.max(NEAR) * hw) || quad.iter().all(|p| p.c.x < -p.c.z.max(NEAR) * hw) {
            return;
        }
        if quad.iter().all(|p| p.c.z > NEAR) {
            let sy = |p: &Vert| view.cy - p.c.y / p.c.z * view.focal;
            if quad.iter().all(|p| sy(p) < self.y0 as f64 - 1.0) || quad.iter().all(|p| sy(p) > self.y1 as f64 + 1.0) {
                return;
            }
            let _ = hh;
            self.tri(view, [quad[0], quad[1], quad[2]], tex, light, fog);
            self.tri(view, [quad[0], quad[2], quad[3]], tex, light, fog);
            return;
        }
        let (poly, n) = clip_near(&quad);
        for i in 1..n.saturating_sub(1) {
            self.tri(view, [poly[0], poly[i], poly[i + 1]], tex, light, fog);
        }
    }

    fn tri(&mut self, view: &View, v: [Vert; 3], tex: &Tex, light: f64, fog: &Fog) {
        let s: [(f64, f64, f64); 3] = std::array::from_fn(|k| {
            let p = v[k].c;
            (view.cx + p.x / p.z * view.focal, view.cy - p.y / p.z * view.focal, 1.0 / p.z)
        });
        let area = (s[1].0 - s[0].0) * (s[2].1 - s[0].1) - (s[2].0 - s[0].0) * (s[1].1 - s[0].1);
        if area.abs() < 1e-9 {
            return;
        }
        let minx = s.iter().map(|p| p.0).fold(f64::INFINITY, f64::min).floor().max(0.0) as i64;
        let maxx = s.iter().map(|p| p.0).fold(f64::NEG_INFINITY, f64::max).ceil().min(self.w as f64 - 1.0) as i64;
        let miny = s.iter().map(|p| p.1).fold(f64::INFINITY, f64::min).floor().max(self.y0 as f64) as i64;
        let maxy = s.iter().map(|p| p.1).fold(f64::NEG_INFINITY, f64::max).ceil().min(self.y1 as f64 - 1.0) as i64;
        if minx > maxx || miny > maxy {
            return;
        }
        let inv = 1.0 / area;
        let uz: [f64; 3] = std::array::from_fn(|k| v[k].u * s[k].2);
        let vz: [f64; 3] = std::array::from_fn(|k| v[k].v * s[k].2);
        let (tw, th) = (tex.w as f64, tex.h as f64);
        for y in miny..=maxy {
            let py = y as f64 + 0.5;
            for x in minx..=maxx {
                let px = x as f64 + 0.5;
                let w0 = ((s[1].0 - px) * (s[2].1 - py) - (s[2].0 - px) * (s[1].1 - py)) * inv;
                let w1 = ((s[2].0 - px) * (s[0].1 - py) - (s[0].0 - px) * (s[2].1 - py)) * inv;
                let w2 = 1.0 - w0 - w1;
                if w0 < -1e-9 || w1 < -1e-9 || w2 < -1e-9 {
                    continue;
                }
                let iz = w0 * s[0].2 + w1 * s[1].2 + w2 * s[2].2;
                let z = 1.0 / iz;
                let i = (y as usize - self.y0) * self.w + x as usize;
                if z as f32 >= self.depth[i] {
                    continue;
                }
                let u = (w0 * uz[0] + w1 * uz[1] + w2 * uz[2]) * z;
                let vv = (w0 * vz[0] + w1 * vz[1] + w2 * vz[2]) * z;
                let tx = ((u * tw) as i64).clamp(0, tex.w as i64 - 1) as usize;
                let ty = ((vv * th) as i64).clamp(0, tex.h as i64 - 1) as usize;
                let (r, g, b) = rgb(tex.px[ty * tex.w + tx]);
                let c = pack(r * light, g * light, b * light);
                let ft = ((z - fog.start) / (fog.end - fog.start)).clamp(0.0, 1.0);
                self.color[i] = fog_mix(c, fog.color, ft);
                self.depth[i] = z as f32;
            }
        }
    }

}

/// Cuts a polygon (camera space) to the part in front of the near plane.
fn clip_near(poly: &[Vert; 4]) -> ([Vert; 8], usize) {
    let mut out = [poly[0]; 8];
    let mut n = 0;
    for i in 0..4 {
        let a = poly[i];
        let b = poly[(i + 1) % 4];
        let (ina, inb) = (a.c.z > NEAR, b.c.z > NEAR);
        if ina {
            out[n] = a;
            n += 1;
        }
        if ina != inb {
            let t = (NEAR - a.c.z) / (b.c.z - a.c.z);
            out[n] = Vert { c: a.c.lerp(b.c, t), u: a.u + (b.u - a.u) * t, v: a.v + (b.v - a.v) * t };
            n += 1;
        }
    }
    (out, n)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_quad_facing_the_camera_is_drawn_and_depth_tested() {
        let mut f = Frame::new(64, 64);
        f.clear_sky(0xff000000, 0xff000000);
        let cam = Camera { pos: v3(0.0, 0.0, -5.0), target: v3(0.0, 0.0, 0.0), fov: 60.0 };
        let view = View::new(&cam, 64, 64);
        let tex = Tex { w: 1, h: 1, px: vec![0xffffff] };
        let fog = Fog { color: 0, start: 100.0, end: 200.0 };
        // a square at z=0 facing the camera (-z), wound so it's a front face
        let q = [v3(-1.0, 1.0, 0.0), v3(1.0, 1.0, 0.0), v3(1.0, -1.0, 0.0), v3(-1.0, -1.0, 0.0)];
        f.quad(&view, q, &tex, 1.0, &fog);
        let back = [q[3], q[2], q[1], q[0]];
        let lit = f.color.iter().filter(|c| **c & 0xffffff != 0).count();
        assert!(lit > 100, "front face drawn ({lit} px)");
        let mut g = Frame::new(64, 64);
        g.quad(&view, back, &tex, 1.0, &fog);
        assert!(g.color.iter().all(|c| *c & 0xffffff == 0), "back face culled");
        // a sphere in front of the quad wins the depth test
        f.sphere(&view, v3(0.0, 0.0, -1.5), 0.5, v3(0.0, 0.0, -1.0), &|_, _| 0xffff0000, &fog);
        assert_eq!(f.color[32 * 64 + 32] & 0xffffff, 0xff0000);
    }
}
