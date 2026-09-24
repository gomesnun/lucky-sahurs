//! Day-to-day game logic: manual / automatic rolls, animations and particles (ui/gameplay.py).

use super::Game;
use super::audio::AUTO_QUIET_RPS;
use crate::core::data::{MAX_MANUAL_CPS, TIER_COSMIC, rarities};
use crate::core::state::now_ts;
use crate::ui::widgets::Particle;

impl Game {
    pub fn update_roll_rate(&mut self, dt: f64) {
        let total = self.state.total_rolls;
        match self.rate_prev {
            Some(p) if total >= p => {
                let inst = (total - p) as f64 / dt.max(1e-3);
                self.rate_prev = Some(total);
                self.roll_rate += (inst - self.roll_rate) * (dt * 5.0).min(1.0);
            }
            _ => self.rate_prev = Some(total),
        }
    }

    pub fn update_auto(&mut self, dt: f64) {
        let rps = self.state.auto_rolls_per_second();
        if rps <= 0.0 || !self.state.auto_on {
            self.auto_accum = 0.0;
            return;
        }
        let quiet = rps > AUTO_QUIET_RPS;
        self.auto_accum += rps * dt;
        let n = self.auto_accum as i64;
        if n <= 0 {
            return;
        }
        self.auto_accum -= n as f64;
        let n = n.min(50);
        let mut gained_charges = 0;
        let (mut best_r, mut best_m): (i64, &'static str) = (-1, "normal");
        for _ in 0..n {
            let (r, m, gained, _b) = self.state.roll();
            if r as i64 > best_r {
                best_r = r as i64;
                best_m = m;
            }
            if gained {
                gained_charges += 1;
            }
        }
        if best_r >= 0 {
            self.trigger_cutscene(best_r as usize, best_m);
        }
        self.roll_anim_start = now_ts();
        if self.animations() {
            if let Some((r, _)) = self.state.last_roll {
                self.spawn_roll_particles(r);
            }
        }
        if best_r >= 0 {
            self.play_roll_sfx(best_r as usize, best_m, false, quiet);
        }
        if gained_charges > 0 {
            self.notify_trait_charges(gained_charges);
            if !quiet {
                self.play("trait_charge", 0.4);
            }
        }
        if self.state.auto_equip_unlocked() && self.state.auto_equip_best_on {
            self.state.equip_best();
        }
    }

    pub fn update_animations(&mut self, dt: f64) {
        if !self.particles.is_empty() {
            for p in self.particles.iter_mut() {
                p.update(dt);
            }
            self.particles.retain(|p| p.life > 0.0);
        }
        if self.toast_timer > 0.0 {
            self.toast_timer -= dt;
        }
    }

    pub fn do_roll(&mut self) {
        let now = crate::core::state::perf_counter();
        if let Some(last) = self.last_manual_roll {
            if now - last < 1.0 / MAX_MANUAL_CPS {
                return;
            }
        }
        self.last_manual_roll = Some(now);
        let st = &self.state;
        let was = (st.cyclic_bonus_ready, st.diamond_bonus_ready, st.rainbow_bonus_ready);
        let (r_idx, m, gained, _b) = self.state.roll();
        self.trigger_cutscene(r_idx, m);
        self.roll_anim_start = now_ts();
        self.spawn_roll_particles(r_idx);
        self.play_roll_sfx(r_idx, m, true, false);
        if gained {
            self.notify_trait_charges(1);
            self.play("trait_charge", 0.0);
        }
        let st = &self.state;
        let now_r = (st.cyclic_bonus_ready, st.diamond_bonus_ready, st.rainbow_bonus_ready);
        if (now_r.0 && !was.0) || (now_r.1 && !was.1) || (now_r.2 && !was.2) {
            self.play("cyclic", 0.0);
        }
        if self.state.auto_equip_unlocked() && self.state.auto_equip_best_on {
            self.state.equip_best();
        }
    }

    pub fn spawn_roll_particles(&mut self, rarity_index: usize) {
        let r = &rarities()[rarity_index];
        if !self.animations() || r.tier < 4 {
            return;
        }
        let n = if r.tier < 7 {
            14
        } else if r.tier < TIER_COSMIC {
            24
        } else {
            36
        };
        let (cx, cy) = self.main_card_rect().center();
        for _ in 0..n {
            self.particles.push(Particle::new(cx as f64, cy as f64, r.color));
        }
    }
}
