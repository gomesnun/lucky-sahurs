//! Feedback: one message per account that can be rewritten or deleted (online/feedback.py) and
//! its page (ui/feedback_panel.py).

use super::base::{Bo, FieldRef};
use super::chat_panel::chat_when as feedback_when;
use super::{Game, KeyEv, cb};
use crate::config::VIRTUAL_H;
use crate::core::state::now_ts;
use crate::gfx::{Color, Rect, draw, ti};
use crate::i18n::tr;
use crate::online::firebase::{Feedback, online_error_text};
use crate::theme::*;
use crate::ui::drawing::{dim_overlay, draw_panel};
use crate::ui::fonts::{fit_text, wrap_text};
use crate::ui::widgets::TextField;
use sdl2::keyboard::Keycode;
use std::rc::Rc;

pub const FEEDBACK_REFRESH: f64 = 120.0;
pub const FEEDBACK_MAX: usize = 60;
pub const FEEDBACK_MIN_LEN: usize = 4;

const PANEL_W: i32 = 680;
const ROW_GAP: i32 = 8;
const EDITOR_H: i32 = 104;
const FEEDBACK_MAX_LEN: i64 = 280;

pub struct FeedbackUi {
    pub open: bool,
    pub scroll: f64,
    pub max_scroll: f64,
    pub list_rect: Rect,
    pub list: Vec<Feedback>,
    pub mine: Option<Feedback>,
    pub loaded: bool,
    pub loading: bool,
    pub error: Option<String>,
    pub next_refresh: f64,
    pub field: TextField,
    pub editing: bool,
    pub focus: bool,
    pub busy: bool,
    pub msg: Option<(String, Color)>,
    pub confirm_delete: bool,
}

impl FeedbackUi {
    pub fn new() -> FeedbackUi {
        FeedbackUi {
            open: false,
            scroll: 0.0,
            max_scroll: 0.0,
            list_rect: Rect::ZERO,
            list: Vec::new(),
            mine: None,
            loaded: false,
            loading: false,
            error: None,
            next_refresh: 0.0,
            field: TextField::new("text"),
            editing: false,
            focus: false,
            busy: false,
            msg: None,
            confirm_delete: false,
        }
    }
}

impl Game {
    fn feedback_ready(&self) -> bool {
        self.client.is_some() && self.account.is_some()
    }

    pub fn open_feedback(&mut self) {
        self.close_overlays();
        self.fb.open = true;
        self.fb.scroll = 0.0;
        self.fb.msg = None;
        self.fb.confirm_delete = false;
        self.refresh_feedback(false);
    }

    pub fn close_feedback(&mut self) {
        self.fb.open = false;
        self.fb.confirm_delete = false;
        self.set_feedback_focus(false);
        self.fb.editing = false;
    }

    pub fn toggle_feedback(&mut self) {
        if self.fb.open {
            self.close_feedback();
        } else {
            self.open_feedback();
        }
    }

    pub fn set_feedback_focus(&mut self, focused: bool) {
        if focused == self.fb.focus {
            return;
        }
        self.fb.focus = focused;
        if focused {
            self.start_text_input();
        } else {
            self.stop_text_input();
        }
    }

    pub fn handle_feedback_key(&mut self, ev: KeyEv) -> bool {
        if !(self.fb.open && self.fb.focus) {
            return false;
        }
        if matches!(ev.key, Keycode::Return | Keycode::KpEnter) {
            self.submit_feedback();
            return true;
        }
        self.edit_field_key(FieldRef::Feedback, ev, false)
    }

    pub fn refresh_feedback(&mut self, force: bool) {
        if !self.feedback_ready() || self.fb.loading {
            return;
        }
        let now = now_ts();
        if !force && now < self.fb.next_refresh {
            return;
        }
        self.fb.loading = true;
        self.fb.next_refresh = now + FEEDBACK_REFRESH;
        let client = self.client.clone().unwrap();
        let uid = self.account.as_ref().unwrap().uid.clone();
        self.run_job(
            move || {
                let entries = client.list_feedback(FEEDBACK_MAX)?;
                let mine = match entries.iter().find(|e| e.uid == uid) {
                    Some(m) => Some(m.clone()),
                    None => client.get_my_feedback()?,
                };
                Ok((entries, mine))
            },
            |g, (entries, mine): (Vec<Feedback>, Option<Feedback>)| {
                g.fb.loading = false;
                g.fb.loaded = true;
                g.fb.error = None;
                g.fb.list = entries;
                if !g.fb.editing {
                    let t = mine.as_ref().map(|m| m.text.clone()).unwrap_or_default();
                    g.fb.field.set_text(&t);
                }
                g.fb.mine = mine;
            },
            |g, e| {
                g.fb.loading = false;
                g.fb.error = Some(online_error_text(&e));
            },
        );
    }

    pub fn start_feedback_edit(&mut self) {
        if !self.feedback_ready() || self.fb.busy {
            return;
        }
        self.fb.editing = true;
        self.fb.confirm_delete = false;
        self.fb.msg = None;
        let t = self.fb.mine.as_ref().map(|m| m.text.clone()).unwrap_or_default();
        self.fb.field.set_text(&t);
        self.set_feedback_focus(true);
    }

    pub fn cancel_feedback_edit(&mut self) {
        self.fb.editing = false;
        self.set_feedback_focus(false);
        let t = self.fb.mine.as_ref().map(|m| m.text.clone()).unwrap_or_default();
        self.fb.field.set_text(&t);
        self.fb.msg = None;
    }

    pub fn submit_feedback(&mut self) {
        if !self.feedback_ready() || self.fb.busy {
            return;
        }
        let text = self.fb.field.text.split_whitespace().collect::<Vec<_>>().join(" ");
        if text.chars().count() < FEEDBACK_MIN_LEN {
            self.fb.msg = Some((tr("Write a bit more before sending."), BAD));
            return;
        }
        self.fb.busy = true;
        self.fb.msg = None;
        let client = self.client.clone().unwrap();
        let acc = self.account.clone().unwrap();
        let text_ok = text.clone();
        self.run_job(
            move || client.publish_feedback(&text),
            move |g, _: ()| {
                g.fb.busy = false;
                g.fb.editing = false;
                g.set_feedback_focus(false);
                let entry = Feedback { uid: acc.uid.clone(), username: acc.username.clone(), text: text_ok.clone(), updated_at: Some(now_ts()) };
                let mut list = vec![entry.clone()];
                list.extend(g.fb.list.drain(..).filter(|e| e.uid != entry.uid));
                g.fb.list = list;
                g.fb.mine = Some(entry);
                g.fb.field.set_text(&text_ok);
                g.fb.msg = Some((tr("Thanks! Your feedback is published."), GOOD));
                g.fb.next_refresh = 0.0;
            },
            |g, e| {
                g.fb.busy = false;
                g.fb.msg = Some((online_error_text(&e), BAD));
            },
        );
    }

    pub fn ask_delete_feedback(&mut self) {
        self.fb.confirm_delete = true;
        self.fb.msg = None;
    }

    pub fn delete_feedback(&mut self) {
        if !self.feedback_ready() || self.fb.busy || self.fb.mine.is_none() {
            return;
        }
        self.fb.busy = true;
        self.fb.confirm_delete = false;
        self.fb.msg = None;
        let client = self.client.clone().unwrap();
        let uid = self.account.as_ref().unwrap().uid.clone();
        self.run_job(
            move || client.delete_feedback(),
            move |g, _: ()| {
                g.fb.busy = false;
                g.fb.mine = None;
                g.fb.editing = false;
                g.set_feedback_focus(false);
                g.fb.list.retain(|e| e.uid != uid);
                g.fb.field.set_text("");
                g.fb.msg = Some((tr("Feedback deleted. You can write a new one."), GOOD));
                g.fb.next_refresh = 0.0;
            },
            |g, e| {
                g.fb.busy = false;
                g.fb.msg = Some((online_error_text(&e), BAD));
            },
        );
    }

    // ================================================================ drawing
    pub fn draw_feedback(&mut self, mouse_pos: (f64, f64)) {
        self.refresh_feedback(false);
        let ov = dim_overlay(self.vw, VIRTUAL_H, 170);
        self.canvas.blit(&ov, 0, 0);

        let panel_w = PANEL_W.min(self.vw - 40);
        let top = 40;
        let rect = Rect::new(self.vw / 2 - panel_w / 2, top, panel_w, VIRTUAL_H - top - 30);
        draw_panel(&mut self.canvas, rect, Some(panel()), 16, true, None);
        self.register_button(rect, Rc::new(|_: &mut Game| {}), None);

        let title = self.f.big.render(&tr("Feedback"), WHITE);
        self.canvas.blit(&title, rect.x + 26, rect.y + 16);
        let sub = self.f.small.render(&tr("One message per player - you can edit or delete yours."), grey());
        self.canvas.blit(&sub, rect.x + 26, rect.y + 18 + title.h);
        let close_rect = Rect::new(rect.right() - 48, rect.y + 18, 30, 30);
        let sb = self.f.small_b.clone();
        self.button(close_rect, "X", &sb, mouse_pos, panel_light(), BAD, WHITE, cb(|g| g.close_feedback()), Bo::r(8));

        let body_top = rect.y + 24 + title.h + sub.h;
        let body = Rect::new(rect.x + 22, body_top, panel_w - 44, rect.bottom() - 20 - body_top);
        if !self.feedback_ready() {
            self.draw_feedback_message(body, &tr("Log in from the main menu to leave feedback."));
            return;
        }
        let mine_h = self.draw_feedback_mine(body, mouse_pos);
        let foot_h = self.draw_feedback_footer(rect, mouse_pos);
        let list_rect = Rect::new(body.x, body.y + mine_h + 14, body.w, rect.bottom() - 20 - foot_h - (body.y + mine_h + 14));
        self.fb.list_rect = list_rect;
        self.draw_feedback_list(list_rect, mouse_pos);
    }

    fn draw_feedback_message(&mut self, rect: Rect, text: &str) {
        let t = self.f.med.render(text, grey());
        let r = Rect::with_center(t.w, t.h, rect.center());
        self.canvas.blit(&t, r.x, r.y);
    }

    fn draw_feedback_mine(&mut self, body: Rect, mouse_pos: (f64, f64)) -> i32 {
        let sb = self.f.small_b.clone();
        let busy = self.fb.busy;
        if self.fb.editing {
            let bx = Rect::new(body.x, body.y, body.w, EDITOR_H);
            let focus = self.fb.focus;
            self.draw_text_area(bx, FieldRef::Feedback, &tr("What would make the game better?"), focus, Rc::new(|g: &mut Game| g.set_feedback_focus(true)));
            let left = self.f.tiny.render(&format!("{}/{}", self.fb.field.text.chars().count(), FEEDBACK_MAX_LEN), grey_dim());
            self.canvas.blit(&left, bx.right() - left.w - 12, bx.bottom() - 18);
            let btn_y = bx.bottom() + 8;
            let bw = (body.w - 10) / 2;
            self.button(Rect::new(body.x, btn_y, bw, 38), &if busy { tr("Sending...") } else { tr("Publish") }, &sb, mouse_pos, accent(), accent_hover(), BLACK, if busy { None } else { cb(|g| g.submit_feedback()) }, Bo::r(10));
            self.button(Rect::new(body.x + bw + 10, btn_y, bw, 38), &tr("Cancel"), &sb, mouse_pos, panel_light(), panel_lighter(), WHITE, cb(|g| g.cancel_feedback_edit()), Bo::r(10));
            return EDITOR_H + 8 + 38;
        }
        let Some(mine) = self.fb.mine.clone() else {
            let bx = Rect::new(body.x, body.y, body.w, 56);
            draw::rect(&mut self.canvas, panel_light(), bx, 0, 12);
            draw::rect(&mut self.canvas, outline(), bx, 2, 12);
            let t = sb.render(&tr("You haven't left feedback yet."), grey());
            self.canvas.blit(&t, bx.x + 14, bx.centery() - t.h / 2);
            self.button(Rect::new(bx.right() - 152, bx.y + 9, 140, bx.h - 18), &tr("Write feedback"), &sb, mouse_pos, accent(), accent_hover(), BLACK, cb(|g| g.start_feedback_edit()), Bo::r(10));
            return bx.h;
        };
        let mut lines = wrap_text(&mine.text, &sb, body.w - 28);
        lines.truncate(3);
        let h = 64.max(34 + lines.len() as i32 * (sb.get_height() + 3) + 44);
        let bx = Rect::new(body.x, body.y, body.w, h);
        draw::rect(&mut self.canvas, panel_light(), bx, 0, 12);
        draw::rect(&mut self.canvas, accent(), bx, 2, 12);
        let head = sb.render(&tr("Your feedback"), accent());
        self.canvas.blit(&head, bx.x + 14, bx.y + 10);
        let mut y = bx.y + 12 + head.h;
        for line in &lines {
            let t = sb.render(line, WHITE);
            self.canvas.blit(&t, bx.x + 14, y);
            y += sb.get_height() + 3;
        }
        let bw = (bx.w - 28 - 10) / 2;
        let by = bx.bottom() - 42;
        self.button(Rect::new(bx.x + 14, by, bw, 32), &tr("Edit"), &sb, mouse_pos, panel_lighter(), accent_hover(), WHITE, if busy { None } else { cb(|g| g.start_feedback_edit()) }, Bo::r(9));
        if self.fb.confirm_delete {
            self.button(Rect::new(bx.x + 24 + bw, by, bw, 32), &tr("Confirm delete"), &sb, mouse_pos, Color::rgb(150, 60, 60), BAD, WHITE, if busy { None } else { cb(|g| g.delete_feedback()) }, Bo::r(9));
        } else {
            self.button(Rect::new(bx.x + 24 + bw, by, bw, 32), &tr("Delete"), &sb, mouse_pos, panel_lighter(), BAD, WHITE, if busy { None } else { cb(|g| g.ask_delete_feedback()) }, Bo::r(9));
        }
        h
    }

    fn feedback_card_height(&self, entry: &Feedback, width: i32) -> i32 {
        let sb = &self.f.small_b;
        let n = wrap_text(&entry.text, sb, width - 28).len().min(4) as i32;
        30 + n * (sb.get_height() + 3) + 12
    }

    fn draw_feedback_card(&mut self, rect: Rect, entry: &Feedback) {
        let mine = self.account.as_ref().is_some_and(|a| a.uid == entry.uid);
        let sb = self.f.small_b.clone();
        draw::rect(&mut self.canvas, panel_light(), rect, 0, 12);
        draw::rect(&mut self.canvas, if mine { accent() } else { outline() }, rect, 2, 12);
        let name = sb.render(&fit_text(&sb, &entry.username, rect.w - 140), if mine { accent() } else { WHITE });
        self.canvas.blit(&name, rect.x + 14, rect.y + 8);
        let when = feedback_when(entry.updated_at);
        if !when.is_empty() {
            let t = self.f.tiny.render(&when, grey_dim());
            self.canvas.blit(&t, rect.right() - t.w - 14, rect.y + 11);
        }
        let mut y = rect.y + 10 + name.h;
        for line in wrap_text(&entry.text, &sb, rect.w - 28).iter().take(4) {
            let t = sb.render(line, grey());
            self.canvas.blit(&t, rect.x + 14, y);
            y += sb.get_height() + 3;
        }
    }

    fn draw_feedback_list(&mut self, rect: Rect, mouse_pos: (f64, f64)) {
        let uid = self.account.as_ref().map(|a| a.uid.clone());
        let entries: Vec<Feedback> = self.fb.list.iter().filter(|e| Some(&e.uid) != uid.as_ref()).cloned().collect();
        if entries.is_empty() {
            let msg = if self.fb.loading { tr("Loading...") } else { tr("No feedback yet. Be the first!") };
            self.draw_feedback_message(rect, &msg);
            self.fb.max_scroll = 0.0;
            return;
        }
        let scroll = self.fb.scroll;
        self.push_clip(rect);
        let mut y = rect.top() as f64 - scroll;
        let width = rect.w - 16;
        let mut content_h = 0;
        for entry in &entries {
            let h = self.feedback_card_height(entry, width);
            let card = Rect::new(rect.x, ti(y), width, h);
            if card.bottom() >= rect.top() - 4 && card.top() <= rect.bottom() + 4 {
                self.draw_feedback_card(card, entry);
            }
            y += (h + ROW_GAP) as f64;
            content_h += h + ROW_GAP;
        }
        let content_h = (content_h - ROW_GAP).max(0) as f64;
        self.pop_clip();
        self.fb.max_scroll = (content_h - rect.h as f64).max(0.0);
        self.fb.scroll = self.fb.scroll.min(self.fb.max_scroll).max(0.0);
        self.draw_scrollbar(rect, scroll, content_h, Some("feedback"), Some(mouse_pos));
    }

    fn draw_feedback_footer(&mut self, rect: Rect, mouse_pos: (f64, f64)) -> i32 {
        let foot_h = 44;
        let y = rect.bottom() - 18 - 34;
        let sb = self.f.small_b.clone();
        self.button(Rect::new(rect.x + 22, y, 120, 34), &tr("Refresh"), &sb, mouse_pos, panel_light(), panel_lighter(), WHITE, cb(|g| g.refresh_feedback(true)), Bo::r(9));
        let (text, color) = if let Some((m, c)) = &self.fb.msg {
            (m.clone(), *c)
        } else if let Some(e) = &self.fb.error {
            (e.clone(), BAD)
        } else if self.fb.loading {
            (tr("Updating..."), grey_dim())
        } else {
            (String::new(), grey_dim())
        };
        if !text.is_empty() {
            let small = self.f.small.clone();
            let t = small.render(&fit_text(&small, &text, rect.w - 190), color);
            self.canvas.blit(&t, rect.right() - 22 - t.w, y + 17 - t.h / 2);
        }
        foot_h
    }
}
