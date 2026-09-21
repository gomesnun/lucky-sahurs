"""Fontes e texto: fonte com contorno, quebra de linhas, texto com sombra."""

import os
import sys
import pygame

from config import GAME_DIR
from theme import BLACK, FONT_NAMES, OUTLINE, WHITE


def wrap_text(text, font, max_width):
    words = text.split(" ")
    lines, cur = [], ""
    for w in words:
        test = (cur + " " + w).strip()
        if font.size(test)[0] <= max_width or not cur:
            cur = test
        else:
            lines.append(cur)
            cur = w
    if cur:
        lines.append(cur)
    return lines


def fit_text(font, text, max_width):
    """Encurta o texto com '...' se não couber em max_width (nunca sai da caixa)."""
    if font.size(text)[0] <= max_width:
        return text
    ell = "..."
    while text and font.size(text + ell)[0] > max_width:
        text = text[:-1]
    return text.rstrip() + ell


def outline_color_for(text_color):
    """Devolve preto ou branco, o que fizer mais contraste com a cor de texto dada."""
    r, g, b = text_color[:3]
    lum = 0.299 * r + 0.587 * g + 0.114 * b
    return WHITE if lum < 128 else BLACK


def is_light(color):
    r, g, b = color[:3]
    return 0.299 * r + 0.587 * g + 0.114 * b >= 110


_CUSTOM_FONT = "unset"


def find_custom_font():
    """Se existir uma pasta 'fonts' com um .ttf/.otf, usa essa fonte (a Fredoka vem la dentro). Procura
    ao lado do jogo / .exe (para poderes trocar a letra), na pasta do projeto e dentro do .exe (PyInstaller)."""
    global _CUSTOM_FONT
    if _CUSTOM_FONT == "unset":
        _CUSTOM_FONT = None
        folders = [os.path.join(GAME_DIR, "fonts"),
                   os.path.join(os.path.dirname(os.path.dirname(os.path.abspath(__file__))), "fonts")]
        bundled = getattr(sys, "_MEIPASS", None)
        if bundled:
            folders.append(os.path.join(bundled, "fonts"))
        for folder in folders:
            try:
                if os.path.isdir(folder):
                    for fn in sorted(os.listdir(folder)):
                        if fn.lower().endswith((".ttf", ".otf")):
                            _CUSTOM_FONT = os.path.join(folder, fn)
                            break
            except OSError:
                continue
            if _CUSTOM_FONT:
                break
    return _CUSTOM_FONT


# fontes "gordas e redondinhas" que já venham no PC (a primeira que existir é usada)
HEAVY_FONT_NAMES = ("lilitaone", "fredokaone", "fredoka", "baloo2", "luckiestguy",
                    "segoeuiblack", "arialroundedmtbold")
HEAVY_FONT_SCALE = {"segoeuiblack": 0.93, "arialroundedmtbold": 0.97}


class StyledFont:
    """Usa-se como uma pygame Font normal, mas o texto claro sai com um contorno preto
    grosso (estilo cartoon). O texto escuro (ex.: preto em cima de dourado) sai sem contorno.
    Tudo o que não seja render() (size, get_height...) vai direto para a fonte de baixo."""

    _cache = {}

    def __init__(self, raw, outline):
        self.raw = raw
        self.outline = outline
        t = outline
        self._offsets = [(dx, dy) for dx in range(-t, t + 1) for dy in range(-t, t + 1)
                         if (dx or dy) and dx * dx + dy * dy <= t * t + 1]

    def __getattr__(self, name):
        if name == "raw":
            raise AttributeError(name)
        return getattr(self.raw, name)

    def render(self, text, antialias=True, color=(255, 255, 255), background=None):
        color = tuple(color[:3])
        key = (id(self), text, color)
        surf = StyledFont._cache.get(key)
        if surf is not None:
            return surf
        base = self.raw.render(text, True, color)
        if self.outline <= 0 or not is_light(color):
            surf = base
        else:
            t = self.outline
            w, h = base.get_size()
            surf = pygame.Surface((w + 2 * t, h + 2 * t), pygame.SRCALPHA)
            shade = self.raw.render(text, True, OUTLINE)
            for dx, dy in self._offsets:
                surf.blit(shade, (t + dx, t + dy))
            surf.blit(base, (t, t))
        try:
            surf = surf.convert_alpha()     # mesmo formato do ecra: o blit deixa de converter pixeis
        except pygame.error:
            pass
        if len(StyledFont._cache) > 1500:
            StyledFont._cache.clear()
        StyledFont._cache[key] = surf
        return surf


def make_font(size, outline=1, heavy=False):
    custom = find_custom_font()
    raw = None
    if custom:
        try:
            raw = pygame.font.Font(custom, size)
        except (OSError, pygame.error):
            raw = None
    if raw is None and heavy:
        for name in HEAVY_FONT_NAMES:
            path = pygame.font.match_font(name)
            if path:
                try:
                    raw = pygame.font.Font(path, max(8, int(round(size * HEAVY_FONT_SCALE.get(name, 1.0)))))
                    break
                except (OSError, pygame.error):
                    raw = None
    if raw is None:
        raw = pygame.font.SysFont(FONT_NAMES, size, bold=True)
    return StyledFont(raw, outline)


_FONT_AT_CACHE = {}


def _default_outline(size, heavy):
    """Espessura do contorno que a fonte deste tamanho tem noutros sítios do jogo (font_med,
    font_big...), para os cartões (que pedem tamanhos "soltos") ficarem consistentes com o resto."""
    if not heavy:
        return 1
    if size <= 14:
        return 1
    if size <= 22:
        return 2
    if size <= 34:
        return 3
    if size <= 54:
        return 4
    return 5


def font_at(size, heavy=True, outline=None):
    """Fonte com o tamanho exato pedido (com cache): os cartões dos pets precisam de muitos
    tamanhos diferentes consoante o espaço disponível (ver ui/cards.py)."""
    size = max(1, int(size))
    if outline is None:
        outline = _default_outline(size, heavy)
    key = (size, heavy, outline)
    f = _FONT_AT_CACHE.get(key)
    if f is None:
        if len(_FONT_AT_CACHE) > 400:
            _FONT_AT_CACHE.clear()
        f = make_font(size, outline=outline, heavy=heavy)
        _FONT_AT_CACHE[key] = f
    return f


def clear_font_caches():
    """Chamado ao trocar de tema (o contorno do texto muda de cor): esquece as fontes e os
    textos já desenhados, para saírem a seguir com o contorno certo."""
    _FONT_AT_CACHE.clear()
    StyledFont._cache.clear()


def render_outlined(font, text, color, outline_color=None, thickness=1):
    """Texto com um contorno à volta (nas 8 direções)."""
    if outline_color is None:
        outline_color = outline_color_for(color)
    font = getattr(font, "raw", font)
    base = font.render(text, True, color)
    w, h = base.get_size()
    pad = thickness + 1
    surf = pygame.Surface((w + pad * 2, h + pad * 2), pygame.SRCALPHA)
    outline = font.render(text, True, outline_color)
    for dx in range(-thickness, thickness + 1):
        for dy in range(-thickness, thickness + 1):
            if dx == 0 and dy == 0:
                continue
            surf.blit(outline, (pad + dx, pad + dy))
    surf.blit(base, (pad, pad))
    return surf


def blit_text(surf, font, text, color, center=None, topleft=None, shadow=True):
    """Escreve texto com uma sombrinha de 1px (cor de contraste) — fica nítido em
    cima de fundos lisos sem 'borrar' as letras como um contorno grosso fazia."""
    t = font.render(text, True, color)
    r = t.get_rect(center=center) if center is not None else t.get_rect(topleft=topleft)
    if shadow and not (isinstance(font, StyledFont) and is_light(color)):
        s = getattr(font, "raw", font).render(text, True, outline_color_for(color))
        s.set_alpha(150)
        surf.blit(s, (r.x + 1, r.y + 1))
    surf.blit(t, r)
    return r
