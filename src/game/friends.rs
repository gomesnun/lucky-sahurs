//! Friends: search, requests, list and public profiles (online/friends.py) and the
//! Friends page (ui/friends_panel.py).

use super::base::{Bo, FieldRef};
use super::{Cb, Game, KeyEv, cb};
use crate::config::{TOPBAR_H, VIRTUAL_H};
use crate::core::data::{MUT_ORDER, is_mutation, mut_key, mutation, pet_order, rarities};
use crate::core::formatting::{format_number, format_playtime};
use crate::core::state::now_ts;
use crate::gfx::{Color, Rect, draw, ti};
use crate::i18n::{tr, tr_short};
use crate::online::firebase::{Person, PublicStats, username_ok, online_error_text};
use crate::theme::*;
use crate::tr;
use crate::ui::avatar::{avatar_ring_color, avatar_surface};
use crate::ui::drawing::{dim_overlay, draw_panel};
use crate::ui::fonts::{fit_text, wrap_text};
use crate::ui::widgets::TextField;
use sdl2::keyboard::Keycode;
use std::collections::HashMap;
use std::rc::Rc;

pub const FRIEND_REFRESH: f64 = 90.0;
pub const FRIEND_BADGE_POLL: f64 = 240.0;
pub const FRIEND_PROFILE_FETCH: usize = 24;
pub const FRIEND_MAX: usize = 100;

const PANEL_W: i32 = 680;
const ROW_H: i32 = 56;
const ROW_GAP: i32 = 6;
const AVATAR_ROW: i32 = 40;
const AVATAR_BIG: i32 = 96;
const PICKER_CELL: i32 = 84;
const PICKER_GAP: i32 = 10;

const FRIEND_TABS: [(&str, &str); 3] = [("friends", "Friends"), ("requests", "Requests"), ("add", "Add friend")];

pub struct FriendsUi {
    pub open: bool,
    pub tab: &'static str,
    pub scroll: f64,
    pub max_scroll: f64,
    pub list_rect: Rect,
    pub list: Vec<Person>,
    pub incoming: Vec<Person>,
    pub outgoing: Vec<Person>,
    pub loaded: bool,
    pub loading: bool,
    pub error: Option<String>,
    pub next_refresh: f64,
    pub badge_at: f64,
    pub search: TextField,
    pub focus: bool,
    pub search_busy: bool,
    /// None = nothing searched, Some(None) = "none" (nobody with that name)
    pub result: Option<Option<Person>>,
    pub msg: Option<(String, Color)>,
    pub action: Option<String>,
    pub view: Option<String>,
    /// uid -> public stats (None = "none")
    pub stats: HashMap<String, Option<PublicStats>>,
    pub stats_busy: Option<String>,
    pub avatar_picker: bool,
    pub profile_pushed: bool,
}

impl FriendsUi {
    pub fn new() -> FriendsUi {
        FriendsUi {
            open: false,
            tab: "friends",
            scroll: 0.0,
            max_scroll: 0.0,
            list_rect: Rect::ZERO,
            list: Vec::new(),
            incoming: Vec::new(),
            outgoing: Vec::new(),
            loaded: false,
            loading: false,
            error: None,
            next_refresh: 0.0,
            badge_at: 0.0,
            search: TextField::new("username"),
            focus: false,
            search_busy: false,
            result: None,
            msg: None,
            action: None,
            view: None,
            stats: HashMap::new(),
            stats_busy: None,
            avatar_picker: false,
            profile_pushed: false,
        }
    }
}

struct Refreshed {
    friends: Vec<Person>,
    incoming: Vec<Person>,
    outgoing: Vec<Person>,
    chats: Vec<(String, f64)>,
}

type CardButton = (String, Color, Option<Cb>);

enum Row {
    Section(String),
    Card(Person, Vec<CardButton>, Option<Cb>),
}

impl Game {
    // ---------------------------------------------------------------- open / close
    pub fn friends_ready(&self) -> bool {
        self.client.is_some() && self.account.is_some()
    }

    pub fn open_friends(&mut self) {
        self.close_overlays();
        self.fr.open = true;
        self.fr.scroll = 0.0;
        self.fr.msg = None;
        self.fr.view = None;
        self.fr.avatar_picker = false;
        self.push_profile(false);
        self.refresh_friends(false);
    }

    pub fn close_friends(&mut self) {
        self.close_chat();
        self.fr.open = false;
        self.fr.avatar_picker = false;
        self.fr.view = None;
        self.set_friends_focus(false);
    }

    pub fn toggle_friends(&mut self) {
        if self.fr.open {
            self.close_friends();
        } else {
            self.open_friends();
        }
    }

    pub fn set_friends_tab(&mut self, tab: &'static str) {
        self.fr.tab = tab;
        self.fr.scroll = 0.0;
        self.fr.msg = None;
        self.fr.view = None;
        self.set_friends_focus(tab == "add");
    }

    pub fn set_friends_focus(&mut self, focused: bool) {
        if focused == self.fr.focus {
            return;
        }
        self.fr.focus = focused;
        if focused {
            self.start_text_input();
        } else {
            self.stop_text_input();
        }
    }

    pub fn friends_pending_count(&self) -> i64 {
        self.fr.incoming.len() as i64
    }

    pub fn handle_friends_key(&mut self, ev: KeyEv) -> bool {
        if !(self.fr.open && self.fr.focus) {
            return false;
        }
        if matches!(ev.key, Keycode::Return | Keycode::KpEnter) {
            self.submit_friend_search();
            return true;
        }
        self.edit_field_key(FieldRef::FriendsSearch, ev, true)
    }

    // ---------------------------------------------------------------- public profile
    pub fn avatar_pair(&self) -> (Option<i64>, &'static str) {
        match self.state.avatar {
            Some((i, m)) => (Some(i as i64), m),
            None => (None, "normal"),
        }
    }

    pub fn push_profile(&mut self, force: bool) {
        if !self.friends_ready() {
            return;
        }
        if self.fr.profile_pushed && !force {
            return;
        }
        self.fr.profile_pushed = true;
        let (pet, m) = self.avatar_pair();
        let client = self.client.clone().unwrap();
        self.worker.run::<()>(move || client.publish_profile(pet, m), None, Some(Box::new(|_: &mut Game, _| {})));
    }

    fn avatar_options(&self) -> Vec<(usize, &'static str)> {
        let mut out = Vec::new();
        for &idx in pet_order() {
            for m in MUT_ORDER {
                if self.state.owned.get(&format!("{}_{}", idx, m)).copied().unwrap_or(0) > 0 {
                    out.push((idx, m));
                }
            }
        }
        out
    }

    pub fn set_avatar(&mut self, pet_index: usize, m: &'static str) {
        if !(pet_index < rarities().len() && is_mutation(m)) {
            return;
        }
        self.state.avatar = Some((pet_index, mut_key(m)));
        self.state.dirty = true;
        self.fr.avatar_picker = false;
        self.play("click", 0.0);
        self.push_profile(true);
    }

    pub fn clear_avatar(&mut self) {
        self.state.avatar = None;
        self.state.dirty = true;
        self.fr.avatar_picker = false;
        self.push_profile(true);
    }

    // ---------------------------------------------------------------- lists
    pub fn refresh_friends(&mut self, force: bool) {
        if !self.friends_ready() || self.fr.loading {
            return;
        }
        let now = now_ts();
        if !force && now < self.fr.next_refresh {
            return;
        }
        self.fr.next_refresh = now + FRIEND_REFRESH;
        self.fr.loading = true;
        self.fr.error = None;
        let client = self.client.clone().unwrap();
        self.run_job(
            move || {
                let mut friends = client.list_friends()?;
                for entry in friends.iter_mut().take(FRIEND_PROFILE_FETCH) {
                    if let Ok(Some(prof)) = client.get_public_profile(&entry.uid) {
                        entry.avatar_pet = prof.avatar_pet;
                        entry.avatar_mut = prof.avatar_mut.clone();
                        if !prof.username.is_empty() {
                            entry.username = prof.username;
                        }
                    }
                }
                let mut incoming = client.list_friend_requests(true)?;
                let outgoing = client.list_friend_requests(false)?;
                for entry in incoming.iter_mut().take(FRIEND_PROFILE_FETCH) {
                    if let Ok(Some(prof)) = client.get_public_profile(&entry.uid) {
                        entry.avatar_pet = prof.avatar_pet;
                        entry.avatar_mut = prof.avatar_mut;
                    }
                }
                let mut chats = Vec::new();
                for entry in friends.iter().take(FRIEND_PROFILE_FETCH) {
                    if let Ok(Some(Some(last))) = client.get_chat_summary(&entry.uid) {
                        if last != 0.0 {
                            chats.push((entry.uid.clone(), last));
                        }
                    }
                }
                Ok(Refreshed { friends, incoming, outgoing, chats })
            },
            |g, res: Refreshed| {
                g.fr.loading = false;
                g.fr.loaded = true;
                g.fr.list = res.friends;
                g.fr.incoming = res.incoming;
                g.fr.outgoing = res.outgoing;
                for (uid, last) in res.chats {
                    g.chat.last.insert(uid, last);
                }
                g.fr.badge_at = now_ts() + FRIEND_BADGE_POLL;
            },
            |g, e| {
                g.fr.loading = false;
                g.fr.error = Some(online_error_text(&e));
                g.fr.next_refresh = now_ts() + 60.0;
            },
        );
    }

    pub fn tick_friends(&mut self, now: f64) {
        if !self.friends_ready() || self.fr.open || self.fr.loading {
            return;
        }
        if now < self.fr.badge_at {
            return;
        }
        self.fr.badge_at = now + FRIEND_BADGE_POLL;
        let client = self.client.clone().unwrap();
        self.worker.run(
            move || client.list_friend_requests(true),
            Some(Box::new(|g: &mut Game, reqs: Vec<Person>| g.fr.incoming = reqs)),
            Some(Box::new(|_: &mut Game, _| {})),
        );
    }

    // ---------------------------------------------------------------- search / add
    pub fn submit_friend_search(&mut self) {
        if !self.friends_ready() || self.fr.search_busy {
            return;
        }
        let name = self.fr.search.text.trim().to_lowercase();
        if !username_ok(&name) {
            self.fr.msg = Some((tr("Usernames have 3 to 16 letters, numbers or _."), BAD));
            return;
        }
        if self.account.as_ref().is_some_and(|a| a.username == name) {
            self.fr.msg = Some((tr("That's you!"), BAD));
            return;
        }
        self.fr.search_busy = true;
        self.fr.result = None;
        self.fr.msg = None;
        let client = self.client.clone().unwrap();
        self.run_job(
            move || client.find_profile(&name),
            |g, profile: Option<Person>| {
                g.fr.search_busy = false;
                if profile.is_none() {
                    g.fr.msg = Some((tr("No player with that name."), BAD));
                }
                g.fr.result = Some(profile);
            },
            |g, e| {
                g.fr.search_busy = false;
                g.fr.msg = Some((online_error_text(&e), BAD));
            },
        );
    }

    fn friend_relation(&self, uid: &str) -> Option<&'static str> {
        if self.fr.list.iter().any(|f| f.uid == uid) {
            Some("friend")
        } else if self.fr.outgoing.iter().any(|r| r.uid == uid) {
            Some("sent")
        } else if self.fr.incoming.iter().any(|r| r.uid == uid) {
            Some("incoming")
        } else {
            None
        }
    }

    fn friend_err(g: &mut Game, e: crate::online::firebase::OnlineError) {
        g.fr.action = None;
        g.fr.msg = Some((online_error_text(&e), BAD));
    }

    pub fn send_friend_request(&mut self, uid: String, username: String) {
        if !self.friends_ready() || self.fr.action.is_some() {
            return;
        }
        if self.fr.list.len() >= FRIEND_MAX {
            self.fr.msg = Some((tr!("Your friends list is full (%d).", FRIEND_MAX as i64), BAD));
            return;
        }
        self.fr.action = Some(uid.clone());
        self.fr.msg = None;
        let client = self.client.clone().unwrap();
        let (u2, n2) = (uid.clone(), username.clone());
        self.run_job(
            move || client.send_friend_request(&uid, &username),
            move |g, _: ()| {
                g.fr.action = None;
                g.fr.outgoing.push(Person { uid: u2, username: n2.clone(), avatar_pet: None, avatar_mut: "normal".into(), time: None });
                g.fr.msg = Some((tr!("Friend request sent to %s.", n2), GOOD));
            },
            Game::friend_err,
        );
    }

    pub fn accept_friend(&mut self, uid: String, username: String) {
        if !self.friends_ready() || self.fr.action.is_some() {
            return;
        }
        self.fr.action = Some(uid.clone());
        let client = self.client.clone().unwrap();
        let (u2, n2) = (uid.clone(), username.clone());
        self.run_job(
            move || client.accept_friend_request(&uid, &username),
            move |g, _: ()| {
                g.fr.action = None;
                g.fr.incoming.retain(|r| r.uid != u2);
                if !g.fr.list.iter().any(|f| f.uid == u2) {
                    g.fr.list.push(Person { uid: u2, username: n2.clone(), avatar_pet: None, avatar_mut: "normal".into(), time: None });
                    g.fr.list.sort_by(|a, b| a.username.cmp(&b.username));
                }
                g.fr.msg = Some((tr!("%s is now your friend.", n2), GOOD));
                g.refresh_friends(true);
            },
            Game::friend_err,
        );
    }

    fn drop_request(&mut self, uid: String, incoming: bool) {
        if !self.friends_ready() || self.fr.action.is_some() {
            return;
        }
        let me = self.account.as_ref().unwrap().uid.clone();
        let (to_uid, from_uid) = if incoming { (me, uid.clone()) } else { (uid.clone(), me) };
        self.fr.action = Some(uid.clone());
        let client = self.client.clone().unwrap();
        self.run_job(
            move || client.delete_friend_request(&to_uid, &from_uid),
            move |g, _: ()| {
                g.fr.action = None;
                if incoming {
                    g.fr.incoming.retain(|r| r.uid != uid);
                } else {
                    g.fr.outgoing.retain(|r| r.uid != uid);
                }
            },
            Game::friend_err,
        );
    }

    pub fn remove_friend(&mut self, uid: String) {
        if !self.friends_ready() || self.fr.action.is_some() {
            return;
        }
        self.fr.action = Some(uid.clone());
        let client = self.client.clone().unwrap();
        let u2 = uid.clone();
        self.run_job(
            move || client.remove_friend(&uid),
            move |g, _: ()| {
                g.fr.action = None;
                g.fr.list.retain(|f| f.uid != u2);
                if g.fr.view.as_deref() == Some(u2.as_str()) {
                    g.fr.view = None;
                }
            },
            Game::friend_err,
        );
    }

    pub fn open_friend(&mut self, uid: String) {
        self.fr.view = Some(uid.clone());
        self.fr.msg = None;
        self.set_friends_focus(false);
        if self.fr.stats.contains_key(&uid) || self.fr.stats_busy.as_deref() == Some(uid.as_str()) || !self.friends_ready() {
            return;
        }
        self.fr.stats_busy = Some(uid.clone());
        let client = self.client.clone().unwrap();
        let u2 = uid.clone();
        self.run_job(
            move || client.get_public_stats(&uid),
            move |g, stats: Option<PublicStats>| {
                g.fr.stats_busy = None;
                g.fr.stats.insert(u2, stats);
            },
            |g, e| {
                g.fr.stats_busy = None;
                g.fr.msg = Some((online_error_text(&e), BAD));
            },
        );
    }

    pub fn close_friend_view(&mut self) {
        self.close_chat();
        self.fr.view = None;
        self.fr.scroll = 0.0;
    }

    // ================================================================ drawing
    pub fn draw_friends(&mut self, mouse_pos: (f64, f64)) {
        self.refresh_friends(false);
        let ov = dim_overlay(self.vw, VIRTUAL_H, 170);
        self.canvas.blit(&ov, 0, 0);

        let top = TOPBAR_H + 12;
        let panel_w = PANEL_W.min(self.vw - 40);
        let rect = Rect::new(self.vw / 2 - panel_w / 2, top, panel_w, VIRTUAL_H - top - 14);
        draw_panel(&mut self.canvas, rect, Some(panel()), 16, true, None);
        self.register_button(rect, Rc::new(|_: &mut Game| {}), None);

        let sb = self.f.small_b.clone();
        let med = self.f.med.clone();
        let title = self.f.big.render(&tr("Friends"), WHITE);
        self.canvas.blit(&title, rect.x + 26, rect.y + 16);
        let close_rect = Rect::new(rect.right() - 48, rect.y + 18, 30, 30);
        self.button(close_rect, "X", &sb, mouse_pos, panel_light(), BAD, WHITE, cb(|g| g.close_friends()), Bo::r(8));

        let me_rect = Rect::new(rect.x + 22, rect.y + 16 + title.h + 10, panel_w - 44, 64);
        self.draw_friends_me(me_rect, mouse_pos);

        let body_top = me_rect.bottom() + 12;
        let body = Rect::new(rect.x + 22, body_top, panel_w - 44, rect.bottom() - 22 - body_top);
        if !self.friends_ready() {
            self.draw_friends_message(body, &tr("Log in from the main menu to add friends."), grey());
            return;
        }
        if self.fr.avatar_picker {
            self.draw_avatar_picker(body, mouse_pos);
            return;
        }
        if self.chat.uid.is_some() {
            self.draw_chat(body, mouse_pos);
            return;
        }
        if self.fr.view.is_some() {
            self.draw_friend_details(body, mouse_pos);
            return;
        }

        let tab_gap = 10;
        let nt = FRIEND_TABS.len() as i32;
        let tab_w = (panel_w - 44 - tab_gap * (nt - 1)) / nt;
        for (i, (key, label)) in FRIEND_TABS.iter().enumerate() {
            let mut text = tr_short(label);
            let n = match *key {
                "friends" => self.fr.list.len(),
                "requests" => self.fr.incoming.len(),
                _ => 0,
            };
            if n != 0 {
                text = format!("{} ({})", text, n);
            }
            let trect = Rect::new(rect.x + 22 + i as i32 * (tab_w + tab_gap), body_top, tab_w, 40);
            let active = self.fr.tab == *key;
            let font = if med.size(&text).0 <= trect.w - 16 { med.clone() } else { sb.clone() };
            let k: &'static str = key;
            self.button(
                trect,
                &text,
                &font,
                mouse_pos,
                if active { accent() } else { panel_light() },
                if active { accent_hover() } else { panel_lighter() },
                if active { BLACK } else { WHITE },
                cb(move |g| g.set_friends_tab(k)),
                Bo::r(10),
            );
            if *key == "requests" && n != 0 && !active {
                self.draw_friends_badge(trect.topright(), n as i64);
            }
        }

        let content_top = body_top + 40 + 12;
        let foot_h = self.draw_friends_footer(rect, mouse_pos);
        let list_rect = Rect::new(rect.x + 22, content_top, panel_w - 44, rect.bottom() - 22 - foot_h - content_top);
        self.fr.list_rect = list_rect;
        match self.fr.tab {
            "add" => self.draw_friends_add(list_rect, mouse_pos),
            "requests" => self.draw_friends_requests(list_rect, mouse_pos),
            _ => self.draw_friends_friends(list_rect, mouse_pos),
        }
    }

    fn draw_friends_me(&mut self, rect: Rect, mouse_pos: (f64, f64)) {
        draw::rect(&mut self.canvas, panel_light(), rect, 0, 12);
        draw::rect(&mut self.canvas, outline(), rect, 2, 12);
        let (pet, m) = self.avatar_pair();
        let img = avatar_surface(pet, m, 48);
        let r = Rect::with_midleft(img.w, img.h, (rect.x + 12, rect.centery()));
        self.canvas.blit(&img, r.x, r.y);
        let med = self.f.med.clone();
        let name = self.account.as_ref().map(|a| a.username.clone()).unwrap_or_else(|| tr("Not logged in"));
        let nt = med.render(&fit_text(&med, &name, rect.w - 220), WHITE);
        self.canvas.blit(&nt, rect.x + 72, rect.centery() - nt.h - 1);
        let sub = if self.friends_ready() { tr!("%d friends", self.fr.list.len() as i64) } else { tr("Offline") };
        let t = self.f.tiny.render(&sub, grey_dim());
        self.canvas.blit(&t, rect.x + 72, rect.centery() + 3);
        if self.friends_ready() {
            let sb = self.f.small_b.clone();
            self.button(Rect::new(rect.right() - 152, rect.y + 12, 140, rect.h - 24), &tr("Change photo"), &sb, mouse_pos, panel_lighter(), accent_hover(), WHITE, cb(|g| g.open_avatar_picker()), Bo::r(10));
        }
    }

    pub fn open_avatar_picker(&mut self) {
        self.fr.avatar_picker = true;
        self.fr.scroll = 0.0;
        self.fr.msg = None;
        self.set_friends_focus(false);
    }

    pub fn draw_friends_badge(&mut self, topright: (i32, i32), count: i64) {
        let r = 11;
        let center = (topright.0 - 4, topright.1 + 2);
        draw::circle(&mut self.canvas, BAD, center, r, 0);
        draw::circle(&mut self.canvas, outline(), center, r, 2);
        let t = self.f.tiny_b.render(&count.min(99).to_string(), WHITE);
        let tr_ = Rect::with_center(t.w, t.h, center);
        self.canvas.blit(&t, tr_.x, tr_.y);
    }

    fn draw_friends_message(&mut self, rect: Rect, text: &str, color: Color) {
        self.fr.max_scroll = 0.0;
        let med = self.f.med.clone();
        let mut y = rect.centery() - 16;
        for line in wrap_text(text, &med, rect.w - 60) {
            let t = med.render(&line, color);
            let r = Rect::with_center(t.w, t.h, (rect.centerx(), y));
            self.canvas.blit(&t, r.x, r.y);
            y += 26;
        }
    }

    fn draw_friends_footer(&mut self, rect: Rect, mouse_pos: (f64, f64)) -> i32 {
        let h = 40;
        let y = rect.bottom() - 22 - h;
        let sb = self.f.small_b.clone();
        self.button(Rect::new(rect.x + 22, y, 130, h), &tr("Refresh"), &sb, mouse_pos, panel_light(), panel_lighter(), WHITE, cb(|g| g.refresh_friends(true)), Bo::r(10));
        let (text, color) = if self.fr.loading {
            (tr("Updating..."), grey_dim())
        } else if let Some(e) = &self.fr.error {
            (e.clone(), BAD)
        } else if let Some((m, c)) = &self.fr.msg {
            (m.clone(), *c)
        } else {
            (String::new(), grey())
        };
        if !text.is_empty() {
            let small = self.f.small.clone();
            let t = small.render(&fit_text(&small, &text, rect.w - 200), color);
            let r = Rect::with_midright(t.w, t.h, (rect.right() - 26, y + h / 2));
            self.canvas.blit(&t, r.x, r.y);
        }
        h + 12
    }

    fn draw_friend_card(&mut self, rect: Rect, entry: &Person, mouse_pos: (f64, f64), buttons: Vec<CardButton>, on_click: Option<Cb>) -> bool {
        let hovering = on_click.is_some() && rect.collidepoint(mouse_pos) && self.clip_allows(&rect);
        draw::rect(&mut self.canvas, if hovering { panel_lighter() } else { panel_light() }, rect, 0, 12);
        draw::rect(&mut self.canvas, outline(), rect, 2, 12);
        let img = avatar_surface(entry.avatar_pet, &entry.avatar_mut, AVATAR_ROW);
        let r = Rect::with_midleft(img.w, img.h, (rect.x + 10, rect.centery()));
        self.canvas.blit(&img, r.x, r.y);
        if self.chat_unread(&entry.uid) {
            let c = (rect.x + 10 + AVATAR_ROW - 2, rect.centery() - AVATAR_ROW / 2 + 4);
            draw::circle(&mut self.canvas, BAD, c, 6, 0);
            draw::circle(&mut self.canvas, panel(), c, 6, 2);
        }
        let sb = self.f.small_b.clone();
        let med = self.f.med.clone();
        let mut bx = rect.right() - 10;
        let busy = self.fr.action.as_deref() == Some(entry.uid.as_str());
        for (label, color, callback) in buttons.into_iter().rev() {
            let bw = 84.max(sb.size(&label).0 + 26);
            let brect = Rect::new(bx - bw, rect.centery() - 16, bw, 32);
            let hover = Color::rgb(color.r.saturating_add(30), color.g.saturating_add(30), color.b.saturating_add(30));
            let text_color = if color == accent() || color == GOOD { BLACK } else { WHITE };
            self.button(brect, &label, &sb, mouse_pos, color, hover, text_color, callback, Bo::r(8).enabled(!busy));
            bx = brect.x - 8;
        }
        let name_w = 40.max(bx - (rect.x + 60));
        let nt = med.render(&fit_text(&med, &entry.username, name_w), WHITE);
        self.canvas.blit(&nt, rect.x + 60, rect.centery() - nt.h / 2);
        if let Some(c) = on_click {
            self.register_button(rect, c, Some("click"));
        }
        hovering
    }

    fn draw_friends_rows(&mut self, rect: Rect, rows: Vec<(i32, Row)>, mouse_pos: (f64, f64)) {
        let scroll = self.fr.scroll;
        self.push_clip(rect);
        let mut y = rect.top() as f64 - scroll;
        let content_h = rows.iter().map(|r| r.0).sum::<i32>() + ROW_GAP * (rows.len() as i32 - 1).max(0);
        for (height, row) in rows {
            let rrect = Rect::new(rect.x, ti(y), rect.w - 16, height);
            if rrect.bottom() >= rect.top() - 4 && rrect.top() <= rect.bottom() + 4 {
                match row {
                    Row::Section(text) => self.draw_friends_section(rrect, &text),
                    Row::Card(p, b, c) => {
                        self.draw_friend_card(rrect, &p, mouse_pos, b, c);
                    }
                }
            }
            y += (height + ROW_GAP) as f64;
        }
        self.pop_clip();
        self.fr.max_scroll = (content_h as f64 - rect.h as f64).max(0.0);
        self.fr.scroll = self.fr.scroll.min(self.fr.max_scroll).max(0.0);
        self.draw_scrollbar(rect, scroll, content_h as f64, Some("friends"), Some(mouse_pos));
    }

    fn draw_friends_section(&mut self, rect: Rect, text: &str) {
        let t = self.f.small_b.render(text, grey_dim());
        self.canvas.blit(&t, rect.x + 4, rect.bottom() - t.h - 4);
        draw::line(&mut self.canvas, panel_light(), (rect.x + 4, rect.bottom()), (rect.right() - 4, rect.bottom()), 2);
    }

    fn draw_friends_friends(&mut self, rect: Rect, mouse_pos: (f64, f64)) {
        if self.fr.list.is_empty() {
            let msg = if self.fr.loading && !self.fr.loaded { tr("Loading friends...") } else { tr("No friends yet - use \"Add friend\" to find someone.") };
            self.draw_friends_message(rect, &msg, grey());
            return;
        }
        let mut rows = Vec::new();
        for e in self.fr.list.clone() {
            let (u1, n1, u2, u3) = (e.uid.clone(), e.username.clone(), e.uid.clone(), e.uid.clone());
            let buttons = vec![
                (tr("Chat"), accent(), cb(move |g| g.open_chat(&u1, &n1))),
                (tr("Stats"), panel_lighter(), cb(move |g| g.open_friend(u2.clone()))),
            ];
            rows.push((ROW_H, Row::Card(e, buttons, cb(move |g| g.open_friend(u3.clone())))));
        }
        self.draw_friends_rows(rect, rows, mouse_pos);
    }

    fn draw_friends_requests(&mut self, rect: Rect, mouse_pos: (f64, f64)) {
        if self.fr.incoming.is_empty() && self.fr.outgoing.is_empty() {
            let msg = if self.fr.loading && !self.fr.loaded { tr("Loading requests...") } else { tr("No friend requests right now.") };
            self.draw_friends_message(rect, &msg, grey());
            return;
        }
        let mut rows = Vec::new();
        if !self.fr.incoming.is_empty() {
            rows.push((22, Row::Section(tr("Received"))));
            for req in self.fr.incoming.clone() {
                let (u1, n1, u2) = (req.uid.clone(), req.username.clone(), req.uid.clone());
                let buttons = vec![
                    (tr("Accept"), GOOD, cb(move |g| g.accept_friend(u1.clone(), n1.clone()))),
                    (tr("Decline"), BAD, cb(move |g| g.drop_request(u2.clone(), true))),
                ];
                rows.push((ROW_H, Row::Card(req, buttons, None)));
            }
        }
        if !self.fr.outgoing.is_empty() {
            rows.push((28, Row::Section(tr("Sent"))));
            for req in self.fr.outgoing.clone() {
                let u = req.uid.clone();
                let buttons = vec![(tr("Cancel"), panel_lighter(), cb(move |g| g.drop_request(u.clone(), false)))];
                rows.push((ROW_H, Row::Card(req, buttons, None)));
            }
        }
        self.draw_friends_rows(rect, rows, mouse_pos);
    }

    fn draw_friends_add(&mut self, rect: Rect, mouse_pos: (f64, f64)) {
        self.fr.max_scroll = 0.0;
        let field = Rect::new(rect.x, rect.y, rect.w - 130, 46);
        let text = self.fr.search.text.clone();
        let focus = self.fr.focus;
        self.draw_text_field(field, FieldRef::FriendsSearch, &text, &tr("username"), focus, Rc::new(|g: &mut Game| g.set_friends_focus(true)));
        let med = self.f.med.clone();
        let busy = self.fr.search_busy;
        self.button(Rect::new(field.right() + 10, field.y, 120, 46), &tr("Search"), &med, mouse_pos, accent(), accent_hover(), BLACK, cb(|g| g.submit_friend_search()), Bo::r(10).enabled(!busy));
        let hint = self.f.tiny.render(&tr("Type the exact username of the player you want to add."), grey_dim());
        self.canvas.blit(&hint, rect.x + 4, field.bottom() + 8);

        let card = Rect::new(rect.x, field.bottom() + 34, rect.w, 76);
        if busy {
            let t = med.render(&tr("Searching..."), grey());
            let r = Rect::with_center(t.w, t.h, card.center());
            self.canvas.blit(&t, r.x, r.y);
            return;
        }
        let Some(Some(profile)) = self.fr.result.clone() else { return };
        let (u, n) = (profile.uid.clone(), profile.username.clone());
        let buttons: Vec<CardButton> = match self.friend_relation(&profile.uid) {
            Some("friend") => vec![(tr("Friends"), panel_light(), None)],
            Some("sent") => vec![(tr("Request sent"), panel_light(), None)],
            Some(_) => vec![(tr("Accept"), GOOD, cb(move |g| g.accept_friend(u.clone(), n.clone())))],
            None => vec![(tr("Add"), accent(), cb(move |g| g.send_friend_request(u.clone(), n.clone())))],
        };
        self.draw_friend_card(card, &profile, mouse_pos, buttons, None);
    }

    fn draw_friend_details(&mut self, rect: Rect, mouse_pos: (f64, f64)) {
        self.fr.max_scroll = 0.0;
        let Some(entry) = self.fr.list.iter().find(|f| Some(&f.uid) == self.fr.view.as_ref()).cloned() else {
            self.close_friend_view();
            return;
        };
        let img = avatar_surface(entry.avatar_pet, &entry.avatar_mut, AVATAR_BIG);
        let r = Rect::with_midtop(img.w, img.h, (rect.centerx(), rect.y + 6));
        self.canvas.blit(&img, r.x, r.y);
        let big = self.f.big.clone();
        let name = big.render(&fit_text(&big, &entry.username, rect.w - 40), WHITE);
        let r = Rect::with_midtop(name.w, name.h, (rect.centerx(), rect.y + AVATAR_BIG + 14));
        self.canvas.blit(&name, r.x, r.y);

        if let Some(pet) = entry.avatar_pet.filter(|p| *p >= 0 && (*p as usize) < rarities().len()) {
            let mlabel = mutation(&entry.avatar_mut).map(|m| m.label).unwrap_or("");
            let label = format!("{} {}", tr(mlabel), rarities()[pet as usize].pet).trim().to_string();
            let t = self.f.small.render(&label, avatar_ring_color(&entry.avatar_mut));
            let r = Rect::with_midtop(t.w, t.h, (rect.centerx(), rect.y + AVATAR_BIG + 14 + name.h + 4));
            self.canvas.blit(&t, r.x, r.y);
        }

        let med = self.f.med.clone();
        let mut y = rect.y + AVATAR_BIG + 82;
        match self.fr.stats.get(&entry.uid).cloned() {
            None => {
                let t = med.render(&tr("Loading stats..."), grey());
                let r = Rect::with_center(t.w, t.h, (rect.centerx(), y + 40));
                self.canvas.blit(&t, r.x, r.y);
            }
            Some(None) => {
                let t = med.render(&tr("This player has no public stats yet."), grey());
                let r = Rect::with_center(t.w, t.h, (rect.centerx(), y + 40));
                self.canvas.blit(&t, r.x, r.y);
            }
            Some(Some(stats)) => {
                let lines = [
                    (tr("Total Coins Earned"), format!("${}", format_number(stats.coins))),
                    (tr("Playtime"), format_playtime(stats.playtime)),
                    (tr("Total Rolls"), format_number(stats.rolls as f64)),
                    (tr("Rebirths"), format_number(stats.rebirths as f64)),
                ];
                let row_h = 36;
                for (label, value) in &lines {
                    let lt = med.render(label, grey());
                    let vt = med.render(value, WHITE);
                    self.canvas.blit(&lt, rect.x + 20, y);
                    self.canvas.blit(&vt, rect.right() - 20 - vt.w, y);
                    draw::line(&mut self.canvas, panel_light(), (rect.x + 20, y + row_h - 8), (rect.right() - 20, y + row_h - 8), 1);
                    y += row_h;
                }
                if stats.updated_at.is_some_and(|t| t != 0.0) {
                    let t = self.f.tiny.render(&tr!("Stats update every %d minutes.", 10), grey_dim());
                    self.canvas.blit(&t, rect.x + 20, y + 4);
                }
            }
        }

        let sb = self.f.small_b.clone();
        self.button(Rect::new(rect.x, rect.bottom() - 40, 140, 40), &tr("Back"), &sb, mouse_pos, panel_light(), panel_lighter(), WHITE, cb(|g| g.close_friend_view()), Bo::r(10));
        let (u1, n1, u2) = (entry.uid.clone(), entry.username.clone(), entry.uid.clone());
        self.button(Rect::new(rect.centerx() - 75, rect.bottom() - 40, 150, 40), &tr("Message"), &sb, mouse_pos, accent(), accent_hover(), BLACK, cb(move |g| g.open_chat(&u1, &n1)), Bo::r(10));
        let busy = self.fr.action.as_deref() == Some(entry.uid.as_str());
        self.button(Rect::new(rect.right() - 170, rect.bottom() - 40, 170, 40), &tr("Remove friend"), &sb, mouse_pos, panel_light(), BAD, WHITE, cb(move |g| g.remove_friend(u2.clone())), Bo::r(10).enabled(!busy));
    }

    fn draw_avatar_picker(&mut self, rect: Rect, mouse_pos: (f64, f64)) {
        let head = self.f.med.render(&tr("Choose a photo"), WHITE);
        self.canvas.blit(&head, rect.x + 4, rect.y);
        let sub = self.f.tiny.render(&tr("Any verity you own can be your photo - Golden and Diamond keep their coloured ring."), grey_dim());
        self.canvas.blit(&sub, rect.x + 4, rect.y + head.h + 2);

        let sb = self.f.small_b.clone();
        let foot_y = rect.bottom() - 40;
        self.button(Rect::new(rect.x, foot_y, 140, 40), &tr("Back"), &sb, mouse_pos, panel_light(), panel_lighter(), WHITE, cb(|g| g.fr.avatar_picker = false), Bo::r(10));
        if self.avatar_pair().0.is_some() {
            self.button(Rect::new(rect.right() - 160, foot_y, 160, 40), &tr("No photo"), &sb, mouse_pos, panel_light(), BAD, WHITE, cb(|g| g.clear_avatar()), Bo::r(10));
        }

        let grid = Rect::new(rect.x, rect.y + head.h + 24, rect.w, foot_y - 12 - (rect.y + head.h + 24));
        let options = self.avatar_options();
        if options.is_empty() {
            self.draw_friends_message(grid, &tr("Roll a verity first - then it can be your photo."), grey());
            return;
        }
        let per_row = 1.max((grid.w - 16).div_euclid(PICKER_CELL + PICKER_GAP));
        let current = self.state.avatar;
        let scroll = self.fr.scroll;
        let tiny = self.f.tiny.clone();
        self.push_clip(grid);
        for (i, &(idx, m)) in options.iter().enumerate() {
            let (col, row) = (i as i32 % per_row, i as i32 / per_row);
            let cell = Rect::new(grid.x + col * (PICKER_CELL + PICKER_GAP), ti((grid.y + row * (PICKER_CELL + PICKER_GAP)) as f64 - scroll), PICKER_CELL, PICKER_CELL);
            if cell.bottom() < grid.top() - 4 || cell.top() > grid.bottom() + 4 {
                continue;
            }
            let chosen = current == Some((idx, m));
            let hovering = cell.collidepoint(mouse_pos) && self.clip_allows(&cell);
            let back = if chosen { accent() } else if hovering { panel_lighter() } else { panel_light() };
            draw::rect(&mut self.canvas, back, cell, 0, 12);
            draw::rect(&mut self.canvas, outline(), cell, 2, 12);
            let img = avatar_surface(Some(idx as i64), m, PICKER_CELL - 32);
            let r = Rect::with_midtop(img.w, img.h, (cell.centerx(), cell.y + 6));
            self.canvas.blit(&img, r.x, r.y);
            let label = fit_text(&tiny, rarities()[idx].pet, cell.w - 8);
            let lt = tiny.render(&label, if chosen { BLACK } else { WHITE });
            let r = Rect::with_midbottom(lt.w, lt.h, (cell.centerx(), cell.bottom() - 5));
            self.canvas.blit(&lt, r.x, r.y);
            self.register_button(cell, Rc::new(move |g: &mut Game| g.set_avatar(idx, m)), Some("click"));
        }
        self.pop_clip();
        let rows = (options.len() as i32 + per_row - 1) / per_row;
        let content_h = (rows * (PICKER_CELL + PICKER_GAP) - PICKER_GAP) as f64;
        self.fr.max_scroll = (content_h - grid.h as f64).max(0.0);
        self.fr.scroll = self.fr.scroll.min(self.fr.max_scroll).max(0.0);
        self.draw_scrollbar(grid, scroll, content_h, Some("friends"), Some(mouse_pos));
    }
}
