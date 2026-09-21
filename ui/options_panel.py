"""Painel de Options."""

import pygame

from config import VIRTUAL_H
from i18n import get_language, language_name, next_language, set_language, tr
from storage import SFX_CATEGORIES, save_settings
from theme import ACCENT, BAD, GREY, GREY_DIM, OUTLINE, PANEL, PANEL_LIGHT, PANEL_LIGHTER, WHITE
from ui.audio import SLIDER_KNOB_R
from ui.drawing import draw_panel, ease_out_cubic
from ui.fonts import fit_text, wrap_text

DROP_H = 150            # altura do menu que expande (Volume ou SFX) quando está todo aberto
DROP_GAP = 10           # espaço entre o menu que expande e o botão seguinte
DROP_SPEED = 7.0        # velocidade da animação (1 / segundos para abrir por completo)
ROW_H = 46              # distância entre linhas dentro do menu que expande
OFF_COLOR = (66, 52, 52)      # cor de um botão desligado (igual aos outros botões das Options)
DROP_BG = (42, 42, 46)        # fundo do menu que expande
TRACK_BG = (22, 22, 25)       # fundo da barra dos sliders


class OptionsPanelMixin:
    """Painel de Options."""

    # Options, Stats, Traits e Rebirth nunca estão abertos ao mesmo tempo: abrir um fecha os outros.
    # (Os botões do topo e os laterais continuam clicáveis com qualquer um deles aberto.)
    def open_options(self):
        if self.stats_open:
            self.close_stats()
        self.traits_open = False
        self.rebirth_open = False
        self.rebirth_confirm = False
        self.leaderboard_open = False
        self.credits_open = False
        self.options_open = True
        self.reset_options_dropdown()

    def close_options(self):
        self.options_open = False
        self.reset_options_dropdown()
        save_settings(self.settings)

    def toggle_options(self):
        if self.options_open:
            self.close_options()
        else:
            self.open_options()

    # ---------------------------------------------------------------- menus que expandem (Volume / SFX)
    def reset_options_dropdown(self):
        """As Options abrem sempre com o Volume e o SFX fechados."""
        self.dragging_slider = None
        self.slider_bars = {}
        self.slider_hits = {}
        self.options_section = None       # o que o jogador quer aberto: None | "volume" | "sfx"
        self.options_shown = None         # o que está a ser mostrado (só muda quando o outro acaba de fechar)
        self.options_prog = 0.0           # 0 = fechado, 1 = aberto por completo

    def cycle_language(self):
        """Botão Language: passa ao idioma seguinte (English -> Português -> ...), guarda e avisa.
        Como todo o texto é traduzido no momento de desenhar, o jogo muda logo, sem reiniciar."""
        code = set_language(next_language(get_language()))
        self.settings["language"] = code
        save_settings(self.settings)
        self.show_toast(tr("Language: %s", language_name(code)))

    def toggle_options_section(self, name):
        self.options_section = None if self.options_section == name else name

    def update_options_dropdown(self, dt):
        """Só um menu de cada vez: ao passar do Volume para o SFX, o primeiro fecha e depois o segundo abre."""
        if self.options_shown != self.options_section:
            self.dragging_slider = None
            self.options_prog = max(0.0, self.options_prog - dt * DROP_SPEED)
            if self.options_prog <= 0.0:
                self.options_shown = self.options_section
        elif self.options_shown is not None:
            self.options_prog = min(1.0, self.options_prog + dt * DROP_SPEED)

    def draw_slider(self, bar, value, active):
        """Slider com contorno: barra escura, enchimento dourado e bolinha branca com contorno preto."""
        r = SLIDER_KNOB_R
        half = bar.height // 2
        knob_x = bar.x + r + int((bar.width - 2 * r) * value)
        pygame.draw.rect(self.canvas, TRACK_BG, bar, border_radius=half)
        fill = pygame.Rect(bar.x, bar.y, knob_x - bar.x, bar.height)
        if fill.width > 0:
            pygame.draw.rect(self.canvas, ACCENT if active else GREY_DIM, fill, border_radius=half)
        pygame.draw.rect(self.canvas, OUTLINE, bar, width=3, border_radius=half)
        pygame.draw.circle(self.canvas, OUTLINE, (knob_x, bar.centery), r + 2)
        pygame.draw.circle(self.canvas, WHITE if active else GREY, (knob_x, bar.centery), r - 1)

    def draw_dropdown_arrow(self, rect, is_open):
        cx, cy = rect.right - 20, rect.centery
        if is_open:
            pts = [(cx - 6, cy + 3), (cx + 6, cy + 3), (cx, cy - 4)]
        else:
            pts = [(cx - 6, cy - 3), (cx + 6, cy - 3), (cx, cy + 4)]
        pygame.draw.polygon(self.canvas, WHITE, pts)

    def draw_volume_section(self, box, mouse_pos):
        """Master / Music / SFX: cada um com o seu slider (clica ou arrasta) e um botão On/Off."""
        cfg = self.settings
        inner_x, inner_w = box.x + 12, box.width - 24
        master_on = cfg.get("sound_on", True)
        rows = (
            ("master", tr("Master volume"), "volume", 0.6, "sound_on", self.toggle_sound),
            ("music", tr("Music volume"), "music_volume", 0.5, "music_on", self.toggle_music),
            ("sfx", tr("SFX volume"), "sfx_volume", 1.0, "sfx_on", self.toggle_sfx),
        )
        for i, (slider, name, vol_key, default, on_key, toggle) in enumerate(rows):
            ry = box.y + 8 + i * ROW_H
            vol = max(0.0, min(1.0, float(cfg.get(vol_key, default))))
            switch_on = cfg.get(on_key, True)
            active = switch_on and (slider == "master" or master_on)     # "Master: Off" cala tudo

            label = "%s: %d%%" % (name, int(round(vol * 100)))
            if slider == "master" and not self.mixer_ok:
                label += "   " + tr("(no audio on this PC)")
            pill_text = tr("On") if switch_on else tr("Off")
            pill_w = max(52, self.font_tiny.size(pill_text)[0] + 20)       # "Desligado" é mais largo que "Off"
            label = fit_text(self.font_small_b, label, inner_w - pill_w - 10)
            self.canvas.blit(self.font_small_b.render(label, True, WHITE if active else GREY), (inner_x, ry + 1))

            pill = pygame.Rect(inner_x + inner_w - pill_w, ry - 1, pill_w, 20)
            self.button(pill, pill_text, self.font_tiny, mouse_pos,
                        PANEL_LIGHT if switch_on else OFF_COLOR, PANEL_LIGHTER, WHITE,
                        callback=toggle, radius=8, sfx="click" if slider == "music" else None)

            bar = pygame.Rect(inner_x, ry + 26, inner_w, 16)
            self.draw_slider(bar, vol, active)
            hit = pygame.Rect(bar.x - SLIDER_KNOB_R, bar.y - 6, bar.width + 2 * SLIDER_KNOB_R, bar.height + 8)
            if self.clip_stack:          # a parte que ainda está escondida (a abrir/fechar) não responde ao rato
                hit = hit.clip(self.clip_stack[-1])
            self.slider_bars[slider] = bar
            if hit.width > 0 and hit.height > 0:
                self.slider_hits[slider] = hit

    def draw_sfx_section(self, box, mouse_pos):
        """Um botão por grupo de sons (2 colunas), para ligar/desligar cada um."""
        inner_x, inner_w = box.x + 12, box.width - 24
        col_w = (inner_w - 8) // 2
        for i, (cat, label) in enumerate(SFX_CATEGORIES):
            col, row = i % 2, i // 2
            r = pygame.Rect(inner_x + col * (col_w + 8), box.y + 8 + row * ROW_H, col_w, 40)
            on = self.settings.get("sfx_" + cat, True)
            self.button(r, "%s: %s" % (tr(label), tr("On") if on else tr("Off")), self.font_small_b, mouse_pos,
                        PANEL_LIGHT if on else OFF_COLOR, PANEL_LIGHTER, WHITE,
                        callback=lambda c=cat: self.toggle_sfx_category(c), radius=9, sfx=None)

    # ---------------------------------------------------------------- opções
    def draw_options(self, mouse_pos):
        # Idem: só os botões de navegação ficam clicáveis por baixo (ver begin_modal).
        overlay = pygame.Surface((self.vw, VIRTUAL_H), pygame.SRCALPHA)
        overlay.fill((0, 0, 0, 170))
        self.canvas.blit(overlay, (0, 0))

        self.update_options_dropdown(self.frame_dt)
        self.slider_hits = {}          # voltam a ser registados abaixo, só se o menu Volume estiver aberto

        in_game = self.screen_mode == "game"
        panel_w = 420
        # Altura sem o menu que expande: título + linhas de botões + nota. A página fica sempre centrada no
        # ecrã: quando o menu Volume / SFX abre ela cresce para cima e para baixo, e volta a centrar-se ao fechar.
        rows = 3 + 1 + 1 + 1 + (2 if in_game else 0)     # 3 toggles, Language, Volume|SFX, Leaderboard, [Save now, Main Menu|Quit]
        base_h = 72 + 54 * rows + 76
        open_amount = ease_out_cubic(self.options_prog) if self.options_shown else 0.0
        extra = int((DROP_H + DROP_GAP) * open_amount)       # quanto o menu já cresceu
        panel_h = base_h + extra
        top = max(8, VIRTUAL_H // 2 - panel_h // 2)
        rect = pygame.Rect(self.vw // 2 - panel_w // 2, top, panel_w, panel_h)
        draw_panel(self.canvas, rect, PANEL, radius=16)
        self.register_button(rect, lambda: None, None)     # clicar dentro da página não fecha

        title = self.font_big.render(tr("Options"), True, WHITE)
        self.canvas.blit(title, (rect.x + 24, rect.y + 20))
        close_rect = pygame.Rect(rect.right - 46, rect.y + 20, 28, 28)
        self.button(close_rect, "X", self.font_small_b, mouse_pos, PANEL_LIGHT, BAD, WHITE,
                    callback=self.close_options, radius=8)

        y = rect.y + 72
        btn_w = panel_w - 48
        x0 = rect.x + 24

        self.button(pygame.Rect(x0, y, btn_w, 44),
                    tr("Fullscreen: %s", tr("On") if self.fullscreen else tr("Off")),
                    self.font_med, mouse_pos, PANEL_LIGHT, PANEL_LIGHTER, WHITE,
                    callback=self.toggle_fullscreen, radius=10)
        y += 54

        def toggle_anim():
            self.settings["animations"] = not self.animations
            self.particles = []
            save_settings(self.settings)
            self.show_toast(tr("Animations on") if self.animations else tr("Animations off"))

        self.button(pygame.Rect(x0, y, btn_w, 44),
                    tr("Animations: %s", tr("On") if self.animations else tr("Off")),
                    self.font_med, mouse_pos,
                    PANEL_LIGHT if self.animations else OFF_COLOR, PANEL_LIGHTER, WHITE,
                    callback=toggle_anim, radius=10)
        y += 54

        def toggle_trait_notifs():
            on = not self.settings.get("trait_notifications", True)
            self.settings["trait_notifications"] = on
            if not on and self.toast_kind == "trait":
                self.toast_timer = 0.0
            save_settings(self.settings)

        trait_notifs = self.settings.get("trait_notifications", True)
        self.button(pygame.Rect(x0, y, btn_w, 44),
                    tr("Trait notifications: %s", tr("On") if trait_notifs else tr("Off")),
                    self.font_med, mouse_pos,
                    PANEL_LIGHT if trait_notifs else OFF_COLOR, PANEL_LIGHTER, WHITE,
                    callback=toggle_trait_notifs, radius=10)
        y += 54

        # ---- idioma: um botão que passa ao idioma seguinte (mostra sempre o nome no próprio idioma) ----
        self.button(pygame.Rect(x0, y, btn_w, 44), tr("Language: %s", language_name()),
                    self.font_med, mouse_pos, PANEL_LIGHT, PANEL_LIGHTER, WHITE,
                    callback=self.cycle_language, radius=10)
        y += 54

        # ---- Volume | SFX: dois botões lado a lado; cada um expande um menu por baixo (só um aberto de cada vez) ----
        half = (btn_w - 10) // 2
        for i, (name, label) in enumerate((("volume", tr("Volume")), ("sfx", tr("SFX")))):
            hb = pygame.Rect(x0 + i * (half + 10), y, half if i == 0 else btn_w - half - 10, 44)
            is_open = self.options_section == name
            self.button(hb, label, self.font_med, mouse_pos,
                        PANEL_LIGHTER if is_open else PANEL_LIGHT, PANEL_LIGHTER, WHITE,
                        callback=lambda n=name: self.toggle_options_section(n), radius=10,
                        border_color=ACCENT if is_open else None)
            self.draw_dropdown_arrow(hb, is_open)
        y += 54

        box_h = extra - DROP_GAP
        if box_h > 0:
            box = pygame.Rect(x0, y, btn_w, box_h)
            pygame.draw.rect(self.canvas, DROP_BG, box, border_radius=10)
            self.push_clip(box)
            # o conteúdo tem posição fixa e vai sendo revelado à medida que a caixa cresce
            content = pygame.Rect(box.x, box.y, box.width, DROP_H)
            if self.options_shown == "volume":
                self.draw_volume_section(content, mouse_pos)
            elif self.options_shown == "sfx":
                self.draw_sfx_section(content, mouse_pos)
            self.pop_clip()
            pygame.draw.rect(self.canvas, OUTLINE, box, width=3, border_radius=10)
        y += extra

        self.button(pygame.Rect(x0, y, btn_w, 44), tr("Leaderboard"), self.font_med, mouse_pos,
                    PANEL_LIGHT, PANEL_LIGHTER, ACCENT, callback=self.open_leaderboard, radius=10,
                    icon="leaderboard")
        y += 54

        if in_game:
            self.button(pygame.Rect(x0, y, btn_w, 44), tr("Save now"), self.font_med, mouse_pos,
                        PANEL_LIGHT, PANEL_LIGHTER, WHITE,
                        callback=lambda: (self.state.save(), self.show_toast(tr("Progress saved!"))),
                        radius=10, icon="saves")
            y += 54
            # Main Menu à esquerda, Quit Game à direita (lado a lado, para as Options não ficarem tão altas)
            self.button(pygame.Rect(x0, y, half, 44), tr("Main Menu"), self.font_med, mouse_pos,
                        PANEL_LIGHT, PANEL_LIGHTER, WHITE, callback=self.go_to_menu, radius=10)
            self.button(pygame.Rect(x0 + half + 10, y, btn_w - half - 10, 44), tr("Quit Game"), self.font_med,
                        mouse_pos, BAD, (250, 120, 120), WHITE, callback=self.quit_game, radius=10)
            y += 54

        note = tr("Turning animations off removes the particles and the card effect — the game gets much "
                  "lighter. Progress is saved automatically every 10 seconds.") if in_game else \
               tr("Options are shared by all saves.")
        for line in wrap_text(note, self.font_tiny, btn_w):
            self.canvas.blit(self.font_tiny.render(line, True, GREY), (x0, y + 8))
            y += 16
