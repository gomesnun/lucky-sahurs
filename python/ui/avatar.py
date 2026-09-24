"""Foto de perfil: a imagem de um verity dentro de um círculo, com um anel da cor da mutação.

A imagem de cada verity (icons/pets/*.png) tem a bola ao centro e espaço à volta (ver ui/cards.py),
por isso aqui aumenta-se a imagem e desloca-se para a BOLA ficar a preencher o círculo - senão a foto
saía pequena e descentrada. O resultado fica em cache (as fotos aparecem em listas, a cada frame).
"""

import collections
import pygame

from core.pets import MUTATIONS, RARITIES
from theme import GREY_DIM, OUTLINE, PANEL_LIGHT, PANEL_LIGHTER
from ui.cards import BALL_CY, BALL_FRAC
from ui.icons import load_pet_image

_CACHE = collections.OrderedDict()
_CACHE_MAX = 80


def avatar_ring_color(mutation):
    """Cor do anel: dourado no Golden, azul-claro no Diamond, cinzento no normal."""
    return MUTATIONS.get(mutation, {}).get("border") or PANEL_LIGHTER


def avatar_letter_font(size):
    return pygame.font.SysFont("arial", max(10, int(size * 0.5)), bold=True)


def avatar_surface(pet_index, mutation="normal", size=48):
    """Surface quadrada (size x size) com a foto redonda e o anel. pet_index None / inválido dá a
    foto vazia (círculo com um ponto de interrogação). Está em cache: não a alteres."""
    ring = avatar_ring_color(mutation)
    key = (pet_index, mutation, size, ring, PANEL_LIGHT, OUTLINE)
    hit = _CACHE.get(key)
    if hit is not None:
        _CACHE.move_to_end(key)
        return hit

    size = max(16, int(size))
    border = max(3, size // 14)
    center = (size // 2, size // 2)
    radius = size // 2 - 1
    surf = pygame.Surface((size, size), pygame.SRCALPHA)

    img = None
    valid = isinstance(pet_index, int) and 0 <= pet_index < len(RARITIES)
    inner = size - 2 * border
    if valid:
        # a bola ocupa BALL_FRAC da largura da imagem: para ela encher o círculo, a imagem
        # tem de ser desenhada maior do que o círculo e recortada
        img_w = max(8, int(round(inner / BALL_FRAC)))
        img = load_pet_image(RARITIES[pet_index]["pet"], img_w)

    if img is not None:
        surf.blit(img, (center[0] - img_w // 2, center[1] - int(img_w * BALL_CY)))
        mask = pygame.Surface((size, size), pygame.SRCALPHA)
        pygame.draw.circle(mask, (255, 255, 255, 255), center, radius - border + 1)
        surf.blit(mask, (0, 0), special_flags=pygame.BLEND_RGBA_MULT)
    else:
        fill = RARITIES[pet_index]["color"] if valid else PANEL_LIGHT
        pygame.draw.circle(surf, fill, center, radius - border + 1)
        if not valid:
            font = avatar_letter_font(size)
            t = font.render("?", True, GREY_DIM)
            surf.blit(t, t.get_rect(center=center))

    pygame.draw.circle(surf, ring, center, radius, border)
    # fio escuro por fora e por dentro do anel: é o mesmo contorno "cartoon" dos botões e cartões
    pygame.draw.circle(surf, OUTLINE, center, radius, 1)
    pygame.draw.circle(surf, OUTLINE, center, max(1, radius - border), 1)

    if len(_CACHE) >= _CACHE_MAX:
        _CACHE.popitem(last=False)
    _CACHE[key] = surf
    return surf
