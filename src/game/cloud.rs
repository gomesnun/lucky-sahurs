//! Cloud account: session, online saves, sync and leaderboard publishing (online/cloud.py).

use super::title_saves::{SlotInfo, offline_message};
use super::{Account, Game};
use crate::config::SAVE_SLOTS;
use crate::core::state::{GameState, now_ts, rand_uniform};
use crate::i18n::tr;
use crate::online::cloud_cache::{
    CacheEntry, backup_state, clear_session, delete_cache, lb_publish_period, load_session, peek_cache, pick_save, read_cache, store_session, write_cache,
};
use crate::online::firebase::{
    CLOUD_SYNC_INTERVAL, CloudDoc, LEADERBOARD_MIN_GAP, LEADERBOARD_PERIOD, LEADERBOARD_PUBLISH_JITTER, OnlineError, SESSION_HEARTBEAT, online_error_text, save_summary,
};
use crate::storage::save_slot_path;
use crate::tr;
use serde_json::Value;

pub struct StartRes {
    busy: bool,
    held: bool,
    cloud: Option<CloudDoc>,
}

impl Game {
    pub fn restore_session(&mut self) {
        let Some(client) = self.client.clone() else { return };
        let Some(sess) = load_session() else { return };
        client.restore(&sess.uid, &sess.username, &sess.refresh_token);
        self.account = Some(Account { uid: sess.uid.clone(), username: sess.username.clone(), email: None, email_verified: false });
        self.pub_last_time = match sess.raw.get("last_pub_time") {
            None => 0.0,
            Some(v) => crate::storage::value_f64(v).unwrap_or(0.0),
        };
        self.refresh_slot_info();
        self.load_cloud_slots();
        let c = client.clone();
        self.run_job(move || c.get_profile(), |g, p| g.on_startup_profile(p), |_, _| {});
    }

    pub fn on_startup_profile(&mut self, profile: Option<crate::online::firebase::AccountProfile>) {
        let Some(acc) = self.account.as_mut() else { return };
        if let Some(p) = &profile {
            if p.email_verified {
                acc.email_verified = true;
                acc.email = p.email.clone();
                return;
            }
            if p.email.as_deref().is_some_and(|e| !e.is_empty()) {
                acc.email = p.email.clone();
                self.acc.pending_email = p.email.clone();
            }
        }
        self.require_email_link();
    }

    pub fn save_session_file(&self) {
        if let (Some(c), Some(a)) = (&self.client, &self.account) {
            if let Some(rt) = c.refresh_token().filter(|s| !s.is_empty()) {
                store_session(&a.uid, &a.username, &rt, self.pub_last_time);
            }
        }
    }

    pub fn log_out(&mut self) {
        self.session_held = false;
        if let Some(c) = &self.client {
            c.sign_out();
        }
        self.account = None;
        self.ev.is_admin = false;
        self.adm.menu_open = false;
        self.adm.ban_open = false;
        self.ev.admin_checked = false;
        self.ev.admin_open = false;
        self.cloud_slots.clear();
        self.cloud_slots_loaded = false;
        self.cloud_slots_loading = false;
        self.cloud_slots_error = None;
        self.cache_info.clear();
        self.pub_last_time = 0.0;
        self.acc.stage = "form";
        self.acc.pending_email = None;
        clear_session();
    }

    pub fn log_out_clicked(&mut self) {
        self.log_out();
        self.show_toast(&tr("Logged out."), 1.8);
        self.close_account();
    }

    pub fn load_cloud_slots(&mut self) {
        let (Some(client), Some(acc)) = (self.client.clone(), self.account.clone()) else { return };
        if self.cloud_slots_loading {
            return;
        }
        self.cloud_slots_loading = true;
        self.cloud_slots_error = None;
        self.cloud_slots_error_msg.clear();
        self.cache_info = (1..=SAVE_SLOTS).filter_map(|i| peek_cache(&acc.uid, i).map(|c| (i, c))).collect();
        self.run_job(
            move || client.list_saves(),
            |g, slots| {
                g.cloud_slots_loading = false;
                g.cloud_slots = slots;
                g.cloud_slots_loaded = true;
            },
            |g, e| {
                g.cloud_slots_loading = false;
                if e.code == "auth" {
                    g.log_out();
                    g.show_toast(&tr("Session expired. Please log in again."), 1.8);
                } else {
                    g.cloud_slots_error = Some(e.code.clone());
                    let detail = e.message.trim().to_string();
                    g.cloud_slots_error_msg = online_error_text(&e) + &if detail.is_empty() { String::new() } else { format!(" [{}]", detail) };
                }
            },
        );
    }

    /// (kind, summary, label) for a slot card on the Saves screen.
    pub fn slot_view(&self, slot: i64) -> (&'static str, Option<SlotInfo>, &'static str) {
        let local = self.slot_info.get(&slot).cloned().flatten().map(|i| SlotInfo { coins: i.coins, total_rolls: i.total_rolls, playtime: i.playtime });
        if self.account.is_none() {
            return match local {
                Some(i) => ("save", Some(i), ""),
                None => ("empty", None, ""),
            };
        }
        let cached = self.cache_info.get(&slot).map(|c| SlotInfo { coins: c.coins, total_rolls: c.total_rolls, playtime: c.playtime });
        if self.cloud_slots_loaded {
            if let Some(s) = self.cloud_slots.get(&slot) {
                return ("save", Some(SlotInfo { coins: s.coins, total_rolls: s.rolls, playtime: s.playtime }), "Cloud");
            }
            if let Some(c) = cached {
                return ("save", Some(c), "Not uploaded yet");
            }
            if let Some(l) = local {
                return ("importable", Some(l), "Local save found");
            }
            return ("empty", None, "");
        }
        if let Some(c) = cached {
            return ("save", Some(c), "Offline copy");
        }
        ("unknown", None, "")
    }

    pub fn start_cloud_slot(&mut self, slot: i64) {
        if self.slot_starting {
            return;
        }
        if self.upload_inflight {
            self.show_toast(&tr("Syncing your save... try again in a second."), 1.8);
            return;
        }
        let (Some(client), Some(acc)) = (self.client.clone(), self.account.clone()) else { return };
        let uid = acc.uid.clone();
        let cache = read_cache(&uid, slot);
        self.slot_starting = true;
        self.show_toast(&tr("Loading save..."), 1.8);
        let iid = self.install_id.clone();
        let cache2 = cache.clone();
        let uid2 = uid.clone();
        self.run_job(
            move || {
                // taking this account over on THIS device, even if another one currently holds it - it will
                // notice (tick_session) and back off on its own next heartbeat, the same way a save conflict
                // already does
                let mut held = false;
                match client.write_session(&iid, true) {
                    Ok(()) => held = true,
                    Err(e) if e.code == "denied" => return Ok(StartRes { busy: true, held: false, cloud: None }),
                    Err(_) => {}
                }
                let cloud = client.get_save(slot)?;
                Ok(StartRes { busy: false, held, cloud })
            },
            move |g, res: StartRes| {
                g.slot_starting = false;
                if res.busy {
                    g.show_toast(&tr("Couldn't claim this account right now. Try again."), 3.0);
                    return;
                }
                g.session_held = res.held;
                g.session_timer = 0.0;
                let p = pick_save(res.cloud.as_ref().map(|c| crate::online::cloud_cache::CloudSave { data: c.data.clone(), time: c.time.clone() }).as_ref(), cache.as_ref());
                if let Some((label, st)) = &p.loser {
                    backup_state(&uid, slot, label, st);
                }
                g.begin_cloud_game(slot, p.state, p.base_time, p.dirty, p.note);
            },
            move |g, e: OnlineError| {
                g.slot_starting = false;
                if e.code == "auth" {
                    g.show_toast(&tr("Session expired. Please log in again."), 1.8);
                } else if let (Some(c), true) = (cache2.as_ref(), ["offline", "server", "denied"].contains(&e.code.as_str())) {
                    g.session_held = false;
                    g.session_timer = 0.0;
                    let _ = &uid2;
                    g.begin_cloud_game(slot, Some(c.state.clone()), c.base_time.clone(), true, Some(tr("Offline: playing from this PC's copy. It will sync later.")));
                } else {
                    g.show_toast(&online_error_text(&e), 1.8);
                }
            },
        );
    }

    pub fn begin_cloud_game(&mut self, slot: i64, state: Option<Value>, base_time: Option<String>, dirty: bool, note: Option<String>) {
        let Some(acc) = self.account.clone() else { return };
        let mut st = GameState::new();
        st.slot = Some(slot);
        st.cloud_uid = Some(acc.uid.clone());
        let mut dirty = dirty;
        if let Some(d) = &state {
            if st.load_dict(d).is_err() {
                self.show_toast(&tr("This save couldn't be read. Nothing was changed."), 1.8);
                return;
            }
        }
        let (gain, away) = st.claim_offline_earnings();
        if gain > 0.0 {
            dirty = true;
        }
        st.cloud_base_time = base_time.clone();
        st.dirty = dirty;
        write_cache(&acc.uid, slot, base_time.as_deref(), dirty, &st.to_dict());
        self.replace_state(st);
        self.reset_ui();
        self.options_open = false;
        self.stats_open = false;
        self.leaderboard_open = false;
        self.autosave_timer = 0.0;
        self.sync_timer = CLOUD_SYNC_INTERVAL - 5.0;
        self.sync_error = None;
        self.screen_mode = "game";
        self.on_enter_game();
        let text = note.unwrap_or_else(|| tr!("Slot %d loaded", slot));
        if gain > 0.0 {
            self.show_toast(&format!("{}\n{}", text, offline_message(gain, away)), 6.0);
        } else {
            self.show_toast(&text, 1.8);
        }
    }

    pub fn import_local_slot(&mut self, slot: i64) {
        if self.import_busy {
            return;
        }
        let Some(client) = self.client.clone() else { return };
        let data = std::fs::read_to_string(save_slot_path(slot))
            .ok()
            .and_then(|t| serde_json::from_str::<Value>(&t).ok())
            .filter(|d| d.is_object() && GameState::new().load_dict(d).is_ok());
        let Some(data) = data else {
            self.show_toast(&tr!("Couldn't read the local save for slot %d.", slot), 1.8);
            return;
        };
        self.import_busy = true;
        self.run_job(
            move || client.put_save(slot, &data, None),
            move |g, _t| {
                g.import_busy = false;
                g.show_toast(&tr!("Slot %d imported. Your local file was kept.", slot), 1.8);
                g.load_cloud_slots();
            },
            move |g, e| {
                g.import_busy = false;
                if e.code == "conflict" {
                    g.show_toast(&tr!("Slot %d already has a cloud save - nothing was overwritten.", slot), 1.8);
                    g.load_cloud_slots();
                } else {
                    g.show_toast(&online_error_text(&e), 1.8);
                }
            },
        );
    }

    pub fn delete_account_slot(&mut self, slot: i64) {
        let (Some(client), Some(acc)) = (self.client.clone(), self.account.clone()) else { return };
        delete_cache(&acc.uid, slot);
        self.clear_slot_name(slot);
        self.cache_info.remove(&slot);
        self.cloud_slots.remove(&slot);
        self.run_job(
            move || client.delete_save(slot),
            move |g, _| {
                g.show_toast(&tr!("Slot %d deleted.", slot), 1.8);
                g.load_cloud_slots();
            },
            |g, e| {
                g.show_toast(&online_error_text(&e), 1.8);
                g.load_cloud_slots();
            },
        );
    }

    // ================================================================ sync
    pub fn start_upload(&mut self) {
        let Some(client) = self.client.clone() else { return };
        let st = &self.state;
        let (Some(uid), Some(slot)) = (st.cloud_uid.clone(), st.slot) else { return };
        if !st.dirty || st.sync_conflict || self.upload_inflight || slot == 0 || uid.is_empty() {
            return;
        }
        let snap = st.to_dict();
        let (seq, base, sid) = (st.save_seq, st.cloud_base_time.clone(), st.obj_id);
        self.upload_inflight = true;
        let snap2 = snap.clone();
        self.run_job(
            move || client.put_save(slot, &snap2, base.as_deref()),
            move |g, update_time: Option<String>| {
                g.upload_inflight = false;
                if let Some(st) = g.state_by_id(sid) {
                    st.cloud_base_time = update_time.clone();
                    if st.save_seq == seq {
                        st.dirty = false;
                    }
                    let d = if !st.dirty { snap.clone() } else { st.to_dict() };
                    write_cache(&uid, slot, update_time.as_deref(), st.dirty, &d);
                }
                let mut summary = save_summary(&snap);
                summary.time = update_time;
                g.cloud_slots.insert(slot, summary);
                g.sync_error = None;
                g.sync_last_ok = Some(now_ts());
            },
            move |g, e| {
                g.upload_inflight = false;
                if e.code == "conflict" {
                    if let Some(st) = g.state_by_id(sid) {
                        st.sync_conflict = true;
                    }
                    g.sync_error = Some("conflict".into());
                    g.show_toast(&tr("This save changed on another device. Go to the main menu and open the slot again."), 1.8);
                } else if e.code == "auth" || e.code == "denied" {
                    g.sync_error = Some(e.code.clone());
                } else {
                    g.sync_error = Some("offline".into());
                }
            },
        );
    }

    pub fn flush_cloud_blocking(&mut self) {
        self.upload_cloud_blocking();
        self.release_session_blocking();
    }

    pub fn upload_cloud_blocking(&mut self) {
        let Some(client) = self.client.clone() else { return };
        if self.state.cloud_uid.is_none() || self.state.slot.is_none() {
            return;
        }
        let deadline = now_ts() + 6.0;
        while self.upload_inflight && now_ts() < deadline {
            self.poll_worker();
            std::thread::sleep(std::time::Duration::from_millis(50));
        }
        let st = &self.state;
        if !st.dirty || st.sync_conflict {
            return;
        }
        let snap = st.to_dict();
        let Ok(t) = client.put_save(st.slot.unwrap(), &snap, st.cloud_base_time.as_deref()) else { return };
        let st = &mut self.state;
        st.cloud_base_time = t.clone();
        st.dirty = false;
        write_cache(st.cloud_uid.as_deref().unwrap_or(""), st.slot.unwrap(), t.as_deref(), false, &snap);
    }

    // ================================================================ one account, one game at a time
    pub fn tick_session(&mut self, dt: f64) {
        if self.session_inflight {
            return;
        }
        self.session_timer += dt;
        if self.session_timer < SESSION_HEARTBEAT {
            return;
        }
        self.session_timer = 0.0;
        self.session_inflight = true;
        let Some(client) = self.client.clone() else { return };
        let iid = self.install_id.clone();
        self.run_job(
            move || {
                // someone else has since claimed this account on another device: back off rather than fight
                // them for the lock (whoever claims it most recently wins - see start_cloud_slot)
                if client.session_blocker(&iid)?.is_some() {
                    return Ok(false);
                }
                client.write_session(&iid, true)?;
                Ok(true)
            },
            |g, held: bool| {
                g.session_inflight = false;
                if g.session_held && !held {
                    // just lost it: stop autosaving over whatever the other device is now writing - the same
                    // recovery the save-conflict path already offers
                    g.state.sync_conflict = true;
                    g.show_toast(&tr("This account is being played on another device now. Go to the main menu and open the slot again."), 4.0);
                }
                g.session_held = held;
            },
            |g, _| {
                g.session_inflight = false;
                g.session_held = false;
            },
        );
    }

    pub fn release_session(&mut self) {
        let Some(client) = self.client.clone() else { return };
        if self.account.is_none() || !self.session_held {
            return;
        }
        self.session_held = false;
        self.session_timer = 0.0;
        let iid = self.install_id.clone();
        self.run_job_quiet(move || client.write_session(&iid, false));
    }

    pub fn release_session_blocking(&mut self) {
        let Some(client) = self.client.clone() else { return };
        if self.account.is_none() || !self.session_held {
            return;
        }
        self.session_held = false;
        let _ = client.write_session(&self.install_id, false);
    }

    pub fn tick_online(&mut self, dt: f64) {
        if self.client.is_none() || self.account.is_none() || self.state.cloud_uid.is_none() {
            return;
        }
        let now = now_ts();
        self.sync_timer += dt;
        if self.sync_timer >= CLOUD_SYNC_INTERVAL {
            self.sync_timer = 0.0;
            self.start_upload();
        }
        self.tick_session(dt);
        if !self.cloud_slots_loaded && !self.cloud_slots_loading && now >= self.slots_retry_at {
            self.slots_retry_at = now + 60.0;
            self.load_cloud_slots();
        }
        self.tick_publish(now);
        self.tick_friends(now);
        self.tick_chat(now);
        self.tick_events(now);
        self.tick_wallet(now);
    }

    /// (money, playtime, rolls, rebirths, prestige). Rebirths/Prestige: the best slot, Prestige first (the other
    /// slots' Prestige isn't in the cloud summary, so they count as 0 there).
    pub fn leaderboard_values(&self) -> (f64, f64, i64, i64, i64) {
        let st = &self.state;
        let (mut best_coins, mut best_rb, mut total_pt, mut total_rolls) = (0.0f64, (0i64, 0i64), 0.0f64, 0i64);
        for slot in 1..=SAVE_SLOTS {
            let (coins, pt, rolls, rb) = if Some(slot) == st.slot {
                (st.total_coins_earned, st.playtime, st.total_rolls, (st.prestige, st.rebirths))
            } else if let Some(s) = self.cloud_slots.get(&slot) {
                (s.total_earned, s.playtime, s.rolls, (0, s.rebirths))
            } else {
                (0.0, 0.0, 0, (0, 0))
            };
            best_coins = best_coins.max(coins);
            best_rb = best_rb.max(rb);
            total_pt += pt;
            total_rolls += rolls;
        }
        (best_coins, total_pt, total_rolls, best_rb.1, best_rb.0)
    }

    pub fn tick_publish(&mut self, now: f64) {
        if self.pub_inflight || !self.cloud_slots_loaded || now < self.pub_retry_at {
            return;
        }
        let period = lb_publish_period(now);
        if self.pub_last_time != 0.0 && lb_publish_period(self.pub_last_time) >= period {
            return;
        }
        let due = (period as f64 * LEADERBOARD_PERIOD + self.pub_jitter).max(self.pub_last_time + LEADERBOARD_MIN_GAP);
        if now < due {
            return;
        }
        let Some(client) = self.client.clone() else { return };
        let v = self.leaderboard_values();
        // the Prestige is only sent once the rules allow it (a "denied" with it -> sent without it from then on)
        let prestige = if self.lb_no_prestige { None } else { Some(v.4).filter(|p| *p > 0) };
        self.pub_inflight = true;
        self.run_job(
            move || client.publish_score(v.0, v.1, v.2, v.3, prestige),
            |g, _| {
                g.pub_inflight = false;
                g.pub_last_time = now_ts();
                g.pub_jitter = rand_uniform(0.0, LEADERBOARD_PUBLISH_JITTER);
                g.save_session_file();
            },
            move |g, e| {
                g.pub_inflight = false;
                if e.code == "denied" && prestige.is_some() && !g.lb_no_prestige {
                    // maybe the rules don't have "prestige" yet: try once without it
                    g.lb_no_prestige = true;
                    g.pub_retry_at = now_ts() + 5.0;
                } else if e.code == "denied" && prestige.is_none() && g.lb_no_prestige {
                    // refused without it too: it wasn't the prestige (e.g. the 5-minute limit) - send it again next time
                    g.lb_no_prestige = false;
                    g.pub_retry_at = now_ts() + LEADERBOARD_PERIOD;
                } else if e.code == "denied" || e.status == 429 {
                    g.pub_retry_at = now_ts() + LEADERBOARD_PERIOD;
                } else {
                    g.pub_retry_at = now_ts() + 120.0;
                }
            },
        );
    }
}

#[allow(dead_code)]
fn _cache_type_check(c: CacheEntry) -> CacheEntry {
    c
}
