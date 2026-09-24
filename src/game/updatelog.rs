//! Update Log page: what's new in each version (ui/updatelog_panel.py).

use super::base::Bo;
use super::{Game, cb};
use crate::config::VIRTUAL_H;
use crate::gfx::{Rect, draw};
use crate::i18n::tr;
use crate::theme::*;
use crate::ui::drawing::{dim_overlay, draw_panel};
use crate::ui::fonts::wrap_text;
use std::rc::Rc;

pub const UPDATE_LOG: &[(&str, &str, &[&str])] = &[
    ("v2.4.0", "2026-09-22", &[
        "Chat: talk to your friends in the game - the Friends page now has a Chat button on each friend, and a red dot shows up when a new message arrives.",
        "Global events: one account can start a timed bonus (like 10x Luck for 5 minutes) that applies to every player online, with a banner at the top while it lasts.",
    ]),
    ("v2.3.0", "2026-09-22", &[
        "Feedback: the new button next to Update Log opens a page where you leave one message for the game - you can edit it or delete it, and after deleting you can write another one.",
        "Android: everything online (account, leaderboard, friends, feedback) was failing on the phone because the app had no HTTPS certificates. Fixed.",
    ]),
    ("v2.2.1", "2026-09-22", &[
        "Friend search now also finds players who have not opened the Friends page yet.",
        "Typing anywhere in the game: arrow keys, Home/End and Delete move the cursor, and clicking inside a box puts the cursor where you clicked.",
        "No more stray space at the end of what you type (it was breaking the friend search).",
    ]),
    ("v2.2.0", "2026-09-22", &[
        "Friends: the new button next to Stats opens a page where you search a player by username, send a friend request and, once they accept, see their stats.",
        "Profile photos: any Verity you already own can be your photo - Golden and Diamond keep their coloured ring.",
        "A red dot on the Friends button means someone is waiting for an answer.",
    ]),
    ("v2.1.0", "2026-09-22", &[
        "The game is now Lucky Verities: new name, new art, and 22 Verities to collect.",
        "Your saves come with you - the first time you open it, everything is copied over from the old Lucky Sahurs folder.",
        "New cutscenes when a rare Verity shows up, and the Options page is now split into tabs.",
        "On the phone this installs as a new app next to the old one (Android needs it that way for the rename): log in to your account to bring your save across, then you can remove the old icon.",
    ]),
    ("v2.0.0", "2026-09-22", &[
        "The Android build updates itself now: when a new version comes out the game downloads it and asks Android to install it, the same way the computer version does.",
        "The first time, Android asks you to allow the game to install apps - say yes and tap Try again.",
    ]),
    ("v1.0.5", "2026-09-22", &[
        "Android: the game now runs at 60 FPS on the phone.",
        "The main screen was drawing the pet card and its glow pixel by pixel every frame (34 ms of the 40 ms each frame took); they are now blended into the background once.",
        "Blended images that nothing asks for any more are thrown away, so a long session no longer gets slower and slower.",
        "The phone build is on the Releases page: Lucky-Verities-Android.apk.",
    ]),
];

const EMPTY_NOTE: &str = "Nothing here yet - future updates will be listed on this page.";

impl Game {
    pub fn open_update_log(&mut self) {
        self.close_overlays();
        self.update_log_open = true;
    }

    pub fn close_update_log(&mut self) {
        self.update_log_open = false;
    }

    pub fn toggle_update_log(&mut self) {
        if self.update_log_open {
            self.close_update_log();
        } else {
            self.open_update_log();
        }
    }

    pub fn draw_update_log(&mut self, mouse_pos: (f64, f64)) {
        let ov = dim_overlay(self.vw, VIRTUAL_H, 170);
        self.canvas.blit(&ov, 0, 0);

        let panel_w = 580;
        let pad = 28;
        let box_pad = 18;
        let small = self.f.small.clone();
        let med = self.f.med.clone();
        let tiny = self.f.tiny.clone();
        let title = self.f.big.render(&tr("Update Log"), WHITE);
        let sub = small.render(&tr("What's new in Lucky Verities"), grey());
        let title_y = 24;
        let sub_y = title_y + title.h + 6;
        let head_h = sub_y + sub.h + 22;

        let inner_w = panel_w - pad * 2;
        let gap = 14;
        let mut entries: Vec<(&str, &str, Vec<String>, i32)> = Vec::new();
        let mut note_lines = Vec::new();
        let body_h;
        if !UPDATE_LOG.is_empty() {
            for (version, date, lines) in UPDATE_LOG {
                let mut wrapped = Vec::new();
                for line in lines.iter() {
                    wrapped.extend(wrap_text(&format!("- {}", tr(line)), &small, inner_w - box_pad * 2));
                }
                let row_h = box_pad + med.get_height() + 10 + 22 * wrapped.len() as i32 + box_pad - 4;
                entries.push((version, date, wrapped, row_h));
            }
            body_h = entries.iter().map(|e| e.3).sum::<i32>() + gap * (entries.len() as i32 - 1);
        } else {
            note_lines = wrap_text(&tr(EMPTY_NOTE), &small, inner_w - box_pad * 2);
            body_h = box_pad * 2 + 24 * note_lines.len() as i32 - 4;
        }

        let panel_h = head_h + body_h + pad;
        let rect = Rect::new(self.vw / 2 - panel_w / 2, VIRTUAL_H / 2 - panel_h / 2, panel_w, panel_h);
        draw_panel(&mut self.canvas, rect, Some(panel()), 16, true, None);
        self.register_button(rect, Rc::new(|_: &mut Game| {}), None);

        self.canvas.blit(&title, rect.x + pad, rect.y + title_y);
        self.canvas.blit(&sub, rect.x + pad + 2, rect.y + sub_y);
        let close_rect = Rect::new(rect.right() - pad - 32, rect.y + title_y + 2, 32, 32);
        let sb = self.f.small_b.clone();
        self.button(close_rect, "X", &sb, mouse_pos, panel_light(), BAD, WHITE, cb(|g| g.close_update_log()), Bo::r(8));

        let mut y = rect.y + head_h;
        if UPDATE_LOG.is_empty() {
            let bx = Rect::new(rect.x + pad, y, inner_w, body_h);
            draw::rect(&mut self.canvas, panel_light(), bx, 0, 12);
            let mut ny = bx.y + box_pad;
            for line in &note_lines {
                let t = small.render(line, grey_dim());
                let r = Rect::with_midtop(t.w, t.h, (bx.centerx(), ny));
                self.canvas.blit(&t, r.x, r.y);
                ny += 24;
            }
            return;
        }
        for (version, date, wrapped, row_h) in &entries {
            let row = Rect::new(rect.x + pad, y, inner_w, *row_h);
            draw::rect(&mut self.canvas, panel_light(), row, 0, 12);
            let head = med.render(version, WHITE);
            self.canvas.blit(&head, row.x + box_pad, row.y + box_pad - 2);
            let dtxt = tiny.render(date, grey_dim());
            self.canvas.blit(&dtxt, row.right() - box_pad - dtxt.w, row.y + box_pad - 2 + (head.h - dtxt.h).div_euclid(2));
            let mut ny = row.y + box_pad - 2 + head.h + 10;
            for wl in wrapped {
                let t = small.render(wl, grey());
                self.canvas.blit(&t, row.x + box_pad, ny);
                ny += 22;
            }
            y += row_h + gap;
        }
    }
}
