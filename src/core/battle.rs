//! Turn-based battles, Pokemon style: two teams of up to TEAM_SIZE verities in a set order; when one faints the next
//! comes out by itself. Every verity fights in its own phase, except Monsters: they start in their Phase 3 form and
//! build up RAGE each turn (faster when they hit or get hit). With a full meter they can TRANSFORM into the Monster
//! for MONSTER_TURNS turns - stronger in everything and with a huge boost to their moves - then they calm back down
//! and the meter starts again. Each turn both sides pick an action (a move, a switch, an item or the
//! transformation); Transform, switches, items and Guard go first, then the faster verity.
//! The battle only produces events (who hit whom, for how much...); game/battle_panel.rs plays them back.
//! Stats come from what the verity is: rarity (tier), mutation and phase (the Monster form is the strongest).

use super::data::{PHASES, mutation, rarities};
use crate::i18n::tr;
use crate::pyrand::PyRandom;
use crate::tr;

pub const TEAM_SIZE: usize = 5;
/// a Monster fights in its Phase 3 form until it transforms (index into PHASES)
pub const CALM_PHASE: usize = 2;
pub const RAGE_MAX: i32 = 100;
/// rage a Monster gets each turn, and when it hits / gets hit
pub const RAGE_TURN: i32 = 20;
pub const RAGE_HIT: i32 = 10;
/// how long the Monster form lasts
pub const MONSTER_TURNS: i32 = 3;
/// in Monster form: attack, defence and speed x STAT, and its moves hit x MOVE harder
pub const MONSTER_STAT: f64 = 1.35;
pub const MONSTER_MOVE: f64 = 1.5;
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

/// Battle items (bought in the Shop, used from the battle's BAG; each use takes the turn).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Item {
    Potion,
    Power,
    Iron,
    Feather,
}

pub const ITEMS: [Item; 4] = [Item::Potion, Item::Power, Item::Iron, Item::Feather];

impl Item {
    pub fn key(self) -> &'static str {
        match self {
            Item::Potion => "potion",
            Item::Power => "power",
            Item::Iron => "iron",
            Item::Feather => "feather",
        }
    }
    pub fn from_key(k: &str) -> Option<Item> {
        ITEMS.iter().copied().find(|i| i.key() == k)
    }
    pub fn name(self) -> String {
        tr(match self {
            Item::Potion => "Battle Potion",
            Item::Power => "Power Charm",
            Item::Iron => "Iron Charm",
            Item::Feather => "Swift Feather",
        })
    }
    pub fn info(self) -> String {
        tr(match self {
            Item::Potion => "Heals 50% of the active verity's HP.",
            Item::Power => "+30% attack for the active verity.",
            Item::Iron => "+30% defence for the active verity.",
            Item::Feather => "+50% speed for the active verity.",
        })
    }
    /// its letter in an online battle's move string
    pub fn letter(self) -> char {
        match self {
            Item::Potion => 'h',
            Item::Power => 'k',
            Item::Iron => 'i',
            Item::Feather => 'f',
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Action {
    Use(Move),
    /// switch to this team member
    Switch(usize),
    Item(Item),
    /// the active verity turns into its Monster form (once per battle)
    Transform,
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
    /// it was evolved into a Monster: it builds rage and can transform
    pub monster: bool,
    /// 0..RAGE_MAX
    pub rage: i32,
    /// turns left in Monster form (0 = calm)
    pub raging: i32,
    // item boosts (multipliers)
    pub atk_up: f64,
    pub def_up: f64,
    pub spd_up: f64,
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
    [1.0, 1.12, 1.26, 1.7][phase.min(PHASES.len() - 1)]
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
            monster: false,
            rage: 0,
            raging: 0,
            atk_up: 1.0,
            def_up: 1.0,
            spd_up: 1.0,
            guarding: false,
        }
    }

    /// A verity entering a battle in the phase you have it in; a Monster starts calm (Phase 3) and has to rage up.
    pub fn battle(pet: usize, m: &'static str, owned_phase: usize) -> Fighter {
        if owned_phase > CALM_PHASE {
            let mut f = Fighter::new(pet, m, CALM_PHASE);
            f.monster = true;
            f
        } else {
            Fighter::new(pet, m, owned_phase)
        }
    }

    pub fn can_transform(&self) -> bool {
        self.monster && self.raging == 0 && self.rage >= RAGE_MAX && !self.fainted()
    }

    fn add_rage(&mut self, n: i32) {
        if self.monster && self.raging == 0 {
            self.rage = (self.rage + n).min(RAGE_MAX);
        }
    }

    fn form(&self) -> f64 {
        if self.raging > 0 { MONSTER_STAT } else { 1.0 }
    }
    pub fn eff_atk(&self) -> f64 {
        self.atk * self.atk_up * self.form()
    }
    pub fn eff_def(&self) -> f64 {
        self.def * self.def_up * self.form()
    }
    pub fn eff_spd(&self) -> f64 {
        self.spd * self.spd_up * self.form()
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
    /// side sends out team member idx, which has hp left and is in this phase (form)
    SendOut { side: usize, idx: usize, hp: i32, phase: usize },
    /// side's active verity turns into its Monster form (now with hp left)
    Transform { side: usize, hp: i32 },
    /// side's Monster calms back down to Phase 3
    Calm { side: usize },
    /// side's active verity's rage meter is now at rage
    Rage { side: usize, rage: i32 },
    /// side used an item
    UseItem { side: usize, item: Item },
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
            Ev::SendOut { side: other, idx: 0, hp: b.teams[other][0].hp, phase: b.teams[other][0].phase },
            Ev::Say(b.sent_out_text(other)),
            Ev::SendOut { side: me, idx: 0, hp: b.teams[me][0].hp, phase: b.teams[me][0].phase },
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
        vec![Ev::SendOut { side, idx, hp: self.fighter(side).hp, phase: self.fighter(side).phase }, Ev::Say(self.sent_out_text(side))]
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

    /// The computer's choice: rage out when it can, heal when low, guard now and then, otherwise hit hard.
    fn rival_action(&mut self) -> Action {
        let me = self.fighter(1).clone();
        let r = self.roll();
        if me.can_transform() {
            return Action::Transform;
        }
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

    /// Whether this side may take this action now.
    pub fn allowed(&self, side: usize, a: Action) -> bool {
        let f = self.fighter(side);
        match a {
            Action::Use(mv) => f.can_use(mv),
            Action::Switch(i) => self.can_switch_to(side, i),
            Action::Item(_) => true,
            Action::Transform => f.can_transform(),
        }
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
        // anything not allowed (a stale online move...) becomes a Strike
        let actions = [0, 1].map(|s| if self.allowed(s, actions[s]) { actions[s] } else { Action::Use(Move::Strike) });
        for side in 0..2 {
            self.teams[side][self.active[side]].guarding = false;
        }
        // Transform, then switches, then items and Guard, then the faster one
        let prio = |a: &Action| match a {
            Action::Transform => 4,
            Action::Switch(_) => 3,
            Action::Item(_) | Action::Use(Move::Guard) => 1,
            _ => 0,
        };
        let (p0, p1) = (prio(&actions[0]), prio(&actions[1]));
        let first = if p0 != p1 {
            if p0 > p1 { 0 } else { 1 }
        } else {
            let (s0, s1) = (self.fighter(0).eff_spd(), self.fighter(1).eff_spd());
            if (s0 - s1).abs() < 1e-9 { usize::from(self.roll() < 0.5) } else if s0 > s1 { 0 } else { 1 }
        };
        let acting = [self.active[0], self.active[1]];
        for side in [first, 1 - first] {
            if self.winner.is_some() || self.fighter(side).fainted() || self.active[side] != acting[side] {
                continue;
            }
            self.act(side, actions[side], &mut evs);
        }
        // end of the turn: Monsters rage up, raging ones count down and calm down when time's up
        if self.winner.is_none() {
            for side in 0..2 {
                let f = &mut self.teams[side][self.active[side]];
                if f.fainted() || !f.monster {
                    continue;
                }
                if f.raging > 0 {
                    f.raging -= 1;
                    if f.raging == 0 {
                        f.phase = CALM_PHASE;
                        evs.push(Ev::Calm { side });
                        evs.push(Ev::Say(tr!("%s calmed down.", self.who(side))));
                        evs.push(Ev::Rage { side, rage: 0 });
                    }
                } else if actions[side] != Action::Transform {
                    let before = f.rage;
                    f.add_rage(RAGE_TURN);
                    let now = f.rage;
                    evs.push(Ev::Rage { side, rage: now });
                    if before < RAGE_MAX && now >= RAGE_MAX {
                        evs.push(Ev::Say(tr!("%s is full of RAGE!", self.who(side))));
                    }
                }
            }
        }
        evs
    }

    fn act(&mut self, side: usize, action: Action, evs: &mut Vec<Ev>) {
        let other = 1 - side;
        match action {
            Action::Transform => {
                let f = &mut self.teams[side][self.active[side]];
                f.phase = 3;
                f.raging = MONSTER_TURNS;
                f.rage = 0;
                f.hp = (f.hp + (f.max_hp as f64 * 0.15).round() as i32).min(f.max_hp);
                let hp = f.hp;
                evs.push(Ev::Say(tr!("%s is transforming!", self.who(side))));
                evs.push(Ev::Transform { side, hp });
                evs.push(Ev::Rage { side, rage: 0 });
                evs.push(Ev::Say(tr!("%s became a MONSTER!", self.who(side))));
            }
            Action::Item(item) => {
                evs.push(Ev::Say(tr!("%s used a %s!", if side == self.me { tr("You") } else { self.rival_name.clone() }, item.name())));
                evs.push(Ev::UseItem { side, item });
                let f = &mut self.teams[side][self.active[side]];
                match item {
                    Item::Potion => {
                        f.hp = (f.hp + (f.max_hp as f64 * 0.5).round() as i32).min(f.max_hp);
                        let hp = f.hp;
                        evs.push(Ev::Heal { side, hp });
                    }
                    Item::Power => f.atk_up *= 1.3,
                    Item::Iron => f.def_up *= 1.3,
                    Item::Feather => f.spd_up *= 1.5,
                }
            }
            Action::Switch(idx) => {
                if self.can_switch_to(side, idx) {
                    evs.push(Ev::Say(if side == self.me {
                        tr!("Come back, %s!", self.fighter(side).name())
                    } else {
                        tr!("%s called back %s!", self.rival_name.clone(), self.fighter(side).name())
                    }));
                    // a raging Monster that leaves calms down
                    let f = &mut self.teams[side][self.active[side]];
                    if f.raging > 0 {
                        f.raging = 0;
                        f.phase = CALM_PHASE;
                    }
                    self.active[side] = idx;
                    evs.push(Ev::SendOut { side, idx, hp: self.fighter(side).hp, phase: self.fighter(side).phase });
                    evs.push(Ev::Rage { side, rage: self.fighter(side).rage });
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
                        let raging = self.fighter(side).raging > 0;
                        let (atk, def) = (self.fighter(side).eff_atk(), self.fighter(other).eff_def());
                        let mut dmg = (power * atk / def * 0.85 + 3.0) * spread * if crit { 1.5 } else { 1.0 } * if raging { MONSTER_MOVE } else { 1.0 };
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
                        // hitting and getting hit both feed the rage
                        for (s, n) in [(side, RAGE_HIT), (other, RAGE_HIT)] {
                            let f = &mut self.teams[s][self.active[s]];
                            if f.monster && f.raging == 0 && !f.fainted() {
                                f.add_rage(n);
                                evs.push(Ev::Rage { side: s, rage: f.rage });
                            }
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

    /// A verity fainted: the next one in the team's order comes out by itself.
    fn after_faint(&mut self, side: usize, evs: &mut Vec<Ev>) {
        let n = self.teams[side].len();
        let next = (1..n).map(|k| (self.active[side] + k) % n).find(|&i| !self.teams[side][i].fainted());
        let Some(i) = next else {
            self.winner = Some(1 - side);
            return;
        };
        self.active[side] = i;
        let _ = self.auto_rival;
        evs.push(Ev::SendOut { side, idx: i, hp: self.fighter(side).hp, phase: self.fighter(side).phase });
        evs.push(Ev::Rage { side, rage: self.fighter(side).rage });
        evs.push(Ev::Say(self.sent_out_text(side)));
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
            // a Monster gets a Monster back (it rages too); others a verity of about the same phase
            let owned = if p.monster { 3 } else if rng.random() < 0.5 { p.phase } else { p.phase.saturating_sub(1) };
            Fighter::battle(pet, p.m, owned)
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_battle_ends_with_a_winner() {
        let team = vec![Fighter::battle(0, "normal", 0), Fighter::battle(10, "golden", 1), Fighter::battle(20, "normal", 3)];
        let rival = rival_team(&team, 7);
        assert_eq!(rival.len(), 3);
        let (mut b, start) = Battle::new(team, rival, 42);
        assert!(!start.is_empty());
        for _ in 0..500 {
            if b.winner.is_some() {
                break;
            }
            // fainted verities are replaced by themselves
            assert!(!b.needs_switch());
            let a = if b.fighter(0).can_transform() { Action::Transform } else { Action::Use(Move::Strike) };
            assert!(!b.turn(a).is_empty());
        }
        assert!(b.winner.is_some());
    }

    #[test]
    fn monsters_rage_up_transform_and_calm_down() {
        let tough = || Fighter::battle(40, "normal", 0);
        let (mut b, _) = Battle::new(vec![Fighter::battle(40, "rainbow", 3)], vec![tough(), tough(), tough()], 5);
        assert_eq!(b.fighter(0).phase, CALM_PHASE);
        assert!(b.fighter(0).monster && !b.fighter(0).can_transform());
        let mut turns = 0;
        while !b.fighter(0).can_transform() {
            b.turn(Action::Use(Move::Guard));
            turns += 1;
            assert!(turns < 8, "the rage meter fills in a few turns");
        }
        let before = b.fighter(0).eff_atk();
        let evs = b.turn(Action::Transform);
        assert!(evs.iter().any(|e| matches!(e, Ev::Transform { side: 0, .. })));
        assert_eq!(b.fighter(0).phase, 3);
        assert!(b.fighter(0).eff_atk() > before * 1.3);
        for _ in 0..MONSTER_TURNS - 1 {
            b.turn(Action::Use(Move::Guard));
        }
        assert_eq!(b.fighter(0).phase, CALM_PHASE, "back to Phase 3 after the Monster turns");
        assert_eq!(b.fighter(0).rage, 0);
        // a Phase 1 verity fights as Phase 1 and never rages
        let f = Fighter::battle(3, "normal", 0);
        assert_eq!((f.phase, f.monster), (0, false));
    }

    #[test]
    fn both_players_see_the_same_online_battle() {
        let teams = || [vec![Fighter::battle(10, "golden", 3), Fighter::battle(4, "normal", 1)], vec![Fighter::battle(12, "normal", 2), Fighter::battle(7, "rainbow", 0)]];
        let (mut a, _) = Battle::new_match(teams(), 99, 0, "B".into(), false);
        let (mut b, _) = Battle::new_match(teams(), 99, 1, "A".into(), false);
        let moves = [Action::Use(Move::Special), Action::Item(Item::Power), Action::Use(Move::Guard), Action::Use(Move::Strike)];
        for i in 0..200 {
            if a.winner.is_some() {
                break;
            }
            let pick = |bt: &Battle, s: usize, k: usize| if bt.fighter(s).can_transform() { Action::Transform } else { moves[k % 4] };
            let acts = [pick(&a, 0, i), pick(&a, 1, i + 1)];
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
