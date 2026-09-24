"""Página de Traits."""

import pygame

from config import TOPBAR_H, VIRTUAL_H
from core.traits import TRAITS
from i18n import tr
from theme import (
    ACCENT, ACCENT_HOVER, BAD, BLACK,
    GOOD, GREY, PANEL, PANEL_LIGHT,
    PANEL_LIGHTER, WHITE,
)
from ui.drawing import bake, dim_overlay, draw_panel
from ui.fonts import fit_text, wrap_text


class TraitsPanelMixin:
    """Página de Traits."""

    def close_traits(self):
        self.traits_open = False

    def toggle_traits(self):
        if self.traits_open:
            self.traits_open = False
        else:
            self.close_overlays()      # Traits não convive com Stats nem com Options
            self.traits_open = True
            self.right_panel.close()
            self.left_panel.close()

    def notify_trait_charges(self, n=1):
        """Aviso de carga de trait. Se ganhares mais cargas antes de o aviso desaparecer,
        o mesmo aviso passa a dizer x2, x3... (e volta a contar o tempo)."""
        if n <= 0 or not self.settings.get("trait_notifications", True):
            return
        if self.toast_kind == "trait" and self.toast_timer > 0:
            self.trait_toast_count += n
        else:
            self.trait_toast_count = n
        self.toast_kind = "trait"
        if self.trait_toast_count == 1:
            self.toast_text = tr("You gained a Trait Roll!")
        else:
            self.toast_text = tr("You gained a Trait Roll!  x%d", self.trait_toast_count)
        self.toast_timer = 1.8

    # ---------------------------------------------------------------- página de traits
    def draw_traits_page(self, mouse_pos):
        # Só os botões de navegação (Stats/Options no topo e os laterais) ficam clicáveis por
        # baixo (ver begin_modal); tudo fica escurecido pelo overlay, incluindo os botões laterais.
        self.canvas.blit(dim_overlay(self.vw, VIRTUAL_H, 170), (0, 0))

        # deixa espaço dos dois lados para os botões laterais e os nomes deles (MILESTONES...)
        # e começa por baixo da barra do topo, para não tapar os botões Stats / Options
        panel_w = max(640, min(self.vw - 400, 1080))
        top = TOPBAR_H + 12
        panel_h = VIRTUAL_H - top - 14
        rect = pygame.Rect(self.vw // 2 - panel_w // 2, top, panel_w, panel_h)
        draw_panel(self.canvas, rect, PANEL, radius=16)
        self.register_button(rect, lambda: None, None)     # clicar dentro da página não fecha

        # título + a frase de ajuda por baixo dele (antes ficava por cima e ilegível)
        title = self.font_big.render(tr("Traits"), True, WHITE)
        self.canvas.blit(bake(title, PANEL), (rect.x + 26, rect.y + 16))
        close_rect = pygame.Rect(rect.right - 48, rect.y + 18, 30, 30)
        self.button(close_rect, "X", self.font_small_b, mouse_pos, PANEL_LIGHT, BAD, WHITE,
                    callback=self.close_traits, radius=8)

        sub_y = rect.y + 16 + title.get_height() + 2
        sub_text = fit_text(self.font_small, tr("Your collection — click a trait you own to equip it."),
                            panel_w - 52 - 60)
        self.canvas.blit(bake(self.font_small.render(sub_text, True, GREY), PANEL), (rect.x + 26, sub_y))

        content_top = sub_y + self.font_small.get_height() + 14
        content_h = rect.bottom - 22 - content_top
        list_w = int(panel_w * 0.36)
        list_rect = pygame.Rect(rect.x + 22, content_top, list_w, content_h)
        roll_rect = pygame.Rect(list_rect.right + 22, content_top, panel_w - list_w - 66, content_h)

        self.draw_traits_list(list_rect, mouse_pos)
        self.draw_traits_roll_area(roll_rect, mouse_pos)

    def draw_traits_list(self, rect, mouse_pos):
        self.traits_list_rect = rect

        self.push_clip(rect)
        scroll = self.traits_scroll
        card_w = rect.width - 16          # sobra uma faixa à direita para a barra de scroll
        card_h = 92
        gap = 12
        y = rect.top - scroll

        for i, trait in enumerate(TRAITS):
            crect = pygame.Rect(rect.x, y, card_w, card_h)
            if crect.bottom >= rect.top - 4 and crect.top <= rect.bottom + 4:
                owned = i in self.state.owned_traits
                equipped = self.state.equipped_trait == i
                surf = self.render_trait_card(i, card_w, card_h, owned, equipped)
                # os cartoes estao em cache e ficam por cima do painel (cor lisa): mistura-se
                # cada um com essa cor uma vez e passa a copiar-se sem alfa (ver bake())
                self.canvas.blit(bake(surf, PANEL), crect.topleft)
                if owned:
                    if crect.collidepoint(mouse_pos) and rect.collidepoint(mouse_pos):
                        pygame.draw.rect(self.canvas, WHITE, crect, width=2, border_radius=12)

                    def make_cb(idx=i):
                        return lambda: self.state.equip_trait(idx)
                    self.register_button(crect, make_cb(), "equip")
            y += card_h + gap

        content_h = (len(TRAITS) * (card_h + gap)) - gap
        self.traits_max_scroll = max(0.0, content_h - rect.height)
        self.traits_scroll = max(0.0, min(self.traits_max_scroll, self.traits_scroll))
        self.pop_clip()
        self.draw_scrollbar(rect, scroll, content_h, key="traits", mouse_pos=mouse_pos)

    BATCH_ROW_H = 20

    def traits_batch_height(self):
        """Altura do resumo do 'Use All Charges': cabeçalho + uma linha por trait que saiu."""
        return 36 + len(self.state.last_trait_batch) * self.BATCH_ROW_H + 10

    def draw_traits_batch_summary(self, rect):
        """Mostra o resumo de tudo o que saiu no último 'Usar Todas as Cargas'.
        A caixa adapta-se ao conteúdo (largura e nº de linhas), por isso nada sai dela."""
        batch = self.state.last_trait_batch
        total = sum(batch.values())
        draw_panel(self.canvas, rect, PANEL_LIGHT, radius=12)
        title_txt = fit_text(self.font_small_b, tr("Batch summary (%d charges used):", total), rect.width - 28)
        title = self.font_small_b.render(title_txt, True, WHITE)
        self.canvas.blit(title, (rect.x + 14, rect.y + 10))

        ty = rect.y + 36
        row_h = self.BATCH_ROW_H
        for idx in sorted(batch.keys(), reverse=True):
            trait = TRAITS[idx]
            count = batch[idx]
            swatch = pygame.Rect(rect.x + 14, ty + 3, 14, 14)
            pygame.draw.rect(self.canvas, trait["color"], swatch, border_radius=3)
            line = fit_text(self.font_small, "x%d  %s" % (count, tr(trait["name"])),
                            rect.right - 14 - (swatch.right + 10))
            self.canvas.blit(self.font_small.render(line, True, WHITE), (swatch.right + 10, ty))
            ty += row_h
        return rect

    def draw_traits_roll_area(self, rect, mouse_pos):
        card_w, card_h = 260, 172
        card_rect = pygame.Rect(rect.centerx - card_w // 2, rect.top + 10, card_w, card_h)

        if self.state.last_trait_batch:
            # mais larga que o cartão normal e com altura para TODAS as traits (até 10 linhas)
            sw = max(card_w, min(rect.width - 20, 380))
            summary_rect = pygame.Rect(rect.centerx - sw // 2, rect.top + 10, sw, self.traits_batch_height())
            self.draw_traits_batch_summary(summary_rect)
            y = summary_rect.bottom + 16
        elif self.state.last_trait_roll is not None:
            surf = self.render_trait_card(self.state.last_trait_roll, card_w, card_h,
                                          True, self.state.equipped_trait == self.state.last_trait_roll)
            self.canvas.blit(surf, card_rect.topleft)
            y = card_rect.bottom + 22
        else:
            draw_panel(self.canvas, card_rect, PANEL_LIGHT, radius=12)
            txt = self.font_small.render(tr("You haven't rolled any traits yet."), True, GREY)
            self.canvas.blit(bake(txt, PANEL_LIGHT), txt.get_rect(center=card_rect.center))
            y = card_rect.bottom + 22
        charges = self.state.trait_charges
        charges_txt = self.font_med.render(tr("Charges available: %d", charges), True, GOOD if charges else GREY)
        self.canvas.blit(charges_txt, charges_txt.get_rect(center=(rect.centerx, y + charges_txt.get_height() // 2)))
        y += 34

        chance_str = tr("Every pet roll (manual or Auto Roller) has a %.3f%% chance of giving 1 charge.",
                        self.state.trait_charge_chance() * 100)
        for line in wrap_text(chance_str, self.font_tiny, rect.width - 20):
            chance_txt = self.font_tiny.render(line, True, GREY)
            self.canvas.blit(bake(chance_txt, PANEL), chance_txt.get_rect(center=(rect.centerx, y + chance_txt.get_height() // 2)))
            y += 16
        y += 12

        btn_w = min(260, rect.width - 20)

        def do_single_roll():
            self.state.roll_trait()
            self.state.last_trait_batch = None
            self.play("trait_roll")

        self.button(pygame.Rect(rect.centerx - btn_w // 2, y, btn_w, 46), tr("Roll Trait  (1 charge)"),
                    self.font_med, mouse_pos, ACCENT if charges > 0 else (70, 73, 88),
                    ACCENT_HOVER, BLACK if charges > 0 else GREY,
                    callback=do_single_roll if charges > 0 else None, enabled=charges > 0, radius=10,
                    sfx=None)
        y += 56

        def do_all_rolls():
            n = self.state.trait_charges
            results = {}
            while self.state.trait_charges > 0:
                idx = self.state.roll_trait()
                if idx is not None:
                    results[idx] = results.get(idx, 0) + 1
            if n > 0:
                self.state.last_trait_batch = results
                self.play("trait_roll")
                self.show_toast(tr("Used %d trait charges!", n))

        self.button(pygame.Rect(rect.centerx - btn_w // 2, y, btn_w, 40),
                    tr("Use All Charges (%d)", charges), self.font_small_b, mouse_pos,
                    (52, 120, 80) if charges > 0 else (70, 73, 88), PANEL_LIGHTER,
                    WHITE if charges > 0 else GREY,
                    callback=do_all_rolls if charges > 0 else None, enabled=charges > 0, radius=10,
                    sfx=None)
        y += 54

        # ---- resumo da trait equipada ----
        eq = self.state.equipped_trait
        box_rect = pygame.Rect(rect.x + 10, y, rect.width - 20, rect.bottom - y - 4)
        if box_rect.height > 40:
            draw_panel(self.canvas, box_rect, PANEL_LIGHT, radius=10, shadow=False)
            self.push_clip(box_rect)      # nada do que está lá dentro pode sair da caixa
            inner_w = box_rect.width - 24
            line_h = 20
            if eq is None:
                ty = box_rect.y + 12
                for text in (tr("No trait equipped."),
                             tr("Equip a trait from the list on the side to get its buffs.")):
                    for wline in wrap_text(text, self.font_small, inner_w):
                        self.canvas.blit(bake(self.font_small.render(wline, True, WHITE if ty == box_rect.y + 12 else GREY),
                                              PANEL_LIGHT), (box_rect.x + 12, ty))
                        ty += line_h
            else:
                trait = TRAITS[eq]
                labels = {"money": tr("Money"), "luck": tr("Luck"), "secret_luck": tr("Secret+ Luck"),
                          "mutation": tr("Mutation Chance"), "auto_speed": tr("Auto Speed"),
                          "charge_chance": tr("Trait Charge Chance")}
                items = ["+%.0f%% %s" % (trait["buffs"][k] * 100, label)
                         for k, label in labels.items() if trait["buffs"].get(k)]
                head = self.font_small.render(fit_text(self.font_small, tr("Equipped: %s", tr(trait["name"])), inner_w),
                                              True, WHITE)
                self.canvas.blit(head, (box_rect.x + 12, box_rect.y + 12))
                items_top = box_rect.y + 12 + line_h
                # 1 coluna se couber na altura; senão 2 colunas (as traits raras têm 6-7 buffs)
                one_col_h = len(items) * line_h + 12
                cols = 1 if (items_top - box_rect.y) + one_col_h <= box_rect.height else 2
                col_w = inner_w // cols
                rows = (len(items) + cols - 1) // cols
                for n_i, text in enumerate(items):
                    col, row = n_i // rows, n_i % rows
                    text = fit_text(self.font_small, text, col_w - 8)
                    self.canvas.blit(self.font_small.render(text, True, GREY),
                                     (box_rect.x + 12 + col * col_w, items_top + row * line_h))
            self.pop_clip()
