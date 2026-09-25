//! The Monster form in the 3D arena: the real 3D model (icons/monster.lvm - its mesh and, for each move, the
//! skeleton's joint matrices baked at 30 fps), skinned and drawn every frame, wearing the verity's own colours
//! and pattern (icons/pets/clean/<slug>.png, read where each vertex projects onto the ball).
//! Moves: Idle (loops), Attack, Special, Hit, Faint.

use crate::gfx::r3d::{Fog, Frame, V3, View, v3};
use crate::gfx::load_png_bytes;
use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;

pub struct Move {
    frames: usize,
    fps: f64,
    /// frames x joints x (3x4 row-major)
    data: Vec<f32>,
}

pub struct Model {
    pos: Vec<[f32; 3]>,
    uv: Vec<[f32; 2]>,
    ball_uv: Vec<[f32; 2]>,
    joints: Vec<[u8; 4]>,
    weights: Vec<[f32; 4]>,
    tris: Vec<[u32; 3]>,
    nb: usize,
    moves: HashMap<String, Move>,
    /// the skin texture (512x512): 0xKKRRGGBB, KK=1 for texels that keep their own colour (teeth, gums, eyes)
    skin: Vec<u32>,
    skin_w: usize,
    /// vertices at the top of the head (their average = where the head is)
    head: Vec<usize>,
}

fn rd_u32(b: &[u8], o: &mut usize) -> u32 {
    let v = u32::from_le_bytes(b[*o..*o + 4].try_into().unwrap());
    *o += 4;
    v
}
fn rd_f32(b: &[u8], o: &mut usize) -> f32 {
    let v = f32::from_le_bytes(b[*o..*o + 4].try_into().unwrap());
    *o += 4;
    v
}

fn load() -> Option<Model> {
    let b = crate::assets::read("icons/monster.lvm")?;
    let b: &[u8] = &b;
    if b.len() < 16 || &b[0..4] != b"LVM1" {
        return None;
    }
    let mut o = 4;
    let nv = rd_u32(b, &mut o) as usize;
    let nt = rd_u32(b, &mut o) as usize;
    let nb = rd_u32(b, &mut o) as usize;
    let (mut pos, mut uv, mut ball_uv, mut joints, mut weights) = (Vec::with_capacity(nv), Vec::with_capacity(nv), Vec::with_capacity(nv), Vec::with_capacity(nv), Vec::with_capacity(nv));
    for _ in 0..nv {
        pos.push([rd_f32(b, &mut o), rd_f32(b, &mut o), rd_f32(b, &mut o)]);
        uv.push([rd_f32(b, &mut o), rd_f32(b, &mut o)]);
        ball_uv.push([rd_f32(b, &mut o), rd_f32(b, &mut o)]);
        joints.push([b[o], b[o + 1], b[o + 2], b[o + 3]]);
        weights.push([b[o + 4] as f32 / 255.0, b[o + 5] as f32 / 255.0, b[o + 6] as f32 / 255.0, b[o + 7] as f32 / 255.0]);
        o += 8;
    }
    let mut tris = Vec::with_capacity(nt);
    for _ in 0..nt {
        tris.push([rd_u32(b, &mut o), rd_u32(b, &mut o), rd_u32(b, &mut o)]);
    }
    let nm = rd_u32(b, &mut o) as usize;
    let mut moves = HashMap::new();
    for _ in 0..nm {
        let l = b[o] as usize;
        o += 1;
        let name = String::from_utf8_lossy(&b[o..o + l]).to_string();
        o += l;
        let frames = rd_u32(b, &mut o) as usize;
        let fps = rd_f32(b, &mut o) as f64;
        let n = frames * nb * 12;
        let data: Vec<f32> = (0..n).map(|_| rd_f32(b, &mut o)).collect();
        moves.insert(name, Move { frames, fps, data });
    }
    // the skin texture, with the texels that keep their own colour marked
    let img = crate::assets::read("icons/monster_skin.png").and_then(|b| load_png_bytes(&b))?;
    let skin: Vec<u32> = img
        .px
        .iter()
        .map(|p| {
            let (r, g, bl) = (((p >> 16) & 255) as f32, ((p >> 8) & 255) as f32, (p & 255) as f32);
            let mx = r.max(g).max(bl);
            let mn = r.min(g).min(bl);
            let sat = if mx > 0.0 { (mx - mn) / mx } else { 0.0 };
            let keep = (sat < 0.15 && mx > 185.0) || (r > g * 1.9 && r > bl * 1.5 && r > 110.0);
            (p & 0x00ff_ffff) | if keep { 0x0100_0000 } else { 0 }
        })
        .collect();
    let mut m = Model { pos, uv, ball_uv, joints, weights, tris, nb, moves, skin, skin_w: img.w as usize, head: Vec::new() };
    // the head: the top 6% of the idle pose
    let idle = m.skinned("Idle", 0.0);
    m.head = (0..idle.len()).filter(|&i| idle[i].y > 0.94).collect();
    Some(m)
}

thread_local! {
    static MODEL: RefCell<Option<Option<Rc<Model>>>> = const { RefCell::new(None) };
    static COLORS: RefCell<HashMap<usize, Rc<Vec<u32>>>> = RefCell::new(HashMap::new());
}

pub fn model() -> Option<Rc<Model>> {
    MODEL.with(|m| m.borrow_mut().get_or_insert_with(|| load().map(Rc::new)).clone())
}

impl Model {
    pub fn duration(&self, name: &str) -> f64 {
        self.moves.get(name).map(|m| (m.frames.max(1) - 1) as f64 / m.fps).unwrap_or(0.0)
    }

    /// The joints' matrices at time t of a move (blended between the two nearest baked frames).
    fn joints_at(&self, name: &str, t: f64) -> Vec<[f32; 12]> {
        let Some(mv) = self.moves.get(name).or_else(|| self.moves.get("Idle")) else { return vec![[1., 0., 0., 0., 0., 1., 0., 0., 0., 0., 1., 0.]; self.nb] };
        let last = mv.frames.max(1) - 1;
        let f = (t * mv.fps).max(0.0);
        let looping = matches!(name, "Idle" | "Walk" | "Wave");
        let (fa, k) = if looping { ((f as usize) % last.max(1), f.fract() as f32) } else { ((f as usize).min(last), if f as usize >= last { 0.0 } else { f.fract() as f32 }) };
        let fb = (fa + 1).min(last);
        (0..self.nb)
            .map(|j| {
                let a = &mv.data[(fa * self.nb + j) * 12..(fa * self.nb + j) * 12 + 12];
                let b = &mv.data[(fb * self.nb + j) * 12..(fb * self.nb + j) * 12 + 12];
                std::array::from_fn(|i| a[i] + (b[i] - a[i]) * k)
            })
            .collect()
    }

    /// Every vertex posed at time t of a move, in the model's own space (feet on y=0, 1 tall, facing +z).
    pub fn skinned(&self, name: &str, t: f64) -> Vec<V3> {
        let js = self.joints_at(name, t);
        let n = self.pos.len();
        let mut out = vec![V3::default(); n];
        let threads = std::thread::available_parallelism().map(|n| n.get()).unwrap_or(1).clamp(1, 8);
        let chunk = n.div_ceil(threads);
        std::thread::scope(|scope| {
            for (ci, part) in out.chunks_mut(chunk).enumerate() {
                let js = &js;
                scope.spawn(move || {
                    for (k, o) in part.iter_mut().enumerate() {
                        let i = ci * chunk + k;
                        let [x, y, z] = self.pos[i];
                        let (mut px, mut py, mut pz) = (0.0f32, 0.0f32, 0.0f32);
                        for s in 0..4 {
                            let w = self.weights[i][s];
                            if w <= 0.0 {
                                continue;
                            }
                            let m = &js[self.joints[i][s] as usize];
                            px += w * (m[0] * x + m[1] * y + m[2] * z + m[3]);
                            py += w * (m[4] * x + m[5] * y + m[6] * z + m[7]);
                            pz += w * (m[8] * x + m[9] * y + m[10] * z + m[11]);
                        }
                        *o = v3(px as f64, py as f64, pz as f64);
                    }
                });
            }
        });
        out
    }
}

/// Each vertex's colour for this verity: its ball (without the face), read where the vertex projects onto it.
fn vertex_colors(model: &Model, slug_pet: usize) -> Rc<Vec<u32>> {
    COLORS.with(|c| {
        let mut c = c.borrow_mut();
        if c.len() > 6 {
            c.clear();
        }
        c.entry(slug_pet)
            .or_insert_with(|| {
                let name = crate::core::data::rarities()[slug_pet].pet;
                let slug = crate::ui::icons::pet_slug(name);
                // the OG Verity is the model exactly as it was made: its own texture, no recolouring
                if slug == "verity" {
                    return Rc::new(vec![ORIGINAL; model.ball_uv.len()]);
                }
                let img = crate::assets::read(&format!("icons/pets/clean/{}.png", slug)).and_then(|b| load_png_bytes(&b));
                Rc::new(
                    model
                        .ball_uv
                        .iter()
                        .map(|[u, v]| match &img {
                            Some(img) => {
                                let x = ((*u * img.w as f32) as i32).clamp(0, img.w - 1);
                                let y = ((*v * img.h as f32) as i32).clamp(0, img.h - 1);
                                img.px[(y * img.w + x) as usize] & 0x00ff_ffff
                            }
                            None => 0x00e8_d27a,
                        })
                        .collect(),
                )
            })
            .clone()
    })
}

/// A vertex colour meaning "show the skin texture as it is".
pub const ORIGINAL: u32 = 0x0100_0000;

/// Where a monster is and what it's doing.
pub struct MonsterDraw {
    pub pet: usize,
    /// feet position
    pub at: V3,
    pub facing: V3,
    pub height: f64,
    pub move_name: &'static str,
    pub move_t: f64,
    pub alpha: f64,
    pub flash: f64,
}

/// Draws the monster; returns the middle of its head (world), for the accessories and the damage numbers.
pub fn draw_monster(fr: &mut Frame, view: &View, fog: &Fog, d: &MonsterDraw) -> Option<V3> {
    let model = model()?;
    let local = model.skinned(d.move_name, d.move_t);
    let colors = vertex_colors(&model, d.pet);
    // model space (facing +z) -> world: turn to face `facing`, scale, stand at `at`
    let f = v3(d.facing.x, 0.0, d.facing.z).norm();
    let (sn, cs) = (f.x, f.z);
    let world: Vec<V3> = local.iter().map(|p| v3(p.x * cs + p.z * sn, p.y, -p.x * sn + p.z * cs) * d.height + d.at).collect();
    let head = if model.head.is_empty() {
        None
    } else {
        let s = model.head.iter().fold(V3::default(), |a, &i| a + world[i]);
        Some(s * (1.0 / model.head.len() as f64))
    };
    fr.skinned_mesh(view, &world, &model.uv, &colors, &model.tris, &model.skin, model.skin_w, fog, d.alpha, d.flash);
    head
}
