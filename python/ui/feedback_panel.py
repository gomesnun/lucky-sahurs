"""Ecrã de Feedback: o recado desta conta (escrever / editar / apagar) e os recados dos outros.
A lógica e as chamadas à cloud estão em online/feedback.py."""

import time
import pygame

from config import VIRTUAL_H
from i18n import tr
from theme import (
    ACCENT, ACCENT_HOVER, BAD, BLACK, GOOD, GREY, GREY_DIM,
    OUTLINE, PANEL, PANEL_LIGHT, PANEL_LIGHTER, WHITE,
)
from ui.drawing import bake, dim_overlay, draw_panel
from ui.fonts import fit_text, wrap_text


PANEL_W = 680
ROW_GAP = 8
EDITOR_H = 104
FEEDBACK_MAX_LEN = 280


class FeedbackPanelMixin:
    """Página de Feedback (abre-se no menu principal, ao lado do Update Log)."""

    # ================================================================ desenho
    def draw_feedback(self, mouse_pos):
        self.refresh_feedback()
        self.canvas.blit(dim_overlay(self.vw, VIRTUAL_H, 170), (0, 0))

        panel_w = min(PANEL_W, self.vw - 40)
        top = 40
        rect = pygame.Rect(self.vw // 2 - panel_w // 2, top, panel_w, VIRTUAL_H - top - 30)
        draw_panel(self.canvas, rect, PANEL, radius=16)
        self.register_button(rect, lambda: None, None)      # clicar dentro da página não a fecha

        title = self.font_big.render(tr("Feedback"), True, WHITE)
        self.canvas.blit(bake(title, PANEL), (rect.x + 26, rect.y + 16))
        sub = self.font_small.render(tr("One message per player - you can edit or delete yours."),
                                     True, GREY)
        self.canvas.blit(bake(sub, PANEL), (rect.x + 26, rect.y + 18 + title.get_height()))
        close_rect = pygame.Rect(rect.right - 48, rect.y + 18, 30, 30)
        self.button(close_rect, "X", self.font_small_b, mouse_pos, PANEL_LIGHT, BAD, WHITE,
                    callback=self.close_feedback, radius=8)

        body_top = rect.y + 24 + title.get_height() + sub.get_height()
        body = pygame.Rect(rect.x + 22, body_top, panel_w - 44, rect.bottom - 20 - body_top)
        if not self.feedback_ready():
            self.draw_feedback_message(body, tr("Log in from the main menu to leave feedback."))
            return

        mine_h = self.draw_feedback_mine(body, mouse_pos)
        foot_h = self.draw_feedback_footer(rect, mouse_pos)
        list_rect = pygame.Rect(body.x, body.y + mine_h + 14, body.width,
                                rect.bottom - 20 - foot_h - (body.y + mine_h + 14))
        self.feedback_list_rect = list_rect
        self.draw_feedback_list(list_rect, mouse_pos)

    def draw_feedback_message(self, rect, text):
        t = self.font_med.render(text, True, GREY)
        self.canvas.blit(t, t.get_rect(center=rect.center))

    # ---------------------------------------------------------------- o recado desta conta
    def draw_feedback_mine(self, body, mouse_pos):
        """Cartão de cima: escrever, ou o que já está publicado com Edit / Delete. Devolve a altura."""
        if self.feedback_editing:
            box = pygame.Rect(body.x, body.y, body.width, EDITOR_H)
            self.draw_text_area(box, self.feedback_field, tr("What would make the game better?"),
                                self.feedback_focus, lambda: self.set_feedback_focus(True))
            left = self.font_tiny.render("%d/%d" % (len(self.feedback_field.text), FEEDBACK_MAX_LEN),
                                         True, GREY_DIM)
            self.canvas.blit(left, (box.right - left.get_width() - 12, box.bottom - 18))
            btn_y = box.bottom + 8
            bw = (body.width - 10) // 2
            self.button(pygame.Rect(body.x, btn_y, bw, 38),
                        tr("Sending...") if self.feedback_busy else tr("Publish"),
                        self.font_small_b, mouse_pos, ACCENT, ACCENT_HOVER, BLACK,
                        callback=None if self.feedback_busy else self.submit_feedback, radius=10)
            self.button(pygame.Rect(body.x + bw + 10, btn_y, bw, 38), tr("Cancel"),
                        self.font_small_b, mouse_pos, PANEL_LIGHT, PANEL_LIGHTER, WHITE,
                        callback=self.cancel_feedback_edit, radius=10)
            return EDITOR_H + 8 + 38

        if not self.feedback_mine:
            box = pygame.Rect(body.x, body.y, body.width, 56)
            pygame.draw.rect(self.canvas, PANEL_LIGHT, box, border_radius=12)
            pygame.draw.rect(self.canvas, OUTLINE, box, width=2, border_radius=12)
            t = self.font_small_b.render(tr("You haven't left feedback yet."), True, GREY)
            self.canvas.blit(t, (box.x + 14, box.centery - t.get_height() // 2))
            self.button(pygame.Rect(box.right - 152, box.y + 9, 140, box.height - 18), tr("Write feedback"),
                        self.font_small_b, mouse_pos, ACCENT, ACCENT_HOVER, BLACK,
                        callback=self.start_feedback_edit, radius=10)
            return box.height

        # já publicado: mostra-se o texto com Edit / Delete
        lines = wrap_text(self.feedback_mine["text"], self.font_small_b, body.width - 28)[:3]
        h = max(64, 34 + len(lines) * (self.font_small_b.get_height() + 3) + 44)
        box = pygame.Rect(body.x, body.y, body.width, h)
        pygame.draw.rect(self.canvas, PANEL_LIGHT, box, border_radius=12)
        pygame.draw.rect(self.canvas, ACCENT, box, width=2, border_radius=12)
        head = self.font_small_b.render(tr("Your feedback"), True, ACCENT)
        self.canvas.blit(head, (box.x + 14, box.y + 10))
        y = box.y + 12 + head.get_height()
        for line in lines:
            self.canvas.blit(self.font_small_b.render(line, True, WHITE), (box.x + 14, y))
            y += self.font_small_b.get_height() + 3

        bw = (box.width - 28 - 10) // 2
        by = box.bottom - 42
        self.button(pygame.Rect(box.x + 14, by, bw, 32), tr("Edit"), self.font_small_b, mouse_pos,
                    PANEL_LIGHTER, ACCENT_HOVER, WHITE,
                    callback=None if self.feedback_busy else self.start_feedback_edit, radius=9)
        if self.feedback_confirm_delete:
            self.button(pygame.Rect(box.x + 24 + bw, by, bw, 32), tr("Confirm delete"),
                        self.font_small_b, mouse_pos, (150, 60, 60), BAD, WHITE,
                        callback=None if self.feedback_busy else self.delete_feedback, radius=9)
        else:
            self.button(pygame.Rect(box.x + 24 + bw, by, bw, 32), tr("Delete"),
                        self.font_small_b, mouse_pos, PANEL_LIGHTER, BAD, WHITE,
                        callback=None if self.feedback_busy else self.ask_delete_feedback, radius=9)
        return h

    # ---------------------------------------------------------------- lista de todos
    def feedback_card_height(self, entry, width):
        lines = wrap_text(entry["text"], self.font_small_b, width - 28)[:4]
        return 30 + len(lines) * (self.font_small_b.get_height() + 3) + 12

    def draw_feedback_card(self, rect, entry):
        mine = bool(self.account) and entry["uid"] == self.account.get("uid")
        pygame.draw.rect(self.canvas, PANEL_LIGHT, rect, border_radius=12)
        pygame.draw.rect(self.canvas, ACCENT if mine else OUTLINE, rect, width=2, border_radius=12)
        name = self.font_small_b.render(fit_text(self.font_small_b, entry["username"], rect.width - 140),
                                        True, ACCENT if mine else WHITE)
        self.canvas.blit(name, (rect.x + 14, rect.y + 8))
        when = feedback_when(entry.get("updated_at"))
        if when:
            t = self.font_tiny.render(when, True, GREY_DIM)
            self.canvas.blit(t, (rect.right - t.get_width() - 14, rect.y + 11))
        y = rect.y + 10 + name.get_height()
        for line in wrap_text(entry["text"], self.font_small_b, rect.width - 28)[:4]:
            self.canvas.blit(self.font_small_b.render(line, True, GREY), (rect.x + 14, y))
            y += self.font_small_b.get_height() + 3

    def draw_feedback_list(self, rect, mouse_pos):
        # o recado desta conta já aparece no cartão de cima, não se repete na lista
        uid = self.account.get("uid") if self.account else None
        entries = [e for e in self.feedback_list if e["uid"] != uid]
        if not entries:
            self.draw_feedback_message(rect, tr("Loading...") if self.feedback_loading
                                       else tr("No feedback yet. Be the first!"))
            self.feedback_max_scroll = 0.0
            return
        scroll = self.feedback_scroll
        self.push_clip(rect)
        y = rect.top - scroll
        width = rect.width - 16
        content_h = 0
        for entry in entries:
            h = self.feedback_card_height(entry, width)
            card = pygame.Rect(rect.x, y, width, h)
            if card.bottom >= rect.top - 4 and card.top <= rect.bottom + 4:
                self.draw_feedback_card(card, entry)
            y += h + ROW_GAP
            content_h += h + ROW_GAP
        content_h = max(0, content_h - ROW_GAP)
        self.pop_clip()
        self.feedback_max_scroll = max(0.0, content_h - rect.height)
        self.feedback_scroll = max(0.0, min(self.feedback_max_scroll, self.feedback_scroll))
        self.draw_scrollbar(rect, scroll, content_h, key="feedback", mouse_pos=mouse_pos)

    # ---------------------------------------------------------------- rodapé
    def draw_feedback_footer(self, rect, mouse_pos):
        foot_h = 44
        y = rect.bottom - 18 - 34
        self.button(pygame.Rect(rect.x + 22, y, 120, 34), tr("Refresh"), self.font_small_b, mouse_pos,
                    PANEL_LIGHT, PANEL_LIGHTER, WHITE,
                    callback=lambda: self.refresh_feedback(force=True), radius=9)
        if self.feedback_msg:
            text, color = self.feedback_msg
        elif self.feedback_error:
            text, color = self.feedback_error, BAD
        elif self.feedback_loading:
            text, color = tr("Updating..."), GREY_DIM
        else:
            text, color = "", GREY_DIM
        if text:
            t = self.font_small.render(fit_text(self.font_small, text, rect.width - 190), True, color)
            self.canvas.blit(t, (rect.right - 22 - t.get_width(), y + 17 - t.get_height() // 2))
        return foot_h


def feedback_when(stamp):
    """'há 3 min' / 'há 2 h' / 'há 4 d' - o mesmo estilo curto da leaderboard."""
    if not stamp:
        return ""
    secs = max(0.0, time.time() - stamp)
    if secs < 90:
        return tr("just now")
    if secs < 3600:
        return tr("%d min ago", int(secs // 60))
    if secs < 86400:
        return tr("%d h ago", int(secs // 3600))
    return tr("%d d ago", int(secs // 86400))
