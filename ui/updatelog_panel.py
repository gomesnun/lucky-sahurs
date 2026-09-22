"""Página de Update Log: histórico de novidades do jogo (abre-se no menu principal).

Acrescenta entradas a UPDATE_LOG (mais recente primeiro) quando houver novidades para mostrar.
Cada entrada é (versão, data, [linhas de texto])."""

import pygame

from config import VIRTUAL_H
from i18n import tr
from theme import BAD, GREY, GREY_DIM, PANEL, PANEL_LIGHT, WHITE
from ui.drawing import bake, dim_overlay, draw_panel
from ui.fonts import wrap_text

# (versão, data, linhas de descrição) - a mais recente primeiro.
UPDATE_LOG = (
    ("v2.3.0", "2026-09-22", [
        "Feedback: the new button next to Update Log opens a page where you leave one message for the "
        "game - you can edit it or delete it, and after deleting you can write another one.",
        "Android: everything online (account, leaderboard, friends, feedback) was failing on the phone "
        "because the app had no HTTPS certificates. Fixed.",
    ]),
    ("v2.2.1", "2026-09-22", [
        "Friend search now also finds players who have not opened the Friends page yet.",
        "Typing anywhere in the game: arrow keys, Home/End and Delete move the cursor, and clicking "
        "inside a box puts the cursor where you clicked.",
        "No more stray space at the end of what you type (it was breaking the friend search).",
    ]),
    ("v2.2.0", "2026-09-22", [
        "Friends: the new button next to Stats opens a page where you search a player by username, "
        "send a friend request and, once they accept, see their stats.",
        "Profile photos: any Verity you already own can be your photo - Golden and Diamond keep their "
        "coloured ring.",
        "A red dot on the Friends button means someone is waiting for an answer.",
    ]),
    ("v2.1.0", "2026-09-22", [
        "The game is now Lucky Verities: new name, new art, and 22 Verities to collect.",
        "Your saves come with you - the first time you open it, everything is copied over from the "
        "old Lucky Sahurs folder.",
        "New cutscenes when a rare Verity shows up, and the Options page is now split into tabs.",
        "On the phone this installs as a new app next to the old one (Android needs it that way for the "
        "rename): log in to your account to bring your save across, then you can remove the old icon.",
    ]),
    ("v2.0.0", "2026-09-22", [
        "The Android build updates itself now: when a new version comes out the game downloads it and "
        "asks Android to install it, the same way the computer version does.",
        "The first time, Android asks you to allow the game to install apps - say yes and tap Try again.",
    ]),
    ("v1.0.5", "2026-09-22", [
        "Android: the game now runs at 60 FPS on the phone.",
        "The main screen was drawing the pet card and its glow pixel by pixel every frame "
        "(34 ms of the 40 ms each frame took); they are now blended into the background once.",
        "Blended images that nothing asks for any more are thrown away, so a long session no "
        "longer gets slower and slower.",
        "The phone build is on the Releases page: Lucky-Verities-Android.apk.",
    ]),
)

EMPTY_NOTE = "Nothing here yet - future updates will be listed on this page."


class UpdateLogPanelMixin:
    """Página de Update Log (abre-se no menu principal)."""

    def open_update_log(self):
        self.close_overlays()
        self.update_log_open = True

    def close_update_log(self):
        self.update_log_open = False

    def toggle_update_log(self):
        if self.update_log_open:
            self.close_update_log()
        else:
            self.open_update_log()

    def draw_update_log(self, mouse_pos):
        self.canvas.blit(dim_overlay(self.vw, VIRTUAL_H, 170), (0, 0))

        # ---- medidas: tudo calculado a partir do tamanho real das fontes (nada de números "à sorte") ----
        panel_w = 580
        pad = 28                      # margem interior do painel
        box_pad = 18                  # margem interior das caixas de conteúdo
        title = self.font_big.render(tr("Update Log"), True, WHITE)
        sub = self.font_small.render(tr("What's new in Lucky Verities"), True, GREY)
        title_y = 24
        sub_y = title_y + title.get_height() + 6
        head_h = sub_y + sub.get_height() + 22          # onde começa o conteúdo (relativo ao topo do painel)

        inner_w = panel_w - pad * 2
        if UPDATE_LOG:
            entries = []
            for version, date, lines in UPDATE_LOG:
                wrapped = []
                for line in lines:
                    wrapped.extend(wrap_text("- " + tr(line), self.font_small, inner_w - box_pad * 2))
                row_h = box_pad + self.font_med.get_height() + 10 + 22 * len(wrapped) + box_pad - 4
                entries.append((version, date, wrapped, row_h))
            gap = 14
            body_h = sum(e[3] for e in entries) + gap * (len(entries) - 1)
        else:
            note_lines = wrap_text(tr(EMPTY_NOTE), self.font_small, inner_w - box_pad * 2)
            body_h = box_pad * 2 + 24 * len(note_lines) - 4

        panel_h = head_h + body_h + pad
        rect = pygame.Rect(self.vw // 2 - panel_w // 2, VIRTUAL_H // 2 - panel_h // 2, panel_w, panel_h)
        draw_panel(self.canvas, rect, PANEL, radius=16)
        self.register_button(rect, lambda: None, None)     # clicar dentro da página não fecha

        self.canvas.blit(bake(title, PANEL), (rect.x + pad, rect.y + title_y))
        self.canvas.blit(bake(sub, PANEL), (rect.x + pad + 2, rect.y + sub_y))
        close_rect = pygame.Rect(rect.right - pad - 32, rect.y + title_y + 2, 32, 32)
        self.button(close_rect, "X", self.font_small_b, mouse_pos, PANEL_LIGHT, BAD, WHITE,
                    callback=self.close_update_log, radius=8)

        y = rect.y + head_h
        if not UPDATE_LOG:
            box = pygame.Rect(rect.x + pad, y, inner_w, body_h)
            pygame.draw.rect(self.canvas, PANEL_LIGHT, box, border_radius=12)
            ny = box.y + box_pad
            for line in note_lines:
                t_surf = self.font_small.render(line, True, GREY_DIM)
                self.canvas.blit(bake(t_surf, PANEL_LIGHT), t_surf.get_rect(midtop=(box.centerx, ny)))
                ny += 24
            return

        for version, date, wrapped, row_h in entries:
            row = pygame.Rect(rect.x + pad, y, inner_w, row_h)
            pygame.draw.rect(self.canvas, PANEL_LIGHT, row, border_radius=12)
            head = self.font_med.render(version, True, WHITE)
            self.canvas.blit(head, (row.x + box_pad, row.y + box_pad - 2))
            dtxt = self.font_tiny.render(date, True, GREY_DIM)
            self.canvas.blit(dtxt, (row.right - box_pad - dtxt.get_width(),
                                    row.y + box_pad - 2 + (head.get_height() - dtxt.get_height()) // 2))
            ny = row.y + box_pad - 2 + head.get_height() + 10
            for wrapped_line in wrapped:
                t_surf = self.font_small.render(wrapped_line, True, GREY)
                self.canvas.blit(t_surf, (row.x + box_pad, ny))
                ny += 22
            y += row_h + gap
