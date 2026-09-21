"""
LUCKY SAHURS  -  v7
===================
Roll pets, equip them to earn money, buy upgrades and collect traits.

How to play:
    Click the ROLL button to roll a pet (rolling is mouse-only).
    Side buttons: Index / Upgrades / Milestones / Daily (right side), Bag / Rebirth / Traits (left side).
    Top bar: Stats (rolls, slots, playtime...) and Options.

Shortcuts:
    U / I / M / B -> open the Upgrades / Index / Milestones / Bag panels
    F11           -> toggle fullscreen / windowed
    ESC           -> close panels / Traits page / Stats / Options, or go back in the menus

How to run:
    pip install pygame
    python main.py

Para criar o .exe (para mandar aos amigos): duplo clique em build_exe.bat
(instala o pygame + pyinstaller e cria dist\\Lucky Sahurs.exe).

Progress is stored in savegame_slot1.json / slot2 / slot3 in the same folder as main.py
(an old savegame.json is copied to Slot 1 automatically).
Settings (sound, volume, animations, fullscreen) are stored in lucky_sahurs_settings.json.

Icons: the PNGs in the "icons" folder are used for the side buttons, the money in the top bar, the
Stats / Options / Leaderboard / Saves buttons, the red "!!" on the Rebirth button (alert.png) and the
mascot on the main menu (tung.png). Delete or replace any of them - a missing file falls back to the
drawn icon (or simply to no icon).

Sounds: the files in the "sounds" folder are the click / upgrade / achievement / rebirth sounds and the
background music (music.ogg). Their credits are in sounds/CREDITS.txt and in the Credits button of the
main menu. Replace or delete any file (a missing file falls back to a generated beep / no music).

Font: the game uses the first .ttf / .otf inside the "fonts" folder next to main.py (it ships with
Fredoka Bold, licence in fonts/OFL.txt). Drop another font there to change every text.

Theme: Options -> "Theme" cycles Windows default (follows the system, default) / Dark / Light. Colours live in theme.py.

MAPA DOS FICHEIROS
------------------
main.py                 este ficheiro: junta tudo na classe Game e tem o ciclo principal
build_exe.bat           cria o executável (.exe) com o PyInstaller
config.py               constantes gerais (janela, versão, pastas de saves)
theme.py                cores do estilo cartoon
storage.py              saves locais (slots) e definições

core/                   as regras do jogo (não desenham nada)
    balance.py          TODOS os números de balanceamento (dinheiro, sorte, auto roller, anti auto-clicker...)
    game_state.py       GameState = o save completo (junta os ficheiros abaixo)
    pets.py             pets, raridades, mutações, chances e roll
    traits.py           traits
    upgrades.py         upgrades e os Golden / Diamond / Rainbow Roll
    milestones.py       metas e recompensas
    economy.py          dinheiro por segundo, multiplicadores, auto-roll
    rebirths.py         rebirths: reset de dinheiro/upgrades por bónus permanente
    daily_missions.py   missões diárias (3 por dia, recompensa em cargas de trait)
    offline.py          ganhos offline (30% do rendimento, máx. 8 h)
    formatting.py       formatar números, chances e tempo

online/                 conta e cloud
    firebase.py         cliente do Firebase (contas + Firestore) e Worker de fundo
    cloud_cache.py      sessão guardada, cache local dos saves da conta
    cloud.py            lógica online do jogo (login, sincronizar, leaderboard)
    updater.py          atualizações automáticas: vê no GitHub se há versão nova, descarrega e troca o programa
    tls.py              certificados HTTPS (certifi) para o online e as atualizações

ui/                     tudo o que se vê / ouve
    fonts.py, drawing.py, widgets.py        peças de desenho e texto
    icons.py                                carrega os PNG da pasta icons/ (com fallback)
    base.py, cards.py                       botões, scrollbar, cartões dos pets e traits
    audio.py                                som e música (ficheiros da pasta sounds/)
    title_saves.py                          ecrãs do título e dos saves
    game_screen.py, gameplay.py             ecrã principal e roll / auto-roll / partículas
    pets_panel.py                           Index e Bag (Equipados / Inventory)
    upgrades_panel.py, milestones_panel.py  Upgrade Tree e Milestones
    traits_panel.py, options_panel.py       página de Traits e Options
    rebirth_panel.py, daily_panel.py        página de Rebirths e painel de Missões Diárias
    account_panel.py, leaderboard_panel.py  conta e leaderboard
    credits_panel.py                        página de Credits (sons e música)
    update_panel.py                         ecrã "Nova versão disponível" (atualização obrigatória)
"""


import os
import sys
import time
import traceback
import pygame

import theme
from config import (AUTOSAVE_INTERVAL, FPS, GAME_TITLE, IS_ANDROID, SAVE_DIR, TOUCH_DRAG_SLOP,
                    VIRTUAL_H, VW_MAX, VW_MIN)
from core.game_state import GameState

# Medicao de desempenho: no Android escreve os FPS no logcat de 2 em 2 segundos (e a unica forma de
# medir no telemovel); no computador so com a variavel de ambiente LUCKY_SAHURS_PERF=1.
PERF_LOG = IS_ANDROID or bool(os.environ.get("LUCKY_SAHURS_PERF"))
from i18n import set_language, tr
from online.cloud import CloudMixin
from storage import load_settings, migrate_legacy_save, save_settings
from ui.account_panel import AccountPanelMixin
from ui.audio import AudioMixin
from ui.base import UIBaseMixin
from ui.cards import CardsMixin
from ui.credits_panel import CreditsPanelMixin
from ui.daily_panel import DailyPanelMixin
from ui.drawing import clear_drawing_caches, make_game_background
from ui.fonts import clear_font_caches, make_font
from ui.icons import set_window_icon
from ui.game_screen import GameScreenMixin
from ui.gameplay import GameplayMixin
from ui.leaderboard_panel import LeaderboardPanelMixin
from ui.milestones_panel import MilestonesPanelMixin
from ui.options_panel import OptionsPanelMixin
from ui.pets_panel import PetsPanelMixin
from ui.rebirth_panel import RebirthPanelMixin
from ui.title_saves import TitleSavesMixin
from ui.traits_panel import TraitsPanelMixin
from ui.update_panel import UpdatePanelMixin
from ui.updatelog_panel import UpdateLogPanelMixin
from ui.upgrades_panel import UpgradesPanelMixin
from ui.widgets import SlidePanel


class Game(
    AudioMixin,             # som
    UIBaseMixin,            # botões, scrollbar, ícones, toasts
    CardsMixin,             # cartões dos pets e das traits
    TitleSavesMixin,        # ecrãs do título e dos saves
    GameScreenMixin,        # ecrã principal (topo, botões laterais, carta, stats)
    GameplayMixin,          # roll manual / automático, animações, partículas
    OptionsPanelMixin,      # Options
    UpgradesPanelMixin,     # Upgrade Tree
    MilestonesPanelMixin,   # Milestones
    PetsPanelMixin,         # Index e Bag (Inventory)
    TraitsPanelMixin,       # página de Traits
    RebirthPanelMixin,      # página de Rebirths
    DailyPanelMixin,        # painel das Missões Diárias
    CloudMixin,             # conta na cloud, sincronização, leaderboard (lógica)
    AccountPanelMixin,      # ecrã de conta
    LeaderboardPanelMixin,  # ecrã da leaderboard
    CreditsPanelMixin,      # página de Credits (sons e música)
    UpdateLogPanelMixin,    # página de Update Log (novidades do jogo)
    UpdatePanelMixin,       # "Nova versão disponível": atualização automática pelo GitHub (obrigatória)
):
    """O jogo: junta todas as peças (som, ecrãs, painéis, online) e tem o ciclo principal."""

    def __init__(self):
        self.settings = load_settings()
        set_language(self.settings.get("language"))       # idioma do jogo (English / Português): ver i18n.py
        # tema: "system" (segue o Windows), "dark" ou "light" (ver theme.py); antes de desenhar seja o que for
        theme.set_dark(theme.resolve_dark(self.settings.get("theme_mode", theme.DEFAULT_THEME_MODE)))
        self.theme_check_timer = 0.0

        try:
            pygame.mixer.pre_init(22050, -16, 2, 512)
        except pygame.error:
            pass
        pygame.init()
        pygame.display.set_caption(GAME_TITLE)
        set_window_icon()               # o Tung (icons/tung.png) no lugar do ícone por defeito do pygame
        self.mixer_ok = self.init_mixer()

        self.font_tiny = make_font(13, outline=1)
        self.font_tiny_b = make_font(12, outline=1, heavy=True)      # cartões pequenos do Inventory
        self.font_small = make_font(15, outline=1)
        self.font_small_b = make_font(15, outline=1, heavy=True)
        self.font_med = make_font(19, outline=2, heavy=True)
        self.font_big = make_font(27, outline=2, heavy=True)
        self.font_huge = make_font(40, outline=3, heavy=True)
        self.font_title = make_font(96, outline=5, heavy=True)

        migrate_legacy_save()
        self.state = GameState()          # estado "vazio" enquanto estás no menu
        self.screen_mode = "title"        # "title" | "saves" | "game"
        self.slot_info = {}
        self.delete_confirm_slot = None
        self.delete_confirm_timer = 0.0

        self.fullscreen = bool(self.settings.get("fullscreen", True))
        self.screen = None
        self.canvas = None
        self.gpu_scaled = False
        self.bg_surface = None
        self.vw = 1422
        self.set_fullscreen(self.fullscreen, announce=False)

        self.options_open = False
        self.stats_open = False
        self.credits_open = False
        self.update_log_open = False
        self.reset_options_dropdown()       # sliders / menus Volume e SFX das Options (ui/options_panel.py)

        self.reset_ui()

        self.toast_text = None
        self.toast_timer = 0.0
        self.toast_kind = None          # None = aviso normal; "trait" = cargas de trait (acumulam)
        self.trait_toast_count = 0

        self.frame_dt = 1.0 / FPS
        self.roll_rate = 0.0            # rolls por segundo (medidos), suavizados
        self._rate_prev = None
        self.bar_display = {}           # barras Golden/Diamond/Rainbow: enchimento mostrado (0..1)
        self.bar_continuous = {}        # True = roll tão rápido que a barra fica sempre cheia

        self.autosave_timer = 0.0
        self.buttons = []
        self.nav_mode = False           # True enquanto se registam botões de navegação (topo / laterais)
        self.clip_stack = []
        self.clock = pygame.time.Clock()
        # Medicao de desempenho: LUCKY_SAHURS_PERF=1 faz o jogo escrever os FPS e o tempo de desenho.
        # No Android isso sai no logcat ("adb logcat -s python"), que e a unica forma de medir no telemovel.
        self._perf_draw = 0.0
        self._perf_elapsed = 0.0
        self._perf_frames = 0
        self._perf_window = 0
        self._perf_prof = None
        self._perf_said_format = False
        self._perf_flip = 0.0
        self._perf_bg = 0.0

        self.dragging_scrollbar = None  # key da scrollbar a ser arrastada (ver ui/base.py draw_scrollbar)
        self.scrollbar_hits = {}        # registadas de novo a cada frame, só as que estão visíveis

        # Toque (Android): o botão só dispara quando o dedo levanta sem ter arrastado (ver handle_events).
        self.touch_down = False
        self.touch_pending = None       # onde o dedo tocou (pixéis do ecrã)
        self.touch_last = (0, 0)
        self.touch_dragged = False

        self.sounds = {}
        self.last_sfx = {}
        self.build_sounds()
        self.start_music()
        self.apply_volume()

        self.init_online()
        self.init_updater()             # verificação de versões novas (ver ui/update_panel.py)

    def reset_ui(self):
        """Volta a pôr painéis, scrolls e animações no estado inicial (novo save / voltar ao menu)."""
        self.right_panel = SlidePanel("right")
        self.left_panel = SlidePanel("left")
        self.index_tab = "normal"
        self.bag_view = "equipped"       # Bag: "equipped" (pets equipados) ou "inventory" (todos os pets)
        self.inv_sort = "money"          # Inventory: ordenar por money / rarity / mutation / quantity
        self.inv_high_first = True       # Inventory: do maior para o menor (False = do menor para o maior)
        self.inv_mut = "all"             # Inventory: filtro de mutação ("all", "normal", "golden", "diamond")
        self.inv_tier = None             # Inventory: filtro de raridade (índice do tier; None = todas)
        self.traits_open = False
        self.traits_scroll = 0.0
        self.traits_max_scroll = 0.0
        self.traits_list_rect = pygame.Rect(0, 0, 0, 0)
        self.rebirth_open = False
        self.rebirth_confirm = False
        self.rebirth_confirm_timer = 0.0
        self.rebirth_scroll = 0.0                     # lista de recompensas da página de Rebirth
        self.rebirth_max_scroll = 0.0
        self.rebirth_list_rect = pygame.Rect(0, 0, 0, 0)
        self.milestones_selected_category = None
        self.tree_selected_category = None
        self.roll_anim_start = 0.0
        self.particles = []
        self.auto_accum = 0.0
        self.right_rect = pygame.Rect(0, 0, 0, 0)
        self.left_rect = pygame.Rect(0, 0, 0, 0)

    @property
    def animations(self):
        return self.settings.get("animations", True)

    # ---------------------------------------------------------------- ecrã
    def set_fullscreen(self, fs, announce=True):
        if IS_ANDROID:
            # O Android só tem ecrã inteiro (a orientação e a barra de estado ficam no buildozer.spec).
            # A escala pela GPU (SCALED) pode nao compensar em todos os telemoveis. Para comparar as
            # duas sem refazer o APK, basta pôr "android_gpu_scale": false no ficheiro das definicoes.
            want_gpu = bool(self.settings.get("android_gpu_scale", True))
            if not want_gpu:
                self.screen = pygame.display.set_mode((0, 0))
                self.gpu_scaled = False
                self.fullscreen = True
                self.recompute_layout()
                return
            if not self.gpu_scaled:
                # Primeiro abre-se o ecrã como ele é, só para saber o tamanho real e o formato.
                real = pygame.display.set_mode((0, 0))
                print("PERF real_screen=%dx%d" % (real.get_width(), real.get_height()))
                rw, rh = max(320, real.get_width()), max(240, real.get_height())
                vw = int(round(VIRTUAL_H * rw / float(rh)))
                vw = max(VW_MIN, min(VW_MAX, vw))
                # Depois pede-se esse tamanho com SCALED: o SDL passa a desenhar numa imagem pequena e
                # é a GPU que a estica até ao ecrã. Sem isto, era o CPU a esticar ~2.6 milhões de píxeis
                # a cada frame (com um transform.scale), que é o que punha o jogo a 15 FPS no telemóvel.
                # O SDL também já converte sozinho a posição dos toques para estas coordenadas.
                try:
                    self.screen = pygame.display.set_mode((vw, VIRTUAL_H), pygame.SCALED | pygame.FULLSCREEN)
                    self.gpu_scaled = True
                except pygame.error:
                    self.screen = real          # sem SCALED: fica o caminho antigo, mais lento
            self.fullscreen = True
            self.recompute_layout()
            return
        try:
            if fs:
                self.screen = pygame.display.set_mode((0, 0), pygame.FULLSCREEN)
            else:
                info = pygame.display.Info()
                w = int(min(1500, max(1000, info.current_w * 0.8)))
                h = int(w * 9 / 16)
                self.screen = pygame.display.set_mode((w, h), pygame.RESIZABLE)
        except pygame.error:
            self.screen = pygame.display.set_mode((1280, 720), pygame.RESIZABLE)
            fs = False
        self.fullscreen = fs
        self.settings["fullscreen"] = fs
        self.recompute_layout()

    def toggle_fullscreen(self):
        self.set_fullscreen(not self.fullscreen)
        save_settings(self.settings)
        self.show_toast(tr("Fullscreen on") if self.fullscreen else tr("Windowed mode"))

    def recompute_layout(self):
        real_w, real_h = self.screen.get_size()
        real_w = max(320, real_w)
        real_h = max(240, real_h)
        # a largura virtual segue o formato do ecrã -> a imagem preenche tudo, sem barras pretas
        vw = int(round(VIRTUAL_H * real_w / float(real_h)))
        self.vw = max(VW_MIN, min(VW_MAX, vw))
        if self.gpu_scaled and (real_w, real_h) == (self.vw, VIRTUAL_H):
            # Com SCALED o "ecrã" já tem o tamanho de desenho: pinta-se lá diretamente, sem cópia nenhuma.
            self.canvas = self.screen
        else:
            self.canvas = pygame.Surface((self.vw, VIRTUAL_H))
        self.bg_surface = make_game_background(self.vw, VIRTUAL_H, theme.BG_TOP, theme.BG_BOTTOM, dark=theme.DARK_MODE)
        self.scale_x = real_w / float(self.vw)
        self.scale_y = real_h / float(VIRTUAL_H)
        self.right_w = int(min(470, self.vw * 0.34))
        self.left_w = int(min(370, self.vw * 0.28))

    # ---------------------------------------------------------------- tema (claro / escuro)
    def theme_mode_label(self):
        """Nome do tema escolhido, para o botão das Options."""
        mode = self.settings.get("theme_mode", theme.DEFAULT_THEME_MODE)
        return {"dark": tr("Dark"), "light": tr("Light")}.get(mode, tr(theme.SYSTEM_LABEL))

    def apply_dark(self, dark):
        """Passa para o tema escuro / claro e refaz tudo o que guarda cores do tema antigo (fundo, fontes com
        contorno, cartões). As cores dos ficheiros já importados são trocadas por theme.set_dark()."""
        dark = bool(dark)
        if dark == theme.DARK_MODE:
            return
        theme.set_dark(dark)
        clear_font_caches()             # o contorno do texto muda de cor
        clear_drawing_caches()
        self.clear_card_cache()         # cartões dos pets / traits (contorno)
        self.recompute_layout()         # refaz o fundo (em modo escuro tem estrelinhas)

    def set_theme_mode(self, mode, announce=True):
        """mode = "system" (segue o Windows / sistema), "dark" ou "light"."""
        if mode not in theme.THEME_MODES:
            mode = theme.DEFAULT_THEME_MODE
        self.settings["theme_mode"] = mode
        self.theme_check_timer = 0.0
        self.apply_dark(theme.resolve_dark(mode))
        save_settings(self.settings)
        if announce:
            self.show_toast(tr("Theme: %s", self.theme_mode_label()))

    def cycle_theme_mode(self):
        modes = theme.THEME_MODES
        cur = self.settings.get("theme_mode", theme.DEFAULT_THEME_MODE)
        self.set_theme_mode(modes[(modes.index(cur) + 1) % len(modes)] if cur in modes else modes[0])

    def tick_theme(self, dt):
        """No modo 'system' acompanha o Windows: se mudares o tema do Windows com o jogo aberto, o jogo muda também.
        (Só no Windows, onde ler o tema é instantâneo; no macOS / Linux lê-se ao abrir o jogo e ao escolher a opção.)"""
        if sys.platform != "win32" or self.settings.get("theme_mode", theme.DEFAULT_THEME_MODE) != "system":
            return
        self.theme_check_timer -= dt
        if self.theme_check_timer <= 0:
            self.theme_check_timer = 2.0
            self.apply_dark(theme.system_prefers_dark(theme.DARK_MODE))

    def screen_to_canvas(self, pos):
        return (pos[0] / self.scale_x, pos[1] / self.scale_y)

    def mouse_canvas(self):
        if IS_ANDROID and not self.touch_down:
            return (-10000.0, -10000.0)     # sem dedo no ecrã não há nada "sob o rato" (nenhum botão aceso)
        return self.screen_to_canvas(pygame.mouse.get_pos())

    # ---------------------------------------------------------------- scrollbars arrastáveis
    def scroll_from_track(self, key, canvas_y):
        """Converte uma posição do rato (eixo Y, coordenadas do canvas) num valor de scroll,
        para a scrollbar identificada por 'key' (ver ui/base.py draw_scrollbar)."""
        hit = self.scrollbar_hits.get(key)
        if hit is None:
            return None
        track, max_scroll, bar_h = hit
        span = track.height - bar_h
        if span <= 0 or max_scroll <= 0:
            return 0.0
        frac = (canvas_y - track.top - bar_h / 2.0) / float(span)
        return max(0.0, min(max_scroll, frac * max_scroll))

    def apply_scroll(self, key, value):
        """Aplica um valor de scroll absoluto ao painel/lista certo, conforme a 'key' da scrollbar."""
        if key == "right":
            self.right_panel.set_scroll_abs(value)
        elif key == "left":
            self.left_panel.set_scroll_abs(value)
        elif key == "traits":
            self.traits_scroll = max(0.0, min(self.traits_max_scroll, value))
        elif key == "rebirth":
            self.rebirth_scroll = max(0.0, min(self.rebirth_max_scroll, value))
        elif key == "leaderboard":
            self.lb_scroll = max(0.0, min(self.lb_max_scroll, value))

    def start_scrollbar_drag(self, canvas_pos):
        """Clique dentro da barra de uma scrollbar visível -> começa a arrastar. Devolve True se apanhou alguma."""
        for key, (track, _max_scroll, _bar_h) in self.scrollbar_hits.items():
            if track.collidepoint(canvas_pos):
                self.dragging_scrollbar = key
                value = self.scroll_from_track(key, canvas_pos[1])
                if value is not None:
                    self.apply_scroll(key, value)
                return True
        return False

    def close_overlays(self):
        """Fecha Options / Stats / Traits / Rebirth / Leaderboard (o que estiver aberto)."""
        if self.options_open:
            self.close_options()
        self.stats_open = False
        self.credits_open = False
        self.update_log_open = False
        self.traits_open = False
        self.rebirth_open = False
        self.rebirth_confirm = False
        self.leaderboard_open = False

    def close_overlay_on_outside_click(self):
        """Clique fora da página de Leaderboard / Options / Stats / Traits / Rebirth -> fecha-a."""
        if self.leaderboard_open:
            self.close_leaderboard()
        elif self.options_open:
            self.close_options()
        elif self.stats_open:
            self.close_stats()
        elif self.credits_open:
            self.close_credits()
        elif self.update_log_open:
            self.close_update_log()
        elif self.traits_open and self.screen_mode == "game":
            self.close_traits()
        elif self.rebirth_open and self.screen_mode == "game":
            self.close_rebirth()
        else:
            return
        self.play("click")

    def _perf_profile_draw(self):
        """Desenha e, entre o frame 150 e o 450, mede com o cProfile onde e que o tempo se vai.
        O resultado sai uma unica vez para o logcat, com as 25 funcoes mais caras."""
        n = self._perf_frames
        if n < 150 or n >= 450:
            self.draw()
            return
        if self._perf_prof is None:
            import cProfile
            self._perf_prof = cProfile.Profile()
        self._perf_prof.enable()
        self.draw()
        self._perf_prof.disable()
        if n == 449:
            # Le-se o cProfile a mao: o modulo pstats nao vem no Python do Android, e importa-lo
            # aqui rebentava com a app exatamente neste frame (fechava sem erro nenhum).
            try:
                entries = self._perf_prof.getstats()
            except Exception as err:          # noqa: BLE001 - so medicao, nunca pode matar o jogo
                print("PROF indisponivel: %r" % (err,))
                return
            rows = []
            for e in entries:
                code = e.code
                if isinstance(code, str):
                    name = code                  # funcao em C (blit, draw.rect...)
                else:
                    name = "%s:%d(%s)" % (os.path.basename(code.co_filename),
                                          code.co_firstlineno, code.co_name)
                rows.append((e.inlinetime, e.totaltime, e.callcount, name))
            rows.sort(reverse=True)
            print("PROF 300 frames: tottime cumtime calls funcao")
            for inline, total, calls, name in rows[:25]:
                print("PROF %8.3f %8.3f %7d %s" % (inline, total, calls, name))

    # ---------------------------------------------------------------- loop
    def run(self):
        running = True
        while running:
            dt = min(0.1, self.clock.tick(FPS) / 1000.0)
            self.worker.poll()          # entrega os resultados de rede (login, saves, leaderboard...) pendentes
            self.tick_updater()         # de tempos a tempos vê se há versão nova no GitHub
            self.tick_theme(dt)         # tema "system": acompanha o tema do Windows
            running = self.handle_events()

            if self.screen_mode == "game":
                gain = self.state.income_per_second() * dt
                self.state.coins += gain
                self.state.total_coins_earned += gain
                self.state.playtime += dt
                self.state.last_seen = time.time()      # mantido fresco para os ganhos offline da próxima vez
                self.update_auto(dt)
                self.update_roll_rate(dt)
                self.check_milestones()
                self.state.ensure_daily_missions()      # troca as missões sozinho se o dia mudou
                self.right_panel.update(dt)
                self.left_panel.update(dt)

                self.autosave_timer += dt
                if self.autosave_timer >= AUTOSAVE_INTERVAL:
                    self.autosave_timer = 0.0
                    self.state.save()

                self.tick_online(dt)

            else:
                self._rate_prev = None
                self.roll_rate = 0.0

            if self.delete_confirm_slot is not None:
                self.delete_confirm_timer -= dt
                if self.delete_confirm_timer <= 0:
                    self.delete_confirm_slot = None

            if self.rebirth_confirm:
                self.rebirth_confirm_timer -= dt
                if self.rebirth_confirm_timer <= 0:
                    self.rebirth_confirm = False

            self.update_animations(dt)
            self.frame_dt = dt
            if PERF_LOG:
                t0 = time.perf_counter()
                self._perf_profile_draw()
                self._perf_draw += time.perf_counter() - t0
                self._perf_frames += 1
                self._perf_window += 1
                self._perf_elapsed += dt
                if self._perf_elapsed >= 2.0:
                    n = max(1, self._perf_window)
                    if not self._perf_said_format:
                        self._perf_said_format = True
                        # Se o ecra nao for 32 bits, cada blit de uma imagem com transparencia tem de
                        # converter pixeis; e a primeira coisa a confirmar quando o desenho esta lento.
                        print("PERF screen=%dx%d bits=%d masks=%s canvas_bits=%d"
                              % (self.screen.get_width(), self.screen.get_height(),
                                 self.screen.get_bitsize(), self.screen.get_masks(),
                                 self.canvas.get_bitsize()))
                    print("PERF fps=%.1f draw=%.1fms flip=%.1fms bg=%.1fms"
                          % (n / self._perf_elapsed, 1000.0 * self._perf_draw / n,
                             1000.0 * self._perf_flip / n, 1000.0 * self._perf_bg / n))
                    self._perf_flip = self._perf_bg = 0.0
                    self._perf_draw = self._perf_elapsed = 0.0
                    self._perf_window = 0
            else:
                self.draw()

        self.save_everything()
        pygame.quit()
        sys.exit()

    # ---------------------------------------------------------------- eventos
    def handle_events(self):
        for event in pygame.event.get():
            if event.type == pygame.QUIT:
                return False

            elif event.type == pygame.VIDEORESIZE and not self.fullscreen:
                try:
                    self.screen = pygame.display.set_mode((event.w, event.h), pygame.RESIZABLE)
                except pygame.error:
                    pass
                self.recompute_layout()

            elif event.type == pygame.KEYDOWN:
                if self.update_modal_active() and event.key != pygame.K_F11:
                    pass                # ecrã "Nova versão": ESC e atalhos não fazem nada (só Atualizar ou Não)
                elif self.screen_mode == "account" and self.handle_account_key(event):
                    pass
                elif event.key == pygame.K_F11 or (
                        event.key == pygame.K_f and (event.mod & (pygame.KMOD_META | pygame.KMOD_CTRL))):
                    self.toggle_fullscreen()
                elif event.key in (pygame.K_ESCAPE, pygame.K_AC_BACK):
                    if self.screen_mode == "account":
                        self.close_account()
                    elif self.leaderboard_open:
                        self.close_leaderboard()
                    elif self.options_open:
                        self.close_options()
                    elif self.stats_open:
                        self.close_stats()
                    elif self.credits_open:
                        self.close_credits()
                    elif self.update_log_open:
                        self.close_update_log()
                    elif self.screen_mode == "saves":
                        self.back_to_title()
                    elif self.screen_mode == "title":
                        pass
                    elif self.traits_open:
                        self.traits_open = False
                    elif self.rebirth_open:
                        self.close_rebirth()
                    elif self.right_panel.is_open:
                        self.right_panel.close()
                    elif self.left_panel.is_open:
                        self.left_panel.close()
                elif (self.options_open or self.stats_open or self.traits_open or self.rebirth_open
                      or self.leaderboard_open or self.credits_open or self.update_log_open
                      or self.screen_mode != "game"):
                    pass
                elif event.key in (pygame.K_u, pygame.K_t):
                    self.open_right_panel("tree")
                elif event.key == pygame.K_i:
                    self.open_right_panel("index")
                elif event.key == pygame.K_b:
                    self.open_left_panel("bag")
                elif event.key == pygame.K_m:
                    self.open_right_panel("milestones")

            elif event.type == pygame.TEXTINPUT:
                if self.screen_mode == "account" and self.acc_focus and not self.update_modal_active():
                    f = self.acc_fields.get(self.acc_focus)
                    if f:
                        f.add(event.text)

            elif event.type == pygame.MOUSEWHEEL:
                self.scroll_at(self.mouse_canvas(), -event.y * 60)

            elif event.type == pygame.MOUSEMOTION:
                if IS_ANDROID and self.touch_down and not self.dragging_slider and not self.dragging_scrollbar:
                    start = self.touch_pending or event.pos
                    if (abs(event.pos[0] - start[0]) > TOUCH_DRAG_SLOP
                            or abs(event.pos[1] - start[1]) > TOUCH_DRAG_SLOP):
                        self.touch_dragged = True
                    if self.touch_dragged:
                        # arrastar para cima empurra a lista para baixo, como em qualquer app
                        dy = (event.pos[1] - self.touch_last[1]) / self.scale_y
                        self.scroll_at(self.screen_to_canvas(event.pos), -dy)
                    self.touch_last = event.pos
                    continue
                if self.dragging_slider:
                    self.set_slider_from_pos(self.dragging_slider, self.screen_to_canvas(event.pos))
                elif self.dragging_scrollbar:
                    canvas_pos = self.screen_to_canvas(event.pos)
                    value = self.scroll_from_track(self.dragging_scrollbar, canvas_pos[1])
                    if value is None:
                        self.dragging_scrollbar = None    # o painel fechou-se a meio do arrasto
                    else:
                        self.apply_scroll(self.dragging_scrollbar, value)

            elif event.type == pygame.MOUSEBUTTONUP and event.button == 1:
                if self.dragging_slider:
                    self.dragging_slider = None
                    save_settings(self.settings)
                    self.play("click")          # serve de "amostra" do novo volume
                self.dragging_scrollbar = None
                if IS_ANDROID:
                    # No telemóvel o botão só dispara quando o dedo levanta: assim um arrasto faz scroll
                    # da lista em vez de carregar no que estava por baixo.
                    pending, dragged = self.touch_pending, self.touch_dragged
                    self.touch_pending = None
                    self.touch_dragged = False
                    self.touch_down = False
                    if pending is not None and not dragged:
                        self.dispatch_click(self.screen_to_canvas(pending))

            elif event.type == pygame.MOUSEBUTTONDOWN and event.button == 1:
                canvas_pos = self.screen_to_canvas(event.pos)
                if self.options_open and not self.update_modal_active():
                    hit = next((name for name, r in self.slider_hits.items() if r.collidepoint(canvas_pos)), None)
                    if hit:
                        self.dragging_slider = hit
                        self.set_slider_from_pos(hit, canvas_pos)
                        continue
                if self.start_scrollbar_drag(canvas_pos):
                    continue
                if IS_ANDROID:
                    self.touch_down = True
                    self.touch_pending = event.pos
                    self.touch_last = event.pos
                    self.touch_dragged = False
                    continue
                self.dispatch_click(canvas_pos)

            elif event.type in (pygame.APP_WILLENTERBACKGROUND, pygame.APP_DIDENTERBACKGROUND):
                # O Android pode matar a app enquanto está em segundo plano: grava já.
                self.save_everything()
        return True

    def dispatch_click(self, canvas_pos):
        """Carrega no botão que estiver debaixo de 'canvas_pos' (ou fecha o painel aberto, se não houver nenhum)."""
        for rect, callback, sfx, _nav in reversed(self.buttons):
            if rect.collidepoint(canvas_pos):
                if sfx:
                    self.play(sfx)
                callback()
                return True
        if not self.update_modal_active():
            self.close_overlay_on_outside_click()
        return False

    def scroll_at(self, pos, step):
        """Faz scroll da lista que estiver debaixo de 'pos' (coordenadas do canvas). 'step' em pixéis do canvas."""
        if (self.screen_mode != "game" or self.options_open or self.stats_open
                or self.leaderboard_open or self.credits_open or self.update_log_open
                or self.update_modal_active()):
            return False
        if self.traits_open and self.traits_list_rect.collidepoint(pos):
            self.traits_scroll = max(0.0, min(self.traits_max_scroll, self.traits_scroll + step))
        elif self.rebirth_open and self.rebirth_list_rect.collidepoint(pos):
            self.rebirth_scroll = max(0.0, min(self.rebirth_max_scroll, self.rebirth_scroll + step))
        elif self.right_panel.visible and self.right_rect.collidepoint(pos):
            self.right_panel.add_scroll(step)
        elif self.left_panel.visible and self.left_rect.collidepoint(pos):
            self.left_panel.add_scroll(step)
        else:
            return False
        return True

    def draw(self):
        t_bg = time.perf_counter()
        self.canvas.blit(self.bg_surface, (0, 0))
        self._perf_bg += time.perf_counter() - t_bg
        self.buttons = []
        self.nav_mode = False
        self.clip_stack = []
        self.scrollbar_hits = {}
        self.canvas.set_clip(None)
        mouse_pos = self.mouse_canvas()

        if self.screen_mode == "title":
            self.draw_title(mouse_pos)
            self.draw_toast()
        elif self.screen_mode == "saves":
            self.draw_saves(mouse_pos)
            self.draw_toast()
        elif self.screen_mode == "account":
            self.draw_account(mouse_pos)
            self.draw_toast()
        else:
            self.draw_game_screen(mouse_pos)

        if self.stats_open:
            self.begin_modal()
            self.draw_stats(mouse_pos)

        if self.options_open:
            self.begin_modal()
            self.draw_options(mouse_pos)

        if self.leaderboard_open:
            self.begin_modal()
            self.draw_leaderboard(mouse_pos)

        if self.credits_open:
            self.begin_modal()
            self.draw_credits(mouse_pos)

        if self.update_log_open:
            self.begin_modal()
            self.draw_update_log(mouse_pos)

        if self.update_modal_active():          # "Nova versão disponível": por cima de tudo, bloqueia o resto
            self.draw_update_modal(mouse_pos)

        t_flip = time.perf_counter()
        if self.canvas is not self.screen:
            real_size = self.screen.get_size()
            # O smoothscale de um ecrã inteiro custa caro no telemóvel (e a diferença nem se vê num ecrã
            # tão pequeno), por isso no Android é sempre o scale simples.
            if self.animations and not IS_ANDROID:
                scaled = pygame.transform.smoothscale(self.canvas, real_size)
            else:
                scaled = pygame.transform.scale(self.canvas, real_size)
            self.screen.blit(scaled, (0, 0))
        pygame.display.flip()
        # O flip (e o scale, quando existe) medido a parte: assim sabe-se se o tempo se vai no
        # desenho ou na entrega do frame ao ecra, que sao problemas diferentes.
        self._perf_flip += time.perf_counter() - t_flip

    def save_everything(self):
        """Grava tudo (nuvem, save, definições) sem fechar o jogo. Usado ao sair e ao ir para segundo plano."""
        self.flush_cloud_blocking()
        self.state.save()
        save_settings(self.settings)

    def quit_game(self):
        self.save_everything()
        pygame.quit()
        sys.exit()


if __name__ == "__main__":
    try:
        Game().run()
    except SystemExit:
        raise
    except Exception:
        # com o .exe (sem consola) os erros não se veem: ficam escritos em crash_log.txt ao lado dos saves
        try:
            with open(os.path.join(SAVE_DIR, "crash_log.txt"), "w", encoding="utf-8") as f:
                f.write(traceback.format_exc())
        except OSError:
            pass
        raise
