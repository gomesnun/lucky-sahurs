//! Battle page (first version, Pokemon style): pick up to 3 of your verities, then fight a rival team turn by turn.
//! The rules are in core/battle.rs; this page plays back its events (text, lunges, hits, HP bars, faints) one at a
//! time. For now the rival is played by the computer.

use super::base::Bo;
use super::{Game, KeyEv, cb};
use crate::config::VIRTUAL_H;
use super::battle_online::{OnlineMatch, PATIENCE, RankedUi, letter_of, tier_of};
use crate::online::firebase::BattleDoc;
use super::battle3d::{TRANSFORM_FLASH, TRANSFORM_TIME, SEND_FALL, SPECIAL_ARRIVE, STRIKE_ARRIVE, Shot, Shown, Stage, draw_arena_3d};
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
    /// "hub" (the 3 modes: Explore, Ranked, Fight a friend), "pick" (choosing the team) or "fight"
    pub stage: &'static str,
    /// the hub's clock (its previews move)
    hub_t: f64,
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
    /// the form each side's verity is shown in, its rage meter (shown value, animated) and when it transformed /
    /// calmed down / used a boost item
    disp_phase: [usize; 2],
    rage_from: [f64; 2],
    rage_to: [f64; 2],
    rage_t0: [f64; 2],
    transform_t0: [f64; 2],
    calm_t0: [f64; 2],
    boost_t0: [f64; 2],
    boost_color: [(f64, f64, f64); 2],
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
    hover_item: Option<crate::core::battle::Item>,
    seed: u64,
    /// the arena background, scaled to the arena (w, h, image)
    bg: Option<(i32, i32, crate::gfx::Surf)>,
    /// whether I won the battle that just ended
    won: bool,
    /// "cpu" (against the computer) or "online" (against friends)
    pub mode: &'static str,
    /// the online battle being played (game/battle_online.rs)
    pub online: Option<OnlineMatch>,
    /// challenges I got / sent that are still waiting
    pub received: Vec<BattleDoc>,
    pub sent: Vec<BattleDoc>,
    pub ch_loading: bool,
    pub ch_loaded: bool,
    pub ch_next: f64,
    pub ch_msg: Option<(String, Color)>,
    /// a challenge being sent / accepted / dropped (its uid or id)
    pub ch_action: Option<String>,
    pub ranked: RankedUi,
    /// how much a ranked battle moved my rating
    rank_delta: Option<i64>,
    /// a wild Verity Pet encounter from Explore (game/explore.rs): the index into explore.wild being fought.
    /// Win it to catch the pet; None for every other kind of battle.
    pub wild: Option<usize>,
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
            disp_phase: [0; 2],
            rage_from: [0.0; 2],
            rage_to: [0.0; 2],
            rage_t0: [far; 2],
            transform_t0: [far; 2],
            calm_t0: [far; 2],
            boost_t0: [far; 2],
            boost_color: [(255.0, 255.0, 255.0); 2],
            hit_crit: [false; 2],
            last_dmg: [0; 2],
            dmg_t0: [far; 2],
            shot: Shot::Idle,
            shot_t0: 0.0,
            prev_shot: Shot::Idle,
            reward: None,
            hover_move: None,
            hover_item: None,
            seed: 1,
            bg: None,
            won: false,
            mode: "cpu",
            hub_t: 0.0,
            online: None,
            received: Vec::new(),
            sent: Vec::new(),
            ch_loading: false,
            ch_loaded: false,
            ch_next: 0.0,
            ch_msg: None,
            ch_action: None,
            ranked: RankedUi::default(),
            rank_delta: None,
            wild: None,
        }
    }

    /// Still showing something (the menu waits).
    pub fn busy(&self) -> bool {
        self.cur.is_some() || !self.queue.is_empty()
    }

    pub fn shown_rage(&self, side: usize) -> f64 {
        let p = ((self.t - self.rage_t0[side]) / 0.5).clamp(0.0, 1.0);
        self.rage_from[side] + (self.rage_to[side] - self.rage_from[side]) * ease_out_cubic(p)
    }

    /// The form to draw a side's verity in right now (a transformation shows the Monster only after its flash).
    pub fn shown_phase(&self, side: usize) -> usize {
        let d = self.t - self.transform_t0[side];
        if (0.0..TRANSFORM_FLASH).contains(&d) { crate::core::battle::CALM_PHASE } else { self.disp_phase[side] }
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
        Ev::Transform { .. } => TRANSFORM_TIME,
        Ev::Calm { .. } => 0.9,
        Ev::Rage { .. } => 0.0,
        Ev::UseItem { .. } => 0.9,
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
        self.battle.stage = "hub";
        self.battle.battle = None;
        let owned: Vec<(usize, &'static str)> = self.battle_choices();
        self.battle.picks.retain(|p| owned.contains(p));
        if self.battle.picks.is_empty() {
            self.battle_auto_pick();
        }
    }

    /// From the hub: the team picker for a mode ("cpu", "online" or "ranked").
    pub fn battle_mode(&mut self, mode: &'static str) {
        self.battle.stage = "pick";
        self.battle.mode = mode;
        if mode == "online" {
            self.refresh_battles(true);
            self.refresh_friends(false);
        }
        if mode == "ranked" {
            self.refresh_top(false);
        }
    }

    /// Back to the hub (from the team picker).
    pub fn battle_back_to_hub(&mut self) {
        self.cancel_search();
        self.battle.stage = "hub";
    }

    pub fn close_battle(&mut self) {
        if self.explore.open {
            self.close_explore();
        }
        self.cancel_search();
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
        Fighter::battle(pet, m, self.state.phase(pet, m))
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
        self.battle.online = None;
        self.begin_battle_playback(b, evs);
    }

    /// Shows a new battle from its first events.
    pub fn begin_battle_playback(&mut self, b: Battle, evs: Vec<Ev>) {
        let ui = &mut self.battle;
        ui.rank_delta = None;
        ui.rage_from = [0.0; 2];
        ui.rage_to = [0.0; 2];
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

    pub fn battle_push(&mut self, evs: Vec<Ev>) {
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
        if !b.fighter(b.me).can_use(mv) {
            return;
        }
        if self.battle.online.is_some() {
            self.online_send(letter_of(mv));
            return;
        }
        let evs = b.turn(Action::Use(mv));
        self.battle_push(evs);
    }

    /// Uses a battle item (it takes the turn; one comes out of the Bag).
    pub fn battle_item(&mut self, item: crate::core::battle::Item) {
        if self.battle.busy() || self.state.battle_items.get(item.key()).copied().unwrap_or(0) <= 0 {
            return;
        }
        if self.battle.online.is_some() {
            if !self.online_my_turn() {
                return;
            }
            self.online_send(item.letter());
        } else {
            let Some(b) = self.battle.battle.as_mut() else { return };
            let evs = b.turn(Action::Item(item));
            self.battle_push(evs);
        }
        if let Some(n) = self.state.battle_items.get_mut(item.key()) {
            *n -= 1;
        }
        self.state.dirty = true;
    }

    /// Full rage: into the Monster form.
    pub fn battle_transform(&mut self) {
        if self.battle.busy() {
            return;
        }
        let Some(b) = self.battle.battle.as_mut() else { return };
        if !b.fighter(b.me).can_transform() {
            return;
        }
        if self.battle.online.is_some() {
            self.online_send('T');
            return;
        }
        let evs = b.turn(Action::Transform);
        self.battle_push(evs);
    }

    pub fn battle_switch(&mut self, idx: usize) {
        if self.battle.busy() {
            return;
        }
        let Some(b) = self.battle.battle.as_mut() else { return };
        if self.battle.online.is_some() {
            if b.can_switch_to(b.me, idx) && idx < 10 {
                self.online_send(char::from(b'0' + idx as u8));
            }
            return;
        }
        let evs = if b.needs_switch() { b.send_out(idx) } else if b.can_switch_to(0, idx) { b.turn(Action::Switch(idx)) } else { Vec::new() };
        if !evs.is_empty() {
            self.battle_push(evs);
        }
    }

    pub fn battle_run(&mut self) {
        if self.battle.online.is_some() {
            self.leave_online_battle();
            return;
        }
        if self.battle.wild.is_some() {
            // running from a wild Verity Pet: it gets away, straight back to Explore (no team picker to go to)
            self.battle.wild = None;
            self.battle.battle = None;
            self.battle.queue.clear();
            self.battle.cur = None;
            self.show_toast(&tr("You ran away safely."), 1.8);
            return;
        }
        self.battle.stage = "pick";
        self.battle.battle = None;
        self.battle.queue.clear();
        self.battle.cur = None;
        self.show_toast(&tr("You ran away safely."), 1.8);
    }

    /// Explore (game/explore.rs): a wild Verity Pet fights back. Win to catch it - explore_catch calls this
    /// instead of catching it outright, whenever you have at least one Verity to send out.
    pub fn start_wild_battle(&mut self, wild_idx: usize) {
        if self.battle.wild.is_some() {
            return;
        }
        let Some(w) = self.explore_wild_pet(wild_idx) else { return };
        let phase = match w.stars {
            1 | 2 => 0,
            3 | 4 => 1,
            _ => 2,
        };
        let wild_fighter = Fighter::new(w.pet, "normal", phase);
        let team: Vec<Fighter> = self.battle_choices().into_iter().take(TEAM_SIZE).map(|(p, m)| self.fighter_for(p, m)).collect();
        if team.is_empty() {
            return;
        }
        self.battle.seed = self.battle.seed.wrapping_mul(6364136223846793005).wrapping_add((crate::core::state::now_ts() * 1000.0) as u64 | 1);
        let (b, evs) = Battle::new_match([team, vec![wild_fighter]], self.battle.seed, 0, tr("Wild"), true);
        self.battle.online = None;
        self.battle.open = true;
        self.battle.wild = Some(wild_idx);
        self.begin_battle_playback(b, evs);
    }

    /// Closes the wild encounter's result screen (Continue): back to walking around in Explore.
    pub fn battle_wild_continue(&mut self) {
        self.battle.wild = None;
        self.battle.battle = None;
        self.battle.reward = None;
        self.battle.queue.clear();
        self.battle.cur = None;
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
        self.tick_battle_online();
        if self.battle.open && self.battle.stage == "hub" {
            self.battle.hub_t += dt;
        }
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
                // "X used Y!": the camera already goes to the attacker, before the attack itself
                Ev::Say(_) => match ui.queue.front() {
                    Some(Ev::Lunge { side, special }) => Some(Shot::Attack(*side, *special)),
                    _ => None,
                },
                Ev::SendOut { side, .. } => Some(Shot::SendOut(*side)),
                // a monster's special is a dash, like a Strike
                Ev::Lunge { side, special } => Some(Shot::Attack(*side, *special)),
                Ev::Hit { side, .. } => Some(Shot::Impact(*side)),
                Ev::Heal { side, .. } => Some(Shot::Heal(*side)),
                Ev::Guard { side } => Some(Shot::Guard(*side)),
                Ev::Faint { side } => Some(Shot::Faint(*side)),
                Ev::Transform { side, .. } => Some(Shot::Transform(*side)),
                Ev::Calm { side } | Ev::UseItem { side, .. } => Some(Shot::Heal(*side)),
                Ev::Rage { .. } => None,
            };
            if let Some(sh) = new_shot {
                // the attack shot may have started on its "X used Y!" line already: keep it going
                if sh != ui.shot {
                    ui.prev_shot = ui.shot;
                    ui.shot = sh;
                    ui.shot_t0 = t;
                }
            }
            match &ev {
                Ev::Say(s) => ui.text = s.clone(),
                Ev::SendOut { side, idx, hp, phase } => {
                    ui.shown[*side] = Some(*idx);
                    ui.disp_phase[*side] = *phase;
                    ui.transform_t0[*side] = -1e9;
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
                Ev::Transform { side, hp } => {
                    ui.transform_t0[*side] = t;
                    ui.disp_phase[*side] = 3;
                    // the HP it gets back shows up at the flash
                    let now = ui.shown_hp(*side);
                    ui.hp_from[*side] = now;
                    ui.hp_to[*side] = *hp as f64;
                    ui.hp_t0[*side] = t + TRANSFORM_FLASH;
                }
                Ev::Calm { side } => {
                    ui.calm_t0[*side] = t;
                    ui.disp_phase[*side] = crate::core::battle::CALM_PHASE;
                }
                Ev::Rage { side, rage } => {
                    let now = ui.shown_rage(*side);
                    ui.rage_from[*side] = now;
                    ui.rage_to[*side] = *rage as f64;
                    ui.rage_t0[*side] = t;
                }
                Ev::UseItem { side, item } => {
                    use crate::core::battle::Item;
                    ui.boost_t0[*side] = t;
                    ui.boost_color[*side] = match item {
                        Item::Potion => (120.0, 255.0, 150.0),
                        Item::Power => (255.0, 110.0, 60.0),
                        Item::Iron => (150.0, 190.0, 255.0),
                        Item::Feather => (255.0, 245.0, 140.0),
                    };
                }
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
        // the battle is over and everything was shown: pay the win once (or, in Explore, catch the pet)
        if !self.battle.busy() && self.battle.reward.is_none() {
            let winner = self.battle.battle.as_ref().and_then(|b| b.winner.map(|w| w == b.me));
            if let Some(won) = winner {
                self.battle.won = won;
                if let Some(wild_idx) = self.battle.wild {
                    self.battle.reward = Some(0.0);
                    if won {
                        self.explore_finish_catch(wild_idx);
                    } else {
                        self.show_toast(&tr("It got away..."), 1.8);
                    }
                } else {
                    let ranked = self.battle.online.as_ref().is_some_and(|om| om.ranked);
                    self.battle.rank_delta = self.apply_ranked_result(won);
                    let reward = if won { (self.state.income_per_second() * if ranked { 600.0 } else { 300.0 }).max(100.0).round() } else { 0.0 };
                    if reward > 0.0 {
                        self.state.coins += reward;
                        self.state.total_coins_earned += reward;
                        self.state.battles_won += 1;
                        self.state.dirty = true;
                    }
                    self.battle.reward = Some(reward);
                }
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
        if self.explore.open && self.battle.wild.is_none() {
            return self.handle_explore_key(ev);
        }
        if self.battle.stage == "hub" {
            if ev.key == K::Escape {
                self.close_battle();
            }
            return true;
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
                    self.battle_back_to_hub();
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
        if self.explore.open && self.battle.wild.is_none() {
            self.draw_explore(mouse_pos);
            return;
        }
        // the game isn't drawn under the battle (see draw_game_screen): a plain dark backdrop is enough, and cheap
        draw::rect(&mut self.canvas, Color::rgb(12, 13, 22), Rect::new(0, crate::config::TOPBAR_H, self.vw, VIRTUAL_H - crate::config::TOPBAR_H), 0, 0);
        // nothing behind the battle can be clicked
        self.buttons.clear();
        self.register_button(Rect::new(0, 0, self.vw, VIRTUAL_H), Rc::new(|_: &mut Game| {}), None);
        if self.battle.stage == "fight" && self.battle.battle.is_some() {
            self.draw_battle_fight(mouse_pos);
        } else if self.battle.stage == "hub" {
            self.draw_battle_hub(mouse_pos);
        } else {
            self.draw_battle_pick(mouse_pos);
        }
    }

    /// The hub: three big buttons, each with a live preview of its mode - Explore, Ranked, Fight a friend.
    fn draw_battle_hub(&mut self, mouse_pos: (f64, f64)) {
        let w = 1140.min(self.vw - 40);
        let h = 660.min(VIRTUAL_H - crate::config::TOPBAR_H - 30);
        let rect = Rect::with_center(w, h, (self.vw / 2, (VIRTUAL_H + crate::config::TOPBAR_H) / 2));
        draw_panel(&mut self.canvas, rect, Some(panel()), 16, true, None);
        let pad = 24;
        let sb = self.f.small_b.clone();
        let small = self.f.small.clone();
        let title = self.f.big.render(&tr("Verity Battle"), WHITE);
        self.canvas.blit(&title, rect.x + pad, rect.y + 18);
        let sub = small.render(&tr("Pick a mode"), grey());
        self.canvas.blit(&sub, rect.x + pad + 2, rect.y + 20 + title.h);
        self.button(Rect::new(rect.right() - 48, rect.y + 20, 30, 30), "X", &sb, mouse_pos, panel_light(), BAD, WHITE, cb(|g| g.close_battle()), Bo::r(8));
        let t = self.battle.hub_t;
        let gap = 18;
        let top = rect.y + 92;
        let cw = (w - pad * 2 - gap * 2) / 3;
        let ch = rect.bottom() - pad - top;
        // your strongest verities (the arena previews show them)
        let best: Vec<Shown> = self
            .battle_choices()
            .into_iter()
            .take(2)
            .map(|(pet, m)| Shown { pet, m, phase: self.state.phase(pet, m).min(3) })
            .collect();
        let mine = best.first().copied().unwrap_or(Shown { pet: 0, m: "normal", phase: 0 });
        let theirs = best.get(1).copied().unwrap_or(Shown { pet: 13, m: "normal", phase: 1 });
        let ex = self.state.explore.clone();
        let (rank_name, rank_color) = tier_of(self.state.rank_rating);
        let modes: [(&str, String, String, String, Color); 3] = [
            ("explore", tr("Explore"), tr("Walk Steve through 3D worlds and catch Verity Pets. Equip them for money and luck boosts!"), tr!("Level %d  ·  %d pets", ex.level(), ex.pets.len() as i64), Color::rgb(90, 200, 110)),
            ("ranked", tr("Ranked"), tr("Get matched with a player of your level. Win to climb from Bronze to Master."), tr!("%s  ·  %d rating", tr(rank_name), self.state.rank_rating), rank_color),
            ("online", tr("Fight a Friend"), tr("Challenge a friend to a battle, or practice against the computer."), tr!("Battles won: %s", format_number(self.state.battles_won as f64)), Color::rgb(235, 110, 90)),
        ];
        for (i, (key, name, desc, info, color)) in modes.into_iter().enumerate() {
            let r = Rect::new(rect.x + pad + i as i32 * (cw + gap), top, cw, ch);
            let hover = r.collidepoint(mouse_pos);
            let lift = if hover { -4 } else { 0 };
            let r = Rect::new(r.x, r.y + lift, r.w, r.h);
            draw::rect(&mut self.canvas, panel_light(), r, 0, 16);
            // the preview: a little live 3D scene of the mode
            let pv = Rect::new(r.x + 10, r.y + 10, r.w - 20, (r.h as f64 * 0.56) as i32);
            let (lw, lh) = ((pv.w / 2).max(8) as usize, (pv.h / 2).max(8) as usize);
            let img = match key {
                "explore" => {
                    let look = super::explore::SteveLook { shirt: ex.shirt, pants: ex.pants, hat: ex.hat };
                    let eq = ex.equipped.iter().filter_map(|&k| ex.pets.get(k)).map(|p| Shown { pet: p.pet, m: "normal", phase: 0 }).collect();
                    super::explore::explore_preview(look, eq, t, lw, lh)
                }
                _ => {
                    // the arena: in Ranked they stare each other down, with a friend they trade blows
                    let cycle = 3.2;
                    let k = t % cycle;
                    let t0 = t - k;
                    let fight = key == "online";
                    let side_hit = ((t / cycle) as i64 % 2) as usize;
                    let st = Stage {
                        t,
                        shown: [Some(mine), Some(theirs)],
                        send_t0: [-99.0; 2],
                        lunge_t0: if fight { if side_hit == 0 { [t0 + 0.3, -99.0] } else { [-99.0, t0 + 0.3] } } else { [-99.0; 2] },
                        lunge_special: [side_hit == 1; 2],
                        hit_t0: if fight { if side_hit == 0 { [-99.0, t0 + 0.3 + STRIKE_ARRIVE] } else { [t0 + 0.3 + SPECIAL_ARRIVE, -99.0] } } else { [-99.0; 2] },
                        hit_crit: [false; 2],
                        heal_t0: [-99.0; 2],
                        guard_t0: [-99.0; 2],
                        faint_t0: [None; 2],
                        transform_t0: [-99.0; 2],
                        calm_t0: [-99.0; 2],
                        boost_t0: [-99.0; 2],
                        boost_color: [(255.0, 255.0, 255.0); 2],
                        shot: Shot::Idle,
                        shot_t0: -99.0,
                        prev_shot: Shot::Idle,
                    };
                    draw_arena_3d(&st, lw, lh).0
                }
            };
            let img = transform::scale(&img, pv.w, pv.h);
            self.canvas.blit(&img, pv.x, pv.y);
            draw::rect(&mut self.canvas, color, pv, 3, 12);
            if key == "ranked" {
                // the tier badge over the preview
                let badge = self.f.med.render(&tr(rank_name), BLACK);
                let b = Rect::new(pv.x + 12, pv.y + 12, badge.w + 24, badge.h + 10);
                draw::rect(&mut self.canvas, rank_color, b, 0, 10);
                self.canvas.blit(&badge, b.x + 12, b.y + 5);
            }
            // the name, what it is, a line about you
            let nt = self.f.big.render(&name, WHITE);
            self.canvas.blit(&nt, r.x + 18, pv.bottom() + 12);
            let mut y = pv.bottom() + 16 + nt.h;
            for line in wrap_text(&desc, &small, r.w - 36) {
                let l = small.render(&line, grey());
                self.canvas.blit(&l, r.x + 18, y);
                y += l.h + 2;
            }
            let it = sb.render(&info, color);
            self.canvas.blit(&it, r.x + 18, r.bottom() - 18 - it.h);
            draw::rect(&mut self.canvas, if hover { color } else { panel_lighter() }, r, if hover { 4 } else { 2 }, 16);
            let k: &'static str = key;
            self.register_button(
                r,
                Rc::new(move |g: &mut Game| {
                    if k == "explore" {
                        g.open_explore();
                    } else {
                        g.battle_mode(k);
                    }
                }),
                Some("click"),
            );
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
        self.button(Rect::new(rect.x + pad, rect.y + 20, 40, 36), "<", &sb, mouse_pos, panel_light(), panel_lighter(), WHITE, cb(|g| g.battle_back_to_hub()), Bo::r(9));
        let title = self.f.big.render(&tr("Battle"), WHITE);
        self.canvas.blit(&title, rect.x + pad + 52, rect.y + 18);
        let sub = small.render(&tr!("Pick up to %d of your Verities. Monsters and rare mutations hit the hardest.", TEAM_SIZE as i64), grey());
        self.canvas.blit(&sub, rect.x + pad + 2, rect.y + 20 + title.h);
        self.button(Rect::new(rect.right() - 48, rect.y + 20, 30, 30), "X", &sb, mouse_pos, panel_light(), BAD, WHITE, cb(|g| g.close_battle()), Bo::r(8));
        // vs Computer / vs Friends / Ranked
        let online = self.battle.mode != "cpu";
        for (i, (key, label)) in [("cpu", tr("vs Computer")), ("online", tr("vs Friends")), ("ranked", tr("Ranked"))].iter().enumerate() {
            let r = Rect::new(rect.right() - 60 - 3 * 150 + i as i32 * 150, rect.y + 20, 142, 34);
            let active = self.battle.mode == *key;
            let k: &'static str = key;
            self.button(r, label, &sb, mouse_pos, if active { accent() } else { panel_light() }, if active { accent() } else { panel_lighter() }, if active { BLACK } else { WHITE }, cb(move |g| {
                g.battle.mode = k;
                if k == "online" {
                    g.refresh_battles(true);
                    g.refresh_friends(false);
                }
                if k == "ranked" {
                    g.refresh_top(false);
                }
            }), Bo::r(9));
        }
        if self.state.battles_won > 0 {
            let t = self.f.tiny_b.render(&tr!("Battles won: %s", format_number(self.state.battles_won as f64)), accent());
            self.canvas.blit(&t, rect.right() - 60 - t.w, rect.y + 60);
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
        if online {
            let hint = small.render(&if self.battle.mode == "ranked" { tr("Pick your team, then find a match on the right.") } else { tr("Pick your team, then challenge a friend on the right.") }, grey());
            self.canvas.blit(&hint, bx, ty + 24 + 56 + 16);
        } else {
            self.button(Rect::new(bx, ty + 24 + 56, bw, 56), &tr("Fight!"), &self.f.big.clone(), mouse_pos, BAD, Color::rgb(250, 110, 110), WHITE, if can_fight { cb(|g| g.start_battle()) } else { None }, Bo::r(12).enabled(can_fight).icon("battle"));
        }

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
        // online: the grid makes room for the friends / challenges column
        let side_w = if online { 330 } else { 0 };
        if online {
            let col = Rect::new(rect.right() - pad - side_w, gy, side_w, rect.bottom() - pad - gy);
            if self.battle.mode == "ranked" {
                self.draw_battle_ranked(col, mouse_pos);
            } else {
                self.draw_battle_friends(col, mouse_pos);
            }
        }
        let cols = if online { 4 } else { 6 };
        let gap = 10;
        let card_w = (w - pad * 2 - side_w - if online { 16 } else { 0 } - gap * (cols - 1)) / cols;
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
        // the arena always shows MY verity near (its side 0): d() turns a battle side into an arena side
        let me = b.me;
        let d = move |s: usize| if me == 0 { s } else { 1 - s };
        let sw = |a: [f64; 2]| [a[d(0)], a[d(1)]];
        let shown = [d(0), d(1)].map(|s| {
            ui.shown[s].map(|i| {
                let f = &b.teams[s][i];
                Shown { pet: f.pet, m: f.m, phase: ui.shown_phase(s) }
            })
        });
        let shot = |sh: Shot| match sh {
            Shot::Idle => Shot::Idle,
            Shot::SendOut(s) => Shot::SendOut(d(s)),
            Shot::Attack(s, sp) => Shot::Attack(d(s), sp),
            Shot::Impact(s) => Shot::Impact(d(s)),
            Shot::Guard(s) => Shot::Guard(d(s)),
            Shot::Heal(s) => Shot::Heal(d(s)),
            Shot::Faint(s) => Shot::Faint(d(s)),
            Shot::Transform(s) => Shot::Transform(d(s)),
        };
        let st = Stage {
            t: ui.t,
            shown,
            send_t0: sw(ui.send_t0),
            lunge_t0: sw(ui.lunge_t0),
            lunge_special: [ui.lunge_special[d(0)], ui.lunge_special[d(1)]],
            hit_t0: sw(ui.hit_t0),
            hit_crit: [ui.hit_crit[d(0)], ui.hit_crit[d(1)]],
            heal_t0: sw(ui.heal_t0),
            guard_t0: sw(ui.guard_t0),
            faint_t0: [ui.faint_t0[d(0)], ui.faint_t0[d(1)]],
            transform_t0: sw(ui.transform_t0),
            calm_t0: sw(ui.calm_t0),
            boost_t0: sw(ui.boost_t0),
            boost_color: [ui.boost_color[d(0)], ui.boost_color[d(1)]],
            shot: shot(ui.shot),
            shot_t0: ui.shot_t0,
            prev_shot: shot(ui.prev_shot),
        };
        // the 3D arena is drawn at half size and scaled up without smoothing (crisp, blocky pixels)
        let (lw, lh) = ((arena.w / 2).max(8) as usize, (arena.h / 2).max(8) as usize);
        let (img, marks) = draw_arena_3d(&st, lw, lh);
        let big = transform::scale(&img, arena.w, arena.h);
        self.canvas.blit(&big, arena.x, arena.y);
        // a white flash when a Monster bursts out, and on every hit (brighter on a critical one)
        for side in 0..2 {
            let d = ui.t - ui.transform_t0[side] - TRANSFORM_FLASH;
            if (0.0..0.5).contains(&d) {
                let mut flash = crate::gfx::Surface::new_alpha(arena.w, arena.h);
                flash.fill(Color::rgba(255, 245, 235, 255), None);
                flash.set_alpha(((1.0 - d / 0.5) * 255.0) as i32);
                self.canvas.blit(&flash, arena.x, arena.y);
            }
        }
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
            let (Some((hx, hy)), true) = (marks.head[if me == 0 { side } else { 1 - side }], (0.0..1.2).contains(&d) && ui.last_dmg[side] > 0) else { continue };
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
        let boxes = [Rect::new(arena.right() - 24 - 360, arena.bottom() - 24 - 104, 360, 104), Rect::new(arena.x + 24, arena.y + 24, 340, 96)];
        let me = b.me;
        let infos: Vec<(usize, Fighter, f64, Vec<Fighter>, usize, f64)> = [1usize, 0]
            .iter()
            .filter_map(|&side| ui.shown[side].map(|idx| (side, b.teams[side][idx].clone(), ui.shown_hp(side), b.teams[side].clone(), ui.shown_phase(side), ui.shown_rage(side))))
            .collect();
        for (side, f, hp, team, phase, rage) in infos {
            let mine = side == me;
            self.draw_battle_namebox(boxes[if mine { 0 } else { 1 }], &f, hp, mine, &team, phase, rage);
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn draw_battle_namebox(&mut self, r: Rect, f: &Fighter, hp: f64, mine: bool, team: &[Fighter], phase: usize, rage: f64) {
        draw_panel(&mut self.canvas, r, Some(Color::rgb(248, 246, 236)), 12, false, None);
        draw::rect(&mut self.canvas, Color::rgb(60, 64, 80), r, 3, 12);
        let sb = self.f.small_b.clone();
        let tiny_b = self.f.tiny_b.clone();
        let name = sb.render(&fit_text(&sb, &f.name(), r.w - 120), Color::rgb(30, 32, 44));
        self.canvas.blit(&name, r.x + 14, r.y + 10);
        let rarity = &rarities()[f.pet];
        let tag = if phase > 0 { format!("{} · {}", tr(rarity.name), tr(PHASES[phase].name)) } else { tr(rarity.name) };
        let tag_col = if phase >= 3 { PHASES[3].color } else { crate::ui::drawing::shade(rarity_glow_color(rarity.key), 0.7) };
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
        // a Monster's rage meter (while calm) / how long it stays a Monster
        if f.monster {
            let bar = Rect::new(r.x + 50, r.y + 64, r.w - 64, 9);
            let label = tiny_b.render(&tr("RAGE"), Color::rgb(220, 50, 40));
            self.canvas.blit(&label, r.x + 12, bar.y - 3);
            draw::rect(&mut self.canvas, Color::rgb(50, 54, 66), bar.inflate(4, 4), 0, 6);
            let raging = phase >= 3;
            let frac = if raging { 1.0 } else { (rage / crate::core::battle::RAGE_MAX as f64).clamp(0.0, 1.0) };
            let full = frac >= 1.0;
            let pulse = 0.5 + 0.5 * (self.battle.t * 8.0).sin();
            let col = if raging { Color::rgb(255, 60, 40) } else if full { crate::ui::drawing::mix(Color::rgb(255, 70, 40), Color::rgb(255, 220, 120), pulse) } else { Color::rgb(230, 70, 50) };
            let fill = Rect::new(bar.x, bar.y, (bar.w as f64 * frac) as i32, bar.h);
            if fill.w > 0 {
                draw::rect(&mut self.canvas, col, fill, 0, 5);
            }
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
        let me = b.fighter(b.me).clone();
        let team = b.teams[b.me].clone();
        let active = b.active[b.me];
        let forced = b.needs_switch();
        let over = b.winner.is_some();
        // online: nothing to choose while it's the friend's move
        let waiting = self.battle.online.as_ref().map(|om| (om.friend.clone(), om.gone, crate::core::state::now_ts() - om.waiting_since));
        let busy = self.battle.busy() || (waiting.is_some() && !over && !self.online_my_turn());
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
        } else if !busy && !over && self.battle.menu == "bag" {
            match self.battle.hover_item {
                Some(it) => format!("{}: {}", it.name(), it.info()),
                None => tr("Use an item (it takes your turn)."),
            }
        } else if !busy && !over && self.battle.menu == "main" && me.can_transform() {
            tr!("%s is full of RAGE! Transform it into its Monster form for %d turns!", me.name(), crate::core::battle::MONSTER_TURNS as i64)
        } else if !busy && forced {
            tr("Choose your next Verity!")
        } else if !busy && !over && self.battle.menu == "main" {
            tr!("What will %s do?", me.name())
        } else if let (Some((friend, gone, _)), false, false) = (&waiting, self.battle.busy(), over) {
            let dots = ".".repeat(1 + (crate::core::state::now_ts() * 2.0) as usize % 3);
            if *gone { tr!("%s left the battle.", friend.clone()) } else { format!("{}{}", tr!("Waiting for %s", friend.clone()), dots) }
        } else {
            self.battle.text.clone()
        };
        let mut y = text_box.y + 22;
        for line in wrap_text(&text, &med, text_box.w - 44).iter().take(3) {
            let t = med.render(line, WHITE);
            self.canvas.blit(&t, text_box.x + 22, y);
            y += t.h + 4;
        }
        if self.battle.busy() {
            self.register_button(text_box, Rc::new(|g: &mut Game| g.battle_skip()), None);
            let hint = tiny.render(&tr("click to skip"), grey_dim());
            self.canvas.blit(&hint, text_box.right() - 16 - hint.w, text_box.bottom() - 14 - hint.h);
        }
        // the menu
        let menu = Rect::new(text_box.right() + 12, area.y, menu_w, area.h);
        draw_panel(&mut self.canvas, menu, Some(Color::rgb(34, 38, 56)), 12, false, None);
        // online, waiting too long for the friend (or they left): a way out
        if let (Some((_, gone, waited)), false, false) = (&waiting, self.battle.busy(), over) {
            if *gone || *waited > PATIENCE {
                let r = Rect::with_center(menu.w - 60, 50, menu.center());
                self.button(r, &tr("Leave"), &med, mouse_pos, panel_light(), panel_lighter(), WHITE, cb(|g| g.leave_online_battle()), Bo::r(10));
            }
        }
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
            "bag" => {
                self.battle.hover_item = None;
                for (i, it) in crate::core::battle::ITEMS.iter().enumerate() {
                    let it = *it;
                    let r = cell_at(i as i32);
                    let have = self.state.battle_items.get(it.key()).copied().unwrap_or(0);
                    let ok = have > 0;
                    if r.collidepoint(mouse_pos) {
                        self.battle.hover_item = Some(it);
                    }
                    self.button(r, &fit_text(&sb, &it.name(), r.w - 12), &sb, mouse_pos, Color::rgb(70, 76, 100), Color::rgb(96, 104, 136), WHITE, if ok { cb(move |g| g.battle_item(it)) } else { None }, Bo::r(10).enabled(ok));
                    let t = tiny.render(&format!("x{}", have), if ok { WHITE } else { BAD });
                    self.canvas.blit(&t, r.right() - 8 - t.w, r.bottom() - 6 - t.h);
                }
                let back = Rect::new(menu.right() - 70, menu.y - 30, 70, 26);
                self.button(back, &tr("Back"), &tiny, mouse_pos, panel_light(), panel_lighter(), WHITE, cb(|g| g.battle.menu = "main"), Bo::r(8));
            }
            _ => {
                // with a full rage meter: a big TRANSFORM button on top, the four choices below it
                let (cell_at, top_row) = if me.can_transform() {
                    let row_h = (inner.h - gap * 2) / 3;
                    let tr_rect = Rect::new(inner.x, inner.y, inner.w, row_h);
                    let pulse = 0.5 + 0.5 * (self.battle.t * 6.0).sin();
                    let col = crate::ui::drawing::mix(Color::rgb(200, 30, 30), Color::rgb(255, 110, 40), pulse);
                    let big = self.f.big.clone();
                    self.button(tr_rect, &tr("TRANSFORM!"), &big, mouse_pos, col, Color::rgb(255, 140, 60), WHITE, cb(|g| g.battle_transform()), Bo::r(12).border(Some(Color::rgb(255, 220, 120))));
                    let cell = move |i: i32| Rect::new(inner.x + (i % 2) * (half_w + gap), inner.y + (1 + i / 2) * (row_h + gap), half_w, row_h);
                    (Box::new(cell) as Box<dyn Fn(i32) -> Rect>, true)
                } else {
                    (Box::new(cell_at) as Box<dyn Fn(i32) -> Rect>, false)
                };
                let _ = top_row;
                let fight_font = if me.can_transform() { med.clone() } else { self.f.big.clone() };
                self.button(cell_at(0), &tr("FIGHT"), &fight_font, mouse_pos, BAD, Color::rgb(250, 110, 110), WHITE, cb(|g| g.battle.menu = "fight"), Bo::r(12).icon("battle"));
                let items: i64 = self.state.battle_items.values().sum();
                self.button(cell_at(1), &tr("BAG"), &med, mouse_pos, Color::rgb(170, 120, 50), Color::rgb(200, 150, 70), WHITE, cb(|g| g.battle.menu = "bag"), Bo::r(10).enabled(items > 0));
                let can_switch = team.iter().enumerate().any(|(i, f)| i != active && !f.fainted());
                self.button(cell_at(2), &tr("SWITCH"), &med, mouse_pos, Color::rgb(60, 130, 190), Color::rgb(90, 160, 220), WHITE, if can_switch { cb(|g| g.battle.menu = "switch") } else { None }, Bo::r(10).enabled(can_switch));
                let run = if self.battle.online.is_some() { tr("GIVE UP") } else { tr("RUN") };
                self.button(cell_at(3), &run, &med, mouse_pos, panel_light(), panel_lighter(), WHITE, cb(|g| g.battle_run()), Bo::r(10));
            }
        }
    }

    fn draw_battle_result(&mut self, arena: Rect, reward: f64, mouse_pos: (f64, f64)) {
        let won = self.battle.won;
        if self.battle.wild.is_some() {
            let veil = dim_overlay(arena.w, arena.h, 150);
            self.canvas.blit(&veil, arena.x, arena.y);
            let r = Rect::with_center(420, 200, arena.center());
            draw_panel(&mut self.canvas, r, Some(panel()), 16, true, None);
            let title_text = if won { tr("Caught it!") } else { tr("It got away...") };
            let title = self.f.big.render(&title_text, if won { accent() } else { BAD });
            let scale = 1.3;
            let title = transform::smoothscale(&title, (title.w as f64 * scale) as i32, (title.h as f64 * scale) as i32);
            self.canvas.blit(&title, r.centerx() - title.w / 2, r.y + 26);
            let msg = if won { tr("Check your Verity Pets in the Bag.") } else { tr("Train your Verities and try again!") };
            let t = self.f.med.render(&msg, if won { GOOD } else { grey() });
            self.canvas.blit(&t, r.centerx() - t.w / 2, r.y + 40 + title.h);
            let back = Rect::new(r.x + 20, r.bottom() - 66, r.w - 40, 46);
            self.button(back, &tr("Continue"), &self.f.small_b.clone(), mouse_pos, accent(), accent_hover(), BLACK, cb(|g| g.battle_wild_continue()), Bo::r(10));
            return;
        }
        let friend = self.battle.online.as_ref().map(|om| om.friend.clone());
        let veil = dim_overlay(arena.w, arena.h, 150);
        self.canvas.blit(&veil, arena.x, arena.y);
        let r = Rect::with_center(460, 230, arena.center());
        draw_panel(&mut self.canvas, r, Some(panel()), 16, true, None);
        let title_text = match (&friend, won) {
            (Some(f), true) => tr!("You beat %s!", f.clone()),
            (Some(f), false) => tr!("%s won...", f.clone()),
            (None, true) => tr("You won!"),
            (None, false) => tr("You lost..."),
        };
        let title = self.f.big.render(&fit_text(&self.f.big, &title_text, 300), if won { accent() } else { BAD });
        let scale = 1.4;
        let title = transform::smoothscale(&title, (title.w as f64 * scale) as i32, (title.h as f64 * scale) as i32);
        self.canvas.blit(&title, r.centerx() - title.w / 2, r.y + 22);
        let msg = if won && reward > 0.0 { tr!("+$%s for winning!", format_number(reward)) } else { tr("Train your Verities and try again!") };
        let t = self.f.med.render(&msg, if won { GOOD } else { grey() });
        self.canvas.blit(&t, r.centerx() - t.w / 2, r.y + 36 + title.h);
        if let Some(delta) = self.battle.rank_delta {
            let (tier, col) = tier_of(self.state.rank_rating);
            let line = format!("{}  ·  {} {}", tr!("Rating %s%d", if delta >= 0 { "+" } else { "" }, delta), tr(tier), self.state.rank_rating);
            let t = self.f.small_b.render(&line, col);
            self.canvas.blit(&t, r.centerx() - t.w / 2, r.y + 64 + title.h);
        }
        let sb = self.f.small_b.clone();
        let bw = (r.w - 60) / 2;
        if friend.is_some() {
            let back = Rect::new(r.x + 20, r.bottom() - 66, r.w - 40, 46);
            self.button(back, &tr("Back"), &sb, mouse_pos, panel_light(), panel_lighter(), WHITE, cb(|g| g.leave_online_battle()), Bo::r(10));
            return;
        }
        self.button(Rect::new(r.x + 20, r.bottom() - 66, bw, 46), &tr("Battle again"), &sb, mouse_pos, BAD, Color::rgb(250, 110, 110), WHITE, cb(|g| g.start_battle()), Bo::r(10).icon("battle"));
        self.button(Rect::new(r.right() - 20 - bw, r.bottom() - 66, bw, 46), &tr("Change team"), &sb, mouse_pos, panel_light(), panel_lighter(), WHITE, cb(|g| g.battle_run_quiet()), Bo::r(10));
    }

    /// "Ranked": my rank, Find match / searching, and the top 10.
    fn draw_battle_ranked(&mut self, col: Rect, mouse_pos: (f64, f64)) {
        draw_panel(&mut self.canvas, col, Some(panel_light()), 12, false, None);
        let inner = col.inflate(-24, -24);
        let (sb, small, tiny, med) = (self.f.small_b.clone(), self.f.small.clone(), self.f.tiny.clone(), self.f.med.clone());
        let mut y = inner.y;
        if self.account.is_none() {
            for line in wrap_text(&tr("Log in to your account (Account, on the title screen) to play ranked."), &small, inner.w) {
                let t = small.render(&line, grey());
                self.canvas.blit(&t, inner.x, y);
                y += 22;
            }
            return;
        }
        // my rank: tier badge, rating, wins / losses
        let (tier, tcol) = tier_of(self.state.rank_rating);
        let badge = Rect::new(inner.x, y, 64, 64);
        draw::circle(&mut self.canvas, crate::ui::drawing::shade(tcol, 0.55), badge.center(), 32, 0);
        draw::circle(&mut self.canvas, tcol, badge.center(), 32, 4);
        let initial = self.f.big.render(&tr(tier).chars().next().unwrap_or('?').to_string(), WHITE);
        self.canvas.blit(&initial, badge.centerx() - initial.w / 2, badge.centery() - initial.h / 2);
        let t = med.render(&tr(tier), tcol);
        self.canvas.blit(&t, inner.x + 78, y + 4);
        let t = small.render(&tr!("Rating %d  ·  %dW %dL", self.state.rank_rating, self.state.rank_wins, self.state.rank_losses), WHITE);
        self.canvas.blit(&t, inner.x + 78, y + 36);
        y += 80;
        // find a match / searching
        let has_team = !self.battle.picks.is_empty();
        let r = Rect::new(inner.x, y, inner.w, 50);
        if self.battle.ranked.searching {
            let secs = (crate::core::state::now_ts() - self.battle.ranked.since).max(0.0) as i64;
            let dots = ".".repeat(1 + (crate::core::state::now_ts() * 2.0) as usize % 3);
            let t = med.render(&format!("{}{}  {}:{:02}", tr("Searching"), dots, secs / 60, secs % 60), accent());
            self.canvas.blit(&t, inner.x, y + 4);
            self.button(Rect::new(inner.right() - 100, y + 4, 100, 38), &tr("Cancel"), &sb, mouse_pos, panel(), BAD, WHITE, cb(|g| g.cancel_search()), Bo::r(8));
        } else {
            self.button(r, &tr("Find match"), &med, mouse_pos, BAD, Color::rgb(250, 110, 110), WHITE, if has_team { cb(|g| g.start_search()) } else { None }, Bo::r(10).enabled(has_team).icon("battle"));
        }
        y += 58;
        if let Some((m, c)) = self.battle.ranked.msg.clone() {
            let t = tiny.render(&fit_text(&tiny, &m, inner.w), c);
            self.canvas.blit(&t, inner.x, y);
            y += 18;
        }
        let t = tiny.render(&fit_text(&tiny, &tr("Win to climb: Bronze, Silver, Gold, Platinum, Diamond, Master."), inner.w), grey());
        self.canvas.blit(&t, inner.x, y);
        y += 26;
        // the top 10
        let t = sb.render(&tr("Top players"), accent());
        self.canvas.blit(&t, inner.x, y);
        y += 26;
        let top = self.battle.ranked.top.clone();
        if top.is_empty() {
            let t = small.render(&tr("Nobody yet: be the first!"), grey());
            self.canvas.blit(&t, inner.x, y);
        }
        for (i, e) in top.iter().enumerate() {
            if y + 22 > inner.bottom() {
                break;
            }
            let (_, c) = tier_of(e.rating);
            let t = small.render(&fit_text(&small, &format!("{}. {}", i + 1, e.name), inner.w - 70), WHITE);
            self.canvas.blit(&t, inner.x, y);
            let rt = sb.render(&e.rating.to_string(), c);
            self.canvas.blit(&rt, inner.right() - rt.w, y);
            y += 24;
        }
    }

    /// "vs Friends": the challenges waiting (got / sent) and the friends to challenge.
    fn draw_battle_friends(&mut self, col: Rect, mouse_pos: (f64, f64)) {
        draw_panel(&mut self.canvas, col, Some(panel_light()), 12, false, None);
        let inner = col.inflate(-24, -24);
        let (sb, small, tiny) = (self.f.small_b.clone(), self.f.small.clone(), self.f.tiny.clone());
        let mut y = inner.y;
        if self.account.is_none() {
            for line in wrap_text(&tr("Log in to your account (Account, on the title screen) to battle your friends."), &small, inner.w) {
                let t = small.render(&line, grey());
                self.canvas.blit(&t, inner.x, y);
                y += 22;
            }
            return;
        }
        if let Some((msg, c)) = self.battle.ch_msg.clone() {
            for line in wrap_text(&msg, &tiny, inner.w).iter().take(2) {
                let t = tiny.render(line, c);
                self.canvas.blit(&t, inner.x, y);
                y += 16;
            }
            y += 6;
        }
        let row = |g: &mut Game, y: i32, label: &str, color: Color| {
            let t = small.render(&fit_text(&small, label, inner.w - 170), color);
            g.canvas.blit(&t, inner.x, y + 20 - t.h / 2);
        };
        let busy = self.battle.ch_action.is_some();
        let has_team = !self.battle.picks.is_empty();
        // challenges I got
        let received = self.battle.received.clone();
        if !received.is_empty() {
            let t = sb.render(&tr("Challenges"), accent());
            self.canvas.blit(&t, inner.x, y);
            y += 26;
            for d in received.iter().take(3) {
                row(self, y, &tr!("From %s", d.from_name.clone()), WHITE);
                let (d1, id) = (d.clone(), d.id.clone());
                self.button(Rect::new(inner.right() - 160, y + 2, 84, 36), &tr("Accept"), &sb, mouse_pos, GOOD, accent_hover(), BLACK, if has_team && !busy { cb(move |g| g.accept_challenge(d1.clone())) } else { None }, Bo::r(8).enabled(has_team && !busy));
                self.button(Rect::new(inner.right() - 70, y + 2, 70, 36), &tr("No"), &sb, mouse_pos, panel(), BAD, WHITE, if !busy { cb(move |g| g.drop_challenge(id.clone())) } else { None }, Bo::r(8).enabled(!busy));
                y += 44;
            }
            y += 6;
        }
        // challenges I sent
        let sent = self.battle.sent.clone();
        for d in sent.iter().take(3) {
            row(self, y, &tr!("Waiting for %s...", d.to_name.clone()), grey());
            let id = d.id.clone();
            self.button(Rect::new(inner.right() - 90, y + 2, 90, 36), &tr("Cancel"), &sb, mouse_pos, panel(), BAD, WHITE, if !busy { cb(move |g| g.drop_challenge(id.clone())) } else { None }, Bo::r(8).enabled(!busy));
            y += 44;
        }
        // friends
        let t = sb.render(&tr("Friends"), accent());
        self.canvas.blit(&t, inner.x, y);
        y += 26;
        let friends = self.fr.list.clone();
        if friends.is_empty() {
            let msg = if self.fr.loading || !self.fr.loaded { tr("Loading...") } else { tr("No friends yet: add some on the Friends page.") };
            for line in wrap_text(&msg, &small, inner.w) {
                let t = small.render(&line, grey());
                self.canvas.blit(&t, inner.x, y);
                y += 22;
            }
            return;
        }
        for p in friends {
            if y + 40 > inner.bottom() {
                break;
            }
            let waiting = sent.iter().any(|d| d.to_uid == p.uid);
            row(self, y, &p.username, WHITE);
            let (uid, name) = (p.uid.clone(), p.username.clone());
            let ok = has_team && !busy && !waiting;
            let label = if waiting { tr("Sent") } else { tr("Challenge") };
            self.button(Rect::new(inner.right() - 120, y + 2, 120, 36), &label, &sb, mouse_pos, BAD, Color::rgb(250, 110, 110), WHITE, if ok { cb(move |g| g.challenge_friend(uid.clone(), name.clone())) } else { None }, Bo::r(8).enabled(ok).icon("battle"));
            y += 44;
        }
    }

    /// Back to the team picker after a battle (no "ran away" message).
    pub fn battle_run_quiet(&mut self) {
        self.battle.stage = "pick";
        self.battle.battle = None;
        self.battle.queue.clear();
        self.battle.cur = None;
    }
}
