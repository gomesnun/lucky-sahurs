"""Peças base da interface: botões, clip, scrollbar, ícones, toasts."""

import math
import pygame

from config import TOPBAR_H
from theme import (
    ACCENT, ACCENT_HOVER, BAD, BLACK, BORDER_W,
    BORDER_W_SMALL, GREY_DIM, OUTLINE, PANEL,
    PANEL_LIGHT, PANEL_LIGHTER, PAUSED_RED, WHITE,
)
from ui.drawing import ALPHA_IS_SLOW, draw_panel, draw_state_border, rounded_box
from ui.fonts import fit_text
from ui.fonts import font_at as _font_at
from ui.icons import load_icon


class UIBaseMixin:
    """Peças base da interface: botões, clip, scrollbar, ícones, toasts."""

    def font_at(self, size, heavy=True, outline=None):
        """Fonte com o tamanho exato pedido, com cache (ver ui/fonts.py). Usado pelos cartões
        (ui/cards.py), que precisam de muitos tamanhos consoante o espaço disponível."""
        return _font_at(size, heavy=heavy, outline=outline)

    def show_toast(self, text, duration=1.8):
        """Aviso no topo do ecrã (duration em segundos). Um texto com quebras de linha mostra várias linhas."""
        self.toast_text = text
        self.toast_timer = duration
        self.toast_kind = None

    def draw_toast(self):
        if self.toast_timer > 0 and self.toast_text:
            lines = [self.font_med.render(part, True, WHITE) for part in self.toast_text.split("\n")]
            line_h = lines[0].get_height()
            w = max(t.get_width() for t in lines) + 40
            h = line_h * len(lines) + 4 * (len(lines) - 1) + 20
            surf = pygame.Surface((w, h), pygame.SRCALPHA)
            pygame.draw.rect(surf, (*PANEL, 245), surf.get_rect(), border_radius=10)
            pygame.draw.rect(surf, OUTLINE, surf.get_rect(), width=BORDER_W_SMALL, border_radius=10)
            for i, t in enumerate(lines):
                surf.blit(t, (20, 10 + i * (line_h + 4)))
            self.canvas.blit(surf, (self.vw // 2 - w // 2, TOPBAR_H + 10))

    # ---------------------------------------------------------------- clipping / botões
    def push_clip(self, rect):
        self.clip_stack.append(rect)
        self.canvas.set_clip(rect)

    def pop_clip(self):
        if self.clip_stack:
            self.clip_stack.pop()
        self.canvas.set_clip(self.clip_stack[-1] if self.clip_stack else None)

    def register_button(self, rect, callback, sfx="click"):
        if self.clip_stack:
            rect = rect.clip(self.clip_stack[-1])
            if rect.width <= 0 or rect.height <= 0:
                return
        self.buttons.append((rect, callback, sfx, self.nav_mode))

    def begin_modal(self):
        """Chamado mesmo antes de desenhar uma página por cima de tudo (Options / Stats / Traits):
        só ficam clicáveis os botões de navegação (Stats/Options no topo e os laterais).
        Tudo o resto (ROLL, painéis...) fica bloqueado, por isso um clique fora da página
        só a fecha, sem carregar em nada por baixo."""
        self.buttons = [b for b in self.buttons if b[3]]
        self.scrollbar_hits = {}      # as scrollbars que ficam por baixo da página também deixam de ser arrastáveis

    def clip_allows(self, rect):
        if not self.clip_stack:
            return True
        return self.clip_stack[-1].colliderect(rect)

    def button(self, rect, label, font, mouse_pos, base_color, hover_color, text_color,
               callback=None, radius=10, enabled=True, border_color=None, sfx="click", icon=None):
        if not self.clip_allows(rect):
            return False
        hovering = enabled and rect.collidepoint(mouse_pos) and (
            not self.clip_stack or self.clip_stack[-1].collidepoint(mouse_pos))
        color = base_color
        if not enabled:
            color = tuple(max(0, c - 55) for c in base_color)
        elif hovering:
            color = hover_color
        if ALPHA_IS_SLOW:
            # No telemovel desenhar sai muito mais barato do que copiar a imagem ja pronta, que
            # tem alfa nos cantos (0.068 ms contra 3.8 ms medidos). Ver ui/drawing.py.
            pygame.draw.rect(self.canvas, color, rect, border_radius=radius)
            pygame.draw.rect(self.canvas, OUTLINE, rect, width=BORDER_W_SMALL, border_radius=radius)
        else:
            self.canvas.blit(rounded_box(rect.width, rect.height, color, radius, BORDER_W_SMALL), rect)
        if border_color:
            draw_state_border(self.canvas, rect, border_color, radius)
        if label or icon:
            self.draw_button_content(rect, label, font, text_color if enabled else GREY_DIM, icon, enabled)
        if enabled and callback is not None:
            self.register_button(rect, callback, sfx)
        return hovering

    # Letras por ordem de tamanho, para reduzir o texto de um botão quando não cabe (Português é mais comprido).
    _FONTES_GROSSAS = ("font_huge", "font_big", "font_med", "font_small_b", "font_tiny_b")
    _FONTES_FINAS = ("font_small", "font_tiny")

    def fonts_menores(self, font):
        """As letras mais pequenas que 'font' (do mesmo estilo: grossas ou finas), da maior para a mais pequena."""
        finas = any(font is getattr(self, n, None) for n in self._FONTES_FINAS)
        nomes = self._FONTES_FINAS if finas else self._FONTES_GROSSAS
        return [f for f in (getattr(self, n) for n in nomes) if f.get_height() < font.get_height()]

    def texto_que_cabe(self, label, font, color, max_w):
        """Texto já desenhado que cabe em max_w pixéis: usa 'font' e, se for largo demais, letras mais
        pequenas; em último caso corta com reticências."""
        txt = font.render(label, True, color)
        if txt.get_width() <= max_w:
            return txt
        menores = self.fonts_menores(font)
        for f in menores:
            t = f.render(label, True, color)
            if t.get_width() <= max_w:
                return t
        ultima = menores[-1] if menores else font
        return ultima.render(fit_text(ultima, label, max_w - 4), True, color)      # -4: o contorno das letras

    def draw_button_content(self, rect, label, font, color, icon=None, enabled=True):
        """Conteúdo de um botão. Sem ícone: o texto centrado. Com 'icon' (icons/<icon>.png): o ícone fica
        encostado ao lado DIREITO e o texto fica centrado no espaço que sobra à esquerda dele (assim o
        texto não fica colado ao ícone). Se o texto não couber, usa letras mais pequenas (e só em último
        caso corta com reticências). Sem o ficheiro do ícone, só aparece o texto."""
        pad, gap = 8, 8
        img = load_icon(icon, max(16, rect.height - 12)) if icon else None
        txt = font.render(label, True, color) if label else None
        if img is None:
            if txt is not None:
                if txt.get_width() > rect.width - 6:
                    txt = self.texto_que_cabe(label, font, color, rect.width - 2 * pad)
                self.canvas.blit(txt, txt.get_rect(center=rect.center))
            return
        if not enabled:
            img = img.copy()
            img.set_alpha(110)
        if txt is None:
            self.canvas.blit(img, img.get_rect(center=rect.center))
            return
        icon_left = rect.right - pad - img.get_width()
        area_l, area_r = rect.x + pad, icon_left - gap            # o espaço livre para o texto
        if area_r - area_l < 24:                                   # botão muito estreito: texto + ícone lado a lado
            total = txt.get_width() + gap + img.get_width()
            x = rect.centerx - total // 2
            self.canvas.blit(txt, txt.get_rect(midleft=(x, rect.centery)))
            self.canvas.blit(img, img.get_rect(midleft=(x + txt.get_width() + gap, rect.centery)))
            return
        if txt.get_width() > area_r - area_l:
            txt = self.texto_que_cabe(label, font, color, area_r - area_l)
        self.canvas.blit(txt, txt.get_rect(center=((area_l + area_r) // 2, rect.centery)))
        self.canvas.blit(img, img.get_rect(midleft=(icon_left, rect.centery)))

    # ---------------------------------------------------------------- cabeçalho de painel
    def panel_header(self, rect, title, mouse_pos, close_cb):
        # tudo o que esteja por baixo do painel deixa de ser clicável (e sem som de clique)
        self.register_button(rect, lambda: None, None)
        draw_panel(self.canvas, rect, PANEL, radius=0, shadow=False, border=BORDER_W)

        ttxt = self.font_big.render(title, True, WHITE)
        self.canvas.blit(ttxt, (rect.x + 22, rect.y + 16))
        close_rect = pygame.Rect(rect.right - 44, rect.y + 18, 28, 28)
        self.button(close_rect, "X", self.font_small_b, mouse_pos, PANEL_LIGHT, BAD, WHITE,
                    callback=close_cb, radius=8)

    def draw_scrollbar(self, content_rect, scroll, content_h, key=None, mouse_pos=None):
        """Barra de scroll à direita de 'content_rect'. Se 'key' for passado, a barra fica arrastável
        com o rato (clicar e puxar para cima/baixo) - ver apply_scroll() e o tratamento do rato em main.py.
        'mouse_pos' só é preciso para o efeito de hover; sem ele a barra fica sempre na cor normal."""
        if content_h <= content_rect.height:
            return
        track = pygame.Rect(content_rect.right - 12, content_rect.top + 4, 10, content_rect.height - 8)
        pygame.draw.rect(self.canvas, (24, 24, 27), track, border_radius=5)
        pygame.draw.rect(self.canvas, OUTLINE, track, width=2, border_radius=5)
        frac = content_rect.height / float(content_h)
        bar_h = max(34, int(track.height * frac))
        max_scroll = content_h - content_rect.height
        t = 0.0 if max_scroll <= 0 else scroll / float(max_scroll)
        bar_y = track.top + int((track.height - bar_h) * t)
        thumb = pygame.Rect(track.x, bar_y, track.width, bar_h)

        dragging = key is not None and self.dragging_scrollbar == key
        hovering = (mouse_pos is not None and self.clip_allows(track)
                    and track.collidepoint(mouse_pos))
        if dragging:
            color = ACCENT_HOVER
        elif hovering:
            color = ACCENT
        else:
            color = PANEL_LIGHTER
        pygame.draw.rect(self.canvas, color, thumb, border_radius=5)
        pygame.draw.rect(self.canvas, OUTLINE, thumb, width=2, border_radius=5)
        if key is not None and self.clip_allows(track):
            hit_track = track.clip(self.clip_stack[-1]) if self.clip_stack else track
            if hit_track.width > 0 and hit_track.height > 0:
                self.scrollbar_hits[key] = (track, max_scroll, bar_h)

    # ---------------------------------------------------------------- botões laterais
    def draw_icon(self, kind, rect, color, bg=None):
        """Ícones dos botões laterais. Se existir icons/<kind>.png usa-o (a cores, sem tingir);
        senão desenha o ícone por código ('color' = cor do desenho, 'bg' = cor do botão, para os recortes)."""
        if bg is None:
            bg = PANEL_LIGHT            # lido aqui (e nao no def) para acompanhar o modo claro / escuro
        img = load_icon(kind, min(rect.width, rect.height) - 14)
        if img is not None:
            self.canvas.blit(img, img.get_rect(center=rect.center))
            return
        cx, cy = rect.centerx, rect.centery
        cv = self.canvas
        if kind == "index":
            # livro aberto
            pygame.draw.polygon(cv, color, [(cx - 1, cy - 8), (cx - 14, cy - 12), (cx - 14, cy + 9), (cx - 1, cy + 13)])
            pygame.draw.polygon(cv, color, [(cx + 1, cy - 8), (cx + 14, cy - 12), (cx + 14, cy + 9), (cx + 1, cy + 13)])
            for i in range(3):
                yy = cy - 5 + i * 5
                pygame.draw.line(cv, bg, (cx - 11, yy - 1), (cx - 4, yy + 1), 2)
                pygame.draw.line(cv, bg, (cx + 4, yy + 1), (cx + 11, yy - 1), 2)
        elif kind == "tree":
            # upgrades: seta para cima com listras por baixo
            pygame.draw.polygon(cv, color, [(cx, cy - 15), (cx + 13, cy - 1), (cx - 13, cy - 1)])
            for i in range(3):
                pygame.draw.rect(cv, color, pygame.Rect(cx - 5, cy + 3 + i * 5, 10, 3), border_radius=1)
        elif kind == "bag":
            # mochila de frente: pega, corpo redondo com aba, bolsos laterais e bolso da frente
            pygame.draw.rect(cv, color, pygame.Rect(cx - 6, cy - 21, 12, 13), width=3, border_radius=6)     # pega
            pygame.draw.rect(cv, color, pygame.Rect(cx - 19, cy + 2, 9, 17), border_radius=4)               # bolso esquerdo
            pygame.draw.rect(cv, color, pygame.Rect(cx + 10, cy + 2, 9, 17), border_radius=4)               # bolso direito
            pygame.draw.rect(cv, color, pygame.Rect(cx - 14, cy - 15, 28, 35), border_radius=12)            # corpo
            pygame.draw.line(cv, bg, (cx - 14, cy + 3), (cx - 14, cy + 18), 2)                              # separa os bolsos do corpo
            pygame.draw.line(cv, bg, (cx + 13, cy + 3), (cx + 13, cy + 18), 2)
            pygame.draw.lines(cv, bg, False, [(cx - 14, cy - 4), (cx - 1, cy + 1), (cx, cy + 1), (cx + 13, cy - 4)], 2)   # aba
            pygame.draw.rect(cv, bg, pygame.Rect(cx - 8, cy + 6, 16, 11), width=2, border_radius=4)         # bolso da frente
        elif kind == "trait":
            pts = []
            for i in range(10):
                ang = -math.pi / 2 + i * math.pi / 5
                r = 12 if i % 2 == 0 else 5
                pts.append((cx + r * math.cos(ang), cy + r * math.sin(ang)))
            pygame.draw.polygon(cv, color, pts)
        elif kind == "milestones":
            # troféu: taça + duas asas + haste + base
            pygame.draw.ellipse(cv, color, pygame.Rect(cx - 17, cy - 11, 10, 11), 3)
            pygame.draw.ellipse(cv, color, pygame.Rect(cx + 7, cy - 11, 10, 11), 3)
            pygame.draw.polygon(cv, color, [
                (cx - 10, cy - 13), (cx + 10, cy - 13), (cx + 9, cy - 4),
                (cx + 6, cy + 2), (cx + 2, cy + 4), (cx - 2, cy + 4),
                (cx - 6, cy + 2), (cx - 9, cy - 4)])
            pygame.draw.rect(cv, color, pygame.Rect(cx - 2, cy + 3, 4, 6))
            pygame.draw.rect(cv, color, pygame.Rect(cx - 8, cy + 9, 16, 4), border_radius=2)
        elif kind == "rebirth":
            # seta circular (ciclo / renascer): um arco quase fechado + ponta de seta
            arc_rect = pygame.Rect(cx - 13, cy - 13, 26, 26)
            pygame.draw.arc(cv, color, arc_rect, math.radians(35), math.radians(325), 4)
            ang = math.radians(35)
            tip = (cx + 13 * math.cos(ang), cy - 13 * math.sin(ang))
            pygame.draw.polygon(cv, color, [
                (tip[0] - 8, tip[1] - 3), (tip[0] + 3, tip[1] - 9), (tip[0] + 2, tip[1] + 5)])
        elif kind == "daily":
            # calendário com uma marca de "feito"
            pygame.draw.rect(cv, color, pygame.Rect(cx - 14, cy - 11, 28, 23), width=3, border_radius=4)
            pygame.draw.line(cv, color, (cx - 8, cy - 17), (cx - 8, cy - 9), 3)
            pygame.draw.line(cv, color, (cx + 8, cy - 17), (cx + 8, cy - 9), 3)
            pygame.draw.line(cv, color, (cx - 14, cy - 3), (cx + 14, cy - 3), 2)
            pygame.draw.lines(cv, color, False, [(cx - 6, cy + 4), (cx - 1, cy + 9), (cx + 8, cy - 3)], 3)

    def draw_badge(self, rect, count):
        """Bolinha vermelha com um número, no canto superior direito de 'rect' (nada se count <= 0)."""
        if count <= 0:
            return
        txt = self.font_small_b.render(str(count) if count < 100 else "99+", True, WHITE)
        h = 26
        w = max(h, txt.get_width() + 14)
        badge = pygame.Rect(0, 0, w, h)
        badge.center = (rect.right - 3, rect.top + 3)
        pygame.draw.rect(self.canvas, PAUSED_RED, badge, border_radius=h // 2)
        pygame.draw.rect(self.canvas, OUTLINE, badge, width=BORDER_W_SMALL, border_radius=h // 2)
        self.canvas.blit(txt, txt.get_rect(center=badge.center))

    def draw_alert_mark(self, rect):
        """'!!' vermelho (icons/alert.png) no canto superior direito de 'rect' - avisa que há algo para
        fazer (ex.: Rebirth disponível). Fica parado, sem animação."""
        size = 36
        center = (rect.right - 3, rect.top + 7)
        img = load_icon("alert", size)
        if img is None:
            # sem o ficheiro: bolinha vermelha com um '!'
            pygame.draw.circle(self.canvas, PAUSED_RED, center, 13)
            pygame.draw.circle(self.canvas, OUTLINE, center, 13, BORDER_W_SMALL)
            txt = self.font_small_b.render("!", True, WHITE)
            self.canvas.blit(txt, txt.get_rect(center=center))
            return
        self.canvas.blit(img, img.get_rect(center=center))

    def side_button(self, rect, label, icon, mouse_pos, active, callback, label_font=None,
                    badge=0, alert=False):
        """Botão lateral (ícone quadrado). O texto vai POR BAIXO do ícone, centrado: assim nunca fica em
        cima do Auto Roller nem encosta ao texto do botão do lado (com um painel aberto de cada lado
        não sobra espaço para o texto ao lado do ícone).
        badge = número vermelho (ex.: upgrades que dá para comprar); alert = '!!' vermelho."""
        hovering = rect.collidepoint(mouse_pos)
        base = ACCENT if active else PANEL_LIGHT
        if hovering and not active:
            base = PANEL_LIGHTER
        draw_panel(self.canvas, rect, base, radius=12, shadow=True)
        self.draw_icon(icon, rect, BLACK if active else WHITE, bg=base)

        if label:
            font = label_font or self.font_small_b
            txt = font.render(label, True, ACCENT if (hovering or active) else WHITE)
            self.canvas.blit(txt, txt.get_rect(midtop=(rect.centerx, rect.bottom + 4)))
        self.register_button(rect, callback)
        self.draw_badge(rect, badge)
        if alert:
            self.draw_alert_mark(rect)
