//! Pet trades with friends (online/trades.py) and their screen inside the Friends page (ui/trades_panel.py).
//!
//! How it works (see online/firebase.rs for the Firestore details):
//! - The "wallet" (/users/{uid}/wallet/pets) is the online copy of what you have - it's what the Firestore rules
//!   use to confirm a trade, never the local save (nobody outside can read that). It syncs by itself, bit by bit,
//!   whenever you roll new pets (see tick_wallet).
//! - Proposing a trade creates a /trades/{id} document. The friend accepts or declines.
//! - Accepting changes the accepter's wallet right away and leaves a "receipt" in the proposer's inbox. That
//!   receipt is only applied (and leaves the inbox) the next time THEY have the game open - tick_wallet handles
//!   it by itself, the player doesn't have to do anything.

use super::base::Bo;
use super::{Game, cb};
use crate::core::data::{MUT_ORDER, mutation, pet_order, rarities};
use crate::core::formatting::format_number;
use crate::core::state::now_ts;
use crate::gfx::{Color, Rect, draw, ti};
use crate::i18n::tr;
use crate::online::firebase::{MAX_TRADE_ITEMS, OnlineError, Receipt, Trade, online_error_text};
use crate::theme::*;
use crate::tr;
use crate::ui::fonts::{fit_text, wrap_text};
use indexmap::IndexMap;
use std::collections::HashMap;

/// seconds between sending what you rolled since the last time
pub const WALLET_FLUSH_INTERVAL: f64 = 45.0;
/// how often it checks whether a receipt arrived (5 min)
pub const INCOMING_POLL_INTERVAL: f64 = 300.0;
/// with the Trades page open, how often it refreshes
pub const TRADES_POLL: f64 = 30.0;
pub const MAX_TRADE_QTY: i64 = 999999999;

const ROW_H: i32 = 40;
const STEP_W: i32 = 28;

/// "3_golden" -> "Golden Fidelity" (the verity's name; the mutation in the current language).
pub fn pet_key_label(key: &str) -> String {
    let Some((idx_s, m)) = key.split_once('_') else { return key.to_string() };
    let Ok(idx) = idx_s.trim().parse::<i64>() else { return key.to_string() };
    let Some(mt) = mutation(m) else { return key.to_string() };
    if idx < 0 || idx as usize >= rarities().len() {
        return key.to_string();
    }
    let prefix = if mt.label.is_empty() { String::new() } else { tr(mt.label) };
    format!("{} {}", prefix, rarities()[idx as usize].pet).trim().to_string()
}

fn fmt_items(items: &[(String, i64)]) -> String {
    if items.is_empty() {
        return "-".into();
    }
    items.iter().map(|(k, q)| format!("{} x{}", pet_key_label(k), q)).collect::<Vec<_>>().join(", ")
}

pub struct TradesUi {
    pub wallet_flush_at: f64,
    pub incoming_check_at: f64,
    pub wallet_syncing: bool,
    pub incoming_checking: bool,
    /// (uid, save) whose online wallet was already set equal to the save
    pub wallet_full_for: Option<(String, u64)>,

    /// uid of the friend a trade is being proposed to now
    pub target: Option<String>,
    pub target_name: String,
    /// "idx_mut" -> how many I'm offering
    pub my_offer: IndexMap<String, i64>,
    /// "idx_mut" -> how many I'm asking for
    pub their_request: IndexMap<String, i64>,
    /// the friend's wallet (None = not here yet)
    pub their_wallet: Option<HashMap<String, i64>>,
    pub wallet_busy: bool,
    pub sending: bool,
    pub msg: Option<(String, Color)>,

    pub sent: Vec<Trade>,
    pub received: Vec<Trade>,
    pub loaded: bool,
    pub loading: bool,
    pub error: Option<String>,
    pub next_refresh: f64,
    /// id of the trade with an action running (buttons off)
    pub action: Option<String>,
}

impl TradesUi {
    pub fn new() -> TradesUi {
        TradesUi {
            wallet_flush_at: 0.0,
            incoming_check_at: 0.0,
            wallet_syncing: false,
            incoming_checking: false,
            wallet_full_for: None,
            target: None,
            target_name: String::new(),
            my_offer: IndexMap::new(),
            their_request: IndexMap::new(),
            their_wallet: None,
            wallet_busy: false,
            sending: false,
            msg: None,
            sent: Vec::new(),
            received: Vec::new(),
            loaded: false,
            loading: false,
            error: None,
            next_refresh: 0.0,
            action: None,
        }
    }
}

fn add_to(map: &mut IndexMap<String, i64>, items: &IndexMap<String, i64>) {
    for (k, q) in items {
        *map.entry(k.clone()).or_insert(0) += q;
    }
}

impl Game {
    // ---------------------------------------------------------------- wallet: automatic sync
    /// Called now and then (see tick_online): sends to the cloud what you rolled since the last time, and applies
    /// to the local save any trade receipt that arrived - all without the player seeing anything but the
    /// inventory changing.
    pub fn tick_wallet(&mut self, now: f64) {
        if !(self.client.is_some() && self.account.is_some() && self.state.cloud_uid.is_some()) {
            return;
        }
        let key = (self.account.as_ref().unwrap().uid.clone(), self.state.obj_id);
        if !self.trades.wallet_syncing && self.trades.wallet_full_for.as_ref() != Some(&key) {
            self.sync_wallet_full(now, key);
            return;
        }
        if !self.trades.wallet_syncing && !self.state.wallet_pending.is_empty() && now >= self.trades.wallet_flush_at {
            self.flush_wallet(now);
        }
        if !self.trades.incoming_checking && now >= self.trades.incoming_check_at {
            self.check_incoming(now);
        }
    }

    /// On entering the game (or changing save): the online wallet becomes EXACTLY what you have. Before, this
    /// only happened when proposing a trade, so whoever never proposed one showed friends an empty wallet.
    fn sync_wallet_full(&mut self, now: f64, key: (String, u64)) {
        let snapshot: Vec<(String, i64)> = self.state.owned.iter().filter(|(_, v)| **v > 0).map(|(k, v)| (k.clone(), *v)).collect();
        // everything is already in the total; what you roll next is added on top
        let pending = std::mem::take(&mut self.state.wallet_pending);
        self.trades.wallet_syncing = true;
        self.trades.wallet_flush_at = now + WALLET_FLUSH_INTERVAL;
        let client = self.client.clone().unwrap();
        let sid = self.state.obj_id;
        self.run_job(
            move || client.set_wallet_full(&snapshot),
            move |g, _: ()| {
                g.trades.wallet_syncing = false;
                g.trades.wallet_full_for = Some(key);
            },
            move |g, _| {
                g.trades.wallet_syncing = false;
                if let Some(st) = g.state_by_id(sid) {
                    add_to(&mut st.wallet_pending, &pending); // try again later, without losing anything
                }
                g.trades.wallet_flush_at = now + 60.0;
                g.trades.wallet_full_for = None;
            },
        );
    }

    fn flush_wallet(&mut self, now: f64) {
        let delta = std::mem::take(&mut self.state.wallet_pending);
        let list: Vec<(String, i64)> = delta.iter().map(|(k, v)| (k.clone(), *v)).collect();
        self.trades.wallet_syncing = true;
        self.trades.wallet_flush_at = now + WALLET_FLUSH_INTERVAL;
        let client = self.client.clone().unwrap();
        let sid = self.state.obj_id;
        self.run_job(
            move || client.sync_wallet(&list),
            |g, _: ()| g.trades.wallet_syncing = false,
            move |g, e: OnlineError| {
                g.trades.wallet_syncing = false;
                // doesn't lose what wasn't sent - tries again on the next tick
                if let Some(st) = g.state_by_id(sid) {
                    add_to(&mut st.wallet_pending, &delta);
                }
                if e.code != "offline" && e.code != "denied" {
                    g.trades.wallet_flush_at = now + WALLET_FLUSH_INTERVAL;
                }
            },
        );
    }

    fn check_incoming(&mut self, now: f64) {
        self.trades.incoming_checking = true;
        self.trades.incoming_check_at = now + INCOMING_POLL_INTERVAL;
        let client = self.client.clone().unwrap();
        self.run_job(
            move || client.list_incoming(),
            |g, receipts: Vec<Receipt>| {
                g.trades.incoming_checking = false;
                if receipts.is_empty() {
                    return;
                }
                let mut gained: Vec<String> = Vec::new();
                for receipt in receipts {
                    let st = &mut g.state;
                    for (k, q) in &receipt.give {
                        *st.owned.entry(k.clone()).or_insert(0) += q;
                        if *q > 0 {
                            st.seen_pets.insert(k.clone());
                            gained.push(k.clone());
                        }
                    }
                    for (k, q) in &receipt.take {
                        let v = st.owned.get(k).copied().unwrap_or(0);
                        st.owned.insert(k.clone(), (v - q).max(0));
                    }
                    let client = g.client.clone().unwrap();
                    g.worker.run::<()>(move || client.apply_incoming(&receipt), None, Some(Box::new(|_: &mut Game, _| {})));
                }
                g.state.dirty = true;
                if !gained.is_empty() {
                    let names: String = gained.iter().map(|k| pet_key_label(k)).collect::<Vec<_>>().join(", ").chars().take(80).collect();
                    let t = tr!("Trade completed! You received: %s", names);
                    g.show_toast(&t, 2.2);
                }
            },
            |g, _| g.trades.incoming_checking = false,
        );
    }

    // ---------------------------------------------------------------- proposing a trade
    pub fn trades_ready(&self) -> bool {
        self.client.is_some() && self.account.is_some() && self.state.cloud_uid.is_some()
    }

    pub fn open_trade_propose(&mut self, uid: &str, username: &str) {
        if !self.trades_ready() {
            return;
        }
        self.trades.target = Some(uid.to_string());
        self.trades.target_name = if username.is_empty() { uid.to_string() } else { username.to_string() };
        self.fr.scroll = 0.0;
        self.trades.my_offer.clear();
        self.trades.their_request.clear();
        self.trades.msg = None;
        self.trades.their_wallet = None;
        self.trades.wallet_busy = true;
        let client = self.client.clone().unwrap();
        let uid = uid.to_string();
        self.run_job(
            move || client.get_wallet(Some(&uid)),
            |g, wallet: HashMap<String, i64>| {
                g.trades.wallet_busy = false;
                g.trades.their_wallet = Some(wallet);
            },
            |g, e| {
                g.trades.wallet_busy = false;
                g.trades.their_wallet = Some(HashMap::new());
                g.trades.msg = Some((online_error_text(&e), BAD));
            },
        );
    }

    pub fn close_trade_propose(&mut self) {
        self.trades.target = None;
        self.trades.their_wallet = None;
    }

    /// [(key, count)] of what you have, by rarity - to pick what you offer.
    pub fn my_owned_for_trade(&self) -> Vec<(String, i64)> {
        let mut out = Vec::new();
        for &idx in pet_order() {
            for m in MUT_ORDER {
                let key = format!("{}_{}", idx, m);
                let q = self.state.owned.get(&key).copied().unwrap_or(0);
                if q > 0 {
                    out.push((key, q));
                }
            }
        }
        out
    }

    /// The same, from the friend's online wallet (what they can really offer/confirm).
    pub fn their_wallet_for_trade(&self) -> Vec<(String, i64)> {
        let empty = HashMap::new();
        let wallet = self.trades.their_wallet.as_ref().unwrap_or(&empty);
        let mut out = Vec::new();
        for &idx in pet_order() {
            for m in MUT_ORDER {
                let key = format!("{}_{}", idx, m);
                let q = wallet.get(&key).copied().unwrap_or(0);
                if q > 0 {
                    out.push((key, q));
                }
            }
        }
        out
    }

    /// side: "offer" (what I give) or "request" (what I ask for).
    pub fn set_trade_offer_qty(&mut self, side: &str, key: &str, qty: i64) {
        let cap = if side == "offer" {
            self.state.owned.get(key).copied().unwrap_or(0)
        } else {
            self.trades.their_wallet.as_ref().and_then(|w| w.get(key).copied()).unwrap_or(0)
        };
        let qty = qty.min(cap).min(MAX_TRADE_QTY).max(0);
        let d = if side == "offer" { &mut self.trades.my_offer } else { &mut self.trades.their_request };
        if qty <= 0 {
            d.shift_remove(key);
        } else if !d.contains_key(key) && d.len() >= MAX_TRADE_ITEMS {
            // already 3 different kinds on this side
        } else {
            d.insert(key.to_string(), qty);
        }
    }

    pub fn submit_trade(&mut self) {
        if !(self.trades_ready() && self.trades.target.is_some()) || self.trades.sending {
            return;
        }
        if self.trades.my_offer.is_empty() || self.trades.their_request.is_empty() {
            self.trades.msg = Some((tr("Pick at least one pet on each side."), BAD));
            return;
        }
        let offer: Vec<(String, i64)> = self.trades.my_offer.iter().map(|(k, v)| (k.clone(), *v)).collect();
        let request: Vec<(String, i64)> = self.trades.their_request.iter().map(|(k, v)| (k.clone(), *v)).collect();
        let to_uid = self.trades.target.clone().unwrap();
        let owned: Vec<(String, i64)> = self.state.owned.iter().map(|(k, v)| (k.clone(), *v)).collect();
        self.trades.sending = true;
        let client = self.client.clone().unwrap();
        self.run_job(
            move || {
                // makes sure the online wallet matches the save BEFORE proposing - covers pets you had before the
                // wallet existed (sync_wallet only adds what you roll from then on)
                client.set_wallet_full(&owned)?;
                client.create_trade(&to_uid, &offer, &request)
            },
            |g, _id: String| {
                g.trades.sending = false;
                g.close_trade_propose();
                g.trades.msg = Some((tr("Trade offer sent!"), GOOD));
                g.refresh_trades(true);
            },
            |g, e| {
                g.trades.sending = false;
                g.trades.msg = Some((online_error_text(&e), BAD));
            },
        );
    }

    // ---------------------------------------------------------------- the list (sent / received)
    pub fn trades_pending_count(&self) -> usize {
        self.trades.received.len()
    }

    pub fn refresh_trades(&mut self, force: bool) {
        if !self.trades_ready() || self.trades.loading {
            return;
        }
        let now = now_ts();
        if !force && now < self.trades.next_refresh {
            return;
        }
        self.trades.next_refresh = now + TRADES_POLL;
        self.trades.loading = true;
        self.trades.error = None;
        let client = self.client.clone().unwrap();
        self.run_job(
            move || Ok((client.trades_sent()?, client.trades_received()?)),
            |g, (sent, received): (Vec<Trade>, Vec<Trade>)| {
                g.trades.loading = false;
                g.trades.loaded = true;
                g.trades.sent = sent;
                g.trades.received = received;
            },
            |g, e| {
                g.trades.loading = false;
                g.trades.error = Some(online_error_text(&e));
                g.trades.next_refresh = now_ts() + 60.0;
            },
        );
    }

    /// Cancel (the proposer) or decline (the receiver) - the same call from both sides.
    pub fn cancel_trade_offer(&mut self, trade: &Trade) {
        if !self.trades_ready() || self.trades.action.is_some() {
            return;
        }
        let id = trade.id.clone();
        self.trades.action = Some(id.clone());
        let client = self.client.clone().unwrap();
        let id2 = id.clone();
        self.run_job(
            move || client.cancel_trade(&id2),
            move |g, _: ()| {
                g.trades.action = None;
                g.trades.sent.retain(|t| t.id != id);
                g.trades.received.retain(|t| t.id != id);
            },
            |g, e| {
                g.trades.action = None;
                g.trades.msg = Some((online_error_text(&e), BAD));
            },
        );
    }

    /// Accept: MY side (local owned + online wallet) changes now; what the friend still has to apply waits for
    /// them (see tick_wallet on their side).
    pub fn accept_trade_offer(&mut self, trade: &Trade) {
        if !self.trades_ready() || self.trades.action.is_some() {
            return;
        }
        self.trades.msg = None;
        for (k, q) in &trade.request {
            // what I give
            let have = self.state.owned.get(k).copied().unwrap_or(0);
            if have < *q {
                // (the save open now is what counts: another slot may have more)
                self.trades.msg = Some((
                    tr!("You don't have enough %s in this save: you have %s, they want %s.", pet_key_label(k), format_number(have as f64), format_number(*q as f64)),
                    BAD,
                ));
                return;
            }
        }
        let trade = trade.clone();
        self.trades.action = Some(trade.id.clone());
        let client = self.client.clone().unwrap();
        let sid = self.state.obj_id;
        let t2 = trade.clone();
        self.run_job(
            move || client.accept_trade(&t2.id, &t2.from_uid, &t2.offer, &t2.request),
            move |g, _: ()| {
                g.trades.action = None;
                if let Some(st) = g.state_by_id(sid) {
                    for (k, q) in &trade.request {
                        let v = st.owned.get(k).copied().unwrap_or(0);
                        st.owned.insert(k.clone(), (v - q).max(0));
                    }
                    for (k, q) in &trade.offer {
                        *st.owned.entry(k.clone()).or_insert(0) += q;
                        if *q > 0 {
                            st.seen_pets.insert(k.clone());
                        }
                    }
                    st.dirty = true;
                }
                g.trades.received.retain(|t| t.id != trade.id);
                g.trades.msg = Some((tr("Trade completed!"), GOOD));
            },
            |g, e| {
                g.trades.action = None;
                g.trades.msg = Some((online_error_text(&e), BAD));
            },
        );
    }

    // ================================================================ drawing
    pub fn draw_trade_propose(&mut self, rect: Rect, mouse_pos: (f64, f64)) {
        let med = self.f.med.clone();
        let sb = self.f.small_b.clone();
        let head = med.render(&fit_text(&med, &tr!("Trade with %s", self.trades.target_name.clone()), rect.w - 130), WHITE);
        self.canvas.blit(&head, rect.x, rect.y);
        self.button(Rect::new(rect.right() - 120, rect.y - 4, 120, 34), &tr("Back"), &sb, mouse_pos, panel_light(), panel_lighter(), WHITE, cb(|g| g.close_trade_propose()), Bo::r(9));

        if self.trades.wallet_busy {
            let t = med.render(&tr!("Loading %s's wallet...", self.trades.target_name.clone()), grey());
            let r = Rect::with_center(t.w, t.h, rect.center());
            self.canvas.blit(&t, r.x, r.y);
            return;
        }

        let top = rect.y + head.h + 14;
        let col_w = (rect.w - 20) / 2;
        let left = Rect::new(rect.x, top, col_w, rect.bottom() - top - 60);
        let right = Rect::new(rect.x + col_w + 20, top, col_w, rect.bottom() - top - 60);

        let mine = self.my_owned_for_trade();
        let theirs = self.their_wallet_for_trade();
        let rows = mine.len().max(theirs.len()) as i32;
        self.fr.max_scroll = ((rows * ROW_H) as f64 - (left.h - sb.get_height() - 6) as f64).max(0.0);
        self.fr.scroll = self.fr.scroll.clamp(0.0, self.fr.max_scroll);

        self.draw_trade_column(left, mouse_pos, &tr("You give"), &mine, "offer");
        self.draw_trade_column(right, mouse_pos, &tr("You get"), &theirs, "request");

        let foot_y = rect.bottom() - 44;
        if let Some((text, color)) = self.trades.msg.clone() {
            let tiny = self.f.tiny.clone();
            let t = tiny.render(&fit_text(&tiny, &text, rect.w - 180), color);
            self.canvas.blit(&t, rect.x, foot_y + 12);
        }
        let can_send = !self.trades.my_offer.is_empty() && !self.trades.their_request.is_empty() && !self.trades.sending;
        let label = if self.trades.sending { tr("Sending...") } else { tr("Send trade offer") };
        self.button(Rect::new(rect.right() - 220, foot_y, 220, 40), &label, &sb, mouse_pos, accent(), accent_hover(), BLACK, if can_send { cb(|g| g.submit_trade()) } else { None }, Bo::r(10).enabled(can_send));
    }

    fn draw_trade_column(&mut self, rect: Rect, mouse_pos: (f64, f64), title: &str, entries: &[(String, i64)], side: &'static str) {
        let chosen = if side == "offer" { self.trades.my_offer.clone() } else { self.trades.their_request.clone() };
        let sb = self.f.small_b.clone();
        let small = self.f.small.clone();
        let t = sb.render(&format!("{} ({}/{})", title, chosen.len(), MAX_TRADE_ITEMS), WHITE);
        self.canvas.blit(&t, rect.x, rect.y);
        let list_rect = Rect::new(rect.x, rect.y + t.h + 6, rect.w, rect.h - t.h - 6);
        self.push_clip(list_rect);
        if entries.is_empty() {
            let msg = small.render(&tr("Nothing to trade."), grey());
            let r = Rect::with_center(msg.w, msg.h, list_rect.center());
            self.canvas.blit(&msg, r.x, r.y);
        }
        let mut y = list_rect.y as f64 - self.fr.scroll;
        for (key, have) in entries {
            let row = Rect::new(list_rect.x, ti(y), list_rect.w, ROW_H - 4);
            if row.bottom() >= list_rect.top() && row.top() <= list_rect.bottom() {
                let qty = chosen.get(key).copied().unwrap_or(0);
                draw::rect(&mut self.canvas, if qty > 0 { panel_light() } else { panel() }, row, 0, 8);
                let label = fit_text(&small, &format!("{} ({})", pet_key_label(key), have), row.w - 2 * STEP_W - 56);
                let lt = small.render(&label, if qty > 0 { WHITE } else { grey() });
                self.canvas.blit(&lt, row.x + 8, row.y + row.h / 2 - lt.h / 2);

                let minus = Rect::new(row.right() - 2 * STEP_W - 40, row.y + 2, STEP_W, row.h - 4);
                let qty_rect = Rect::new(row.right() - STEP_W - 36, row.y + 2, 32, row.h - 4);
                let plus = Rect::new(row.right() - 32, row.y + 2, STEP_W, row.h - 4);
                let (k1, k2) = (key.clone(), key.clone());
                self.button(minus, "-", &sb, mouse_pos, panel_lighter(), accent_hover(), WHITE, if qty > 0 { cb(move |g| g.set_trade_offer_qty(side, &k1, qty - 1)) } else { None }, Bo::r(6).enabled(qty > 0));
                let qt = sb.render(&qty.to_string(), WHITE);
                let r = Rect::with_center(qt.w, qt.h, qty_rect.center());
                self.canvas.blit(&qt, r.x, r.y);
                let can_add = qty < *have && (qty > 0 || chosen.len() < MAX_TRADE_ITEMS);
                self.button(plus, "+", &sb, mouse_pos, panel_lighter(), accent_hover(), WHITE, if can_add { cb(move |g| g.set_trade_offer_qty(side, &k2, qty + 1)) } else { None }, Bo::r(6).enabled(can_add));
            }
            y += ROW_H as f64;
        }
        self.pop_clip();
    }

    pub fn draw_trades_list(&mut self, rect: Rect, mouse_pos: (f64, f64)) {
        self.refresh_trades(false);
        let small = self.f.small.clone();
        let mut rect = rect;
        // message of the last Accept / Decline / Cancel (it used to show only on the propose screen, so a failed
        // Accept seemed to do nothing)
        let msg = self.trades.msg.clone().or_else(|| self.trades.error.clone().map(|e| (e, BAD)));
        if let Some((text, color)) = msg {
            let lines: Vec<String> = wrap_text(&text, &small, rect.w - 8).into_iter().take(2).collect();
            for (i, line) in lines.iter().enumerate() {
                let t = small.render(line, color);
                self.canvas.blit(&t, rect.x + 4, rect.y + i as i32 * 20);
            }
            let used = 20 * lines.len() as i32 + 8;
            rect = Rect::new(rect.x, rect.y + used, rect.w, rect.h - used);
        }
        let entries: Vec<(&'static str, Trade)> =
            self.trades.received.iter().map(|t| ("received", t.clone())).chain(self.trades.sent.iter().map(|t| ("sent", t.clone()))).collect();
        if entries.is_empty() {
            let text = if self.trades.loading { tr("Loading...") } else { tr("No trade offers. Open a friend's page to propose one.") };
            let med = self.f.med.clone();
            let t = med.render(&text, grey());
            let r = Rect::with_center(t.w, t.h, rect.center());
            self.canvas.blit(&t, r.x, r.y);
            return;
        }
        let tiny = self.f.tiny.clone();
        self.push_clip(rect);
        let mut y = rect.y as f64 - self.fr.scroll;
        let row_h = 78;
        for (kind, trade) in &entries {
            let row = Rect::new(rect.x, ti(y), rect.w, row_h - 8);
            if row.bottom() >= rect.top() && row.top() <= rect.bottom() {
                draw::rect(&mut self.canvas, panel_light(), row, 0, 10);
                draw::rect(&mut self.canvas, outline(), row, 1, 10);
                let (a_label, b_label) = if *kind == "received" { (tr("They offer"), tr("They want")) } else { (tr("You offer"), tr("You want")) };
                let line1 = format!("{}: {}", a_label, fmt_items(&trade.offer));
                let line2 = format!("{}: {}", b_label, fmt_items(&trade.request));
                let t1 = small.render(&fit_text(&small, &line1, row.w - 200), WHITE);
                let t2 = small.render(&fit_text(&small, &line2, row.w - 200), grey_dim());
                self.canvas.blit(&t1, row.x + 12, row.y + 8);
                self.canvas.blit(&t2, row.x + 12, row.y + 8 + t1.h + 4);
                let busy = self.trades.action.as_deref() == Some(trade.id.as_str());
                let mid = row.y + row.h / 2 - 16;
                let (ta, tb) = (trade.clone(), trade.clone());
                if *kind == "received" {
                    self.button(Rect::new(row.right() - 190, mid, 88, 32), &tr("Accept"), &tiny, mouse_pos, GOOD, GOOD, BLACK, if busy { None } else { cb(move |g| g.accept_trade_offer(&ta)) }, Bo::r(8).enabled(!busy));
                    self.button(Rect::new(row.right() - 96, mid, 84, 32), &tr("Decline"), &tiny, mouse_pos, panel_lighter(), BAD, WHITE, if busy { None } else { cb(move |g| g.cancel_trade_offer(&tb)) }, Bo::r(8).enabled(!busy));
                } else {
                    self.button(Rect::new(row.right() - 110, mid, 98, 32), &tr("Cancel"), &tiny, mouse_pos, panel_lighter(), BAD, WHITE, if busy { None } else { cb(move |g| g.cancel_trade_offer(&tb)) }, Bo::r(8).enabled(!busy));
                }
            }
            y += row_h as f64;
        }
        self.pop_clip();
        let content_h = (entries.len() as i32 * row_h) as f64;
        self.fr.max_scroll = (content_h - rect.h as f64).max(0.0);
        self.fr.scroll = self.fr.scroll.clamp(0.0, self.fr.max_scroll);
        let scroll = self.fr.scroll;
        self.draw_scrollbar(rect, scroll, content_h, Some("friends"), Some(mouse_pos));
    }
}
