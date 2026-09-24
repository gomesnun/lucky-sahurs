"""Ecrã "Nova versão disponível": aparece POR CIMA DE TUDO quando o GitHub tem uma versão mais recente
(a verificação está em online/updater.py). Só tem duas saídas: Atualizar agora, ou Não - que fecha o jogo,
porque o jogo tem de estar sempre atualizado.

Fases (self.upd_phase):  idle -> checking -> available -> downloading -> installing   (ou "error")
"""

import sys
import time

import pygame

from config import IS_ANDROID, UPDATE_CHECK_INTERVAL, UPDATE_RETRY_AFTER_ERROR, VERSION, VIRTUAL_H
from i18n import tr
from online import updater
from storage import save_settings
from theme import ACCENT, ACCENT_HOVER, BAD, BLACK, GOOD, GREY, GREY_DIM, OUTLINE, PANEL, PANEL_LIGHT, PANEL_LIGHTER, WHITE
from ui.drawing import draw_panel
from ui.fonts import wrap_text

ACTIVE_PHASES = ("available", "downloading", "installing", "error")


class UpdatePanelMixin:
    # ---------------------------------------------------------------- estado / verificação
    def init_updater(self):
        self.upd_phase = "idle"
        self.upd_info = None            # a versão nova (ver updater.fetch_latest)
        self.upd_plan = None            # o que já está descarregado e pronto a instalar
        self.upd_progress = 0.0         # 0..1 enquanto descarrega
        self.upd_error = "other"        # net | corrupt | perm | other
        self.upd_error_detail = ""
        self.upd_next_check = time.time() + 1.5          # a 1.ª verificação é logo a seguir a abrir
        self.upd_enabled = updater.updates_enabled()     # só no programa empacotado, feito a partir de uma tag
        if self.upd_enabled:
            updater.cleanup_leftovers()

    def update_modal_active(self):
        return getattr(self, "upd_phase", "idle") in ACTIVE_PHASES

    def tick_updater(self):
        """Chamado a cada frame: de vez em quando pergunta ao GitHub se há versão nova (numa thread de fundo)."""
        if not self.upd_enabled or self.upd_phase != "idle" or time.time() < self.upd_next_check:
            return
        self.upd_phase = "checking"

        def ok(info):
            if info:
                self.upd_info = info
                self.upd_phase = "available"
            else:
                self.upd_phase = "idle"
                self.upd_next_check = time.time() + UPDATE_CHECK_INTERVAL

        def err(_e):                    # sem net / GitHub em baixo: NÃO bloqueia o jogo, tenta mais tarde
            self.upd_phase = "idle"
            self.upd_next_check = time.time() + UPDATE_RETRY_AFTER_ERROR

        self.worker.run(updater.fetch_latest, ok, err)

    # ---------------------------------------------------------------- ações
    def upd_retry(self):
        """Botão "Tentar outra vez": se o ficheiro já está descarregado, não o volta a descarregar
        (no Android é normal falhar só por faltar a autorização para instalar)."""
        if self.upd_plan is not None:
            self.upd_phase = "installing"
            self.upd_finish(self.upd_plan)
        else:
            self.upd_start_download()

    def upd_start_download(self):
        info = self.upd_info
        self.upd_phase = "downloading"
        self.upd_progress = 0.0

        def progress(p):
            self.upd_progress = p

        def job():
            try:
                return ("ok", updater.prepare(info, progress), "")
            except updater.UpdateError as e:
                return ("err", e.code, e.detail)

        def done(res):
            if res[0] == "ok":
                self.upd_plan = res[1]
                self.upd_phase = "installing"
                self.upd_finish(res[1])
            else:
                self.upd_fail(res[1], res[2])

        self.worker.run(job, done, lambda e: self.upd_fail("other", str(e)))

    def upd_fail(self, code, detail=""):
        self.upd_phase = "error"
        self.upd_error = code
        self.upd_error_detail = detail

    def upd_finish(self, plan):
        """Descarregou e preparou tudo: guarda o jogo, pede ao ajudante que troque o programa e reabra, e fecha."""
        self.flush_cloud_blocking()
        self.state.save()
        save_settings(self.settings)
        try:
            updater.launch(plan)
        except Exception as e:
            code = getattr(e, "code", "other")
            self.upd_fail(code if code in ("permission", "net", "corrupt", "perm") else "other", str(e))
            return
        if plan.get("kind") == "android":
            # Quem instala é o Android, e o jogo tem de continuar aberto por trás: se o utilizador
            # cancelar a instalação, volta aqui e pode tentar outra vez.
            return
        pygame.quit()
        sys.exit()

    def upd_open_page(self):
        page = (self.upd_info or {}).get("page") or "https://github.com/"
        self.open_url(page)

    # ---------------------------------------------------------------- desenho
    def draw_update_modal(self, mouse_pos):
        # nada do que está por baixo fica clicável (nem os botões do topo), e ESC / clicar fora não o fecham
        self.buttons = []
        self.scrollbar_hits = {}
        overlay = pygame.Surface((self.vw, VIRTUAL_H), pygame.SRCALPHA)
        overlay.fill((0, 0, 0, 205))
        self.canvas.blit(overlay, (0, 0))

        phase = self.upd_phase
        panel_w, panel_h = 640, 330
        rect = pygame.Rect(self.vw // 2 - panel_w // 2, VIRTUAL_H // 2 - panel_h // 2, panel_w, panel_h)
        draw_panel(self.canvas, rect, PANEL, radius=16)
        self.register_button(rect, lambda: None, None)       # clicar dentro do painel não faz nada
        inner_x, inner_w = rect.x + 28, panel_w - 56

        if phase == "error":
            title, title_color = tr("The update failed"), BAD
        else:
            title, title_color = tr("New version available"), ACCENT
        self.canvas.blit(self.font_big.render(title, True, title_color), (inner_x, rect.y + 24))

        info = self.upd_info or {}
        y = rect.y + 84
        if phase == "available":
            lines = [tr("Version %s is out (you have %s).", info.get("tag", "?"), VERSION),
                     tr("You need to update to keep playing.")]
        elif phase == "downloading":
            lines = [tr("Downloading the update... %d%%", int(self.upd_progress * 100))]
        elif phase == "installing":
            lines = [tr("Android is asking you to install the new version. Accept it to keep playing.")
                     if IS_ANDROID else tr("Installing... the game restarts in a moment.")]
        else:
            lines = [self.upd_error_text()]
        for i, text in enumerate(lines):
            font = self.font_med if (i == 0 and phase != "error") else self.font_small
            for line in wrap_text(text, font, inner_w):
                surf = font.render(line, True, WHITE if i == 0 else GREY)
                self.canvas.blit(surf, (inner_x, y))
                y += surf.get_height() + 6
            y += 6

        if phase == "downloading":
            bar = pygame.Rect(inner_x, rect.bottom - 90, inner_w, 24)
            pygame.draw.rect(self.canvas, PANEL_LIGHT, bar, border_radius=12)
            fill = bar.copy()
            fill.width = max(0, int(bar.width * self.upd_progress))
            if fill.width >= 6:
                pygame.draw.rect(self.canvas, GOOD, fill, border_radius=12)
            pygame.draw.rect(self.canvas, OUTLINE, bar, width=3, border_radius=12)

        by, bh, gap = rect.bottom - 82, 54, 14
        if phase == "available":
            bw = (inner_w - gap) // 2
            self.button(pygame.Rect(inner_x, by, bw, bh), tr("Update now"), self.font_med, mouse_pos,
                        ACCENT, ACCENT_HOVER, BLACK, callback=self.upd_start_download, radius=12)
            self.button(pygame.Rect(inner_x + bw + gap, by, bw, bh), tr("No (closes the game)"), self.font_med,
                        mouse_pos, PANEL_LIGHT, PANEL_LIGHTER, WHITE, callback=self.quit_game, radius=12)
        elif phase == "installing" and IS_ANDROID:
            bw = (inner_w - gap) // 2
            self.button(pygame.Rect(inner_x, by, bw, bh), tr("Install again"), self.font_med, mouse_pos,
                        ACCENT, ACCENT_HOVER, BLACK, callback=self.upd_retry, radius=12)
            self.button(pygame.Rect(inner_x + bw + gap, by, bw, bh), tr("No (closes the game)"), self.font_med,
                        mouse_pos, PANEL_LIGHT, PANEL_LIGHTER, WHITE, callback=self.quit_game, radius=12)
        elif phase == "error":
            bw = (inner_w - 2 * gap) // 3
            self.button(pygame.Rect(inner_x, by, bw, bh), tr("Try again"), self.font_med, mouse_pos,
                        ACCENT, ACCENT_HOVER, BLACK, callback=self.upd_retry, radius=12,
                        enabled=bool(self.upd_info or self.upd_plan))
            self.button(pygame.Rect(inner_x + bw + gap, by, bw, bh), tr("Open download page"), self.font_med,
                        mouse_pos, PANEL_LIGHT, PANEL_LIGHTER, WHITE, callback=self.upd_open_page, radius=12)
            self.button(pygame.Rect(inner_x + 2 * (bw + gap), by, bw, bh), tr("Close game"), self.font_med,
                        mouse_pos, PANEL_LIGHT, PANEL_LIGHTER, WHITE, callback=self.quit_game, radius=12)

    def upd_error_text(self):
        code = self.upd_error
        if code == "net":
            return tr("Couldn't download the update. Check your internet connection.")
        if code == "corrupt":
            return tr("The downloaded file is damaged. Please try again.")
        if code == "permission":
            return tr("Android needs your permission to install apps from the game. Allow it on the page that "
                      "just opened, then come back and tap Try again.")
        if code == "perm":
            return tr("The game can't replace itself in this folder. Move it to another folder (for example the "
                      "Desktop) or download the new version by hand.")
        return tr("Something went wrong (%s).", self.upd_error_detail or code)
