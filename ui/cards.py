"""Cartões dos pets e das traits (estilo "brainrot": nome grande com contorno, raridade numa pílula e o
rendimento / a chance em placas escuras). Os cartões são quase quadrados para não ocuparem o ecrã todo."""

import pygame

from core.formatting import format_one_in
from core.pets import MUTATIONS
from core.traits import TRAITS
from i18n import tr
from theme import BORDER_W_SMALL, GOLD_BORDER, OUTLINE, PANEL_LIGHT, WHITE
from ui.drawing import draw_rarity_bg, mix, rounded_gradient, shade
from ui.fonts import fit_text, wrap_text

# Cor "viva" de cada raridade: o texto da pílula, o brilho por trás do cartão principal e as faíscas.
RARITY_GLOW = {
    "comum": (232, 234, 245), "incomum": (96, 232, 126), "raro": (92, 168, 255),
    "epico": (196, 118, 255), "lendario": (255, 216, 64), "mitico": (255, 88, 88),
    "exotico": (255, 168, 56), "secreto": (245, 245, 250), "divino": (255, 156, 210),
    "cosmico": (160, 130, 255), "transcendente": (130, 240, 255),
}

PLATE_FILL = (10, 11, 20, 196)         # placas escuras (rendimento, chance...)
PLATE_LABEL = (196, 202, 226)          # o "Income" pequenino de cada placa
PLATE_SUB = (150, 156, 184)            # a linha pequenina por baixo (chance base, etc.)
LOCKED_MARK = (150, 156, 184)          # o "?" dos pets / traits que ainda não tens

_CARD_CACHE = {}
_OVERLAY_CACHE = {}


def rarity_glow_color(rarity):
    """Cor viva da raridade deste pet."""
    return RARITY_GLOW.get(rarity["key"], (255, 255, 255))


def _remember(cache, key, surf, limit=320):
    if len(cache) >= limit:
        cache.clear()
    cache[key] = surf
    return surf


def _shade_overlay(w, h, radius):
    """Brilho suave no topo e sombra no fundo, por cima da cor da raridade (dá volume ao cartão)."""
    key = (w, h, radius)
    surf = _OVERLAY_CACHE.get(key)
    if surf is not None:
        return surf
    surf = pygame.Surface((w, h), pygame.SRCALPHA)
    for y in range(h):
        t = y / float(max(1, h - 1))
        if t < 0.34:
            pygame.draw.line(surf, (255, 255, 255, int(46 * (1.0 - t / 0.34))), (0, y), (w, y))
        elif t > 0.42:
            pygame.draw.line(surf, (0, 0, 0, int(118 * ((t - 0.42) / 0.58) ** 1.3)), (0, y), (w, y))
    mask = pygame.Surface((w, h), pygame.SRCALPHA)
    pygame.draw.rect(mask, (255, 255, 255, 255), mask.get_rect(), border_radius=radius)
    surf.blit(mask, (0, 0), special_flags=pygame.BLEND_RGBA_MIN)
    return _remember(_OVERLAY_CACHE, key, surf)


def _pill(text_surf, edge=None, pad_x=10, pad_y=2):
    """Pílula escura à volta de um texto já desenhado (com um contorno fininho na cor 'edge', se houver)."""
    w, h = text_surf.get_width() + 2 * pad_x, text_surf.get_height() + 2 * pad_y
    surf = pygame.Surface((w, h), pygame.SRCALPHA)
    pygame.draw.rect(surf, (6, 7, 16, 170), surf.get_rect(), border_radius=h // 2)
    if edge:
        pygame.draw.rect(surf, (edge[0], edge[1], edge[2], 190), surf.get_rect(), width=2, border_radius=h // 2)
    surf.blit(text_surf, text_surf.get_rect(center=(w // 2, h // 2)))
    return surf


class CardsMixin:
    """Cartões dos pets e das traits."""

    def clear_card_cache(self):
        """Esquece os cartões já feitos (quando o tema muda, o contorno deles muda de cor)."""
        _CARD_CACHE.clear()
        _OVERLAY_CACHE.clear()

    # ---------------------------------------------------------------- medidas e peças
    @staticmethod
    def _card_scale(w):
        """Fator de tamanho do cartão (1.0 = o cartão principal, ~236 px de largura)."""
        return max(0.55, min(1.3, w / 236.0))

    def _plate_fonts(self, s):
        """(etiqueta, valor, linha pequenina) para o fator de tamanho s."""
        return (self.font_at(max(11, int(round(12 * s))), heavy=False, outline=0),
                self.font_at(max(13, int(round(19 * s))), heavy=True),
                self.font_at(max(10, int(round(12 * s))), heavy=False, outline=0))

    @staticmethod
    def _plate_pad(s):
        return max(3, int(round(4 * s)))

    def _plate_height(self, cells, fonts, s):
        label_f, value_f, sub_f = fonts
        h = label_f.get_height() + value_f.get_height() - 2
        if any(len(c) > 2 and c[2] for c in cells):
            h += sub_f.get_height() - 2
        return h + 2 * self._plate_pad(s)

    def _fit_value(self, text, size, max_w):
        """Valor (texto grosso) que cabe em max_w: reduz a letra e, no fim, corta com '...'."""
        while size > 11:
            f = self.font_at(size, heavy=True)
            if f.size(text)[0] + 2 * f.outline <= max_w:
                return f.render(text, True, WHITE)
            size -= 1
        f = self.font_at(11, heavy=True)
        return f.render(fit_text(f, text, max_w - 2 * f.outline), True, WHITE)

    def _draw_plate(self, surf, rect, cells, fonts, s):
        """Uma placa escura com 1 ou 2 'células' (etiqueta pequena em cima, valor grande por baixo)."""
        label_f, value_f, sub_f = fonts
        pygame.draw.rect(surf, PLATE_FILL, rect, border_radius=max(6, int(round(9 * s))))
        cell_w = rect.width // len(cells)
        value_size = max(13, int(round(19 * s)))
        for i, cell in enumerate(cells):
            label, value = cell[0], cell[1]
            sub = cell[2] if len(cell) > 2 else None
            cx = rect.x + i * cell_w + cell_w // 2
            y = rect.y + self._plate_pad(s)
            lab = label_f.render(fit_text(label_f, label, cell_w - 8), True, PLATE_LABEL)
            surf.blit(lab, lab.get_rect(midtop=(cx, y)))
            y += label_f.get_height() - 2
            val = self._fit_value(value, value_size, cell_w - 8)
            surf.blit(val, val.get_rect(midtop=(cx, y - 1)))
            if sub:
                y += value_f.get_height() - 2
                sub_t = sub_f.render(fit_text(sub_f, sub, cell_w - 8), True, PLATE_SUB)
                surf.blit(sub_t, sub_t.get_rect(midtop=(cx, y)))
            if i > 0:                                        # traço fininho entre as duas células
                pygame.draw.line(surf, (255, 255, 255, 34), (rect.x + i * cell_w, rect.y + 6),
                                 (rect.x + i * cell_w, rect.bottom - 7), 1)

    def _fit_name(self, name, base_size, max_w, max_h, extra_h):
        """Escolhe a maior letra (a partir de base_size) para o nome caber em max_w x max_h, sem contar
        extra_h (o que vai por baixo do nome). Devolve (linhas já desenhadas, distância entre linhas, altura)."""
        size = base_size
        while True:
            font = self.font_at(size, heavy=True)
            lines = wrap_text(name, font, max_w)
            pitch = font.get_height() - max(2, size // 8)
            widest = max(font.size(t)[0] + 2 * font.outline for t in lines)
            total = len(lines) * pitch + 2 * font.outline + extra_h
            if (widest <= max_w and total <= max_h) or size <= 12:
                break
            size -= 1
        return [font.render(t, True, WHITE) for t in lines], pitch, total

    # ---------------------------------------------------------------- cartões de pet
    def render_pet_card(self, rarity, mutation, w, h, plates=(), footer_h=0, locked=False):
        """Cartão do pet, quase quadrado: nome grande com contorno, a raridade numa pílula e, em baixo,
        placas escuras com o rendimento e a chance. A mutação é só a borda (dourada / azul); o fundo é
        sempre a cor da raridade.

        plates    lista de placas; cada placa é uma lista de 1 ou 2 células (etiqueta, valor[, linha pequena]),
                  ex.: [[("Income", "+1.8K/sec")], [("Have", "3"), ("Equipped", "1")]]
        footer_h  altura livre em baixo (para os botões + / - do Inventory, que são desenhados por cima)
        locked    pet ainda não apanhado (Index): fundo escuro com a cor da raridade e um '?' no lugar do nome

        O resultado fica em cache (só se refaz se mudar o texto, o tamanho ou o tema): não o alteres."""
        key = (rarity["pet"], mutation, tr(rarity["name"]), w, h,
               tuple(tuple(tuple(c) for c in p) for p in plates), footer_h, locked)
        surf = _CARD_CACHE.get(key)
        if surf is not None:
            return surf

        s = self._card_scale(w)
        radius = max(9, int(round(16 * s)))
        pad = max(6, int(round(10 * s)))
        gap = max(3, int(round(5 * s)))
        surf = pygame.Surface((w, h), pygame.SRCALPHA)
        rect = pygame.Rect(0, 0, w, h)

        # ---- fundo (a cor da raridade; bloqueado = um tom escuro dessa cor) ----
        if locked:
            dark = mix(rarity["color"], (16, 18, 30), 0.80)
            surf.blit(rounded_gradient(w, h, shade(dark, 1.25), shade(dark, 0.80), radius), (0, 0))
        else:
            draw_rarity_bg(surf, rect, rarity, radius=radius)
            surf.blit(_shade_overlay(w, h, radius), (0, 0))

        # ---- borda: mutação (dourada / diamante) ou o contorno normal ----
        border = MUTATIONS[mutation]["border"]
        if border:
            pygame.draw.rect(surf, OUTLINE, rect, width=2, border_radius=radius)
            pygame.draw.rect(surf, border, rect.inflate(-4, -4), width=max(3, int(round(4 * s))),
                             border_radius=max(4, radius - 2))
        else:
            pygame.draw.rect(surf, OUTLINE, rect, width=BORDER_W_SMALL, border_radius=radius)

        # ---- medidas: as placas ficam em baixo; o nome + a pílula ficam no espaço que sobra ----
        fonts = self._plate_fonts(s)
        heights = [self._plate_height(p, fonts, s) for p in plates]
        plates_h = sum(heights) + gap * max(0, len(plates) - 1)
        bottom = h - pad - footer_h - (gap if footer_h else 0)
        plates_top = bottom - plates_h
        inner_w = w - 2 * pad
        area_top, area_bottom = pad, plates_top - gap

        accent = rarity_glow_color(rarity)
        pill_size = max(11, int(round(16 * s)))
        pill = _pill(self.font_at(pill_size, heavy=True).render(tr(rarity["name"]), True, accent), edge=accent)
        if locked:
            q = self.font_at(max(20, int(round(58 * s))), heavy=True).render("?", True, LOCKED_MARK)
            block_h = q.get_height() - 8 + gap + pill.get_height()
            y = area_top + max(0, (area_bottom - area_top - block_h) // 2)
            surf.blit(q, q.get_rect(midtop=(w // 2, y - 6)))
            surf.blit(pill, pill.get_rect(midtop=(w // 2, y + q.get_height() - 8 + gap)))
        else:
            base_size = max(13, int(round(33 * s)))
            lines, pitch, total = self._fit_name(rarity["pet"], base_size, inner_w - 4,
                                                 area_bottom - area_top, gap + 2 + pill.get_height())
            y = area_top + max(0, (area_bottom - area_top - total) // 2)
            for t in lines:
                surf.blit(t, t.get_rect(midtop=(w // 2, y)))
                y += pitch
            surf.blit(pill, pill.get_rect(midtop=(w // 2, y + gap + 2)))

        # ---- placas ----
        y = plates_top
        for cells, ph in zip(plates, heights):
            self._draw_plate(surf, pygame.Rect(pad, y, inner_w, ph), cells, fonts, s)
            y += ph + gap
        return _remember(_CARD_CACHE, key, surf)

    # ---------------------------------------------------------------- cartões de trait
    def render_trait_card(self, trait_index, w, h, owned, equipped):
        """Cartão de uma trait: o mesmo estilo dos pets (nome com contorno + placa com a chance)."""
        trait = TRAITS[trait_index]
        chance_text = format_one_in(self.state.trait_chance(trait_index))
        key = ("trait", trait_index, w, h, owned, equipped, chance_text, tr("Chance"), tr("EQUIPPED"))
        surf = _CARD_CACHE.get(key)
        if surf is not None:
            return surf

        s = max(0.6, min(1.2, min(w / 250.0, h / 150.0) * 1.05))
        radius = max(9, int(round(14 * s)))
        pad = max(6, int(round(9 * s)))
        surf = pygame.Surface((w, h), pygame.SRCALPHA)
        rect = pygame.Rect(0, 0, w, h)
        fonts = self._plate_fonts(s)
        cells = [(tr("Chance"), chance_text)]
        plate_h = self._plate_height(cells, fonts, s)
        plate_rect = pygame.Rect(pad, h - pad - plate_h, w - 2 * pad, plate_h)
        area_h = plate_rect.top - 2 * pad

        if not owned:
            surf.blit(rounded_gradient(w, h, shade(PANEL_LIGHT, 1.10), shade(PANEL_LIGHT, 0.84), radius), (0, 0))
            pygame.draw.rect(surf, OUTLINE, rect, width=BORDER_W_SMALL, border_radius=radius)
            q = self.font_at(max(24, int(round(52 * s))), heavy=True).render("?", True, LOCKED_MARK)
            surf.blit(q, q.get_rect(center=(w // 2, pad + area_h // 2 - 2)))
            self._draw_plate(surf, plate_rect, cells, fonts, s)
            return _remember(_CARD_CACHE, key, surf)

        base = trait["color"]
        surf.blit(rounded_gradient(w, h, shade(base, 1.12), shade(base, 0.84), radius), (0, 0))
        surf.blit(_shade_overlay(w, h, radius), (0, 0))
        if equipped:
            pygame.draw.rect(surf, OUTLINE, rect, width=2, border_radius=radius)
            pygame.draw.rect(surf, GOLD_BORDER, rect.inflate(-4, -4), width=4, border_radius=max(4, radius - 2))
        else:
            pygame.draw.rect(surf, OUTLINE, rect, width=BORDER_W_SMALL, border_radius=radius)

        # nome grande com contorno e, se estiver equipada, a pílula EQUIPPED por baixo dele
        pill = None
        if equipped:
            pill = _pill(self.font_at(max(11, int(round(13 * s))), heavy=True).render(tr("EQUIPPED"), True, GOLD_BORDER),
                         edge=GOLD_BORDER)
        pill_h = (pill.get_height() + 3) if pill else 0
        lines, pitch, total = self._fit_name(tr(trait["name"]), max(14, int(round(32 * s))), w - 2 * pad - 4,
                                             area_h, pill_h)
        y = pad + max(0, (area_h - total) // 2)
        for t in lines:
            surf.blit(t, t.get_rect(midtop=(w // 2, y)))
            y += pitch
        if pill:
            surf.blit(pill, pill.get_rect(midtop=(w // 2, y + 3)))
        self._draw_plate(surf, plate_rect, cells, fonts, s)
        return _remember(_CARD_CACHE, key, surf)