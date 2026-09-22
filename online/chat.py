"""Chat entre amigos (lógica; o ecrã está em ui/chat_panel.py).

Uma conversa por par de amigos: /chats/{uidA__uidB}/messages. Só se lê enquanto a conversa está
aberta (de POLL em POLL segundos) - com a página fechada não se gasta uma única chamada."""

import time
import pygame

from i18n import tr
from online.firebase import online_error_text
from theme import BAD
from ui.widgets import TextField


CHAT_POLL = 5.0             # segundos entre leituras, com a conversa aberta
CHAT_LIMIT = 50             # quantas mensagens se guardam/mostram
CHAT_MIN_LEN = 1


class ChatMixin:
    """Estado e chamadas à cloud do chat. O desenho está em ui/chat_panel.py."""

    # ================================================================ ONLINE: chat (estado)
    def init_chat(self):
        self.chat_uid = None                # com quem se está a falar (None = conversa fechada)
        self.chat_name = ""
        self.chat_messages = []             # [{"id", "from_uid", "text", "sent_at"}], da mais antiga à mais nova
        self.chat_loading = False
        self.chat_error = None
        self.chat_next_poll = 0.0
        self.chat_scroll = 0.0              # 0 = no fim (mensagens mais recentes)
        self.chat_max_scroll = 0.0
        self.chat_list_rect = pygame.Rect(0, 0, 0, 0)
        self.chat_field = TextField("text")
        self.chat_focus = False
        self.chat_sending = False
        self.chat_seen = {}                 # uid -> hora da última mensagem já vista
        self.chat_last = {}                 # uid -> hora da última mensagem da conversa (ponto vermelho)

    # ---------------------------------------------------------------- abrir / fechar
    def open_chat(self, uid, username=""):
        if not self.friends_ready():
            return
        self.chat_uid = uid
        self.chat_name = username or uid
        self.chat_messages = []
        self.chat_error = None
        self.chat_scroll = 0.0
        self.chat_next_poll = 0.0
        self.chat_field.set_text("")
        self.set_chat_focus(True)
        self.poll_chat(force=True)

    def close_chat(self):
        self.chat_uid = None
        self.set_chat_focus(False)
        self.chat_field.set_text("")

    def set_chat_focus(self, focused):
        if focused == self.chat_focus:
            return
        self.chat_focus = bool(focused)
        if self.chat_focus:
            self.start_text_input()         # no telemóvel é isto que faz aparecer o teclado
        else:
            self.stop_text_input()

    def chat_unread(self, uid):
        """True se a última mensagem desse amigo ainda não foi vista (ponto vermelho na lista)."""
        last = self.chat_last.get(uid)
        return bool(last and last > self.chat_seen.get(uid, 0.0) + 0.5)

    # ---------------------------------------------------------------- teclado
    def handle_chat_key(self, event):
        if not (self.chat_uid and self.chat_focus):
            return False
        f = self.chat_field
        k = event.key
        if k in (pygame.K_RETURN, pygame.K_KP_ENTER):
            self.send_chat_message()
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
    def poll_chat(self, force=False):
        """Lê as mensagens da conversa aberta. Só corre com a conversa à frente do jogador."""
        if not (self.chat_uid and self.friends_ready()) or self.chat_loading:
            return
        now = time.time()
        if not force and now < self.chat_next_poll:
            return
        self.chat_loading = True
        self.chat_next_poll = now + CHAT_POLL
        client, uid = self.client, self.chat_uid

        def ok(messages):
            self.chat_loading = False
            if self.chat_uid != uid:        # entretanto fechou-se (ou abriu-se outra)
                return
            self.chat_error = None
            at_end = self.chat_scroll <= 1.0
            self.chat_messages = messages
            if messages:
                self.chat_seen[uid] = max(m["sent_at"] or 0.0 for m in messages)
                self.chat_last[uid] = self.chat_seen[uid]
            if at_end:
                self.chat_scroll = 0.0      # a chegar mensagem nova, continua a mostrar o fim

        def err(e):
            self.chat_loading = False
            if self.chat_uid == uid:
                self.chat_error = online_error_text(e)

        self.worker.run(lambda: client.list_messages(uid, CHAT_LIMIT), ok, err)

    def tick_chat(self, now):
        if self.chat_uid and self.friends_open:
            self.poll_chat()

    # ---------------------------------------------------------------- escrever
    def send_chat_message(self):
        if not (self.chat_uid and self.friends_ready()) or self.chat_sending:
            return
        text = " ".join(self.chat_field.text.split())
        if len(text) < CHAT_MIN_LEN:
            return
        self.chat_sending = True
        client, uid = self.client, self.chat_uid
        me = self.account.get("uid")

        def ok(_res):
            self.chat_sending = False
            if self.chat_uid != uid:
                return
            self.chat_field.set_text("")
            # mostra-se já a mensagem (a leitura seguinte traz a versão do servidor)
            self.chat_messages = (self.chat_messages +
                                  [{"id": "local", "from_uid": me, "text": text,
                                    "sent_at": time.time()}])[-CHAT_LIMIT:]
            self.chat_scroll = 0.0
            self.chat_next_poll = time.time() + 1.0

        def err(e):
            self.chat_sending = False
            if self.chat_uid == uid:
                self.chat_error = online_error_text(e)

        self.worker.run(lambda: client.send_message(uid, text), ok, err)
