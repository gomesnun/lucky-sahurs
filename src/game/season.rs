//! v3.0.1 seasons: an admin can reset everyone's progress (admin menu > "Reset everyone"). That raises the season
//! number in /public/season and empties the leaderboard. Every game checks the number at start and every couple of
//! minutes; a save from an older season (the one being played, the other slots on this PC, the account's cloud
//! slots) starts again from 0. Saves already in the new season are never touched, so a new PC is safe.
//! Accounts, friends and settings stay.
//! An admin can also reset ONE player (admin menu > Resets > search): /resets/{uid} goes up by 1 and that player's
//! game wipes their account's saves the same way (the save keeps the number it was made with: `player_reset`).

use super::base::Bo;
use super::{Game, cb};
use crate::config::SAVE_SLOTS;
use crate::core::state::{GameState, now_ts};
use crate::gfx::{Color, Rect};
use crate::i18n::tr;
use crate::online::cloud_cache::delete_cache;
use crate::storage::{delete_slot, save_slot_path};
use crate::theme::*;
use crate::tr;
use serde_json::Value;

/// how often the season is checked while playing
const SEASON_CHECK_EVERY: f64 = 120.0;

#[derive(Default)]
pub struct SeasonUi {
    /// the season on the server (None = not known yet)
    pub server: Option<i64>,
    /// this account's reset number on the server (None = not known / not logged in)
    pub my_reset: Option<i64>,
    /// the account `my_reset` belongs to
    pub my_reset_uid: Option<String>,
    pub next_check: f64,
    pub fetching: bool,
    /// (account, season, reset) whose cloud slots were already checked on this run
    pub cloud_checked: Option<(String, i64, i64)>,
    /// admin: "Reset <player>" is at "Are you sure?"
    pub player_confirm: bool,
    /// admin: the reset button is at "Are you sure?" / running
    pub confirm: bool,
    pub confirm_timer: f64,
    pub busy: bool,
}

/// The season of the save file in this slot on this PC (0 = older than seasons). None = no save there.
fn local_slot_season(slot: i64) -> Option<i64> {
    let text = std::fs::read_to_string(save_slot_path(slot)).ok()?;
    let v: Value = serde_json::from_str(&text).ok()?;
    Some(v.get("season").and_then(crate::storage::value_i64).unwrap_or(0))
}

impl Game {
    pub fn tick_season(&mut self, dt: f64) {
        if self.season.confirm || self.season.player_confirm {
            self.season.confirm_timer -= dt;
            if self.season.confirm_timer <= 0.0 {
                self.season.confirm = false;
                self.season.player_confirm = false;
            }
        }
        let now = now_ts();
        let Some(client) = self.client.clone() else { return };
        // a different account (or logged out): its own reset number has to be read again
        let uid = self.account.as_ref().map(|a| a.uid.clone());
        if self.season.my_reset_uid != uid {
            self.season.my_reset = None;
            self.season.my_reset_uid = uid.clone();
            self.season.next_check = 0.0;
        }
        if self.season.fetching || now < self.season.next_check {
            return;
        }
        self.season.fetching = true;
        self.run_job(
            move || {
                let season = client.get_season()?;
                let mine = if uid.is_some() { Some(client.get_my_reset()?) } else { None };
                Ok((season, mine, uid))
            },
            |g, (n, mine, uid): (i64, Option<i64>, Option<String>)| {
                g.season.fetching = false;
                g.season.next_check = now_ts() + SEASON_CHECK_EVERY;
                g.season.server = Some(n);
                if g.season.my_reset_uid == uid {
                    g.season.my_reset = mine;
                }
                g.on_season_known();
            },
            |g, _| {
                g.season.fetching = false;
                g.season.next_check = now_ts() + SEASON_CHECK_EVERY;
            },
        );
    }

    /// Wipes whatever is from an older season: the save being played, this PC's slots and the account's cloud slots.
    pub fn on_season_known(&mut self) {
        self.enforce_season_state();
        let r = self.season.my_reset.unwrap_or(0);
        let n = self.season.server.unwrap_or(0);
        if r > 0 || n > 0 {
            self.check_cloud_season(n, r);
        }
        if n <= 0 {
            return;
        }
        // this PC's slots (not tied to an account): only the season counts
        for slot in 1..=SAVE_SLOTS {
            if self.state.slot == Some(slot) {
                continue; // already handled (and saved) above
            }
            if local_slot_season(slot).is_some_and(|s| s < n) {
                delete_slot(slot);
            }
        }
        self.refresh_slot_info();
    }

    /// The save being played: from an older season with progress -> starts again from 0 (keeping the slot, the cloud
    /// link and the settings). Called when the season becomes known and whenever a save is loaded.
    pub fn enforce_season_state(&mut self) {
        if self.state.slot.is_none() {
            return;
        }
        let n = self.season.server.unwrap_or(0).max(self.state.season);
        // the personal reset only counts for this account's cloud saves
        let mine = self.account.as_ref().is_some_and(|a| self.state.cloud_uid.as_deref() == Some(a.uid.as_str()));
        let r = if mine { self.season.my_reset.unwrap_or(0).max(self.state.player_reset) } else { self.state.player_reset };
        if self.state.season >= n && self.state.player_reset >= r {
            return;
        }
        let personal = self.state.season >= n;
        if self.state.total_rolls == 0 && self.state.coins == 0.0 {
            // a brand-new save: just mark it
            self.state.season = n;
            self.state.player_reset = r;
            self.state.dirty = true;
            return;
        }
        let mut fresh = GameState::new();
        fresh.slot = self.state.slot;
        fresh.season = n;
        fresh.player_reset = r;
        fresh.prefs = self.state.prefs.clone();
        fresh.cloud_uid = self.state.cloud_uid.clone();
        fresh.cloud_base_time = self.state.cloud_base_time.clone();
        fresh.save_seq = self.state.save_seq + 1;
        fresh.dirty = true;
        let old = std::mem::replace(&mut self.state, fresh);
        if self.upload_inflight {
            self.orphan_states.insert(old.obj_id, old);
        }
        self.state.save();
        self.reset_ui();
        self.show_toast(&if personal { tr("An admin reset your progress: you start again from 0.") } else { tr("A new season started! Everyone starts again from 0.") }, 4.0);
    }

    /// The account's other cloud slots from an older season / reset are deleted (once per account, season and reset
    /// on this run).
    fn check_cloud_season(&mut self, n: i64, r: i64) {
        let (Some(client), Some(acc)) = (self.client.clone(), self.account.clone()) else { return };
        if self.season.cloud_checked.as_ref() == Some(&(acc.uid.clone(), n, r)) {
            return;
        }
        self.season.cloud_checked = Some((acc.uid.clone(), n, r));
        let playing = self.state.slot;
        self.run_job(
            move || {
                let mut old = Vec::new();
                for slot in 1..=SAVE_SLOTS {
                    if Some(slot) == playing {
                        continue;
                    }
                    if let Some(doc) = client.get_save(slot)? {
                        let num = |k: &str| doc.data.get(k).and_then(crate::storage::value_i64).unwrap_or(0);
                        if num("season") < n || num("player_reset") < r {
                            client.delete_save(slot)?;
                            old.push(slot);
                        }
                    }
                }
                Ok(old)
            },
            move |g, old: Vec<i64>| {
                for slot in &old {
                    delete_cache(&acc.uid, *slot);
                    g.cache_info.remove(slot);
                    g.cloud_slots.remove(slot);
                }
                if !old.is_empty() {
                    g.load_cloud_slots();
                }
            },
            |g, _| g.season.cloud_checked = None, // try again at the next check
        );
    }

    // ---------------------------------------------------------------- admin
    /// 1st click: "Are you sure?". 2nd click: new season for everyone + the leaderboard emptied.
    pub fn press_reset_everyone(&mut self) {
        if self.season.busy {
            return;
        }
        if !self.season.confirm {
            self.season.confirm = true;
            self.season.confirm_timer = 5.0;
            return;
        }
        self.season.confirm = false;
        let Some(client) = self.client.clone() else { return };
        self.season.busy = true;
        self.run_job(
            move || {
                let n = client.get_season()? + 1;
                client.set_season(n)?;
                let removed = client.clear_leaderboard().unwrap_or(0);
                Ok((n, removed))
            },
            |g, (n, removed): (i64, usize)| {
                g.season.busy = false;
                g.season.server = Some(n);
                g.lb_data = None;
                g.show_toast(&tr!("Season %d started: everyone starts from 0 (%d leaderboard entries removed).", n, removed as i64), 4.0);
                g.on_season_known();
            },
            |g, e| {
                g.season.busy = false;
                g.show_toast(&crate::online::firebase::online_error_text(&e), 3.0);
            },
        );
    }

    /// Admin, Resets page: 1st click "Are you sure?", 2nd click resets the player that was found.
    pub fn press_reset_player(&mut self) {
        let Some(found) = self.adm.found.clone() else { return };
        if self.season.busy {
            return;
        }
        if !self.season.player_confirm {
            self.season.player_confirm = true;
            self.season.confirm_timer = 5.0;
            return;
        }
        self.season.player_confirm = false;
        let Some(client) = self.client.clone() else { return };
        self.season.busy = true;
        let uid = found.uid.clone();
        let name = found.username.clone();
        self.run_job(
            move || client.reset_player(&uid),
            move |g, _n: i64| {
                g.season.busy = false;
                g.adm.msg = Some((tr!("%s will start again from 0 the next time their game checks (within a couple of minutes if they're playing).", name), GOOD));
                g.season.next_check = 0.0; // if it's you, right away
            },
            |g, e| {
                g.season.busy = false;
                g.adm.msg = Some((crate::online::firebase::online_error_text(&e), BAD));
            },
        );
    }

    pub fn draw_reset_everyone_button(&mut self, rect: Rect, mouse_pos: (f64, f64)) {
        let med = self.f.med.clone();
        let (label, base) = if self.season.busy {
            (tr("Resetting..."), panel_light())
        } else if self.season.confirm {
            (tr("Sure? This wipes EVERYONE's progress"), BAD)
        } else {
            (tr("Reset everyone (new season)"), panel_light())
        };
        self.button(rect, &label, &med, mouse_pos, base, Color::rgb(235, 90, 90), WHITE, cb(|g| g.press_reset_everyone()), Bo::r(12).enabled(!self.season.busy).icon("rebirth"));
    }
}
