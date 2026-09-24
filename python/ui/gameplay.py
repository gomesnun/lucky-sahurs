"""Lógica do dia-a-dia do jogo: roll manual / automático, animações e partículas."""

import time

from core import balance as B
from core.pets import RARITIES, TIER_COSMIC
from ui.audio import AUTO_QUIET_RPS
from ui.widgets import Particle


class GameplayMixin:
    """Lógica do dia-a-dia do jogo: roll manual / automático, animações e partículas."""

    def update_roll_rate(self, dt):
        """Rolls por segundo REAIS (auto + cliques), suavizados. Serve para as barras
        Golden/Diamond/Rainbow ficarem cheias quando o ciclo passa mais depressa do que se vê."""
        total = self.state.total_rolls
        if self._rate_prev is None or total < self._rate_prev:
            self._rate_prev = total
            return
        inst = (total - self._rate_prev) / max(dt, 1e-3)
        self._rate_prev = total
        self.roll_rate += (inst - self.roll_rate) * min(1.0, dt * 5.0)

    def update_auto(self, dt):
        rps = self.state.auto_rolls_per_second()
        if rps <= 0 or not self.state.auto_on:
            self.auto_accum = 0.0
            return
        quiet = rps > AUTO_QUIET_RPS        # rápido demais para ter um som por roll (ver ui/audio.py)
        self.auto_accum += rps * dt
        n = int(self.auto_accum)
        if n <= 0:
            return
        self.auto_accum -= n
        n = min(n, 50)
        gained_charges = 0
        best_r, best_m = -1, "normal"
        for _ in range(n):
            r, m, gained, _bonus = self.state.roll()
            if r > best_r:
                best_r, best_m = r, m
            if gained:
                gained_charges += 1
        if best_r >= 0:
            self.trigger_cutscene(best_r, best_m)      # não faz nada abaixo de Secret (ver ui/cutscene_panel.py)
        self.roll_anim_start = time.time()
        if self.animations and self.state.last_roll:
            self.spawn_roll_particles(self.state.last_roll[0])
        self.play_roll_sfx(best_r, best_m, manual=False, quiet=quiet)
        if gained_charges:
            self.notify_trait_charges(gained_charges)
            if not quiet:
                self.play("trait_charge", min_gap=0.4)
        if self.state.auto_equip_unlocked() and self.state.auto_equip_best_on:
            self.state.equip_best()

    def update_animations(self, dt):
        if self.particles:
            for p in self.particles:
                p.update(dt)
            self.particles = [p for p in self.particles if p.life > 0]
        if self.toast_timer > 0:
            self.toast_timer -= dt

    def do_roll(self):
        # ANTI AUTO-CLICKER: no maximo B.MAX_MANUAL_CPS rolls por clique / segundo (20 => 1 a cada 0.05 s).
        # Cliques a mais (de um auto-clicker) sao simplesmente ignorados. O Auto Roller nao passa por aqui.
        now = time.perf_counter()
        if now - getattr(self, "_last_manual_roll", -1.0) < 1.0 / B.MAX_MANUAL_CPS:
            return
        self._last_manual_roll = now
        was_ready = (self.state.cyclic_bonus_ready, self.state.diamond_bonus_ready,
                    self.state.rainbow_bonus_ready)
        r_idx, mut, gained_charge, _used_bonus = self.state.roll()
        self.trigger_cutscene(r_idx, mut)      # não faz nada abaixo de Secret (ver ui/cutscene_panel.py)
        self.roll_anim_start = time.time()
        self.spawn_roll_particles(r_idx)
        self.play_roll_sfx(r_idx, mut, manual=True)
        if gained_charge:
            self.notify_trait_charges(1)
            self.play("trait_charge")
        now_ready = (self.state.cyclic_bonus_ready, self.state.diamond_bonus_ready,
                    self.state.rainbow_bonus_ready)
        if any(n and not w for w, n in zip(was_ready, now_ready)):
            self.play("cyclic")
        if self.state.auto_equip_unlocked() and self.state.auto_equip_best_on:
            self.state.equip_best()

    def spawn_roll_particles(self, rarity_index):
        tier = RARITIES[rarity_index]["tier"]
        if not self.animations or tier < 4:
            return
        color = RARITIES[rarity_index]["color"]
        n = 14 if tier < 7 else (24 if tier < TIER_COSMIC else 36)
        cx, cy = self.main_card_rect().center
        for _ in range(n):
            self.particles.append(Particle(cx, cy, color))