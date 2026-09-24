//! Day-to-day game logic: manual / automatic rolls, animations and particles (ui/gameplay.py).

use super::Game;
use super::audio::AUTO_QUIET_RPS;
use crate::core::data::{AUTO_UPGRADE_EVERY, MAX_MANUAL_CPS, TIER_COSMIC, TIER_ETHEREAL, pet_order, rarities};
use crate::core::state::now_ts;
use crate::gfx::Color;
use crate::ui::cards::rarity_glow_color;
use crate::ui::widgets::Particle;

/// rolls done one by one per frame; the rest go in bulk
pub const AUTO_MAX_EXACT: i64 = 50;

/// PET_RANK: "best pet" = rarest (the list order isn't the rarity order)
pub fn pet_rank(pet: usize) -> i64 {
    pet_order().iter().position(|&p| p == pet).map(|p| p as i64).unwrap_or(-1)
}

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
        // up to AUTO_MAX_EXACT rolls per frame are done one by one (with the Golden/Diamond/Rainbow Roll bonuses);
        // the rest (e.g. a x1M speed event) at once, by statistics (see roll_bulk)
        let exact = n.min(AUTO_MAX_EXACT);
        self.state.probs_cache = Some(self.state.pet_probs(1.0, None)); // the same for every roll of this frame
        let mut gained_charges = 0;
        let (mut best_r, mut best_m): (i64, &'static str) = (-1, "normal");
        let rank_of = |r: i64| if r < 0 { -1 } else { pet_rank(r as usize) };
        for _ in 0..exact {
            let (r, m, gained, _b) = self.state.roll();
            if pet_rank(r) > rank_of(best_r) {
                best_r = r as i64;
                best_m = m;
            }
            if gained {
                gained_charges += 1;
            }
        }
        self.state.probs_cache = None;
        if n > exact {
            let (best, charges) = self.state.roll_bulk(n - exact);
            gained_charges += charges;
            if let Some((r, m)) = best {
                if pet_rank(r) > rank_of(best_r) {
                    best_r = r as i64;
                    best_m = m;
                }
            }
            if best_r >= 0 {
                self.state.last_roll = Some((best_r as usize, best_m));
            }
            // too fast to show each pet: the middle card says so (see draw_too_fast_card), with the best pet
            // since it got this fast
            let now = now_ts();
            if now > self.too_fast_until {
                self.too_fast_best = None;
            }
            self.too_fast_until = now + 0.6;
            if best_r >= 0 && self.too_fast_best.is_none_or(|b| pet_rank(best_r as usize) > pet_rank(b.0)) {
                self.too_fast_best = Some((best_r as usize, best_m));
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

    /// Auto Upgrader (a Misc upgrade): every AUTO_UPGRADE_EVERY seconds it buys the cheapest upgrades it can pay.
    pub fn update_auto_upgrade(&mut self, dt: f64) {
        self.auto_upgrade_timer += dt;
        if self.auto_upgrade_timer < AUTO_UPGRADE_EVERY {
            return;
        }
        self.auto_upgrade_timer = 0.0;
        if self.state.auto_upgrade_step() > 0 {
            self.play("buy", 1.0);
            if self.state.auto_equip_unlocked() && self.state.auto_equip_best_on {
                self.state.equip_best(); // it may have bought new slots
            }
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
        // the "bright" colour (the dark rarities glow too)
        let color = rarity_glow_color(r.key);
        let n = if r.tier < 7 {
            14
        } else if r.tier < TIER_COSMIC {
            24
        } else if r.tier < TIER_ETHEREAL {
            36
        } else {
            56
        };
        let speed = 1.0 + 0.06 * (r.tier as f64 - 4.0).max(0.0);
        let (cx, cy) = self.main_card_rect().center();
        for k in 0..n {
            let c = if k % 4 != 0 { color } else { Color::rgb(255, 255, 255) };
            self.particles.push(Particle::with_speed(cx as f64, cy as f64, c, speed));
        }
    }
}
