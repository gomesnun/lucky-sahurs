"""Cartões dos pets e das traits."""

import pygame

from core.formatting import format_one_in
from core.pets import MUTATIONS
from core.traits import TRAITS
from i18n import tr
from theme import BORDER_W_SMALL, GOLD_BORDER, GREY, GREY_DIM, OUTLINE, PANEL_LIGHT, WHITE
from ui.drawing import draw_rarity_bg
from ui.fonts import blit_text, wrap_text


class CardsMixin:
    """Cartões dos pets e das traits."""

    # ---------------------------------------------------------------- cartões de pet
    def render_pet_card(self, rarity, mutation, w, h, name_font=None, bottom_lines=None,
                        reserve_bottom=0, show_rarity=True, label_font=None):
        """Cartão do pet: raridade fininha em cima, nome ao centro, extras em baixo.
        A mutação é só uma borda — a cor de fundo nunca muda."""
        surf = pygame.Surface((w, h), pygame.SRCALPHA)
        rect_local = pygame.Rect(0, 0, w, h)
        draw_rarity_bg(surf, rect_local, rarity, radius=12)

        border = MUTATIONS[mutation]["border"]
        if border:
            pygame.draw.rect(surf, border, rect_local, width=4, border_radius=12)
        else:
            pygame.draw.rect(surf, OUTLINE, rect_local, width=BORDER_W_SMALL, border_radius=12)

        color = rarity["text"]
        # Fundos com duas cores / riscas (Exótico, Secreto, Divino): nenhuma cor de texto
        # se lê bem em cima de tudo, por isso o texto é sempre BRANCO sobre uma placa
        # escura quase opaca. Nos fundos lisos basta o texto com uma sombrinha.
        use_plate = rarity.get("color2") is not None

        def draw_text(font, text, cx, cy_top):
            if use_plate:
                t = getattr(font, "raw", font).render(text, True, WHITE)
                r = t.get_rect(center=(cx, cy_top + font.get_height() // 2))
                bg_rect = r.inflate(14, 2)
                bg = pygame.Surface(bg_rect.size, pygame.SRCALPHA)
                pygame.draw.rect(bg, (8, 8, 12, 218), bg.get_rect(), border_radius=6)
                surf.blit(bg, bg_rect.topleft)
                surf.blit(t, r.topleft)
            else:
                blit_text(surf, font, text, color, center=(cx, cy_top + font.get_height() // 2))

        label_font = label_font or self.font_small_b       # fonte da raridade e das linhas de baixo
        top = 8
        if show_rarity:
            draw_text(label_font, tr(rarity["name"]), w // 2, top)
            top += label_font.get_height() + 5

        bottom_lines = bottom_lines or []
        bottom_font = label_font
        line_pitch = bottom_font.get_height() + 3
        bottom_h = len(bottom_lines) * line_pitch
        bottom_start = h - reserve_bottom - bottom_h - 8

        name_font = name_font or self.font_med
        name_lines = wrap_text(rarity["pet"], name_font, w - 22)
        name_h = len(name_lines) * (name_font.get_height() + 3)
        ny = top + max(4, (bottom_start - top - name_h) // 2)
        for line in name_lines:
            draw_text(name_font, line, w // 2, ny)
            ny += name_font.get_height() + 3

        by = bottom_start
        for line in bottom_lines:
            draw_text(bottom_font, line, w // 2, by)
            by += line_pitch
        return surf

    def render_trait_card(self, trait_index, w, h, owned, equipped):
        trait = TRAITS[trait_index]
        surf = pygame.Surface((w, h), pygame.SRCALPHA)
        rect_local = pygame.Rect(0, 0, w, h)

        if not owned:
            pygame.draw.rect(surf, PANEL_LIGHT, rect_local, border_radius=12)
            pygame.draw.rect(surf, OUTLINE, rect_local, width=BORDER_W_SMALL, border_radius=12)
            q = self.font_huge.render("?", True, GREY_DIM)
            surf.blit(q, q.get_rect(center=(w // 2, h // 2 - 6)))
            ch_txt = self.font_small.render(format_one_in(self.state.trait_chance(trait_index)), True, GREY)
            surf.blit(ch_txt, ch_txt.get_rect(center=(w // 2, h - 16)))
            return surf

        pygame.draw.rect(surf, trait["color"], rect_local, border_radius=12)
        border_color = GOLD_BORDER if equipped else OUTLINE
        pygame.draw.rect(surf, border_color, rect_local, width=4 if equipped else BORDER_W_SMALL, border_radius=12)

        color = trait["text"]
        name_lines = wrap_text(tr(trait["name"]), self.font_med, w - 20)
        ny = 10
        for line in name_lines:
            blit_text(surf, self.font_med, line, color,
                      center=(w // 2, ny + self.font_med.get_height() // 2))
            ny += self.font_med.get_height()

        blit_text(surf, self.font_small_b, format_one_in(self.state.trait_chance(trait_index)), color,
                  center=(w // 2, h - 16))
        if equipped:
            blit_text(surf, self.font_small_b, tr("EQUIPPED"), color, center=(w // 2, h - 36))
        return surf
