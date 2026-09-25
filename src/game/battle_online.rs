//! Online battles between friends. The challenger sends their team; the friend accepts with theirs; then each side
//! only sends its own moves (one letter each). Both games work out the very same battle from the teams, a shared
//! seed and the moves (core/battle.rs is deterministic), so nothing else goes online.
//! Firestore use: while waiting for the friend's move the battle is read every POLL seconds; with the Battle page
//! open on "vs Friends" the challenge lists are refreshed every LIST_REFRESH seconds. One write per move.

use super::Game;
use crate::core::battle::{Action, Battle, Ev, Fighter, Move};
use crate::core::data::{MAX_PHASE, is_mutation, mut_key, rarities};
use crate::core::state::now_ts;
use crate::i18n::tr;
use crate::online::firebase::{BattleDoc, online_error_text};
use crate::theme::*;
use crate::tr;

/// seconds between reads of the battle while waiting for the friend's move
pub const POLL: f64 = 2.5;
/// seconds between refreshes of the challenge lists (Battle page open on "vs Friends")
pub const LIST_REFRESH: f64 = 15.0;
/// after this long without a move from the friend, "Leave" shows up
pub const PATIENCE: f64 = 90.0;

pub struct OnlineMatch {
    pub id: String,
    /// 0 = I challenged, 1 = I accepted (the battle's side order is always challenger, friend)
    pub me: usize,
    pub friend: String,
    pub my_moves: String,
    pub their_moves: String,
    /// how many of each side's moves the battle has used (side order)
    used: [usize; 2],
    next_poll: f64,
    polling: bool,
    /// my moves haven't reached the server yet
    unsent: bool,
    pushing: bool,
    pub waiting_since: f64,
    /// the friend deleted the battle (left)
    pub gone: bool,
    finished: bool,
}

/// "pet_mutation_phase,..." for my picked team.
pub fn team_code(picks: &[(usize, &'static str, usize)]) -> String {
    picks.iter().map(|(p, m, ph)| format!("{}_{}_{}", p, m, ph)).collect::<Vec<_>>().join(",")
}

/// The fighters of a team code (anything invalid is left out; at most 3).
pub fn team_from_code(code: &str) -> Vec<Fighter> {
    code.split(',')
        .filter_map(|part| {
            let mut it = part.split('_');
            let pet = it.next()?.parse::<usize>().ok()?;
            let m = it.next()?;
            let phase = it.next()?.parse::<usize>().ok()?;
            (pet < rarities().len() && is_mutation(m) && phase <= MAX_PHASE).then(|| Fighter::new(pet, mut_key(m), phase))
        })
        .take(3)
        .collect()
}

fn action_of(c: char) -> Option<Action> {
    Some(match c {
        'S' => Action::Use(Move::Strike),
        'P' => Action::Use(Move::Special),
        'G' => Action::Use(Move::Guard),
        'R' => Action::Use(Move::Rest),
        d if d.is_ascii_digit() => Action::Switch(d.to_digit(10)? as usize),
        _ => return None,
    })
}

pub fn letter_of(mv: Move) -> char {
    match mv {
        Move::Strike => 'S',
        Move::Special => 'P',
        Move::Guard => 'G',
        Move::Rest => 'R',
    }
}

impl Game {
    pub fn online_ready(&self) -> bool {
        self.client.is_some() && self.account.is_some()
    }

    fn my_username(&self) -> String {
        self.client.as_ref().and_then(|c| c.username()).unwrap_or_else(|| "?".into())
    }

    // ---------------------------------------------------------------- the challenge lists
    pub fn refresh_battles(&mut self, force: bool) {
        if !self.online_ready() || self.battle.ch_loading {
            return;
        }
        let now = now_ts();
        if !force && now < self.battle.ch_next {
            return;
        }
        self.battle.ch_next = now + LIST_REFRESH;
        self.battle.ch_loading = true;
        let client = self.client.clone().unwrap();
        self.run_job(
            move || Ok((client.list_battles("from_uid")?, client.list_battles("to_uid")?)),
            |g, (sent, got): (Vec<BattleDoc>, Vec<BattleDoc>)| {
                g.battle.ch_loading = false;
                g.battle.ch_loaded = true;
                // finished ones I started are cleaned up (a few minutes after)
                for d in sent.iter().filter(|d| d.status == "over" && crate::online::firebase::server_now() - d.created > 300.0) {
                    let (client, id) = (g.client.clone().unwrap(), d.id.clone());
                    g.worker.run::<()>(move || client.delete_battle(&id), None, None);
                }
                // a challenge I sent was accepted: the battle starts
                if g.battle.online.is_none() && g.battle.open {
                    if let Some(d) = sent.iter().find(|d| d.status == "live") {
                        let d = d.clone();
                        g.start_online_battle(&d, 0);
                    }
                }
                g.battle.sent = sent.into_iter().filter(|d| d.status == "pending").collect();
                g.battle.received = got.into_iter().filter(|d| d.status == "pending").collect();
            },
            |g, e| {
                g.battle.ch_loading = false;
                g.battle.ch_msg = Some((online_error_text(&e), BAD));
                g.battle.ch_next = now_ts() + 45.0;
            },
        );
    }

    /// Challenges a friend with the team picked now.
    pub fn challenge_friend(&mut self, uid: String, name: String) {
        if !self.online_ready() || self.battle.picks.is_empty() || self.battle.ch_action.is_some() {
            return;
        }
        let picks: Vec<(usize, &'static str, usize)> = self.battle.picks.iter().map(|(p, m)| (*p, *m, self.state.phase(*p, m))).collect();
        let team = team_code(&picks);
        let seed = ((now_ts() * 1000.0) as i64).rem_euclid(1 << 40) ^ (self.state.total_rolls & 0xffff);
        let me = self.my_username();
        self.battle.ch_action = Some(uid.clone());
        let client = self.client.clone().unwrap();
        let name2 = name.clone();
        self.run_job(
            move || client.create_battle(&uid, &me, &name2, &team, seed),
            move |g, _id: String| {
                g.battle.ch_action = None;
                g.battle.ch_msg = Some((tr!("Challenge sent to %s! The battle starts when they accept.", name), GOOD));
                g.battle.ch_next = 0.0;
                g.refresh_battles(true);
            },
            |g, e| {
                g.battle.ch_action = None;
                g.battle.ch_msg = Some((online_error_text(&e), BAD));
            },
        );
    }

    /// Accept a friend's challenge with the team picked now.
    pub fn accept_challenge(&mut self, doc: BattleDoc) {
        if !self.online_ready() || self.battle.picks.is_empty() || self.battle.ch_action.is_some() {
            return;
        }
        let picks: Vec<(usize, &'static str, usize)> = self.battle.picks.iter().map(|(p, m)| (*p, *m, self.state.phase(*p, m))).collect();
        let team = team_code(&picks);
        self.battle.ch_action = Some(doc.id.clone());
        let client = self.client.clone().unwrap();
        let id = doc.id.clone();
        let t2 = team.clone();
        self.run_job(
            move || client.accept_battle(&id, &t2),
            move |g, _: ()| {
                g.battle.ch_action = None;
                let mut d = doc.clone();
                d.to_team = team.clone();
                d.status = "live".into();
                g.battle.received.retain(|x| x.id != d.id);
                g.start_online_battle(&d, 1);
            },
            |g, e| {
                g.battle.ch_action = None;
                g.battle.ch_msg = Some((online_error_text(&e), BAD));
            },
        );
    }

    /// Decline (a challenge I got) or cancel (one I sent).
    pub fn drop_challenge(&mut self, id: String) {
        if !self.online_ready() || self.battle.ch_action.is_some() {
            return;
        }
        self.battle.ch_action = Some(id.clone());
        let client = self.client.clone().unwrap();
        let id2 = id.clone();
        self.run_job(
            move || client.delete_battle(&id2),
            move |g, _: ()| {
                g.battle.ch_action = None;
                g.battle.sent.retain(|d| d.id != id);
                g.battle.received.retain(|d| d.id != id);
            },
            |g, e| {
                g.battle.ch_action = None;
                g.battle.ch_msg = Some((online_error_text(&e), BAD));
            },
        );
    }

    // ---------------------------------------------------------------- the battle itself
    pub fn start_online_battle(&mut self, d: &BattleDoc, me: usize) {
        let teams = [team_from_code(&d.from_team), team_from_code(&d.to_team)];
        if teams[0].is_empty() || teams[1].is_empty() {
            self.battle.ch_msg = Some((tr("That battle has an invalid team."), BAD));
            return;
        }
        let friend = if me == 0 { d.to_name.clone() } else { d.from_name.clone() };
        let (b, evs) = Battle::new_match(teams, d.seed as u64, me, friend.clone(), false);
        self.battle.online = Some(OnlineMatch {
            id: d.id.clone(),
            me,
            friend,
            my_moves: String::new(),
            their_moves: String::new(),
            used: [0, 0],
            next_poll: now_ts() + POLL,
            polling: false,
            unsent: false,
            pushing: false,
            waiting_since: now_ts(),
            gone: false,
            finished: false,
        });
        self.begin_battle_playback(b, evs);
    }

    /// For the --onlinetest harness: my moves so far, and the friend's (as if read from the server).
    pub fn online_moves(&self) -> String {
        self.battle.online.as_ref().map(|om| om.my_moves.clone()).unwrap_or_default()
    }
    pub fn online_feed(&mut self, theirs: &str) {
        if let Some(om) = self.battle.online.as_mut() {
            om.their_moves = theirs.to_string();
        }
    }

    /// Whether the battle waits for MY next move (online).
    pub fn online_my_turn(&self) -> bool {
        let (Some(om), Some(b)) = (self.battle.online.as_ref(), self.battle.battle.as_ref()) else { return false };
        if b.winner.is_some() || om.gone {
            return false;
        }
        let need = [b.needs_switch_side(0), b.needs_switch_side(1)];
        let wants_me = need[om.me] || (!need[0] && !need[1]);
        wants_me && om.my_moves.chars().count() == om.used[om.me]
    }

    /// My move (a letter): kept locally, sent to the server in the background.
    pub fn online_send(&mut self, c: char) {
        if !self.online_my_turn() {
            return;
        }
        if let Some(om) = self.battle.online.as_mut() {
            om.my_moves.push(c);
            om.unsent = true;
            om.waiting_since = now_ts();
        }
        self.battle.menu = "main";
        self.push_online_moves();
    }

    fn push_online_moves(&mut self) {
        let Some(client) = self.client.clone() else { return };
        let Some(om) = self.battle.online.as_mut() else { return };
        if !om.unsent || om.pushing {
            return;
        }
        om.pushing = true;
        om.unsent = false;
        let (id, field, moves) = (om.id.clone(), if om.me == 0 { "from_moves" } else { "to_moves" }, om.my_moves.clone());
        self.run_job(
            move || client.push_battle_moves(&id, field, &moves),
            |g, _: ()| {
                if let Some(om) = g.battle.online.as_mut() {
                    om.pushing = false;
                    if om.unsent {
                        g.push_online_moves();
                    }
                }
            },
            |g, e| {
                if let Some(om) = g.battle.online.as_mut() {
                    om.pushing = false;
                    om.unsent = true; // tried again with the next poll
                    if e.code == "not_found" {
                        om.gone = true;
                    }
                }
            },
        );
    }

    /// Leave an online battle: if it isn't over, I give up (my friend wins).
    pub fn leave_online_battle(&mut self) {
        let Some(om) = self.battle.online.as_ref() else { return };
        let over = self.battle.battle.as_ref().is_none_or(|b| b.winner.is_some()) || om.gone;
        if !over {
            // "X" = I give up; the friend's game sees it with its next poll
            if let Some(om) = self.battle.online.as_mut() {
                om.my_moves.push('X');
                om.unsent = true;
            }
            self.push_online_moves();
        } else if om.me == 0 {
            let (client, id) = (self.client.clone(), om.id.clone());
            if let Some(client) = client {
                self.worker.run::<()>(move || client.finish_battle(&id), None, None);
            }
        }
        self.battle.online = None;
        self.battle_run_quiet();
    }

    /// Every frame: send what's unsent, read the friend's moves while waiting, and play every step the battle
    /// has both moves for.
    pub fn tick_battle_online(&mut self) {
        if self.battle.open && self.battle.mode == "online" && self.battle.online.is_none() {
            self.refresh_battles(false);
        }
        let Some(om) = self.battle.online.as_ref() else { return };
        let now = now_ts();
        if om.unsent && !om.pushing {
            self.push_online_moves();
        }
        self.online_resolve();
        let Some(om) = self.battle.online.as_ref() else { return };
        let Some(b) = self.battle.battle.as_ref() else { return };
        let other = 1 - om.me;
        let need = [b.needs_switch_side(0), b.needs_switch_side(1)];
        let wants_them = b.winner.is_none() && (need[other] || (!need[0] && !need[1]));
        let missing = om.their_moves.chars().count() <= om.used[other];
        // the battle is over: the challenger marks it finished (it gets cleaned up later)
        if b.winner.is_some() && !om.finished && !self.battle.busy() {
            let (me, id) = (om.me, om.id.clone());
            if let Some(om) = self.battle.online.as_mut() {
                om.finished = true;
            }
            if me == 0 {
                if let Some(client) = self.client.clone() {
                    self.worker.run::<()>(move || client.finish_battle(&id), None, None);
                }
            }
            return;
        }
        if !(wants_them && missing) || om.polling || now < om.next_poll || om.gone {
            return;
        }
        let (id, me) = (om.id.clone(), om.me);
        let Some(client) = self.client.clone() else { return };
        if let Some(om) = self.battle.online.as_mut() {
            om.polling = true;
            om.next_poll = now + POLL;
        }
        self.run_job(
            move || client.get_battle(&id),
            move |g, doc: Option<BattleDoc>| {
                let Some(om) = g.battle.online.as_mut() else { return };
                om.polling = false;
                match doc {
                    None => om.gone = true,
                    Some(d) => {
                        let theirs = if me == 0 { d.to_moves } else { d.from_moves };
                        if theirs.chars().count() > om.their_moves.chars().count() {
                            om.their_moves = theirs;
                        }
                    }
                }
            },
            |g, _e| {
                if let Some(om) = g.battle.online.as_mut() {
                    om.polling = false;
                }
            },
        );
    }

    /// Plays the next step of the battle if both needed moves are in (one step at a time: the events play first).
    fn online_resolve(&mut self) {
        if self.battle.busy() {
            return;
        }
        let evs = {
            let ui = &mut self.battle;
            let (Some(om), Some(b)) = (ui.online.as_mut(), ui.battle.as_mut()) else { return };
            if b.winner.is_some() {
                return;
            }
            let lists: [Vec<char>; 2] = if om.me == 0 {
                [om.my_moves.chars().collect(), om.their_moves.chars().collect()]
            } else {
                [om.their_moves.chars().collect(), om.my_moves.chars().collect()]
            };
            let next = |side: usize, used: &[usize; 2]| lists[side].get(used[side]).copied();
            let mut evs: Vec<Ev> = Vec::new();
            // someone gave up
            if let Some(side) = (0..2).find(|&s| next(s, &om.used) == Some('X')) {
                om.used[side] += 1;
                evs = b.forfeit(side);
            } else {
                let need = [b.needs_switch_side(0), b.needs_switch_side(1)];
                if need[0] || need[1] {
                    for side in 0..2 {
                        if need[side] {
                            if let Some(c) = next(side, &om.used) {
                                let idx = c.to_digit(10).unwrap_or(0) as usize;
                                evs.extend(b.send_out_side(side, idx));
                                om.used[side] += 1;
                            }
                        }
                    }
                } else if let (Some(a), Some(c)) = (next(0, &om.used), next(1, &om.used)) {
                    let acts = [action_of(a).unwrap_or(Action::Use(Move::Strike)), action_of(c).unwrap_or(Action::Use(Move::Strike))];
                    evs = b.turn_both(acts);
                    om.used[0] += 1;
                    om.used[1] += 1;
                }
            }
            if !evs.is_empty() {
                om.waiting_since = now_ts();
            }
            evs
        };
        if !evs.is_empty() {
            self.battle_push(evs);
        }
    }
}
