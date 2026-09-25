//! v4.0 Explore: Steve walks around blocky 3D worlds (seen from above, a little behind him) looking for Verity
//! Pets. Walk up to one and catch it (E, Space, or click it); walking and catching give XP, levels open the next
//! world (Overworld -> Nether -> The End -> End City -> Emerald City: better worlds, better pets) and new clothes
//! for Steve (the Wardrobe). The pets you equip (in the Bag, or here) follow Steve around, dancing.
//! The saved part is core/explore.rs; the worlds are built here (the same software 3D renderer as the battles).

use super::base::Bo;
use super::battle3d::{Shown, draw_pet_ball, hash, tex_from};
use super::{Game, KeyEv, cb};
use crate::config::{TOPBAR_H, VIRTUAL_H};
use crate::core::data::rarities;
use crate::core::explore::{DIMENSIONS, HATS, MAX_EQUIPPED, MAX_PETS, PANTS, SHIRTS, VerityPet, xp_for_level};
use crate::core::formatting::format_number;
use crate::core::state::rand_random;
use crate::gfx::r3d::{Camera, Fog, Frame, Tex, UP, V3, View, v3};
use crate::gfx::{Color, Rect, Surface, draw, transform};
use crate::i18n::tr;
use crate::theme::*;
use crate::tr;
use crate::ui::drawing::{dim_overlay, draw_panel};
use crate::ui::icons::load_pet_image;
use std::cell::RefCell;
use std::collections::{HashMap, VecDeque};
use std::f64::consts::{PI, TAU};
use std::rc::Rc;

// ================================================================ the worlds
/// every world is N x N columns, up to H blocks tall
const N: i64 = 56;
const H: i64 = 22;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum B {
    Air,
    Grass,
    Dirt,
    Stone,
    Log,
    Leaves,
    Water,
    Sand,
    Netherrack,
    Lava,
    Glowstone,
    SoulSand,
    NetherBrick,
    EndStone,
    Obsidian,
    Purpur,
    PurpurPillar,
    EndRod,
    Emerald,
    Gold,
    Quartz,
    Path,
    Crystal,
    Flower,
    Chorus,
}

impl B {
    /// can be seen through (the faces behind it are drawn)
    fn clear(self) -> bool {
        matches!(self, B::Air | B::Leaves | B::Water | B::Lava | B::Flower)
    }
    fn glows(self) -> Option<(f64, f64, f64)> {
        match self {
            B::Lava => Some((255.0, 120.0, 20.0)),
            B::Glowstone => Some((255.0, 215.0, 120.0)),
            B::EndRod => Some((250.0, 240.0, 255.0)),
            B::Crystal => Some((255.0, 120.0, 230.0)),
            _ => None,
        }
    }
    /// Steve can't stand in / on it
    fn blocks_walk(self) -> bool {
        matches!(self, B::Water | B::Lava | B::Log | B::Leaves | B::Crystal | B::EndRod | B::Chorus)
    }
}

struct Face {
    corners: [V3; 4],
    tex: usize,
    light: f64,
    /// the block's column (for culling around Steve)
    x: i64,
    z: i64,
    /// leaves: left out when they'd hide Steve
    leafy: bool,
}

pub struct World {
    faces: Vec<Face>,
    texes: Rc<Vec<Tex>>,
    /// per column: the height Steve stands at on it (the top of its highest block), -1 = nothing (the void)
    top: Vec<i64>,
    /// per column: Steve can go there (and it can be reached from the spawn)
    reach: Vec<bool>,
    /// glowing blocks (their light is drawn around them)
    lights: Vec<(V3, (f64, f64, f64))>,
    spawn: (f64, f64),
    sky: (u32, u32),
    fog: u32,
    /// how bright its faces are (the Nether and the End are darker)
    ambient: f64,
    /// stars in the sky (the End)
    stars: bool,
}

fn col(x: i64, z: i64) -> usize {
    (z * N + x) as usize
}

fn smooth(x: f64, z: f64, sc: f64, seed: i64) -> f64 {
    let (fx, fz) = (x / sc, z / sc);
    let (ix, iz) = (fx.floor() as i64, fz.floor() as i64);
    let (tx, tz) = (fx - ix as f64, fz - iz as f64);
    let (tx, tz) = (tx * tx * (3.0 - 2.0 * tx), tz * tz * (3.0 - 2.0 * tz));
    let a = hash(ix, iz, seed) + (hash(ix + 1, iz, seed) - hash(ix, iz, seed)) * tx;
    let b = hash(ix, iz + 1, seed) + (hash(ix + 1, iz + 1, seed) - hash(ix, iz + 1, seed)) * tx;
    a + (b - a) * tz
}

fn noise(x: i64, z: i64, seed: i64) -> f64 {
    smooth(x as f64, z as f64, 11.0, seed) * 0.7 + smooth(x as f64, z as f64, 4.0, seed + 1) * 0.3
}

fn textures() -> Vec<Tex> {
    let n = |x: usize, y: usize, s: i64| hash(x as i64, y as i64, s) - 0.5;
    let solid = |c: (f64, f64, f64), s: i64, amp: f64| tex_from(move |x, y| {
        let k = 1.0 + n(x, y, s) * amp;
        (c.0 * k, c.1 * k, c.2 * k)
    });
    let grass = (96.0, 168.0, 58.0);
    let dirt = (134.0, 96.0, 67.0);
    vec![
        // 0 grass top
        solid(grass, 1, 0.28),
        // 1 grass side
        tex_from(|x, y| {
            let edge = 3 + (hash(x as i64, 0, 2) * 2.4) as usize;
            let k = 1.0 + n(x, y, 3) * 0.3;
            if y < edge { (grass.0 * k * 0.95, grass.1 * k * 0.95, grass.2 * k) } else { (dirt.0 * k, dirt.1 * k, dirt.2 * k) }
        }),
        // 2 dirt
        solid(dirt, 4, 0.32),
        // 3 stone
        tex_from(|x, y| {
            let k = 128.0 + n(x, y, 5) * 40.0 + if hash(x as i64 / 3, y as i64 / 2, 6) > 0.8 { -22.0 } else { 0.0 };
            (k, k, k + 3.0)
        }),
        // 4 log
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
        // 6 water
        tex_from(|x, y| {
            let wave = ((x as f64 * 0.8 + y as f64 * 0.4).sin() * 0.5 + 0.5) * 0.18;
            let k = 0.9 + n(x, y, 9) * 0.1 + wave;
            (52.0 * k, 110.0 * k, 220.0 * k)
        }),
        // 7 sand
        solid((219.0, 206.0, 152.0), 10, 0.12),
        // 8 netherrack
        tex_from(|x, y| {
            let k = 1.0 + n(x, y, 11) * 0.35 + if hash(x as i64 / 2, y as i64, 12) > 0.82 { -0.25 } else { 0.0 };
            (112.0 * k, 38.0 * k, 38.0 * k)
        }),
        // 9 lava
        tex_from(|x, y| {
            let h = hash(x as i64 / 2, y as i64 / 2, 13);
            let k = 0.85 + h * 0.3;
            (255.0 * k, (110.0 + h * 90.0) * k, 20.0 * k)
        }),
        // 10 glowstone
        tex_from(|x, y| {
            let h = hash(x as i64 / 2, y as i64 / 2, 14);
            if h > 0.55 { (255.0, 236.0, 160.0) } else { (190.0 + h * 60.0, 140.0 + h * 40.0, 70.0) }
        }),
        // 11 soul sand
        tex_from(|x, y| {
            let face = (y % 8 == 3 || y % 8 == 4) && (x % 8 == 2 || x % 8 == 5);
            let k = 1.0 + n(x, y, 15) * 0.25;
            if face { (50.0, 36.0, 28.0) } else { (86.0 * k, 66.0 * k, 52.0 * k) }
        }),
        // 12 nether brick
        tex_from(|x, y| {
            let mortar = y % 4 == 3 || (x + if (y / 4) % 2 == 0 { 0 } else { 4 }) % 8 == 7;
            let k = 1.0 + n(x, y, 16) * 0.2;
            if mortar { (30.0, 14.0, 18.0) } else { (70.0 * k, 32.0 * k, 38.0 * k) }
        }),
        // 13 end stone
        tex_from(|x, y| {
            let k = 1.0 + n(x, y, 17) * 0.14 + if hash(x as i64, y as i64, 18) > 0.9 { -0.12 } else { 0.0 };
            (222.0 * k, 222.0 * k, 160.0 * k)
        }),
        // 14 obsidian
        tex_from(|x, y| {
            let h = hash(x as i64, y as i64, 19);
            let k = 0.7 + h * 0.5;
            (24.0 * k, 16.0 * k, 38.0 * k + if h > 0.9 { 40.0 } else { 0.0 })
        }),
        // 15 purpur block
        tex_from(|x, y| {
            let edge = x % 8 == 0 || y % 8 == 0;
            let k = 1.0 + n(x, y, 20) * 0.12;
            if edge { (140.0, 92.0, 140.0) } else { (170.0 * k, 122.0 * k, 170.0 * k) }
        }),
        // 16 purpur pillar
        tex_from(|x, y| {
            let groove = x % 4 == 0;
            let k = 1.0 + n(x, y, 21) * 0.1;
            let _ = y;
            if groove { (138.0, 96.0, 138.0) } else { (176.0 * k, 130.0 * k, 176.0 * k) }
        }),
        // 17 end rod
        tex_from(|x, _| if (6..10).contains(&x) { (255.0, 250.0, 240.0) } else { (230.0, 220.0, 240.0) }),
        // 18 emerald block
        tex_from(|x, y| {
            let edge = x == 0 || y == 0 || x == 15 || y == 15;
            let shine = (x + y) % 7 == 0;
            let k = 1.0 + n(x, y, 22) * 0.12;
            if edge { (20.0, 120.0, 60.0) } else if shine { (140.0, 255.0, 190.0) } else { (40.0 * k, 205.0 * k, 105.0 * k) }
        }),
        // 19 gold block
        tex_from(|x, y| {
            let edge = x == 0 || y == 0 || x == 15 || y == 15;
            let shine = (x * 2 + y) % 9 == 0;
            let k = 1.0 + n(x, y, 23) * 0.1;
            if edge { (190.0, 140.0, 30.0) } else if shine { (255.0, 250.0, 190.0) } else { (250.0 * k, 205.0 * k, 60.0 * k) }
        }),
        // 20 quartz
        tex_from(|x, y| {
            let k = 1.0 + n(x, y, 24) * 0.05;
            let edge = x == 0 || y == 0;
            if edge { (210.0, 204.0, 198.0) } else { (238.0 * k, 234.0 * k, 228.0 * k) }
        }),
        // 21 path (sand-coloured cobbles)
        tex_from(|x, y| {
            let k = 1.0 + n(x, y, 25) * 0.2 + if hash(x as i64 / 3, y as i64 / 3, 26) > 0.7 { -0.1 } else { 0.0 };
            (188.0 * k, 164.0 * k, 110.0 * k)
        }),
        // 22 end crystal
        tex_from(|x, y| {
            let d = ((x as f64 - 7.5).abs() + (y as f64 - 7.5).abs()) / 15.0;
            (255.0 - d * 80.0, 150.0 - d * 60.0, 240.0)
        }),
        // 23 log top
        tex_from(|x, y| {
            let d = ((x as f64 - 7.5).powi(2) + (y as f64 - 7.5).powi(2)).sqrt();
            let ring = (d as i64) % 3 == 0;
            if d > 6.8 { (104.0, 80.0, 50.0) } else if ring { (150.0, 118.0, 72.0) } else { (180.0, 146.0, 92.0) }
        }),
        // 24 flower: grass with a red poppy
        tex_from(|x, y| {
            let d = ((x as f64 - 7.5).powi(2) + (y as f64 - 7.5).powi(2)).sqrt();
            let k = 1.0 + n(x, y, 27) * 0.28;
            if d < 2.0 { (250.0, 220.0, 60.0) } else if d < 4.5 { (220.0, 40.0, 40.0) } else { (grass.0 * k, grass.1 * k, grass.2 * k) }
        }),
        // 25 chorus plant
        tex_from(|x, y| {
            let k = 1.0 + n(x, y, 28) * 0.25;
            let spot = hash(x as i64 / 3, y as i64 / 3, 29) > 0.75;
            if spot { (200.0, 160.0, 210.0) } else { (104.0 * k, 64.0 * k, 110.0 * k) }
        }),
    ]
}

fn tex_of(b: B, dy: i64) -> usize {
    match (b, dy) {
        (B::Grass, 1) => 0,
        (B::Grass, _) => 1,
        (B::Dirt, _) => 2,
        (B::Stone, _) => 3,
        (B::Log, 1) => 23,
        (B::Log, _) => 4,
        (B::Leaves, _) => 5,
        (B::Water, _) => 6,
        (B::Sand, _) => 7,
        (B::Netherrack, _) => 8,
        (B::Lava, _) => 9,
        (B::Glowstone, _) => 10,
        (B::SoulSand, _) => 11,
        (B::NetherBrick, _) => 12,
        (B::EndStone, _) => 13,
        (B::Obsidian, _) => 14,
        (B::Purpur, _) => 15,
        (B::PurpurPillar, 1) => 15,
        (B::PurpurPillar, _) => 16,
        (B::EndRod, _) => 17,
        (B::Emerald, _) => 18,
        (B::Gold, _) => 19,
        (B::Quartz, _) => 20,
        (B::Path, _) => 21,
        (B::Crystal, _) => 22,
        (B::Flower, 1) => 24,
        (B::Flower, _) => 1,
        (B::Chorus, _) => 25,
        (B::Air, _) => 3,
    }
}

struct Blocks {
    v: Vec<B>,
}

impl Blocks {
    fn new() -> Blocks {
        Blocks { v: vec![B::Air; (N * N * H) as usize] }
    }
    fn get(&self, x: i64, y: i64, z: i64) -> B {
        if !(0..N).contains(&x) || !(0..N).contains(&z) || !(0..H).contains(&y) { B::Air } else { self.v[((y * N + z) * N + x) as usize] }
    }
    fn set(&mut self, x: i64, y: i64, z: i64, b: B) {
        if (0..N).contains(&x) && (0..N).contains(&z) && (0..H).contains(&y) {
            self.v[((y * N + z) * N + x) as usize] = b;
        }
    }
    /// a column of `b` from y0 up to (not including) y1
    fn fill(&mut self, x: i64, z: i64, y0: i64, y1: i64, b: B) {
        for y in y0..y1 {
            self.set(x, y, z, b);
        }
    }
    fn height(&self, x: i64, z: i64) -> i64 {
        (0..H).rev().find(|&y| self.get(x, y, z) != B::Air).map_or(-1, |y| y + 1)
    }
}

/// A block tree (the Overworld).
fn tree(bl: &mut Blocks, x: i64, z: i64, h: i64, tall: i64) {
    let top = h + tall;
    bl.fill(x, z, h, top, B::Log);
    for y in top - 2..top + 2 {
        let r = if y >= top { 1 } else { 2 };
        for lx in x - r..=x + r {
            for lz in z - r..=z + r {
                let corner = (lx - x).abs() == r && (lz - z).abs() == r;
                if bl.get(lx, y, lz) == B::Air && !(corner && hash(lx, y, lz) < 0.6) {
                    bl.set(lx, y, lz, B::Leaves);
                }
            }
        }
    }
}

/// A tower: a w x w box of `wall` from y0 to y1 with `corner` pillars at its corners.
fn tower(bl: &mut Blocks, x0: i64, z0: i64, w: i64, y0: i64, y1: i64, wall: B, corner: B) {
    for x in x0..x0 + w {
        for z in z0..z0 + w {
            let c = (x == x0 || x == x0 + w - 1) && (z == z0 || z == z0 + w - 1);
            bl.fill(x, z, y0, y1, if c { corner } else { wall });
        }
    }
}

fn gen_overworld(bl: &mut Blocks) {
    let c = N / 2;
    for x in 0..N {
        for z in 0..N {
            let d = (((x - c).pow(2) + (z - c).pow(2)) as f64).sqrt();
            let mut h = 3 + (noise(x, z, 101) * 7.0) as i64;
            if d < 5.0 {
                h = 5;
            }
            bl.fill(x, z, 0, h, B::Dirt);
            bl.fill(x, z, 0, (h - 3).max(1), B::Stone);
            if h <= 4 {
                // lakes
                bl.set(x, h - 1, z, B::Sand);
                bl.fill(x, z, h, 5, B::Water);
                if h == 4 && hash(x, z, 104) < 0.5 {
                    bl.set(x, 4, z, B::Air);
                    bl.set(x, 3, z, B::Sand);
                }
            } else if h == 5 && noise(x, z, 102) > 0.62 {
                bl.set(x, h - 1, z, B::Sand);
            } else {
                bl.set(x, h - 1, z, if hash(x, z, 103) < 0.05 { B::Flower } else { B::Grass });
            }
        }
    }
    // a trail across the middle
    for x in 0..N {
        for z in c - 1..=c {
            let h = bl.height(x, z);
            if h > 0 && matches!(bl.get(x, h - 1, z), B::Grass | B::Flower) {
                bl.set(x, h - 1, z, B::Path);
            }
        }
    }
    for x in 2..N - 2 {
        for z in 2..N - 2 {
            let h = bl.height(x, z);
            let d = (((x - c).pow(2) + (z - c).pow(2)) as f64).sqrt();
            if d > 6.0 && matches!(bl.get(x, h - 1, z), B::Grass) && hash(x, z, 105) < 0.035 {
                tree(bl, x, z, h, 4 + (hash(x, z, 106) * 2.0) as i64);
            }
        }
    }
}

fn gen_nether(bl: &mut Blocks) {
    let c = N / 2;
    for x in 0..N {
        for z in 0..N {
            let d = (((x - c).pow(2) + (z - c).pow(2)) as f64).sqrt();
            let mut h = 3 + (noise(x, z, 201) * 8.0) as i64;
            if d < 5.0 {
                h = 6;
            }
            let top = if noise(x, z, 202) > 0.66 { B::SoulSand } else { B::Netherrack };
            bl.fill(x, z, 0, h, B::Netherrack);
            bl.set(x, h - 1, z, top);
            if h <= 5 {
                bl.fill(x, z, h, 5, B::Lava);
                bl.set(x, 4, z, B::Lava);
            }
        }
    }
    // glowstone lumps and netherrack spikes
    for x in 1..N - 1 {
        for z in 1..N - 1 {
            let h = bl.height(x, z);
            let d = (((x - c).pow(2) + (z - c).pow(2)) as f64).sqrt();
            if d > 6.0 && h >= 7 && hash(x, z, 203) < 0.03 {
                bl.fill(x, z, h, h + 2, B::Glowstone);
            } else if d > 6.0 && h >= 6 && hash(x, z, 204) < 0.012 {
                bl.fill(x, z, h, h + 4 + (hash(x, z, 205) * 4.0) as i64, B::Netherrack);
            }
        }
    }
    // a ruined fortress
    let (fx, fz) = (c + 9, c - 16);
    for x in fx..fx + 11 {
        for z in fz..fz + 9 {
            let edge = x == fx || x == fx + 10 || z == fz || z == fz + 8;
            let h = bl.height(x, z).max(5);
            bl.fill(x, z, 0, h, B::NetherBrick);
            if edge && hash(x, z, 206) > 0.25 {
                bl.fill(x, z, h, h + 2 + (hash(x, z, 207) * 3.0) as i64, B::NetherBrick);
            }
        }
    }
}

fn island(bl: &mut Blocks, cx: i64, cz: i64, r: f64, base: i64, seed: i64) {
    for x in 0..N {
        for z in 0..N {
            let d = (((x - cx).pow(2) + (z - cz).pow(2)) as f64).sqrt();
            let edge = r * (0.85 + noise(x, z, seed) * 0.3);
            if d > edge {
                continue;
            }
            let h = base + if d < 5.0 { 0 } else { (noise(x, z, seed + 1) * 2.4) as i64 };
            // islands are thicker in the middle and thin at the edge (seen from above: a floating rock)
            let depth = ((1.0 - d / edge) * 6.0) as i64 + 1;
            bl.fill(x, z, (h - depth).max(0), h, B::EndStone);
        }
    }
}

/// Chorus plants: purple stalks with little branches (the End).
fn chorus(bl: &mut Blocks, seed: i64, count: usize, min_d: f64) {
    let c = N / 2;
    let mut placed = 0;
    for x in 1..N - 1 {
        for z in 1..N - 1 {
            let d = (((x - c).pow(2) + (z - c).pow(2)) as f64).sqrt();
            let h = bl.height(x, z);
            if placed >= count || d < min_d || h <= 0 || bl.get(x, h - 1, z) != B::EndStone || hash(x, z, seed) > 0.02 {
                continue;
            }
            placed += 1;
            let tall = 2 + (hash(x, z, seed + 1) * 4.0) as i64;
            bl.fill(x, z, h, h + tall, B::Chorus);
            let (bx, bz) = if hash(x, z, seed + 2) > 0.5 { (1, 0) } else { (0, 1) };
            bl.set(x + bx, h + tall - 1, z + bz, B::Chorus);
            bl.set(x - bx, h + tall - 2, z - bz, B::Chorus);
        }
    }
}

fn gen_end(bl: &mut Blocks) {
    let c = N / 2;
    island(bl, c, c, 23.0, 5, 301);
    // obsidian pillars in a ring, a crystal on each
    for i in 0..7 {
        let a = i as f64 / 7.0 * TAU + 0.3;
        let (px, pz) = (c + (a.cos() * 14.0) as i64, c + (a.sin() * 14.0) as i64);
        let tall = 9 + (hash(i, 0, 302) * 7.0) as i64;
        let base = bl.height(px, pz).max(4);
        for x in px - 1..=px + 1 {
            for z in pz - 1..=pz + 1 {
                bl.fill(x, z, 0, base + tall, B::Obsidian);
            }
        }
        bl.set(px, base + tall, pz, B::Crystal);
    }
    // small islands out in the void
    for (x, z, r) in [(6, 8, 4.0), (49, 12, 3.5), (8, 47, 3.0), (48, 46, 4.5)] {
        island(bl, x, z, r, 3, 303 + x);
    }
    chorus(bl, 310, 40, 4.0);
}

fn gen_end_city(bl: &mut Blocks) {
    let c = N / 2;
    island(bl, c, c, 26.0, 5, 401);
    // purpur paths out of the middle
    for (dx, dz) in [(1i64, 0i64), (-1, 0), (0, 1), (0, -1)] {
        for s in 0..24 {
            for w in -1..=1 {
                let (x, z) = (c + dx * s + dz.abs() * w, c + dz * s + dx.abs() * w);
                let h = bl.height(x, z);
                if h > 0 {
                    bl.set(x, h - 1, z, B::Purpur);
                }
            }
        }
    }
    // the towers, each with a wider room at the top and end rods on it
    for (i, (tx, tz, w, tall)) in [(c - 16, c - 14, 5, 11), (c + 9, c - 17, 5, 14), (c + 12, c + 6, 5, 9), (c - 15, c + 8, 5, 12), (c - 4, c - 22, 3, 7)].into_iter().enumerate() {
        let base = bl.height(tx + w / 2, tz + w / 2).max(4);
        tower(bl, tx, tz, w, 0, base + tall, B::Purpur, B::PurpurPillar);
        tower(bl, tx - 1, tz - 1, w + 2, base + tall, base + tall + 3, B::Purpur, B::PurpurPillar);
        for (ex, ez) in [(tx - 1, tz - 1), (tx + w, tz - 1), (tx - 1, tz + w), (tx + w, tz + w)] {
            bl.set(ex, base + tall + 3, ez, B::EndRod);
        }
        let _ = i;
    }
    chorus(bl, 410, 25, 5.0);
}

fn gen_emerald_city(bl: &mut Blocks) {
    let c = N / 2;
    for x in 0..N {
        for z in 0..N {
            bl.fill(x, z, 0, 4, B::Stone);
            // roads every 12 blocks, the rest grass
            let road = (x - c).rem_euclid(12) < 3 || (z - c).rem_euclid(12) < 3;
            bl.set(x, 3, z, if road { B::Quartz } else if hash(x, z, 501) < 0.04 { B::Flower } else { B::Grass });
        }
    }
    // the buildings: in each block of the grid, an emerald tower with gold corners on a quartz base
    let mut bx = c + 3 - 36;
    while bx < N {
        let mut bz = c + 3 - 36;
        while bz < N {
            let near = (bx + 4 - c).abs() < 8 && (bz + 4 - c).abs() < 8;
            if !near && hash(bx, bz, 502) > 0.18 {
                let w = 5 + (hash(bx, bz, 503) * 3.0) as i64;
                let (x0, z0) = (bx + (9 - w) / 2, bz + (9 - w) / 2);
                let tall = 5 + (hash(bx, bz, 504) * 10.0) as i64;
                tower(bl, x0, z0, w, 4, 5, B::Quartz, B::Quartz);
                tower(bl, x0, z0, w, 5, 5 + tall, B::Emerald, B::Gold);
                tower(bl, x0 + 1, z0 + 1, w - 2, 5 + tall, 6 + tall, B::Gold, B::Gold);
            } else if !near {
                // a little park: water and a tree
                for x in bx + 2..bx + 7 {
                    for z in bz + 2..bz + 7 {
                        bl.set(x, 3, z, B::Water);
                    }
                }
                tree(bl, bx + 1, bz + 1, 4, 4);
            }
            bz += 12;
        }
        bx += 12;
    }
    // the plaza: a fountain in front of a golden monument with an emerald on top
    let (mx, mz) = (c - 1, c - 6);
    tower(bl, mx - 1, mz - 1, 5, 4, 5, B::Gold, B::Gold);
    tower(bl, mx, mz, 3, 5, 9, B::Quartz, B::Gold);
    tower(bl, mx, mz, 3, 9, 11, B::Emerald, B::Emerald);
    bl.set(mx + 1, 11, mz + 1, B::Glowstone);
    for x in c - 7..c - 3 {
        for z in c + 3..c + 7 {
            bl.set(x, 3, z, B::Water);
        }
    }
    // lamps at the crossings
    for x in (c - 36..N).step_by(12) {
        for z in (c - 36..N).step_by(12) {
            if (0..N).contains(&x) && (0..N).contains(&z) && !(x == c && z == c) {
                bl.fill(x, z, 4, 6, B::Gold);
                bl.set(x, 6, z, B::Glowstone);
            }
        }
    }
}

fn build_world(dim: usize) -> World {
    let mut bl = Blocks::new();
    match DIMENSIONS[dim].key {
        "nether" => gen_nether(&mut bl),
        "end" => gen_end(&mut bl),
        "end_city" => gen_end_city(&mut bl),
        "emerald_city" => gen_emerald_city(&mut bl),
        _ => gen_overworld(&mut bl),
    }
    // where Steve can stand, and what he can reach from the spawn (up one block at a time, down up to three)
    let mut top = vec![-1i64; (N * N) as usize];
    let mut ok = vec![false; (N * N) as usize];
    for x in 0..N {
        for z in 0..N {
            let h = bl.height(x, z);
            top[col(x, z)] = h;
            ok[col(x, z)] = h > 0 && !bl.get(x, h - 1, z).blocks_walk();
        }
    }
    let spawn = (N / 2, N / 2);
    let mut reach = vec![false; (N * N) as usize];
    let mut q = VecDeque::new();
    if ok[col(spawn.0, spawn.1)] {
        reach[col(spawn.0, spawn.1)] = true;
        q.push_back(spawn);
    }
    while let Some((x, z)) = q.pop_front() {
        for (dx, dz) in [(1, 0), (-1, 0), (0, 1), (0, -1)] {
            let (nx, nz) = (x + dx, z + dz);
            if !(0..N).contains(&nx) || !(0..N).contains(&nz) {
                continue;
            }
            let (a, b) = (col(x, z), col(nx, nz));
            if !reach[b] && ok[b] && step_ok(top[a], top[b]) {
                reach[b] = true;
                q.push_back((nx, nz));
            }
        }
    }
    // the faces where a block meets something see-through
    let mut faces = Vec::new();
    let mut lights = Vec::new();
    // every side but the bottom: the camera orbits Steve, so the -z sides show too once it's turned around
    let dirs: [(i64, i64, i64); 5] = [(0, 1, 0), (1, 0, 0), (-1, 0, 0), (0, 0, 1), (0, 0, -1)];
    for y in 0..H {
        for z in 0..N {
            for x in 0..N {
                let b = bl.get(x, y, z);
                if b == B::Air {
                    continue;
                }
                if let Some(glow) = b.glows() {
                    // one light per lava pool block would be too many: every other one
                    if b != B::Lava || (bl.get(x, y + 1, z) == B::Air && (x + z) % 3 == 0) {
                        lights.push((v3(x as f64 + 0.5, y as f64 + 1.1, z as f64 + 0.5), glow));
                    }
                }
                for (dx, dy, dz) in dirs {
                    let nb = bl.get(x + dx, y + dy, z + dz);
                    if !nb.clear() || (nb == b && b != B::Leaves) {
                        continue;
                    }
                    // the ground's sides at the world's edge only show in the void worlds
                    let n = v3(dx as f64, dy as f64, dz as f64);
                    let c = v3(x as f64 + 0.5, y as f64 + 0.5, z as f64 + 0.5) + n * 0.5;
                    let (rt, upv) = if dy == 1 { (v3(1.0, 0.0, 0.0), v3(0.0, 0.0, -1.0)) } else { (UP.cross(n), UP) };
                    // liquids sit a little lower than a full block
                    let drop = if matches!(b, B::Water | B::Lava) && dy == 1 { 0.12 } else { 0.0 };
                    let c = c - v3(0.0, drop, 0.0);
                    let corners = [c - rt * 0.5 + upv * 0.5, c - rt * 0.5 - upv * 0.5, c + rt * 0.5 - upv * 0.5, c + rt * 0.5 + upv * 0.5];
                    let light = if b.glows().is_some() {
                        1.25
                    } else {
                        match (dx, dy) {
                            (_, 1) => 1.0,
                            (1, _) | (-1, _) => 0.8,
                            _ => 0.66,
                        }
                    };
                    faces.push(Face { corners, tex: tex_of(b, dy), light, x, z, leafy: matches!(b, B::Leaves | B::Log) });
                }
            }
        }
    }
    let (sky, fog, ambient, stars) = match DIMENSIONS[dim].key {
        "nether" => ((0xff2a0606, 0xff6a1a10), 0x5a1810, 0.82, false),
        "end" => ((0xff05030c, 0xff1b1030), 0x120a20, 0.8, true),
        "end_city" => ((0xff080414, 0xff2a1640), 0x1c1030, 0.85, true),
        "emerald_city" => ((0xff58c8a8, 0xfff2e6a6), 0xe8ecc0, 1.0, false),
        _ => ((0xff5c95f0, 0xffb4d4ff), 0xb4d4ff, 1.0, false),
    };
    for f in faces.iter_mut() {
        if f.light < 1.2 {
            f.light *= ambient;
        }
    }
    World { faces, texes: Rc::new(textures()), top, reach, lights, spawn: (spawn.0 as f64 + 0.5, spawn.1 as f64 + 0.5), sky, fog, ambient, stars }
}

/// Can Steve step from a column of height a onto one of height b?
fn step_ok(a: i64, b: i64) -> bool {
    b > 0 && b - a <= 1 && a - b <= 3
}

thread_local! {
    static WORLDS: RefCell<HashMap<usize, Rc<World>>> = RefCell::new(HashMap::new());
    static STEVE_TEX: RefCell<HashMap<(usize, usize), Rc<SteveTex>>> = RefCell::new(HashMap::new());
}

fn world(dim: usize) -> Rc<World> {
    WORLDS.with(|w| w.borrow_mut().entry(dim).or_insert_with(|| Rc::new(build_world(dim))).clone())
}

impl World {
    fn ground(&self, x: f64, z: f64) -> f64 {
        let (cx, cz) = (x.floor() as i64, z.floor() as i64);
        if !(0..N).contains(&cx) || !(0..N).contains(&cz) {
            return 0.0;
        }
        self.top[col(cx, cz)].max(0) as f64
    }
    fn can_go(&self, from: (f64, f64), to: (f64, f64)) -> bool {
        let (fx, fz) = (from.0.floor() as i64, from.1.floor() as i64);
        let (tx, tz) = (to.0.floor() as i64, to.1.floor() as i64);
        if !(0..N).contains(&tx) || !(0..N).contains(&tz) {
            return false;
        }
        if (fx, fz) == (tx, tz) {
            return true;
        }
        let b = col(tx, tz);
        let a = if (0..N).contains(&fx) && (0..N).contains(&fz) { self.top[col(fx, fz)] } else { self.top[b] };
        self.reach[b] && step_ok(a, self.top[b])
    }
    /// a random place a pet can be, away from (x, z)
    fn random_spot(&self, away: (f64, f64), min_d: f64) -> (f64, f64) {
        for _ in 0..400 {
            let (x, z) = ((rand_random() * N as f64).floor(), (rand_random() * N as f64).floor());
            let c = col(x as i64, z as i64);
            let d = ((x - away.0).powi(2) + (z - away.1).powi(2)).sqrt();
            if self.reach[c] && d >= min_d {
                return (x + 0.5, z + 0.5);
            }
        }
        self.spawn
    }
}

// ================================================================ Steve
/// Steve's textures for an outfit: one per face of each part (front, back, left, right, top, bottom).
struct SteveTex {
    head: [Tex; 6],
    body: [Tex; 6],
    arm: [Tex; 6],
    leg: [Tex; 6],
}

const SKIN: (f64, f64, f64) = (196.0, 142.0, 108.0);
const HAIR: (f64, f64, f64) = (60.0, 40.0, 24.0);

fn rgb_of(c: u32) -> (f64, f64, f64) {
    (((c >> 16) & 255) as f64, ((c >> 8) & 255) as f64, (c & 255) as f64)
}

fn tex_wh(w: usize, h: usize, f: impl Fn(usize, usize) -> (f64, f64, f64)) -> Tex {
    let mut px = vec![0u32; w * h];
    for y in 0..h {
        for x in 0..w {
            let (r, g, b) = f(x, y);
            px[y * w + x] = ((r.clamp(0.0, 255.0) as u32) << 16) | ((g.clamp(0.0, 255.0) as u32) << 8) | (b.clamp(0.0, 255.0) as u32);
        }
    }
    Tex { w, h, px }
}

fn shade(c: (f64, f64, f64), k: f64) -> (f64, f64, f64) {
    (c.0 * k, c.1 * k, c.2 * k)
}

fn steve_tex(shirt: usize, pants: usize) -> Rc<SteveTex> {
    STEVE_TEX.with(|m| {
        m.borrow_mut()
            .entry((shirt, pants))
            .or_insert_with(|| {
                let sh = rgb_of(SHIRTS[shirt.min(SHIRTS.len() - 1)].1);
                let pa = rgb_of(PANTS[pants.min(PANTS.len() - 1)].1);
                let n = |x: usize, y: usize, s: i64| 1.0 + (hash(x as i64, y as i64, s) - 0.5) * 0.12;
                let face = tex_wh(8, 8, |x, y| match (x, y) {
                    (_, 0) | (_, 1) => shade(HAIR, n(x, y, 1)),
                    (0, 2) | (7, 2) => shade(HAIR, 0.9),
                    (1, 4) | (6, 4) => (250.0, 250.0, 250.0),
                    (2, 4) | (5, 4) => (72.0, 60.0, 170.0),
                    (3, 5) | (4, 5) => shade(SKIN, 0.78),
                    (2..=5, 6) => (110.0, 60.0, 50.0),
                    (1, 6) | (6, 6) => shade(HAIR, 1.1),
                    _ => shade(SKIN, n(x, y, 2)),
                });
                let head_side = tex_wh(8, 8, |x, y| if y < 2 || (y < 3 && x > 4) { shade(HAIR, n(x, y, 3)) } else { shade(SKIN, n(x, y, 4)) });
                let head_back = tex_wh(8, 8, |x, y| if y < 7 { shade(HAIR, n(x, y, 5)) } else { shade(SKIN, 0.9) });
                let hair = tex_wh(8, 8, |x, y| shade(HAIR, n(x, y, 6)));
                let chin = tex_wh(8, 8, |x, y| shade(SKIN, n(x, y, 7) * 0.9));
                let shirt_t = |w: usize, h: usize, s: i64| tex_wh(w, h, move |x, y| shade(sh, n(x, y, s)));
                let body_front = tex_wh(8, 12, |x, y| if y == 0 && (2..6).contains(&x) { shade(SKIN, 0.95) } else { shade(sh, n(x, y, 8)) });
                let arm_side = tex_wh(4, 12, |x, y| if y < 4 { shade(sh, n(x, y, 9)) } else { shade(SKIN, n(x, y, 10)) });
                let hand = tex_wh(4, 4, |x, y| shade(SKIN, n(x, y, 11) * 0.95));
                let leg_side = tex_wh(4, 12, |x, y| if y >= 10 { shade((120.0, 120.0, 126.0), n(x, y, 12)) } else { shade(pa, n(x, y, 13)) });
                let shoe = tex_wh(4, 4, |_, _| (110.0, 110.0, 116.0));
                let pants_top = tex_wh(4, 4, |x, y| shade(pa, n(x, y, 14)));
                Rc::new(SteveTex {
                    head: [face, head_back, head_side.clone(), head_side, hair, chin],
                    body: [body_front, shirt_t(8, 12, 15), shirt_t(4, 12, 16), shirt_t(4, 12, 17), shirt_t(8, 4, 18), shirt_t(8, 4, 19)],
                    arm: [arm_side.clone(), arm_side.clone(), arm_side.clone(), arm_side, shirt_t(4, 4, 20), hand],
                    leg: [leg_side.clone(), leg_side.clone(), leg_side.clone(), leg_side, pants_top, shoe],
                })
            })
            .clone()
    })
}

/// A quad to draw: corners, texture, light.
type Q<'a> = ([V3; 4], &'a Tex, f64);

/// Where a part of a model goes: a point in the part's own space (Minecraft pixels, 16 to a block) -> world.
#[derive(Clone, Copy)]
struct Place {
    at: V3,
    yaw: f64,
    /// size of a pixel, in blocks
    px: f64,
}

impl Place {
    /// `p` in model pixels (x left-right, y up, z forward) -> world
    fn world(&self, p: V3) -> V3 {
        let (s, c) = self.yaw.sin_cos();
        let q = p * self.px;
        v3(q.x * c + q.z * s, q.y, -q.x * s + q.z * c) + self.at
    }
}

/// A box of the model: `center` and `half` size (model pixels), swung by `swing` radians about the x axis at
/// `pivot`; texs = front, back, left, right, top, bottom.
fn model_box<'a>(out: &mut Vec<Q<'a>>, pl: &Place, center: V3, half: V3, pivot: V3, swing: f64, texs: [&'a Tex; 6], light: f64) {
    let (s, c) = swing.sin_cos();
    let rot = |p: V3| {
        let d = p - pivot;
        pivot + v3(d.x, d.y * c - d.z * s, d.y * s + d.z * c)
    };
    // (normal, right, up) with right x up = normal
    let faces: [(V3, V3, V3, usize); 6] = [
        (v3(0.0, 0.0, 1.0), v3(1.0, 0.0, 0.0), UP, 0),
        (v3(0.0, 0.0, -1.0), v3(-1.0, 0.0, 0.0), UP, 1),
        (v3(1.0, 0.0, 0.0), v3(0.0, 0.0, -1.0), UP, 2),
        (v3(-1.0, 0.0, 0.0), v3(0.0, 0.0, 1.0), UP, 3),
        (UP, v3(1.0, 0.0, 0.0), v3(0.0, 0.0, -1.0), 4),
        (v3(0.0, -1.0, 0.0), v3(1.0, 0.0, 0.0), v3(0.0, 0.0, 1.0), 5),
    ];
    let light_dir = v3(-0.4, 0.8, 0.45).norm();
    for (n, rt, up, ti) in faces {
        let ext = |a: V3| (a.x * half.x).abs() + (a.y * half.y).abs() + (a.z * half.z).abs();
        let fc = center + n * ext(n);
        let (r, u) = (rt * ext(rt), up * ext(up));
        let corners = [fc - r + u, fc - r - u, fc + r - u, fc + r + u].map(|p| pl.world(rot(p)));
        let wn = (corners[1] - corners[0]).cross(corners[2] - corners[0]).norm();
        let l = (0.62 + 0.45 * wn.dot(light_dir).max(0.0)) * light;
        out.push((corners, texs[ti], l));
    }
}

pub struct SteveLook {
    pub shirt: usize,
    pub pants: usize,
    pub hat: usize,
}

/// Steve standing at `at` facing yaw (0 = +z), his legs and arms swinging by `walk` (0 = still).
fn draw_steve(fr: &mut Frame, view: &View, fog: &Fog, at: V3, yaw: f64, walk: f64, wave: f64, look: &SteveLook, light: f64) {
    let tx = steve_tex(look.shirt, look.pants);
    let pl = Place { at, yaw, px: 1.8 / 32.0 };
    fn r(t: &[Tex; 6]) -> [&Tex; 6] {
        [&t[0], &t[1], &t[2], &t[3], &t[4], &t[5]]
    }
    let mut qs: Vec<Q> = Vec::new();
    let swing = walk.sin() * 0.7 * walk.min(1.0).max(if walk > 0.0 { 1.0 } else { 0.0 });
    let head_c = v3(0.0, 28.0, 0.0);
    model_box(&mut qs, &pl, head_c, v3(4.0, 4.0, 4.0), head_c, 0.0, r(&tx.head), light);
    model_box(&mut qs, &pl, v3(0.0, 18.0, 0.0), v3(4.0, 6.0, 2.0), v3(0.0, 18.0, 0.0), 0.0, r(&tx.body), light);
    // arms swing against the legs; `wave` lifts the right arm
    model_box(&mut qs, &pl, v3(6.0, 18.0, 0.0), v3(2.0, 6.0, 2.0), v3(6.0, 22.0, 0.0), -swing, r(&tx.arm), light);
    let right_arm = if wave > 0.0 { -2.6 + (wave * 9.0).sin() * 0.35 } else { swing };
    model_box(&mut qs, &pl, v3(-6.0, 18.0, 0.0), v3(2.0, 6.0, 2.0), v3(-6.0, 22.0, 0.0), right_arm, r(&tx.arm), light);
    model_box(&mut qs, &pl, v3(2.0, 6.0, 0.0), v3(2.0, 6.0, 2.0), v3(2.0, 12.0, 0.0), swing, r(&tx.leg), light);
    model_box(&mut qs, &pl, v3(-2.0, 6.0, 0.0), v3(2.0, 6.0, 2.0), v3(-2.0, 12.0, 0.0), -swing, r(&tx.leg), light);
    // the hat
    let hat_tex: Vec<Tex>;
    let hat = HATS.get(look.hat).map_or("None", |h| h.0);
    let solid = |c: (f64, f64, f64)| tex_wh(4, 4, move |x, y| shade(c, 1.0 + (hash(x as i64, y as i64, 77) - 0.5) * 0.15));
    match hat {
        "Cap" => {
            hat_tex = vec![solid((200.0, 40.0, 40.0)), solid((240.0, 240.0, 240.0))];
            let t = &hat_tex[0];
            model_box(&mut qs, &pl, v3(0.0, 32.6, 0.0), v3(4.3, 0.9, 4.3), head_c, 0.0, [t; 6], light);
            model_box(&mut qs, &pl, v3(0.0, 32.0, 5.5), v3(3.6, 0.35, 2.0), head_c, 0.0, [&hat_tex[1]; 6], light);
        }
        "Top hat" => {
            hat_tex = vec![solid((26.0, 24.0, 30.0)), solid((150.0, 30.0, 40.0))];
            model_box(&mut qs, &pl, v3(0.0, 32.4, 0.0), v3(5.5, 0.4, 5.5), head_c, 0.0, [&hat_tex[0]; 6], light);
            model_box(&mut qs, &pl, v3(0.0, 33.2, 0.0), v3(3.6, 0.6, 3.6), head_c, 0.0, [&hat_tex[1]; 6], light);
            model_box(&mut qs, &pl, v3(0.0, 36.0, 0.0), v3(3.5, 2.6, 3.5), head_c, 0.0, [&hat_tex[0]; 6], light);
        }
        "Crown" | "Emerald crown" => {
            let (band, gem) = if hat == "Crown" { ((250.0, 200.0, 50.0), (220.0, 40.0, 60.0)) } else { ((40.0, 205.0, 110.0), (250.0, 210.0, 60.0)) };
            hat_tex = vec![solid(band), solid(gem)];
            let (t, g) = (&hat_tex[0], &hat_tex[1]);
            model_box(&mut qs, &pl, v3(0.0, 32.8, 4.1), v3(4.3, 1.0, 0.3), head_c, 0.0, [t; 6], light);
            model_box(&mut qs, &pl, v3(0.0, 32.8, -4.1), v3(4.3, 1.0, 0.3), head_c, 0.0, [t; 6], light);
            model_box(&mut qs, &pl, v3(4.1, 32.8, 0.0), v3(0.3, 1.0, 3.9), head_c, 0.0, [t; 6], light);
            model_box(&mut qs, &pl, v3(-4.1, 32.8, 0.0), v3(0.3, 1.0, 3.9), head_c, 0.0, [t; 6], light);
            for (x, z) in [(-3.6, 4.1), (0.0, 4.1), (3.6, 4.1), (-3.6, -4.1), (3.6, -4.1), (0.0, -4.1)] {
                model_box(&mut qs, &pl, v3(x, 34.6, z), v3(0.6, 0.9, 0.35), head_c, 0.0, [t; 6], light);
            }
            model_box(&mut qs, &pl, v3(0.0, 32.8, 4.5), v3(0.8, 0.8, 0.2), head_c, 0.0, [g; 6], light * 1.2);
        }
        _ => hat_tex = Vec::new(),
    }
    for (c, t, l) in &qs {
        fr.quad(view, *c, t, *l, fog);
    }
    drop(qs);
    drop(hat_tex);
    if hat == "Halo" {
        for i in 0..18 {
            let a = i as f64 / 18.0 * TAU;
            let p = pl.world(v3(a.cos() * 5.5, 37.0, a.sin() * 5.5));
            fr.glow(view, p, 0.1, (255.0, 230.0, 120.0), 0.9);
        }
    }
    if hat == "Emerald crown" {
        fr.glow(view, pl.world(v3(0.0, 33.0, 4.6)), 0.25, (80.0, 255.0, 150.0), 0.5);
    }
}

// ================================================================ the page's state
/// A pet out in the world, waiting to be caught.
struct Wild {
    pet: VerityPet,
    pos: (f64, f64),
    to: (f64, f64),
    /// when it next picks somewhere to go
    think: f64,
    seed: f64,
    /// Some(t0): being caught since t0 (it flies into Steve)
    caught: Option<f64>,
}

pub struct ExploreUi {
    pub open: bool,
    dim: usize,
    pub pos: (f64, f64),
    y: f64,
    yaw: f64,
    /// the limbs' swing phase and how much they swing (0 still - 1 walking)
    stride: f64,
    moving: f64,
    t: f64,
    /// v4.0.1: the camera orbits Steve by this much (radians); Steve's own facing/movement never depends on it
    pub cam_yaw: f64,
    /// where a click sent Steve, and the pet he's going to catch there
    pub target: Option<(f64, f64)>,
    target_pet: Option<usize>,
    wild: Vec<Wild>,
    respawn: Vec<f64>,
    walked: f64,
    /// travelling to another world: (to, since)
    travel: Option<(usize, f64)>,
    /// "", "wardrobe" or "pets"
    pub overlay: &'static str,
    pub pets_scroll: f64,
    pub pets_max_scroll: f64,
    /// the pet asked to be released (click again to confirm) and until when
    pub release_confirm: Option<(usize, f64)>,
    /// floating texts: text, world position, since, colour
    pops: Vec<(String, V3, f64, Color)>,
    /// the view's rect and camera (for clicks)
    view_rect: Rect,
    view: Option<View>,
}

impl ExploreUi {
    pub fn new() -> ExploreUi {
        ExploreUi {
            open: false,
            dim: 0,
            pos: (N as f64 / 2.0 + 0.5, N as f64 / 2.0 + 0.5),
            y: 5.0,
            yaw: 0.0,
            stride: 0.0,
            moving: 0.0,
            t: 0.0,
            cam_yaw: 0.0,
            target: None,
            target_pet: None,
            wild: Vec::new(),
            respawn: Vec::new(),
            walked: 0.0,
            travel: None,
            overlay: "",
            pets_scroll: 0.0,
            pets_max_scroll: 0.0,
            release_confirm: None,
            pops: Vec::new(),
            view_rect: Rect::new(0, 0, 1, 1),
            view: None,
        }
    }
}

impl Default for ExploreUi {
    fn default() -> Self {
        Self::new()
    }
}

const SPEED: f64 = 5.2;
const CATCH_R: f64 = 1.9;
const CATCH_TIME: f64 = 0.7;
const PET_R: f64 = 0.42;
/// XP per block walked (times the world's XP)
const XP_WALK: f64 = 0.35;

/// Keys held down right now (for walking).
fn held(keys: &[sdl2::keyboard::Scancode]) -> bool {
    unsafe {
        let mut n = 0;
        let p = sdl2::sys::SDL_GetKeyboardState(&mut n);
        if p.is_null() {
            return false;
        }
        let state = std::slice::from_raw_parts(p, n.max(0) as usize);
        keys.iter().any(|k| state.get(*k as usize).is_some_and(|v| *v != 0))
    }
}

fn catch_xp(p: &VerityPet) -> f64 {
    (20.0 + 12.0 * rarities()[p.pet].tier as f64 + 8.0 * p.stars as f64) * DIMENSIONS[p.dim].xp
}

fn stars(n: u8) -> String {
    "*".repeat(n as usize)
}

impl Game {
    // ---------------------------------------------------------------- open / close / travel
    pub fn open_explore(&mut self) {
        let dim = self.state.explore.dim;
        self.explore.open = true;
        self.explore.overlay = "";
        self.enter_world(dim);
    }

    pub fn close_explore(&mut self) {
        self.explore.open = false;
        self.explore.overlay = "";
        self.explore.wild.clear();
        self.state.save();
    }

    pub fn enter_world(&mut self, dim: usize) {
        let w = world(dim);
        let e = &mut self.explore;
        e.dim = dim;
        e.pos = w.spawn;
        e.y = w.ground(w.spawn.0, w.spawn.1);
        e.yaw = 0.0;
        e.target = None;
        e.target_pet = None;
        e.wild.clear();
        e.respawn.clear();
        e.pops.clear();
        for _ in 0..DIMENSIONS[dim].pets {
            let pos = w.random_spot(e.pos, 7.0);
            e.wild.push(Wild { pet: VerityPet::roll(dim, [rand_random(), rand_random(), rand_random(), rand_random()]), pos, to: pos, think: 0.0, seed: rand_random() * 100.0, caught: None });
        }
        self.state.explore.dim = dim;
    }

    pub fn explore_travel(&mut self, dim: usize) {
        if dim == self.explore.dim || self.explore.travel.is_some() {
            return;
        }
        if !self.state.explore.unlocked(dim) {
            self.show_toast(&tr!("%s opens at level %d", tr(DIMENSIONS[dim].name), DIMENSIONS[dim].level), 2.0);
            return;
        }
        self.explore.travel = Some((dim, self.explore.t));
        self.play("rebirth", 0.0);
    }

    // ---------------------------------------------------------------- every frame
    pub fn tick_explore(&mut self, dt: f64) {
        if !self.explore.open || self.battle.wild.is_some() {
            // fighting a wild Verity Pet: Steve, the other pets and the timers all pause until it's resolved
            return;
        }
        self.explore.t += dt;
        let t = self.explore.t;
        // travelling: half the fade out, switch, half back in
        if let Some((to, t0)) = self.explore.travel {
            if t - t0 >= 0.6 && self.explore.dim != to {
                self.enter_world(to);
            }
            if t - t0 >= 1.2 {
                self.explore.travel = None;
            }
        }
        let w = world(self.explore.dim);
        // ---- Steve walks: the keys, or toward where you clicked
        let mut dir = (0.0, 0.0);
        if self.explore.overlay.is_empty() && self.explore.travel.is_none() && !self.index_open {
            use sdl2::keyboard::Scancode as S;
            let mut raw = (0.0, 0.0);
            if held(&[S::W, S::Up]) {
                raw.1 -= 1.0;
            }
            if held(&[S::S, S::Down]) {
                raw.1 += 1.0;
            }
            if held(&[S::A, S::Left]) {
                raw.0 -= 1.0;
            }
            if held(&[S::D, S::Right]) {
                raw.0 += 1.0;
            }
            // WASD is relative to what's on screen, not the world: "forward" is always the top of the
            // screen even after the camera's been turned around Steve, so rotate the input by cam_yaw.
            let cy = self.explore.cam_yaw;
            dir.0 = raw.0 * cy.cos() + raw.1 * cy.sin();
            dir.1 = raw.1 * cy.cos() - raw.0 * cy.sin();
            // turn the camera around Steve (left/right only - never pitch) so you can see what's behind
            // you without it changing where WASD walks you
            const CAM_ROT_SPEED: f64 = 2.4;
            if held(&[S::LeftBracket, S::Q, S::I]) {
                self.explore.cam_yaw -= CAM_ROT_SPEED * dt;
            }
            if held(&[S::RightBracket, S::R, S::O]) {
                self.explore.cam_yaw += CAM_ROT_SPEED * dt;
            }
        }
        if dir != (0.0, 0.0) {
            self.explore.target = None;
            self.explore.target_pet = None;
        } else if let Some(tg) = self.explore.target {
            let d = (tg.0 - self.explore.pos.0, tg.1 - self.explore.pos.1);
            let reach = if self.explore.target_pet.is_some() { CATCH_R * 0.6 } else { 0.15 };
            if (d.0 * d.0 + d.1 * d.1).sqrt() <= reach {
                self.explore.target = None;
            } else {
                dir = d;
            }
        }
        let len = (dir.0 * dir.0 + dir.1 * dir.1).sqrt();
        let e = &mut self.explore;
        let mut moved = 0.0;
        if len > 1e-6 {
            let (dx, dz) = (dir.0 / len * SPEED * dt, dir.1 / len * SPEED * dt);
            // x and z separately, so Steve slides along walls
            let try_to = |from: (f64, f64), to: (f64, f64)| -> bool {
                // his body is a bit wide: check a little ahead too
                let ahead = (to.0 + (to.0 - from.0).signum() * 0.25, to.1 + (to.1 - from.1).signum() * 0.25);
                w.can_go(from, to) && w.can_go(from, ahead)
            };
            let before = e.pos;
            if try_to(e.pos, (e.pos.0 + dx, e.pos.1)) {
                e.pos.0 += dx;
            }
            if try_to(e.pos, (e.pos.0, e.pos.1 + dz)) {
                e.pos.1 += dz;
            }
            moved = ((e.pos.0 - before.0).powi(2) + (e.pos.1 - before.1).powi(2)).sqrt();
            // turn toward where he walks
            let want = dir.0.atan2(dir.1);
            let mut d = want - e.yaw;
            while d > PI {
                d -= TAU;
            }
            while d < -PI {
                d += TAU;
            }
            e.yaw += d * (dt * 12.0).min(1.0);
            if moved < 1e-4 {
                e.target = None;
            }
        }
        e.moving += ((if moved > 1e-4 { 1.0 } else { 0.0 }) - e.moving) * (dt * 10.0).min(1.0);
        e.stride += moved * 2.2;
        let g = w.ground(e.pos.0, e.pos.1);
        e.y += (g - e.y) * (dt * 14.0).min(1.0);
        e.walked += moved;
        // ---- the wild pets wander, hop, and get caught
        for p in e.wild.iter_mut() {
            if p.caught.is_some() {
                continue;
            }
            p.think -= dt;
            if p.think <= 0.0 {
                p.think = 2.0 + rand_random() * 4.0;
                let a = rand_random() * TAU;
                let r = 1.0 + rand_random() * 3.0;
                let to = (p.pos.0 + a.cos() * r, p.pos.1 + a.sin() * r);
                if w.can_go(p.pos, to) {
                    p.to = to;
                }
            }
            let d = (p.to.0 - p.pos.0, p.to.1 - p.pos.1);
            let l = (d.0 * d.0 + d.1 * d.1).sqrt();
            if l > 0.05 {
                let step = (1.1 * dt).min(l);
                let next = (p.pos.0 + d.0 / l * step, p.pos.1 + d.1 / l * step);
                if w.can_go(p.pos, next) {
                    p.pos = next;
                } else {
                    p.to = p.pos;
                }
            }
        }
        // walked XP, a little at a time
        let mut xp = 0.0;
        if e.walked >= 2.0 {
            xp += e.walked * XP_WALK * DIMENSIONS[e.dim].xp;
            e.walked = 0.0;
        }
        // arrived at the pet you clicked
        if let Some(i) = e.target_pet {
            if let Some(p) = e.wild.get(i) {
                let d = ((p.pos.0 - e.pos.0).powi(2) + (p.pos.1 - e.pos.1).powi(2)).sqrt();
                if d <= CATCH_R && p.caught.is_none() {
                    e.target_pet = None;
                    e.target = None;
                    self.explore_catch(i);
                }
            } else {
                e.target_pet = None;
            }
        }
        // caught pets: into the bag once their flight ends
        let e = &mut self.explore;
        let mut done = Vec::new();
        for (i, p) in e.wild.iter().enumerate() {
            if p.caught.is_some_and(|t0| t - t0 >= CATCH_TIME) {
                done.push(i);
            }
        }
        for i in done.into_iter().rev() {
            // explore_finish_catch awards its own XP (also used by the wild-battle win path in battle_panel.rs)
            self.explore_finish_catch(i);
        }
        // new pets show up (away from Steve)
        let dim = self.explore.dim;
        let before = self.explore.respawn.len();
        self.explore.respawn.retain(|at| *at > t);
        for _ in 0..before - self.explore.respawn.len() {
            let pos = w.random_spot(self.explore.pos, 8.0);
            self.explore.wild.push(Wild { pet: VerityPet::roll(dim, [rand_random(), rand_random(), rand_random(), rand_random()]), pos, to: pos, think: 0.0, seed: rand_random() * 100.0, caught: None });
        }
        self.explore.pops.retain(|p| t - p.2 < 1.4);
        if let Some((_, until)) = self.explore.release_confirm {
            if t > until {
                self.explore.release_confirm = None;
            }
        }
        if xp > 0.0 {
            self.explore_add_xp(xp);
        }
    }

    fn explore_add_xp(&mut self, xp: f64) {
        if let Some(level) = self.state.explore.add_xp(xp) {
            let opened = DIMENSIONS.iter().find(|d| d.level == level);
            let clothes = SHIRTS.iter().any(|s| s.2 == level) || PANTS.iter().any(|s| s.2 == level) || HATS.iter().any(|s| s.1 == level);
            let msg = match opened {
                Some(d) => tr!("Level %d! %s is open", level, tr(d.name)),
                None if clothes => tr!("Level %d! New clothes in the Wardrobe", level),
                None => tr!("Level %d!", level),
            };
            self.show_toast(&msg, 3.0);
            let at = v3(self.explore.pos.0, self.explore.y + 2.9, self.explore.pos.1);
            self.explore.pops.push((tr("LEVEL UP!"), at, self.explore.t, accent()));
            self.play("milestone", 0.0);
        }
    }

    /// The nearest wild pet in reach, if any.
    /// The Verity Pet at explore.wild[i] (its `Wild` wrapper is private to this module) - for start_wild_battle.
    pub fn explore_wild_pet(&self, i: usize) -> Option<&VerityPet> {
        self.explore.wild.get(i).map(|w| &w.pet)
    }

    fn explore_nearest(&self) -> Option<usize> {
        let e = &self.explore;
        e.wild
            .iter()
            .enumerate()
            .filter(|(_, p)| p.caught.is_none())
            .map(|(i, p)| (i, ((p.pos.0 - e.pos.0).powi(2) + (p.pos.1 - e.pos.1).powi(2)).sqrt()))
            .filter(|(_, d)| *d <= CATCH_R)
            .min_by(|a, b| a.1.partial_cmp(&b.1).unwrap_or(std::cmp::Ordering::Equal))
            .map(|(i, _)| i)
    }

    /// Actually gives you a wild pet - a win in the fight it starts, or (with no Verities to fight with yet) an
    /// outright catch - removes it from the world, adds it to your Verity Pets, and gives XP. None if your Verity
    /// Pets were already full (the pet still gets away either way).
    pub fn explore_finish_catch(&mut self, i: usize) -> Option<f64> {
        if i >= self.explore.wild.len() {
            return None;
        }
        let t = self.explore.t;
        let p = self.explore.wild.remove(i);
        self.explore.respawn.push(t + 3.0 + rand_random() * 4.0);
        if let Some(tp) = self.explore.target_pet {
            self.explore.target_pet = if tp == i { None } else if tp > i { Some(tp - 1) } else { Some(tp) };
        }
        let name = rarities()[p.pet.pet].pet;
        let what = if p.pet.luck { tr("Luck") } else { tr("Money") };
        let gain = catch_xp(&p.pet);
        if self.state.explore.add_pet(p.pet.clone()) {
            // counts toward the main Index too - it's the same pet, just met out here instead of rolled
            self.state.seen_pets.insert(format!("{}_normal", p.pet.pet));
            self.show_toast(&tr!("Caught %s %s  +%s%% %s", name, stars(p.pet.stars), format_number(p.pet.boost), what), 2.4);
            let at = v3(self.explore.pos.0, self.explore.y + 2.4, self.explore.pos.1);
            self.explore.pops.push((tr!("+%d XP", gain.round() as i64), at, t, Color::rgb(140, 255, 150)));
            self.play("equip", 0.0);
            self.explore_add_xp(gain);
            Some(gain)
        } else {
            self.show_toast(&tr!("Your Verity Pets are full (%d). Release some in the Bag.", MAX_PETS as i64), 2.6);
            None
        }
    }

    /// E / the Catch! button / clicking a pet in reach: with at least one Verity to send out, it fights back -
    /// win the battle to catch it. Brand new players (no Verities at all yet) just catch it outright.
    pub fn explore_catch(&mut self, i: usize) {
        if self.state.explore.pets.len() >= MAX_PETS {
            self.show_toast(&tr!("Your Verity Pets are full (%d). Release some in the Bag.", MAX_PETS as i64), 2.6);
            return;
        }
        let Some(p) = self.explore.wild.get(i) else { return };
        if p.caught.is_some() {
            return;
        }
        if self.battle_choices().is_empty() {
            // nothing to fight with yet: catch it outright, like before
            if let Some(p) = self.explore.wild.get_mut(i) {
                p.caught = Some(self.explore.t);
            }
            self.play("click", 0.0);
            return;
        }
        self.start_wild_battle(i);
    }

    fn explore_catch_nearest(&mut self) {
        if let Some(i) = self.explore_nearest() {
            self.explore_catch(i);
        }
    }

    /// A click on the world: on a pet, go catch it; anywhere else, walk there.
    fn explore_click(&mut self) {
        let Some(pos) = self.last_click_pos else { return };
        let Some(view) = self.explore.view else { return };
        let r = self.explore.view_rect;
        // the 3D view is drawn at half size
        let (sx, sy) = ((pos.0 - r.x as f64) / 2.0, (pos.1 - r.y as f64) / 2.0);
        // a pet under the mouse?
        let e = &self.explore;
        let w = world(e.dim);
        let mut best: Option<(usize, f64)> = None;
        for (i, p) in e.wild.iter().enumerate() {
            if p.caught.is_some() {
                continue;
            }
            let c = v3(p.pos.0, w.ground(p.pos.0, p.pos.1) + 0.6, p.pos.1);
            if let Some((px, py, z)) = view.project(c) {
                let rad = (PET_R * 1.8) / z * view.focal + 6.0;
                let d = ((px - sx).powi(2) + (py - sy).powi(2)).sqrt();
                if d <= rad && best.is_none_or(|b| d < b.1) {
                    best = Some((i, d));
                }
            }
        }
        if let Some((i, _)) = best {
            let p = &e.wild[i];
            let d = ((p.pos.0 - e.pos.0).powi(2) + (p.pos.1 - e.pos.1).powi(2)).sqrt();
            if d <= CATCH_R {
                self.explore_catch(i);
            } else {
                self.explore.target = Some(p.pos);
                self.explore.target_pet = Some(i);
            }
            return;
        }
        // the ground: where the ray through the mouse meets Steve's height
        let ray = view.ray(sx, sy);
        if ray.y.abs() < 1e-6 {
            return;
        }
        let k = (self.explore.y - view.pos.y) / ray.y;
        if k <= 0.0 {
            return;
        }
        let hit = view.pos + ray * k;
        self.explore.target = Some((hit.x, hit.z));
        self.explore.target_pet = None;
    }

    /// Sends Steve after the nearest wild pet (he catches it when he gets there). For the demo video.
    pub fn explore_chase_nearest(&mut self) {
        let e = &self.explore;
        let best = e
            .wild
            .iter()
            .enumerate()
            .filter(|(_, p)| p.caught.is_none())
            .map(|(i, p)| (i, ((p.pos.0 - e.pos.0).powi(2) + (p.pos.1 - e.pos.1).powi(2)).sqrt()))
            .min_by(|a, b| a.1.partial_cmp(&b.1).unwrap_or(std::cmp::Ordering::Equal));
        if let Some((i, _)) = best {
            self.explore.target = Some(self.explore.wild[i].pos);
            self.explore.target_pet = Some(i);
        }
    }

    pub fn explore_busy(&self) -> bool {
        self.explore.target.is_some() || self.explore.travel.is_some()
    }

    pub fn handle_explore_key(&mut self, ev: KeyEv) -> bool {
        use sdl2::keyboard::Keycode as K;
        if !self.explore.open {
            return false;
        }
        if self.index_open {
            // the Pet Index page is on top: Escape closes it, nothing reaches the world behind it
            if ev.key == K::Escape {
                self.close_index_page();
            }
            return true;
        }
        match ev.key {
            K::Escape => {
                if !self.explore.overlay.is_empty() {
                    self.explore.overlay = "";
                } else {
                    self.close_explore();
                }
            }
            K::E | K::Space | K::Return => {
                if self.explore.overlay.is_empty() {
                    self.explore_catch_nearest();
                }
            }
            K::P => self.explore.overlay = if self.explore.overlay == "pets" { "" } else { "pets" },
            K::C => self.explore.overlay = if self.explore.overlay == "wardrobe" { "" } else { "wardrobe" },
            _ => {}
        }
        true
    }

    pub fn explore_scroll(&mut self, step: f64) -> bool {
        if !self.explore.open || self.explore.overlay != "pets" || self.index_open {
            return false;
        }
        self.explore.pets_scroll = (self.explore.pets_scroll + step).clamp(0.0, self.explore.pets_max_scroll);
        true
    }
}

// ================================================================ drawing the world
/// Everything draw_world needs.
pub struct Scene<'a> {
    pub dim: usize,
    pub steve: V3,
    pub yaw: f64,
    pub stride: f64,
    pub moving: f64,
    pub wave: f64,
    pub look: SteveLook,
    pub t: f64,
    /// the equipped pets (they dance around Steve)
    pub equipped: Vec<Shown>,
    pub wild: &'a [(Shown, V3, f64)],
    pub cam: Camera,
    /// leave out the tall blocks between the camera and Steve
    pub cut_front: bool,
}

/// v4.0.1: view-frustum culling - is this face's quad even possibly on screen? (every face here is one block,
/// ~0.87 across corner to corner, so a 0.6 radius covers it with slack). See View::quad_in_view.
fn face_in_view(view: &View, corners: &[V3; 4]) -> bool {
    view.quad_in_view(corners, 0.6)
}

/// Draws a world at w x h; returns the frame and its view.
fn draw_world(sc: &Scene, w: usize, h: usize) -> (Frame, View) {
    let world = world(sc.dim);
    let mut fr = Frame::new(w, h);
    fr.clear_sky(world.sky.0, world.sky.1);
    if world.stars {
        for i in 0..(w * h / 400) {
            let (x, y) = ((hash(i as i64, 1, 900) * w as f64) as usize, (hash(i as i64, 2, 900) * h as f64) as usize);
            let tw = 150.0 + 100.0 * (sc.t * 2.0 + i as f64).sin();
            fr.color[y.min(h - 1) * w + x.min(w - 1)] = 0xff00_0000 | ((tw as u32) << 16) | ((tw as u32) << 8) | (tw as u32).saturating_add(20).min(255);
        }
    }
    let view = View::new(&sc.cam, w, h);
    let fog = Fog { color: world.fog, start: 26.0, end: 44.0 };
    // the blocks near Steve, on all the cores. Steve's box is a cheap first reject (a face further than the fog
    // could ever draw is never worth even frustum-testing); it's symmetric in every direction now that the
    // camera can turn around Steve - a fixed box behind-vs-ahead would go wrong the moment you looked backward.
    let (sx, sz) = (sc.steve.x.floor() as i64, sc.steve.z.floor() as i64);
    // (tree tops between the camera and Steve are left out; behind anything else he shows through, see below)
    let cut = sc.steve.y + 2.2;
    // which side of Steve the camera is actually on - not always +z any more, now that it can turn around him
    let (cd0, cd1) = (sc.cam.pos.x - sc.steve.x, sc.cam.pos.z - sc.steve.z);
    let cam_len = (cd0 * cd0 + cd1 * cd1).sqrt().max(1e-6);
    let (cam_dx, cam_dz) = (cd0 / cam_len, cd1 / cam_len);
    let faces: Vec<&Face> = world
        .faces
        .iter()
        .filter(|f| (f.x - sx).abs() <= 20 && (f.z - sz).abs() <= 20)
        .filter(|f| face_in_view(&view, &f.corners))
        .filter(|f| !(sc.cut_front && f.leafy && (f.x - sx) as f64 * cam_dx + (f.z - sz) as f64 * cam_dz > -2.0 && f.corners[0].y.min(f.corners[1].y) >= cut))
        .collect();
    let threads = std::thread::available_parallelism().map(|n| n.get()).unwrap_or(1).clamp(1, 8);
    let texes: &Vec<Tex> = &world.texes;
    std::thread::scope(|scope| {
        for mut band in fr.bands(h.div_ceil(threads)) {
            let (view, fog, faces) = (&view, &fog, &faces);
            scope.spawn(move || {
                for f in faces.iter() {
                    band.quad(view, f.corners, &texes[f.tex], f.light, fog);
                }
            });
        }
    });
    // the wild pets: hopping balls with a glow in their rarity's colour
    for (s, at, seed) in sc.wild {
        let hop = ((sc.t * 3.2 + seed).sin().abs()) * 0.35;
        let c = *at + v3(0.0, PET_R + 0.05 + hop, 0.0);
        let face = (sc.steve - *at).norm();
        let rc = rarities()[s.pet].color;
        fr.glow(&view, *at + v3(0.0, 0.1, 0.0), 0.6, (rc.r as f64, rc.g as f64, rc.b as f64), 0.35);
        draw_pet_ball(&mut fr, &view, &fog, *s, c, PET_R, face, 0.0, 1.0);
    }
    // Steve and his pets, in their own frame (see the x-ray below)
    let mut own = Frame::new(w, h);
    let light = world.ambient.max(0.85);
    draw_steve(&mut own, &view, &fog, sc.steve, sc.yaw, if sc.moving > 0.05 { sc.stride } else { 0.0 } * sc.moving.min(1.0), sc.wave, &sc.look, light);
    // the equipped ones dance around him: circling, hopping, spinning, with sparkles
    let n = sc.equipped.len().max(1) as f64;
    let mut sparkles = Vec::new();
    for (i, s) in sc.equipped.iter().enumerate() {
        let a = sc.t * 1.3 + i as f64 * TAU / n;
        let r = 1.35 + (sc.t * 2.0 + i as f64).sin() * 0.15;
        let hop = (sc.t * 5.0 + i as f64 * 1.7).sin().abs() * 0.55;
        let at = sc.steve + v3(a.cos() * r, 0.35 + hop, a.sin() * r);
        let spin = sc.t * 4.0 + i as f64;
        let face = v3(spin.sin(), 0.0, spin.cos());
        draw_pet_ball(&mut own, &view, &fog, *s, at, PET_R * 0.8, face, 0.0, 1.0);
        let rc = rarities()[s.pet].color;
        for k in 0..3 {
            let st = sc.t * 2.0 + k as f64 * 2.1 + i as f64;
            let ph = st.fract();
            let sa = a + k as f64 * 2.0;
            let p = at + v3(sa.cos() * 0.45, -0.2 + ph * 0.9, sa.sin() * 0.45);
            sparkles.push((p, (rc.r as f64, rc.g as f64, rc.b as f64), (1.0 - ph) * 0.9));
        }
    }
    // x-ray: where Steve and his pets are in front, they're drawn; where something hides them, their outline
    // shows through (so you never lose him behind a hill or a tower)
    for i in 0..w * h {
        let d = own.depth[i];
        if !d.is_finite() {
            continue;
        }
        if d <= fr.depth[i] {
            fr.color[i] = own.color[i];
            fr.depth[i] = d;
        } else {
            let c = fr.color[i];
            let mix = |sh: u32, to: f64| ((((c >> sh) & 255) as f64) * 0.45 + to * 0.55) as u32;
            fr.color[i] = 0xff00_0000 | (mix(16, 250.0) << 16) | (mix(8, 220.0) << 8) | mix(0, 120.0);
        }
    }
    for (p, c, k) in sparkles {
        fr.glow(&view, p, 0.06, c, k);
    }
    // glowing blocks light up around them (the nearest ones) - a plain radius, not biased to a fixed
    // direction, since the camera can now face any way around Steve
    for (p, c) in world.lights.iter().filter(|(p, _)| (p.x - sc.steve.x).powi(2) + (p.z - sc.steve.z).powi(2) < 17.0 * 17.0).take(48) {
        let flick = 0.8 + 0.2 * (sc.t * 3.0 + p.x * 1.7 + p.z).sin();
        fr.glow(&view, *p, 0.55, *c, 0.3 * flick);
    }
    (fr, view)
}

/// The camera: above and behind Steve, orbited around him by `cam_yaw` (0 = the usual view, straight behind).
fn follow_cam(steve: V3, cam_yaw: f64) -> Camera {
    let (s, c) = cam_yaw.sin_cos();
    let rot = |v: V3| v3(v.x * c + v.z * s, v.y, -v.x * s + v.z * c);
    Camera { pos: steve + rot(v3(0.0, 13.5, 7.0)), target: steve + rot(v3(0.0, 0.6, -0.8)), fov: 44.0 }
}

impl Game {
    fn explore_scene(&self) -> (Scene<'static>, Vec<(Shown, V3, f64)>) {
        let e = &self.explore;
        let w = world(e.dim);
        let st = &self.state.explore;
        let steve = v3(e.pos.0, e.y, e.pos.1);
        let mut wild = Vec::new();
        for p in &e.wild {
            let ground = v3(p.pos.0, w.ground(p.pos.0, p.pos.1), p.pos.1);
            let at = match p.caught {
                Some(t0) => {
                    // flies into Steve
                    let k = ((e.t - t0) / CATCH_TIME).clamp(0.0, 1.0);
                    ground.lerp(steve + v3(0.0, 1.0, 0.0), k * k) + v3(0.0, (k * PI).sin() * 1.5, 0.0)
                }
                None => ground,
            };
            wild.push((Shown { pet: p.pet.pet, m: "normal", phase: 0 }, at, p.seed));
        }
        let equipped = st.equipped.iter().filter_map(|&i| st.pets.get(i)).map(|p| Shown { pet: p.pet, m: "normal", phase: 0 }).collect();
        let sc = Scene {
            dim: e.dim,
            steve,
            yaw: e.yaw,
            stride: e.stride,
            moving: e.moving,
            wave: 0.0,
            look: SteveLook { shirt: st.shirt, pants: st.pants, hat: st.hat },
            t: e.t,
            equipped,
            wild: &[],
            cam: follow_cam(steve, e.cam_yaw),
            cut_front: true,
        };
        (sc, wild)
    }

    pub fn draw_explore(&mut self, mouse_pos: (f64, f64)) {
        let area = Rect::new(0, TOPBAR_H, self.vw, VIRTUAL_H - TOPBAR_H);
        self.buttons.clear();
        self.register_button(Rect::new(0, 0, self.vw, VIRTUAL_H), Rc::new(|_: &mut Game| {}), None);
        // the world (half size, scaled up: crisp blocky pixels)
        let (mut sc, wild) = self.explore_scene();
        sc.wild = &wild;
        let (lw, lh) = ((area.w / 2).max(8) as usize, (area.h / 2).max(8) as usize);
        let (fr, view) = draw_world(&sc, lw, lh);
        let img = transform::scale(&fr.to_surface(), area.w, area.h);
        self.canvas.blit(&img, area.x, area.y);
        self.explore.view = Some(view);
        self.explore.view_rect = area;
        if self.explore.overlay.is_empty() {
            self.register_button(area, Rc::new(|g: &mut Game| g.explore_click()), None);
        }
        // names over the pets near Steve
        let tiny = self.f.tiny_b.clone();
        let e = &self.explore;
        let near = self.explore_nearest();
        let mut labels = Vec::new();
        for (i, p) in e.wild.iter().enumerate() {
            let d = ((p.pos.0 - e.pos.0).powi(2) + (p.pos.1 - e.pos.1).powi(2)).sqrt();
            if d > 9.0 || p.caught.is_some() {
                continue;
            }
            let at = wild[i].1 + v3(0.0, PET_R * 2.0 + 0.7, 0.0);
            if let Some((x, y, _)) = view.project(at) {
                let r = &rarities()[p.pet.pet];
                labels.push((format!("{} {}", r.pet, stars(p.pet.stars)), r.color, (area.x as f64 + x * 2.0, area.y as f64 + y * 2.0), near == Some(i)));
            }
        }
        for (text, color, (x, y), is_near) in labels {
            let t = tiny.render(&text, color);
            let bg = Rect::new(x as i32 - t.w / 2 - 6, y as i32 - t.h - 4, t.w + 12, t.h + 6);
            draw::rect(&mut self.canvas, Color::rgba(0, 0, 0, 150), bg, 0, 6);
            self.canvas.blit(&t, bg.x + 6, bg.y + 3);
            if is_near {
                let k = self.f.small_b.render(&tr("E: catch"), WHITE);
                let kb = Rect::new(x as i32 - k.w / 2 - 8, bg.y - k.h - 10, k.w + 16, k.h + 6);
                draw::rect(&mut self.canvas, accent(), kb, 0, 8);
                let k = self.f.small_b.render(&tr("E: catch"), BLACK);
                self.canvas.blit(&k, kb.x + 8, kb.y + 3);
            }
        }
        // floating texts
        let t = self.explore.t;
        let pops = self.explore.pops.clone();
        for (text, at, t0, color) in pops {
            let k = (t - t0) / 1.4;
            if let Some((x, y, _)) = view.project(at + v3(0.0, k * 1.2, 0.0)) {
                let mut s = (*self.f.med.render(&text, color)).clone();
                s.set_alpha(((1.0 - k) * 255.0) as i32);
                self.canvas.blit(&s, area.x + (x * 2.0) as i32 - s.w / 2, area.y + (y * 2.0) as i32 - s.h / 2);
            }
        }
        self.draw_explore_hud(area, mouse_pos);
        match self.explore.overlay {
            "wardrobe" => self.draw_explore_wardrobe(mouse_pos),
            "pets" => self.draw_explore_pets(mouse_pos),
            _ => {}
        }
        // travelling: a portal's purple swirl out and back in
        if let Some((to, t0)) = self.explore.travel {
            let k = (t - t0) / 1.2;
            let a = (1.0 - (k * 2.0 - 1.0).abs()).clamp(0.0, 1.0);
            let mut veil = Surface::new_alpha(self.vw, VIRTUAL_H);
            veil.fill(Color::rgba(70, 20, 120, (a * 255.0) as u8), None);
            self.canvas.blit(&veil, 0, 0);
            if a > 0.5 {
                let name = self.f.big.render(&tr(DIMENSIONS[to].name), WHITE);
                let mut name = (*name).clone();
                name.set_alpha(((a - 0.5) * 2.0 * 255.0) as i32);
                self.canvas.blit(&name, self.vw / 2 - name.w / 2, VIRTUAL_H / 2 - name.h / 2);
            }
        }
    }

    fn draw_explore_hud(&mut self, area: Rect, mouse_pos: (f64, f64)) {
        let sb = self.f.small_b.clone();
        let small = self.f.small.clone();
        let st = self.state.explore.clone();
        let level = st.level();
        let (x0, x1) = (xp_for_level(level), xp_for_level(level + 1));
        let frac = ((st.xp - x0) / (x1 - x0).max(1.0)).clamp(0.0, 1.0);
        // ---- top left: the level and its XP bar, the world
        let card = Rect::new(area.x + 16, area.y + 14, 300, 84);
        draw::rect(&mut self.canvas, Color::rgba(10, 12, 22, 190), card, 0, 12);
        let lt = self.f.med.render(&tr!("Level %d", level), accent());
        self.canvas.blit(&lt, card.x + 14, card.y + 8);
        let dn = sb.render(&tr(DIMENSIONS[self.explore.dim].name), WHITE);
        self.canvas.blit(&dn, card.right() - 14 - dn.w, card.y + 12);
        let bar = Rect::new(card.x + 14, card.y + 44, card.w - 28, 14);
        draw::rect(&mut self.canvas, Color::rgb(40, 44, 60), bar, 0, 7);
        draw::rect(&mut self.canvas, Color::rgb(110, 230, 120), Rect::new(bar.x, bar.y, ((bar.w as f64) * frac) as i32, bar.h), 0, 7);
        let xt = self.f.tiny.render(&tr!("%s / %s XP", format_number((st.xp - x0).floor()), format_number(x1 - x0)), grey());
        self.canvas.blit(&xt, card.x + 14, bar.bottom() + 4);
        let pets = self.f.tiny.render(&tr!("Verity Pets %d/%d", st.pets.len() as i64, MAX_PETS as i64), grey());
        self.canvas.blit(&pets, card.right() - 14 - pets.w, bar.bottom() + 4);
        // ---- top right: Wardrobe, Pets, Leave
        let bw = 150;
        let mut bx = area.right() - 16 - 46;
        self.button(Rect::new(bx, area.y + 14, 46, 42), "X", &sb, mouse_pos, Color::rgba(10, 12, 22, 200), BAD, WHITE, cb(|g| g.close_explore()), Bo::r(10));
        bx -= bw + 8;
        let pets_on = self.explore.overlay == "pets";
        self.button(Rect::new(bx, area.y + 14, bw, 42), &tr("Pets (P)"), &sb, mouse_pos, if pets_on { accent() } else { Color::rgba(10, 12, 22, 200) }, accent_hover(), if pets_on { BLACK } else { WHITE }, cb(|g| g.explore.overlay = if g.explore.overlay == "pets" { "" } else { "pets" }), Bo::r(10).icon("bag"));
        bx -= bw + 8;
        let wr_on = self.explore.overlay == "wardrobe";
        self.button(Rect::new(bx, area.y + 14, bw, 42), &tr("Wardrobe (C)"), &sb, mouse_pos, if wr_on { accent() } else { Color::rgba(10, 12, 22, 200) }, accent_hover(), if wr_on { BLACK } else { WHITE }, cb(|g| g.explore.overlay = if g.explore.overlay == "wardrobe" { "" } else { "wardrobe" }), Bo::r(10));
        bx -= bw + 8;
        // which Verity Pet species you've met so far, in Explore or rolled - the same Pet Index as the main game
        self.button(Rect::new(bx, area.y + 14, bw, 42), &tr("Index"), &sb, mouse_pos, Color::rgba(10, 12, 22, 200), accent_hover(), WHITE, cb(|g| g.toggle_index_page()), Bo::r(10).icon("index"));
        // the equipped pets' boosts
        let (mm, lm) = (st.money_mult(), st.luck_mult());
        let boost = tr!("Pets: +%s%% money  ·  +%s%% luck", format_number(((mm - 1.0) * 100.0).round()), format_number(((lm - 1.0) * 100.0).round()));
        let bt = self.f.tiny_b.render(&boost, GOOD);
        let bb = Rect::new(area.right() - 16 - bt.w - 20, area.y + 64, bt.w + 20, bt.h + 10);
        draw::rect(&mut self.canvas, Color::rgba(10, 12, 22, 170), bb, 0, 8);
        self.canvas.blit(&bt, bb.x + 10, bb.y + 5);
        // ---- bottom: the worlds (portals)
        let n = DIMENSIONS.len() as i32;
        let pw = 170.min((area.w - 32 - 8 * (n - 1)) / n);
        let total = pw * n + 8 * (n - 1);
        let by = area.bottom() - 16 - 50;
        for (i, d) in DIMENSIONS.iter().enumerate() {
            let r = Rect::new(area.centerx() - total / 2 + i as i32 * (pw + 8), by, pw, 50);
            let open = self.state.explore.unlocked(i);
            let here = i == self.explore.dim;
            let label = if open { tr(d.name) } else { tr!("Lvl %d", d.level) };
            let (base, hover, text) = if here { (accent(), accent(), BLACK) } else if open { (Color::rgba(10, 12, 22, 200), panel_lighter(), WHITE) } else { (Color::rgba(10, 12, 22, 150), Color::rgba(10, 12, 22, 150), grey_dim()) };
            let font = if sb.size(&label).0 <= pw - 14 { sb.clone() } else { self.f.tiny_b.clone() };
            self.button(r, &label, &font, mouse_pos, base, hover, text, cb(move |g| g.explore_travel(i)), Bo::r(10).sfx(None));
            if !open {
                let lock = self.f.tiny.render(&tr(d.name), grey_dim());
                self.canvas.blit(&lock, r.centerx() - lock.w / 2, r.y - lock.h - 2);
            }
        }
        let hint = small.render(&tr("WASD / arrows or click to walk  ·  E to catch  ·  I / O to turn camera"), WHITE);
        let hb = Rect::new(area.centerx() - hint.w / 2 - 10, by - hint.h - 16, hint.w + 20, hint.h + 8);
        draw::rect(&mut self.canvas, Color::rgba(0, 0, 0, 110), hb, 0, 8);
        self.canvas.blit(&hint, hb.x + 10, hb.y + 4);
        // a catch button when a pet is in reach (for mouse / touch players)
        if self.explore.overlay.is_empty() && self.explore_nearest().is_some() {
            let r = Rect::new(area.right() - 16 - 170, by - 70, 170, 56);
            self.button(r, &tr("Catch!"), &self.f.med.clone(), mouse_pos, GOOD, Color::rgb(140, 245, 160), BLACK, cb(|g| g.explore_catch_nearest()), Bo::r(12).sfx(None));
        }
        // turn-camera buttons (mouse / touch): tap to swing the view a quarter turn around Steve
        if self.explore.overlay.is_empty() {
            let cy = area.centery();
            let l = Rect::new(area.x + 16, cy - 26, 52, 52);
            self.button(l, "<", &self.f.big.clone(), mouse_pos, Color::rgba(10, 12, 22, 170), Color::rgba(30, 34, 52, 200), WHITE, cb(|g| g.explore.cam_yaw -= std::f64::consts::FRAC_PI_2), Bo::r(26).sfx(Some("click")));
            let r = Rect::new(area.right() - 16 - 52, cy - 26, 52, 52);
            self.button(r, ">", &self.f.big.clone(), mouse_pos, Color::rgba(10, 12, 22, 170), Color::rgba(30, 34, 52, 200), WHITE, cb(|g| g.explore.cam_yaw += std::f64::consts::FRAC_PI_2), Bo::r(26).sfx(Some("click")));
        }
    }

    // ---------------------------------------------------------------- the wardrobe
    fn draw_explore_wardrobe(&mut self, mouse_pos: (f64, f64)) {
        let ov = dim_overlay(self.vw, VIRTUAL_H, 120);
        self.canvas.blit(&ov, 0, 0);
        let rect = Rect::with_center(900.min(self.vw - 40), 560, (self.vw / 2, VIRTUAL_H / 2 + 20));
        draw_panel(&mut self.canvas, rect, Some(panel()), 16, true, None);
        self.register_blocker(rect);
        let sb = self.f.small_b.clone();
        let title = self.f.big.render(&tr("Wardrobe"), WHITE);
        self.canvas.blit(&title, rect.x + 24, rect.y + 18);
        self.button(Rect::new(rect.right() - 48, rect.y + 20, 30, 30), "X", &sb, mouse_pos, panel_light(), BAD, WHITE, cb(|g| g.explore.overlay = ""), Bo::r(8));
        // Steve, turning slowly, on a little grass block
        let prev = Rect::new(rect.x + 24, rect.y + 70, 300, rect.h - 94);
        draw::rect(&mut self.canvas, Color::rgb(24, 28, 44), prev, 0, 12);
        let st = &self.state.explore;
        let look = SteveLook { shirt: st.shirt, pants: st.pants, hat: st.hat };
        let img = steve_portrait(&look, self.explore.t, (prev.w / 2) as usize, (prev.h / 2) as usize);
        let img = transform::scale(&img, prev.w, prev.h);
        self.canvas.blit(&img, prev.x, prev.y);
        // the choices
        let level = st.level();
        let (shirt, pants, hat) = (st.shirt, st.pants, st.hat);
        let x = prev.right() + 24;
        let cw = rect.right() - 24 - x;
        let mut y = rect.y + 74;
        for (kind, n) in [("shirt", SHIRTS.len()), ("pants", PANTS.len()), ("hat", HATS.len())] {
            let head = sb.render(&tr(match kind {
                "shirt" => "Shirt",
                "pants" => "Pants",
                _ => "Hat",
            }), grey_dim());
            self.canvas.blit(&head, x, y);
            y += head.h + 6;
            let per_row = 4;
            let bw = (cw - 8 * (per_row - 1)) / per_row;
            for i in 0..n {
                let (name, color, need) = match kind {
                    "shirt" => (SHIRTS[i].0, Some(SHIRTS[i].1), SHIRTS[i].2),
                    "pants" => (PANTS[i].0, Some(PANTS[i].1), PANTS[i].2),
                    _ => (HATS[i].0, None, HATS[i].1),
                };
                let r = Rect::new(x + (i as i32 % per_row) * (bw + 8), y + (i as i32 / per_row) * 44, bw, 38);
                let on = match kind {
                    "shirt" => shirt == i,
                    "pants" => pants == i,
                    _ => hat == i,
                };
                let open = level >= need;
                let label = if open { tr(name) } else { tr!("Lvl %d", need) };
                let font = if sb.size(&label).0 <= bw - 30 { sb.clone() } else { self.f.tiny_b.clone() };
                let k: &'static str = kind;
                self.button(
                    r,
                    &label,
                    &font,
                    mouse_pos,
                    if on { accent() } else { panel_light() },
                    if on { accent() } else { panel_lighter() },
                    if on { BLACK } else if open { WHITE } else { grey_dim() },
                    if open {
                        cb(move |g| {
                            match k {
                                "shirt" => g.state.explore.shirt = i,
                                "pants" => g.state.explore.pants = i,
                                _ => g.state.explore.hat = i,
                            };
                        })
                    } else {
                        None
                    },
                    Bo::r(8).enabled(open),
                );
                if let Some(c) = color {
                    let (r0, g0, b0) = rgb_of(c);
                    draw::circle(&mut self.canvas, Color::rgb(r0 as u8, g0 as u8, b0 as u8), (r.x + 14, r.centery()), 7, 0);
                }
            }
            y += ((n as i32 + per_row - 1) / per_row) * 44 + 12;
        }
    }

    // ---------------------------------------------------------------- the pets (also in the Bag)
    fn draw_explore_pets(&mut self, mouse_pos: (f64, f64)) {
        let ov = dim_overlay(self.vw, VIRTUAL_H, 120);
        self.canvas.blit(&ov, 0, 0);
        let rect = Rect::with_center(1000.min(self.vw - 40), 600.min(VIRTUAL_H - TOPBAR_H - 30), (self.vw / 2, (VIRTUAL_H + TOPBAR_H) / 2));
        draw_panel(&mut self.canvas, rect, Some(panel()), 16, true, None);
        self.register_blocker(rect);
        let sb = self.f.small_b.clone();
        let title = self.f.big.render(&tr("Verity Pets"), WHITE);
        self.canvas.blit(&title, rect.x + 24, rect.y + 18);
        self.button(Rect::new(rect.right() - 48, rect.y + 20, 30, 30), "X", &sb, mouse_pos, panel_light(), BAD, WHITE, cb(|g| g.explore.overlay = ""), Bo::r(8));
        let content = Rect::new(rect.x + 10, rect.y + 70, rect.w - 20, rect.h - 80);
        let scroll = self.explore.pets_scroll;
        let h = self.draw_verity_pets(content, scroll, mouse_pos, false);
        self.explore.pets_max_scroll = (h - content.h as f64).max(0.0);
        self.explore.pets_scroll = self.explore.pets_scroll.min(self.explore.pets_max_scroll);
    }

    /// The Verity Pets: a header (how many, the boosts, Equip Best) and a card per pet (equip / release).
    /// Drawn in `content` scrolled by `scroll`; returns the height of it all. Used by the Bag and by Explore.
    pub fn draw_verity_pets(&mut self, content: Rect, scroll: f64, mouse_pos: (f64, f64), in_bag: bool) -> f64 {
        let pad = 14;
        let sb = self.f.small_b.clone();
        let small = self.f.small.clone();
        let tiny = self.f.tiny.clone();
        let st = self.state.explore.clone();
        let n_eq = st.equipped.len();
        let (mm, lm) = (st.money_mult(), st.luck_mult());
        self.push_clip(content);
        let top = content.y - scroll as i32;
        let head = small.render(&tr!("%d/%d pets  ·  equipped %d/%d  ·  +%s%% money  ·  +%s%% luck", st.pets.len() as i64, MAX_PETS as i64, n_eq as i64, MAX_EQUIPPED as i64, format_number(((mm - 1.0) * 100.0).round()), format_number(((lm - 1.0) * 100.0).round())), grey());
        self.canvas.blit(&head, content.x + pad, top + 12);
        let mut bx = content.right() - pad - 170;
        self.button(Rect::new(bx, top + 4, 170, 34), &tr("Equip Best"), &sb, mouse_pos, Color::rgb(52, 120, 80), Color::rgb(66, 150, 100), WHITE, cb(|g| g.state.explore.equip_best()), Bo::r(9).sfx(Some("equip")));
        if in_bag {
            bx -= 170 + 8;
            self.button(Rect::new(bx, top + 4, 170, 34), &tr("Go Explore"), &sb, mouse_pos, accent(), accent_hover(), BLACK, cb(|g| {
                g.left_panel.close();
                g.open_battle();
                g.open_explore();
            }), Bo::r(9));
        }
        let pets = self.state.explore.pets.clone();
        let y0 = top + 52;
        if pets.is_empty() {
            for (k, line) in [tr("No Verity Pets yet."), tr("Open Battle > Explore, walk around and catch some!")].iter().enumerate() {
                let t = small.render(line, grey());
                self.canvas.blit(&t, content.x + pad, y0 + 10 + k as i32 * 26);
            }
            self.pop_clip();
            return 120.0;
        }
        let gap = 12;
        let cols = ((content.w - pad * 2 + gap) / (170 + gap)).max(2);
        let cw = (content.w - pad * 2 - gap * (cols - 1)) / cols;
        let ch = 218;
        // equipped ones first, then the best boosts
        let mut order: Vec<usize> = (0..pets.len()).collect();
        let eq = self.state.explore.equipped.clone();
        order.sort_by(|a, b| {
            let (ea, eb) = (eq.contains(a), eq.contains(b));
            eb.cmp(&ea).then(pets[*b].boost.partial_cmp(&pets[*a].boost).unwrap_or(std::cmp::Ordering::Equal))
        });
        let confirm = self.explore.release_confirm.map(|c| c.0);
        for (k, &i) in order.iter().enumerate() {
            let p = &pets[i];
            let r = Rect::new(content.x + pad + (k as i32 % cols) * (cw + gap), y0 + (k as i32 / cols) * (ch + gap), cw, ch);
            if r.bottom() < content.y || r.y > content.bottom() {
                continue;
            }
            let rar = &rarities()[p.pet];
            let on = eq.contains(&i);
            draw::rect(&mut self.canvas, panel_light(), r, 0, 12);
            draw::rect(&mut self.canvas, if on { accent() } else { rar.color }, r, if on { 3 } else { 2 }, 12);
            if let Some(img) = load_pet_image(rar.pet, 92, false) {
                self.canvas.blit(&img, r.centerx() - img.w / 2, r.y + 4);
            }
            let name = sb.render(rar.pet, WHITE);
            self.canvas.blit(&name, r.centerx() - name.w / 2, r.y + 96);
            let rn = tiny.render(&format!("{}  {}", tr(rar.name), stars(p.stars)), rar.color);
            self.canvas.blit(&rn, r.centerx() - rn.w / 2, r.y + 96 + name.h);
            let what = if p.luck { tr!("+%s%% luck", format_number(p.boost)) } else { tr!("+%s%% money", format_number(p.boost)) };
            let bt = sb.render(&what, if p.luck { Color::rgb(120, 210, 255) } else { GOOD });
            self.canvas.blit(&bt, r.centerx() - bt.w / 2, r.y + 116 + name.h);
            let wt = tiny.render(&tr(DIMENSIONS[p.dim].name), grey_dim());
            self.canvas.blit(&wt, r.centerx() - wt.w / 2, r.y + 138 + name.h);
            let by = r.bottom() - 40;
            let ew = r.w - 16 - 44;
            let can = on || n_eq < MAX_EQUIPPED;
            self.button(
                Rect::new(r.x + 8, by, ew, 32),
                &if on { tr("Unequip") } else { tr("Equip") },
                &sb,
                mouse_pos,
                if on { panel_lighter() } else { Color::rgb(52, 120, 80) },
                if on { panel_lighter() } else { Color::rgb(66, 150, 100) },
                WHITE,
                if can { cb(move |g| {
                    g.state.explore.toggle_equip(i);
                }) } else { None },
                Bo::r(8).enabled(can).sfx(Some("equip")),
            );
            let sure = confirm == Some(i);
            self.button(
                Rect::new(r.right() - 8 - 40, by, 40, 32),
                if sure { "?" } else { "X" },
                &sb,
                mouse_pos,
                if sure { BAD } else { panel_lighter() },
                BAD,
                WHITE,
                cb(move |g| {
                    if g.explore.release_confirm.map(|c| c.0) == Some(i) {
                        g.state.explore.release(i);
                        g.explore.release_confirm = None;
                        g.show_toast(&tr("Released it back into the wild."), 1.6);
                    } else {
                        g.explore.release_confirm = Some((i, g.explore.t + 3.0));
                        g.show_toast(&tr("Click X again to release this pet."), 1.8);
                    }
                }),
                Bo::r(8),
            );
        }
        self.pop_clip();
        let rows = (pets.len() as i32 + cols - 1) / cols;
        (52 + rows * (ch + gap) + 10) as f64
    }
}

/// Steve alone, turning (the wardrobe's preview).
fn steve_portrait(look: &SteveLook, t: f64, w: usize, h: usize) -> Surface {
    let mut fr = Frame::new(w, h);
    fr.clear_sky(0xff1c2034, 0xff2c3350);
    let cam = Camera { pos: v3(0.0, 1.5, 4.4), target: v3(0.0, 1.05, 0.0), fov: 40.0 };
    let view = View::new(&cam, w, h);
    let fog = Fog { color: 0x2c3350, start: 50.0, end: 90.0 };
    let texes = textures();
    // a grass block to stand on
    let c = v3(0.0, -0.5, 0.0);
    for (n, rt, up, tex) in [(UP, v3(1.0, 0.0, 0.0), v3(0.0, 0.0, -1.0), 0), (v3(0.0, 0.0, 1.0), v3(1.0, 0.0, 0.0), UP, 1), (v3(1.0, 0.0, 0.0), v3(0.0, 0.0, -1.0), UP, 1), (v3(-1.0, 0.0, 0.0), v3(0.0, 0.0, 1.0), UP, 1)] {
        let fc = c + n * 0.5;
        let corners = [fc - rt * 0.5 + up * 0.5, fc - rt * 0.5 - up * 0.5, fc + rt * 0.5 - up * 0.5, fc + rt * 0.5 + up * 0.5];
        let l = if n.y > 0.5 { 1.0 } else if n.z > 0.5 { 0.75 } else { 0.85 };
        fr.quad(&view, corners, &texes[tex], l, &fog);
    }
    let wave = if (t % 6.0) > 4.5 { t } else { 0.0 };
    draw_steve(&mut fr, &view, &fog, v3(0.0, 0.0, 0.0), (t * 0.8).sin() * 0.9, 0.0, wave, look, 1.0);
    fr.to_surface()
}

/// --shots / debugging only: puts one wild pet right at Steve's feet (`Wild` is private to this module).
pub fn debug_place_wild(g: &mut Game, pet: VerityPet) {
    let at = g.explore.pos;
    g.explore.wild = vec![Wild { pet, pos: at, to: at, think: 99.0, seed: 0.0, caught: None }];
}

/// A still of Explore for the Battle hub's button: Steve and his pets in the Overworld.
pub fn explore_preview(look: SteveLook, equipped: Vec<Shown>, t: f64, w: usize, h: usize) -> Surface {
    let wd = world(0);
    let steve = v3(wd.spawn.0, wd.ground(wd.spawn.0, wd.spawn.1), wd.spawn.1);
    // a pet to look at while it waits
    let wild_at = steve + v3(2.6, 0.0, -1.4);
    let wild = [(Shown { pet: 5, m: "normal", phase: 0 }, v3(wild_at.x, wd.ground(wild_at.x, wild_at.z), wild_at.z), 1.0)];
    let a = t * 0.25;
    let cam = Camera { pos: steve + v3(a.sin() * 5.0, 5.5, a.cos() * 5.0 + 2.0), target: steve + v3(0.4, 0.8, -0.4), fov: 50.0 };
    let equipped = if equipped.is_empty() { vec![Shown { pet: 2, m: "normal", phase: 0 }, Shown { pet: 12, m: "normal", phase: 0 }] } else { equipped };
    let sc = Scene { dim: 0, steve, yaw: 0.6, stride: 0.0, moving: 0.0, wave: if (t % 5.0) > 3.8 { t } else { 0.0 }, look, t, equipped, wild: &wild, cam, cut_front: false };
    draw_world(&sc, w, h).0.to_surface()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn worlds_have_room_to_walk() {
        for dim in 0..DIMENSIONS.len() {
            let w = build_world(dim);
            let reachable = w.reach.iter().filter(|r| **r).count();
            assert!(reachable > 400, "{}: only {} columns to walk on", DIMENSIONS[dim].key, reachable);
            assert!(w.reach[col(N / 2, N / 2)]);
            assert!(!w.faces.is_empty());
        }
    }

    /// E (or the Catch! button) on a pet you can fight starts a battle - not an instant catch - and winning it is
    /// what actually puts the pet in your Verity Pets.
    #[test]
    fn wild_battle_catches_on_win() {
        let mut g = Game::headless(1200, 700);
        // a high-tier team so the fight is a lopsided, quick win regardless of RNG
        g.state.owned.insert(crate::core::data::owned_key(8, "normal", 0), 1);
        g.open_explore();
        assert!(g.explore.open);
        let at = g.explore.pos;
        g.explore.wild = vec![Wild { pet: VerityPet::roll(0, [0.9, 0.05, 0.0, 0.0]), pos: at, to: at, think: 99.0, seed: 0.0, caught: None }];
        let caught_pet = g.explore.wild[0].pet.pet;
        g.explore_catch(0);
        assert!(g.battle.wild.is_some(), "pressing catch with a team to fight with should start a battle, not an instant catch");
        assert!(g.battle.battle.is_some());
        assert!(g.explore.open, "Explore stays open (paused) behind the fight");
        // the world freezes while the fight is on
        let pos_before = g.explore.pos;
        g.tick_explore(1.0);
        assert_eq!(g.explore.pos, pos_before);
        // play it out with Strike (never misses) until there's a winner
        let mut turns = 0;
        while g.battle.battle.as_ref().is_some_and(|b| b.winner.is_none()) {
            g.tick_battle(0.05);
            if !g.battle.busy() {
                g.battle_use(crate::core::battle::Move::Strike);
                turns += 1;
                assert!(turns < 40, "the fight never resolved");
            }
        }
        for _ in 0..300 {
            // drain the rest of the playback (the faint, the result screen's reward block)
            g.tick_battle(0.1);
        }
        assert_eq!(g.state.explore.pets.len(), 1);
        assert_eq!(g.state.explore.pets[0].pet, caught_pet);
        // Continue: back to walking, no battle state left over
        g.battle_wild_continue();
        assert!(g.battle.wild.is_none());
        assert!(g.explore.open);
    }
}
