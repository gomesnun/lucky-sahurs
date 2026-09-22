"""Painel de Missões Diárias."""

import pygame

from core.daily_missions import mission_label
from i18n import tr
from theme import ACCENT, GOOD, GREY, PANEL, PANEL_LIGHT, WHITE
from ui.drawing import bake, draw_panel, draw_state_border


class DailyPanelMixin:
    """Painel de Missões Diárias."""

    def claim_daily_mission(self, index):
        reward = self.state.claim_daily_mission(index)
        if reward:
            self.play("trait_charge")
            self.show_toast(tr("Daily mission claimed: +%d trait charge!", reward) if reward == 1
                            else tr("Daily mission claimed: +%d trait charges!", reward))

    def draw_daily_panel(self, rect, mouse_pos):
        self.state.ensure_daily_missions()
        self.panel_header(rect, tr("Daily Missions"), mouse_pos, self.right_panel.close)

        top = rect.y + 62
        sub = self.font_small.render(tr("Resets every day - rewards are trait charges."), True, GREY)
        self.canvas.blit(bake(sub, PANEL), (rect.x + 20, top))
        top += sub.get_height() + 14

        content_rect = pygame.Rect(rect.x, top, rect.width, rect.bottom - top)
        scroll = self.right_panel.get_scroll()
        self.push_clip(content_rect)
        content_h = self.draw_daily_list(content_rect, scroll, mouse_pos)
        self.pop_clip()
        self.right_panel.set_max_scroll(max(0, content_h - content_rect.height))
        self.draw_scrollbar(content_rect, scroll, content_h, key="right", mouse_pos=mouse_pos)

    def draw_daily_list(self, content_rect, scroll, mouse_pos):
        st = self.state
        pad = 20
        row_w = content_rect.width - pad * 2      # margens iguais dos dois lados (a scrollbar cabe na margem da direita)
        # Alturas calculadas a partir das fontes. A borda colorida (missão feita / recebida) fica ~7 px por
        # dentro do contorno, por isso o conteúdo tem 14 px de folga em cima e em baixo para nunca lhe tocar.
        pad_v = 14
        bar_h = 12
        name_h = self.font_med.get_height()
        reward_h = self.font_small_b.get_height()
        prog_h = self.font_tiny.get_height()
        y_name = pad_v
        y_reward = y_name + name_h + 4
        y_bar = y_reward + reward_h + 9
        y_prog = y_bar + bar_h + 5
        row_h = y_prog + prog_h + pad_v
        y = content_rect.top + 6 - scroll

        for i, mission in enumerate(st.daily_missions):
            target = mission["target"]
            progress = st.daily_mission_progress(i)
            claimed = i in st.daily_claimed
            done = progress >= target
            row_rect = pygame.Rect(content_rect.x + pad, y, row_w, row_h)

            if row_rect.bottom > content_rect.top and row_rect.top < content_rect.bottom:
                draw_panel(self.canvas, row_rect, PANEL_LIGHT, radius=12, shadow=False)
                if claimed:
                    draw_state_border(self.canvas, row_rect, (80, 150, 100), 12)
                elif done:
                    draw_state_border(self.canvas, row_rect, ACCENT, 12)

                name_txt = self.font_med.render(mission_label(mission), True, WHITE)
                self.canvas.blit(bake(name_txt, PANEL_LIGHT), (row_rect.x + 16, row_rect.y + y_name))

                reward_txt = self.font_small_b.render(
                    tr("Reward: %d trait charge", mission["reward"]) if mission["reward"] == 1
                    else tr("Reward: %d trait charges", mission["reward"]),
                    True, GOOD if claimed else GREY)
                self.canvas.blit(bake(reward_txt, PANEL_LIGHT), (row_rect.x + 16, row_rect.y + y_reward))

                bar_rect = pygame.Rect(row_rect.x + 16, row_rect.y + y_bar, row_rect.width - 140, bar_h)
                pygame.draw.rect(self.canvas, PANEL, bar_rect, border_radius=6)
                frac = 1.0 if done else max(0.0, min(1.0, progress / target if target else 1.0))
                fill_w = max(0, int(bar_rect.width * frac))
                if fill_w > 0:
                    pygame.draw.rect(self.canvas, GOOD if done else ACCENT,
                                     pygame.Rect(bar_rect.x, bar_rect.y, fill_w, bar_rect.height), border_radius=6)
                prog_txt = self.font_tiny.render("%s / %s" % (min(progress, target), target), True, GREY)
                self.canvas.blit(bake(prog_txt, PANEL_LIGHT), (bar_rect.x, row_rect.y + y_prog))

                btn_rect = pygame.Rect(row_rect.right - 116, row_rect.y + (row_h - 40) // 2, 100, 40)     # centrado na vertical
                if claimed:
                    self.button(btn_rect, tr("Claimed"), self.font_small_b, mouse_pos, PANEL, PANEL, GREY,
                                callback=None, enabled=False, radius=8)
                else:
                    self.button(btn_rect, tr("Claim"), self.font_small_b, mouse_pos,
                                ACCENT if done else (70, 73, 88), ACCENT, WHITE if done else GREY,
                                callback=(lambda idx=i: self.claim_daily_mission(idx)) if done else None,
                                enabled=done, radius=8, sfx=None)

            y += row_h + 14

        if not st.daily_missions:
            msg = self.font_med.render(tr("No missions today."), True, GREY)
            self.canvas.blit(msg, msg.get_rect(center=(content_rect.centerx, content_rect.top + 40)))

        return (y + scroll) - (content_rect.top + 6)