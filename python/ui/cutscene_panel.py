"""Cutscene ao apanhar um verity Secret ou melhor (Divine, Cosmic, Transcendent): um ecrã cheio
que aparece por cima de tudo, com raios de luz, faíscas e o cartão do verity em grande no meio.
Quanto melhor a raridade, mais chamativa a cutscene (mais raios, mais faíscas, mais tempo no ar
e, no Cosmic/Transcendent, a imagem treme um pouco). Uma tecla, um clique ou o tempo esgotar-se
fecham-na. Pode ser desligada de vez nas Options (Gameplay > Cutscenes)."""

import math

import pygame

from config import VIRTUAL_H
from core.formatting import format_number
from core.pets import MUTATIONS, RARITIES, TIER_SECRET
from i18n import tr
from ui.cards import rarity_glow_color
from ui.drawing import draw_rainbow_border, ease_out_cubic, rarity_glow
from ui.widgets import Particle

# Por raridade (chave em RARITIES): duração no ar, nº de raios a rodar, faíscas por segundo (ambiente),
# quantas faíscas rebentam logo ao aparecer, e quanto treme a imagem (0 = nada).
CUTSCENE_STYLES = {
    "secreto":       {"duration": 1.8, "rays": 8,  "ambient": 10, "burst": 26, "shake": 0.0, "label": "SECRET PET!"},
    "divino":        {"duration": 2.2, "rays": 12, "ambient": 16, "burst": 38, "shake": 0.0, "label": "DIVINE PET!"},
    "cosmico":       {"duration": 2.6, "rays": 16, "ambient": 24, "burst": 54, "shake": 3.0, "label": "COSMIC PET!"},
    "transcendente": {"duration": 3.2, "rays": 22, "ambient": 34, "burst": 76, "shake": 6.0,
                      "label": "TRANSCENDENT PET!"},
}
FADE_IN = 0.25       # segundos a escurecer o ecrã ao abrir
FADE_OUT = 0.35      # segundos a desvanecer no fim


class CutscenePanelMixin:
    """Cutscene dos verities Secret+ (ver cabeçalho do ficheiro)."""

    def init_cutscenes(self):
        self.cutscene_active = None       # (índice do pet, mutação) ou None
        self.cutscene_elapsed = 0.0
        self.cutscene_queue = []          # próximas à espera (a fila quase nunca passa de 1)
        self.cutscene_particles = []
        self.cutscene_ambient_accum = 0.0

    # ---------------------------------------------------------------- disparo / avanço
    def trigger_cutscene(self, rarity_index, mutation):
        """Chamado depois de QUALQUER roll (manual ou automático). Não faz nada a não ser que o
        pet seja Secret ou melhor e as cutscenes dessa raridade estejam ligadas nas Options
        (cada raridade tem o seu próprio interruptor, ver ui/options_panel.py)."""
        rarity = RARITIES[rarity_index]
        if rarity["tier"] < TIER_SECRET:
            return
        if not self.settings.get("cutscenes_" + rarity["key"], True):
            return
        if self.cutscene_active is None and not self.cutscene_queue:
            self.start_cutscene(rarity_index, mutation)
        elif len(self.cutscene_queue) < 3:      # nunca deixa a fila crescer sem parar (auto roller rápido)
            self.cutscene_queue.append((rarity_index, mutation))

    def start_cutscene(self, rarity_index, mutation):
        self.cutscene_active = (rarity_index, mutation)
        self.cutscene_elapsed = 0.0
        self.cutscene_particles = []
        self.cutscene_ambient_accum = 0.0
        self.play_roll_sfx(rarity_index, mutation, manual=True)

    def end_cutscene(self):
        self.cutscene_active = None
        self.cutscene_elapsed = 0.0
        self.cutscene_particles = []

    def skip_cutscene(self):
        if self.cutscene_active is not None:
            self.play("click")
            self.end_cutscene()

    def update_cutscenes(self, dt):
        if self.cutscene_active is None:
            if self.cutscene_queue:
                self.start_cutscene(*self.cutscene_queue.pop(0))
            return
        r_idx, _mutation = self.cutscene_active
        style = CUTSCENE_STYLES[RARITIES[r_idx]["key"]]
        self.cutscene_elapsed += dt
        for p in self.cutscene_particles:
            p.update(dt)
        self.cutscene_particles = [p for p in self.cutscene_particles if p.life > 0]
        if self.animations:
            self.cutscene_ambient_accum += dt * style["ambient"]
            n = int(self.cutscene_ambient_accum)
            if n > 0:
                self.cutscene_ambient_accum -= n
                color = RARITIES[r_idx]["color"]
                cx, cy = self.vw // 2, VIRTUAL_H // 2
                for _ in range(min(n, 6)):
                    self.cutscene_particles.append(Particle(cx, cy, color))
        if self.cutscene_elapsed >= style["duration"]:
            self.end_cutscene()

    # ---------------------------------------------------------------- desenho
    def draw_cutscene(self, mouse_pos):
        # por cima de tudo: nada por baixo fica clicável, só clicar / carregar numa tecla a fecha
        self.buttons = []
        self.scrollbar_hits = {}

        r_idx, mutation = self.cutscene_active
        rarity = RARITIES[r_idx]
        style = CUTSCENE_STYLES[rarity["key"]]
        dur = style["duration"]
        elapsed = self.cutscene_elapsed

        fade_in = min(1.0, elapsed / FADE_IN)
        fade_out = min(1.0, (dur - elapsed) / FADE_OUT) if dur - elapsed < FADE_OUT else 1.0
        alpha_mul = max(0.0, min(fade_in, fade_out))

        shake_x = shake_y = 0.0
        if style["shake"] and self.animations:
            shake_x = math.sin(elapsed * 29.0) * style["shake"]
            shake_y = math.cos(elapsed * 23.0) * style["shake"]

        cx, cy = self.vw / 2.0 + shake_x, VIRTUAL_H / 2.0 + shake_y
        color = tuple(rarity["color"][:3])

        # ---- fundo escuro + raios de luz a rodar (a cor da raridade) ----
        overlay = pygame.Surface((self.vw, VIRTUAL_H), pygame.SRCALPHA)
        overlay.fill((0, 0, 0, int(225 * alpha_mul)))
        if self.animations:
            n_rays = style["rays"]
            spin = elapsed * 0.5
            ray_len = max(self.vw, VIRTUAL_H) * 0.75
            half_w = math.pi / max(10, n_rays * 2)
            for i in range(n_rays):
                ang = spin + i * (2 * math.pi / n_rays)
                p2 = (cx + math.cos(ang - half_w) * ray_len, cy + math.sin(ang - half_w) * ray_len)
                p3 = (cx + math.cos(ang + half_w) * ray_len, cy + math.sin(ang + half_w) * ray_len)
                pygame.draw.polygon(overlay, (*color, int(46 * alpha_mul)), [(cx, cy), p2, p3])
        self.canvas.blit(overlay, (0, 0))

        # ---- brilho + faíscas por trás do cartão ----
        glow_size = int(min(self.vw, VIRTUAL_H) * 0.62)
        pulse = 0.75 + 0.25 * math.sin(elapsed * 5.0)
        glow = rarity_glow((glow_size, glow_size), rarity_glow_color(rarity), spread=46)
        glow = glow.copy()
        glow.set_alpha(int(255 * alpha_mul * pulse))
        self.canvas.blit(glow, glow.get_rect(center=(cx, cy)))
        if self.animations:
            for p in self.cutscene_particles:
                p.draw(self.canvas)

        # ---- cartão do verity, a crescer no início (efeito de "pop") ----
        pop_t = ease_out_cubic(min(1.0, elapsed / 0.4))
        scale = 0.35 + 0.65 * pop_t
        base = max(220, min(380, int(min(self.vw, VIRTUAL_H) * 0.46)))
        income = self.state.pet_income(r_idx, mutation)
        plates = [[(tr("Income"), tr("+%s/sec", format_number(income)))]]
        card_surf = self.render_pet_card(rarity, mutation, base, base, plates=plates)
        if scale < 0.995:
            card_surf = pygame.transform.smoothscale(
                card_surf, (max(1, int(base * scale)), max(1, int(base * scale))))
        if alpha_mul < 0.999:
            card_surf = card_surf.copy()
            card_surf.set_alpha(int(255 * alpha_mul))
        card_rect = card_surf.get_rect(center=(cx, cy))
        self.canvas.blit(card_surf, card_rect)
        if rarity["key"] == "transcendente" and self.animations:
            draw_rainbow_border(self.canvas, card_rect.inflate(-9, -9), 14, width=3)
        if MUTATIONS[mutation]["border"]:
            pygame.draw.rect(self.canvas, MUTATIONS[mutation]["border"], card_rect.inflate(2, 2),
                             width=2, border_radius=14)

        # ---- título grande, com um pequeno "pop" a entrar ----
        title_scale = 0.6 + 0.4 * ease_out_cubic(min(1.0, elapsed / 0.3))
        title = self.font_huge.render(tr(style["label"]), True, color if sum(color) > 260 else (255, 255, 255))
        if title_scale < 0.995:
            title = pygame.transform.smoothscale(
                title, (max(1, int(title.get_width() * title_scale)), max(1, int(title.get_height() * title_scale))))
        if alpha_mul < 0.999:
            title = title.copy()
            title.set_alpha(int(255 * alpha_mul))
        self.canvas.blit(title, title.get_rect(center=(cx, card_rect.top - 46)))

        # ---- aviso para saltar, em baixo ----
        hint_alpha = int(160 * alpha_mul * (0.7 + 0.3 * math.sin(elapsed * 3.0)))
        hint = self.font_small.render(tr("Click or press any key to continue"), True, (255, 255, 255))
        hint = hint.copy()
        hint.set_alpha(max(0, hint_alpha))
        self.canvas.blit(hint, hint.get_rect(center=(self.vw / 2.0, VIRTUAL_H - 44)))

        # clicar em qualquer sítio do ecrã salta a cutscene
        self.register_button(pygame.Rect(0, 0, self.vw, VIRTUAL_H), self.skip_cutscene, None)