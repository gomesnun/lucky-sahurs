//! "New version available" screen (ui/update_panel.py): shows ON TOP OF EVERYTHING when GitHub has
//! a newer version (the check is in online/updater.rs). Only two ways out: Update now, or No -
//! which closes the game, because the game must always be up to date.
//!
//! Phases (upd_phase):  idle -> checking -> available -> downloading -> installing   (or "error")

use super::base::Bo;
use super::{Game, cb};
use crate::config::{UPDATE_CHECK_INTERVAL, UPDATE_RETRY_AFTER_ERROR, VERSION, VIRTUAL_H};
use crate::core::state::now_ts;
use crate::gfx::{Color, Rect, Surface, draw};
use crate::i18n::tr;
use crate::online::updater::{self, Plan, Progress, ReleaseInfo, UpdateError};
use crate::theme::*;
use crate::tr;
use crate::ui::drawing::draw_panel;
use crate::ui::fonts::wrap_text;
use std::rc::Rc;
use std::sync::{Arc, Mutex};

const ACTIVE_PHASES: [&str; 4] = ["available", "downloading", "installing", "error"];

pub struct UpdaterUi {
    pub phase: &'static str,
    pub info: Option<ReleaseInfo>, // the new version (see updater::fetch_latest)
    pub plan: Option<Plan>,        // already downloaded and ready to install
    pub progress: Progress,        // 0..1 while downloading
    pub error: &'static str,       // net | corrupt | perm | other
    pub error_detail: String,
    pub next_check: f64,
    pub enabled: bool,
}

impl Default for UpdaterUi {
    fn default() -> Self {
        UpdaterUi {
            phase: "idle",
            info: None,
            plan: None,
            progress: Arc::new(Mutex::new(0.0)),
            error: "other",
            error_detail: String::new(),
            next_check: 0.0,
            enabled: false,
        }
    }
}

impl Game {
    // ---------------------------------------------------------------- state / checking
    pub fn init_updater(&mut self) {
        self.upd = UpdaterUi::default();
        self.upd.next_check = now_ts() + 1.5; // the 1st check is right after opening
        self.upd.enabled = updater::updates_enabled(); // only in builds made from a tag
        if self.upd.enabled {
            updater::cleanup_leftovers();
        }
    }

    pub fn update_modal_active(&self) -> bool {
        ACTIVE_PHASES.contains(&self.upd.phase)
    }

    fn upd_progress(&self) -> f64 {
        self.upd.progress.lock().map(|g| *g).unwrap_or(0.0)
    }

    /// Every frame: now and then asks GitHub whether there's a new version (in a background thread).
    pub fn tick_updater(&mut self) {
        if !self.upd.enabled || self.upd.phase != "idle" || now_ts() < self.upd.next_check {
            return;
        }
        self.upd.phase = "checking";
        self.run_job(
            || Ok(updater::fetch_latest()),
            |g: &mut Game, r: Result<Option<ReleaseInfo>, UpdateError>| match r {
                Ok(Some(info)) => {
                    g.upd.info = Some(info);
                    g.upd.phase = "available";
                }
                Ok(None) => {
                    g.upd.phase = "idle";
                    g.upd.next_check = now_ts() + UPDATE_CHECK_INTERVAL;
                }
                // no internet / GitHub down: does NOT block the game, tries again later
                Err(_) => {
                    g.upd.phase = "idle";
                    g.upd.next_check = now_ts() + UPDATE_RETRY_AFTER_ERROR;
                }
            },
            |g: &mut Game, _| {
                g.upd.phase = "idle";
                g.upd.next_check = now_ts() + UPDATE_RETRY_AFTER_ERROR;
            },
        );
    }

    // ---------------------------------------------------------------- actions
    /// "Try again": if the file is already downloaded, it isn't downloaded again.
    pub fn upd_retry(&mut self) {
        if let Some(plan) = self.upd.plan.clone() {
            self.upd.phase = "installing";
            self.upd_finish(plan);
        } else {
            self.upd_start_download();
        }
    }

    pub fn upd_start_download(&mut self) {
        let Some(info) = self.upd.info.clone() else { return };
        self.upd.phase = "downloading";
        let progress = Arc::new(Mutex::new(0.0));
        self.upd.progress = progress.clone();
        self.run_job(
            move || Ok(updater::prepare(&info, &Some(progress))),
            |g: &mut Game, r: Result<Plan, UpdateError>| match r {
                Ok(plan) => {
                    g.upd.plan = Some(plan.clone());
                    g.upd.phase = "installing";
                    g.upd_finish(plan);
                }
                Err(e) => g.upd_fail(e.code, &e.detail),
            },
            |g: &mut Game, e| g.upd_fail("other", &e.message),
        );
    }

    pub fn upd_fail(&mut self, code: &'static str, detail: &str) {
        self.upd.phase = "error";
        self.upd.error = code;
        self.upd.error_detail = detail.to_string();
    }

    /// Everything downloaded and prepared: saves the game, asks the helper to swap the program and
    /// reopen it, and closes.
    pub fn upd_finish(&mut self, plan: Plan) {
        self.save_everything();
        if let Err(e) = updater::launch(&plan) {
            self.upd_fail(e.code, &e.detail);
            return;
        }
        std::process::exit(0);
    }

    pub fn upd_open_page(&mut self) {
        let page = self.upd.info.as_ref().map(|i| i.page.clone()).filter(|p| !p.is_empty());
        self.open_url(&page.unwrap_or_else(|| "https://github.com/".to_string()));
    }

    // ---------------------------------------------------------------- drawing
    pub fn draw_update_modal(&mut self, mouse_pos: (f64, f64)) {
        // nothing underneath stays clickable (not even the top buttons), and ESC / clicking outside don't close it
        self.buttons.clear();
        self.scrollbar_hits.clear();
        let mut overlay = Surface::new_alpha(self.vw, VIRTUAL_H);
        overlay.fill(Color::rgba(0, 0, 0, 205), None);
        self.canvas.blit(&overlay, 0, 0);

        let phase = self.upd.phase;
        let (panel_w, panel_h) = (640, 330);
        let rect = Rect::new(self.vw / 2 - panel_w / 2, VIRTUAL_H / 2 - panel_h / 2, panel_w, panel_h);
        draw_panel(&mut self.canvas, rect, Some(panel()), 16, true, None);
        self.register_button(rect, Rc::new(|_: &mut Game| {}), None); // clicking inside the panel does nothing
        let (inner_x, inner_w) = (rect.x + 28, panel_w - 56);

        let (title, title_color) =
            if phase == "error" { (tr("The update failed"), BAD) } else { (tr("New version available"), accent()) };
        let t = self.f.big.render(&title, title_color);
        self.canvas.blit(&t, inner_x, rect.y + 24);

        let tag = self.upd.info.as_ref().map(|i| i.tag.clone()).unwrap_or_else(|| "?".into());
        let mut y = rect.y + 84;
        let lines: Vec<String> = match phase {
            "available" => vec![tr!("Version %s is out (you have %s).", tag, VERSION), tr("You need to update to keep playing.")],
            "downloading" => vec![tr!("Downloading the update... %d%%", (self.upd_progress() * 100.0) as i64)],
            "installing" => vec![tr("Installing... the game restarts in a moment.")],
            _ => vec![self.upd_error_text()],
        };
        for (i, text) in lines.iter().enumerate() {
            let font = if i == 0 && phase != "error" { self.f.med.clone() } else { self.f.small.clone() };
            for line in wrap_text(text, &font, inner_w) {
                let surf = font.render(&line, if i == 0 { WHITE } else { grey() });
                self.canvas.blit(&surf, inner_x, y);
                y += surf.h + 6;
            }
            y += 6;
        }

        if phase == "downloading" {
            let bar = Rect::new(inner_x, rect.bottom() - 90, inner_w, 24);
            draw::rect(&mut self.canvas, panel_light(), bar, 0, 12);
            let mut fill = bar;
            fill.w = 0.max((bar.w as f64 * self.upd_progress()) as i32);
            if fill.w >= 6 {
                draw::rect(&mut self.canvas, GOOD, fill, 0, 12);
            }
            draw::rect(&mut self.canvas, outline(), bar, 3, 12);
        }

        let (by, bh, gap) = (rect.bottom() - 82, 54, 14);
        let med = self.f.med.clone();
        if phase == "available" {
            let bw = (inner_w - gap) / 2;
            self.button(Rect::new(inner_x, by, bw, bh), &tr("Update now"), &med, mouse_pos, accent(), accent_hover(), BLACK,
                cb(|g| g.upd_start_download()), Bo::r(12));
            self.button(Rect::new(inner_x + bw + gap, by, bw, bh), &tr("No (closes the game)"), &med, mouse_pos, panel_light(),
                panel_lighter(), WHITE, cb(|g| g.quit_game()), Bo::r(12));
        } else if phase == "error" {
            let bw = (inner_w - 2 * gap) / 3;
            let can_retry = self.upd.info.is_some() || self.upd.plan.is_some();
            self.button(Rect::new(inner_x, by, bw, bh), &tr("Try again"), &med, mouse_pos, accent(), accent_hover(), BLACK,
                cb(|g| g.upd_retry()), Bo::r(12).enabled(can_retry));
            self.button(Rect::new(inner_x + bw + gap, by, bw, bh), &tr("Open download page"), &med, mouse_pos, panel_light(),
                panel_lighter(), WHITE, cb(|g| g.upd_open_page()), Bo::r(12));
            self.button(Rect::new(inner_x + 2 * (bw + gap), by, bw, bh), &tr("Close game"), &med, mouse_pos, panel_light(),
                panel_lighter(), WHITE, cb(|g| g.quit_game()), Bo::r(12));
        }
    }

    fn upd_error_text(&self) -> String {
        match self.upd.error {
            "net" => tr("Couldn't download the update. Check your internet connection."),
            "corrupt" => tr("The downloaded file is damaged. Please try again."),
            "perm" => tr("The game can't replace itself in this folder. Move it to another folder (for example the Desktop) or download the new version by hand."),
            code => {
                let d = if self.upd.error_detail.is_empty() { code.to_string() } else { self.upd.error_detail.clone() };
                tr!("Something went wrong (%s).", d)
            }
        }
    }
}
