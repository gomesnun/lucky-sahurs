"""Evento global: a faixa que toda a gente ve enquanto ele dura, e o painel de quem o pode
comecar (a lógica está em online/events.py)."""

import pygame

from config import VIRTUAL_H, TOPBAR_H
from i18n import tr
from online.events import EVENT_KINDS, EVENT_MINUTES, EVENT_MULTS, event_kind_label
from theme import (
    ACCENT, ACCENT_HOVER, BAD, BLACK, GOOD, GREY, GREY_DIM,
    OUTLINE, PANEL, PANEL_LIGHT, PANEL_LIGHTER, WHITE,
)
from ui.drawing import bake, dim_overlay, draw_panel
from ui.fonts import fit_text


class EventsPanelMixin:
    """Faixa do evento (em cima) + página de admin."""

    # ---------------------------------------------------------------- faixa do evento
    def event_banner_text(self):
        ev = self.active_event()
        if not ev:
            return None
        left = int(self.event_seconds_left())
        clock = "%d:%02d" % (left // 60, left % 60)
        return tr("GLOBAL EVENT: %sx %s - %s left", ("%g" % ev["mult"]),
                  event_kind_label(ev["kind"]), clock)

    def draw_event_banner(self):
        """Faixa fina por baixo da topbar. Não é clicável: é só para se ver."""
        text = self.event_banner_text()
        if not text:
            return
        h = 26
        rect = pygame.Rect(0, TOPBAR_H + 2, self.vw, h)
        pygame.draw.rect(self.canvas, ACCENT, rect)
        t = self.font_small_b.render(fit_text(self.font_small_b, text, self.vw - 24), True, BLACK)
        self.canvas.blit(t, t.get_rect(center=rect.center))

    # ---------------------------------------------------------------- página de admin
    def draw_event_admin(self, mouse_pos):
        self.canvas.blit(dim_overlay(self.vw, VIRTUAL_H, 170), (0, 0))
        panel_w = min(520, self.vw - 40)
        panel_h = 430
        rect = pygame.Rect(self.vw // 2 - panel_w // 2, max(20, VIRTUAL_H // 2 - panel_h // 2),
                           panel_w, panel_h)
        draw_panel(self.canvas, rect, PANEL, radius=16)
        self.register_button(rect, lambda: None, None)

        title = self.font_big.render(tr("Global event"), True, WHITE)
        self.canvas.blit(bake(title, PANEL), (rect.x + 24, rect.y + 18))
        self.button(pygame.Rect(rect.right - 46, rect.y + 20, 28, 28), "X", self.font_small_b, mouse_pos,
                    PANEL_LIGHT, BAD, WHITE, callback=self.close_event_admin, radius=8)
        sub = self.font_small.render(tr("Starts a bonus for every player online."), True, GREY)
        self.canvas.blit(bake(sub, PANEL), (rect.x + 24, rect.y + 20 + title.get_height()))

        x0 = rect.x + 24
        w = panel_w - 48
        y = rect.y + 30 + title.get_height() + sub.get_height()

        # ---- multiplicador ----
        y = self.draw_event_choices(x0, y, w, tr("Multiplier"), mouse_pos,
                                    [("%gx" % m, m) for m in EVENT_MULTS],
                                    self.event_admin_mult, self.set_event_mult)
        # ---- duração ----
        y = self.draw_event_choices(x0, y, w, tr("Duration"), mouse_pos,
                                    [(tr("%d min", m), m) for m in EVENT_MINUTES],
                                    self.event_admin_minutes, self.set_event_minutes)

        # ---- começar (um botão por tipo) ----
        label = self.font_small_b.render(tr("Start for everyone"), True, GREY_DIM)
        self.canvas.blit(bake(label, PANEL), (x0, y))
        y += label.get_height() + 6
        gap = 8
        bw = (w - gap * (len(EVENT_KINDS) - 1)) // len(EVENT_KINDS)
        for i, kind in enumerate(EVENT_KINDS):
            self.button(pygame.Rect(x0 + i * (bw + gap), y, bw, 44), event_kind_label(kind),
                        self.font_small_b, mouse_pos, ACCENT, ACCENT_HOVER, BLACK,
                        callback=None if self.event_busy else (lambda k=kind: self.start_event(k)),
                        radius=10)
        y += 56

        # ---- o que está a decorrer ----
        ev = self.active_event()
        if ev:
            info = tr("Now: %sx %s (%d s left), by %s", "%g" % ev["mult"],
                      event_kind_label(ev["kind"]), int(self.event_seconds_left()), ev.get("by") or "?")
            t = self.font_small.render(fit_text(self.font_small, info, w), True, ACCENT)
            self.canvas.blit(bake(t, PANEL), (x0, y))
            y += t.get_height() + 8
            self.button(pygame.Rect(x0, y, w, 40), tr("Stop event"), self.font_small_b, mouse_pos,
                        (150, 60, 60), BAD, WHITE,
                        callback=None if self.event_busy else self.stop_event, radius=10)
        else:
            t = self.font_small.render(tr("No event running."), True, GREY)
            self.canvas.blit(bake(t, PANEL), (x0, y))

        if self.event_msg:
            text, color = self.event_msg
            t = self.font_small.render(fit_text(self.font_small, text, w), True, color)
            self.canvas.blit(bake(t, PANEL), (x0, rect.bottom - 30))

    def draw_event_choices(self, x0, y, w, label, mouse_pos, options, chosen, on_pick):
        """Uma linha de botões onde só um fica aceso. Devolve o y a seguir."""
        t = self.font_small_b.render(label, True, GREY_DIM)
        self.canvas.blit(bake(t, PANEL), (x0, y))
        y += t.get_height() + 6
        gap = 8
        bw = (w - gap * (len(options) - 1)) // len(options)
        for i, (text, value) in enumerate(options):
            on = value == chosen
            self.button(pygame.Rect(x0 + i * (bw + gap), y, bw, 40), text, self.font_small_b, mouse_pos,
                        PANEL_LIGHTER if on else PANEL_LIGHT, PANEL_LIGHTER, ACCENT if on else WHITE,
                        callback=lambda v=value: on_pick(v), radius=9,
                        border_color=ACCENT if on else None)
        return y + 52
