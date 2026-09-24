//! Chat between friends: state and cloud calls (online/chat.py) and the conversation screen
//! drawn inside the Friends page (ui/chat_panel.py).

use super::base::{Bo, FieldRef};
use super::{Game, KeyEv, cb};
use crate::core::state::now_ts;
use crate::gfx::{Color, Rect, draw};
use crate::i18n::tr;
use crate::online::firebase::{Message, online_error_text};
use crate::theme::*;
use crate::tr;
use crate::ui::avatar::avatar_surface;
use crate::ui::fonts::{fit_text, wrap_text};
use crate::ui::widgets::TextField;
use sdl2::keyboard::Keycode;
use std::collections::HashMap;
use std::rc::Rc;

pub const CHAT_POLL: f64 = 10.0;
pub const CHAT_LIMIT: usize = 50;
const CHAT_MIN_LEN: usize = 1;

const BUBBLE_GAP: i32 = 6;
const BUBBLE_PAD: i32 = 10;
const INPUT_H: i32 = 44;

pub struct ChatUi {
    pub uid: Option<String>,
    pub name: String,
    pub messages: Vec<Message>,
    pub loading: bool,
    pub error: Option<String>,
    pub next_poll: f64,
    pub scroll: f64,
    pub max_scroll: f64,
    pub list_rect: Rect,
    pub field: TextField,
    pub focus: bool,
    pub sending: bool,
    pub seen: HashMap<String, f64>,
    pub last: HashMap<String, f64>,
}

impl ChatUi {
    pub fn new() -> ChatUi {
        ChatUi {
            uid: None,
            name: String::new(),
            messages: Vec::new(),
            loading: false,
            error: None,
            next_poll: 0.0,
            scroll: 0.0,
            max_scroll: 0.0,
            list_rect: Rect::ZERO,
            field: TextField::new("text"),
            focus: false,
            sending: false,
            // saved on disk, otherwise the red "unread" dot came back every time the game reopened
            seen: crate::storage::load_chat_seen(),
            last: HashMap::new(),
        }
    }
}

pub fn chat_when(stamp: Option<f64>) -> String {
    let Some(stamp) = stamp.filter(|s| *s != 0.0) else { return String::new() };
    let secs = (now_ts() - stamp).max(0.0);
    if secs < 90.0 {
        tr("just now")
    } else if secs < 3600.0 {
        tr!("%d min ago", (secs / 60.0).floor() as i64)
    } else if secs < 86400.0 {
        tr!("%d h ago", (secs / 3600.0).floor() as i64)
    } else {
        tr!("%d d ago", (secs / 86400.0).floor() as i64)
    }
}

impl Game {
    pub fn open_chat(&mut self, uid: &str, username: &str) {
        if !self.friends_ready() {
            return;
        }
        self.chat.uid = Some(uid.to_string());
        self.chat.name = if username.is_empty() { uid.to_string() } else { username.to_string() };
        self.chat.messages.clear();
        self.chat.error = None;
        self.chat.scroll = 0.0;
        self.chat.next_poll = 0.0;
        self.chat.field.set_text("");
        self.set_chat_focus(true);
        self.poll_chat(true);
    }

    pub fn close_chat(&mut self) {
        self.chat.uid = None;
        self.set_chat_focus(false);
        self.chat.field.set_text("");
    }

    pub fn set_chat_focus(&mut self, focused: bool) {
        if focused == self.chat.focus {
            return;
        }
        self.chat.focus = focused;
        if focused {
            self.start_text_input();
        } else {
            self.stop_text_input();
        }
    }

    /// How many conversations have unread messages (for the dot on the top bar's Friends button).
    pub fn chat_unread_count(&self) -> usize {
        self.chat.last.keys().filter(|uid| self.chat_unread(uid)).count()
    }

    pub fn chat_unread(&self, uid: &str) -> bool {
        match self.chat.last.get(uid) {
            Some(&last) if last != 0.0 => last > self.chat.seen.get(uid).copied().unwrap_or(0.0) + 0.5,
            _ => false,
        }
    }

    pub fn handle_chat_key(&mut self, ev: KeyEv) -> bool {
        if !(self.chat.uid.is_some() && self.chat.focus) {
            return false;
        }
        if matches!(ev.key, Keycode::Return | Keycode::KpEnter) {
            self.send_chat_message();
            return true;
        }
        self.edit_field_key(FieldRef::Chat, ev, false)
    }

    pub fn poll_chat(&mut self, force: bool) {
        if !(self.chat.uid.is_some() && self.friends_ready()) || self.chat.loading {
            return;
        }
        let now = now_ts();
        if !force && now < self.chat.next_poll {
            return;
        }
        self.chat.loading = true;
        self.chat.next_poll = now + CHAT_POLL;
        let client = self.client.clone().unwrap();
        let uid = self.chat.uid.clone().unwrap();
        let (uid_ok, uid_err) = (uid.clone(), uid.clone());
        self.run_job(
            move || client.list_messages(&uid, CHAT_LIMIT),
            move |g, messages| {
                g.chat.loading = false;
                if g.chat.uid.as_deref() != Some(uid_ok.as_str()) {
                    return;
                }
                g.chat.error = None;
                let at_end = g.chat.scroll <= 1.0;
                if !messages.is_empty() {
                    let seen = messages.iter().map(|m| m.sent_at.unwrap_or(0.0)).fold(f64::NEG_INFINITY, f64::max);
                    g.chat.seen.insert(uid_ok.clone(), seen);
                    g.chat.last.insert(uid_ok.clone(), seen);
                    crate::storage::save_chat_seen(&g.chat.seen);
                }
                g.chat.messages = messages;
                if at_end {
                    g.chat.scroll = 0.0;
                }
            },
            move |g, e| {
                g.chat.loading = false;
                if g.chat.uid.as_deref() == Some(uid_err.as_str()) {
                    g.chat.error = Some(online_error_text(&e));
                }
            },
        );
    }

    pub fn tick_chat(&mut self, _now: f64) {
        if self.chat.uid.is_some() && self.fr.open {
            self.poll_chat(false);
        }
    }

    pub fn send_chat_message(&mut self) {
        if !(self.chat.uid.is_some() && self.friends_ready()) || self.chat.sending {
            return;
        }
        let text = self.chat.field.text.split_whitespace().collect::<Vec<_>>().join(" ");
        if text.chars().count() < CHAT_MIN_LEN {
            return;
        }
        self.chat.sending = true;
        let client = self.client.clone().unwrap();
        let uid = self.chat.uid.clone().unwrap();
        let me = self.account.as_ref().map(|a| a.uid.clone()).unwrap_or_default();
        let (uid_ok, uid_err, text_ok) = (uid.clone(), uid.clone(), text.clone());
        self.run_job(
            move || client.send_message(&uid, &text),
            move |g, _: ()| {
                g.chat.sending = false;
                if g.chat.uid.as_deref() != Some(uid_ok.as_str()) {
                    return;
                }
                g.chat.field.set_text("");
                g.chat.messages.push(Message { id: "local".into(), from_uid: me, text: text_ok, sent_at: Some(now_ts()) });
                let n = g.chat.messages.len();
                if n > CHAT_LIMIT {
                    g.chat.messages.drain(..n - CHAT_LIMIT);
                }
                g.chat.scroll = 0.0;
                g.chat.next_poll = now_ts() + 1.0;
            },
            move |g, e| {
                g.chat.sending = false;
                if g.chat.uid.as_deref() == Some(uid_err.as_str()) {
                    g.chat.error = Some(online_error_text(&e));
                }
            },
        );
    }

    // ---------------------------------------------------------------- drawing
    pub fn draw_chat(&mut self, rect: Rect, mouse_pos: (f64, f64)) {
        let entry = self.fr.list.iter().find(|f| Some(&f.uid) == self.chat.uid.as_ref()).cloned();
        let head_h = 44;
        let img = match &entry {
            Some(e) => avatar_surface(e.avatar_pet, &e.avatar_mut, 34),
            None => avatar_surface(None, "normal", 34),
        };
        let r = Rect::with_midleft(img.w, img.h, (rect.x + 2, rect.y + head_h / 2));
        self.canvas.blit(&img, r.x, r.y);
        let med = self.f.med.clone();
        let sb = self.f.small_b.clone();
        let name = med.render(&fit_text(&med, &self.chat.name, rect.w - 220), WHITE);
        self.canvas.blit(&name, rect.x + 44, rect.y + head_h / 2 - name.h / 2);
        self.button(Rect::new(rect.right() - 120, rect.y + 4, 120, 34), &tr("Back"), &sb, mouse_pos, panel_light(), panel_lighter(), WHITE, cb(|g| g.close_chat()), Bo::r(9));

        let input_rect = Rect::new(rect.x, rect.bottom() - INPUT_H, rect.w - 110, INPUT_H);
        let text = self.chat.field.text.clone();
        let focus = self.chat.focus;
        self.draw_text_field(input_rect, FieldRef::Chat, &text, &tr("Write a message..."), focus, Rc::new(|g: &mut Game| g.set_chat_focus(true)));
        let sending = self.chat.sending;
        self.button(
            Rect::new(input_rect.right() + 10, input_rect.y, 100, INPUT_H),
            &if sending { tr("Sending...") } else { tr("Send") },
            &sb,
            mouse_pos,
            accent(),
            accent_hover(),
            BLACK,
            if sending { None } else { cb(|g| g.send_chat_message()) },
            Bo::r(10),
        );

        let list_rect = Rect::new(rect.x, rect.y + head_h + 6, rect.w, input_rect.top() - 10 - (rect.y + head_h + 6));
        self.chat.list_rect = list_rect;
        self.draw_chat_messages(list_rect, mouse_pos);

        if let Some(e) = self.chat.error.clone() {
            let tiny = self.f.tiny.clone();
            let t = tiny.render(&fit_text(&tiny, &e, rect.w), BAD);
            self.canvas.blit(&t, rect.x, input_rect.top() - 16);
        }
    }

    fn draw_chat_messages(&mut self, rect: Rect, mouse_pos: (f64, f64)) {
        let med = self.f.med.clone();
        let sb = self.f.small_b.clone();
        let tiny = self.f.tiny.clone();
        if self.chat.messages.is_empty() {
            let text = if self.chat.loading { tr("Loading...") } else { tr("No messages yet - say hi!") };
            let t = med.render(&text, grey());
            let r = Rect::with_center(t.w, t.h, rect.center());
            self.canvas.blit(&t, r.x, r.y);
            self.chat.max_scroll = 0.0;
            return;
        }
        let me = self.account.as_ref().map(|a| a.uid.clone());
        let max_w = (rect.w as f64 * 0.72) as i32;
        let line_h = sb.get_height() + 2;

        let mut laid: Vec<(Message, Vec<String>, i32)> = Vec::new();
        let mut total = 0;
        for msg in &self.chat.messages {
            let lines = wrap_text(&msg.text, &sb, max_w - 2 * BUBBLE_PAD);
            let h = 2 * BUBBLE_PAD + lines.len() as i32 * line_h + tiny.get_height();
            total += h + BUBBLE_GAP;
            laid.push((msg.clone(), lines, h));
        }
        total = (total - BUBBLE_GAP).max(0);
        self.chat.max_scroll = (total as f64 - rect.h as f64).max(0.0);
        self.chat.scroll = self.chat.scroll.min(self.chat.max_scroll).max(0.0);

        let mut y = (rect.bottom() - 2 - total) as f64 + self.chat.scroll;
        self.push_clip(rect);
        for (msg, lines, h) in &laid {
            if y + *h as f64 >= (rect.top() - 4) as f64 && y <= (rect.bottom() + 4) as f64 {
                let mine = me.as_deref() == Some(msg.from_uid.as_str());
                let widest = lines.iter().map(|l| sb.size(l).0).max().unwrap_or(0);
                let when = chat_when(msg.sent_at);
                let when_w = if when.is_empty() { 0 } else { tiny.size(&when).0 };
                let w = 60.max(widest.max(when_w) + 2 * BUBBLE_PAD);
                let x = if mine { rect.right() - 16 - w } else { rect.x };
                let bx = Rect::new(x, y as i32, w, *h);
                draw::rect(&mut self.canvas, if mine { accent() } else { panel_light() }, bx, 0, 12);
                if !mine {
                    draw::rect(&mut self.canvas, outline(), bx, 2, 12);
                }
                let mut ty = bx.y + BUBBLE_PAD;
                for line in lines {
                    let t = sb.render(line, if mine { BLACK } else { WHITE });
                    self.canvas.blit(&t, bx.x + BUBBLE_PAD, ty);
                    ty += line_h;
                }
                if !when.is_empty() {
                    let t = tiny.render(&when, if mine { Color::rgb(40, 40, 40) } else { grey_dim() });
                    self.canvas.blit(&t, bx.right() - t.w - BUBBLE_PAD, bx.bottom() - BUBBLE_PAD - t.h + 2);
                }
            }
            y += (h + BUBBLE_GAP) as f64;
        }
        self.pop_clip();
        let sc = self.chat.max_scroll - self.chat.scroll;
        self.draw_scrollbar(rect, sc, total as f64, Some("chat"), Some(mouse_pos));
    }
}
