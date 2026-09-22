"""Amigos: procurar, pedidos, lista e perfis públicos (lógica; o ecrã está em ui/friends_panel.py)."""

import time
import pygame

from core.pets import MUT_ORDER, MUTATIONS, PET_ORDER, RARITIES
from i18n import tr
from online.firebase import USERNAME_RE, online_error_text
from theme import BAD, GOOD
from ui.widgets import TextField


FRIEND_REFRESH = 90.0          # segundos entre atualizações da lista (com a página aberta)
FRIEND_BADGE_POLL = 240.0      # com a página fechada só se contam os pedidos recebidos, de 4 em 4 min
FRIEND_PROFILE_FETCH = 24      # fotos de perfil pedidas de uma vez (uma chamada por amigo)
FRIEND_MAX = 100


class FriendsMixin:
    """Amigos: estado e chamadas à cloud. O desenho está em ui/friends_panel.py."""

    # ================================================================ ONLINE: amigos (estado)
    def init_friends(self):
        self.friends_open = False
        self.friends_tab = "friends"            # "friends" | "requests" | "add"
        self.friends_scroll = 0.0
        self.friends_max_scroll = 0.0
        self.friends_list_rect = pygame.Rect(0, 0, 0, 0)

        self.friends_list = []                  # [{"uid", "username", "avatar_pet", "avatar_mut"}]
        self.friends_incoming = []              # pedidos recebidos
        self.friends_outgoing = []              # pedidos enviados
        self.friends_loaded = False
        self.friends_loading = False
        self.friends_error = None
        self.friends_next_refresh = 0.0
        self.friends_badge_at = 0.0             # próxima contagem de pedidos com a página fechada

        self.friends_search = TextField("username")
        self.friends_focus = False              # o campo de procura está a receber o teclado
        self.friends_search_busy = False
        self.friends_result = None              # perfil encontrado, ou "none" se não existir ninguém assim
        self.friends_msg = None                 # (texto, cor)
        self.friends_action = None              # uid com uma ação a decorrer (botões desse cartão desligados)

        self.friend_view = None                 # uid do amigo aberto (ecrã de stats)
        self.friend_stats = {}                  # uid -> stats públicas (ou "none")
        self.friend_stats_busy = None

        self.avatar_picker = False
        self.friends_profile_pushed = False      # já se publicou o perfil público nesta sessão

    # ---------------------------------------------------------------- abrir / fechar
    def friends_ready(self):
        return bool(self.client and self.account)

    def open_friends(self):
        self.close_overlays()
        self.friends_open = True
        self.friends_scroll = 0.0
        self.friends_msg = None
        self.friend_view = None
        self.avatar_picker = False
        self.push_profile()
        self.refresh_friends()

    def close_friends(self):
        self.close_chat()
        self.friends_open = False
        self.avatar_picker = False
        self.friend_view = None
        self.set_friends_focus(False)

    def toggle_friends(self):
        if self.friends_open:
            self.close_friends()
        else:
            self.open_friends()

    def set_friends_tab(self, tab):
        self.friends_tab = tab
        self.friends_scroll = 0.0
        self.friends_msg = None
        self.friend_view = None
        self.set_friends_focus(tab == "add")

    def set_friends_focus(self, focused):
        if focused == self.friends_focus:
            return
        self.friends_focus = bool(focused)
        if self.friends_focus:
            self.start_text_input()             # no telemóvel é isto que faz aparecer o teclado
        else:
            self.stop_text_input()

    def friends_pending_count(self):
        return len(self.friends_incoming)

    # ---------------------------------------------------------------- teclado (campo de procura)
    def handle_friends_key(self, event):
        """Teclas com o campo de procura selecionado. Devolve True se a tecla foi usada."""
        if not (self.friends_open and self.friends_focus):
            return False
        k = event.key
        if k in (pygame.K_RETURN, pygame.K_KP_ENTER):
            self.submit_friend_search()
            return True
        f = self.friends_search
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
        if k in (pygame.K_HOME, pygame.K_UP):
            f.home()
            return True
        if k in (pygame.K_END, pygame.K_DOWN):
            f.end()
            return True
        if k == pygame.K_v and (event.mod & (pygame.KMOD_CTRL | pygame.KMOD_META)):
            from ui.widgets import clipboard_text
            self.friends_search.add(clipboard_text())
            return True
        if k == pygame.K_ESCAPE:
            return False                        # o ESC continua a fechar a página
        return False

    # ---------------------------------------------------------------- perfil público (username + foto)
    def avatar_pair(self):
        """[indice_do_verity, mutação] da foto escolhida, ou (None, "normal") se não houver."""
        av = getattr(self.state, "avatar", None)
        if not av:
            return None, "normal"
        return av[0], av[1]

    def push_profile(self, force=False):
        """Publica o username + a foto em /profiles/{uid}. É por aqui que os amigos te encontram na
        procura, por isso publica-se assim que a página abre (e sempre que a foto muda)."""
        if not self.friends_ready():
            return
        if self.friends_profile_pushed and not force:
            return
        self.friends_profile_pushed = True
        pet, mut = self.avatar_pair()
        self.worker.run(lambda: self.client.publish_profile(pet, mut), None, self._friends_quiet_err)

    def _friends_quiet_err(self, _e):
        pass

    def avatar_options(self):
        """Os verities que já tens (pelo menos 1), por ordem de raridade: as fotos que podes escolher."""
        owned = getattr(self.state, "owned", {}) or {}
        out = []
        for idx in PET_ORDER:
            for mut in MUT_ORDER:
                try:
                    if int(owned.get("%d_%s" % (idx, mut), 0)) > 0:
                        out.append((idx, mut))
                except (TypeError, ValueError):
                    continue
        return out

    def set_avatar(self, pet_index, mutation):
        if not (0 <= pet_index < len(RARITIES) and mutation in MUTATIONS):
            return
        self.state.avatar = [pet_index, mutation]
        self.state.dirty = True
        self.avatar_picker = False
        self.play("click")
        self.push_profile(force=True)

    def clear_avatar(self):
        self.state.avatar = None
        self.state.dirty = True
        self.avatar_picker = False
        self.push_profile(force=True)

    # ---------------------------------------------------------------- lista de amigos e pedidos
    def refresh_friends(self, force=False):
        if not self.friends_ready() or self.friends_loading:
            return
        now = time.time()
        if not force and now < self.friends_next_refresh:
            return
        self.friends_next_refresh = now + FRIEND_REFRESH
        self.friends_loading = True
        self.friends_error = None

        client = self.client

        def job():
            friends = client.list_friends()
            # as fotos vêm de /profiles/{uid}: uma chamada por amigo, e só para os primeiros
            # (com muitos amigos não vale a pena segurar a lista à espera de todas as fotos)
            for entry in friends[:FRIEND_PROFILE_FETCH]:
                try:
                    prof = client.get_public_profile(entry["uid"])
                except Exception:
                    prof = None
                if prof:
                    entry["avatar_pet"] = prof.get("avatar_pet")
                    entry["avatar_mut"] = prof.get("avatar_mut", "normal")
                    if prof.get("username"):
                        entry["username"] = prof["username"]
            incoming = client.list_friend_requests(True)
            outgoing = client.list_friend_requests(False)
            # quem te mandou um pedido também aparece com a foto dele (senão ficava o círculo vazio)
            for entry in incoming[:FRIEND_PROFILE_FETCH]:
                try:
                    prof = client.get_public_profile(entry["uid"])
                except Exception:
                    prof = None
                if prof:
                    entry["avatar_pet"] = prof.get("avatar_pet")
                    entry["avatar_mut"] = prof.get("avatar_mut", "normal")
            # última mensagem de cada conversa (um GET por amigo, só para os primeiros): é o que
            # acende o ponto vermelho na lista sem ter de ler as mensagens todas
            chats = {}
            for entry in friends[:FRIEND_PROFILE_FETCH]:
                try:
                    summary = client.get_chat_summary(entry["uid"])
                except Exception:
                    summary = None
                if summary and summary.get("last_at"):
                    chats[entry["uid"]] = summary["last_at"]
            return {"friends": friends, "incoming": incoming, "outgoing": outgoing, "chats": chats}

        def ok(res):
            self.friends_loading = False
            self.friends_loaded = True
            self.friends_list = res["friends"]
            self.friends_incoming = res["incoming"]
            self.friends_outgoing = res["outgoing"]
            self.chat_last.update(res.get("chats") or {})
            self.friends_badge_at = time.time() + FRIEND_BADGE_POLL

        def err(e):
            self.friends_loading = False
            self.friends_error = online_error_text(e)
            self.friends_next_refresh = time.time() + 60.0

        self.worker.run(job, ok, err)

    def tick_friends(self, now):
        """Com a página fechada, só se vai buscar a contagem de pedidos (para o sinal no botão)."""
        if not self.friends_ready() or self.friends_open or self.friends_loading:
            return
        if now < self.friends_badge_at:
            return
        self.friends_badge_at = now + FRIEND_BADGE_POLL
        self.worker.run(lambda: self.client.list_friend_requests(True),
                        self._on_badge, self._friends_quiet_err)

    def _on_badge(self, requests):
        self.friends_incoming = requests

    # ---------------------------------------------------------------- procurar e adicionar
    def submit_friend_search(self):
        if not self.friends_ready() or self.friends_search_busy:
            return
        name = self.friends_search.text.strip().lower()
        if not USERNAME_RE.match(name):
            self.friends_msg = (tr("Usernames have 3 to 16 letters, numbers or _."), BAD)
            return
        if self.account and name == self.account.get("username"):
            self.friends_msg = (tr("That's you!"), BAD)
            return
        self.friends_search_busy = True
        self.friends_result = None
        self.friends_msg = None

        def ok(profile):
            self.friends_search_busy = False
            self.friends_result = profile or "none"
            if not profile:
                self.friends_msg = (tr("No player with that name."), BAD)

        def err(e):
            self.friends_search_busy = False
            self.friends_msg = (online_error_text(e), BAD)

        self.worker.run(lambda: self.client.find_profile(name), ok, err)

    def friend_relation(self, uid):
        """"friend" | "sent" | "incoming" | None - para saber que botão mostrar."""
        if any(f["uid"] == uid for f in self.friends_list):
            return "friend"
        if any(r["uid"] == uid for r in self.friends_outgoing):
            return "sent"
        if any(r["uid"] == uid for r in self.friends_incoming):
            return "incoming"
        return None

    def send_friend_request(self, uid, username):
        if not self.friends_ready() or self.friends_action:
            return
        if len(self.friends_list) >= FRIEND_MAX:
            self.friends_msg = (tr("Your friends list is full (%d).", FRIEND_MAX), BAD)
            return
        self.friends_action = uid
        self.friends_msg = None

        def ok(_res):
            self.friends_action = None
            self.friends_outgoing.append({"uid": uid, "username": username, "created_at": None})
            self.friends_msg = (tr("Friend request sent to %s.", username), GOOD)

        def err(e):
            self.friends_action = None
            self.friends_msg = (online_error_text(e), BAD)

        self.worker.run(lambda: self.client.send_friend_request(uid, username), ok, err)

    def accept_friend(self, req):
        if not self.friends_ready() or self.friends_action:
            return
        uid, username = req["uid"], req["username"]
        self.friends_action = uid

        def ok(_res):
            self.friends_action = None
            self.friends_incoming = [r for r in self.friends_incoming if r["uid"] != uid]
            if not any(f["uid"] == uid for f in self.friends_list):
                self.friends_list.append({"uid": uid, "username": username})
                self.friends_list.sort(key=lambda e: e["username"])
            self.friends_msg = (tr("%s is now your friend.", username), GOOD)
            self.refresh_friends(force=True)        # traz a foto de perfil dele

        def err(e):
            self.friends_action = None
            self.friends_msg = (online_error_text(e), BAD)

        self.worker.run(lambda: self.client.accept_friend_request(uid, username), ok, err)

    def decline_friend(self, req):
        """Recusar um pedido recebido."""
        self._drop_request(req, incoming=True)

    def cancel_friend_request(self, req):
        """Desistir de um pedido que enviaste."""
        self._drop_request(req, incoming=False)

    def _drop_request(self, req, incoming):
        if not self.friends_ready() or self.friends_action:
            return
        uid = req["uid"]
        me = self.account["uid"]
        to_uid, from_uid = (me, uid) if incoming else (uid, me)
        self.friends_action = uid

        def ok(_res):
            self.friends_action = None
            if incoming:
                self.friends_incoming = [r for r in self.friends_incoming if r["uid"] != uid]
            else:
                self.friends_outgoing = [r for r in self.friends_outgoing if r["uid"] != uid]

        def err(e):
            self.friends_action = None
            self.friends_msg = (online_error_text(e), BAD)

        self.worker.run(lambda: self.client.delete_friend_request(to_uid, from_uid), ok, err)

    def remove_friend(self, uid):
        if not self.friends_ready() or self.friends_action:
            return
        self.friends_action = uid

        def ok(_res):
            self.friends_action = None
            self.friends_list = [f for f in self.friends_list if f["uid"] != uid]
            if self.friend_view == uid:
                self.friend_view = None

        def err(e):
            self.friends_action = None
            self.friends_msg = (online_error_text(e), BAD)

        self.worker.run(lambda: self.client.remove_friend(uid), ok, err)

    # ---------------------------------------------------------------- stats de um amigo
    def open_friend(self, uid):
        self.friend_view = uid
        self.friends_msg = None
        self.set_friends_focus(False)
        if uid in self.friend_stats or self.friend_stats_busy == uid or not self.friends_ready():
            return
        self.friend_stats_busy = uid

        def ok(stats):
            self.friend_stats_busy = None
            self.friend_stats[uid] = stats or "none"

        def err(e):
            self.friend_stats_busy = None
            self.friends_msg = (online_error_text(e), BAD)

        self.worker.run(lambda: self.client.get_public_stats(uid), ok, err)

    def close_friend_view(self):
        self.close_chat()
        self.friend_view = None
        self.friends_scroll = 0.0
