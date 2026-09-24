"""Peças reutilizáveis: partículas, painéis laterais deslizantes e campo de texto."""

import math
import random
import pygame

from ui.drawing import ease_out_cubic


class TextField:
    """Campo de texto do ecrã de conta / da procura de amigos (só guarda o texto e a posição do
    cursor; o desenho é feito pelo Game, em ui/base.py)."""

    def __init__(self, kind):
        self.kind = kind                    # "username" | "password" | "email" | "code"
        self.text = ""
        self.caret = 0                      # quantas letras ficam à esquerda do cursor
        self.max_len = {"username": 16, "email": 80, "code": 6, "text": 280}.get(kind, 64)

    def set_text(self, text=""):
        self.text = text
        self.caret = len(text)

    def _clamp(self):
        self.caret = max(0, min(len(self.text), self.caret))

    def add(self, s):
        for ch in s:
            if self.kind == "username":
                ch = ch.lower()
                if not (ch.isascii() and (ch.isalnum() or ch == "_")):
                    continue
            elif self.kind == "code":
                if not ch.isdigit():
                    continue
            elif not ch.isprintable():
                continue
            # nenhum destes campos leva espaços (nome, email, código, palavra-passe), e os teclados
            # de telemóvel metem um espaço a mais ao aceitar uma sugestão: ficavam nomes com espaço no
            # fim que depois não davam com ninguém na procura
            if ch == " ":
                # o campo de feedback leva espaços; os outros (nome, email, código, palavra-passe) não,
                # e os teclados de telemóvel metem um espaço a mais ao aceitar uma sugestão
                if self.kind != "text" or not self.text.strip() or self.text.endswith("  "):
                    continue
            if len(self.text) >= self.max_len:
                break
            self._clamp()
            self.text = self.text[:self.caret] + ch + self.text[self.caret:]
            self.caret += 1

    def backspace(self):
        self._clamp()
        if self.caret:
            self.text = self.text[:self.caret - 1] + self.text[self.caret:]
            self.caret -= 1

    def delete(self):
        """Tecla Delete: apaga a letra à direita do cursor."""
        self._clamp()
        self.text = self.text[:self.caret] + self.text[self.caret + 1:]

    def move(self, step):
        self.caret = max(0, min(len(self.text), self.caret + step))

    def home(self):
        self.caret = 0

    def end(self):
        self.caret = len(self.text)


def clipboard_text():
    try:
        if not pygame.scrap.get_init():
            pygame.scrap.init()
        raw = pygame.scrap.get(pygame.SCRAP_TEXT)
        if not raw:
            return ""
        if isinstance(raw, bytes):
            raw = raw.decode("utf-8", "ignore")
        return raw.replace("\x00", "").splitlines()[0] if raw.strip() else ""
    except Exception:
        return ""


class Particle:
    __slots__ = ("x", "y", "vx", "vy", "life", "max_life", "color", "radius")

    def __init__(self, x, y, color):
        angle = random.uniform(0, 2 * math.pi)
        speed = random.uniform(60, 170)
        self.x, self.y = x, y
        self.vx = math.cos(angle) * speed
        self.vy = math.sin(angle) * speed - 60
        self.life = random.uniform(0.45, 0.8)
        self.max_life = self.life
        self.color = color
        self.radius = random.uniform(2.5, 4.5)

    def update(self, dt):
        self.x += self.vx * dt
        self.y += self.vy * dt
        self.vy += 140 * dt
        self.life -= dt

    def draw(self, canvas):
        if self.life <= 0:
            return
        t = max(0.0, self.life / self.max_life)
        r = max(1, int(self.radius * t + 1))
        surf = pygame.Surface((r * 2, r * 2), pygame.SRCALPHA)
        pygame.draw.circle(surf, (self.color[0], self.color[1], self.color[2], int(255 * t)), (r, r), r)
        canvas.blit(surf, (self.x - r, self.y - r))


# ----------------------------------------------------------------------------------
# PAINEL COM SLIDE
# ----------------------------------------------------------------------------------
class SlidePanel:
    """Painel que entra a deslizar pelo lado. side = 'left' ou 'right'."""

    SPEED = 5.5   # 1/segundos para abrir por completo

    def __init__(self, side):
        self.side = side
        self.content = None      # o que está (ou estava) a ser mostrado
        self.pending = None      # o que vai entrar quando o atual acabar de sair
        self.progress = 0.0      # 0 = fechado, 1 = aberto
        self.target = 0.0
        self.scroll = {}         # scroll por conteúdo
        self.max_scroll = {}

    def toggle(self, content):
        if self.content == content and self.target == 1.0:
            self.close()
        else:
            self.open(content)

    def open(self, content):
        if self.content is None or self.progress <= 0.0 or self.content == content:
            self.content = content
            self.pending = None
            self.target = 1.0
        else:
            self.pending = content
            self.target = 0.0

    def close(self):
        self.pending = None
        self.target = 0.0

    def update(self, dt):
        if self.progress < self.target:
            self.progress = min(self.target, self.progress + dt * self.SPEED)
        elif self.progress > self.target:
            self.progress = max(self.target, self.progress - dt * self.SPEED)
        if self.progress <= 0.0 and self.pending is not None:
            self.content = self.pending
            self.pending = None
            self.target = 1.0

    @property
    def visible(self):
        return self.content is not None and self.progress > 0.002

    @property
    def is_open(self):
        return self.target == 1.0 and self.content is not None

    def shown_width(self, width):
        return int(width * ease_out_cubic(self.progress))

    def get_scroll(self):
        return self.scroll.get(self.content, 0.0)

    def add_scroll(self, amount):
        cur = self.scroll.get(self.content, 0.0)
        limit = self.max_scroll.get(self.content, 0.0)
        self.scroll[self.content] = max(0.0, min(limit, cur + amount))

    def set_scroll_abs(self, value):
        """Usado ao arrastar a scrollbar com o rato: põe o scroll direto num valor (sem somar)."""
        limit = self.max_scroll.get(self.content, 0.0)
        self.scroll[self.content] = max(0.0, min(limit, value))

    def set_max_scroll(self, value):
        self.max_scroll[self.content] = max(0.0, value)
        if self.scroll.get(self.content, 0.0) > self.max_scroll[self.content]:
            self.scroll[self.content] = self.max_scroll[self.content]
