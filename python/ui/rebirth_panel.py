"""Página de Rebirths."""

import pygame

from config import TOPBAR_H, VIRTUAL_H
from core.formatting import format_number
from core.rebirths import REBIRTH_LUCK_PER, REBIRTH_MONEY_PER, REBIRTH_REWARDS, format_rebirth_reward
from i18n import tr
from theme import (
    ACCENT, ACCENT_HOVER, BAD, BLACK,
    GOOD, GREY, PANEL, PANEL_LIGHT, WHITE,
)
from ui.drawing import bake, dim_overlay, draw_panel, draw_state_border
from ui.fonts import fit_text, wrap_text

REWARD_ROW_H = 68       # altura de cada recompensa na lista (a borda colorida fica ~7 px por dentro: o texto precisa de folga)
REWARD_GAP = 8


class RebirthPanelMixin:
    """Página de Rebirths."""

    def close_rebirth(self):
        self.rebirth_open = False
        self.rebirth_confirm = False

    def toggle_rebirth(self):
        if self.rebirth_open:
            self.close_rebirth()
        else:
            self.close_overlays()      # Rebirth não convive com Stats/Options/Traits/Leaderboard
            self.rebirth_open = True
            self.rebirth_confirm = False
            self.right_panel.close()
            self.left_panel.close()
            # a lista de recompensas abre já a mostrar a próxima por desbloquear
            nxt = self.state.next_rebirth_reward()
            self.rebirth_scroll = 0.0 if nxt is None else max(0, nxt - 1) * float(REWARD_ROW_H + REWARD_GAP)

    def do_rebirth_clicked(self):
        st = self.state
        if not st.rebirth_available():
            return
        if not self.rebirth_confirm:          # 1º clique: só pede confirmação (é destrutivo)
            self.rebirth_confirm = True
            self.rebirth_confirm_timer = 4.0
            return
        if st.do_rebirth():
            self.rebirth_confirm = False
            self.play("rebirth")
            msg = tr("Rebirth #%d! Permanent +%.0f%% Money, +%.0f%% Luck.",
                     st.rebirths, REBIRTH_MONEY_PER * 100, REBIRTH_LUCK_PER * 100)
            reward_lines = [tr("Reward unlocked: %s - %s", tr(name), format_rebirth_reward(rewards))
                            for need, name, rewards in REBIRTH_REWARDS if need == st.rebirths]
            if reward_lines:
                msg += "\n" + "\n".join(reward_lines)
            self.show_toast(msg, 4.5 if reward_lines else 1.8)

    # ---------------------------------------------------------------- página de rebirths
    def draw_rebirth_page(self, mouse_pos):
        # o mesmo veu escuro das outras paginas: em cache e, no telemovel, opaco
        # (uma superficie do tamanho do ecra com alfa custava ~182 ms por frame)
        self.canvas.blit(dim_overlay(self.vw, VIRTUAL_H, 170), (0, 0))

        panel_w = max(560, min(self.vw - 400, 760))
        top = TOPBAR_H + 12
        panel_h = VIRTUAL_H - top - 14
        rect = pygame.Rect(self.vw // 2 - panel_w // 2, top, panel_w, panel_h)
        draw_panel(self.canvas, rect, PANEL, radius=16)
        self.register_button(rect, lambda: None, None)     # clicar dentro da página não fecha

        title = self.font_big.render(tr("Rebirth"), True, WHITE)
        self.canvas.blit(bake(title, PANEL), (rect.x + 26, rect.y + 16))
        close_rect = pygame.Rect(rect.right - 48, rect.y + 18, 30, 30)
        self.button(close_rect, "X", self.font_small_b, mouse_pos, PANEL_LIGHT, BAD, WHITE,
                    callback=self.close_rebirth, radius=8)

        st = self.state
        keep = st.rebirth_keeps_upgrades()          # a partir do Rebirth 10 só o dinheiro reseta
        sub_y = rect.y + 16 + title.get_height() + 2
        if keep:
            sub_text = tr("Reset your coins for a PERMANENT boost to Money and Luck. "
                          "Upgrades, pets, traits, milestones and playtime are all kept.")
        else:
            sub_text = tr("Reset your coins and upgrades for a PERMANENT boost to Money and Luck. "
                          "Pets, traits, milestones and playtime are all kept.")
        y = sub_y
        for line in wrap_text(sub_text, self.font_small, rect.width - 52):
            self.canvas.blit(bake(self.font_small.render(line, True, GREY), PANEL), (rect.x + 26, y))
            y += self.font_small.get_height() + 2
        y += 14

        # ---- cartão com o estado atual ----
        card_rect = pygame.Rect(rect.x + 26, y, rect.width - 52, 124)
        draw_panel(self.canvas, card_rect, PANEL_LIGHT, radius=12, shadow=False)
        lines = [
            (tr("Rebirths"), "%d" % st.rebirths),
            (tr("Money bonus (permanent)"), "+%.0f%%" % (REBIRTH_MONEY_PER * st.rebirths * 100)),
            (tr("Luck bonus, Rare+ (permanent)"), "+%.0f%%" % (REBIRTH_LUCK_PER * st.rebirths * 100)),
        ]
        ly = card_rect.y + 14
        for label, value in lines:
            l_txt = self.font_med.render(label, True, GREY)
            v_txt = self.font_med.render(value, True, GOOD)
            self.canvas.blit(bake(l_txt, PANEL_LIGHT), (card_rect.x + 18, ly))
            self.canvas.blit(bake(v_txt, PANEL_LIGHT), (card_rect.right - 18 - v_txt.get_width(), ly))
            ly += 36
        y = card_rect.bottom + 22

        cost = st.rebirth_cost()
        can = st.rebirth_available()
        cost_txt = self.font_med.render(
            tr("Next rebirth costs $%s  (you have $%s)", format_number(cost), format_number(st.coins)),
            True, WHITE if can else GREY)
        self.canvas.blit(bake(cost_txt, PANEL), cost_txt.get_rect(center=(rect.centerx, y)))
        y += 24

        btn_w = min(420, rect.width - 60)
        if self.rebirth_confirm:
            label = tr("Click again to confirm - resets your coins!") if keep else \
                    tr("Click again to confirm - resets coins & upgrades!")
            color = BAD
        else:
            label = tr("Rebirth  (+%.0f%% Money, +%.0f%% Luck)", REBIRTH_MONEY_PER * 100, REBIRTH_LUCK_PER * 100)
            color = ACCENT if can else (70, 73, 88)
        self.button(pygame.Rect(rect.centerx - btn_w // 2, y, btn_w, 50), label, self.font_small_b, mouse_pos,
                    color, ACCENT_HOVER, BLACK if can else GREY,
                    callback=self.do_rebirth_clicked if can else None, enabled=can, radius=10, sfx=None)
        y += 50 + 18

        note_text = (tr("Only your coins reset. Everything else (upgrades, pets, traits...) stays.") if keep else
                     tr("Only coins and upgrade levels reset. Everything else (pets, traits, milestones...) stays."))
        note = self.font_tiny.render(note_text, True, GREY)
        self.canvas.blit(bake(note, PANEL), note.get_rect(center=(rect.centerx, y)))
        y += 22

        # ---- recompensas: um bónus permanente ao chegar a certos números de rebirths ----
        got = sum(1 for need, _n, _r in REBIRTH_REWARDS if st.rebirths >= need)
        head = self.font_med.render(tr("Rebirth Rewards  (%d/%d)", got, len(REBIRTH_REWARDS)), True, ACCENT)
        self.canvas.blit(bake(head, PANEL), (rect.x + 26, y))
        y += head.get_height() + 8
        list_rect = pygame.Rect(rect.x + 22, y, rect.width - 44, rect.bottom - 16 - y)
        self.draw_rebirth_rewards(list_rect, mouse_pos)

    def draw_rebirth_rewards(self, rect, mouse_pos):
        """Lista (com scroll) das recompensas de Rebirth: as já desbloqueadas a verde, a próxima a dourado."""
        st = self.state
        self.rebirth_list_rect = rect
        nxt = st.next_rebirth_reward()
        content_h = len(REBIRTH_REWARDS) * (REWARD_ROW_H + REWARD_GAP) - REWARD_GAP
        self.rebirth_max_scroll = max(0.0, content_h - rect.height)
        self.rebirth_scroll = max(0.0, min(self.rebirth_max_scroll, self.rebirth_scroll))
        scroll = self.rebirth_scroll

        self.push_clip(rect)
        row_w = rect.width - 20           # sobra uma faixa à direita para a barra de scroll
        # título + descrição ficam centrados na vertical dentro do cartão (medidos pelas fontes)
        title_h = self.font_small_b.get_height()
        desc_h = self.font_small.get_height()
        text_top = max(8, (REWARD_ROW_H - (title_h + 3 + desc_h)) // 2)
        for i, (need, name, rewards) in enumerate(REBIRTH_REWARDS):
            row = pygame.Rect(rect.x, rect.top + i * (REWARD_ROW_H + REWARD_GAP) - scroll, row_w, REWARD_ROW_H)
            if row.bottom < rect.top or row.top > rect.bottom:
                continue
            unlocked = st.rebirths >= need
            draw_panel(self.canvas, row, PANEL_LIGHT, radius=10, shadow=False)
            if unlocked:
                draw_state_border(self.canvas, row, (80, 150, 100), 10)
            elif i == nxt:
                draw_state_border(self.canvas, row, ACCENT, 10)

            status = self.font_small_b.render(tr("Unlocked") if unlocked else "%d / %d" % (st.rebirths, need),
                                              True, GOOD if unlocked else GREY)
            self.canvas.blit(bake(status, PANEL_LIGHT), (row.right - 16 - status.get_width(), row.y + text_top))
            title_txt = fit_text(self.font_small_b, tr("Rebirth %d  -  %s", need, tr(name)),
                                 row.width - 44 - status.get_width())
            self.canvas.blit(bake(self.font_small_b.render(title_txt, True, WHITE if (unlocked or i == nxt) else GREY),
                                  PANEL_LIGHT), (row.x + 16, row.y + text_top))
            reward_txt = fit_text(self.font_small, format_rebirth_reward(rewards), row.width - 32)
            self.canvas.blit(bake(self.font_small.render(reward_txt, True, GOOD if unlocked else GREY),
                                  PANEL_LIGHT), (row.x + 16, row.y + text_top + title_h + 3))
        self.pop_clip()
        self.draw_scrollbar(rect, scroll, content_h, key="rebirth", mouse_pos=mouse_pos)
