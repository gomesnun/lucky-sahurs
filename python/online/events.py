"""Eventos globais: uma conta de admin comeca um bonus com tempo (ex.: x10 de sorte durante
5 minutos) e ele vale para TODA a gente que esteja online.

Como funciona: o evento e um unico documento na cloud, /events/current, com
{kind, mult, ends_at, by}. Toda a gente o le (uma chamada pequena de EVENT_POLL em EVENT_POLL
segundos) e aplica o multiplicador enquanto ends_at ainda nao passou. Quem o pode escrever
decide-se nas regras do Firestore (/admins/{uid}), nunca aqui - sem esse documento o servidor
recusa, por muito que o jogo peca."""

import time

from core.global_event import set_event_mults
from i18n import tr
from online.firebase import online_error_text
from theme import BAD, GOOD


EVENT_POLL = 45.0           # segundos entre leituras do evento (documento pequeno, uma chamada)
EVENT_KINDS = ("luck", "money", "speed")
EVENT_MULTS = (2, 5, 10, 25)
EVENT_MINUTES = (1, 5, 15, 30)
EVENT_MAX_SECONDS = 3600    # o mesmo limite que esta nas regras do Firestore


def event_kind_label(kind):
    return {"luck": tr("Luck"), "money": tr("Money"), "speed": tr("Auto Speed")}.get(kind, kind)


class EventsMixin:
    """Estado, leitura e (para o admin) controlo dos eventos globais."""

    # ================================================================ ONLINE: eventos (estado)
    def init_events(self):
        self.event_current = None       # {"kind", "mult", "ends_at", "by"} ou None
        self.event_next_poll = 0.0
        self.event_loading = False
        self.event_is_admin = False     # so True depois de a cloud confirmar /admins/{uid}
        self.event_admin_checked = False
        self.event_admin_open = False
        self.event_admin_mult = 10
        self.event_admin_minutes = 5
        self.event_busy = False
        self.event_msg = None           # (texto, cor)

    # ---------------------------------------------------------------- o bonus a decorrer
    def active_event(self):
        """O evento se ainda estiver a decorrer, ou None (tambem serve para o deixar cair sozinho)."""
        ev = self.event_current
        if ev and ev.get("ends_at", 0.0) > time.time():
            return ev
        return None

    def event_mult(self, kind):
        ev = self.active_event()
        if ev and ev.get("kind") == kind:
            return max(1.0, float(ev.get("mult") or 1.0))
        return 1.0

    def event_seconds_left(self):
        ev = self.active_event()
        return max(0.0, ev["ends_at"] - time.time()) if ev else 0.0

    # ---------------------------------------------------------------- ler
    def events_ready(self):
        return bool(self.client and self.account)

    def poll_events(self, force=False):
        if not self.events_ready() or self.event_loading:
            return
        now = time.time()
        if not force and now < self.event_next_poll:
            return
        self.event_loading = True
        self.event_next_poll = now + EVENT_POLL
        client = self.client

        def ok(ev):
            self.event_loading = False
            self.event_current = ev

        def err(_e):
            # falhar a ler o evento nao e um erro que valha a pena mostrar: joga-se na mesma
            self.event_loading = False

        self.worker.run(client.get_event, ok, err)

    def check_admin(self):
        """Pergunta a cloud, uma vez por sessao, se esta conta pode comecar eventos."""
        if not self.events_ready() or self.event_admin_checked:
            return
        self.event_admin_checked = True
        client = self.client
        self.worker.run(client.is_admin, lambda v: setattr(self, "event_is_admin", bool(v)),
                        lambda _e: None)

    def tick_events(self, now):
        if self.events_ready():
            self.check_admin()
            self.poll_events()
        # o GameState nao conhece a parte online: o bonus passa-lhe por core/global_event.py
        ev = self.active_event()
        kind = ev["kind"] if ev else None
        mult = float(ev["mult"]) if ev else 1.0
        set_event_mults(luck=mult if kind == "luck" else 1.0,
                        money=mult if kind == "money" else 1.0,
                        speed=mult if kind == "speed" else 1.0)

    # ---------------------------------------------------------------- admin
    def toggle_event_admin(self):
        self.event_admin_open = not self.event_admin_open
        self.event_msg = None

    def close_event_admin(self):
        self.event_admin_open = False
        self.event_msg = None

    def set_event_mult(self, mult):
        self.event_admin_mult = int(mult)

    def set_event_minutes(self, minutes):
        self.event_admin_minutes = int(minutes)

    def start_event(self, kind):
        if not (self.events_ready() and self.event_is_admin) or self.event_busy:
            return
        seconds = min(EVENT_MAX_SECONDS, self.event_admin_minutes * 60)
        mult = self.event_admin_mult
        self.event_busy = True
        self.event_msg = None
        client = self.client

        def ok(ev):
            self.event_busy = False
            self.event_current = ev
            self.event_next_poll = 0.0
            self.event_msg = (tr("Event started for everyone!"), GOOD)

        def err(e):
            self.event_busy = False
            self.event_msg = (online_error_text(e), BAD)

        self.worker.run(lambda: client.start_event(kind, mult, seconds), ok, err)

    def stop_event(self):
        if not (self.events_ready() and self.event_is_admin) or self.event_busy:
            return
        self.event_busy = True
        self.event_msg = None
        client = self.client

        def ok(_res):
            self.event_busy = False
            self.event_current = None
            self.event_next_poll = 0.0
            self.event_msg = (tr("Event stopped."), GOOD)

        def err(e):
            self.event_busy = False
            self.event_msg = (online_error_text(e), BAD)

        self.worker.run(client.stop_event, ok, err)
