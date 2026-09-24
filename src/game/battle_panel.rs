//! Battle page (first version, Pokemon style): pick up to 3 of your verities, then fight a rival team turn by turn.
//! The rules are in core/battle.rs; this page plays back its events (text, lunges, hits, HP bars, faints) one at a
//! time. For now the rival is played by the computer.

use super::base::Bo;
use super::{Game, KeyEv, cb};
use crate::config::VIRTUAL_H;
use super::battle3d::{SEND_FALL, SPECIAL_ARRIVE, STRIKE_ARRIVE, Shot, Shown, Stage, draw_arena_3d};
use crate::core::battle::{Action, Battle, Ev, Fighter, MOVES, Move, TEAM_SIZE, move_info, move_name, rival_team};
use crate::core::data::{PHASES, is_mutation, mut_key, rarities};
use crate::core::formatting::format_number;
use crate::gfx::{Color, Rect, draw, transform};
use crate::i18n::tr;
use crate::theme::*;
use crate::tr;
use crate::ui::cards::{cell, rarity_glow_color, render_pet_card_phase};
use crate::ui::drawing::{dim_overlay, draw_panel, ease_out_cubic};
use crate::ui::fonts::{fit_text, wrap_text};
use std::collections::VecDeque;
use std::rc::Rc;

/// how many of your verities the picker shows (the strongest first)
const PICK_SHOWN: usize = 18;

pub struct BattleUi {
    pub open: bool,
    /// "pick" (choosing the team) or "fight"
    pub stage: &'static str,
    pub picks: Vec<(usize, &'static str)>,
    pub battle: Option<Battle>,
    /// the battle's clock (advanced by tick_battle), for the playback
    t: f64,
    queue: VecDeque<Ev>,
    cur: Option<(Ev, f64)>,
    /// when the next queued event starts (each one starts when the one before it ends)
    next_start: f64,
    text: String,
    /// "main", "fight" or "switch"
    pub menu: &'static str,
    shown: [Option<usize>; 2],
    hp_from: [f64; 2],
    hp_to: [f64; 2],
    hp_t0: [f64; 2],
    send_t0: [f64; 2],
    lunge_t0: [f64; 2],
    hit_t0: [f64; 2],
    heal_t0: [f64; 2],
    guard_t0: [f64; 2],
    faint_t0: [Option<f64>; 2],
    lunge_special: [bool; 2],
    hit_crit: [bool; 2],
    last_dmg: [i32; 2],
    dmg_t0: [f64; 2],
    /// the 3D camera's shot, since when, and the one before (it glides from one to the next)
    shot: Shot,
    shot_t0: f64,
    prev_shot: Shot,
    /// what the win paid (None until the battle is over and paid)
    reward: Option<f64>,
    hover_move: Option<Move>,
    seed: u64,
    /// the arena background, scaled to the arena (w, h, image)
    bg: Option<(i32, i32, crate::gfx::Surf)>,
}

impl BattleUi {
    pub fn new() -> BattleUi {
        let far = -1e9;
        BattleUi {
            open: false,
            stage: "pick",
            picks: Vec::new(),
            battle: None,
            t: 0.0,
            queue: VecDeque::new(),
            cur: None,
            next_start: 0.0,
            text: String::new(),
            menu: "main",
            shown: [None, None],
            hp_from: [0.0; 2],
            hp_to: [0.0; 2],
            hp_t0: [far; 2],
            send_t0: [far; 2],
            lunge_t0: [far; 2],
            hit_t0: [far; 2],
            heal_t0: [far; 2],
            guard_t0: [far; 2],
            faint_t0: [None; 2],
            lunge_special: [false; 2],
            hit_crit: [false; 2],
            last_dmg: [0; 2],
            dmg_t0: [far; 2],
            shot: Shot::Idle,
            shot_t0: 0.0,
            prev_shot: Shot::Idle,
            reward: None,
            hover_move: None,
            seed: 1,
            bg: None,
        }
    }

    /// Still showing something (the menu waits).
    pub fn busy(&self) -> bool {
        self.cur.is_some() || !self.queue.is_empty()
    }

    fn shown_hp(&self, side: usize) -> f64 {
        let p = ((self.t - self.hp_t0[side]) / 0.6).clamp(0.0, 1.0);
        self.hp_from[side] + (self.hp_to[side] - self.hp_from[side]) * ease_out_cubic(p)
    }
}

/// How long each event plays (the 3D arena's animations are timed to these).
fn duration(ev: &Ev) -> f64 {
    match ev {
        Ev::Say(_) => 1.0,
        Ev::SendOut { .. } => SEND_FALL + 0.6,
        Ev::Lunge { special, .. } => if *special { SPECIAL_ARRIVE } else { STRIKE_ARRIVE },
        Ev::Hit { .. } => 1.0,
        Ev::Heal { .. } => 1.1,
        Ev::Guard { .. } => 1.0,
        Ev::Faint { .. } => 1.4,
    }
}

fn hp_color(frac: f64) -> Color {
    if frac > 0.5 {
        Color::rgb(80, 210, 110)
    } else if frac > 0.2 {
        Color::rgb(245, 200, 60)
    } else {
        Color::rgb(235, 70, 70)
    }
}

impl Game {
    // ---------------------------------------------------------------- open / close
    pub fn open_battle(&mut self) {
        self.close_overlays();
        self.left_panel.close();
        self.right_panel.close();
        self.battle.open = true;
        self.battle.stage = "pick";
        self.battle.battle = None;
        let owned: Vec<(usize, &'static str)> = self.battle_choices();
        self.battle.picks.retain(|p| owned.contains(p));
        if self.battle.picks.is_empty() {
            self.battle_auto_pick();
        }
    }

    pub fn close_battle(&mut self) {
        self.battle.open = false;
        self.battle.battle = None;
        self.battle.queue.clear();
        self.battle.cur = None;
    }

    pub fn toggle_battle(&mut self) {
        if self.battle.open {
            self.close_battle();
        } else {
            self.open_battle();
        }
    }

    // ---------------------------------------------------------------- picking the team
    fn fighter_for(&self, pet: usize, m: &'static str) -> Fighter {
        Fighter::new(pet, m, self.state.phase(pet, m))
    }

    /// Your verities (one entry per verity + mutation), the strongest first.
    pub fn battle_choices(&self) -> Vec<(usize, &'static str)> {
        let mut v: Vec<(usize, &'static str)> = Vec::new();
        for (key, n) in &self.state.owned {
            let Some((idx_s, m)) = key.split_once('_') else { continue };
            let Ok(idx) = idx_s.trim().parse::<usize>() else { continue };
            if *n > 0 && idx < rarities().len() && is_mutation(m) {
                v.push((idx, mut_key(m)));
            }
        }
        v.sort_by_key(|(i, m)| -self.fighter_for(*i, m).power());
        v
    }

    pub fn battle_toggle_pick(&mut self, pet: usize, m: &'static str) {
        if let Some(pos) = self.battle.picks.iter().position(|p| *p == (pet, m)) {
            self.battle.picks.remove(pos);
        } else if self.battle.picks.len() < TEAM_SIZE {
            self.battle.picks.push((pet, m));
        }
    }

    pub fn battle_auto_pick(&mut self) {
        self.battle.picks = self.battle_choices().into_iter().take(TEAM_SIZE).collect();
    }

    // ---------------------------------------------------------------- fighting
    pub fn start_battle(&mut self) {
        if self.battle.picks.is_empty() {
            return;
        }
        let team: Vec<Fighter> = self.battle.picks.iter().map(|(p, m)| self.fighter_for(*p, m)).collect();
        self.battle.seed = self.battle.seed.wrapping_mul(6364136223846793005).wrapping_add((crate::core::state::now_ts() * 1000.0) as u64 | 1);
        let rival = rival_team(&team, self.battle.seed);
        let (b, evs) = Battle::new(team, rival, self.battle.seed);
        let ui = &mut self.battle;
        ui.battle = Some(b);
        ui.stage = "fight";
        ui.menu = "main";
        ui.reward = None;
        ui.text.clear();
        ui.shown = [None, None];
        ui.faint_t0 = [None, None];
        ui.queue = evs.into();
        ui.cur = None;
        ui.next_start = ui.t;
    }

    fn battle_push(&mut self, evs: Vec<Ev>) {
        if !self.battle.busy() {
            self.battle.next_start = self.battle.t;
        }
        self.battle.queue.extend(evs);
        self.battle.menu = "main";
        self.battle.hover_move = None;
    }

    pub fn battle_use(&mut self, mv: Move) {
        if self.battle.busy() {
            return;
        }
        let Some(b) = self.battle.battle.as_mut() else { return };
        if !b.fighter(0).can_use(mv) {
            return;
        }
        let evs = b.turn(Action::Use(mv));
        self.battle_push(evs);
    }

    pub fn battle_switch(&mut self, idx: usize) {
        if self.battle.busy() {
            return;
        }
        let Some(b) = self.battle.battle.as_mut() else { return };
        let evs = if b.needs_switch() { b.send_out(idx) } else if b.can_switch_to(0, idx) { b.turn(Action::Switch(idx)) } else { Vec::new() };
        if !evs.is_empty() {
            self.battle_push(evs);
        }
    }

    pub fn battle_run(&mut self) {
        self.battle.stage = "pick";
        self.battle.battle = None;
        self.battle.queue.clear();
        self.battle.cur = None;
        self.show_toast(&tr("You ran away safely."), 1.8);
    }

    /// Skips the text on screen (click / Enter).
    pub fn battle_skip(&mut self) {
        let t = self.battle.t;
        if let Some((ev, _)) = &self.battle.cur {
            if matches!(ev, Ev::Say(_)) {
                // it ends now (the next one starts from here, not all at once)
                self.battle.cur = Some((ev.clone(), t - duration(ev)));
            }
        }
    }

    /// Advances the playback; the Game's tick calls it every frame.
    pub fn tick_battle(&mut self, dt: f64) {
        if !self.battle.open || self.battle.stage != "fight" {
            return;
        }
        let ui = &mut self.battle;
        ui.t += dt;
        loop {
            // each event starts when the one before it ends (so a long frame still plays them all in order)
            if let Some((ev, t0)) = &ui.cur {
                if ui.t - t0 < duration(ev) {
                    break;
                }
                ui.next_start = t0 + duration(ev);
                ui.cur = None;
            }
            if ui.next_start > ui.t {
                break;
            }
            let Some(ev) = ui.queue.pop_front() else { break };
            let t = ui.next_start;
            let new_shot = match &ev {
                Ev::Say(_) => None,
                Ev::SendOut { side, .. } => Some(Shot::SendOut(*side)),
                // a monster's special is a dash, like a Strike
                Ev::Lunge { side, special } => Some(Shot::Attack(*side, *special)),
                Ev::Hit { side, .. } => Some(Shot::Impact(*side)),
                Ev::Heal { side, .. } => Some(Shot::Heal(*side)),
                Ev::Guard { side } => Some(Shot::Guard(*side)),
                Ev::Faint { side } => Some(Shot::Faint(*side)),
            };
            if let Some(sh) = new_shot {
                ui.prev_shot = ui.shot;
                ui.shot = sh;
                ui.shot_t0 = t;
            }
            match &ev {
                Ev::Say(s) => ui.text = s.clone(),
                Ev::SendOut { side, idx, hp } => {
                    ui.shown[*side] = Some(*idx);
                    ui.hp_from[*side] = *hp as f64;
                    ui.hp_to[*side] = *hp as f64;
                    ui.send_t0[*side] = t;
                    ui.faint_t0[*side] = None;
                }
                Ev::Lunge { side, special } => {
                    ui.lunge_t0[*side] = t;
                    ui.lunge_special[*side] = *special;
                }
                Ev::Hit { side, hp, .. } | Ev::Heal { side, hp } => {
                    let now = ui.shown_hp(*side);
                    if let Ev::Hit { crit, .. } = &ev {
                        ui.hit_crit[*side] = *crit;
                        ui.last_dmg[*side] = (now - *hp as f64).round() as i32;
                        ui.dmg_t0[*side] = t;
                    }
                    ui.hp_from[*side] = now;
                    ui.hp_to[*side] = *hp as f64;
                    ui.hp_t0[*side] = t;
                    if matches!(ev, Ev::Hit { .. }) {
                        ui.hit_t0[*side] = t;
                    } else {
                        ui.heal_t0[*side] = t;
                    }
                }
                Ev::Guard { side } => ui.guard_t0[*side] = t,
                Ev::Faint { side } => ui.faint_t0[*side] = Some(t),
            }
            ui.cur = Some((ev, t));
        }
        // waiting for the player: the camera goes back to its slow orbit
        if !self.battle.busy() && self.battle.shot != Shot::Idle {
            let ui = &mut self.battle;
            ui.prev_shot = ui.shot;
            ui.shot = Shot::Idle;
            ui.shot_t0 = ui.t;
        }
        // the battle is over and everything was shown: pay the win once
        if !self.battle.busy() && self.battle.reward.is_none() {
            let winner = self.battle.battle.as_ref().and_then(|b| b.winner);
            if let Some(w) = winner {
                let reward = if w == 0 { (self.state.income_per_second() * 300.0).max(100.0).round() } else { 0.0 };
                if reward > 0.0 {
                    self.state.coins += reward;
                    self.state.total_coins_earned += reward;
                    self.state.battles_won += 1;
                    self.state.dirty = true;
                }
                self.battle.reward = Some(reward);
            } else if self.battle.battle.as_ref().is_some_and(|b| b.needs_switch()) {
                self.battle.menu = "switch";
            }
        }
    }

    /// Keys with the battle open. Returns true if the key was used (it takes every key).
    pub fn handle_battle_key(&mut self, ev: KeyEv) -> bool {
        use sdl2::keyboard::Keycode as K;
        if !self.battle.open {
            return false;
        }
        match ev.key {
            K::Return | K::KpEnter | K::Space => {
                if self.battle.busy() {
                    self.battle_skip();
                } else if self.battle.stage == "pick" {
                    self.start_battle();
                }
            }
            K::Escape => {
                if self.battle.stage == "pick" {
                    self.close_battle();
                } else if self.battle.menu == "fight" || (self.battle.menu == "switch" && !self.battle.battle.as_ref().is_some_and(|b| b.needs_switch())) {
                    self.battle.menu = "main";
                }
            }
            _ => {}
        }
        true
    }

    // ---------------------------------------------------------------- drawing
    pub fn draw_battle(&mut self, mouse_pos: (f64, f64)) {
        // the game isn't drawn under the battle (see draw_game_screen): a plain dark backdrop is enough, and cheap
        draw::rect(&mut self.canvas, Color::rgb(12, 13, 22), Rect::new(0, crate::config::TOPBAR_H, self.vw, VIRTUAL_H - crate::config::TOPBAR_H), 0, 0);
        // nothing behind the battle can be clicked
        self.buttons.clear();
        self.register_button(Rect::new(0, 0, self.vw, VIRTUAL_H), Rc::new(|_: &mut Game| {}), None);
        if self.battle.stage == "fight" && self.battle.battle.is_some() {
            self.draw_battle_fight(mouse_pos);
        } else {
            self.draw_battle_pick(mouse_pos);
        }
    }

    fn draw_battle_pick(&mut self, mouse_pos: (f64, f64)) {
        let w = 1000.min(self.vw - 40);
        let h = 700.min(VIRTUAL_H - 40);
        let rect = Rect::with_center(w, h, (self.vw / 2, VIRTUAL_H / 2 + 10));
        draw_panel(&mut self.canvas, rect, Some(panel()), 16, true, None);
        let pad = 24;
        let sb = self.f.small_b.clone();
        let med = self.f.med.clone();
        let small = self.f.small.clone();
        let title = self.f.big.render(&tr("Battle"), WHITE);
        self.canvas.blit(&title, rect.x + pad, rect.y + 18);
        let sub = small.render(&tr!("Pick up to %d of your Verities. Monsters and rare mutations hit the hardest.", TEAM_SIZE as i64), grey());
        self.canvas.blit(&sub, rect.x + pad + 2, rect.y + 20 + title.h);
        self.button(Rect::new(rect.right() - 48, rect.y + 20, 30, 30), "X", &sb, mouse_pos, panel_light(), BAD, WHITE, cb(|g| g.close_battle()), Bo::r(8));
        if self.state.battles_won > 0 {
            let t = self.f.tiny_b.render(&tr!("Battles won: %s", format_number(self.state.battles_won as f64)), accent());
            self.canvas.blit(&t, rect.right() - 60 - t.w, rect.y + 28);
        }

        // ---- your team (click one to take it out)
        let ty = rect.y + 88;
        let slot = 112;
        let t = sb.render(&tr("Your team"), grey_dim());
        self.canvas.blit(&t, rect.x + pad, ty);
        for i in 0..TEAM_SIZE {
            let r = Rect::new(rect.x + pad + i as i32 * (slot + 12), ty + 24, slot, slot);
            match self.battle.picks.get(i).copied() {
                Some((pet, m)) => {
                    let card = render_pet_card_phase(&rarities()[pet], m, self.state.phase(pet, m), slot, slot, &[], 0, false);
                    self.canvas.blit(&card, r.x, r.y);
                    if r.collidepoint(mouse_pos) {
                        draw::rect(&mut self.canvas, BAD, r, 3, 12);
                    }
                    self.register_button(r, Rc::new(move |g: &mut Game| g.battle_toggle_pick(pet, m)), Some("click"));
                }
                None => {
                    draw_panel(&mut self.canvas, r, Some(panel_light()), 12, false, None);
                    let e = small.render(&tr("empty"), grey_dim());
                    self.canvas.blit(&e, r.centerx() - e.w / 2, r.centery() - e.h / 2);
                }
            }
        }
        let bx = rect.x + pad + TEAM_SIZE as i32 * (slot + 12) + 20;
        let bw = rect.right() - pad - bx;
        let can_fight = !self.battle.picks.is_empty();
        self.button(Rect::new(bx, ty + 24, bw, 46), &tr("Auto pick best"), &med, mouse_pos, panel_light(), panel_lighter(), WHITE, cb(|g| g.battle_auto_pick()), Bo::r(10));
        self.button(Rect::new(bx, ty + 24 + 56, bw, 56), &tr("Fight!"), &self.f.big.clone(), mouse_pos, BAD, Color::rgb(250, 110, 110), WHITE, if can_fight { cb(|g| g.start_battle()) } else { None }, Bo::r(12).enabled(can_fight).icon("battle"));

        // ---- your verities, the strongest first
        let gy = ty + 24 + slot + 22;
        let t = sb.render(&tr("Your Verities"), grey_dim());
        self.canvas.blit(&t, rect.x + pad, gy);
        let choices = self.battle_choices();
        if choices.is_empty() {
            let t = small.render(&tr("You don't have any pets yet. Roll some!"), grey());
            self.canvas.blit(&t, rect.x + pad, gy + 30);
            return;
        }
        let cols = 6;
        let gap = 10;
        let card_w = (w - pad * 2 - gap * (cols - 1)) / cols;
        let card_h = ((rect.bottom() - pad - (gy + 24) - gap * 2) / 3).min(card_w + 30);
        for (n, (pet, m)) in choices.iter().take(PICK_SHOWN).enumerate() {
            let (pet, m) = (*pet, *m);
            let (col, row) = (n as i32 % cols, n as i32 / cols);
            let r = Rect::new(rect.x + pad + col * (card_w + gap), gy + 24 + row * (card_h + gap), card_w, card_h);
            if r.bottom() > rect.bottom() - 8 {
                break;
            }
            let power = self.fighter_for(pet, m).power();
            let plates = vec![vec![cell(tr("Power"), format_number(power as f64))]];
            let card = render_pet_card_phase(&rarities()[pet], m, self.state.phase(pet, m), card_w, card_h, &plates, 0, false);
            self.canvas.blit(&card, r.x, r.y);
            let picked = self.battle.picks.iter().position(|p| *p == (pet, m));
            if let Some(i) = picked {
                draw::rect(&mut self.canvas, accent(), r, 4, 14);
                let c = (r.right() - 16, r.y + 16);
                draw::circle(&mut self.canvas, accent(), c, 13, 0);
                let t = sb.render(&(i + 1).to_string(), BLACK);
                self.canvas.blit(&t, c.0 - t.w / 2, c.1 - t.h / 2);
            } else if r.collidepoint(mouse_pos) {
                draw::rect(&mut self.canvas, WHITE, r, 3, 14);
            }
            self.register_button(r, Rc::new(move |g: &mut Game| g.battle_toggle_pick(pet, m)), Some("click"));
        }
    }

    fn draw_battle_fight(&mut self, mouse_pos: (f64, f64)) {
        let w = 1000.min(self.vw - 40);
        let h = 700.min(VIRTUAL_H - 40);
        let rect = Rect::with_center(w, h, (self.vw / 2, VIRTUAL_H / 2 + 10));
        draw_panel(&mut self.canvas, rect, Some(panel()), 16, false, None);
        let arena = Rect::new(rect.x + 14, rect.y + 14, w - 28, 450);
        self.draw_battle_arena(arena);
        let bottom = Rect::new(rect.x + 14, arena.bottom() + 12, w - 28, rect.bottom() - 14 - (arena.bottom() + 12));
        self.draw_battle_controls(bottom, mouse_pos);
        if !self.battle.busy() {
            if let Some(reward) = self.battle.reward {
                self.draw_battle_result(arena, reward, mouse_pos);
            }
        }
    }

    fn draw_battle_arena(&mut self, arena: Rect) {
        let Some(b) = self.battle.battle.as_ref() else { return };
        let ui = &self.battle;
        let shown = [0usize, 1].map(|s| {
            ui.shown[s].map(|i| {
                let f = &b.teams[s][i];
                Shown { pet: f.pet, m: f.m, phase: f.phase }
            })
        });
        let st = Stage {
            t: ui.t,
            shown,
            send_t0: ui.send_t0,
            lunge_t0: ui.lunge_t0,
            lunge_special: ui.lunge_special,
            hit_t0: ui.hit_t0,
            hit_crit: ui.hit_crit,
            heal_t0: ui.heal_t0,
            guard_t0: ui.guard_t0,
            faint_t0: ui.faint_t0,
            shot: ui.shot,
            shot_t0: ui.shot_t0,
            prev_shot: ui.prev_shot,
        };
        // the 3D arena is drawn at half size and scaled up without smoothing (crisp, blocky pixels)
        let (lw, lh) = ((arena.w / 2).max(8) as usize, (arena.h / 2).max(8) as usize);
        let (img, marks) = draw_arena_3d(&st, lw, lh);
        let big = transform::scale(&img, arena.w, arena.h);
        self.canvas.blit(&big, arena.x, arena.y);
        // a white flash on every hit (brighter on a critical one)
        for side in 0..2 {
            let d = ui.t - ui.hit_t0[side];
            if (0.0..0.16).contains(&d) {
                let mut flash = crate::gfx::Surface::new_alpha(arena.w, arena.h);
                flash.fill(Color::rgba(255, 255, 255, 255), None);
                flash.set_alpha(((1.0 - d / 0.16) * if ui.hit_crit[side] { 190.0 } else { 120.0 }) as i32);
                self.canvas.blit(&flash, arena.x, arena.y);
            }
        }
        draw::rect(&mut self.canvas, outline(), arena, 3, 12);
        // damage numbers float up from whoever got hit
        let big_font = self.f.big.clone();
        for side in 0..2 {
            let d = ui.t - ui.dmg_t0[side];
            let (Some((hx, hy)), true) = (marks.head[side], (0.0..1.2).contains(&d) && ui.last_dmg[side] > 0) else { continue };
            let crit = ui.hit_crit[side];
            let txt = format!("-{}", ui.last_dmg[side]);
            let col = if crit { Color::rgb(255, 220, 60) } else { Color::rgb(255, 90, 70) };
            let mut face = (*big_font.render(&txt, col)).clone();
            let mut edge = (*big_font.render(&txt, Color::rgb(20, 14, 20))).clone();
            let a = ((1.2 - d) / 0.4).clamp(0.0, 1.0);
            face.set_alpha((a * 255.0) as i32);
            edge.set_alpha((a * 255.0) as i32);
            let x = arena.x + (hx * 2.0) as i32 - face.w / 2;
            // kept below the rival's name box and inside the arena
            let y = (arena.y + (hy * 2.0) as i32 - face.h).clamp(arena.y + 120, arena.bottom() - 150) - (d * 50.0) as i32;
            for (ox, oy) in [(-2, 0), (2, 0), (0, -2), (0, 2)] {
                self.canvas.blit(&edge, x + ox, y + oy);
            }
            self.canvas.blit(&face, x, y);
        }
        // name boxes
        let ui = &self.battle;
        let Some(b) = ui.battle.as_ref() else { return };
        let boxes = [Rect::new(arena.right() - 24 - 360, arena.bottom() - 24 - 104, 360, 104), Rect::new(arena.x + 24, arena.y + 24, 340, 88)];
        let infos: Vec<(usize, Fighter, f64, Vec<Fighter>)> =
            [1usize, 0].iter().filter_map(|&side| ui.shown[side].map(|idx| (side, b.teams[side][idx].clone(), ui.shown_hp(side), b.teams[side].clone()))).collect();
        for (side, f, hp, team) in infos {
            self.draw_battle_namebox(boxes[side], &f, hp, side == 0, &team);
        }
    }

    fn draw_battle_namebox(&mut self, r: Rect, f: &Fighter, hp: f64, mine: bool, team: &[Fighter]) {
        draw_panel(&mut self.canvas, r, Some(Color::rgb(248, 246, 236)), 12, false, None);
        draw::rect(&mut self.canvas, Color::rgb(60, 64, 80), r, 3, 12);
        let sb = self.f.small_b.clone();
        let tiny_b = self.f.tiny_b.clone();
        let name = sb.render(&fit_text(&sb, &f.name(), r.w - 120), Color::rgb(30, 32, 44));
        self.canvas.blit(&name, r.x + 14, r.y + 10);
        let rarity = &rarities()[f.pet];
        let tag = if f.phase > 0 { format!("{} · {}", tr(rarity.name), tr(PHASES[f.phase].name)) } else { tr(rarity.name) };
        let tag_col = if f.phase >= 3 { PHASES[3].color } else { crate::ui::drawing::shade(rarity_glow_color(rarity.key), 0.7) };
        let tt = tiny_b.render(&tag, tag_col);
        self.canvas.blit(&tt, r.right() - 14 - tt.w, r.y + 14);
        // HP bar
        let bar = Rect::new(r.x + 50, r.y + 44, r.w - 64, 14);
        let hpt = tiny_b.render("HP", Color::rgb(230, 160, 40));
        self.canvas.blit(&hpt, r.x + 16, bar.y - 1);
        draw::rect(&mut self.canvas, Color::rgb(50, 54, 66), bar.inflate(4, 4), 0, 8);
        let frac = (hp / f.max_hp as f64).clamp(0.0, 1.0);
        let fill = Rect::new(bar.x, bar.y, (bar.w as f64 * frac) as i32, bar.h);
        if fill.w > 0 {
            draw::rect(&mut self.canvas, hp_color(frac), fill, 0, 6);
        }
        // the team: one dot per verity (grey = fainted)
        for (i, m) in team.iter().enumerate() {
            let c = (r.x + 20 + i as i32 * 20, r.bottom() - 16);
            let col = if m.fainted() { Color::rgb(150, 150, 160) } else { Color::rgb(235, 80, 80) };
            draw::circle(&mut self.canvas, col, c, 7, 0);
            draw::circle(&mut self.canvas, Color::rgb(40, 40, 50), c, 7, 2);
        }
        if mine {
            let t = sb.render(&format!("{} / {}", hp.round() as i64, f.max_hp), Color::rgb(30, 32, 44));
            self.canvas.blit(&t, r.right() - 14 - t.w, r.bottom() - 12 - t.h);
        }
    }

    fn draw_battle_controls(&mut self, area: Rect, mouse_pos: (f64, f64)) {
        let Some(b) = self.battle.battle.as_ref() else { return };
        let me = b.fighter(0).clone();
        let team = b.teams[0].clone();
        let active = b.active[0];
        let forced = b.needs_switch();
        let over = b.winner.is_some();
        let busy = self.battle.busy();
        // the text box
        let menu_w = 420;
        let text_box = Rect::new(area.x, area.y, area.w - menu_w - 12, area.h);
        draw_panel(&mut self.canvas, text_box, Some(Color::rgb(34, 38, 56)), 12, false, None);
        draw::rect(&mut self.canvas, Color::rgb(210, 200, 170), text_box.inflate(-6, -6), 3, 10);
        let med = self.f.med.clone();
        let small = self.f.small.clone();
        let sb = self.f.small_b.clone();
        let tiny = self.f.tiny.clone();
        let text = if !busy && !over && !forced && self.battle.menu == "fight" {
            match self.battle.hover_move {
                Some(mv) => format!("{}: {}", move_name(&me, mv), move_info(mv)),
                None => tr!("What will %s do?", me.name()),
            }
        } else if !busy && forced {
            tr("Choose your next Verity!")
        } else if !busy && !over && self.battle.menu == "main" {
            tr!("What will %s do?", me.name())
        } else {
            self.battle.text.clone()
        };
        let mut y = text_box.y + 22;
        for line in wrap_text(&text, &med, text_box.w - 44).iter().take(3) {
            let t = med.render(line, WHITE);
            self.canvas.blit(&t, text_box.x + 22, y);
            y += t.h + 4;
        }
        if busy {
            self.register_button(text_box, Rc::new(|g: &mut Game| g.battle_skip()), None);
            let hint = tiny.render(&tr("click to skip"), grey_dim());
            self.canvas.blit(&hint, text_box.right() - 16 - hint.w, text_box.bottom() - 14 - hint.h);
        }
        // the menu
        let menu = Rect::new(text_box.right() + 12, area.y, menu_w, area.h);
        draw_panel(&mut self.canvas, menu, Some(Color::rgb(34, 38, 56)), 12, false, None);
        if busy || over {
            return;
        }
        let inner = menu.inflate(-16, -16);
        let gap = 10;
        let half_w = (inner.w - gap) / 2;
        let half_h = (inner.h - gap) / 2;
        let cell_at = |i: i32| Rect::new(inner.x + (i % 2) * (half_w + gap), inner.y + (i / 2) * (half_h + gap), half_w, half_h);
        let menu_name = if forced { "switch" } else { self.battle.menu };
        match menu_name {
            "fight" => {
                self.battle.hover_move = None;
                for (i, mv) in MOVES.iter().enumerate() {
                    let mv = *mv;
                    let r = cell_at(i as i32);
                    let ok = me.can_use(mv);
                    let label = move_name(&me, mv);
                    let base = match mv {
                        Move::Strike => Color::rgb(90, 96, 120),
                        Move::Special => Color::rgb(170, 70, 190),
                        Move::Guard => Color::rgb(60, 130, 190),
                        Move::Rest => Color::rgb(60, 150, 90),
                    };
                    if r.collidepoint(mouse_pos) {
                        self.battle.hover_move = Some(mv);
                    }
                    self.button(r, &fit_text(&sb, &label, r.w - 12), &sb, mouse_pos, base, crate::ui::drawing::shade(base, 1.25), WHITE, if ok { cb(move |g| g.battle_use(mv)) } else { None }, Bo::r(10).enabled(ok));
                    if let Some(pp) = me.pp(mv) {
                        let max = if mv == Move::Special { crate::core::battle::SPECIAL_PP } else { crate::core::battle::REST_PP };
                        let t = tiny.render(&format!("{}/{}", pp, max), if pp > 0 { WHITE } else { BAD });
                        self.canvas.blit(&t, r.right() - 8 - t.w, r.bottom() - 6 - t.h);
                    }
                }
                let back = Rect::new(menu.right() - 70, menu.y - 30, 70, 26);
                self.button(back, &tr("Back"), &tiny, mouse_pos, panel_light(), panel_lighter(), WHITE, cb(|g| g.battle.menu = "main"), Bo::r(8));
            }
            "switch" => {
                let rows = team.len() as i32;
                let row_h = ((inner.h - gap * (rows - 1)) / rows.max(1)).min(44);
                for (i, f) in team.iter().enumerate() {
                    let r = Rect::new(inner.x, inner.y + i as i32 * (row_h + gap), inner.w, row_h);
                    let ok = i != active && !f.fainted();
                    let label = if f.fainted() {
                        tr!("%s (fainted)", f.name())
                    } else if i == active {
                        tr!("%s (in battle)", f.name())
                    } else {
                        format!("{}   {}/{}", f.name(), f.hp, f.max_hp)
                    };
                    self.button(r, &fit_text(&small, &label, r.w - 16), &small, mouse_pos, panel_light(), panel_lighter(), WHITE, if ok { cb(move |g| g.battle_switch(i)) } else { None }, Bo::r(9).enabled(ok));
                }
                if !forced {
                    let back = Rect::new(menu.right() - 70, menu.y - 30, 70, 26);
                    self.button(back, &tr("Back"), &tiny, mouse_pos, panel_light(), panel_lighter(), WHITE, cb(|g| g.battle.menu = "main"), Bo::r(8));
                }
            }
            _ => {
                let big = self.f.big.clone();
                let top = Rect::new(inner.x, inner.y, inner.w, half_h);
                self.button(top, &tr("FIGHT"), &big, mouse_pos, BAD, Color::rgb(250, 110, 110), WHITE, cb(|g| g.battle.menu = "fight"), Bo::r(12).icon("battle"));
                let can_switch = team.iter().enumerate().any(|(i, f)| i != active && !f.fainted());
                self.button(cell_at(2), &tr("SWITCH"), &med, mouse_pos, Color::rgb(60, 130, 190), Color::rgb(90, 160, 220), WHITE, if can_switch { cb(|g| g.battle.menu = "switch") } else { None }, Bo::r(10).enabled(can_switch));
                self.button(cell_at(3), &tr("RUN"), &med, mouse_pos, panel_light(), panel_lighter(), WHITE, cb(|g| g.battle_run()), Bo::r(10));
            }
        }
    }

    fn draw_battle_result(&mut self, arena: Rect, reward: f64, mouse_pos: (f64, f64)) {
        let won = reward > 0.0;
        let veil = dim_overlay(arena.w, arena.h, 150);
        self.canvas.blit(&veil, arena.x, arena.y);
        let r = Rect::with_center(460, 230, arena.center());
        draw_panel(&mut self.canvas, r, Some(panel()), 16, true, None);
        let title = self.f.big.render(&if won { tr("You won!") } else { tr("You lost...") }, if won { accent() } else { BAD });
        let scale = 1.4;
        let title = transform::smoothscale(&title, (title.w as f64 * scale) as i32, (title.h as f64 * scale) as i32);
        self.canvas.blit(&title, r.centerx() - title.w / 2, r.y + 22);
        let msg = if won { tr!("+$%s for winning!", format_number(reward)) } else { tr("Train your Verities and try again!") };
        let t = self.f.med.render(&msg, if won { GOOD } else { grey() });
        self.canvas.blit(&t, r.centerx() - t.w / 2, r.y + 36 + title.h);
        let sb = self.f.small_b.clone();
        let bw = (r.w - 60) / 2;
        self.button(Rect::new(r.x + 20, r.bottom() - 66, bw, 46), &tr("Battle again"), &sb, mouse_pos, BAD, Color::rgb(250, 110, 110), WHITE, cb(|g| g.start_battle()), Bo::r(10).icon("battle"));
        self.button(Rect::new(r.right() - 20 - bw, r.bottom() - 66, bw, 46), &tr("Change team"), &sb, mouse_pos, panel_light(), panel_lighter(), WHITE, cb(|g| g.battle_run_quiet()), Bo::r(10));
    }

    /// Back to the team picker after a battle (no "ran away" message).
    pub fn battle_run_quiet(&mut self) {
        self.battle.stage = "pick";
        self.battle.battle = None;
        self.battle.queue.clear();
        self.battle.cur = None;
    }
}
