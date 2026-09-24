//! Sound: generated effects, sound files, music and volume (ui/audio.py).

use super::Game;
use crate::audio::make_tone;
use crate::core::data::{TIER_SECRET, rarities};
use crate::gfx::Rect;
use crate::storage::save_settings;

pub const SLIDER_KNOB_R: i32 = 10;
pub const AUTO_QUIET_RPS: f64 = 3.0;
pub const AUTO_QUIET_MIN_TIER: usize = TIER_SECRET;
const MUSIC_SCALE: f64 = 0.60;

const SFX_FILES: [(&str, &str, f64); 4] = [
    ("click", "click.ogg", 0.75),
    ("buy", "upgrade.ogg", 0.55),
    ("milestone", "achievement.ogg", 0.50),
    ("rebirth", "rebirth.ogg", 0.90),
];

fn sfx_category_of(name: &str) -> Option<&'static str> {
    Some(match name {
        "click" | "equip" => "click",
        "roll" | "cyclic" | "mut_golden" | "mut_diamond" | "mut_rainbow" => "roll",
        "trait_charge" | "trait_roll" => "traits",
        "buy" => "upgrades",
        "milestone" => "milestones",
        "rebirth" => "rebirth",
        n if n.starts_with("rar_") => match n[4..].parse::<i32>() {
            Ok(t) if (2..16).contains(&t) => "roll",
            _ => return None,
        },
        _ => return None,
    })
}

fn sfx_preview(cat: &str) -> &'static str {
    match cat {
        "click" => "click",
        "roll" => "roll",
        "traits" => "trait_charge",
        "upgrades" => "buy",
        "milestones" => "milestone",
        "rebirth" => "rebirth",
        _ => "click",
    }
}

pub fn slider_setting(slider: &str) -> &'static str {
    match slider {
        "master" => "volume",
        "music" => "music_volume",
        _ => "sfx_volume",
    }
}

impl Game {
    fn level(&self, key: &str, default: f64) -> f64 {
        match self.settings.get(key).and_then(crate::storage::value_f64) {
            Some(v) => v.clamp(0.0, 1.0),
            None => default,
        }
    }

    pub fn build_sounds(&mut self) {
        self.sounds.clear();
        self.sfx_gain.clear();
        let Some(mixer) = self.mixer.as_ref() else { return };
        let tone = |f: f64, d: f64, v: f64, f2: Option<f64>| make_tone(f, d, v, f2);
        let seq = |notes: &[f64], d: f64, v: f64| -> Vec<i16> { notes.iter().flat_map(|n| make_tone(*n, d, v, None)).collect() };
        let raw: Vec<(&str, Vec<i16>)> = vec![
            ("click", tone(720.0, 0.04, 0.22, None)),
            ("equip", tone(520.0, 0.05, 0.24, Some(700.0))),
            ("roll", tone(300.0, 0.09, 0.26, Some(520.0))),
            ("buy", seq(&[660.0, 880.0], 0.07, 0.30)),
            ("milestone", seq(&[523.0, 659.0, 784.0, 1047.0], 0.10, 0.30)),
            ("rebirth", seq(&[392.0, 523.0, 659.0, 784.0, 1047.0], 0.09, 0.30)),
            ("trait_charge", seq(&[880.0, 1175.0], 0.07, 0.30)),
            ("trait_roll", tone(400.0, 0.16, 0.28, Some(900.0))),
            ("cyclic", seq(&[784.0, 988.0, 1175.0], 0.06, 0.26)),
            ("mut_golden", seq(&[880.0, 1109.0, 1319.0], 0.06, 0.30)),
            ("mut_diamond", seq(&[1047.0, 1319.0, 1568.0, 2093.0], 0.06, 0.30)),
            ("mut_rainbow", seq(&[1047.0, 1319.0, 1568.0, 2093.0, 2637.0], 0.06, 0.30)),
            ("rar_2", seq(&[523.0, 659.0], 0.07, 0.30)),
            ("rar_3", seq(&[523.0, 659.0, 784.0], 0.08, 0.30)),
            ("rar_4", seq(&[523.0, 659.0, 784.0, 1047.0], 0.08, 0.30)),
            ("rar_5", seq(&[440.0, 554.0, 659.0, 880.0, 1108.0], 0.08, 0.30)),
            ("rar_6", seq(&[523.0, 659.0, 784.0, 1047.0, 1319.0], 0.08, 0.30)),
            ("rar_7", seq(&[392.0, 494.0, 587.0, 784.0, 988.0, 1175.0], 0.09, 0.30)),
            ("rar_8", seq(&[523.0, 659.0, 784.0, 1047.0, 1319.0, 1568.0, 2093.0], 0.10, 0.30)),
            ("rar_9", seq(&[392.0, 523.0, 659.0, 784.0, 1047.0, 1319.0, 1568.0, 2093.0], 0.10, 0.30)),
            ("rar_10", seq(&[523.0, 659.0, 784.0, 1047.0, 1319.0, 1568.0, 2093.0, 2637.0, 3136.0], 0.11, 0.30)),
            ("rar_11", seq(&[587.0, 740.0, 880.0, 1175.0, 1480.0, 1760.0, 2349.0, 2960.0, 3520.0], 0.11, 0.30)),
            ("rar_12", seq(&[659.0, 831.0, 988.0, 1319.0, 1661.0, 1976.0, 2637.0, 3322.0, 3951.0], 0.12, 0.30)),
            ("rar_13", seq(&[392.0, 523.0, 659.0, 784.0, 1047.0, 1319.0, 1568.0, 2093.0, 2637.0, 3136.0, 4186.0], 0.12, 0.30)),
            ("rar_14", seq(&[330.0, 440.0, 554.0, 659.0, 880.0, 1109.0, 1319.0, 1760.0, 2217.0, 2637.0, 3520.0, 4435.0], 0.12, 0.30)),
            ("rar_15", seq(&[262.0, 392.0, 523.0, 784.0, 1047.0, 1568.0, 2093.0, 3136.0, 4186.0, 3136.0, 4186.0, 5274.0], 0.12, 0.30)),
        ];
        for (name, data) in raw {
            self.sounds.insert(name.to_string(), mixer.chunk_from_samples(data));
        }
        for (name, file, gain) in SFX_FILES {
            if let Some(data) = crate::assets::read(&format!("sounds/{}", file)) {
                if let Some(ch) = mixer.load_chunk(&data) {
                    self.sounds.insert(name.to_string(), ch);
                    self.sfx_gain.insert(name.to_string(), gain);
                }
            }
        }
    }

    pub fn start_music(&mut self) {
        self.music_ok = false;
        let Some(mixer) = self.mixer.as_ref() else { return };
        let Some(data) = crate::assets::read("sounds/music.ogg") else { return };
        if !mixer.play_music_loop(&data) {
            return;
        }
        self.music_ok = true;
        self.apply_music_volume();
    }

    pub fn apply_music_volume(&mut self) {
        if !self.music_ok {
            return;
        }
        let on = self.settings.get_bool("sound_on", true) && self.settings.get_bool("music_on", true);
        let vol = self.level("volume", 0.6);
        let mus = self.level("music_volume", 0.5);
        if let Some(m) = self.mixer.as_ref() {
            m.set_music_volume(if on { vol * mus * MUSIC_SCALE } else { 0.0 });
        }
    }

    pub fn toggle_music(&mut self) {
        let v = !self.settings.get_bool("music_on", true);
        self.settings.set_bool("music_on", v);
        save_settings(&self.settings);
        self.apply_music_volume();
    }

    pub fn apply_volume(&mut self) {
        let vol = self.level("volume", 0.6) * self.level("sfx_volume", 1.0);
        for (name, snd) in &self.sounds {
            snd.set_volume((vol * self.sfx_gain.get(name).copied().unwrap_or(1.0)).min(1.0));
        }
        self.apply_music_volume();
    }

    pub fn play(&mut self, name: &str, min_gap: f64) {
        let cfg = &self.settings;
        if !cfg.get_bool("sound_on", true) || !cfg.get_bool("sfx_on", true) {
            return;
        }
        if cfg.get_f64("volume", 0.0) <= 0.0 || cfg.get_f64("sfx_volume", 1.0) <= 0.0 {
            return;
        }
        if let Some(cat) = sfx_category_of(name) {
            if !cfg.get_bool(&format!("sfx_{}", cat), true) {
                return;
            }
        }
        let Some(snd) = self.sounds.get(name).cloned() else { return };
        let now = crate::core::state::now_ts();
        if min_gap > 0.0 && now - self.last_sfx.get(name).copied().unwrap_or(0.0) < min_gap {
            return;
        }
        self.last_sfx.insert(name.to_string(), now);
        if let Some(m) = self.mixer.as_ref() {
            m.play(&snd);
        }
    }

    pub fn play_roll_sfx(&mut self, r_idx: usize, mutation: &str, manual: bool, quiet: bool) {
        let tier = rarities()[r_idx].tier;
        if !manual && tier < 3 && mutation != "diamond" && mutation != "rainbow" {
            return;
        }
        if !manual && quiet && tier < AUTO_QUIET_MIN_TIER {
            return;
        }
        let name = if tier >= 2 {
            format!("rar_{}", tier)
        } else if mutation != "normal" {
            format!("mut_{}", mutation)
        } else {
            "roll".to_string()
        };
        self.play(&name, if manual { 0.0 } else { 0.35 });
    }

    pub fn toggle_sound(&mut self) {
        let v = !self.settings.get_bool("sound_on", true);
        self.settings.set_bool("sound_on", v);
        save_settings(&self.settings);
        self.apply_music_volume();
        if v {
            self.play("click", 0.0);
        }
    }

    pub fn toggle_sfx(&mut self) {
        let v = !self.settings.get_bool("sfx_on", true);
        self.settings.set_bool("sfx_on", v);
        save_settings(&self.settings);
        if v {
            self.play("click", 0.0);
        }
    }

    pub fn toggle_sfx_category(&mut self, category: &str) {
        let key = format!("sfx_{}", category);
        let v = !self.settings.get_bool(&key, true);
        self.settings.set_bool(&key, v);
        save_settings(&self.settings);
        if v {
            self.play(sfx_preview(category), 0.0);
        }
    }

    pub fn set_slider_from_pos(&mut self, slider: &str, pos: (f64, f64)) {
        let bar: Option<Rect> = self.slider_bars.get(slider).copied();
        let span = bar.map(|b| b.w - 2 * SLIDER_KNOB_R).unwrap_or(0);
        if span <= 0 {
            return;
        }
        let bar = bar.unwrap();
        let frac = (pos.0 - (bar.x + SLIDER_KNOB_R) as f64) / span as f64;
        let v = crate::core::formatting::py_round(frac.clamp(0.0, 1.0) * 100.0) / 100.0;
        // round(x, 2) - rounding to two decimals (matches Python for the values a slider produces)
        let v = py_round2(frac.clamp(0.0, 1.0)).unwrap_or(v);
        self.settings.set_f64(slider_setting(slider), v);
        self.apply_volume();
    }
}

/// Python round(x, 2): correctly rounded via the shortest repr.
pub fn py_round2(x: f64) -> Option<f64> {
    let s = format!("{:.2}", x);
    // Rust's {:.2} rounds the exact binary value half-to-even like CPython's round(x, 2)
    s.parse().ok()
}
