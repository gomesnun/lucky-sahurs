//! Upgrade tree panel (ui/upgrades_panel.py).

use super::base::Bo;
use super::{Game, cb};
use crate::core::data::*;
use crate::core::formatting::format_number;
use crate::gfx::{Color, Rect, draw, ti};
use crate::i18n::tr;
use crate::storage::save_settings;
use crate::theme::*;
use crate::tr;
use crate::ui::drawing::{draw_panel, draw_state_border};
use crate::ui::fonts::{fit_text, wrap_text};
use crate::ui::icons::load_icon;
use serde_json::{Value, json};
use std::rc::Rc;

/// Buy mode as the state wants it: 1, 10 or 0 (= "max").
fn mode_of(v: Option<&Value>) -> i64 {
    match v {
        Some(Value::String(s)) if s == "max" => 0,
        Some(Value::Bool(true)) => 1,
        Some(Value::Number(n)) => match n.as_f64() {
            Some(x) if x == 1.0 => 1,
            Some(x) if x == 10.0 => 10,
            _ => 1,
        },
        _ => 1,
    }
}

/// v3.0.1: the icon of each upgrade (so they're easier to tell apart).
pub fn upgrade_icon(key: &str) -> &'static str {
    match key {
        k if k.starts_with("luck") => "shop/potion_luck",
        k if k.starts_with("money") => "cash",
        "slots" | "slots_plus" | "auto_equip_unlock" => "bag",
        k if k.starts_with("auto_") && k != "auto_upgrade_unlock" && k != "auto_trait_unlock" => "shop/potion_speed",
        "golden_unlock" | "golden_chance" | "golden_chance_2" => "shop/dice_gold",
        "diamond_unlock" | "diamond_chance" | "diamond_chance_2" => "shop/dice_sapphire",
        "rainbow_unlock" | "rainbow_chance" | "rainbow_chance_2" => "shop/dice_prism",
        "golden_roll_unlock" | "cyclic_every" | "cyclic_power" => "shop/dice_gold",
        k if k.starts_with("diamond_roll") => "shop/dice_sapphire",
        k if k.starts_with("rainbow_roll") => "shop/dice_prism",
        k if k.starts_with("trait") || k == "auto_trait_unlock" => "trait",
        k if k.starts_with("offline") => "clock",
        _ => "tree",
    }
}

fn category_icon(key: &str) -> &'static str {
    match key {
        "luck" => "shop/potion_luck",
        "mutations" => "shop/dice_gold",
        "money" => "cash",
        "traits" => "trait",
        "bonus_rolls" => "shop/dice_prism",
        "auto" => "shop/potion_speed",
        "offline" => "clock",
        _ => "tree",
    }
}

impl Game {
    pub fn upgrade_effect_text(&self, key: &str) -> String {
        let s = &self.state;
        let lvl = s.upgrade_level(key) as f64;
        match key {
            "luck" => tr!("Now: x%.2f weight on Rare+", 1.0 + LUCK_PER_LEVEL * lvl),
            "luck_prism" => {
                let b = 1.0 + LUCK_PRISM_PER_LEVEL * lvl;
                tr!("Now: x%.2f Epic+ / x%.2f Mythic+ / x%.2f Secret+", b, b.powi(2), b.powi(3))
            }
            "luck_cosmic" => tr!("Now: x%.2f weight on Exotic+", 1.0 + LUCK_COSMIC_PER_LEVEL * lvl),
            "luck_divine" => tr!("Now: x%.2f weight on Divine+", 1.0 + LUCK_DIVINE_PER_LEVEL * lvl),
            "money" => tr!("Now: x%.1f money (total)", s.money_multiplier()),
            "money_prism" => tr!("Now: x%.2f money (from this upgrade only)", 1.0 + MONEY_PRISM_PER_LEVEL * lvl),
            "money_ultra" => tr!("Now: x%.2f money (from this upgrade only)", 1.0 + MONEY_ULTRA_PER_LEVEL * lvl),
            "luck_2" => tr!("Now: x%.2f weight on Rare+", 1.0 + LUCK_2_PER_LEVEL * lvl),
            "luck_ultra" => tr!("Now: x%.2f weight on Rare+", 1.0 + LUCK_ULTRA_PER_LEVEL * lvl),
            "luck_prism_2" => {
                let b = 1.0 + LUCK_PRISM_2_PER_LEVEL * lvl;
                tr!("Now: x%.2f Divine+ ... x%.2f Ethereal+", b, b.powi(4))
            }
            k if k.starts_with("luck_tier_") => {
                let tier_key = &k["luck_tier_".len()..];
                let per = LUCK_TIER_PER_LEVEL.iter().find(|(t, _)| *t == tier_key).map(|(_, v)| *v).unwrap_or(0.0);
                tr!("Now: x%.2f weight on %s+", 1.0 + per * lvl, tr(RARITY_TIERS[tier_index(tier_key)].name))
            }
            "rainbow_chance" | "rainbow_unlock" | "rainbow_chance_2" => tr!("Now: %.2f%% Rainbow (max %.1f%%)", s.mutation_chances().2 * 100.0, RAINBOW_MAX_CHANCE * 100.0),
            "slots" | "slots_plus" => tr!("Now: %d slots", s.max_slots()),
            "auto_speed" | "auto_unlock" | "auto_turbo" => tr!("Now: %.2f rolls/sec", s.auto_rolls_per_second()),
            "golden_chance" | "golden_unlock" | "golden_chance_2" => tr!("Now: %.1f%% Golden (max %.0f%%)", s.mutation_chances().0 * 100.0, GOLDEN_MAX_CHANCE * 100.0),
            "diamond_chance" | "diamond_unlock" | "diamond_chance_2" => tr!("Now: %.2f%% Diamond (max %.0f%%)", s.mutation_chances().1 * 100.0, DIAMOND_MAX_CHANCE * 100.0),
            "trait_charge_luck" | "trait_charge_luck_2" => tr!("Now: %.3f%% charge chance per roll", s.trait_charge_chance() * 100.0),
            "trait_rarity_luck" => tr!("Now: x%.2f weight on traits from Instinctive onward", 1.0 + 0.09 * lvl),
            "trait_rarity_luck_2" => tr!("Now: x%.2f weight on traits from Instinctive onward", 1.0 + 0.15 * lvl),
            "cyclic_every" => tr!("Now: Golden Roll every %d rolls", s.golden_roll_every()),
            "cyclic_power" => tr!("Now: Golden Roll x%g", s.golden_roll_mult()),
            "diamond_roll_unlock" | "diamond_roll_every" => tr!("Now: Diamond Roll every %d rolls", s.diamond_roll_every()),
            "diamond_roll_power" => tr!("Now: Diamond Roll x%g", s.diamond_roll_mult()),
            "rainbow_roll_unlock" | "rainbow_roll_every" => tr!("Now: Rainbow Roll every %d rolls", s.rainbow_roll_every()),
            "rainbow_roll_power" => tr!("Now: Rainbow Roll x%g", s.rainbow_roll_mult()),
            "offline_rate" | "offline_rate_2" => tr!("Now: %.0f%% offline earnings", s.offline_earn_rate() * 100.0),
            "offline_time" | "offline_time_2" => tr!("Now: up to %g hours offline", s.offline_max_seconds() / 3600.0),
            "auto_equip_unlock" => {
                if lvl < 1.0 {
                    tr("Now: locked")
                } else {
                    tr!("Now: %s  (toggle it in the Bag)", if s.auto_equip_best_on { tr("ON") } else { tr("OFF") })
                }
            }
            _ => String::new(),
        }
    }

    fn tree_category_stats(&self, cat: &UpgradeCategory) -> (i64, i64, i64) {
        let (mut lvls, mut mx, mut ready) = (0, 0, 0);
        for key in cat.upgrades {
            lvls += self.state.upgrade_level(key);
            mx += upgrade_def(key).max_level;
            if self.state.upgrade_affordable(key) {
                ready += 1;
            }
        }
        (lvls, mx, ready)
    }

    pub fn select_tree_category(&mut self, cat_key: Option<&'static str>) {
        self.tree_selected_category = cat_key;
        self.right_panel.scroll.insert(Some("tree"), 0.0);
    }

    pub fn buy_mode(&self) -> i64 {
        mode_of(self.settings.get("buy_mode"))
    }

    pub fn set_buy_mode(&mut self, mode: i64) {
        self.settings.set_value("buy_mode", if mode == 0 { json!("max") } else { json!(mode) });
        save_settings(&self.settings);
    }

    pub fn draw_tree_panel(&mut self, rect: Rect, mouse_pos: (f64, f64)) {
        self.panel_header(rect, &tr("Upgrades"), mouse_pos, Rc::new(|g: &mut Game| g.right_panel.close()));
        let mut top = rect.y + 62;
        let cat = self.tree_selected_category.and_then(upgrade_category);
        let sb = self.f.small_b.clone();
        if let Some(cat) = cat {
            let back_rect = Rect::new(rect.x + 20, top, 128, 32);
            self.button(back_rect, &tr("<  Categories"), &sb, mouse_pos, panel_light(), panel(), WHITE, cb(|g| g.select_tree_category(None)), Bo::r(8));
            let (mode_w, mode_gap) = (48, 6);
            let mx0 = rect.right() - 26 - (mode_w * 3 + mode_gap * 2);
            let modes = [(1i64, "x1".to_string()), (10, "x10".to_string()), (0, tr("Max"))];
            for (n, (mode, label)) in modes.into_iter().enumerate() {
                let mrect = Rect::new(mx0 + n as i32 * (mode_w + mode_gap), top, mode_w, 32);
                let active = self.buy_mode() == mode;
                self.button(
                    mrect,
                    &label,
                    &sb,
                    mouse_pos,
                    if active { accent() } else { panel_light() },
                    if !active { accent_hover() } else { accent() },
                    if active { BLACK } else { WHITE },
                    cb(move |g| g.set_buy_mode(mode)),
                    Bo::r(8),
                );
            }
            let (lvls, mx, _) = self.tree_category_stats(cat);
            let cat_txt = self.f.med.render(&format!("{}  ({}/{})", tr(cat.label), lvls, mx), accent());
            self.canvas.blit(&cat_txt, rect.x + 20, top + 42);
            top += 84;
        } else if self.state.auto_upgrade_unlocked() {
            // the Auto Upgrader switch (only shows after buying it in Misc)
            let on = self.state.auto_upgrade_on;
            self.button(
                Rect::new(rect.x + 20, top, rect.w - 46, 36),
                &tr!("Auto Upgrader: %s", if on { tr("ON") } else { tr("OFF") }),
                &sb,
                mouse_pos,
                if on { Color::rgb(52, 120, 80) } else { panel_light() },
                panel_lighter(),
                WHITE,
                cb(|g| g.state.auto_upgrade_on = !g.state.auto_upgrade_on),
                Bo::r(9),
            );
            top += 46;
        }
        let content_rect = Rect::new(rect.x, top, rect.w, rect.bottom() - top);
        let scroll = self.right_panel.get_scroll();
        self.push_clip(content_rect);
        let content_h = match cat {
            None => self.draw_tree_category_list(content_rect, scroll, mouse_pos),
            Some(cat) => self.draw_tree_upgrade_list(content_rect, scroll, mouse_pos, cat.upgrades),
        };
        self.pop_clip();
        self.right_panel.set_max_scroll((content_h - content_rect.h as f64).max(0.0));
        self.draw_scrollbar(content_rect, scroll, content_h, Some("right"), Some(mouse_pos));
    }

    fn draw_tree_category_list(&mut self, content_rect: Rect, scroll: f64, mouse_pos: (f64, f64)) -> f64 {
        let pad = 20;
        let row_w = content_rect.w - pad * 2 - 6;
        let row_h = 78;
        let mut y = content_rect.top() as f64 + 6.0 - scroll;
        let med = self.f.med.clone();
        let sb = self.f.small_b.clone();
        let tiny = self.f.tiny.clone();
        for cat in UPGRADE_CATEGORIES.iter() {
            let (lvls, mx, ready) = self.tree_category_stats(cat);
            let done = lvls >= mx;
            let row_rect = Rect::new(content_rect.x + pad, ti(y), row_w, row_h);
            if row_rect.bottom() > content_rect.top() && row_rect.top() < content_rect.bottom() {
                draw_panel(&mut self.canvas, row_rect, Some(panel_light()), 12, false, None);
                if done {
                    draw_state_border(&mut self.canvas, row_rect, Color::rgb(80, 150, 100), 12, 2);
                } else if row_rect.collidepoint(mouse_pos) && content_rect.collidepoint(mouse_pos) {
                    draw_state_border(&mut self.canvas, row_rect, accent(), 12, 2);
                }
                if let Some(icon) = load_icon(category_icon(cat.key), 40) {
                    self.canvas.blit(&icon, row_rect.x + 12, row_rect.y + 8);
                }
                let tx = row_rect.x + 62;
                let name_txt = med.render(&tr(cat.label), WHITE);
                self.canvas.blit(&name_txt, tx, row_rect.y + 10);
                let count_txt = sb.render(&format!("{}/{}", lvls, mx), if done { GOOD } else { WHITE });
                self.canvas.blit(&count_txt, row_rect.right() - count_txt.w - 40, row_rect.y + 12);
                let arrow_txt = med.render(">", grey());
                self.canvas.blit(&arrow_txt, row_rect.right() - 26, row_rect.centery() - 12);

                let ready_txt = if ready > 0 { Some(tiny.render(&tr!("%d ready to buy", ready), GOOD)) } else { None };
                let max_desc_w = row_rect.right() - 16 - tx - ready_txt.as_ref().map(|t| t.w + 38).unwrap_or(0);
                let desc = fit_text(&tiny, &tr(cat.desc), max_desc_w);
                let d = tiny.render(&desc, grey());
                self.canvas.blit(&d, tx, row_rect.y + 36);
                if let Some(rt) = &ready_txt {
                    self.canvas.blit(rt, row_rect.right() - rt.w - 40, row_rect.y + 36);
                }

                let bar_rect = Rect::new(row_rect.x + 16, row_rect.bottom() - 20, row_rect.w - 32, 10);
                draw::rect(&mut self.canvas, panel(), bar_rect, 0, 5);
                let frac = if mx != 0 { lvls as f64 / mx as f64 } else { 0.0 };
                let fill_w = 0.max((bar_rect.w as f64 * frac) as i32);
                if fill_w > 0 {
                    draw::rect(&mut self.canvas, if done { GOOD } else { accent() }, Rect::new(bar_rect.x, bar_rect.y, fill_w, bar_rect.h), 0, 5);
                }
                let k = cat.key;
                self.register_button(row_rect, Rc::new(move |g: &mut Game| g.select_tree_category(Some(k))), Some("click"));
            }
            y += (row_h + 14) as f64;
        }
        (y + scroll) - (content_rect.top() + 6) as f64
    }

    fn draw_tree_upgrade_list(&mut self, content_rect: Rect, scroll: f64, mouse_pos: (f64, f64), keys: &'static [&'static str]) -> f64 {
        let pad = 20;
        let row_w = content_rect.w - pad * 2 - 6;
        let mut y = content_rect.top() as f64 + 6.0 - scroll;
        let med = self.f.med.clone();
        let sb = self.f.small_b.clone();
        let small = self.f.small.clone();
        let tiny = self.f.tiny.clone();
        for &key in keys {
            let d = upgrade_def(key);
            let lvl = self.state.upgrade_level(key);
            let maxed = lvl >= d.max_level;
            let locked_by = self.state.upgrade_locked_by(key);
            let need_rebirths = if self.state.upgrade_rebirths_needed(key) != 0 { d.requires_rebirths } else { 0 };

            let desc_lines = wrap_text(&d.desc.tr(), &tiny, row_w - 28);
            let effect = self.upgrade_effect_text(key);
            let row_h = 34 + desc_lines.len() as i32 * 16 + if effect.is_empty() { 0 } else { 18 } + 44;
            let row_rect = Rect::new(content_rect.x + pad, ti(y), row_w, row_h);

            if row_rect.bottom() > content_rect.top() && row_rect.top() < content_rect.bottom() {
                draw_panel(&mut self.canvas, row_rect, Some(panel_light()), 10, false, None);
                if maxed {
                    draw_state_border(&mut self.canvas, row_rect, Color::rgb(80, 150, 100), 10, 2);
                }
                let dim = locked_by.is_some() || need_rebirths != 0;
                let mut nx = row_rect.x + 14;
                if let Some(icon) = load_icon(upgrade_icon(key), 26) {
                    self.canvas.blit_with_alpha(&icon, nx, row_rect.y + 6, if dim { 120 } else { 255 });
                    nx += 32;
                }
                let name_txt = med.render(&fit_text(&med, &tr(d.name), row_rect.right() - 90 - nx), if !dim { WHITE } else { grey() });
                self.canvas.blit(&name_txt, nx, row_rect.y + 9);
                let lvl_txt = small.render(&tr!("Lv %d/%d", lvl, d.max_level), if maxed { GOOD } else { grey() });
                self.canvas.blit(&lvl_txt, row_rect.right() - lvl_txt.w - 14, row_rect.y + 13);

                let mut ly = row_rect.y + 34;
                for line in &desc_lines {
                    let t = tiny.render(line, grey());
                    self.canvas.blit(&t, row_rect.x + 14, ly);
                    ly += 16;
                }
                if !effect.is_empty() {
                    let e = fit_text(&tiny, &effect, row_rect.w - 28);
                    let t = tiny.render(&e, accent());
                    self.canvas.blit(&t, row_rect.x + 14, ly + 2);
                }

                let btn_rect = Rect::new(row_rect.x + 14, row_rect.bottom() - 38, row_rect.w - 28, 30);
                if maxed {
                    self.button(btn_rect, &tr("MAX"), &sb, mouse_pos, Color::rgb(58, 78, 64), Color::rgb(58, 78, 64), GOOD, None, Bo::r(8).enabled(false));
                } else if need_rebirths != 0 {
                    self.button(btn_rect, &tr!("Requires Rebirth %d", need_rebirths), &small, mouse_pos, Color::rgb(60, 62, 72), Color::rgb(60, 62, 72), grey(), None, Bo::r(8).enabled(false));
                } else if let Some(lb) = locked_by {
                    let need = d.requires_level;
                    let req_name = tr(upgrade_def(lb).name);
                    let label = if need <= 1 { tr!("Requires %s", req_name) } else { tr!("Requires %s Lv %d", req_name, need) };
                    self.button(btn_rect, &label, &small, mouse_pos, Color::rgb(60, 62, 72), Color::rgb(60, 62, 72), grey(), None, Bo::r(8).enabled(false));
                } else {
                    let mode = self.buy_mode();
                    let (n_levels, cost, affordable) = self.state.upgrade_bulk_quote(key, mode);
                    let cost_s = format_number(cost as f64);
                    let label = match mode {
                        1 => tr!("Buy  ($%s)", cost_s),
                        0 => {
                            if affordable {
                                tr!("Buy Max  x%d  ($%s)", n_levels, cost_s)
                            } else {
                                tr!("Buy Max  ($%s)", cost_s)
                            }
                        }
                        _ => tr!("Buy x%d  ($%s)", n_levels, cost_s),
                    };
                    self.button(
                        btn_rect,
                        &label,
                        &sb,
                        mouse_pos,
                        if affordable { accent() } else { Color::rgb(70, 73, 88) },
                        accent_hover(),
                        if affordable { BLACK } else { grey() },
                        if affordable {
                            cb(move |g| {
                                let m = g.buy_mode();
                                let got = g.state.buy_upgrade_bulk(key, m);
                                if got != 0 {
                                    g.play("buy", 0.08);
                                    let name = tr(upgrade_def(key).name);
                                    g.show_toast(&if got == 1 { tr!("%s upgraded!", name) } else { tr!("%s upgraded!  +%d levels", name, got) }, 1.8);
                                }
                            })
                        } else {
                            None
                        },
                        Bo::r(8).enabled(affordable).sfx(None),
                    );
                }
            }
            y += (row_h + 12) as f64;
        }
        (y + scroll) - (content_rect.top() + 6) as f64 + 10.0
    }
}
