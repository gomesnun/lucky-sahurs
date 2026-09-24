"""Painel de Milestones (e o aviso quando ganhas uma)."""

import pygame

from core.formatting import format_number, format_playtime
from core.milestones import MILESTONE_CAT_ORDER, MILESTONE_DEFS, MILESTONE_REWARD_LABELS
from i18n import tr
from theme import ACCENT, GOOD, GREY, PANEL, PANEL_LIGHT, WHITE
from ui.drawing import bake, draw_panel, draw_state_border


class MilestonesPanelMixin:
    """Painel de Milestones (e o aviso quando ganhas uma)."""

    def format_milestone_reward(self, reward_type, value):
        label = tr(MILESTONE_REWARD_LABELS.get(reward_type, reward_type))
        return "+%.0f%% %s" % (value * 100, label)

    def check_milestones(self):
        newly = self.state.check_milestones()
        if not newly:
            return
        self.play("milestone")
        if len(newly) == 1:
            category, _i, _threshold, value = newly[0]
            d = MILESTONE_DEFS[category]
            reward_txt = self.format_milestone_reward(d["reward_type"], value)
            self.show_toast(tr("Milestone reached: %s!  (%s permanent)", tr(d["label"]), reward_txt))
        else:
            self.show_toast(tr("%d Milestones reached at once!", len(newly)))

    # ---------------------------------------------------------------- painel: metas
    def milestone_progress_text(self, category, metric, threshold):
        if category == "playtime":
            return "%s / %s" % (format_playtime(metric), format_playtime(threshold))
        if category == "coins":
            return "$%s / $%s" % (format_number(metric), format_number(threshold))
        return "%s / %s" % (format_number(metric), format_number(threshold))

    def select_milestone_category(self, category):
        self.milestones_selected_category = category
        self.right_panel.scroll["milestones"] = 0.0

    def draw_milestones_panel(self, rect, mouse_pos):
        self.panel_header(rect, tr("Milestones"), mouse_pos, self.right_panel.close)

        top = rect.y + 62
        category = self.milestones_selected_category
        if category is not None:
            d = MILESTONE_DEFS[category]
            back_rect = pygame.Rect(rect.x + 20, top, 128, 32)
            self.button(back_rect, tr("<  Categories"), self.font_small_b, mouse_pos,
                        PANEL_LIGHT, PANEL, WHITE,
                        callback=lambda: self.select_milestone_category(None), radius=8)
            claimed_n = sum(1 for i in range(len(d["tiers"]))
                            if ("%s:%d" % (category, i)) in self.state.milestones_claimed)
            cat_txt = self.font_med.render("%s  (%d/%d)" % (tr(d["label"]), claimed_n, len(d["tiers"])),
                                           True, ACCENT)
            self.canvas.blit(cat_txt, (rect.x + 20, top + 42))
            top += 84

        content_rect = pygame.Rect(rect.x, top, rect.width, rect.bottom - top)
        scroll = self.right_panel.get_scroll()
        self.push_clip(content_rect)
        if category is None:
            content_h = self.draw_milestones_category_list(content_rect, scroll, mouse_pos)
        else:
            content_h = self.draw_milestones_tier_list(content_rect, scroll, mouse_pos, category)
        self.pop_clip()
        self.right_panel.set_max_scroll(max(0, content_h - content_rect.height))
        self.draw_scrollbar(content_rect, scroll, content_h, key="right", mouse_pos=mouse_pos)

    def draw_milestones_category_list(self, content_rect, scroll, mouse_pos):
        """Ecrã inicial das Metas: um botão por categoria, com quantas já
        resgataste (ex: '1/8') e uma barrinha a encher com o progresso."""
        pad = 20
        row_w = content_rect.width - pad * 2 - 6
        row_h = 78
        y = content_rect.top + 6 - scroll

        for category in MILESTONE_CAT_ORDER:
            d = MILESTONE_DEFS[category]
            tiers = d["tiers"]
            claimed_n = sum(1 for i in range(len(tiers)) if ("%s:%d" % (category, i)) in
                            self.state.milestones_claimed)
            done = claimed_n >= len(tiers)
            row_rect = pygame.Rect(content_rect.x + pad, y, row_w, row_h)

            if row_rect.bottom > content_rect.top and row_rect.top < content_rect.bottom:
                draw_panel(self.canvas, row_rect, PANEL_LIGHT, radius=12, shadow=False)
                if done:
                    draw_state_border(self.canvas, row_rect, (80, 150, 100), 12)
                elif row_rect.collidepoint(mouse_pos) and content_rect.collidepoint(mouse_pos):
                    draw_state_border(self.canvas, row_rect, ACCENT, 12)

                name_txt = self.font_med.render(tr(d["label"]), True, WHITE)
                self.canvas.blit(bake(name_txt, PANEL_LIGHT), (row_rect.x + 16, row_rect.y + 10))
                reward_lbl = tr(MILESTONE_REWARD_LABELS.get(d["reward_type"], d["reward_type"]))
                active = self.state.milestone_bonus(d["reward_type"])
                sub_txt = self.font_tiny.render(tr("Bonus: %s  (active: +%.0f%%)", reward_lbl, active * 100),
                                                True, GREY)
                self.canvas.blit(bake(sub_txt, PANEL_LIGHT), (row_rect.x + 16, row_rect.y + 36))

                count_txt = self.font_small_b.render("%d/%d" % (claimed_n, len(tiers)), True,
                                                      GOOD if done else WHITE)
                self.canvas.blit(bake(count_txt, PANEL_LIGHT), (row_rect.right - count_txt.get_width() - 40, row_rect.y + 12))
                arrow_txt = self.font_med.render(">", True, GREY)
                self.canvas.blit(bake(arrow_txt, PANEL_LIGHT), (row_rect.right - 26, row_rect.centery - 12))

                bar_rect = pygame.Rect(row_rect.x + 16, row_rect.bottom - 20, row_rect.width - 32, 10)
                pygame.draw.rect(self.canvas, PANEL, bar_rect, border_radius=5)
                frac = claimed_n / len(tiers) if tiers else 0.0
                fill_w = max(0, int(bar_rect.width * frac))
                if fill_w > 0:
                    pygame.draw.rect(self.canvas, GOOD if done else ACCENT,
                                     pygame.Rect(bar_rect.x, bar_rect.y, fill_w, bar_rect.height),
                                     border_radius=5)

                self.register_button(row_rect, lambda c=category: self.select_milestone_category(c))

            y += row_h + 14

        return (y + scroll) - (content_rect.top + 6)

    def draw_milestones_tier_list(self, content_rect, scroll, mouse_pos, category):
        """Ecrã de detalhe de uma categoria: uma linha por meta, com progresso."""
        d = MILESTONE_DEFS[category]
        metric = self.state.milestone_metric(category)
        pad = 20
        row_w = content_rect.width - pad * 2 - 6
        row_h = 62
        y = content_rect.top + 6 - scroll

        for i, (threshold, value) in enumerate(d["tiers"]):
            mkey = "%s:%d" % (category, i)
            claimed = mkey in self.state.milestones_claimed
            row_rect = pygame.Rect(content_rect.x + pad, y, row_w, row_h)

            if row_rect.bottom > content_rect.top and row_rect.top < content_rect.bottom:
                draw_panel(self.canvas, row_rect, PANEL_LIGHT, radius=10, shadow=False)
                if claimed:
                    draw_state_border(self.canvas, row_rect, (80, 150, 100), 10)

                name_txt = self.font_small_b.render(tr("Milestone %d", i + 1), True, WHITE)
                self.canvas.blit(bake(name_txt, PANEL_LIGHT), (row_rect.x + 14, row_rect.y + 8))
                reward_txt = self.font_small_b.render(self.format_milestone_reward(d["reward_type"], value),
                                                      True, GOOD if claimed else GREY)
                self.canvas.blit(reward_txt, (row_rect.right - reward_txt.get_width() - 14, row_rect.y + 8))

                bar_rect = pygame.Rect(row_rect.x + 14, row_rect.y + 32, row_rect.width - 28, 12)
                pygame.draw.rect(self.canvas, PANEL, bar_rect, border_radius=6)
                frac = 1.0 if claimed else max(0.0, min(1.0, metric / threshold if threshold else 1.0))
                fill_w = max(0, int(bar_rect.width * frac))
                if fill_w > 0:
                    pygame.draw.rect(self.canvas, GOOD if claimed else ACCENT,
                                     pygame.Rect(bar_rect.x, bar_rect.y, fill_w, bar_rect.height),
                                     border_radius=6)
                prog_txt = self.font_tiny.render(
                    tr("Claimed!") if claimed else self.milestone_progress_text(category, metric, threshold),
                    True, GOOD if claimed else GREY)
                self.canvas.blit(prog_txt, (bar_rect.right - prog_txt.get_width(), bar_rect.bottom + 3))

            y += row_h + 10

        return (y + scroll) - (content_rect.top + 6)
