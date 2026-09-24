"""Painel da Upgrade Tree."""

import pygame

from core import balance as B
from core.formatting import format_number
from core.pets import DIAMOND_MAX_CHANCE, GOLDEN_MAX_CHANCE
from core.upgrades import UPGRADE_CATEGORIES, UPGRADE_CAT_BY_KEY, UPGRADE_DEFS
from i18n import tr
from storage import save_settings
from theme import ACCENT, ACCENT_HOVER, BLACK, GOOD, GREY, PANEL, PANEL_LIGHT, WHITE
from ui.drawing import bake, draw_panel, draw_state_border
from ui.fonts import fit_text, wrap_text


class UpgradesPanelMixin:
    """Painel da Upgrade Tree."""

    # ---------------------------------------------------------------- painel: árvore
    def upgrade_effect_text(self, key):
        s = self.state
        lvl = s.upgrade_level(key)
        if key == "luck":
            return tr("Now: x%.2f weight on Rare+", 1 + B.LUCK_PER_LEVEL * lvl)
        if key == "luck_prism":
            return tr("Now: x%.2f Epic+ / x%.2f Mythic+ / x%.2f Secret+",
                      1 + B.LUCK_PRISM_PER_LEVEL * lvl, (1 + B.LUCK_PRISM_PER_LEVEL * lvl) ** 2,
                      (1 + B.LUCK_PRISM_PER_LEVEL * lvl) ** 3)
        if key == "luck_cosmic":
            return tr("Now: x%.2f weight on Exotic+", 1 + B.LUCK_COSMIC_PER_LEVEL * lvl)
        if key == "luck_divine":
            return tr("Now: x%.2f weight on Divine+", 1 + B.LUCK_DIVINE_PER_LEVEL * lvl)
        if key == "money":
            return tr("Now: x%.1f money (total)", s.money_multiplier())
        if key == "money_prism":
            return tr("Now: x%.2f money (from this upgrade only)", 1 + B.MONEY_PRISM_PER_LEVEL * lvl)
        if key == "slots" or key == "slots_plus":
            return tr("Now: %d slots", s.max_slots())
        if key in ("auto_speed", "auto_unlock", "auto_turbo"):
            return tr("Now: %.2f rolls/sec", s.auto_rolls_per_second())
        if key in ("golden_chance", "golden_unlock", "golden_chance_2"):
            return tr("Now: %.1f%% Golden (max %.0f%%)", s.mutation_chances()[0] * 100, GOLDEN_MAX_CHANCE * 100)
        if key in ("diamond_chance", "diamond_unlock", "diamond_chance_2"):
            return tr("Now: %.2f%% Diamond (max %.0f%%)", s.mutation_chances()[1] * 100, DIAMOND_MAX_CHANCE * 100)
        if key in ("trait_charge_luck", "trait_charge_luck_2"):
            return tr("Now: %.3f%% charge chance per roll", s.trait_charge_chance() * 100)
        if key == "trait_rarity_luck":
            return tr("Now: x%.2f weight on traits from Instinctive onward", 1 + 0.09 * lvl)
        if key == "trait_rarity_luck_2":
            return tr("Now: x%.2f weight on traits from Instinctive onward", 1 + 0.15 * lvl)
        if key == "cyclic_every":
            return tr("Now: Golden Roll every %d rolls", s.golden_roll_every())
        if key == "cyclic_power":
            return tr("Now: Golden Roll x%g", s.golden_roll_mult())
        if key in ("diamond_roll_unlock", "diamond_roll_every"):
            return tr("Now: Diamond Roll every %d rolls", s.diamond_roll_every())
        if key == "diamond_roll_power":
            return tr("Now: Diamond Roll x%g", s.diamond_roll_mult())
        if key in ("rainbow_roll_unlock", "rainbow_roll_every"):
            return tr("Now: Rainbow Roll every %d rolls", s.rainbow_roll_every())
        if key == "rainbow_roll_power":
            return tr("Now: Rainbow Roll x%g", s.rainbow_roll_mult())
        if key in ("offline_rate", "offline_rate_2"):
            return tr("Now: %.0f%% offline earnings", s.offline_earn_rate() * 100)
        if key in ("offline_time", "offline_time_2"):
            return tr("Now: up to %g hours offline", s.offline_max_seconds() / 3600.0)
        if key == "auto_equip_unlock":
            if lvl < 1:
                return tr("Now: locked")
            return tr("Now: %s  (toggle it in the Bag)", tr("ON") if s.auto_equip_best_on else tr("OFF"))
        return ""

    def tree_category_stats(self, cat):
        """(níveis comprados, níveis máximos, quantas upgrades já dá para comprar agora)."""
        lvls = mx = ready = 0
        for key in cat["upgrades"]:
            lvls += self.state.upgrade_level(key)
            mx += UPGRADE_DEFS[key]["max_level"]
            if self.state.upgrade_affordable(key):
                ready += 1
        return lvls, mx, ready

    def select_tree_category(self, cat_key):
        self.tree_selected_category = cat_key
        self.right_panel.scroll["tree"] = 0.0

    @property
    def buy_mode(self):
        """Quantos níveis cada botão Buy compra: 1, 10 ou "max" (guardado nas definições)."""
        mode = self.settings.get("buy_mode", 1)
        return mode if mode in (1, 10, "max") else 1

    def set_buy_mode(self, mode):
        self.settings["buy_mode"] = mode
        save_settings(self.settings)

    def draw_tree_panel(self, rect, mouse_pos):
        self.panel_header(rect, tr("Upgrades"), mouse_pos, self.right_panel.close)

        top = rect.y + 62
        cat_key = self.tree_selected_category
        cat = UPGRADE_CAT_BY_KEY.get(cat_key)
        if cat is not None:
            back_rect = pygame.Rect(rect.x + 20, top, 128, 32)
            self.button(back_rect, tr("<  Categories"), self.font_small_b, mouse_pos,
                        PANEL_LIGHT, PANEL, WHITE,
                        callback=lambda: self.select_tree_category(None), radius=8)
            # interruptor do modo de compra (x1 / x10 / Max), à direita do botão de voltar
            mode_w, mode_gap = 48, 6
            mx0 = rect.right - 26 - (mode_w * 3 + mode_gap * 2)
            for n, (mode, label) in enumerate(((1, "x1"), (10, "x10"), ("max", tr("Max")))):
                mrect = pygame.Rect(mx0 + n * (mode_w + mode_gap), top, mode_w, 32)
                active = self.buy_mode == mode
                self.button(mrect, label, self.font_small_b, mouse_pos,
                            ACCENT if active else PANEL_LIGHT, ACCENT_HOVER if not active else ACCENT,
                            BLACK if active else WHITE,
                            callback=(lambda m=mode: self.set_buy_mode(m)), radius=8)
            lvls, mx, _ready = self.tree_category_stats(cat)
            cat_txt = self.font_med.render("%s  (%d/%d)" % (tr(cat["label"]), lvls, mx), True, ACCENT)
            self.canvas.blit(cat_txt, (rect.x + 20, top + 42))
            top += 84

        content_rect = pygame.Rect(rect.x, top, rect.width, rect.bottom - top)
        scroll = self.right_panel.get_scroll()
        self.push_clip(content_rect)
        if cat is None:
            content_h = self.draw_tree_category_list(content_rect, scroll, mouse_pos)
        else:
            content_h = self.draw_tree_upgrade_list(content_rect, scroll, mouse_pos, cat["upgrades"])
        self.pop_clip()
        self.right_panel.set_max_scroll(max(0, content_h - content_rect.height))
        self.draw_scrollbar(content_rect, scroll, content_h, key="right", mouse_pos=mouse_pos)

    def draw_tree_category_list(self, content_rect, scroll, mouse_pos):
        """Ecrã inicial da Tree: um botão por categoria (Luck, Mutation Chance, Money...),
        com os níveis comprados e uma barrinha de progresso — igual ao das Milestones."""
        pad = 20
        row_w = content_rect.width - pad * 2 - 6
        row_h = 78
        y = content_rect.top + 6 - scroll

        for cat in UPGRADE_CATEGORIES:
            lvls, mx, ready = self.tree_category_stats(cat)
            done = lvls >= mx
            row_rect = pygame.Rect(content_rect.x + pad, y, row_w, row_h)

            if row_rect.bottom > content_rect.top and row_rect.top < content_rect.bottom:
                draw_panel(self.canvas, row_rect, PANEL_LIGHT, radius=12, shadow=False)
                if done:
                    draw_state_border(self.canvas, row_rect, (80, 150, 100), 12)
                elif row_rect.collidepoint(mouse_pos) and content_rect.collidepoint(mouse_pos):
                    draw_state_border(self.canvas, row_rect, ACCENT, 12)

                name_txt = self.font_med.render(tr(cat["label"]), True, WHITE)
                self.canvas.blit(bake(name_txt, PANEL_LIGHT), (row_rect.x + 16, row_rect.y + 10))

                count_txt = self.font_small_b.render("%d/%d" % (lvls, mx), True, GOOD if done else WHITE)
                self.canvas.blit(bake(count_txt, PANEL_LIGHT), (row_rect.right - count_txt.get_width() - 40, row_rect.y + 12))
                arrow_txt = self.font_med.render(">", True, GREY)
                self.canvas.blit(bake(arrow_txt, PANEL_LIGHT), (row_rect.right - 26, row_rect.centery - 12))

                # linha de baixo: descrição (à esquerda) e, se der para comprar algo, um aviso verde
                ready_txt = None
                if ready > 0:
                    ready_txt = self.font_tiny.render(tr("%d ready to buy", ready), True, GOOD)
                # o aviso verde fica à esquerda da seta ">" (que está encostada à direita da linha)
                max_desc_w = row_rect.width - 32 - (ready_txt.get_width() + 38 if ready_txt else 0)
                desc = fit_text(self.font_tiny, tr(cat["desc"]), max_desc_w)
                self.canvas.blit(bake(self.font_tiny.render(desc, True, GREY), PANEL_LIGHT), (row_rect.x + 16, row_rect.y + 36))
                if ready_txt:
                    self.canvas.blit(bake(ready_txt, PANEL_LIGHT), (row_rect.right - ready_txt.get_width() - 40, row_rect.y + 36))

                bar_rect = pygame.Rect(row_rect.x + 16, row_rect.bottom - 20, row_rect.width - 32, 10)
                pygame.draw.rect(self.canvas, PANEL, bar_rect, border_radius=5)
                frac = lvls / mx if mx else 0.0
                fill_w = max(0, int(bar_rect.width * frac))
                if fill_w > 0:
                    pygame.draw.rect(self.canvas, GOOD if done else ACCENT,
                                     pygame.Rect(bar_rect.x, bar_rect.y, fill_w, bar_rect.height),
                                     border_radius=5)

                self.register_button(row_rect, lambda k=cat["key"]: self.select_tree_category(k))

            y += row_h + 14

        return (y + scroll) - (content_rect.top + 6)

    def draw_tree_upgrade_list(self, content_rect, scroll, mouse_pos, keys):
        """Detalhe de uma categoria: uma cartão por upgrade, com o botão de comprar."""
        pad = 20
        row_w = content_rect.width - pad * 2 - 6
        y = content_rect.top + 6 - scroll

        for key in keys:
            d = UPGRADE_DEFS[key]
            lvl = self.state.upgrade_level(key)
            maxed = lvl >= d["max_level"]
            locked_by = self.state.upgrade_locked_by(key)
            need_rebirths = UPGRADE_DEFS[key].get("requires_rebirths", 0) if self.state.upgrade_rebirths_needed(key) else 0

            desc_lines = wrap_text(tr(d["desc"]), self.font_tiny, row_w - 28)
            effect = self.upgrade_effect_text(key)
            row_h = 34 + len(desc_lines) * 16 + (18 if effect else 0) + 44
            row_rect = pygame.Rect(content_rect.x + pad, y, row_w, row_h)

            if row_rect.bottom > content_rect.top and row_rect.top < content_rect.bottom:
                draw_panel(self.canvas, row_rect, PANEL_LIGHT, radius=10, shadow=False)
                if maxed:
                    draw_state_border(self.canvas, row_rect, (80, 150, 100), 10)

                name_txt = self.font_med.render(tr(d["name"]), True, WHITE if not (locked_by or need_rebirths) else GREY)
                self.canvas.blit(bake(name_txt, PANEL_LIGHT), (row_rect.x + 14, row_rect.y + 9))
                lvl_txt = self.font_small.render(tr("Lv %d/%d", lvl, d["max_level"]), True,
                                                 GOOD if maxed else GREY)
                self.canvas.blit(lvl_txt, (row_rect.right - lvl_txt.get_width() - 14, row_rect.y + 13))

                ly = row_rect.y + 34
                for line in desc_lines:
                    self.canvas.blit(self.font_tiny.render(line, True, GREY), (row_rect.x + 14, ly))
                    ly += 16
                if effect:
                    effect = fit_text(self.font_tiny, effect, row_rect.width - 28)
                    self.canvas.blit(self.font_tiny.render(effect, True, ACCENT), (row_rect.x + 14, ly + 2))
                    ly += 18

                btn_rect = pygame.Rect(row_rect.x + 14, row_rect.bottom - 38, row_rect.width - 28, 30)
                if maxed:
                    self.button(btn_rect, tr("MAX"), self.font_small_b, mouse_pos, (58, 78, 64), (58, 78, 64),
                                GOOD, enabled=False, radius=8)
                elif need_rebirths:
                    self.button(btn_rect, tr("Requires Rebirth %d", need_rebirths), self.font_small, mouse_pos,
                                (60, 62, 72), (60, 62, 72), GREY, enabled=False, radius=8)
                elif locked_by:
                    need = UPGRADE_DEFS[key].get("requires_level", 1)
                    req_name = tr(UPGRADE_DEFS[locked_by]["name"])
                    label = tr("Requires %s", req_name) if need <= 1 else tr("Requires %s Lv %d", req_name, need)
                    self.button(btn_rect, label, self.font_small, mouse_pos, (60, 62, 72), (60, 62, 72),
                                GREY, enabled=False, radius=8)
                else:
                    mode = self.buy_mode
                    n_levels, cost, affordable = self.state.upgrade_bulk_quote(key, mode)
                    if mode == 1:
                        label = tr("Buy  ($%s)", format_number(cost))
                    elif mode == "max":
                        label = (tr("Buy Max  x%d  ($%s)", n_levels, format_number(cost)) if affordable
                                 else tr("Buy Max  ($%s)", format_number(cost)))
                    else:
                        label = tr("Buy x%d  ($%s)", n_levels, format_number(cost))

                    def make_cb(k=key):
                        def cb():
                            got = self.state.buy_upgrade_bulk(k, self.buy_mode)
                            if got:
                                self.play("buy", min_gap=0.08)
                                name = tr(UPGRADE_DEFS[k]["name"])
                                self.show_toast(tr("%s upgraded!", name) if got == 1
                                                else tr("%s upgraded!  +%d levels", name, got))
                        return cb

                    self.button(btn_rect, label, self.font_small_b, mouse_pos,
                                ACCENT if affordable else (70, 73, 88), ACCENT_HOVER,
                                BLACK if affordable else GREY,
                                callback=make_cb() if affordable else None,
                                enabled=affordable, radius=8, sfx=None)

            y += row_h + 12

        return (y + scroll) - (content_rect.top + 6) + 10
