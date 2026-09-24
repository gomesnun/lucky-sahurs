"""Ecrã da leaderboard."""

import time
import pygame

from config import TOPBAR_H, VIRTUAL_H
from core.formatting import format_number, format_playtime
from i18n import tr, tr_short
from online.cloud_cache import (
    fetch_leaderboards, lb_data_complete, lb_fetch_period, lb_next_update, store_lb_cache,
)
from online.firebase import LEADERBOARD_PERIOD, LEADERBOARD_SIZE, online_error_text
from theme import (
    ACCENT, ACCENT_HOVER, BAD, BLACK,
    GOOD, GREY, GREY_DIM, PANEL,
    PANEL_LIGHT, PANEL_LIGHTER, WHITE,
)
from ui.drawing import bake, dim_overlay, draw_panel
from ui.fonts import fit_text, wrap_text
from ui.icons import load_icon


# separadores da leaderboard: (chave, texto do botão, título da coluna do valor, mensagem se estiver vazia)
LB_TABS = (
    ("money", "Money", "Total coins earned", "No scores yet - be the first!"),
    ("playtime", "Playtime", "Total playtime", "No scores yet - be the first!"),
    ("rolls", "Rolls", "Total rolls", "No scores yet - be the first!"),
    ("rebirths", "Rebirths", "Rebirths", "Nobody has rebirthed yet - be the first!"),
)


# ícone de cada separador (icons/<nome>.png): dinheiro, relógio, dado e rebirth
LB_ICONS = {"money": "cash", "playtime": "clock", "rolls": "dice", "rebirths": "rebirth"}
LB_VALUE_ICON = 30


class LeaderboardPanelMixin:
    """Ecrã da leaderboard."""

    # ================================================================ ONLINE: leaderboard
    # A leaderboard é uma página como a Stats / Traits / Options: só uma delas aberta de cada vez.
    def open_leaderboard(self):
        self.close_overlays()
        self.leaderboard_open = True
        self.lb_scroll = 0.0
        self.ensure_leaderboard()

    def close_leaderboard(self):
        self.leaderboard_open = False

    def toggle_leaderboard(self):
        if self.leaderboard_open:
            self.close_leaderboard()
        else:
            self.open_leaderboard()

    def set_lb_tab(self, tab):
        self.lb_tab = tab
        self.lb_scroll = 0.0

    def lb_login(self):
        self.leaderboard_open = False
        self.open_account()

    def lb_retry(self):
        self.ensure_leaderboard(force=True)

    def ensure_leaderboard(self, force=False):
        """Vai buscar as leaderboards se já passou uma marca de 10 min desde a última vez."""
        if not self.client or self.lb_loading:
            return
        now = time.time()
        period = lb_fetch_period(now)
        if not force:
            fresh = bool(self.lb_data) and self.lb_data.get("period", -1) >= period and lb_data_complete(self.lb_data)
            if fresh and self.account and self.pub_last_time != self.lb_refetch_done:
                # Publiquei a minha pontuação DEPOIS de esta fotografia ter sido tirada (jogador novo, ou publicação
                # que chegou tarde): a fotografia não me inclui e só apareceria no período seguinte. Pede-se de novo,
                # uma única vez por publicação (lb_refetch_done evita pedidos em ciclo, mesmo que o relógio salte).
                try:
                    taken = float(self.lb_data.get("fetched_at") or 0.0)
                except (TypeError, ValueError):
                    taken = 0.0
                if self.pub_last_time > taken:
                    fresh = False
            if fresh:
                return
            if now < self.lb_retry_at:
                return
        self.lb_refetch_done = self.pub_last_time
        self.lb_loading = True
        self.lb_error = None

        def ok(res):
            self.lb_loading = False
            self.lb_data = res
            self.lb_retry_at = 0.0
            store_lb_cache(res)

        def err(e):
            self.lb_loading = False
            self.lb_error = online_error_text(e)
            self.lb_retry_at = float("inf")           # só volta a tentar com o botão Retry

        self.worker.run(lambda: fetch_leaderboards(self.client, period), ok, err)

    def lb_format(self, value):
        if self.lb_tab == "money":
            return "$" + format_number(value)
        if self.lb_tab == "playtime":
            return format_playtime(value)
        return format_number(value)             # rolls / rebirths

    def lb_tab_info(self):
        for tab in LB_TABS:
            if tab[0] == self.lb_tab:
                return tab
        return LB_TABS[0]

    def draw_leaderboard(self, mouse_pos):
        self.ensure_leaderboard()
        # o mesmo veu escuro das outras paginas: em cache e, no telemovel, opaco
        # (uma superficie do tamanho do ecra com alfa custava ~182 ms por frame)
        self.canvas.blit(dim_overlay(self.vw, VIRTUAL_H, 170), (0, 0))

        panel_w = 640
        top = TOPBAR_H + 12                    # por baixo da barra do topo (não tapa Stats / Options)
        panel_h = VIRTUAL_H - top - 14
        rect = pygame.Rect(self.vw // 2 - panel_w // 2, top, panel_w, panel_h)
        draw_panel(self.canvas, rect, PANEL, radius=16)
        self.register_button(rect, lambda: None, None)     # clicar dentro da página não fecha

        title = self.font_big.render(tr("Leaderboard"), True, WHITE)
        self.canvas.blit(bake(title, PANEL), (rect.x + 26, rect.y + 16))
        close_rect = pygame.Rect(rect.right - 48, rect.y + 18, 30, 30)
        self.button(close_rect, "X", self.font_small_b, mouse_pos, PANEL_LIGHT, BAD, WHITE,
                    callback=self.close_leaderboard, radius=8)
        sub_y = rect.y + 16 + title.get_height() + 2
        sub = fit_text(self.font_small, tr("Top %d players - updates every %d minutes.",
                                           LEADERBOARD_SIZE, LEADERBOARD_PERIOD // 60),
                       panel_w - 52 - 60)
        self.canvas.blit(bake(self.font_small.render(sub, True, GREY), PANEL), (rect.x + 26, sub_y))

        # separadores Money / Playtime / Rolls / Rebirths
        tabs_y = sub_y + self.font_small.get_height() + 12
        tab_gap = 10
        tab_w = (panel_w - 44 - tab_gap * (len(LB_TABS) - 1)) // len(LB_TABS)
        for i, (key, label, _col, _empty) in enumerate(LB_TABS):
            label = tr_short(label)              # nas abas (apertadas) usa a forma curta, se existir
            trect = pygame.Rect(rect.x + 22 + i * (tab_w + tab_gap), tabs_y, tab_w, 40)
            active = self.lb_tab == key
            # o texto + o ícone têm de caber no separador: se não couberem, usa a letra mais pequena
            tab_font = self.font_med if self.font_med.size(label)[0] + 8 + (trect.height - 12) <= trect.width - 16 \
                else self.font_small_b
            self.button(trect, label, tab_font, mouse_pos,
                        ACCENT if active else PANEL_LIGHT, ACCENT_HOVER if active else PANEL_LIGHTER,
                        BLACK if active else WHITE, callback=lambda k=key: self.set_lb_tab(k), radius=10,
                        icon=LB_ICONS.get(key))

        # cabeçalho da tabela
        head_y = tabs_y + 40 + 14
        value_label = tr(self.lb_tab_info()[2])
        self.canvas.blit(self.font_small_b.render("#", True, GREY_DIM), (rect.x + 34, head_y))
        self.canvas.blit(self.font_small_b.render(tr("Player"), True, GREY_DIM), (rect.x + 96, head_y))
        vh = self.font_small_b.render(value_label, True, GREY_DIM)
        self.canvas.blit(vh, (rect.right - 38 - vh.get_width(), head_y))
        line_y = head_y + self.font_small_b.get_height() + 4
        pygame.draw.line(self.canvas, PANEL_LIGHT, (rect.x + 22, line_y), (rect.right - 22, line_y), 2)

        # rodapé: voltar às Options + estado da atualização
        back_rect = pygame.Rect(rect.x + 22, rect.bottom - 22 - 40, 210, 40)
        self.button(back_rect, tr("Back to Options"), self.font_small_b, mouse_pos,
                    PANEL_LIGHT, PANEL_LIGHTER, WHITE, callback=self.open_options, radius=10)
        if self.lb_error:
            self.button(pygame.Rect(back_rect.right + 10, back_rect.y, 84, 40), tr("Retry"), self.font_small_b,
                        mouse_pos, ACCENT, ACCENT_HOVER, BLACK, callback=self.lb_retry, radius=10)
        now = time.time()
        remaining = max(0, int(lb_next_update(now) - now))
        if self.lb_loading:
            line1, line2 = tr("Updating..."), ""
        elif self.lb_error:
            line1, line2 = tr("Couldn't update."), fit_text(self.font_tiny, self.lb_error, 250)
        else:
            line1 = tr("Updated %s", time.strftime("%H:%M", time.localtime(self.lb_data["fetched_at"]))) \
                if self.lb_data and self.lb_data.get("fetched_at") else ""
            line2 = tr("Next update in %d:%02d", remaining // 60, remaining % 60)
        fy = back_rect.y + 2
        for text, font, color in ((line1, self.font_small, GREY), (line2, self.font_tiny, GREY_DIM)):
            if text:
                t = font.render(text, True, color)
                self.canvas.blit(t, (rect.right - 26 - t.get_width(), fy))
            fy += 19

        # a tua linha, fixa por cima do rodapé
        own_rect = pygame.Rect(rect.x + 22, back_rect.y - 10 - 44, panel_w - 44, 44)
        entries = list(self.lb_data.get(self.lb_tab, [])) if self.lb_data else []
        own_name = self.account["username"] if self.account else None
        self.draw_lb_own_row(own_rect, entries, own_name, mouse_pos)

        # lista com scroll (fica uma faixa livre à direita para a barra de scroll)
        list_rect = pygame.Rect(rect.x + 22, line_y + 8, panel_w - 44, own_rect.y - 10 - (line_y + 8))
        self.lb_list_rect = list_rect
        if not self.client:
            self.draw_lb_message(list_rect, tr("Online features aren't set up yet - see FIREBASE_SETUP.md."))
        elif not entries:
            if self.lb_loading:
                self.draw_lb_message(list_rect, tr("Loading leaderboard..."))
            elif self.lb_error:
                self.draw_lb_message(list_rect, tr("The leaderboard couldn't be loaded."))
            else:
                self.draw_lb_message(list_rect, tr(self.lb_tab_info()[3]))
        else:
            self.draw_lb_rows(list_rect, entries, own_name, mouse_pos)

    def draw_lb_message(self, rect, text):
        self.lb_max_scroll = 0.0
        y = rect.centery - 20
        for line in wrap_text(text, self.font_med, rect.width - 60):
            t = self.font_med.render(line, True, GREY)
            self.canvas.blit(t, t.get_rect(center=(rect.centerx, y)))
            y += 26

    def draw_lb_rows(self, rect, entries, own_name, mouse_pos):
        row_h, gap = 44, 4
        row_w = rect.width - 16
        scroll = self.lb_scroll
        medal = {1: (255, 205, 60), 2: (214, 218, 228), 3: (214, 148, 96)}
        self.push_clip(rect)
        y = rect.top - scroll
        for i, e in enumerate(entries):
            rrect = pygame.Rect(rect.x, y, row_w, row_h)
            if rrect.bottom >= rect.top - 4 and rrect.top <= rect.bottom + 4:
                is_own = own_name is not None and e["username"] == own_name
                pygame.draw.rect(self.canvas, PANEL_LIGHTER if is_own else PANEL_LIGHT, rrect, border_radius=10)
                if is_own:
                    pygame.draw.rect(self.canvas, ACCENT, rrect, width=3, border_radius=10)
                rank = i + 1
                rt = self.font_med.render("%d" % rank, True, medal.get(rank, WHITE))
                self.canvas.blit(rt, (rrect.x + 14, rrect.centery - rt.get_height() // 2))
                value = self.font_med.render(self.lb_format(e["value"]), True,
                                             GOOD if self.lb_tab == "money" else WHITE)
                self.canvas.blit(value, (rrect.right - 16 - value.get_width(), rrect.centery - value.get_height() // 2))
                icon_w = self.draw_lb_value_icon(rrect.right - 16 - value.get_width() - 6, rrect.centery)
                name = fit_text(self.font_med, e["username"], rrect.width - 74 - value.get_width() - 40 - icon_w)
                nt = self.font_med.render(name, True, ACCENT if is_own else WHITE)
                self.canvas.blit(nt, (rrect.x + 74, rrect.centery - nt.get_height() // 2))
            y += row_h + gap
        content_h = len(entries) * (row_h + gap) - gap
        self.lb_max_scroll = max(0.0, content_h - rect.height)
        self.lb_scroll = max(0.0, min(self.lb_max_scroll, self.lb_scroll))
        self.pop_clip()
        self.draw_scrollbar(rect, scroll, content_h, key="leaderboard", mouse_pos=mouse_pos)

    def draw_lb_value_icon(self, right, cy):
        """Ícone da leaderboard atual (dinheiro / relógio / dado / rebirth) encostado à esquerda de x = 'right'.
        Devolve a largura ocupada (0 se o ícone não existir)."""
        img = load_icon(LB_ICONS.get(self.lb_tab, ""), LB_VALUE_ICON)
        if img is None:
            return 0
        self.canvas.blit(img, img.get_rect(midright=(right, cy)))
        return img.get_width() + 6

    def draw_lb_own_row(self, rect, entries, own_name, mouse_pos):
        pygame.draw.rect(self.canvas, PANEL_LIGHT, rect, border_radius=10)
        if own_name is None:
            can_login = self.client is not None and self.screen_mode in ("title", "saves")
            text = tr("Log in to appear on the leaderboard.") if can_login else \
                   tr("Log in from the main menu to appear on the leaderboard.")
            t = self.font_small.render(fit_text(self.font_small, text, rect.width - (150 if can_login else 30)),
                                       True, GREY)
            self.canvas.blit(bake(t, PANEL_LIGHT), (rect.x + 16, rect.centery - t.get_height() // 2))
            if can_login:
                self.button(pygame.Rect(rect.right - 122, rect.y + 6, 112, rect.height - 12), tr("Log in"),
                            self.font_small_b, mouse_pos, ACCENT, ACCENT_HOVER, BLACK,
                            callback=self.lb_login, radius=8)
            return
        for i, e in enumerate(entries):
            if e["username"] == own_name:
                pygame.draw.rect(self.canvas, ACCENT, rect, width=3, border_radius=10)
                head = self.font_med.render(tr("You: #%d", i + 1), True, ACCENT)
                self.canvas.blit(head, (rect.x + 16, rect.centery - head.get_height() // 2))
                val = self.font_med.render(self.lb_format(e["value"]), True,
                                           GOOD if self.lb_tab == "money" else WHITE)
                self.canvas.blit(val, (rect.right - 16 - val.get_width(), rect.centery - val.get_height() // 2))
                self.draw_lb_value_icon(rect.right - 16 - val.get_width() - 6, rect.centery)
                return
        if self.lb_tab == "rebirths":
            text = tr("You: not on this board yet - do a Rebirth!")
        else:
            text = tr("You: not in the top %d yet - keep playing!", LEADERBOARD_SIZE)
        t = self.font_small.render(fit_text(self.font_small, text, rect.width - 30), True, GREY)
        self.canvas.blit(t, (rect.x + 16, rect.centery - t.get_height() // 2))
