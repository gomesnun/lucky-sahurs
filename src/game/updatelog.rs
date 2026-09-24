//! Update Log page: what's new in each version (ui/updatelog_panel.py).

use super::base::Bo;
use super::{Game, cb};
use crate::config::VIRTUAL_H;
use crate::gfx::{Rect, draw, ti};
use crate::i18n::tr;
use crate::theme::*;
use crate::ui::drawing::{dim_overlay, draw_panel};
use crate::ui::fonts::wrap_text;
use std::rc::Rc;

pub const UPDATE_LOG: &[(&str, &str, &[&str])] = &[
    ("v2.9.1", "2026-09-24", &[
        "\"Use All Charges\" with huge amounts of trait charges (like 600M) no longer freezes and crashes the game: they are all used instantly.",
    ]),
    ("v2.9.0", "2026-09-24", &[
        "New Shop (left side, unlocks at Rebirth 1): the stock changes every 10 minutes and is the same for every player; each item has 1 in stock.",
        "10 Dice: buy one once and it's yours forever. Equip one for extra luck - and your ROLL button takes its style.",
        "Potions (Coin, Speed and Luck, levels I to V): use them in the Bag > Potions; 5 of one level combine into 1 of the next.",
        "Base luck is a little lower to make room for Dice and Luck Potions.",
        "Admin Abuse fixed: Luck events now really multiply your chances (x1M turns a 1 in 500B pet into 1 in 500K) - and so do Dice, Luck Potions and Golden/Diamond/Rainbow Rolls.",
        "Very fast Auto Roller (like a x1M Speed event) now really does all its rolls instead of stopping at ~3,000/sec.",
        "When it rolls too fast to show each pet, the card says \"Too fast to show!\" with your rolls/sec and the best pet so far.",
    ]),
    ("v2.8.0", "2026-09-24", &[
        "2 new rarities above Absolute: Primordial (lava) and Paradox (half light, half dark), with 4 verities each, their own card backgrounds, sounds and cutscenes.",
        "5 new traits above Omnipotent: Eternal, Infinite, Godlike, Primeval and Supreme.",
        "Milestones: the rarity milestones (Secret to Absolute) are grouped in \"Rarity Milestones\", which opens its own list.",
        "Trades fixed: accepting a trade gave an error, and pets you rolled were not reaching your online wallet (what your friends see when they propose a trade).",
    ]),
    ("v2.7.0", "2026-09-24", &[
        "3 new rarities above Transcendent: Ethereal, Celestial and Absolute - each one earns more than the one before, with their own card backgrounds and cutscenes.",
        "34 new verities: every rarity now has 4 (each one rarer and worth more than the last).",
        "New Rainbow mutation (x27 money), with its own upgrades, Index tab and milestones.",
        "Lots of new Luck upgrades (Luck II, Prismatic Luck II, Ultimate Luck and one for each top rarity) and Ultimate Money. High upgrade levels are much cheaper now.",
        "Auto Upgrader (in Upgrades > Misc): buys the cheapest upgrade for you and never resets on Rebirth.",
        "The Index opens as a big page, there are 4 new milestone categories and many more milestone goals, and rolls / cutscenes have better animations.",
        "Accepting a friend request no longer fails when the request was sent again.",
    ]),
    ("v2.6.2", "2026-09-24", &[
        "Trades: if Accept can't go through, the Trades list now says why (for example, you don't have enough of that pet in the save you have open). Before, nothing seemed to happen.",
    ]),
    ("v2.6.1", "2026-09-24", &[
        "Trades: your friends now see all your pets when proposing a trade (before, it could say \"Nothing to trade\"). Pets show their own name (Levity, Gravity...) instead of the rarity.",
        "The game checks the server less often, so the free online quota lasts much longer.",
    ]),
    ("v2.6.0", "2026-09-23", &[
        "Sell pets: every pet in Bag > Inventory has a $ button. Type how many to sell (or Sell All) and confirm - more than you have sells them all.",
        "Friends: see who is online right now (green dot) or when they were last seen. New icons on the Friends, Chat and Trade buttons.",
        "Leaderboard: one shared snapshot every 30 minutes, and money no longer gets stuck after big events.",
        "Admins: global events can now last up to 24 hours and any duration works; new Bans page.",
    ]),
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
        self.update_log_scroll = 0.0;
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

        let panel_w = 580.min(self.vw - 40);
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

        // a fixed-height panel (fits the screen) - the content scrolls inside it if it's taller
        let top = 40;
        let rect = Rect::new(self.vw / 2 - panel_w / 2, top, panel_w, VIRTUAL_H - top - 30);
        draw_panel(&mut self.canvas, rect, Some(panel()), 16, true, None);
        self.register_button(rect, Rc::new(|_: &mut Game| {}), None);

        self.canvas.blit(&title, rect.x + pad, rect.y + title_y);
        self.canvas.blit(&sub, rect.x + pad + 2, rect.y + sub_y);
        let close_rect = Rect::new(rect.right() - pad - 32, rect.y + title_y + 2, 32, 32);
        let sb = self.f.small_b.clone();
        self.button(close_rect, "X", &sb, mouse_pos, panel_light(), BAD, WHITE, cb(|g| g.close_update_log()), Bo::r(8));

        let inner_w = rect.w - pad * 2;
        let list_rect = Rect::new(rect.x + pad, rect.y + head_h, inner_w, rect.bottom() - pad - (rect.y + head_h));
        self.update_log_list_rect = list_rect;

        if UPDATE_LOG.is_empty() {
            let note_lines = wrap_text(&tr(EMPTY_NOTE), &small, inner_w - box_pad * 2);
            let body_h = box_pad * 2 + 24 * note_lines.len() as i32 - 4;
            let bx = Rect::new(list_rect.x, list_rect.y, inner_w, body_h);
            draw::rect(&mut self.canvas, panel_light(), bx, 0, 12);
            let mut ny = bx.y + box_pad;
            for line in &note_lines {
                let t = small.render(line, grey_dim());
                let r = Rect::with_midtop(t.w, t.h, (bx.centerx(), ny));
                self.canvas.blit(&t, r.x, r.y);
                ny += 24;
            }
            self.update_log_max_scroll = 0.0;
            return;
        }

        let mut entries: Vec<(&str, &str, Vec<String>, i32)> = Vec::new();
        for (version, date, lines) in UPDATE_LOG {
            let mut wrapped = Vec::new();
            for line in lines.iter() {
                wrapped.extend(wrap_text(&format!("- {}", tr(line)), &small, inner_w - box_pad * 2));
            }
            let row_h = box_pad + med.get_height() + 10 + 22 * wrapped.len() as i32 + box_pad - 4;
            entries.push((version, date, wrapped, row_h));
        }
        let gap = 14;
        let scroll = self.update_log_scroll;
        self.push_clip(list_rect);
        let mut y = list_rect.top() as f64 - scroll;
        let mut content_h = 0;
        for (version, date, wrapped, row_h) in &entries {
            let row = Rect::new(list_rect.x, ti(y), inner_w, *row_h);
            if row.bottom() >= list_rect.top() - 4 && row.top() <= list_rect.bottom() + 4 {
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
            }
            y += (row_h + gap) as f64;
            content_h += row_h + gap;
        }
        let content_h = (content_h - gap).max(0) as f64;
        self.pop_clip();
        self.update_log_max_scroll = (content_h - list_rect.h as f64).max(0.0);
        self.update_log_scroll = self.update_log_scroll.clamp(0.0, self.update_log_max_scroll);
        self.draw_scrollbar(list_rect, scroll, content_h, Some("update_log"), Some(mouse_pos));
    }
}
