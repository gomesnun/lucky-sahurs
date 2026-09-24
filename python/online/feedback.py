"""Feedback: cada conta publica um (e só um) recado, que pode reescrever ou apagar.
O desenho está em ui/feedback_panel.py."""

import time
import pygame

from i18n import tr
from online.firebase import online_error_text
from theme import BAD, GOOD
from ui.widgets import TextField


FEEDBACK_REFRESH = 120.0       # segundos entre atualizações da lista (com a página aberta)
FEEDBACK_MAX = 60              # quantos recados se mostram
FEEDBACK_MIN_LEN = 4


class FeedbackMixin:
    """Estado e chamadas à cloud da página de Feedback."""

    # ================================================================ ONLINE: feedback (estado)
    def init_feedback(self):
        self.feedback_open = False
        self.feedback_scroll = 0.0
        self.feedback_max_scroll = 0.0
        self.feedback_list_rect = pygame.Rect(0, 0, 0, 0)

        self.feedback_list = []                 # [{"uid", "username", "text", "updated_at"}]
        self.feedback_mine = None               # o recado desta conta, ou None se não tiver nenhum
        self.feedback_loaded = False
        self.feedback_loading = False
        self.feedback_error = None
        self.feedback_next_refresh = 0.0

        self.feedback_field = TextField("text")
        self.feedback_editing = False           # True = está a escrever (campo visível)
        self.feedback_focus = False
        self.feedback_busy = False
        self.feedback_msg = None                # (texto, cor)
        self.feedback_confirm_delete = False

    # ---------------------------------------------------------------- abrir / fechar
    def feedback_ready(self):
        return bool(self.client and self.account)

    def open_feedback(self):
        self.close_overlays()
        self.feedback_open = True
        self.feedback_scroll = 0.0
        self.feedback_msg = None
        self.feedback_confirm_delete = False
        self.refresh_feedback()

    def close_feedback(self):
        self.feedback_open = False
        self.feedback_confirm_delete = False
        self.set_feedback_focus(False)
        self.feedback_editing = False

    def toggle_feedback(self):
        if self.feedback_open:
            self.close_feedback()
        else:
            self.open_feedback()

    def set_feedback_focus(self, focused):
        if focused == self.feedback_focus:
            return
        self.feedback_focus = bool(focused)
        if self.feedback_focus:
            self.start_text_input()             # no telemóvel é isto que faz aparecer o teclado
        else:
            self.stop_text_input()

    # ---------------------------------------------------------------- teclado
    def handle_feedback_key(self, event):
        """Teclas com o campo de escrita aberto. Devolve True se a tecla foi usada."""
        if not (self.feedback_open and self.feedback_focus):
            return False
        f = self.feedback_field
        k = event.key
        if k in (pygame.K_RETURN, pygame.K_KP_ENTER):
            self.submit_feedback()
            return True
        if k == pygame.K_BACKSPACE:
            f.backspace()
            return True
        if k == pygame.K_DELETE:
            f.delete()
            return True
        if k == pygame.K_LEFT:
            f.move(-1)
            return True
        if k == pygame.K_RIGHT:
            f.move(1)
            return True
        if k == pygame.K_HOME:
            f.home()
            return True
        if k == pygame.K_END:
            f.end()
            return True
        if k == pygame.K_v and (event.mod & (pygame.KMOD_CTRL | pygame.KMOD_META)):
            from ui.widgets import clipboard_text
            f.add(clipboard_text())
            return True
        return False

    # ---------------------------------------------------------------- ler
    def refresh_feedback(self, force=False):
        if not self.feedback_ready() or self.feedback_loading:
            return
        now = time.time()
        if not force and now < self.feedback_next_refresh:
            return
        self.feedback_loading = True
        self.feedback_next_refresh = now + FEEDBACK_REFRESH
        client = self.client
        uid = self.account.get("uid")

        def job():
            entries = client.list_feedback(FEEDBACK_MAX)
            mine = next((e for e in entries if e["uid"] == uid), None)
            if mine is None:
                # ainda pode existir sem aparecer na lista (limite de FEEDBACK_MAX)
                mine = client.get_my_feedback()
            return entries, mine

        def ok(res):
            entries, mine = res
            self.feedback_loading = False
            self.feedback_loaded = True
            self.feedback_error = None
            self.feedback_list = entries
            self.feedback_mine = mine
            if not self.feedback_editing:
                self.feedback_field.set_text(mine["text"] if mine else "")

        def err(e):
            self.feedback_loading = False
            self.feedback_error = online_error_text(e)

        self.worker.run(job, ok, err)

    # ---------------------------------------------------------------- escrever / apagar
    def start_feedback_edit(self):
        """Abre o campo de escrita (com o texto que já lá estava, se houver)."""
        if not self.feedback_ready() or self.feedback_busy:
            return
        self.feedback_editing = True
        self.feedback_confirm_delete = False
        self.feedback_msg = None
        self.feedback_field.set_text(self.feedback_mine["text"] if self.feedback_mine else "")
        self.set_feedback_focus(True)

    def cancel_feedback_edit(self):
        self.feedback_editing = False
        self.set_feedback_focus(False)
        self.feedback_field.set_text(self.feedback_mine["text"] if self.feedback_mine else "")
        self.feedback_msg = None

    def submit_feedback(self):
        if not self.feedback_ready() or self.feedback_busy:
            return
        text = " ".join(self.feedback_field.text.split())
        if len(text) < FEEDBACK_MIN_LEN:
            self.feedback_msg = (tr("Write a bit more before sending."), BAD)
            return
        self.feedback_busy = True
        self.feedback_msg = None
        client = self.client
        username = self.account.get("username", "")

        def ok(_res):
            self.feedback_busy = False
            self.feedback_editing = False
            self.set_feedback_focus(False)
            entry = {"uid": self.account.get("uid"), "username": username,
                     "text": text, "updated_at": time.time()}
            self.feedback_mine = entry
            self.feedback_list = [entry] + [e for e in self.feedback_list if e["uid"] != entry["uid"]]
            self.feedback_field.set_text(text)
            self.feedback_msg = (tr("Thanks! Your feedback is published."), GOOD)
            self.feedback_next_refresh = 0.0

        def err(e):
            self.feedback_busy = False
            self.feedback_msg = (online_error_text(e), BAD)

        self.worker.run(lambda: client.publish_feedback(text), ok, err)

    def ask_delete_feedback(self):
        self.feedback_confirm_delete = True
        self.feedback_msg = None

    def delete_feedback(self):
        if not self.feedback_ready() or self.feedback_busy or not self.feedback_mine:
            return
        self.feedback_busy = True
        self.feedback_confirm_delete = False
        self.feedback_msg = None
        client = self.client
        uid = self.account.get("uid")

        def ok(_res):
            self.feedback_busy = False
            self.feedback_mine = None
            self.feedback_editing = False
            self.set_feedback_focus(False)
            self.feedback_list = [e for e in self.feedback_list if e["uid"] != uid]
            self.feedback_field.set_text("")
            # apagar liberta a vaga: pode voltar a escrever outro
            self.feedback_msg = (tr("Feedback deleted. You can write a new one."), GOOD)
            self.feedback_next_refresh = 0.0

        def err(e):
            self.feedback_busy = False
            self.feedback_msg = (online_error_text(e), BAD)

        self.worker.run(client.delete_feedback, ok, err)
