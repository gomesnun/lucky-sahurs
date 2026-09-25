//! The 3D battle arena: a blocky (Minecraft-like) world, the two verities facing each other, a cinematic camera that
//! changes shot with what's happening, and the effects (charging, projectiles, impacts, shockwaves, shields...).
//! Everything is worked out from the battle page's clock and event times, so there's no state to keep in sync:
//! draw_arena_3d() is a picture of "the battle at time t". It uses the software renderer in gfx/r3d.rs.

use crate::core::data::rarities;
use crate::gfx::r3d::{Camera, Fog, Frame, Tex, UP, V3, View, v3};
use crate::gfx::{Surf, Surface};
use crate::ui::drawing::hsv_to_rgb;
use super::monster3d::{MonsterDraw, draw_monster};
use crate::ui::icons::load_pet_phase_image;
use std::cell::RefCell;
use std::collections::HashMap;
use std::f64::consts::PI;
use std::rc::Rc;

// ---------------------------------------------------------------- what the page tells the arena
/// What's on the field for one side.
#[derive(Clone, Copy, PartialEq)]
pub struct Shown {
    pub pet: usize,
    pub m: &'static str,
    pub phase: usize,
}

/// The camera's current shot (chosen by the event playing).
#[derive(Clone, Copy, PartialEq, Debug)]
pub enum Shot {
    Idle,
    SendOut(usize),
    /// side attacks (true = Special)
    Attack(usize, bool),
    /// side gets hit
    Impact(usize),
    Guard(usize),
    Heal(usize),
    Faint(usize),
    /// side transforms into its Monster form
    Transform(usize),
}

pub struct Stage {
    pub t: f64,
    pub shown: [Option<Shown>; 2],
    pub send_t0: [f64; 2],
    pub lunge_t0: [f64; 2],
    pub lunge_special: [bool; 2],
    pub hit_t0: [f64; 2],
    pub hit_crit: [bool; 2],
    pub heal_t0: [f64; 2],
    pub guard_t0: [f64; 2],
    pub faint_t0: [Option<f64>; 2],
    pub transform_t0: [f64; 2],
    pub calm_t0: [f64; 2],
    pub boost_t0: [f64; 2],
    pub boost_color: [(f64, f64, f64); 2],
    pub shot: Shot,
    pub shot_t0: f64,
    pub prev_shot: Shot,
}

/// Screen positions (arena pixels) of what the page draws on top: each verity's head.
pub struct Marks {
    pub head: [Option<(f64, f64)>; 2],
}

// ---------------------------------------------------------------- timing (the page uses these for its events)
pub const STRIKE_ARRIVE: f64 = 0.7;
pub const SPECIAL_ARRIVE: f64 = 1.25;
pub const SEND_FALL: f64 = 0.6;
/// the transformation: power gathers until the flash, then the Monster bursts out and roars
pub const TRANSFORM_FLASH: f64 = 2.0;
pub const TRANSFORM_TIME: f64 = 3.4;

// ---------------------------------------------------------------- the world
const PLATFORM_Y: f64 = 2.0;
const BALL_R: f64 = 0.95;
const MONSTER_H: f64 = 4.0;

/// Where each side stands (on its grass platform) and which way it faces.
fn base(side: usize) -> V3 {
    if side == 0 { v3(-5.5, PLATFORM_Y, 0.5) } else { v3(5.5, PLATFORM_Y, 0.5) }
}
fn facing(side: usize) -> V3 {
    if side == 0 { v3(1.0, 0.0, 0.0) } else { v3(-1.0, 0.0, 0.0) }
}
/// toward the default camera side
const SIDEWAYS: V3 = v3(0.0, 0.0, 1.0);

#[derive(Clone, Copy, PartialEq)]
enum B {
    Air,
    Grass,
    Dirt,
    Stone,
    Log,
    Leaves,
    Cloud,
}

struct Face {
    corners: [V3; 4],
    tex: usize,
    light: f64,
}

struct World {
    faces: Vec<Face>,
    texes: Vec<Tex>,
}

pub fn hash(a: i64, b: i64, c: i64) -> f64 {
    let mut h = (a.wrapping_mul(374761393) ^ b.wrapping_mul(668265263) ^ c.wrapping_mul(2147483647)) as u64;
    h = (h ^ (h >> 13)).wrapping_mul(1274126177);
    h ^= h >> 16;
    (h % 10000) as f64 / 10000.0
}

pub fn tex_from(f: impl Fn(usize, usize) -> (f64, f64, f64)) -> Tex {
    let mut px = vec![0u32; 256];
    for y in 0..16 {
        for x in 0..16 {
            let (r, g, b) = f(x, y);
            px[y * 16 + x] = ((r.clamp(0.0, 255.0) as u32) << 16) | ((g.clamp(0.0, 255.0) as u32) << 8) | (b.clamp(0.0, 255.0) as u32);
        }
    }
    Tex { w: 16, h: 16, px }
}

fn textures() -> Vec<Tex> {
    let n = |x: usize, y: usize, s: i64| hash(x as i64, y as i64, s) - 0.5;
    let grass = (96.0, 168.0, 58.0);
    let dirt = (134.0, 96.0, 67.0);
    vec![
        // 0 grass top
        tex_from(|x, y| {
            let k = 1.0 + n(x, y, 1) * 0.28;
            (grass.0 * k, grass.1 * k, grass.2 * k)
        }),
        // 1 grass side: dirt with a ragged green top
        tex_from(|x, y| {
            let edge = 3 + (hash(x as i64, 0, 2) * 2.4) as usize;
            let k = 1.0 + n(x, y, 3) * 0.3;
            if y < edge { (grass.0 * k * 0.95, grass.1 * k * 0.95, grass.2 * k) } else { (dirt.0 * k, dirt.1 * k, dirt.2 * k) }
        }),
        // 2 dirt
        tex_from(|x, y| {
            let k = 1.0 + n(x, y, 4) * 0.32;
            (dirt.0 * k, dirt.1 * k, dirt.2 * k)
        }),
        // 3 stone
        tex_from(|x, y| {
            let k = 128.0 + n(x, y, 5) * 40.0 + if hash(x as i64 / 3, y as i64 / 2, 6) > 0.8 { -22.0 } else { 0.0 };
            (k, k, k + 3.0)
        }),
        // 4 log (bark)
        tex_from(|x, y| {
            let k = 1.0 + n(x, y / 3, 7) * 0.25 + if x % 4 == 0 { -0.18 } else { 0.0 };
            (104.0 * k, 80.0 * k, 50.0 * k)
        }),
        // 5 leaves
        tex_from(|x, y| {
            let h = hash(x as i64, y as i64, 8);
            let k = if h < 0.12 { 0.55 } else { 0.85 + h * 0.35 };
            (58.0 * k, 128.0 * k, 44.0 * k)
        }),
        // 6 cloud
        tex_from(|_, _| (250.0, 252.0, 255.0)),
    ]
}

fn build_world() -> World {
    const X0: i64 = -22;
    const X1: i64 = 22;
    const Z0: i64 = -22;
    const Z1: i64 = 14;
    const H: i64 = 22;
    let (nx, nz) = ((X1 - X0) as usize, (Z1 - Z0) as usize);
    let idx = |x: i64, y: i64, z: i64| ((y as usize) * nz + (z - Z0) as usize) * nx + (x - X0) as usize;
    let mut v = vec![B::Air; nx * nz * H as usize];
    let smooth = |x: i64, z: i64| -> f64 {
        // value noise, two octaves
        let s = |x: f64, z: f64, sc: f64, seed: i64| {
            let (fx, fz) = (x / sc, z / sc);
            let (ix, iz) = (fx.floor() as i64, fz.floor() as i64);
            let (tx, tz) = (fx - ix as f64, fz - iz as f64);
            let (tx, tz) = (tx * tx * (3.0 - 2.0 * tx), tz * tz * (3.0 - 2.0 * tz));
            let a = hash(ix, iz, seed) + (hash(ix + 1, iz, seed) - hash(ix, iz, seed)) * tx;
            let b = hash(ix, iz + 1, seed) + (hash(ix + 1, iz + 1, seed) - hash(ix, iz + 1, seed)) * tx;
            a + (b - a) * tz
        };
        s(x as f64, z as f64, 7.0, 11) * 0.7 + s(x as f64, z as f64, 3.0, 12) * 0.3
    };
    let mut height = HashMap::new();
    for x in X0..X1 {
        for z in Z0..Z1 {
            // flat arena in the middle, hills around it (higher behind, lower in front of the camera)
            let dx = (x.abs() as f64 - 10.0).max(0.0);
            let dz = if z < 0 { (-z as f64 - 5.0).max(0.0) } else { (z as f64 - 7.0).max(0.0) * 0.5 };
            let d = (dx * dx + dz * dz).sqrt();
            let hills = if d > 0.0 { (d * 0.45 + smooth(x, z) * 3.5 - 0.5).max(0.0) } else { 0.0 };
            let h = (1.0 + hills).floor().min(9.0) as i64;
            height.insert((x, z), h);
            for y in 0..h {
                v[idx(x, y, z)] = if y == h - 1 { B::Grass } else if y >= h - 3 { B::Dirt } else { B::Stone };
            }
        }
    }
    // the two platforms (3x3, one block up)
    for side in 0..2 {
        let b = base(side);
        let cx = (b.x - 0.5).round() as i64;
        let cz = (b.z - 0.5).round() as i64;
        for x in cx - 1..=cx + 1 {
            for z in cz - 1..=cz + 1 {
                v[idx(x, 0, z)] = B::Dirt;
                v[idx(x, 1, z)] = B::Grass;
            }
        }
    }
    // oak trees on the hills
    let mut trees = 0;
    for x in X0 + 2..X1 - 2 {
        for z in Z0 + 2..Z1 - 2 {
            let h = height[&(x, z)];
            // trees stay behind the arena and far to the sides (the camera flies in front)
            let far = z < -5 || x.abs() > 15;
            if !far || h < 2 || hash(x, z, 21) > 0.045 || trees > 14 {
                continue;
            }
            trees += 1;
            let top = h + 4;
            for y in h..top.min(H - 1) {
                v[idx(x, y, z)] = B::Log;
            }
            for y in top - 2..(top + 2).min(H) {
                let r = if y >= top { 1 } else { 2 };
                for lx in x - r..=x + r {
                    for lz in z - r..=z + r {
                        if lx < X0 || lx >= X1 || lz < Z0 || lz >= Z1 {
                            continue;
                        }
                        let corner = (lx - x).abs() == r && (lz - z).abs() == r;
                        if v[idx(lx, y, lz)] == B::Air && !(corner && hash(lx, y, lz) < 0.6) {
                            v[idx(lx, y, lz)] = B::Leaves;
                        }
                    }
                }
            }
        }
    }
    // clouds: flat slabs high up
    for (cx, cz, w, d) in [(-14i64, -18i64, 7i64, 4i64), (3, -20, 9, 5), (15, -12, 6, 4), (-4, -9, 5, 3)] {
        for x in cx..cx + w {
            for z in cz..cz + d {
                if x >= X0 && x < X1 && z >= Z0 && z < Z1 && hash(x, z, 31) > 0.12 {
                    v[idx(x, H - 1, z)] = B::Cloud;
                }
            }
        }
    }
    // faces where a block meets air
    let get = |x: i64, y: i64, z: i64| -> B {
        if x < X0 || x >= X1 || z < Z0 || z >= Z1 || y < 0 || y >= H { B::Air } else { v[idx(x, y, z)] }
    };
    let dirs: [(i64, i64, i64); 5] = [(0, 1, 0), (1, 0, 0), (-1, 0, 0), (0, 0, 1), (0, 0, -1)];
    let mut faces = Vec::new();
    for y in 0..H {
        for z in Z0..Z1 {
            for x in X0..X1 {
                let b = get(x, y, z);
                if b == B::Air {
                    continue;
                }
                let dirs_here: Vec<(i64, i64, i64)> = if b == B::Cloud { vec![(0, -1, 0), (1, 0, 0), (-1, 0, 0), (0, 0, 1), (0, 0, -1)] } else { dirs.to_vec() };
                for (dx, dy, dz) in dirs_here {
                    let nb = get(x + dx, y + dy, z + dz);
                    if nb != B::Air && !(nb == B::Leaves && b != B::Leaves) && !(nb == B::Cloud && b != B::Cloud) {
                        continue;
                    }
                    // the edge of the world facing away from the camera is never seen
                    if dz == -1 && z == Z0 {
                        continue;
                    }
                    let n = v3(dx as f64, dy as f64, dz as f64);
                    let c = v3(x as f64 + 0.5, y as f64 + 0.5, z as f64 + 0.5) + n * 0.5;
                    // (rt, up) span the face with rt x up = n; texture top = up
                    let (rt, upv) = if dy == 1 { (v3(1.0, 0.0, 0.0), v3(0.0, 0.0, -1.0)) } else if dy == -1 { (v3(1.0, 0.0, 0.0), v3(0.0, 0.0, 1.0)) } else { (UP.cross(n), UP) };
                    let tl = c - rt * 0.5 + upv * 0.5;
                    let bl = c - rt * 0.5 - upv * 0.5;
                    let br = c + rt * 0.5 - upv * 0.5;
                    let tr = c + rt * 0.5 + upv * 0.5;
                    let tex = match (b, dy) {
                        (B::Grass, 1) => 0,
                        (B::Grass, _) => 1,
                        (B::Dirt, _) => 2,
                        (B::Stone, _) => 3,
                        (B::Log, _) => 4,
                        (B::Leaves, _) => 5,
                        _ => 6,
                    };
                    let light = match (dx, dy, dz) {
                        (_, 1, _) => 1.0,
                        (_, -1, _) => 0.9,
                        (1, _, _) | (-1, _, _) => 0.78,
                        _ => 0.66,
                    };
                    faces.push(Face { corners: [tl, bl, br, tr], tex, light });
                }
            }
        }
    }
    World { faces, texes: textures() }
}

thread_local! {
    static WORLD: RefCell<Option<Rc<World>>> = const { RefCell::new(None) };
    static ACCESSORIES: RefCell<HashMap<(usize, usize), Option<Surf>>> = RefCell::new(HashMap::new());
}

fn world() -> Rc<World> {
    WORLD.with(|w| w.borrow_mut().get_or_insert_with(|| Rc::new(build_world())).clone())
}

const IMG: i32 = 256;

/// The verity image without its ball: its halo, wings, horns, rings... (they float with the 3D ball).
fn accessories(s: Shown) -> Option<Surf> {
    ACCESSORIES.with(|c| {
        c.borrow_mut()
            .entry((s.pet, s.phase))
            .or_insert_with(|| {
                let img = load_pet_phase_image(rarities()[s.pet].pet, s.phase, IMG, false)?;
                let mut out = (*img).clone();
                let (cx, cy, r) = (IMG as f64 * 0.5, IMG as f64 * 0.542, IMG as f64 * 0.28);
                for y in 0..IMG {
                    for x in 0..IMG {
                        let d = ((x as f64 + 0.5 - cx).powi(2) + (y as f64 + 0.5 - cy).powi(2)).sqrt() / r;
                        let i = (y * IMG + x) as usize;
                        let p = out.px[i];
                        let keep = ((d - 1.04) / 0.06).clamp(0.0, 1.0);
                        let a = (((p >> 24) & 255) as f64 * keep) as u32;
                        out.px[i] = (p & 0x00ff_ffff) | (a << 24);
                    }
                }
                Some(Rc::new(out))
            })
            .clone()
    })
}

/// The colour a verity's effects glow in (its special's colour).
fn fx_color(s: Shown, t: f64) -> (f64, f64, f64) {
    if s.phase >= 3 {
        return (255.0, 60.0, 50.0);
    }
    match s.m {
        "golden" => (255.0, 205.0, 60.0),
        "diamond" => (130.0, 225.0, 255.0),
        "rainbow" => {
            let (r, g, b) = hsv_to_rgb((t * 0.6).fract(), 0.75, 1.0);
            (r * 255.0, g * 255.0, b * 255.0)
        }
        _ => match s.phase {
            2 => (120.0, 150.0, 255.0),
            1 => (200.0, 90.0, 255.0),
            _ => (110.0, 255.0, 140.0),
        },
    }
}

fn ease(t: f64) -> f64 {
    let t = t.clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

// ---------------------------------------------------------------- where things are at time t
struct Pose {
    /// ball centre, or the monster's feet
    pos: V3,
    alpha: f64,
    flash: f64,
    visible: bool,
}

fn fighter_pose(st: &Stage, side: usize) -> Pose {
    let t = st.t;
    let Some(s) = st.shown[side] else { return Pose { pos: base(side), alpha: 0.0, flash: 0.0, visible: false } };
    let monster = s.phase >= 3;
    let fwd = facing(side);
    let mut p = base(side) + if monster { v3(0.0, 0.0, 0.0) } else { v3(0.0, BALL_R, 0.0) };
    // idle bob
    p.y += (t * 2.4 + side as f64 * 1.3).sin().abs() * 0.14;
    // transforming: the ball rises, shaking, until the flash
    let dt = t - st.transform_t0[side];
    if !monster && (0.0..TRANSFORM_FLASH).contains(&dt) {
        let k = ease(dt / TRANSFORM_FLASH);
        p.y += k * 1.6;
        p = p + v3((t * 70.0).sin(), 0.0, (t * 63.0).cos()) * (0.06 * k);
    }
    // dropping in from the sky
    let ds = t - st.send_t0[side];
    if ds < SEND_FALL {
        let k = (ds / SEND_FALL).clamp(0.0, 1.0);
        p.y += (1.0 - k * k) * 10.0;
    }
    // attacking
    let dl = t - st.lunge_t0[side];
    let dash = !st.lunge_special[side];
    if monster && st.lunge_special[side] && (0.85..2.0).contains(&dl) {
        // a monster's Special: it roars in place (0-0.85), then dashes in and slashes
        let dist = (base(1).x - base(0).x).abs() - 2.6;
        let off = if dl < SPECIAL_ARRIVE { dist * ease((dl - 0.85) / (SPECIAL_ARRIVE - 0.85)) } else if dl < 1.5 { dist } else { dist * (1.0 - ease((dl - 1.5) / 0.5)) };
        p = p + fwd * off;
    } else if dash && (0.0..1.5).contains(&dl) {
        let dist = (base(1).x - base(0).x).abs() - 2.0;
        let off = if dl < 0.25 {
            -0.7 * ease(dl / 0.25)
        } else if dl < STRIKE_ARRIVE {
            -0.7 + (dist + 0.7) * ease((dl - 0.25) / (STRIKE_ARRIVE - 0.25))
        } else if dl < 0.95 {
            dist
        } else {
            dist * (1.0 - ease((dl - 0.95) / 0.5))
        };
        p = p + fwd * off;
        if (0.25..STRIKE_ARRIVE).contains(&dl) && !monster {
            p.y += ((dl - 0.25) / (STRIKE_ARRIVE - 0.25) * PI).sin() * 1.3;
        }
    } else if !dash && !monster && (0.0..1.8).contains(&dl) {
        // charging: rises a little, then recoils as it fires
        let up = if dl < 0.8 { ease(dl / 0.8) } else { 1.0 - ease((dl - 0.8) / 0.8) };
        p.y += up * 0.5;
        if (0.8..1.1).contains(&dl) {
            p = p - fwd * (((dl - 0.8) / 0.3 * PI).sin() * 0.4);
        }
    }
    // hit: knocked back
    let dh = t - st.hit_t0[side];
    let mut flash = 0.0;
    if (0.0..1.2).contains(&dh) {
        let kb = if dh < 0.1 { dh / 0.1 } else { (-(dh - 0.1) * 5.0).exp() };
        p = p - fwd * (kb * if st.hit_crit[side] { 1.3 } else { 0.8 });
        p.y += kb * 0.25;
        flash = (1.0 - dh / 0.2).max(0.0);
    }
    // fainted: sinks and fades
    let mut alpha = 1.0;
    if let (Some(f0), true) = (st.faint_t0[side], monster) {
        let k = ((t - f0 - 1.3) / 0.5).clamp(0.0, 1.0);
        if k >= 1.0 {
            return Pose { pos: p, alpha: 0.0, flash: 0.0, visible: false };
        }
        return Pose { pos: p, alpha: 1.0 - k, flash, visible: true };
    }
    if let Some(f0) = st.faint_t0[side] {
        let k = ((t - f0) / 1.1).clamp(0.0, 1.0);
        p.y -= ease(k) * 2.4;
        alpha = 1.0 - k;
        if k >= 1.0 {
            return Pose { pos: p, alpha: 0.0, flash: 0.0, visible: false };
        }
    }
    Pose { pos: p, alpha, flash, visible: true }
}

/// Which of the monster's animations plays now, and how far into it.
fn monster_move(st: &Stage, side: usize) -> (&'static str, f64) {
    let t = st.t;
    if let Some(f0) = st.faint_t0[side] {
        return ("Faint", t - f0);
    }
    let dt = t - st.transform_t0[side] - TRANSFORM_FLASH;
    if (0.0..1.4).contains(&dt) {
        return ("Special", 0.15 + dt * 0.5);
    }
    let dh = t - st.hit_t0[side];
    if (0.0..0.6).contains(&dh) {
        return ("Hit", dh);
    }
    let dl = t - st.lunge_t0[side];
    if st.lunge_special[side] {
        if (0.0..0.85).contains(&dl) {
            return ("Special", dl);
        }
        if (0.85..1.5).contains(&dl) {
            return ("Attack", 0.35 + (dl - 0.85));
        }
    } else if (0.0..1.0).contains(&dl) {
        return ("Attack", dl);
    }
    ("Idle", t + side as f64 * 1.3)
}

/// The middle of a verity (what effects aim at).
fn center_of(st: &Stage, side: usize) -> V3 {
    let pose = fighter_pose(st, side);
    let monster = st.shown[side].is_some_and(|s| s.phase >= 3);
    if monster { pose.pos + v3(0.0, MONSTER_H * 0.55, 0.0) } else { pose.pos }
}

// ---------------------------------------------------------------- the camera
/// Is this side a Monster (4 blocks tall: the camera stands further back)?
fn tall(st: &Stage, side: usize) -> bool {
    st.shown[side].is_some_and(|s| s.phase >= 3)
}

fn shot_camera(st: &Stage, shot: Shot, since: f64) -> Camera {
    let mid = (base(0) + base(1)) * 0.5 + v3(0.0, 1.2, 0.0);
    match shot {
        Shot::Idle => {
            // Pokemon style: behind your verity (near, bottom left), looking at the rival (far, top right)
            let sway = (st.t * 0.21).sin();
            let look = base(0).lerp(base(1), 0.62) + v3(0.0, 1.0, 0.0);
            let back = if tall(st, 0) { 9.5 } else { 6.5 };
            let high = if tall(st, 0) { 5.2 } else { 3.6 };
            let pos = base(0) - facing(0) * back + SIDEWAYS * (6.8 + sway * 0.8) + v3(0.0, high + (st.t * 0.15).sin() * 0.3, 0.0);
            let _ = mid;
            Camera { pos, target: look, fov: 46.0 }
        }
        Shot::SendOut(s) => {
            let b = base(s);
            let rise = ease(since / 1.2) * 0.8;
            let k = if tall(st, s) { 1.7 } else { 1.0 };
            Camera { pos: b + facing(s) * 5.0 * k + SIDEWAYS * 3.8 * k + v3(0.0, 2.0 * k + rise, 0.0), target: b + v3(0.0, 1.3 * k, 0.0), fov: 52.0 }
        }
        Shot::Attack(a, special) => {
            let b = 1 - a;
            let push = ease(since / if special { 1.25 } else { 0.7 }) * 1.4;
            let (back, high, out) = if tall(st, a) { (8.0, 5.4, 4.0) } else { (4.2, 2.6, 2.8) };
            let from = base(a) - facing(a) * (back - push) + SIDEWAYS * out + v3(0.0, high, 0.0);
            Camera { pos: from, target: base(b) + v3(0.0, 1.1, 0.0), fov: 54.0 }
        }
        Shot::Impact(b) => {
            let spin = since * 0.35;
            let k = if tall(st, b) { 1.7 } else { 1.0 };
            let pos = base(b) + facing(b) * (4.6 * k - spin) + SIDEWAYS * (2.6 * k + spin) + v3(0.0, 1.7 * k, 0.0);
            Camera { pos, target: base(b) + v3(0.0, 1.0 * k, 0.0), fov: 50.0 }
        }
        Shot::Guard(s) | Shot::Heal(s) => {
            let a = since * 0.5;
            let k = if tall(st, s) { 1.8 } else { 1.0 };
            let pos = base(s) + facing(s) * (3.4 * k * a.cos()) + SIDEWAYS * ((2.4 + a.sin() * 1.5) * k) + v3(0.0, 1.7 * k, 0.0);
            Camera { pos, target: base(s) + v3(0.0, 1.1 * k, 0.0), fov: 50.0 }
        }
        Shot::Transform(s) => {
            // a close, low orbit around it while the power builds; pulls back when the Monster bursts out
            let a = since * 0.9;
            let out = ease((since - TRANSFORM_FLASH) / 0.5);
            let r = 4.2 + out * 6.0;
            let h = 1.2 + ease(since / TRANSFORM_FLASH) * 1.4 + out * 2.6;
            let around = facing(s) * a.cos() + SIDEWAYS * a.sin().abs().max(0.35);
            let target = base(s) + v3(0.0, 1.6 + out * 1.8, 0.0);
            Camera { pos: base(s) + around.norm() * r + v3(0.0, h, 0.0), target, fov: 50.0 - 8.0 * ease(since / TRANSFORM_FLASH) + 10.0 * out }
        }
        Shot::Faint(s) => {
            let k = if tall(st, s) { 1.8 } else { 1.0 };
            let pos = base(s) + facing(s) * (4.5 * k - ease(since / 1.4)) + SIDEWAYS * 1.6 * k + v3(0.0, 0.6 * k, 0.0);
            Camera { pos, target: base(s) + v3(0.0, 0.7 * k, 0.0), fov: 48.0 }
        }
    }
}

fn camera(st: &Stage) -> Camera {
    let since = st.t - st.shot_t0;
    let cur = shot_camera(st, st.shot, since);
    let prev = shot_camera(st, st.prev_shot, since + 2.0);
    // impacts cut hard; everything else glides
    let blend = if matches!(st.shot, Shot::Impact(_)) { 0.12 } else if matches!(st.shot, Shot::Transform(_)) { 0.35 } else { 0.55 };
    let k = ease(since / blend);
    let mut cam = Camera { pos: prev.pos.lerp(cur.pos, k), target: prev.target.lerp(cur.target, k), fov: prev.fov + (cur.fov - prev.fov) * k };
    // a big shake when a Monster bursts out
    for side in 0..2 {
        let d = st.t - st.transform_t0[side] - TRANSFORM_FLASH;
        if (0.0..1.0).contains(&d) {
            let amp = (-d * 4.0).exp() * 0.6;
            let sh = v3((st.t * 57.0).sin(), (st.t * 49.0 + 1.0).sin(), (st.t * 43.0 + 2.0).sin()) * amp;
            cam.pos = cam.pos + sh;
            cam.target = cam.target + sh * 0.6;
        }
    }
    // screen shake after hits
    for side in 0..2 {
        let dh = st.t - st.hit_t0[side];
        if (0.0..0.8).contains(&dh) {
            let amp = (-dh * 6.0).exp() * if st.hit_crit[side] { 0.45 } else { 0.25 };
            let s = v3((st.t * 53.0).sin(), (st.t * 47.0 + 1.0).sin(), (st.t * 41.0 + 2.0).sin()) * amp;
            cam.pos = cam.pos + s;
            cam.target = cam.target + s * 0.6;
        }
    }
    cam
}

// ---------------------------------------------------------------- drawing
const SKY_TOP: u32 = 0xff5c95f0;
const SKY_LOW: u32 = 0xffb4d4ff;

/// Draws the arena at time t into a w x h surface (the page scales it up), and says where things ended up.
pub fn draw_arena_3d(st: &Stage, w: usize, h: usize) -> (Surface, Marks) {
    let world = world();
    let mut fr = Frame::new(w, h);
    fr.clear_sky(SKY_TOP, SKY_LOW);
    let cam = camera(st);
    let view = View::new(&cam, w, h);
    let fog = Fog { color: SKY_LOW & 0xffffff, start: 18.0, end: 40.0 };
    // the sun: a square, far away
    if let Some((sx, sy, _)) = view.project(cam.pos + v3(-0.35, 0.55, -1.0).norm() * 200.0) {
        let r = (h as f64 * 0.045).max(3.0);
        for y in (sy - r).max(0.0) as usize..((sy + r).min(h as f64)) as usize {
            for x in (sx - r).max(0.0) as usize..((sx + r).min(w as f64)) as usize {
                let edge = (x as f64 - (sx - r)).min(sx + r - x as f64).min(y as f64 - (sy - r)).min(sy + r - y as f64) < r * 0.22;
                fr.color[y * w + x] = if edge { 0xfffff0b0 } else { 0xfffffde0 };
            }
        }
    }
    // the blocks, on all the CPU cores (each draws a band of rows)
    let threads = std::thread::available_parallelism().map(|n| n.get()).unwrap_or(1).clamp(1, 8);
    let wref: &World = &world;
    std::thread::scope(|scope| {
        for mut band in fr.bands(h.div_ceil(threads)) {
            let (view, fog) = (&view, &fog);
            scope.spawn(move || {
                for f in &wref.faces {
                    band.quad(view, f.corners, &wref.texes[f.tex], f.light, fog);
                }
            });
        }
    });
    // the verities
    let mut marks = Marks { head: [None, None] };
    for side in 0..2 {
        let Some(s) = st.shown[side] else { continue };
        let pose = fighter_pose(st, side);
        if !pose.visible {
            continue;
        }
        let fwd = facing(side);
        if s.phase >= 3 {
            // the real 3D monster, animated, in the verity's colours; its accessories float at its head
            let (move_name, move_t) = monster_move(st, side);
            let d = MonsterDraw { pet: s.pet, at: pose.pos, facing: fwd, height: MONSTER_H, move_name, move_t, alpha: pose.alpha, flash: pose.flash };
            let head = draw_monster(&mut fr, &view, &fog, &d).unwrap_or(pose.pos + v3(0.0, MONSTER_H * 0.93, 0.0));
            if let Some(acc) = accessories(Shown { phase: 0, ..s }) {
                let head_d = MONSTER_H * 0.14;
                fr.sprite(&view, &acc, head, (0.5, 0.542), head_d * 2.6 / 0.56, pose.alpha, 0.0, false, &fog);
            }
            marks.head[side] = view.project(head + v3(0.0, 0.8, 0.0)).map(|(x, y, _)| (x, y));
        } else {
            let spin = {
                let d = st.t - st.transform_t0[side];
                if (0.0..TRANSFORM_FLASH).contains(&d) { d * d * 5.0 } else { 0.0 }
            };
            let face = v3(fwd.x * spin.cos() - fwd.z * spin.sin(), 0.0, fwd.x * spin.sin() + fwd.z * spin.cos());
            draw_pet_ball(&mut fr, &view, &fog, s, pose.pos, BALL_R, face, pose.flash, pose.alpha);
            marks.head[side] = view.project(pose.pos + v3(0.0, BALL_R + 0.5, 0.0)).map(|(x, y, _)| (x, y));
        }
    }
    draw_effects(st, &mut fr, &view);
    (fr.to_surface(), marks)
}

/// A verity as a 3D ball: its picture on the front, the colour at its edge on the back, lit and rimmed, with its
/// accessories (halo, wings, horns...) floating around it. Also used by Explore (game/explore.rs).
#[allow(clippy::too_many_arguments)]
pub fn draw_pet_ball(fr: &mut Frame, view: &View, fog: &Fog, s: Shown, pos: V3, radius: f64, face: V3, flash: f64, alpha: f64) {
    let Some(img) = load_pet_phase_image(rarities()[s.pet].pet, s.phase, IMG, false) else { return };
    let img: &Surface = &img;
    let light_dir = v3(-0.4, 0.8, 0.45).norm();
    let sample = |u: f64, v: f64| -> u32 {
        let x = ((u * IMG as f64) as i32).clamp(0, IMG - 1);
        let y = ((v * IMG as f64) as i32).clamp(0, IMG - 1);
        img.px[(y * IMG + x) as usize]
    };
    let eye = view.pos;
    let shade = move |n: V3, l: V3| -> u32 {
        // the front half wears the verity's picture; the back takes the colour at the ball's edge
        let (u, v) = if l.z > 0.0 {
            (0.5 + l.x * 0.28, 0.542 - l.y * 0.28)
        } else {
            let d = (l.x * l.x + l.y * l.y).sqrt().max(1e-6);
            (0.5 + l.x / d * 0.25, 0.542 - l.y / d * 0.25)
        };
        let p = sample(u, v);
        let lit = 0.72 + 0.28 * n.dot(light_dir).max(0.0) + if l.z > 0.0 { 0.0 } else { -0.1 };
        let rim = (1.0 - n.dot((eye - pos).norm()).abs()).powi(3) * 60.0;
        let ch = |sh: u32| {
            let c = ((p >> sh) & 255) as f64 * lit + rim;
            c + (255.0 - c) * flash
        };
        0xff00_0000 | ((ch(16).clamp(0.0, 255.0) as u32) << 16) | ((ch(8).clamp(0.0, 255.0) as u32) << 8) | ch(0).clamp(0.0, 255.0) as u32
    };
    fr.sphere(view, pos, radius, face, &shade, fog);
    if let Some(acc) = accessories(s) {
        fr.sprite(view, &acc, pos, (0.5, 0.542), radius * 2.0 / 0.56, alpha, 0.0, false, fog);
    }
}

/// A stable pseudo-random direction for particle i.
fn dir(i: usize, seed: i64) -> V3 {
    let a = hash(i as i64, seed, 1) * 2.0 * PI;
    let y = hash(i as i64, seed, 2) * 2.0 - 1.0;
    let r = (1.0 - y * y).sqrt();
    v3(r * a.cos(), y, r * a.sin())
}

fn draw_effects(st: &Stage, fr: &mut Frame, view: &View) {
    let t = st.t;
    for a in 0..2 {
        let Some(sa) = st.shown[a] else { continue };
        let b = 1 - a;
        let col = fx_color(sa, t);
        let ca = center_of(st, a);
        let dl = t - st.lunge_t0[a];
        let monster = sa.phase >= 3;
        // ---- a Special: charge up, then fire
        if st.lunge_special[a] && (0.0..SPECIAL_ARRIVE + 0.1).contains(&dl) {
            let target = base(b) + v3(0.0, if st.shown[b].is_some_and(|s| s.phase >= 3) { MONSTER_H * 0.55 } else { BALL_R }, 0.0);
            if dl < 0.85 {
                // energy pulled in from all around, and a ring spinning around it
                for i in 0..80 {
                    let p = ((dl - i as f64 * 0.007) / 0.55).clamp(0.0, 1.0);
                    if p <= 0.0 || p >= 1.0 {
                        continue;
                    }
                    let pos = ca + dir(i, 5) * (3.2 * (1.0 - p));
                    fr.glow(view, pos, 0.1, col, 1.1 * p);
                }
                for i in 0..10 {
                    let ang = dl * 9.0 + i as f64 * 2.0 * PI / 10.0;
                    let pos = ca + v3(ang.cos() * 1.4, (dl * 3.0 + i as f64).sin() * 0.4, ang.sin() * 1.4);
                    fr.glow(view, pos, 0.12, col, 0.8);
                }
                fr.glow(view, ca, 0.5 + dl * 0.6, col, 0.35 * ease(dl / 0.85));
            } else if !monster {
                let p = ((dl - 0.85) / (SPECIAL_ARRIVE - 0.85)).clamp(0.0, 1.0);
                let lanes: &[f64] = if sa.m == "diamond" { &[-0.45, 0.0, 0.45] } else { &[0.0] };
                for (li, lane) in lanes.iter().enumerate() {
                    let off = v3(0.0, *lane, *lane);
                    let head = ca.lerp(target, p) + off + v3(0.0, (p * PI).sin() * 0.8, 0.0);
                    fr.glow(view, head, 0.6, col, 1.5);
                    fr.glow(view, head, 0.25, (255.0, 255.0, 255.0), 1.2);
                    fr.glow(view, head, 1.4, col, 0.4);
                    for k in 1..24 {
                        let q = p - k as f64 * 0.018;
                        if q < 0.0 {
                            break;
                        }
                        let c = if sa.m == "rainbow" { fx_color(sa, t - k as f64 * 0.06) } else { col };
                        let pt = ca.lerp(target, q) + off + v3(0.0, (q * PI).sin() * 0.8, 0.0) + dir(k + li * 30, 6) * 0.08;
                        fr.glow(view, pt, 0.26 * (1.0 - k as f64 / 24.0), c, 0.9 * (1.0 - k as f64 / 24.0));
                    }
                    // the golden beam leaves a line behind it
                    if sa.m == "golden" {
                        for k in 0..40 {
                            let q = p * k as f64 / 40.0;
                            fr.glow(view, ca.lerp(target, q) + v3(0.0, (q * PI).sin() * 0.8, 0.0), 0.12, col, 0.5);
                        }
                    }
                }
            }
        }
        // ---- a monster's rampage: a trail of red flames behind the dash
        let (d0, d1) = if st.lunge_special[a] { (0.85, SPECIAL_ARRIVE + 0.1) } else { (0.25, STRIKE_ARRIVE + 0.1) };
        if monster && (d0..d1).contains(&dl) {
            for k in 0..20 {
                let pos = ca - facing(a) * (k as f64 * 0.35) + dir(k, 7) * 0.3;
                fr.glow(view, pos, 0.3 * (1.0 - k as f64 / 20.0), (255.0, 80.0 + k as f64 * 6.0, 40.0), 0.8);
            }
        }
        // ---- getting hit: sparks, a shockwave on the ground, a flash
        let dh = t - st.hit_t0[b];
        if (0.0..1.0).contains(&dh) && st.shown[b].is_some() {
            let cb = center_of(st, b);
            let special = st.lunge_special[a];
            let c = if special || monster { col } else { (255.0, 240.0, 200.0) };
            let n = if st.hit_crit[b] { 110 } else { 70 };
            for i in 0..n {
                let d = dir(i, 9 + (st.hit_t0[b] * 10.0) as i64);
                let d = v3(d.x, d.y.abs() * 0.8 + 0.2, d.z);
                let speed = 3.0 + hash(i as i64, 3, 3) * 5.0;
                let pos = cb + d * (dh * speed) + v3(0.0, -4.5 * dh * dh, 0.0);
                let life = 1.0 - dh / (0.55 + hash(i as i64, 4, 4) * 0.4);
                if life > 0.0 {
                    fr.glow(view, pos, 0.1 + 0.12 * life, if i % 3 == 0 { (255.0, 255.0, 255.0) } else { c }, life * 1.2);
                }
            }
            let ground = base(b) + v3(0.0, 0.08, 0.0);
            let rad = 0.4 + dh * 6.0;
            for i in 0..36 {
                let ang = i as f64 / 36.0 * 2.0 * PI;
                fr.glow(view, ground + v3(ang.cos() * rad, 0.0, ang.sin() * rad), 0.24, c, (1.0 - dh) * 1.1);
                fr.glow(view, ground + v3(ang.cos() * rad * 0.6, 0.3 + dh, ang.sin() * rad * 0.6), 0.16, (255.0, 255.0, 255.0), (1.0 - dh) * 0.6);
            }
            fr.glow(view, cb, 1.6 + dh * 3.0, c, (1.4 - dh * 2.2).max(0.0));
            fr.glow(view, cb, 0.6, (255.0, 255.0, 255.0), (1.0 - dh * 4.0).max(0.0) * 1.5);
        }
    }
    for s in 0..2 {
        let Some(sh) = st.shown[s] else { continue };
        let c = center_of(st, s);
        let b0 = base(s);
        let red = (255.0, 60.0, 40.0);
        // ---- transforming
        let dt = t - st.transform_t0[s];
        if (0.0..TRANSFORM_FLASH).contains(&dt) {
            let k = ease(dt / TRANSFORM_FLASH);
            // a pillar of light
            for i in 0..34 {
                let y = i as f64 * 0.35;
                let wob = (t * 9.0 + i as f64).sin() * 0.08;
                fr.glow(view, b0 + v3(wob, y, -wob), 0.35 + 0.25 * k, (255.0, 120.0, 90.0), 0.25 + 0.5 * k);
            }
            // red energy spiralling in
            for i in 0..90 {
                let p = ((dt * 1.3 + hash(i as i64, 21, 1)) % 1.0).clamp(0.0, 1.0);
                let ang = hash(i as i64, 22, 2) * 2.0 * PI + p * 7.0;
                let r = 4.5 * (1.0 - p);
                let pos = c + v3(ang.cos() * r, (hash(i as i64, 23, 3) - 0.5) * 3.0 * (1.0 - p), ang.sin() * r);
                fr.glow(view, pos, 0.1, if i % 4 == 0 { (255.0, 230.0, 200.0) } else { red }, 0.9 * p * (0.3 + k));
            }
            // rings pulsing out on the ground
            for pulse in 0..3 {
                let q = (dt * 1.5 + pulse as f64 / 3.0) % 1.0;
                for i in 0..30 {
                    let ang = i as f64 / 30.0 * 2.0 * PI;
                    fr.glow(view, b0 + v3(ang.cos() * (0.5 + q * 4.0), 0.1, ang.sin() * (0.5 + q * 4.0)), 0.16, red, (1.0 - q) * 0.9);
                }
            }
            fr.glow(view, c, 0.6 + k * 1.6, (255.0, 200.0, 170.0), 0.3 + k);
        }
        let da = dt - TRANSFORM_FLASH;
        if (0.0..1.6).contains(&da) {
            // the burst: a blast of red and white shards, and a huge shockwave
            let mc = b0 + v3(0.0, MONSTER_H * 0.55, 0.0);
            for i in 0..140 {
                let d = dir(i, 31);
                let pos = mc + d * (da * (5.0 + hash(i as i64, 32, 1) * 7.0)) + v3(0.0, -3.0 * da * da, 0.0);
                let life = 1.0 - da / (0.8 + hash(i as i64, 33, 2) * 0.8);
                if life > 0.0 {
                    fr.glow(view, pos, 0.14 + 0.1 * life, if i % 3 == 0 { (255.0, 255.0, 255.0) } else { red }, life * 1.2);
                }
            }
            for ring in 0..2 {
                let r = 0.5 + da * (9.0 + ring as f64 * 4.0);
                for i in 0..48 {
                    let ang = i as f64 / 48.0 * 2.0 * PI;
                    fr.glow(view, b0 + v3(ang.cos() * r, 0.15 + ring as f64 * 0.6, ang.sin() * r), 0.3, if ring == 0 { red } else { (255.0, 200.0, 120.0) }, (1.0 - da / 1.6) * 1.1);
                }
            }
            fr.glow(view, mc, 3.0 + da * 4.0, red, (1.0 - da * 1.2).max(0.0) * 1.2);
        }
        // ---- a raging Monster burns with a red aura
        if sh.phase >= 3 && st.faint_t0[s].is_none() && da >= 0.0 {
            let pose = fighter_pose(st, s);
            for i in 0..26 {
                let q = (t * 1.1 + hash(i as i64, 41, 1)) % 1.0;
                let ang = hash(i as i64, 42, 2) * 2.0 * PI;
                let r = 0.7 + hash(i as i64, 43, 3) * 0.5;
                let pos = pose.pos + v3(ang.cos() * r, q * MONSTER_H, ang.sin() * r);
                fr.glow(view, pos, 0.18 * (1.0 - q) + 0.05, (255.0, 70.0 + 120.0 * q, 30.0), (1.0 - q) * 0.7);
            }
        }
        // ---- calming down: a puff of smoke
        let dc = t - st.calm_t0[s];
        if (0.0..1.2).contains(&dc) {
            for i in 0..40 {
                let d = dir(i, 51);
                fr.glow(view, c + d * (0.6 + dc * 2.2) + v3(0.0, dc, 0.0), 0.4, (180.0, 170.0, 180.0), (1.0 - dc / 1.2) * 0.45);
            }
        }
        // ---- a boost item: sparkles of its colour rising around it
        let db = t - st.boost_t0[s];
        if (0.0..1.0).contains(&db) {
            let col = st.boost_color[s];
            for i in 0..34 {
                let ang = hash(i as i64, 61, 1) * 2.0 * PI + db * 3.0;
                let r = 0.9 + hash(i as i64, 62, 2) * 0.6;
                let rise = ((db * 1.6 + hash(i as i64, 63, 3)) % 1.0) * 3.2 - 1.0;
                fr.glow(view, c + v3(ang.cos() * r, rise, ang.sin() * r), 0.12, col, (1.0 - db) * 1.1);
            }
            for i in 0..28 {
                let ang = i as f64 / 28.0 * 2.0 * PI;
                let r = 0.4 + db * 2.5;
                fr.glow(view, b0 + v3(ang.cos() * r, 0.1, ang.sin() * r), 0.15, col, (1.0 - db) * 0.9);
            }
        }
        // ---- landing: a ring of dust
        let dd = t - st.send_t0[s] - SEND_FALL;
        if (0.0..0.9).contains(&dd) {
            for i in 0..28 {
                let ang = i as f64 / 28.0 * 2.0 * PI;
                let r = 0.6 + dd * 3.5;
                fr.glow(view, base(s) + v3(ang.cos() * r, 0.15 + dd * 0.6 * hash(i as i64, 5, 5), ang.sin() * r), 0.2, (190.0, 160.0, 120.0), (1.0 - dd / 0.9) * 0.6);
            }
            fr.glow(view, c, 1.5, fx_color(sh, t), (0.9 - dd) * 0.6);
        }
        // ---- Guard: a shield bubble
        let dg = t - st.guard_t0[s];
        if (0.0..1.1).contains(&dg) {
            let k = (1.0 - dg / 1.1) * ease(dg / 0.15);
            for i in 0..70 {
                let d = dir(i, 11);
                let pulse = 1.0 + (t * 6.0 + i as f64).sin() * 0.04;
                fr.glow(view, c + d * (1.65 * pulse), 0.07, (120.0, 220.0, 255.0), k * 0.9);
            }
            fr.glow(view, c, 2.0, (90.0, 180.0, 255.0), k * 0.25);
        }
        // ---- Rest: green sparkles rising
        let dr = t - st.heal_t0[s];
        if (0.0..1.2).contains(&dr) {
            for i in 0..26 {
                let ang = hash(i as i64, 6, 6) * 2.0 * PI;
                let r = 0.6 + hash(i as i64, 7, 7) * 0.8;
                let rise = ((dr * 1.8 + hash(i as i64, 8, 8)) % 1.0) * 3.0 - 1.0;
                fr.glow(view, c + v3(ang.cos() * r, rise, ang.sin() * r), 0.1, (120.0, 255.0, 150.0), (1.0 - dr / 1.2) * 0.9);
            }
        }
        // ---- fainting: puffs of smoke
        if let Some(f0) = st.faint_t0[s] {
            let df = t - f0;
            if (0.0..1.4).contains(&df) {
                for i in 0..22 {
                    let d = dir(i, 13);
                    let pos = base(s) + v3(d.x * (0.5 + df * 1.5), 0.2 + df * 1.6 * hash(i as i64, 9, 9), d.z * (0.5 + df * 1.5));
                    fr.glow(view, pos, 0.35, (170.0, 170.0, 180.0), (1.0 - df / 1.4) * 0.35);
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn world_size() {
        let w = super::build_world();
        eprintln!("faces: {}", w.faces.len());
        assert!(w.faces.len() > 1000);
    }
}
