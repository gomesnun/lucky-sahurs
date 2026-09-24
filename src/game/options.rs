//! Options page with tabs (Gameplay / Interface / Volume / SFX / Game) (ui/options_panel.py).

use super::audio::SLIDER_KNOB_R;
use super::base::Bo;
use super::{Game, cb};
use crate::config::VIRTUAL_H;
use crate::core::data::{RARITY_TIERS, rarities, tier_index};
use crate::core::formatting::py_round;
use crate::gfx::{Color, Rect, draw};
use crate::i18n::{get_language, language_name, next_language, set_language, tr, tr_short};
use crate::storage::{CUTSCENE_RARITIES, SFX_CATEGORIES, save_settings};
use crate::theme::*;
use crate::tr;
use crate::ui::drawing::{dim_overlay, draw_panel};
use crate::ui::fonts::{fit_text, wrap_text};
use std::rc::Rc;

const ROW_H: i32 = 46;
const CONTENT_H: i32 = 230;
const TAB_H: i32 = 42;
const OFF_COLOR: Color = Color::rgb(66, 52, 52);
const TRACK_BG: Color = Color::rgb(22, 22, 25);

const TABS: [(&str, &str); 5] = [("gameplay", "Gameplay"), ("interface", "Interface"), ("volume", "Volume"), ("sfx", "SFX"), ("game", "Game")];

fn on_off(on: bool) -> String {
    if on { tr("On") } else { tr("Off") }
}

impl Game {
    pub fn open_options(&mut self) {
        if self.stats_open {
            self.close_stats();
        }
        self.traits_open = false;
        self.rebirth_open = false;
        self.rebirth_confirm = false;
        self.leaderboard_open = false;
        self.credits_open = false;
        self.options_open = true;
        self.dragging_slider = None;
    }

    pub fn close_options(&mut self) {
        self.options_open = false;
        self.dragging_slider = None;
        save_settings(&self.settings);
    }

    pub fn toggle_options(&mut self) {
        if self.options_open {
            self.close_options();
        } else {
            self.open_options();
        }
    }

    pub fn set_options_tab(&mut self, name: &'static str) {
        self.options_tab = name;
        self.dragging_slider = None;
    }

    pub fn cycle_language(&mut self) {
        let code = set_language(next_language(get_language()));
        self.settings.set_str("language", code);
        save_settings(&self.settings);
        self.show_toast(&tr!("Language: %s", language_name(code)), 1.8);
    }

    pub fn draw_slider(&mut self, bar: Rect, value: f64, active: bool) {
        let r = SLIDER_KNOB_R;
        let half = bar.h / 2;
        let knob_x = bar.x + r + ((bar.w - 2 * r) as f64 * value) as i32;
        draw::rect(&mut self.canvas, TRACK_BG, bar, 0, half);
        let fill = Rect::new(bar.x, bar.y, knob_x - bar.x, bar.h);
        if fill.w > 0 {
            draw::rect(&mut self.canvas, if active { accent() } else { grey_dim() }, fill, 0, half);
        }
        draw::rect(&mut self.canvas, outline(), bar, 3, half);
        draw::circle(&mut self.canvas, outline(), (knob_x, bar.centery()), r + 2, 0);
        draw::circle(&mut self.canvas, if active { WHITE } else { grey() }, (knob_x, bar.centery()), r - 1, 0);
    }

    fn draw_volume_section(&mut self, bx: Rect, mouse_pos: (f64, f64)) {
        let (inner_x, inner_w) = (bx.x + 12, bx.w - 24);
        let master_on = self.settings.get_bool("sound_on", true);
        let rows: [(&'static str, String, &str, f64, &str, fn(&mut Game)); 3] = [
            ("master", tr("Master volume"), "volume", 0.6, "sound_on", Game::toggle_sound),
            ("music", tr("Music volume"), "music_volume", 0.5, "music_on", Game::toggle_music),
            ("sfx", tr("SFX volume"), "sfx_volume", 1.0, "sfx_on", Game::toggle_sfx),
        ];
        let sb = self.f.small_b.clone();
        let tiny = self.f.tiny.clone();
        for (i, (slider, name, vol_key, default, on_key, toggle)) in rows.into_iter().enumerate() {
            let ry = bx.y + 8 + i as i32 * ROW_H;
            let vol = self.settings.get_f64(vol_key, default).clamp(0.0, 1.0);
            let switch_on = self.settings.get_bool(on_key, true);
            let active = switch_on && (slider == "master" || master_on);
            let mut label = format!("{}: {}%", name, py_round(vol * 100.0) as i64);
            if slider == "master" && !self.mixer_ok {
                label += "   ";
                label += &tr("(no audio on this PC)");
            }
            let pill_text = on_off(switch_on);
            let pill_w = 52.max(tiny.size(&pill_text).0 + 20);
            let label = fit_text(&sb, &label, inner_w - pill_w - 10);
            let t = sb.render(&label, if active { WHITE } else { grey() });
            self.canvas.blit(&t, inner_x, ry + 1);

            let pill = Rect::new(inner_x + inner_w - pill_w, ry - 1, pill_w, 20);
            self.button(
                pill,
                &pill_text,
                &tiny,
                mouse_pos,
                if switch_on { panel_light() } else { OFF_COLOR },
                panel_lighter(),
                WHITE,
                cb(move |g| toggle(g)),
                Bo::r(8).sfx(if slider == "music" { Some("click") } else { None }),
            );

            let bar = Rect::new(inner_x, ry + 26, inner_w, 16);
            self.draw_slider(bar, vol, active);
            let mut hit = Rect::new(bar.x - SLIDER_KNOB_R, bar.y - 6, bar.w + 2 * SLIDER_KNOB_R, bar.h + 8);
            if let Some(c) = self.clip_stack.last() {
                hit = hit.clip(c);
            }
            self.slider_bars.insert(slider, bar);
            if hit.w > 0 && hit.h > 0 {
                self.slider_hits.insert(slider, hit);
            }
        }
    }

    fn draw_sfx_section(&mut self, bx: Rect, mouse_pos: (f64, f64)) {
        let (inner_x, inner_w) = (bx.x + 12, bx.w - 24);
        let col_w = (inner_w - 8) / 2;
        let sb = self.f.small_b.clone();
        for (i, (cat, label)) in SFX_CATEGORIES.iter().enumerate() {
            let (col, row) = (i as i32 % 2, i as i32 / 2);
            let r = Rect::new(inner_x + col * (col_w + 8), bx.y + 8 + row * ROW_H, col_w, 40);
            let on = self.settings.get_bool(&format!("sfx_{}", cat), true);
            let c: &'static str = cat;
            self.button(
                r,
                &format!("{}: {}", tr(label), on_off(on)),
                &sb,
                mouse_pos,
                if on { panel_light() } else { OFF_COLOR },
                panel_lighter(),
                WHITE,
                cb(move |g| g.toggle_sfx_category(c)),
                Bo::r(9).sfx(None),
            );
        }
    }

    fn draw_options_gameplay_tab(&mut self, bx: Rect, mouse_pos: (f64, f64)) {
        let (x0, mut y, w) = (bx.x, bx.y, bx.w);
        let med = self.f.med.clone();
        let sb = self.f.small_b.clone();
        let anim = self.animations();
        self.button(
            Rect::new(x0, y, w, 44),
            &tr!("Animations: %s", on_off(anim)),
            &med,
            mouse_pos,
            if anim { panel_light() } else { OFF_COLOR },
            panel_lighter(),
            WHITE,
            cb(|g| {
                let v = !g.animations();
                g.settings.set_bool("animations", v);
                g.particles.clear();
                save_settings(&g.settings);
                g.show_toast(&if g.animations() { tr("Animations on") } else { tr("Animations off") }, 1.8);
            }),
            Bo::r(10),
        );
        y += 54;

        let trait_notifs = self.settings.get_bool("trait_notifications", true);
        self.button(
            Rect::new(x0, y, w, 44),
            &tr!("Trait notifications: %s", on_off(trait_notifs)),
            &med,
            mouse_pos,
            if trait_notifs { panel_light() } else { OFF_COLOR },
            panel_lighter(),
            WHITE,
            cb(|g| {
                let on = !g.settings.get_bool("trait_notifications", true);
                g.settings.set_bool("trait_notifications", on);
                if !on && g.toast_kind == Some("trait") {
                    g.toast_timer = 0.0;
                }
                save_settings(&g.settings);
            }),
            Bo::r(10),
        );
        y += 54;

        let header = sb.render(&(tr("Cutscenes (catching Secret+)") + ":"), grey());
        self.canvas.blit(&header, x0, y);
        y += 22;

        // with the v2.7+ rarities there are 9: 3 per row
        let cols = if CUTSCENE_RARITIES.len() > 4 { 3 } else { 2 };
        let col_w = (w - 8 * (cols - 1)) / cols;
        for (i, key) in CUTSCENE_RARITIES.iter().enumerate() {
            let (col, row) = (i as i32 % cols, i as i32 / cols);
            let r = Rect::new(x0 + col * (col_w + 8), y + row * ROW_H, col_w, 40);
            let on = self.settings.get_bool(&format!("cutscenes_{}", key), true);
            let rarity_name = tr(RARITY_TIERS[tier_index(key)].name);
            let k: &'static str = key;
            self.button(
                r,
                &format!("{}: {}", rarity_name, on_off(on)),
                &sb,
                mouse_pos,
                if on { panel_light() } else { OFF_COLOR },
                panel_lighter(),
                WHITE,
                cb(move |g| {
                    let setting = format!("cutscenes_{}", k);
                    let on2 = !g.settings.get_bool(&setting, true);
                    g.settings.set_bool(&setting, on2);
                    save_settings(&g.settings);
                    if !on2 {
                        g.cutscene_queue.retain(|q| rarities()[q.0].key != k);
                    }
                    g.show_toast(&tr!("%s cutscenes: %s", tr(RARITY_TIERS[tier_index(k)].name), on_off(on2)), 1.8);
                }),
                Bo::r(9).sfx(None),
            );
        }
    }

    fn draw_options_interface_tab(&mut self, bx: Rect, mouse_pos: (f64, f64)) {
        let (x0, mut y, w) = (bx.x, bx.y, bx.w);
        let med = self.f.med.clone();
        self.button(Rect::new(x0, y, w, 44), &tr!("Fullscreen: %s", on_off(self.fullscreen)), &med, mouse_pos, panel_light(), panel_lighter(), WHITE, cb(|g| g.toggle_fullscreen()), Bo::r(10));
        y += 54;
        self.button(Rect::new(x0, y, w, 44), &tr!("Theme: %s", self.theme_mode_label()), &med, mouse_pos, panel_light(), panel_lighter(), WHITE, cb(|g| g.cycle_theme_mode()), Bo::r(10));
        y += 54;
        self.button(Rect::new(x0, y, w, 44), &tr!("Language: %s", language_name(get_language())), &med, mouse_pos, panel_light(), panel_lighter(), WHITE, cb(|g| g.cycle_language()), Bo::r(10));
    }

    fn draw_options_game_tab(&mut self, bx: Rect, mouse_pos: (f64, f64), in_game: bool) {
        let (x0, mut y, w) = (bx.x, bx.y, bx.w);
        let half = (w - 10) / 2;
        let med = self.f.med.clone();
        self.button(Rect::new(x0, y, w, 44), &tr("Leaderboard"), &med, mouse_pos, panel_light(), panel_lighter(), accent(), cb(|g| g.open_leaderboard()), Bo::r(10).icon("leaderboard"));
        y += 54;
        if self.ev.is_admin {
            self.button(Rect::new(x0, y, w, 44), &tr("Admin commands"), &med, mouse_pos, panel_light(), panel_lighter(), accent(), cb(|g| g.toggle_admin_menu()), Bo::r(10).icon("admin_event"));
            y += 54;
        }
        if in_game {
            self.button(
                Rect::new(x0, y, w, 44),
                &tr("Save now"),
                &med,
                mouse_pos,
                panel_light(),
                panel_lighter(),
                WHITE,
                cb(|g| {
                    g.state.save();
                    g.show_toast(&tr("Progress saved!"), 1.8);
                }),
                Bo::r(10).icon("saves"),
            );
            y += 54;
            self.button(Rect::new(x0, y, half, 44), &tr("Main Menu"), &med, mouse_pos, panel_light(), panel_lighter(), WHITE, cb(|g| g.go_to_menu()), Bo::r(10));
            self.button(Rect::new(x0 + half + 10, y, w - half - 10, 44), &tr("Quit Game"), &med, mouse_pos, BAD, Color::rgb(250, 120, 120), WHITE, cb(|g| g.quit_game()), Bo::r(10));
        }
    }

    pub fn draw_options(&mut self, mouse_pos: (f64, f64)) {
        let ov = dim_overlay(self.vw, VIRTUAL_H, 170);
        self.canvas.blit(&ov, 0, 0);
        self.slider_hits.clear();

        let in_game = self.screen_mode == "game";
        let panel_w = 460;
        let panel_h = 72 + TAB_H + 14 + CONTENT_H + 14 + 60;
        let top = 8.max(VIRTUAL_H / 2 - panel_h / 2);
        let rect = Rect::new(self.vw / 2 - panel_w / 2, top, panel_w, panel_h);
        draw_panel(&mut self.canvas, rect, Some(panel()), 16, true, None);
        self.register_button(rect, Rc::new(|_: &mut Game| {}), None);

        let title = self.f.big.render(&tr("Options"), WHITE);
        self.canvas.blit(&title, rect.x + 24, rect.y + 20);
        let close_rect = Rect::new(rect.right() - 46, rect.y + 20, 28, 28);
        let sb = self.f.small_b.clone();
        self.button(close_rect, "X", &sb, mouse_pos, panel_light(), BAD, WHITE, cb(|g| g.close_options()), Bo::r(8));

        let x0 = rect.x + 24;
        let btn_w = panel_w - 48;
        let tab_y = rect.y + 72;
        let gap = 6;
        let tab_w = (btn_w - gap * (TABS.len() as i32 - 1)) / TABS.len() as i32;
        let mut tx = x0;
        for (key, label) in TABS {
            let active = self.options_tab == key;
            let tab_rect = Rect::new(tx, tab_y, tab_w, TAB_H);
            self.button(
                tab_rect,
                &tr_short(label),
                &sb,
                mouse_pos,
                if active { panel_lighter() } else { panel_light() },
                panel_lighter(),
                WHITE,
                cb(move |g| g.set_options_tab(key)),
                Bo::r(9).border(if active { Some(accent()) } else { None }),
            );
            tx += tab_w + gap;
        }

        let content_top = tab_y + TAB_H + 14;
        let content_box = Rect::new(x0, content_top, btn_w, CONTENT_H);
        match self.options_tab {
            "gameplay" => self.draw_options_gameplay_tab(content_box, mouse_pos),
            "interface" => self.draw_options_interface_tab(content_box, mouse_pos),
            "volume" => self.draw_volume_section(content_box, mouse_pos),
            "sfx" => self.draw_sfx_section(content_box, mouse_pos),
            _ => self.draw_options_game_tab(content_box, mouse_pos, in_game),
        }

        let mut y = content_top + CONTENT_H + 14;
        let note = if in_game {
            tr("Turning animations off removes the particles and the card effect — the game gets much lighter. Progress is saved automatically every 10 seconds.")
        } else {
            tr("Options are shared by all saves.")
        };
        let tiny = self.f.tiny.clone();
        for line in wrap_text(&note, &tiny, btn_w) {
            let t = tiny.render(&line, grey());
            self.canvas.blit(&t, x0, y);
            y += 16;
        }
    }
}
