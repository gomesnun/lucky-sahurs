//! Credits page: who made the sounds and music (ui/credits_panel.py).

use super::base::Bo;
use super::{Game, cb};
use crate::config::VIRTUAL_H;
use crate::gfx::{Rect, draw};
use crate::i18n::tr;
use crate::theme::*;
use crate::tr;
use crate::ui::drawing::{dim_overlay, draw_panel, draw_state_border};
use crate::ui::fonts::{fit_text, wrap_text};
use std::rc::Rc;

const CREDITS: [(&str, &str, &str, &str, &str); 5] = [
    ("Level Up 01", "shinephoenixstormcrow  (original by rhodesmas)", "CC BY 3.0", "https://freesound.org/s/337049/", "buying upgrades"),
    ("Level Up 01", "mokasza", "CC BY 4.0", "https://freesound.org/s/810753/", "milestones"),
    ("Holy Protection Skill Buff", "EminYILDIRIM", "CC BY 4.0", "https://freesound.org/s/621206/", "rebirth (cut to the first 1.5 seconds)"),
    ("UI Button Click", "benzix2", "CC0", "https://freesound.org/s/467951/", "button clicks"),
    ("Lofi Hiphop Melody Loop 99 BPM", "holizna", "CC0", "https://freesound.org/s/629155/", "background music"),
];

const CREDITS_NOTE: &str = "Click an entry to open its page on Freesound.org. The sounds were converted to .ogg. CC BY = Creative Commons Attribution; CC0 = public domain (no credit needed).";

/// webbrowser.open(url)
pub fn open_browser(url: &str) -> bool {
    let r = if cfg!(target_os = "windows") {
        std::process::Command::new("cmd").args(["/C", "start", "", url]).spawn()
    } else if cfg!(target_os = "macos") {
        std::process::Command::new("open").arg(url).spawn()
    } else {
        std::process::Command::new("xdg-open").arg(url).spawn()
    };
    r.is_ok()
}

impl Game {
    pub fn open_credits(&mut self) {
        self.close_overlays();
        self.credits_open = true;
    }

    pub fn close_credits(&mut self) {
        self.credits_open = false;
    }

    pub fn toggle_credits(&mut self) {
        if self.credits_open {
            self.close_credits();
        } else {
            self.open_credits();
        }
    }

    pub fn open_url(&mut self, url: &str) {
        if !open_browser(url) {
            self.show_toast(&tr("Couldn't open the browser."), 1.8);
        }
    }

    pub fn draw_credits(&mut self, mouse_pos: (f64, f64)) {
        let ov = dim_overlay(self.vw, VIRTUAL_H, 170);
        self.canvas.blit(&ov, 0, 0);
        let panel_w = 660;
        let (row_h, gap) = (76, 8);
        let tiny = self.f.tiny.clone();
        let small = self.f.small.clone();
        let sb = self.f.small_b.clone();
        let med = self.f.med.clone();
        let note_lines = wrap_text(&tr(CREDITS_NOTE), &tiny, panel_w - 48);
        let head_h = 86;
        let panel_h = head_h + CREDITS.len() as i32 * (row_h + gap) + 10 + 16 * note_lines.len() as i32 + 22;
        let rect = Rect::new(self.vw / 2 - panel_w / 2, VIRTUAL_H / 2 - panel_h / 2, panel_w, panel_h);
        draw_panel(&mut self.canvas, rect, Some(panel()), 16, true, None);
        self.register_button(rect, Rc::new(|_: &mut Game| {}), None);

        let title = self.f.big.render(&tr("Credits"), WHITE);
        self.canvas.blit(&title, rect.x + 24, rect.y + 18);
        let close_rect = Rect::new(rect.right() - 46, rect.y + 20, 28, 28);
        self.button(close_rect, "X", &sb, mouse_pos, panel_light(), BAD, WHITE, cb(|g| g.close_credits()), Bo::r(8));
        let sub = small.render(&tr("Sound effects and music, from Freesound.org"), grey());
        self.canvas.blit(&sub, rect.x + 26, rect.y + 18 + title.h + 2);

        let mut y = rect.y + head_h;
        for (name, author, license_name, url, used_for) in CREDITS {
            let row = Rect::new(rect.x + 22, y, panel_w - 44, row_h);
            let hovering = row.collidepoint(mouse_pos);
            draw::rect(&mut self.canvas, if hovering { panel_lighter() } else { panel_light() }, row, 0, 10);
            if hovering {
                draw_state_border(&mut self.canvas, row, accent(), 10, 2);
            }
            let lic = sb.render(license_name, accent());
            self.canvas.blit(&lic, row.right() - 16 - lic.w, row.y + 10);
            let title_txt = fit_text(&med, name, row.w - 32 - lic.w - 12);
            let t = med.render(&title_txt, WHITE);
            self.canvas.blit(&t, row.x + 16, row.y + 8);
            let by = fit_text(&small, &tr!("by %s", author), row.w - 32);
            let t = small.render(&by, grey());
            self.canvas.blit(&t, row.x + 16, row.y + 34);
            let short_url = url.replace("https://", "");
            let line3 = fit_text(&tiny, &tr!("Used for: %s   -   %s", tr(used_for), short_url), row.w - 32);
            let t = tiny.render(&line3, grey_dim());
            self.canvas.blit(&t, row.x + 16, row.y + 54);
            self.register_button(row, Rc::new(move |g: &mut Game| g.open_url(url)), Some("click"));
            y += row_h + gap;
        }
        let mut ny = y + 2;
        for line in &note_lines {
            let t = tiny.render(line, grey());
            self.canvas.blit(&t, rect.x + 26, ny);
            ny += 16;
        }
    }
}
