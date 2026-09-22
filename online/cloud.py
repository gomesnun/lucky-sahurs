"""Ligação à conta na cloud: sessão, saves online, sincronização e leaderboard (lógica)."""

import json
import random
import time
import pygame

from config import SAVE_SLOTS
from core.game_state import GameState
from core.offline import offline_message
from i18n import tr
from online.cloud_cache import (
    backup_state, clear_session, delete_cache, install_id, lb_publish_period,
    load_lb_cache, load_session, peek_cache, pick_save,
    read_cache, store_session, write_cache,
)
from online.firebase import (
    CLOUD_SYNC_INTERVAL, FirebaseClient, LEADERBOARD_MIN_GAP, LEADERBOARD_PERIOD,
    LEADERBOARD_PUBLISH_JITTER,
    OnlineError, SESSION_HEARTBEAT, Worker, load_firebase_config, online_error_text,
    save_summary,
)
from storage import save_slot_path
from theme import BAD, GOOD, GREY
from ui.widgets import TextField


class CloudMixin:
    """Ligação à conta na cloud: sessão, saves online, sincronização e leaderboard (lógica)."""

    # ================================================================ ONLINE: estado
    def init_online(self):
        key, pid = load_firebase_config()
        self.client = FirebaseClient(key, pid) if (key and pid) else None
        self.worker = Worker()

        # conta
        self.account = None                  # {"uid", "username"} quando há sessão iniciada
        self.account_return = "title"
        self.acc_tab = "login"               # "login" | "register" | "recover"
        self.acc_stage = "form"              # "form" | "verify" | "link_email" | "forgot" | "forgot_sent"
        self.acc_fields = {"username": TextField("username"), "password": TextField("password"),
                           "confirm": TextField("password"), "email": TextField("email"),
                           "code": TextField("code")}
        self.acc_focus = None
        self.acc_msg = None                  # (texto, cor)
        self.acc_busy = False
        self.acc_show_pw = False
        self.acc_pending_email = None        # email por confirmar (ecrã "verify")
        self.acc_reauth_email = None         # email já confirmado, à espera de re-login (ecrã "reauth")
        self.acc_code_sent_at = 0.0          # para o "reenviar código" ter um pequeno intervalo
        self.acc_forgot_email = None         # email para onde foi mandado o link de reset (ecrã final)

        # saves da conta
        self.cache_info = {}                 # slot -> resumo da cache local (para funcionar sem net)
        self.cloud_slots = {}                # slot -> resumo do save na cloud
        self.cloud_slots_loaded = False
        self.cloud_slots_loading = False
        self.cloud_slots_error = None
        self.cloud_slots_error_msg = ""
        self.slots_retry_at = 0.0
        self.slot_starting = False
        self.import_busy = False
        self.upload_inflight = False
        self.sync_timer = 0.0
        self.sync_error = None               # None | "offline" | "conflict" | "auth" | "denied"
        self.sync_last_ok = None

        # "uma conta, um jogo de cada vez"
        self.install_id = install_id()
        self.session_held = False            # este PC é que tem a marca de "a jogar" desta conta
        self.session_timer = 0.0
        self.session_inflight = False

        # leaderboard: publicar a pontuação
        self.pub_inflight = False
        self.pub_last_time = 0.0
        self.pub_retry_at = 0.0
        self.pub_jitter = random.uniform(0.0, LEADERBOARD_PUBLISH_JITTER)

        # leaderboard: página
        self.leaderboard_open = False
        self.lb_tab = "money"                # "money" | "playtime" | "rolls" | "rebirths"
        self.lb_scroll = 0.0
        self.lb_max_scroll = 0.0
        self.lb_list_rect = pygame.Rect(0, 0, 0, 0)
        self.lb_data = load_lb_cache()
        self.lb_loading = False
        self.lb_error = None
        self.lb_retry_at = 0.0
        self.lb_refetch_done = 0.0           # pub_last_time para o qual já se voltou a pedir a leaderboard (ver ensure_leaderboard)

        self.init_friends()                  # amigos (ver online/friends.py)
        self.init_feedback()                 # feedback (ver online/feedback.py)

        self.restore_session()

    def restore_session(self):
        if not self.client:
            return
        sess = load_session()
        if not sess:
            return
        self.client.restore(sess["uid"], sess["username"], sess["refresh_token"])
        self.account = {"uid": sess["uid"], "username": sess["username"]}
        try:
            self.pub_last_time = float(sess.get("last_pub_time", 0.0))
        except (TypeError, ValueError):
            self.pub_last_time = 0.0
        self.refresh_slot_info()
        self.load_cloud_slots()
        # Conta antiga sem email associado (ainda não passou pelo ecrã de link_email): verifica em
        # segundo plano e, se for o caso, força esse ecrã assim que o jogo abre.
        self.worker.run(self.client.get_profile, self.on_startup_profile, lambda _e: None)

    def on_startup_profile(self, profile):
        if not self.account:
            return
        if profile and profile.get("email_verified"):
            # a sessão restaurada não trazia isto (restore_session só põe uid/username) - sem isto,
            # o ecrã de conta mostrava sempre "sem email confirmado", mesmo já estando tudo certo
            self.account["email_verified"] = True
            self.account["email"] = profile.get("email")
            return
        if profile and profile.get("email"):
            # associação a meio (código nunca confirmado) - guarda o email para o ecrã "verify" retomar
            self.account["email"] = profile.get("email")
            self.acc_pending_email = profile.get("email")
        self.require_email_link()

    def save_session_file(self):
        if self.client and self.account and self.client.refresh_token:
            store_session(self.account["uid"], self.account["username"], self.client.refresh_token,
                          self.pub_last_time)

    def log_out(self):
        # Não se larga a marca pela rede aqui: o log out também acontece quando a sessão expira (já não
        # havia autorização para escrever) e uma chamada bloqueante a meio do jogo dava um encravanço de
        # vários segundos. A marca expira sozinha passados SESSION_STALE segundos.
        self.session_held = False
        if self.client:
            self.client.sign_out()
        self.account = None
        self.cloud_slots = {}
        self.cloud_slots_loaded = False
        self.cloud_slots_loading = False
        self.cloud_slots_error = None
        self.cache_info = {}
        self.pub_last_time = 0.0
        self.acc_stage = "form"
        self.acc_pending_email = None
        clear_session()

    def log_out_clicked(self):
        self.log_out()
        self.show_toast(tr("Logged out."))
        self.close_account()

    # ================================================================ ONLINE: saves da conta
    def load_cloud_slots(self):
        if not (self.client and self.account) or self.cloud_slots_loading:
            return
        self.cloud_slots_loading = True
        self.cloud_slots_error = None
        self.cloud_slots_error_msg = ""
        uid = self.account["uid"]
        self.cache_info = {i: c for i, c in ((i, peek_cache(uid, i)) for i in range(1, SAVE_SLOTS + 1)) if c}

        def ok(slots):
            self.cloud_slots_loading = False
            self.cloud_slots = slots
            self.cloud_slots_loaded = True

        def err(e):
            self.cloud_slots_loading = False
            if e.code == "auth":
                self.log_out()
                self.show_toast(tr("Session expired. Please log in again."))
            else:
                self.cloud_slots_error = e.code
                detail = (e.message or "").strip()
                self.cloud_slots_error_msg = online_error_text(e) + ((" [%s]" % detail) if detail else "")

        self.worker.run(self.client.list_saves, ok, err)

    def slot_view(self, slot):
        """(tipo, resumo, etiqueta) para o cartão de um slot no ecrã de Saves.
        tipos: 'save' (jogável) | 'importable' (há um save local antigo e a conta ainda não tem este slot)
               | 'empty' | 'unknown' (a carregar / sem net e sem cópia)."""
        if not self.account:
            info = self.slot_info.get(slot)
            return ("save", info, "") if info else ("empty", None, "")
        cached = self.cache_info.get(slot)
        if self.cloud_slots_loaded:
            s = self.cloud_slots.get(slot)
            if s:
                return "save", {"coins": s["coins"], "total_rolls": s["rolls"], "playtime": s["playtime"]}, "Cloud"
            if cached:
                return "save", cached, "Not uploaded yet"
            local = self.slot_info.get(slot)
            if local:
                return "importable", local, "Local save found"
            return "empty", None, ""
        if cached:
            return "save", cached, "Offline copy"
        return "unknown", None, ""

    def start_cloud_slot(self, slot):
        if self.slot_starting:
            return
        if self.upload_inflight:
            self.show_toast(tr("Syncing your save... try again in a second."))
            return
        uid = self.account["uid"]
        cache = read_cache(uid, slot)
        self.slot_starting = True
        self.show_toast(tr("Loading save..."))

        def job():
            # Primeiro a marca de sessão, só depois o save: se a conta já estiver a ser jogada noutro
            # PC nem se chega a descarregar nada.
            held = False
            try:
                if self.client.session_blocker(self.install_id) is not None:
                    return {"busy": True}
            except OnlineError:
                pass        # só a leitura informativa falhou (rede em baixo): não bloqueia por causa disto
            try:
                self.client.write_session(self.install_id, True)
                held = True
            except OnlineError as e:
                if e.code == "denied":
                    # a Firestore RECUSOU mesmo: outro dispositivo já tem a marca (apanhado aqui mesmo
                    # que a leitura de cima não tivesse dado por isso, por causa da corrida entre os dois)
                    return {"busy": True}
                # falha de rede/servidor: deixa jogar na mesma, a partir da cópia local
            return {"busy": False, "held": held, "cloud": self.client.get_save(slot)}

        def ok(res):
            self.slot_starting = False
            if res["busy"]:
                self.show_toast(tr("This account is already being played on another device. "
                                   "Close the game there and try again."), 6.0)
                return
            self.session_held = res["held"]
            self.session_timer = 0.0
            state_dict, base, dirty, note, loser = pick_save(res["cloud"], cache)
            if loser:
                backup_state(uid, slot, loser[0], loser[1])
            self.begin_cloud_game(slot, state_dict, base, dirty, note)

        def err(e):
            self.slot_starting = False
            if e.code == "auth":
                self.show_toast(tr("Session expired. Please log in again."))
            elif cache and e.code in ("offline", "server", "denied"):
                # sem ligação (ou servidor com problemas): joga a partir da cópia local, sobe mais tarde
                self.session_held = False
                self.session_timer = 0.0
                self.begin_cloud_game(slot, cache["state"], cache["base_time"], True,
                                      tr("Offline: playing from this PC's copy. It will sync later."))
            else:
                self.show_toast(online_error_text(e))

        self.worker.run(job, ok, err)

    def begin_cloud_game(self, slot, state_dict, base_time, dirty, note=None):
        st = GameState()
        st.slot = slot
        st.cloud_uid = self.account["uid"]
        if state_dict is not None:
            try:
                st.load_dict(state_dict)
            except (ValueError, KeyError, TypeError, AttributeError):
                self.show_toast(tr("This save couldn't be read. Nothing was changed."))
                return
        gain, away = st.claim_offline_earnings()      # dinheiro ganho enquanto o jogo esteve fechado
        if gain > 0:
            dirty = True                               # o save mudou (moedas + last_seen): tem de subir para a cloud
        st.cloud_base_time = base_time
        st.dirty = dirty
        write_cache(st.cloud_uid, slot, base_time, dirty, st.to_dict())    # grava já a cache local
        self.state = st
        self.reset_ui()
        self.options_open = False
        self.stats_open = False
        self.leaderboard_open = False
        self.autosave_timer = 0.0
        self.sync_timer = CLOUD_SYNC_INTERVAL - 5.0       # primeiro upload passados ~5 s
        self.sync_error = None
        self.screen_mode = "game"
        text = note or tr("Slot %d loaded", slot)
        if gain > 0:
            self.show_toast("%s\n%s" % (text, offline_message(gain, away)), 6.0)
        else:
            self.show_toast(text)

    def import_local_slot(self, slot):
        """Copia um save local antigo para a conta. O ficheiro local NUNCA é apagado nem alterado,
        e um slot que já exista na cloud nunca é sobrescrito."""
        if self.import_busy or not self.client:
            return
        try:
            with open(save_slot_path(slot), "r", encoding="utf-8") as f:
                data = json.load(f)
            if not isinstance(data, dict):
                raise ValueError("not a dict")
            GameState().load_dict(data)          # só para ver se o save é legível
        except (OSError, ValueError, KeyError, TypeError, AttributeError):
            self.show_toast(tr("Couldn't read the local save for slot %d.", slot))
            return
        self.import_busy = True

        def ok(_t):
            self.import_busy = False
            self.show_toast(tr("Slot %d imported. Your local file was kept.", slot))
            self.load_cloud_slots()

        def err(e):
            self.import_busy = False
            if e.code == "conflict":
                self.show_toast(tr("Slot %d already has a cloud save - nothing was overwritten.", slot))
                self.load_cloud_slots()
            else:
                self.show_toast(online_error_text(e))

        self.worker.run(lambda: self.client.put_save(slot, data, None), ok, err)

    def delete_account_slot(self, slot):
        uid = self.account["uid"]
        delete_cache(uid, slot)
        self.cache_info.pop(slot, None)
        self.cloud_slots.pop(slot, None)

        def ok(_):
            self.show_toast(tr("Slot %d deleted.", slot))
            self.load_cloud_slots()

        def err(e):
            self.show_toast(online_error_text(e))
            self.load_cloud_slots()

        self.worker.run(lambda: self.client.delete_save(slot), ok, err)

    # ================================================================ ONLINE: sincronização
    def start_upload(self, st):
        if not (self.client and st.cloud_uid and st.slot and st.dirty) or st.sync_conflict or self.upload_inflight:
            return
        snap = st.to_dict()
        seq, base, slot, uid = st.save_seq, st.cloud_base_time, st.slot, st.cloud_uid
        self.upload_inflight = True

        def ok(update_time):
            self.upload_inflight = False
            st.cloud_base_time = update_time
            if st.save_seq == seq:
                st.dirty = False                 # nada mudou durante o upload
            write_cache(uid, slot, update_time, st.dirty, snap if not st.dirty else st.to_dict())
            summary = save_summary(snap)
            summary["time"] = update_time
            self.cloud_slots[slot] = summary
            self.sync_error = None
            self.sync_last_ok = time.time()

        def err(e):
            self.upload_inflight = False
            if e.code == "conflict":
                st.sync_conflict = True
                self.sync_error = "conflict"
                self.show_toast(tr("This save changed on another device. Go to the main menu and open the slot again."))
            elif e.code in ("auth", "denied"):
                self.sync_error = e.code
            else:
                self.sync_error = "offline"

        self.worker.run(lambda: self.client.put_save(slot, snap, base), ok, err)

    def flush_cloud_blocking(self):
        """À saída do jogo: tenta enviar já o último progresso (até uns segundos). Se não der,
        o progresso fica na cache local e sobe da próxima vez que abrires o slot."""
        self.upload_cloud_blocking()
        self.release_session_blocking()

    def upload_cloud_blocking(self):
        st = self.state
        if not (self.client and st.cloud_uid and st.slot):
            return
        deadline = time.time() + 6.0
        while self.upload_inflight and time.time() < deadline:
            self.worker.poll()
            time.sleep(0.05)
        if not st.dirty or st.sync_conflict:
            return
        try:
            snap = st.to_dict()
            t = self.client.put_save(st.slot, snap, st.cloud_base_time)
        except OnlineError:
            return
        st.cloud_base_time = t
        st.dirty = False
        write_cache(st.cloud_uid, st.slot, t, False, snap)

    # ================================================================ ONLINE: uma conta, um jogo de cada vez
    def tick_session(self, dt):
        """Renova a marca de "estou a jogar" enquanto o jogo está aberto. Se falhar não acontece nada
        de mau: a marca expira sozinha passados SESSION_STALE segundos."""
        if self.session_inflight:
            return
        self.session_timer += dt
        if self.session_timer < SESSION_HEARTBEAT:
            return
        self.session_timer = 0.0
        self.session_inflight = True

        def ok(_):
            self.session_inflight = False
            self.session_held = True

        def err(_e):
            self.session_inflight = False
            self.session_held = False       # outro PC ficou com ela, ou a rede foi abaixo: tenta outra vez a seguir

        self.worker.run(lambda: self.client.write_session(self.install_id, True), ok, err)

    def release_session(self):
        """Larga a marca ao voltar ao menu: a conta fica logo livre, sem esperar que expire."""
        if not (self.client and self.account and self.session_held):
            return
        self.session_held = False
        self.session_timer = 0.0
        self.worker.run(lambda: self.client.write_session(self.install_id, False))

    def release_session_blocking(self):
        if not (self.client and self.account and self.session_held):
            return
        self.session_held = False
        try:
            self.client.write_session(self.install_id, False)
        except OnlineError:
            pass

    def sync_status(self):
        if self.state.sync_conflict or self.sync_error == "conflict":
            return tr("Cloud: conflict!"), BAD
        if self.sync_error == "auth":
            return tr("Cloud: log in again"), BAD
        if self.sync_error == "denied":
            return tr("Cloud: access denied"), BAD
        if self.sync_error == "offline":
            return tr("Cloud: offline (saved on this PC)"), GREY
        if self.upload_inflight:
            return tr("Cloud: syncing..."), GREY
        if self.sync_last_ok is None:
            return tr("Cloud: not synced yet"), GREY
        return tr("Cloud: synced"), GOOD

    def tick_online(self, dt):
        """Todos os frames dentro do jogo: envia o save para a cloud e publica na leaderboard."""
        st = self.state
        if not (self.client and self.account and st.cloud_uid):
            return
        now = time.time()
        self.sync_timer += dt
        if self.sync_timer >= CLOUD_SYNC_INTERVAL:
            self.sync_timer = 0.0
            self.start_upload(st)
        self.tick_session(dt)
        if not self.cloud_slots_loaded and not self.cloud_slots_loading and now >= self.slots_retry_at:
            self.slots_retry_at = now + 60.0
            self.load_cloud_slots()
        self.tick_publish(now)
        self.tick_friends(now)

    def leaderboard_values(self):
        """Pontuação da CONTA (uma linha na leaderboard):
        dinheiro e rebirths = o melhor save (o total ganho nunca desce ao gastar);
        playtime e rolls = soma de todos os saves."""
        st = self.state
        best_coins, best_rebirths, total_pt, total_rolls = 0.0, 0, 0.0, 0
        for slot in range(1, SAVE_SLOTS + 1):
            if slot == st.slot:
                coins, pt, rolls, rebirths = st.total_coins_earned, st.playtime, st.total_rolls, st.rebirths
            else:
                s = self.cloud_slots.get(slot)
                if s:
                    coins, pt, rolls, rebirths = s["total_earned"], s["playtime"], s["rolls"], s.get("rebirths", 0)
                else:
                    coins, pt, rolls, rebirths = 0.0, 0.0, 0, 0
            best_coins = max(best_coins, coins)
            best_rebirths = max(best_rebirths, rebirths)
            total_pt += pt
            total_rolls += rolls
        return {"coins": best_coins, "playtime": total_pt, "rolls": total_rolls, "rebirths": best_rebirths}

    def tick_publish(self, now):
        if self.pub_inflight or not self.cloud_slots_loaded or now < self.pub_retry_at:
            return
        period = lb_publish_period(now)
        if self.pub_last_time and lb_publish_period(self.pub_last_time) >= period:
            return                                   # já publicou neste período de 10 min
        due = max(period * LEADERBOARD_PERIOD + self.pub_jitter, self.pub_last_time + LEADERBOARD_MIN_GAP)
        if now < due:
            return
        v = self.leaderboard_values()
        self.pub_inflight = True

        def ok(_):
            self.pub_inflight = False
            self.pub_last_time = time.time()
            self.pub_jitter = random.uniform(0.0, LEADERBOARD_PUBLISH_JITTER)
            self.save_session_file()

        def err(e):
            self.pub_inflight = False
            if getattr(e, "code", "") == "denied" or getattr(e, "status", 0) == 429:
                # o servidor recusou (regras do Firestore ainda a pedir um intervalo maior, ou a quota grátis do dia
                # acabou - erro 429): só volta a tentar no período seguinte, em vez de martelar o servidor
                self.pub_retry_at = time.time() + LEADERBOARD_PERIOD
            else:
                self.pub_retry_at = time.time() + 120.0    # sem rede / erro temporário: tenta outra vez daqui a 2 min

        self.worker.run(lambda: self.client.publish_score(v["coins"], v["playtime"], v["rolls"], v["rebirths"]),
                        ok, err)
