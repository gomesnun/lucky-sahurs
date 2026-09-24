"""Conversa com um amigo: balões das mensagens, caixa de escrita e botão de enviar.
A lógica e as chamadas à cloud estão em online/chat.py."""

import time
import pygame

from i18n import tr
from theme import (
    ACCENT, ACCENT_HOVER, BAD, BLACK, GREY, GREY_DIM,
    OUTLINE, PANEL, PANEL_LIGHT, PANEL_LIGHTER, WHITE,
)
from ui.avatar import avatar_surface
from ui.drawing import bake
from ui.fonts import fit_text, wrap_text


BUBBLE_GAP = 6
BUBBLE_PAD = 10
INPUT_H = 44
CHAT_MAX_LEN = 280


class ChatPanelMixin:
    """Ecrã da conversa (desenha-se dentro da página de Amigos)."""

    def draw_chat(self, rect, mouse_pos):
        entry = next((f for f in self.friends_list if f["uid"] == self.chat_uid), None)

        # ---- cabeçalho: foto + nome + Back ----
        head_h = 44
        img = avatar_surface(entry.get("avatar_pet") if entry else None,
                             entry.get("avatar_mut", "normal") if entry else "normal", 34)
        self.canvas.blit(img, img.get_rect(midleft=(rect.x + 2, rect.y + head_h // 2)))
        name = self.font_med.render(fit_text(self.font_med, self.chat_name, rect.width - 220), True, WHITE)
        self.canvas.blit(bake(name, PANEL), (rect.x + 44, rect.y + head_h // 2 - name.get_height() // 2))
        self.button(pygame.Rect(rect.right - 120, rect.y + 4, 120, 34), tr("Back"), self.font_small_b,
                    mouse_pos, PANEL_LIGHT, PANEL_LIGHTER, WHITE, callback=self.close_chat, radius=9)

        # ---- caixa de escrita em baixo ----
        input_rect = pygame.Rect(rect.x, rect.bottom - INPUT_H, rect.width - 110, INPUT_H)
        self.draw_text_field(input_rect, self.chat_field, self.chat_field.text, tr("Write a message..."),
                             self.chat_focus, lambda: self.set_chat_focus(True))
        self.button(pygame.Rect(input_rect.right + 10, input_rect.y, 100, INPUT_H),
                    tr("Sending...") if self.chat_sending else tr("Send"), self.font_small_b, mouse_pos,
                    ACCENT, ACCENT_HOVER, BLACK,
                    callback=None if self.chat_sending else self.send_chat_message, radius=10)

        # ---- mensagens ----
        list_rect = pygame.Rect(rect.x, rect.y + head_h + 6, rect.width,
                                input_rect.top - 10 - (rect.y + head_h + 6))
        self.chat_list_rect = list_rect
        self.draw_chat_messages(list_rect, mouse_pos)

        if self.chat_error:
            t = self.font_tiny.render(fit_text(self.font_tiny, self.chat_error, rect.width), True, BAD)
            self.canvas.blit(t, (rect.x, input_rect.top - 16))

    # ---------------------------------------------------------------- balões
    def chat_bubble_lines(self, text, max_w):
        return wrap_text(text, self.font_small_b, max_w - 2 * BUBBLE_PAD)

    def draw_chat_messages(self, rect, mouse_pos):
        if not self.chat_messages:
            text = tr("Loading...") if self.chat_loading else tr("No messages yet - say hi!")
            t = self.font_med.render(text, True, GREY)
            self.canvas.blit(t, t.get_rect(center=rect.center))
            self.chat_max_scroll = 0.0
            return

        me = self.account.get("uid") if self.account else None
        max_w = int(rect.width * 0.72)
        line_h = self.font_small_b.get_height() + 2

        # mede tudo primeiro: a conversa desenha-se de baixo para cima (o fim é o que interessa)
        laid = []
        total = 0
        for msg in self.chat_messages:
            lines = self.chat_bubble_lines(msg["text"], max_w)
            h = 2 * BUBBLE_PAD + len(lines) * line_h + self.font_tiny.get_height()
            laid.append((msg, lines, h))
            total += h + BUBBLE_GAP
        total = max(0, total - BUBBLE_GAP)
        self.chat_max_scroll = max(0.0, total - rect.height)
        self.chat_scroll = max(0.0, min(self.chat_max_scroll, self.chat_scroll))

        # chat_scroll = 0 é o fim da conversa (mensagens mais recentes em baixo)
        y = rect.bottom - 2 - total + self.chat_scroll
        self.push_clip(rect)
        for msg, lines, h in laid:
            if y + h >= rect.top - 4 and y <= rect.bottom + 4:
                mine = msg["from_uid"] == me
                w = max(60, max((self.font_small_b.size(l)[0] for l in lines), default=0) + 2 * BUBBLE_PAD)
                x = rect.right - 16 - w if mine else rect.x
                box = pygame.Rect(x, int(y), w, h)
                pygame.draw.rect(self.canvas, ACCENT if mine else PANEL_LIGHT, box, border_radius=12)
                if not mine:
                    pygame.draw.rect(self.canvas, OUTLINE, box, width=2, border_radius=12)
                ty = box.y + BUBBLE_PAD
                for line in lines:
                    self.canvas.blit(self.font_small_b.render(line, True, BLACK if mine else WHITE),
                                     (box.x + BUBBLE_PAD, ty))
                    ty += line_h
                when = chat_when(msg.get("sent_at"))
                if when:
                    t = self.font_tiny.render(when, True, (40, 40, 40) if mine else GREY_DIM)
                    self.canvas.blit(t, (box.right - t.get_width() - BUBBLE_PAD,
                                         box.bottom - BUBBLE_PAD - t.get_height() + 2))
            y += h + BUBBLE_GAP
        self.pop_clip()
        self.draw_scrollbar(rect, self.chat_max_scroll - self.chat_scroll, total,
                            key="chat", mouse_pos=mouse_pos)


def chat_when(stamp):
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
