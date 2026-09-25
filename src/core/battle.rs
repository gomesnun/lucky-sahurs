//! Turn-based battles (a first, simple version): two teams of up to 3 verities, Pokemon style. Each turn both sides
//! pick an action (a move or a switch); switches and Guard go first, then the faster verity attacks first.
//! The battle only produces events (who hit whom, for how much...); game/battle_panel.rs plays them back.
//! Stats come from what the verity is: rarity (tier), mutation and phase (the Monster form is the strongest).

use super::data::{PHASES, mutation, rarities};
use crate::i18n::tr;
use crate::pyrand::PyRandom;
use crate::tr;

pub const TEAM_SIZE: usize = 3;
pub const SPECIAL_PP: i32 = 3;
pub const REST_PP: i32 = 2;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Move {
    Strike,
    Special,
    Guard,
    Rest,
}

pub const MOVES: [Move; 4] = [Move::Strike, Move::Special, Move::Guard, Move::Rest];

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Action {
    Use(Move),
    /// switch to this team member
    Switch(usize),
}

#[derive(Clone, Debug)]
pub struct Fighter {
    pub pet: usize,
    pub m: &'static str,
    pub phase: usize,
    pub max_hp: i32,
    pub hp: i32,
    pub atk: f64,
    pub def: f64,
    pub spd: f64,
    pub special_pp: i32,
    pub rest_pp: i32,
    guarding: bool,
}

/// How much a mutation adds to every stat.
fn mutation_boost(m: &str) -> f64 {
    match m {
        "golden" => 1.12,
        "diamond" => 1.25,
        "rainbow" => 1.4,
        _ => 1.0,
    }
}

/// How much a phase adds (Phase 1 ... Monster).
pub fn phase_boost(phase: usize) -> f64 {
    [1.0, 1.12, 1.26, 1.5][phase.min(PHASES.len() - 1)]
}

impl Fighter {
    pub fn new(pet: usize, m: &'static str, phase: usize) -> Fighter {
        let tier = rarities()[pet].tier as f64;
        let k = mutation_boost(m) * phase_boost(phase);
        // a small per-verity flavour so two verities of the same tier aren't identical
        let flav = |salt: usize| 0.9 + 0.2 * (((pet * 7919 + salt * 104729) % 97) as f64 / 96.0);
        let max_hp = ((70.0 + tier * 14.0) * k * flav(1)).round() as i32;
        Fighter {
            pet,
            m,
            phase,
            max_hp,
            hp: max_hp,
            atk: (24.0 + tier * 7.0) * k * flav(2),
            def: (20.0 + tier * 6.0) * k * flav(3),
            spd: (20.0 + tier * 5.0) * k * flav(4),
            special_pp: SPECIAL_PP,
            rest_pp: REST_PP,
            guarding: false,
        }
    }

    pub fn name(&self) -> String {
        let label = mutation(self.m).map(|x| x.label).unwrap_or("");
        let base = rarities()[self.pet].pet;
        if label.is_empty() { base.to_string() } else { format!("{} {}", tr(label), base) }
    }

    pub fn fainted(&self) -> bool {
        self.hp <= 0
    }

    /// One number to compare verities with (shown when picking a team).
    pub fn power(&self) -> i64 {
        (self.max_hp as f64 * 0.5 + self.atk * 1.5 + self.def + self.spd).round() as i64
    }

    pub fn pp(&self, mv: Move) -> Option<i32> {
        match mv {
            Move::Special => Some(self.special_pp),
            Move::Rest => Some(self.rest_pp),
            _ => None,
        }
    }

    pub fn can_use(&self, mv: Move) -> bool {
        self.pp(mv).is_none_or(|pp| pp > 0)
    }
}

/// The special move's name follows what the verity is.
pub fn move_name(f: &Fighter, mv: Move) -> String {
    match mv {
        Move::Strike => tr("Strike"),
        Move::Guard => tr("Guard"),
        Move::Rest => tr("Rest"),
        Move::Special => tr(if f.phase >= 3 {
            "Monster Rampage"
        } else {
            match f.m {
                "golden" => "Golden Beam",
                "diamond" => "Diamond Shards",
                "rainbow" => "Rainbow Blast",
                _ if f.phase == 2 => "Tired Stare",
                _ if f.phase == 1 => "Creepy Grin",
                _ => "Lucky Burst",
            }
        }),
    }
}

/// What a move does, in one line (the move menu shows it).
pub fn move_info(mv: Move) -> String {
    match mv {
        Move::Strike => tr("Power 40, never misses."),
        Move::Special => tr("Power 75, 85% accuracy."),
        Move::Guard => tr("Goes first; the next hit does only a quarter."),
        Move::Rest => tr("Heals 35% of its HP."),
    }
}

/// Something that happened, for the screen to show in order.
#[derive(Clone, Debug)]
pub enum Ev {
    Say(String),
    /// side sends out team member idx, which has hp left
    SendOut { side: usize, idx: usize, hp: i32 },
    /// side attacks the other one (special = its Special move, not a Strike)
    Lunge { side: usize, special: bool },
    /// side was hit and now has hp left
    Hit { side: usize, hp: i32, crit: bool },
    Heal { side: usize, hp: i32 },
    Guard { side: usize },
    Faint { side: usize },
}

pub struct Battle {
    pub teams: [Vec<Fighter>; 2],
    pub active: [usize; 2],
    /// Some(side) once one side has no verities left
    pub winner: Option<usize>,
    /// which side is the one playing on this computer (only changes the wording - never the fight itself, so
    /// both players of an online battle work out exactly the same battle)
    pub me: usize,
    /// what the other side is called ("Rival" against the computer, the friend's name online)
    pub rival_name: String,
    /// against the computer: the rival picks its moves and sends out its next verity by itself
    auto_rival: bool,
    rng: PyRandom,
}

impl Battle {
    /// Against the computer: side 0 is the player, side 1 the rival.
    pub fn new(player: Vec<Fighter>, rival: Vec<Fighter>, seed: u64) -> (Battle, Vec<Ev>) {
        Battle::new_match([player, rival], seed, 0, tr("Rival"), true)
    }

    /// Any battle: `teams` in a fixed order (online: the challenger first), `me` = the side on this computer.
    pub fn new_match(teams: [Vec<Fighter>; 2], seed: u64, me: usize, rival_name: String, auto_rival: bool) -> (Battle, Vec<Ev>) {
        let b = Battle { teams, active: [0, 0], winner: None, me, rival_name, auto_rival, rng: PyRandom::from_int(seed) };
        let other = 1 - me;
        let evs = vec![
            Ev::SendOut { side: other, idx: 0, hp: b.teams[other][0].hp },
            Ev::Say(b.sent_out_text(other)),
            Ev::SendOut { side: me, idx: 0, hp: b.teams[me][0].hp },
            Ev::Say(b.sent_out_text(me)),
        ];
        (b, evs)
    }

    pub fn fighter(&self, side: usize) -> &Fighter {
        &self.teams[side][self.active[side]]
    }

    fn roll(&mut self) -> f64 {
        self.rng.random()
    }

    /// A verity's name as this computer's player reads it ("Levity" / "Tom's Levity").
    fn who(&self, side: usize) -> String {
        if side == self.me { self.fighter(side).name() } else { tr!("%s's %s", self.rival_name.clone(), self.fighter(side).name()) }
    }

    fn sent_out_text(&self, side: usize) -> String {
        if side == self.me { tr!("Go, %s!", self.fighter(side).name()) } else { tr!("%s sent out %s!", self.rival_name.clone(), self.fighter(side).name()) }
    }

    /// This computer's player has to send out another verity (theirs fainted).
    pub fn needs_switch(&self) -> bool {
        self.needs_switch_side(self.me)
    }

    pub fn needs_switch_side(&self, side: usize) -> bool {
        self.winner.is_none() && self.fighter(side).fainted()
    }

    pub fn can_switch_to(&self, side: usize, idx: usize) -> bool {
        idx < self.teams[side].len() && idx != self.active[side] && !self.teams[side][idx].fainted()
    }

    /// After a faint: this computer's player sends out another verity (no turn passes).
    pub fn send_out(&mut self, idx: usize) -> Vec<Ev> {
        self.send_out_side(self.me, idx)
    }

    pub fn send_out_side(&mut self, side: usize, idx: usize) -> Vec<Ev> {
        if !self.needs_switch_side(side) || !self.can_switch_to(side, idx) {
            return Vec::new();
        }
        self.active[side] = idx;
        vec![Ev::SendOut { side, idx, hp: self.fighter(side).hp }, Ev::Say(self.sent_out_text(side))]
    }

    /// A side gives up (leaves an online battle): the other side wins.
    pub fn forfeit(&mut self, side: usize) -> Vec<Ev> {
        if self.winner.is_some() {
            return Vec::new();
        }
        self.winner = Some(1 - side);
        let who = if side == self.me { tr("You") } else { self.rival_name.clone() };
        vec![Ev::Say(tr!("%s gave up!", who))]
    }

    /// The computer's choice: heal when low, guard now and then, otherwise hit as hard as it can.
    fn rival_action(&mut self) -> Action {
        let me = self.fighter(1).clone();
        let r = self.roll();
        if me.hp * 3 < me.max_hp && me.rest_pp > 0 && r < 0.6 {
            return Action::Use(Move::Rest);
        }
        if r < 0.12 {
            return Action::Use(Move::Guard);
        }
        if me.special_pp > 0 && r < 0.55 {
            return Action::Use(Move::Special);
        }
        Action::Use(Move::Strike)
    }

    /// Against the computer: one turn with the player's action (the rival picks its own).
    pub fn turn(&mut self, player: Action) -> Vec<Ev> {
        if self.winner.is_some() || self.needs_switch() {
            return Vec::new();
        }
        let rival = self.rival_action();
        self.turn_both([player, rival])
    }

    /// One turn with both sides' actions (in side order). Returns what happened, in order.
    pub fn turn_both(&mut self, actions: [Action; 2]) -> Vec<Ev> {
        let mut evs = Vec::new();
        if self.winner.is_some() || self.needs_switch_side(0) || self.needs_switch_side(1) {
            return evs;
        }
        for side in 0..2 {
            self.teams[side][self.active[side]].guarding = false;
        }
        // switches first, then Guard, then the faster one
        let prio = |a: &Action| match a {
            Action::Switch(_) => 2,
            Action::Use(Move::Guard) => 1,
            _ => 0,
        };
        let (p0, p1) = (prio(&actions[0]), prio(&actions[1]));
        let first = if p0 != p1 {
            if p0 > p1 { 0 } else { 1 }
        } else {
            let (s0, s1) = (self.fighter(0).spd, self.fighter(1).spd);
            if (s0 - s1).abs() < 1e-9 { usize::from(self.roll() < 0.5) } else if s0 > s1 { 0 } else { 1 }
        };
        for side in [first, 1 - first] {
            if self.winner.is_some() || self.fighter(side).fainted() {
                continue;
            }
            self.act(side, actions[side], &mut evs);
        }
        evs
    }

    fn act(&mut self, side: usize, action: Action, evs: &mut Vec<Ev>) {
        let other = 1 - side;
        match action {
            Action::Switch(idx) => {
                if self.can_switch_to(side, idx) {
                    evs.push(Ev::Say(if side == self.me {
                        tr!("Come back, %s!", self.fighter(side).name())
                    } else {
                        tr!("%s called back %s!", self.rival_name.clone(), self.fighter(side).name())
                    }));
                    self.active[side] = idx;
                    evs.push(Ev::SendOut { side, idx, hp: self.fighter(side).hp });
                    evs.push(Ev::Say(self.sent_out_text(side)));
                }
            }
            Action::Use(mv) => {
                let mv = if self.fighter(side).can_use(mv) { mv } else { Move::Strike };
                evs.push(Ev::Say(tr!("%s used %s!", self.who(side), move_name(self.fighter(side), mv))));
                match mv {
                    Move::Guard => {
                        self.teams[side][self.active[side]].guarding = true;
                        evs.push(Ev::Guard { side });
                    }
                    Move::Rest => {
                        let f = &mut self.teams[side][self.active[side]];
                        f.rest_pp -= 1;
                        f.hp = (f.hp + (f.max_hp as f64 * 0.35).round() as i32).min(f.max_hp);
                        evs.push(Ev::Heal { side, hp: f.hp });
                        evs.push(Ev::Say(tr!("%s feels better.", self.who(side))));
                    }
                    Move::Strike | Move::Special => {
                        let (power, accuracy) = if mv == Move::Special { (75.0, 0.85) } else { (40.0, 1.0) };
                        if mv == Move::Special {
                            self.teams[side][self.active[side]].special_pp -= 1;
                        }
                        evs.push(Ev::Lunge { side, special: mv == Move::Special });
                        if self.roll() > accuracy {
                            evs.push(Ev::Say(tr!("%s missed!", self.who(side))));
                            return;
                        }
                        let crit = self.roll() < 1.0 / 16.0;
                        let spread = 0.85 + 0.15 * self.roll();
                        let (atk, def) = (self.fighter(side).atk, self.fighter(other).def);
                        let mut dmg = (power * atk / def * 0.45 + 2.0) * spread * if crit { 1.5 } else { 1.0 };
                        if self.fighter(other).guarding {
                            dmg *= 0.25;
                        }
                        let dmg = (dmg.round() as i32).max(1);
                        let target = &mut self.teams[other][self.active[other]];
                        target.hp = (target.hp - dmg).max(0);
                        evs.push(Ev::Hit { side: other, hp: target.hp, crit });
                        if crit {
                            evs.push(Ev::Say(tr("A critical hit!")));
                        }
                        if self.fighter(other).guarding {
                            evs.push(Ev::Say(tr!("%s guarded against it!", self.who(other))));
                        }
                        if self.fighter(other).fainted() {
                            evs.push(Ev::Faint { side: other });
                            evs.push(Ev::Say(tr!("%s fainted!", self.who(other))));
                            self.after_faint(other, evs);
                        }
                    }
                }
            }
        }
    }

    fn after_faint(&mut self, side: usize, evs: &mut Vec<Ev>) {
        let left: Vec<usize> = (0..self.teams[side].len()).filter(|&i| !self.teams[side][i].fainted()).collect();
        if left.is_empty() {
            self.winner = Some(1 - side);
            return;
        }
        if self.auto_rival && side != self.me {
            // the computer sends out its next one right away
            self.active[side] = left[0];
            evs.push(Ev::SendOut { side, idx: left[0], hp: self.fighter(side).hp });
            evs.push(Ev::Say(self.sent_out_text(side)));
        }
    }
}

/// A rival team about as strong as the player's: verities of similar rarity, mutation and phase.
pub fn rival_team(player: &[Fighter], seed: u64) -> Vec<Fighter> {
    let mut rng = PyRandom::from_int(seed ^ 0x9e37_79b9);
    let n = rarities().len();
    player
        .iter()
        .map(|p| {
            let tier = rarities()[p.pet].tier as i64;
            let wanted = (tier + (rng.random() * 3.0) as i64 - 1).clamp(0, rarities()[n - 1].tier as i64) as usize;
            let pool: Vec<usize> = (0..n).filter(|&i| rarities()[i].tier == wanted).collect();
            let pet = pool[(rng.random() * pool.len() as f64) as usize % pool.len()];
            let phase = if rng.random() < 0.5 { p.phase } else { p.phase.saturating_sub(1) };
            Fighter::new(pet, p.m, phase)
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_battle_ends_with_a_winner() {
        let team = vec![Fighter::new(0, "normal", 0), Fighter::new(10, "golden", 1), Fighter::new(20, "normal", 3)];
        let rival = rival_team(&team, 7);
        assert_eq!(rival.len(), 3);
        let (mut b, start) = Battle::new(team, rival, 42);
        assert!(!start.is_empty());
        for _ in 0..500 {
            if b.winner.is_some() {
                break;
            }
            if b.needs_switch() {
                let next = (0..3).find(|&i| b.can_switch_to(0, i)).unwrap();
                assert!(!b.send_out(next).is_empty());
                continue;
            }
            let evs = b.turn(Action::Use(Move::Strike));
            assert!(!evs.is_empty());
        }
        assert!(b.winner.is_some());
    }

    #[test]
    fn both_players_see_the_same_online_battle() {
        let teams = || [vec![Fighter::new(10, "golden", 3), Fighter::new(4, "normal", 1)], vec![Fighter::new(12, "normal", 2), Fighter::new(7, "rainbow", 0)]];
        let (mut a, _) = Battle::new_match(teams(), 99, 0, "B".into(), false);
        let (mut b, _) = Battle::new_match(teams(), 99, 1, "A".into(), false);
        let moves = [Action::Use(Move::Special), Action::Use(Move::Strike), Action::Use(Move::Guard), Action::Use(Move::Strike)];
        for i in 0..200 {
            if a.winner.is_some() {
                break;
            }
            let mut stepped = false;
            for side in 0..2 {
                if a.needs_switch_side(side) {
                    let k = (0..2).find(|&k| a.can_switch_to(side, k)).unwrap();
                    a.send_out_side(side, k);
                    b.send_out_side(side, k);
                    stepped = true;
                }
            }
            if stepped {
                continue;
            }
            let acts = [moves[i % 4], moves[(i + 1) % 4]];
            a.turn_both(acts);
            b.turn_both(acts);
            assert_eq!([a.fighter(0).hp, a.fighter(1).hp], [b.fighter(0).hp, b.fighter(1).hp]);
        }
        assert!(a.winner.is_some());
        assert_eq!(a.winner, b.winner);
    }

    #[test]
    fn monsters_are_stronger() {
        let normal = Fighter::new(5, "normal", 0);
        let monster = Fighter::new(5, "normal", 3);
        let rainbow_monster = Fighter::new(5, "rainbow", 3);
        assert!(monster.power() > normal.power());
        assert!(rainbow_monster.power() > monster.power());
    }

    #[test]
    fn special_uses_pp_and_guard_softens_hits() {
        let tough = || Fighter::new(40, "rainbow", 3);
        let (mut b, _) = Battle::new(vec![tough()], vec![tough()], 1);
        b.turn(Action::Use(Move::Special));
        assert_eq!(b.fighter(0).special_pp, SPECIAL_PP - 1);
        b.teams[0][0].special_pp = 0;
        assert!(!b.fighter(0).can_use(Move::Special));
        // the same hit, guarded or not
        let hit = |guard: bool| {
            let (mut b, _) = Battle::new(vec![tough()], vec![tough()], 3);
            b.teams[1][0].guarding = guard;
            let mut evs = Vec::new();
            b.act(0, Action::Use(Move::Strike), &mut evs);
            b.fighter(1).max_hp - b.fighter(1).hp
        };
        assert!(hit(true) * 3 < hit(false));
    }
}
