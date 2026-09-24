"""Painel de Options, com separadores (Gameplay / Interface / Volume / SFX / Game): cada separador
mostra só o que é dele, por isso a página tem sempre a MESMA altura (não cresce nem encolhe
quando se troca de separador, ao contrário do menu antigo que expandia por baixo)."""

import pygame

from config import IS_ANDROID, VIRTUAL_H
from core.pets import RARITIES, RARITY_TIERS, TIER_INDEX
from i18n import get_language, language_name, next_language, set_language, tr
from storage import CUTSCENE_RARITIES, SFX_CATEGORIES, save_settings
from theme import ACCENT, BAD, GREY, GREY_DIM, OUTLINE, PANEL, PANEL_LIGHT, PANEL_LIGHTER, WHITE
from ui.audio import SLIDER_KNOB_R
from ui.drawing import bake, dim_overlay, draw_panel
from ui.fonts import fit_text, wrap_text

ROW_H = 46               # distância entre linhas nos separadores Volume / SFX / cutscenes
CONTENT_H = 230           # altura fixa da área de conteúdo (a maior de todos os separadores cabe aqui)
TAB_H = 42
OFF_COLOR = (66, 52, 52)      # cor de um botão desligado (igual aos outros botões das Options)
TRACK_BG = (22, 22, 25)       # fundo da barra dos sliders

TABS = (("gameplay", "Gameplay"), ("interface", "Interface"), ("volume", "Volume"),
        ("sfx", "SFX"), ("game", "Game"))


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
        self.dragging_slider = None

    def close_options(self):
        self.options_open = False
        self.dragging_slider = None
        save_settings(self.settings)

    def toggle_options(self):
        if self.options_open:
            self.close_options()
        else:
            self.open_options()

    # ---------------------------------------------------------------- estado inicial / separadores
    def reset_options_ui(self):
        """Chamado uma vez ao abrir o jogo. O separador escolhido fica guardado entre aberturas."""
        self.dragging_slider = None
        self.slider_bars = {}
        self.slider_hits = {}
        self.options_tab = "gameplay"

    def set_options_tab(self, name):
        self.options_tab = name
        self.dragging_slider = None

    def cycle_language(self):
        """Botão Language: passa ao idioma seguinte (English -> Português -> ...), guarda e avisa.
        Como todo o texto é traduzido no momento de desenhar, o jogo muda logo, sem reiniciar."""
        code = set_language(next_language(get_language()))
        self.settings["language"] = code
        save_settings(self.settings)
        self.show_toast(tr("Language: %s", language_name(code)))

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

    # ---------------------------------------------------------------- separador: Volume
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
            if self.clip_stack:          # não deve acontecer aqui (a página já não tem clip animado), mas por segurança
                hit = hit.clip(self.clip_stack[-1])
            self.slider_bars[slider] = bar
            if hit.width > 0 and hit.height > 0:
                self.slider_hits[slider] = hit

    # ---------------------------------------------------------------- separador: SFX
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

    # ---------------------------------------------------------------- separador: Gameplay
    def draw_options_gameplay_tab(self, box, mouse_pos):
        x0, y, w = box.x, box.y, box.width

        def toggle_anim():
            self.settings["animations"] = not self.animations
            self.particles = []
            save_settings(self.settings)
            self.show_toast(tr("Animations on") if self.animations else tr("Animations off"))

        self.button(pygame.Rect(x0, y, w, 44),
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
        self.button(pygame.Rect(x0, y, w, 44),
                    tr("Trait notifications: %s", tr("On") if trait_notifs else tr("Off")),
                    self.font_med, mouse_pos,
                    PANEL_LIGHT if trait_notifs else OFF_COLOR, PANEL_LIGHTER, WHITE,
                    callback=toggle_trait_notifs, radius=10)
        y += 54

        # Cutscenes (Secret+): um interruptor por raridade, para dar para escolher só as que se
        # quer ver (ex.: só Transcendent) em vez de "tudo ligado" ou "tudo desligado".
        header = self.font_small_b.render(tr("Cutscenes (catching Secret+)") + ":", True, GREY)
        self.canvas.blit(header, (x0, y))
        y += 22

        col_w = (w - 8) // 2
        for i, key in enumerate(CUTSCENE_RARITIES):
            col, row = i % 2, i // 2
            r = pygame.Rect(x0 + col * (col_w + 8), y + row * ROW_H, col_w, 40)
            on = self.settings.get("cutscenes_" + key, True)
            rarity_name = tr(RARITY_TIERS[TIER_INDEX[key]]["name"])

            def toggle_cutscene_rarity(k=key):
                on2 = not self.settings.get("cutscenes_" + k, True)
                self.settings["cutscenes_" + k] = on2
                save_settings(self.settings)
                if not on2:
                    # limpa da fila só as desta raridade que estavam à espera; as outras ficam
                    self.cutscene_queue = [q for q in self.cutscene_queue if RARITIES[q[0]]["key"] != k]
                self.show_toast(tr("%s cutscenes: %s", tr(RARITY_TIERS[TIER_INDEX[k]]["name"]),
                                   tr("On") if on2 else tr("Off")))

            self.button(r, "%s: %s" % (rarity_name, tr("On") if on else tr("Off")), self.font_small_b,
                        mouse_pos, PANEL_LIGHT if on else OFF_COLOR, PANEL_LIGHTER, WHITE,
                        callback=toggle_cutscene_rarity, radius=9, sfx=None)

    # ---------------------------------------------------------------- separador: Interface
    def draw_options_interface_tab(self, box, mouse_pos):
        x0, y, w = box.x, box.y, box.width
        if not IS_ANDROID:          # no telemóvel o jogo é sempre em ecrã inteiro
            self.button(pygame.Rect(x0, y, w, 44),
                        tr("Fullscreen: %s", tr("On") if self.fullscreen else tr("Off")),
                        self.font_med, mouse_pos, PANEL_LIGHT, PANEL_LIGHTER, WHITE,
                        callback=self.toggle_fullscreen, radius=10)
            y += 54

        self.button(pygame.Rect(x0, y, w, 44),
                    tr("Theme: %s", self.theme_mode_label()),
                    self.font_med, mouse_pos, PANEL_LIGHT, PANEL_LIGHTER, WHITE,
                    callback=self.cycle_theme_mode, radius=10)
        y += 54

        self.button(pygame.Rect(x0, y, w, 44), tr("Language: %s", language_name()),
                    self.font_med, mouse_pos, PANEL_LIGHT, PANEL_LIGHTER, WHITE,
                    callback=self.cycle_language, radius=10)

    # ---------------------------------------------------------------- separador: Game
    def draw_options_game_tab(self, box, mouse_pos, in_game):
        x0, y, w = box.x, box.y, box.width
        half = (w - 10) // 2

        self.button(pygame.Rect(x0, y, w, 44), tr("Leaderboard"), self.font_med, mouse_pos,
                    PANEL_LIGHT, PANEL_LIGHTER, ACCENT, callback=self.open_leaderboard, radius=10,
                    icon="leaderboard")
        y += 54

        if self.event_is_admin:      # só a conta que tem /admins/{uid} na cloud é que vê isto
            self.button(pygame.Rect(x0, y, w, 44), tr("Global event"), self.font_med, mouse_pos,
                        PANEL_LIGHT, PANEL_LIGHTER, ACCENT, callback=self.toggle_event_admin, radius=10)
            y += 54

        if in_game:
            self.button(pygame.Rect(x0, y, w, 44), tr("Save now"), self.font_med, mouse_pos,
                        PANEL_LIGHT, PANEL_LIGHTER, WHITE,
                        callback=lambda: (self.state.save(), self.show_toast(tr("Progress saved!"))),
                        radius=10, icon="saves")
            y += 54
            # Main Menu à esquerda, Quit Game à direita (lado a lado, para as Options não ficarem tão altas)
            self.button(pygame.Rect(x0, y, half, 44), tr("Main Menu"), self.font_med, mouse_pos,
                        PANEL_LIGHT, PANEL_LIGHTER, WHITE, callback=self.go_to_menu, radius=10)
            self.button(pygame.Rect(x0 + half + 10, y, w - half - 10, 44), tr("Quit Game"), self.font_med,
                        mouse_pos, BAD, (250, 120, 120), WHITE, callback=self.quit_game, radius=10)

    # ---------------------------------------------------------------- opções
    def draw_options(self, mouse_pos):
        # Idem: só os botões de navegação ficam clicáveis por baixo (ver begin_modal).
        self.canvas.blit(dim_overlay(self.vw, VIRTUAL_H, 170), (0, 0))
        self.slider_hits = {}          # só voltam a ser registados abaixo se o separador Volume estiver aberto

        in_game = self.screen_mode == "game"
        panel_w = 460
        # A página tem SEMPRE a mesma altura, esteja qual separador estiver aberto (CONTENT_H já é
        # grande o suficiente para o separador com mais linhas): nunca "cresce" nem "encolhe".
        panel_h = 72 + TAB_H + 14 + CONTENT_H + 14 + 60
        top = max(8, VIRTUAL_H // 2 - panel_h // 2)
        rect = pygame.Rect(self.vw // 2 - panel_w // 2, top, panel_w, panel_h)
        draw_panel(self.canvas, rect, PANEL, radius=16)
        self.register_button(rect, lambda: None, None)     # clicar dentro da página não fecha

        title = self.font_big.render(tr("Options"), True, WHITE)
        self.canvas.blit(bake(title, PANEL), (rect.x + 24, rect.y + 20))
        close_rect = pygame.Rect(rect.right - 46, rect.y + 20, 28, 28)
        self.button(close_rect, "X", self.font_small_b, mouse_pos, PANEL_LIGHT, BAD, WHITE,
                    callback=self.close_options, radius=8)

        x0 = rect.x + 24
        btn_w = panel_w - 48

        # ---- separadores ----
        tab_y = rect.y + 72
        gap = 6
        tab_w = (btn_w - gap * (len(TABS) - 1)) // len(TABS)
        tx = x0
        for key, label in TABS:
            active = self.options_tab == key
            tab_rect = pygame.Rect(tx, tab_y, tab_w, TAB_H)
            self.button(tab_rect, tr(label), self.font_small_b, mouse_pos,
                        PANEL_LIGHTER if active else PANEL_LIGHT, PANEL_LIGHTER, WHITE,
                        callback=lambda k=key: self.set_options_tab(k), radius=9,
                        border_color=ACCENT if active else None)
            tx += tab_w + gap

        content_top = tab_y + TAB_H + 14
        content_box = pygame.Rect(x0, content_top, btn_w, CONTENT_H)

        tab = self.options_tab
        if tab == "gameplay":
            self.draw_options_gameplay_tab(content_box, mouse_pos)
        elif tab == "interface":
            self.draw_options_interface_tab(content_box, mouse_pos)
        elif tab == "volume":
            self.draw_volume_section(content_box, mouse_pos)
        elif tab == "sfx":
            self.draw_sfx_section(content_box, mouse_pos)
        else:
            self.draw_options_game_tab(content_box, mouse_pos, in_game)

        y = content_top + CONTENT_H + 14
        note = tr("Turning animations off removes the particles and the card effect — the game gets much "
                  "lighter. Progress is saved automatically every 10 seconds.") if in_game else \
               tr("Options are shared by all saves.")
        for line in wrap_text(note, self.font_tiny, btn_w):
            self.canvas.blit(bake(self.font_tiny.render(line, True, GREY), PANEL), (x0, y))
            y += 16
