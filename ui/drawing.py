"""Desenho: painéis, gradientes, barras e os fundos das raridades (incl. Cosmic e Transcendent)."""

import colorsys
import math
import os
import random
import pygame

from config import IS_ANDROID
import theme
from theme import BORDER_W, BORDER_W_SMALL, OUTLINE, PANEL


def make_vertical_gradient(w, h, top_color, bottom_color):
    surf = pygame.Surface((w, h))
    for y in range(h):
        t = y / max(1, h - 1)
        color = tuple(int(top_color[i] + (bottom_color[i] - top_color[i]) * t) for i in range(3))
        pygame.draw.line(surf, color, (0, y), (w, y))
    return surf


def make_game_background(w, h, top_color, bottom_color, dark=False):
    """Fundo do jogo: o mesmo gradiente vertical de sempre com uns pontinhos subtis espalhados (sempre nos
    mesmos sítios, para não "saltarem" quando a janela é redimensionada): estrelinhas claras em modo escuro
    e pontinhos pretos em modo claro."""
    surf = make_vertical_gradient(w, h, top_color, bottom_color)
    rng = random.Random(1789980865)
    target, low, high = ((255, 255, 255), 0.12, 0.42) if dark else ((0, 0, 0), 0.16, 0.52)
    for _ in range(max(50, (w * h) // 4600)):
        x, y = rng.randrange(w), rng.randrange(h)
        t = y / float(max(1, h - 1))
        base = mix(top_color, bottom_color, t)
        dot = mix(base, target, rng.uniform(low, high))
        pygame.draw.circle(surf, dot, (x, y), 1 if rng.random() < 0.82 else 2)
    # E a maior imagem do jogo e cobre o ecra inteiro uma vez por frame: se nao estiver no formato do
    # ecra, esse unico blit converte centenas de milhares de pixeis a cada frame. O fundo e opaco,
    # por isso usa-se convert() (sem canal alfa), que da o blit mais rapido que existe.
    try:
        surf = surf.convert()
    except pygame.error:
        pass
    return surf


def mix(c1, c2, t):
    """Mistura duas cores (t=0 -> c1, t=1 -> c2)."""
    return tuple(int(c1[i] + (c2[i] - c1[i]) * t) for i in range(3))


def shade(color, factor):
    """A mesma cor mais clara (factor > 1) ou mais escura (factor < 1), sem sair de 0-255."""
    return tuple(max(0, min(255, int(c * factor))) for c in color[:3])


_GRADIENT_CACHE = {}


def rounded_gradient(w, h, top_color, bottom_color, radius):
    """Gradiente vertical (top_color -> bottom_color) já recortado num retângulo arredondado
    (usado nos cartões: pets bloqueados no Index e o fundo das traits)."""
    key = (w, h, tuple(top_color), tuple(bottom_color), radius)
    surf = _GRADIENT_CACHE.get(key)
    if surf is not None:
        return surf
    surf = pygame.Surface((w, h), pygame.SRCALPHA)
    for y in range(h):
        t = y / float(max(1, h - 1))
        color = mix(top_color, bottom_color, t)
        pygame.draw.line(surf, (*color, 255), (0, y), (w, y))
    mask = pygame.Surface((w, h), pygame.SRCALPHA)
    pygame.draw.rect(mask, (255, 255, 255, 255), mask.get_rect(), border_radius=radius)
    surf.blit(mask, (0, 0), special_flags=pygame.BLEND_RGBA_MIN)
    if len(_GRADIENT_CACHE) > 200:
        _GRADIENT_CACHE.clear()
    _GRADIENT_CACHE[key] = surf
    return surf


def ease_out_cubic(t):
    t = max(0.0, min(1.0, t))
    return 1 - (1 - t) ** 3


# No telemovel, misturar alfa por pixel custa cerca de 228 ns POR PIXEL: um blit opaco de 200x80
# leva 0.006 ms e o mesmo blit com alfa leva 3.8 ms (mais de 500x). Nao e o aparelho a ser lento -
# copiar o ecra inteiro opaco leva 0.44 ms - e o caminho com alfa deste build do pygame em ARM.
# Medido no aparelho (ver _perf_blit_bench no main.py):
#     opaco 0.006 ms | colorkey 0.045 ms | alfa 3.856 ms | alfa convertido 4.875 ms
# Por isso, no Android, evita-se o alfa sempre que possivel e nunca se chama convert_alpha().
# LUCKY_SAHURS_SLOW_ALPHA=1 liga este caminho no computador, para se poder ver como fica sem ter
# de fazer um APK e instalar no telemovel.
ALPHA_IS_SLOW = IS_ANDROID or bool(os.environ.get("LUCKY_SAHURS_SLOW_ALPHA"))
COLORKEY = (255, 0, 254)        # cor que nao aparece no jogo, usada como "transparente"


def to_display_format(surf):
    """Poe a imagem no formato em que o blit e mais barato. No computador e convert_alpha(). No
    telemovel convert_alpha() deixa o blit AINDA mais lento (4.875 ms contra 3.856 ms medidos),
    por isso deixa-se a imagem como esta."""
    if ALPHA_IS_SLOW:
        return surf
    try:
        return surf.convert_alpha()
    except pygame.error:
        return surf


def to_opaque(surf, background):
    """A mesma imagem achatada contra uma cor de fundo conhecida: fica sem canal alfa nenhum e o
    blit passa a ser o mais rapido que existe (0.006 ms em vez de 3.8 ms). So se pode usar quando
    se sabe mesmo o que esta por tras - caso contrario ve-se um rectangulo da cor do fundo."""
    flat = pygame.Surface(surf.get_size())
    flat.fill(background[:3])
    flat.blit(surf, (0, 0))
    try:
        return flat.convert()
    except pygame.error:
        return flat


def to_colorkey(surf, near_color):
    """A mesma imagem sem canal alfa, com uma cor a fazer de transparente. O blit com cor-chave
    custa 0.045 ms contra 3.8 ms com alfa por pixel, e continua a deixar ver o fundo a volta.

    O alfa passa a ser tudo-ou-nada (a partir de metade conta como opaco), porque a cor-chave nao
    sabe guardar meios-tons. Para a borda nao ficar aos degraus contra a cor-chave, os pixeis que
    ficam visiveis sao primeiro misturados com near_color - a cor que costuma estar por tras."""
    size = surf.get_size()
    visible = pygame.Surface(size)
    visible.fill(near_color[:3])
    visible.blit(surf, (0, 0))          # borda ja misturada com o fundo, sem franja
    hidden = pygame.Surface(size)
    hidden.fill(COLORKEY)
    out = pygame.Surface(size)
    # A mascara diz, pixel a pixel, o que era opaco: e feita em C, e so se faz uma vez por imagem.
    pygame.mask.from_surface(surf, 127).to_surface(out, setsurface=visible, unsetsurface=hidden)
    out.set_colorkey(COLORKEY)
    try:
        return out.convert()
    except pygame.error:
        return out


_SHADOW_CACHE = {}
_PANEL_CACHE = {}
_DIM_CACHE = {}


def dim_overlay(w, h, alpha):
    """O veu escuro por tras das paginas (Options, Stats, Traits...). E sempre igual: guarda-se
    em cache em vez de criar uma superficie do tamanho do ecra a cada frame.

    No telemovel este era o desenho mais caro de todos: cobre o ecra inteiro (784 mil pixeis) e,
    a 228 ns por pixel, um so destes blits levava perto de 180 ms. Em vez do veu transparente
    usa-se o proprio fundo do jogo ja escurecido, que e opaco e se copia em 0.44 ms. A diferenca
    e que deixa de se ver o ecra de jogo por trás da pagina - e o preco de o painel abrir depressa."""
    key = (w, h, alpha, theme.DARK_MODE)
    surf = _DIM_CACHE.get(key)
    if surf is None:
        if ALPHA_IS_SLOW:
            t = alpha / 255.0
            surf = make_vertical_gradient(w, h, mix(theme.BG_TOP, (0, 0, 0), t),
                                          mix(theme.BG_BOTTOM, (0, 0, 0), t))
            try:
                surf = surf.convert()
            except pygame.error:
                pass
        else:
            surf = pygame.Surface((w, h), pygame.SRCALPHA)
            surf.fill((0, 0, 0, alpha))
            surf = to_display_format(surf)
        _DIM_CACHE.clear()
        _DIM_CACHE[key] = surf
    return surf


def rounded_box(w, h, color, radius, border_w):
    """Rectangulo de cantos redondos (preenchimento + contorno) ja desenhado, guardado em cache.
    So depende do tamanho, da cor, do raio e da espessura, e ha dezenas por frame (paineis e botoes):
    um blit de uma imagem pronta sai muito mais barato do que dois draw.rect com cantos redondos."""
    key = (w, h, radius, tuple(color[:3]), border_w, tuple(OUTLINE[:3]))
    box = _PANEL_CACHE.get(key)

    if box is None:
        box = pygame.Surface((w, h), pygame.SRCALPHA)
        box_rect = box.get_rect()
        pygame.draw.rect(box, color, box_rect, border_radius=radius)
        if border_w > 0:
            pygame.draw.rect(box, OUTLINE, box_rect, width=border_w, border_radius=radius)
        box = to_display_format(box)
        if len(_PANEL_CACHE) > 600:
            _PANEL_CACHE.clear()
        _PANEL_CACHE[key] = box
    return box


def draw_panel(canvas, rect, color=None, radius=14, shadow=True, border=None):
    """Painel com contorno preto grosso. border=None -> 4px nos painéis com sombra, 3px nos outros."""
    if color is None:
        color = PANEL           # lido aqui (e nao no def) para acompanhar o modo claro / escuro
    if ALPHA_IS_SLOW:
        # No telemovel, desenhar os dois rectangulos custa 0.068 ms e copiar a mesma imagem ja
        # pronta (que tem alfa nos cantos) custa 3.8 ms: aqui a cache era 55x mais LENTA que
        # desenhar. A sombra e uma imagem com alfa ainda maior do que o painel, por isso nao se
        # desenha - o contorno preto grosso ja separa o painel do fundo.
        bw = border if border is not None else (BORDER_W if shadow else BORDER_W_SMALL)
        pygame.draw.rect(canvas, color, rect, border_radius=radius)
        if bw > 0:
            pygame.draw.rect(canvas, OUTLINE, rect, width=bw, border_radius=radius)
        return
    if shadow:
        # A sombra e sempre a mesma imagem para o mesmo tamanho: criar a superficie e redesenhar o
        # rectangulo a cada frame custava caro (ha dezenas de paineis por frame). Fica em cache.
        key = (rect.width, rect.height, radius)
        shadow_surf = _SHADOW_CACHE.get(key)
        if shadow_surf is None:
            shadow_surf = pygame.Surface((rect.width + 14, rect.height + 14), pygame.SRCALPHA)
            pygame.draw.rect(shadow_surf, (0, 0, 0, 80), (7, 11, rect.width, rect.height), border_radius=radius)
            if len(_SHADOW_CACHE) > 400:
                _SHADOW_CACHE.clear()
            shadow_surf = to_display_format(shadow_surf)
            _SHADOW_CACHE[key] = shadow_surf
        canvas.blit(shadow_surf, (rect.x - 7, rect.y - 7))
    # O corpo do painel (preenchimento + contorno) so depende do tamanho, do raio, da cor e da
    # espessura: desenha-se uma vez para uma imagem e depois e so um blit, em vez de dois
    # pygame.draw.rect com cantos redondos a cada frame.
    bw = border if border is not None else (BORDER_W if shadow else BORDER_W_SMALL)
    canvas.blit(rounded_box(rect.width, rect.height, color, radius, bw), rect)


# ---------------------------------------------------------------- movimento suave (sub-píxel)
# Quando um sprite se mexe devagar (menos de 1 píxel por frame) e só pode estar em posições inteiras,
# vê-se aos saltinhos. Solução: guardar 8 cópias do sprite já deslocadas 0/8, 1/8, ... 7/8 de píxel
# e desenhar a cópia certa - o movimento fica contínuo.
VSHIFT_STEPS = 8
_VSHIFT_CACHE = {}


def _vshift_variants(surf, steps):
    w, h = surf.get_size()
    hi = pygame.Surface((w * steps, (h + 4) * steps), pygame.SRCALPHA)
    hi.blit(pygame.transform.scale(surf, (w * steps, h * steps)), (0, 2 * steps))
    out = []
    for k in range(steps):
        window = pygame.Rect(0, steps - k, w * steps, (h + 2) * steps)
        out.append(pygame.transform.smoothscale(hi.subsurface(window), (w, h + 2)))
    return out


def clear_drawing_caches():
    """Chamado ao trocar de tema: os sprites guardados em cache foram desenhados com as cores antigas."""
    _VSHIFT_CACHE.clear()
    _SHADOW_CACHE.clear()
    _DIM_CACHE.clear()
    _PANEL_CACHE.clear()
    _BAKE_CACHE.clear()


_BAKE_CACHE = {}


def bake(surf, color):
    """A mesma imagem ja misturada com a cor lisa que vai estar por baixo, guardada em cache.

    No telemovel um blit com alfa por pixel custa ~228 ns/pixel e um opaco ~0.5 ns/pixel: uma linha
    de texto por cima de um botao (que e de cor lisa) fica exatamente igual e passa a custar quase
    nada. So se usa quando se sabe mesmo a cor que esta por baixo - senao veria-se um rectangulo."""
    if not ALPHA_IS_SLOW:
        return surf
    key = (id(surf), tuple(color[:3]))
    hit = _BAKE_CACHE.get(key)
    if hit is None:
        if len(_BAKE_CACHE) > 800:
            _BAKE_CACHE.clear()
        # guarda-se tambem a imagem de origem: assim o id() dela nunca e reaproveitado por outra
        hit = (surf, to_opaque(surf, color))
        _BAKE_CACHE[key] = hit
    return hit[1]


def bake_on(surf, backdrop, topleft):
    """O mesmo que bake(), mas contra um pedaco do fundo do jogo (que e fixo) em vez de uma cor lisa.
    Serve para o que se desenha sempre no mesmo sitio por cima do fundo: o titulo, o rodape.
    Devolve a imagem original se o sitio pedido sair fora do fundo (ai nao ha o que copiar)."""
    if not ALPHA_IS_SLOW or backdrop is None:
        return surf
    rect = pygame.Rect(topleft, surf.get_size())
    if not backdrop.get_rect().contains(rect):
        return surf
    key = (id(surf), id(backdrop), rect.topleft)
    hit = _BAKE_CACHE.get(key)
    if hit is None:
        if len(_BAKE_CACHE) > 800:
            _BAKE_CACHE.clear()
        flat = backdrop.subsurface(rect).copy()
        flat.blit(surf, (0, 0))
        try:
            flat = flat.convert()
        except pygame.error:
            pass
        hit = (surf, flat)
        _BAKE_CACHE[key] = hit
    return hit[1]


def blit_smooth_y(canvas, key, build, x, y, near_color=None):
    """Desenha em (x, y) um sprite ESTÁTICO com y fracionário. 'key' identifica o sprite (para a cache) e
    'build()' cria a Surface na primeira vez que for precisa.
    'near_color' e a cor do fundo por baixo: no telemovel serve para trocar o alfa por uma cor-chave
    (0.045 ms contra 3.8 ms por blit medidos no aparelho)."""
    variants = _VSHIFT_CACHE.get(key)
    if variants is None:
        raw = _vshift_variants(build(), VSHIFT_STEPS)
        if ALPHA_IS_SLOW and near_color is not None:
            variants = [to_colorkey(v, near_color) for v in raw]
        else:
            variants = [to_display_format(v) for v in raw]
        _VSHIFT_CACHE[key] = variants
    iy = int(math.floor(y))
    k = min(VSHIFT_STEPS - 1, int((y - iy) * VSHIFT_STEPS))
    canvas.blit(variants[k], (int(x), iy - 1))


def draw_state_border(canvas, rect, color, radius=12, width=2):
    """Borda colorida por DENTRO do contorno preto (estado: máximo, hover, ativo...)."""
    pygame.draw.rect(canvas, color, rect.inflate(-9, -9), width=width, border_radius=max(2, radius - 4))


_BAR_FILL_CACHE = {}


def bar_fill_surface(kind, color, w, h):
    """Enchimento das barras Golden/Diamond/Rainbow (com cantos redondos, guardado em cache)."""
    key = (kind, color, w, h)
    surf = _BAR_FILL_CACHE.get(key)
    if surf is None:
        surf = pygame.Surface((w, h), pygame.SRCALPHA)
        if kind == "rainbow":
            for x in range(w):
                r, g, b = colorsys.hsv_to_rgb(0.83 * x / float(max(1, w)), 0.65, 1.0)
                pygame.draw.line(surf, (int(r * 255), int(g * 255), int(b * 255)), (x, 0), (x, h))
        else:
            for yy in range(h):
                t = yy / float(max(1, h - 1))
                pygame.draw.line(surf, tuple(int(c * (1.0 - 0.30 * t)) for c in color), (0, yy), (w, yy))
        mask = pygame.Surface((w, h), pygame.SRCALPHA)
        pygame.draw.rect(mask, (255, 255, 255, 255), mask.get_rect(), border_radius=h // 2)
        surf.blit(mask, (0, 0), special_flags=pygame.BLEND_RGBA_MIN)
        if ALPHA_IS_SLOW:
            # so os cantos redondos e que sao transparentes: troca-se o alfa por uma cor-chave
            # contra o fundo escuro do meio da pilula (0.045 ms contra 3.8 ms por blit)
            surf = to_colorkey(surf, (20, 20, 23))
        _BAR_FILL_CACHE[key] = surf
    return surf


_RAINBOW_CACHE = {}


def _rainbow_gradient(w, h, sat):
    """Faixa horizontal arco-íris (as mesmas cores da barra do Rainbow Roll)."""
    surf = pygame.Surface((w, h), pygame.SRCALPHA)
    for x in range(w):
        r, g, b = colorsys.hsv_to_rgb(0.83 * x / float(max(1, w)), sat, 1.0)
        pygame.draw.line(surf, (int(r * 255), int(g * 255), int(b * 255)), (x, 0), (x, h))
    return surf


def rainbow_glow_surface(w, h, radius):
    """Brilho arco-íris (retângulo redondo) que fica por trás do botão ROLL no Rainbow Roll.
    Vem em cache; quem o usa define a transparência com set_alpha()."""
    key = ("glow", w, h, radius)
    surf = _RAINBOW_CACHE.get(key)
    if surf is None:
        surf = _rainbow_gradient(w, h, 0.8)
        mask = pygame.Surface((w, h), pygame.SRCALPHA)
        pygame.draw.rect(mask, (255, 255, 255, 255), mask.get_rect(), border_radius=radius)
        surf.blit(mask, (0, 0), special_flags=pygame.BLEND_RGBA_MIN)
        _RAINBOW_CACHE[key] = surf
    return surf


def draw_rainbow_border(canvas, rect, radius=10, width=3):
    """Contorno arco-íris (igual ao draw_state_border, mas com as cores do arco-íris).
    `rect` é o retângulo já reduzido, como o que o draw_state_border desenha."""
    key = ("border", rect.width, rect.height, radius, width)
    surf = _RAINBOW_CACHE.get(key)
    if surf is None:
        surf = _rainbow_gradient(rect.width, rect.height, 0.85)
        ring = pygame.Surface((rect.width, rect.height), pygame.SRCALPHA)
        pygame.draw.rect(ring, (255, 255, 255, 255), ring.get_rect(), width=width, border_radius=radius)
        surf.blit(ring, (0, 0), special_flags=pygame.BLEND_RGBA_MIN)
        _RAINBOW_CACHE[key] = surf
    canvas.blit(surf, rect.topleft)


_GLOW_CACHE = {}


def rarity_glow(size, color, spread=30):
    """Brilho suave (na cor da raridade) que fica por trás do cartão do último pet."""
    key = (size, color, spread)
    glow = _GLOW_CACHE.get(key)
    if glow is None:
        w, h = size
        glow = pygame.Surface((w + 2 * spread, h + 2 * spread), pygame.SRCALPHA)
        for i in range(spread, 0, -2):
            a = int(140 * ((spread - i) / float(spread)) ** 2)
            pygame.draw.rect(glow, (color[0], color[1], color[2], a),
                             pygame.Rect(spread - i, spread - i, w + 2 * i, h + 2 * i),
                             border_radius=12 + i)
        glow = to_display_format(glow)
        _GLOW_CACHE[key] = glow
    return glow


_RARITY_BG_CACHE = {}


def _paint_cosmic_bg(bg, c1, c2, w, h):
    """Nebulosa: gradiente escuro (topo) -> violeta vivo (fundo) com estrelinhas sempre nos mesmos sítios."""
    for y in range(h):
        t = y / float(max(1, h - 1))
        col = (int(c2[0] + (c1[0] - c2[0]) * t), int(c2[1] + (c1[1] - c2[1]) * t),
               int(c2[2] + (c1[2] - c2[2]) * t), 255)
        pygame.draw.line(bg, col, (0, y), (w, y))
    rng = random.Random(7)
    for _ in range(max(10, (w * h) // 700)):
        x, y = rng.randrange(w), rng.randrange(h)
        col = rng.choice(((255, 255, 255, 255), (170, 230, 255, 255), (255, 190, 240, 255)))
        if rng.random() < 0.2:
            pygame.draw.line(bg, col, (x - 3, y), (x + 3, y))
            pygame.draw.line(bg, col, (x, y - 3), (x, y + 3))
        else:
            pygame.draw.circle(bg, col, (x, y), 1)


def _paint_transcendent_bg(bg, w, h):
    """Iridescente: faixas diagonais de arco-íris pastel com uns brilhos brancos."""
    total = w + h
    for k in range(0, total + 6, 3):
        r, g, b = colorsys.hsv_to_rgb((k / float(total)) % 1.0, 0.38, 1.0)
        pygame.draw.line(bg, (int(r * 255), int(g * 255), int(b * 255), 255), (k, 0), (k - h, h), 4)
    rng = random.Random(11)
    for _ in range(max(4, (w * h) // 2500)):
        x, y = rng.randrange(w), rng.randrange(h)
        pygame.draw.line(bg, (255, 255, 255, 255), (x - 3, y), (x + 3, y))
        pygame.draw.line(bg, (255, 255, 255, 255), (x, y - 3), (x, y + 3))


def draw_rarity_bg(surface, rect_local, rarity, radius=12):
    c1, c2 = rarity["color"], rarity["color2"]
    if c2 is None:
        pygame.draw.rect(surface, c1, rect_local, border_radius=radius)
        return

    # O fundo com duas cores / riscas é desenhado numa superfície à parte e depois
    # recortado pela forma arredondada — assim a 2.ª cor não "sai" pelos cantos.
    w, h = rect_local.size
    key = (rarity["key"], w, h, radius)
    bg = _RARITY_BG_CACHE.get(key)
    if bg is None:
        bg = pygame.Surface((w, h), pygame.SRCALPHA)
        bg.fill((c1[0], c1[1], c1[2], 255))
        full = pygame.Rect(0, 0, w, h)
        if rarity["key"] == "cosmico":
            _paint_cosmic_bg(bg, c1, c2, w, h)
        elif rarity["key"] == "transcendente":
            _paint_transcendent_bg(bg, w, h)
        elif rarity["key"] == "secreto":
            stripe_w = max(8, w // 14)
            x = full.left - full.height
            while x < full.right:
                pygame.draw.polygon(bg, c2, [
                    (x, full.bottom), (x + stripe_w, full.bottom),
                    (x + stripe_w + full.height, full.top), (x + full.height, full.top)
                ])
                x += stripe_w * 2
        else:
            pygame.draw.polygon(bg, c2, [
                (full.right, full.top),
                (full.right, full.bottom),
                (full.left, full.bottom),
            ])
        mask = pygame.Surface((w, h), pygame.SRCALPHA)
        pygame.draw.rect(mask, (255, 255, 255, 255), full, border_radius=radius)
        bg.blit(mask, (0, 0), special_flags=pygame.BLEND_RGBA_MIN)
        _RARITY_BG_CACHE[key] = bg
    surface.blit(bg, rect_local.topleft)
