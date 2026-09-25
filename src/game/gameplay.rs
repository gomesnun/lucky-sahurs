//! Day-to-day game logic: manual / automatic rolls, animations and particles (ui/gameplay.py).

use super::Game;
use super::audio::AUTO_QUIET_RPS;
use crate::core::data::{AUTO_REBIRTH_EVERY, AUTO_TRAIT_EVERY, AUTO_UPGRADE_EVERY, MAX_MANUAL_CPS, TRAIT_POPUP_SECS, TIER_COSMIC, TIER_ETHEREAL, pet_order, rarities};
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
                self.particles.clear(); // the sparkles from before it got this fast
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

    /// Auto Trait Roller (a Traits upgrade): every AUTO_TRAIT_EVERY seconds it spends all your trait charges.
    pub fn update_auto_trait(&mut self, dt: f64) {
        self.auto_trait_timer += dt;
        if self.auto_trait_timer < AUTO_TRAIT_EVERY {
            return;
        }
        self.auto_trait_timer = 0.0;
        if !(self.state.auto_trait_unlocked() && self.state.auto_trait_on) || self.state.trait_charges <= 0 {
            return;
        }
        let before = self.state.owned_traits.clone();
        let n = self.state.trait_charges;
        let results = self.state.roll_traits_bulk(n);
        self.state.last_trait_batch = Some(results);
        self.state.dirty = true;
        // only a trait you didn't have yet is worth a message
        let new: Vec<usize> = self.state.owned_traits.difference(&before).copied().collect();
        if let Some(&best) = new.iter().max() {
            // the traits go from worst to best: a better one than the equipped one is equipped by itself
            let equip = self.state.equipped_trait.is_none_or(|e| best > e);
            if equip {
                self.state.equipped_trait = Some(best);
            }
            self.play("trait_roll", 0.0);
            if self.settings.get_bool("trait_notifications", true) {
                self.trait_popup = Some((best, TRAIT_POPUP_SECS, equip));
            }
        }
    }

    /// v3.0.4, from Prestige II: rebirths by itself as soon as it can (it stops at each Prestige's goal).
    pub fn update_auto_rebirth(&mut self, dt: f64) {
        self.auto_rebirth_timer += dt;
        if self.auto_rebirth_timer < AUTO_REBIRTH_EVERY {
            return;
        }
        self.auto_rebirth_timer = 0.0;
        if !(self.state.auto_rebirth_unlocked() && self.state.auto_rebirth_on) || !self.state.rebirth_available() {
            return;
        }
        if self.state.do_rebirth() {
            self.rebirth_confirm = false;
            self.play("rebirth", 0.0);
            self.show_toast(&crate::tr!("Auto Rebirth: Rebirth #%d!", self.state.rebirths), 1.8);
        }
    }

    pub fn update_animations(&mut self, dt: f64) {
        if let Some(p) = self.trait_popup.as_mut() {
            p.1 -= dt;
            if p.1 <= 0.0 {
                self.trait_popup = None;
            }
        }
        self.update_verity_fx(dt);
        // the top bar's coins count up smoothly (spending shows at once)
        let target = self.state.coins;
        self.coins_display = Some(match self.coins_display {
            Some(d) if self.animations() && target > d && target.is_finite() => {
                let next = d + (target - d) * (dt * 8.0).min(1.0);
                if target - next <= target.abs() * 1e-4 { target } else { next }
            }
            _ => target,
        });
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
        // v3.0 (like Cookie Clicker's clicks): the verity you got pops up from where you clicked
        self.spawn_roll_pop(r_idx, m);
        if gained {
            self.notify_trait_charges(1);
            self.play("trait_charge", 0.0);
        }
        // v3.0.4: the equipped dice can make it a Double Roll (a 2nd roll for free)
        let p = self.state.dice_double_chance();
        if p > 0.0 && crate::core::state::rand_random() < p {
            let (r2, m2, gained2, _) = self.state.roll();
            self.trigger_cutscene(r2, m2);
            self.spawn_roll_particles(r2);
            self.spawn_roll_pop_ex(r2, m2, true);
            self.state.total_double_rolls += 1;
            if gained2 {
                self.notify_trait_charges(1);
            }
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
        // no burst while rolling is "too fast to show": one burst per frame piled up into a cloud over the card
        if !self.animations() || r.tier < 4 || now_ts() < self.too_fast_until {
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
