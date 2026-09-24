"""Ganhos offline: ao reabrir um save, dá uma parte do rendimento do tempo em que estiveste fora."""

import time

from core.formatting import format_number, format_playtime
from i18n import tr
from core.upgrades import OFFLINE_BASE_HOURS, OFFLINE_BASE_RATE, OFFLINE_MAX_HOURS, OFFLINE_RATE_STEP, OFFLINE_TIME_STEP

OFFLINE_MIN_SECONDS = 60          # abaixo disto não vale a pena mostrar/dar nada
OFFLINE_MAX_RATE = 1.0            # nunca mais de 100% do rendimento normal


class OfflineMixin:
    """Ganhos offline: calcula e aplica o dinheiro ganho enquanto o jogo estava fechado."""

    def offline_earn_rate(self):
        """Que fração do rendimento normal (dinheiro/seg) recebes offline. Começa em 30% e sobe com as
        upgrades Offline Earnings e com as recompensas de Rebirth."""
        rate = (OFFLINE_BASE_RATE
                + OFFLINE_RATE_STEP * (self.upgrade_level("offline_rate") + self.upgrade_level("offline_rate_2"))
                + self.rebirth_bonus("offline_rate"))
        return min(OFFLINE_MAX_RATE, rate)

    def offline_max_seconds(self):
        """Quantas horas fora contam, no máximo. Começa em 8h e sobe com as upgrades Offline Time e com as
        recompensas de Rebirth."""
        hours = (OFFLINE_BASE_HOURS
                 + OFFLINE_TIME_STEP * (self.upgrade_level("offline_time") + self.upgrade_level("offline_time_2"))
                 + self.rebirth_bonus("offline_time"))
        return min(OFFLINE_MAX_HOURS, hours) * 3600

    def claim_offline_earnings(self):
        """Chama-se logo a seguir a carregar um save (local ou de conta). Devolve (ganho, segundos):
        ganho = 0.0 se não houver nada a dar (1ª vez a abrir este save, ou esteve pouco tempo fora)."""
        now = time.time()
        last = self.last_seen
        self.last_seen = now
        if last is None:
            return 0.0, 0.0
        elapsed = now - last
        if elapsed < OFFLINE_MIN_SECONDS:
            return 0.0, 0.0
        elapsed = min(elapsed, self.offline_max_seconds())
        gain = self.income_per_second() * elapsed * self.offline_earn_rate()
        if gain > 0:
            self.coins += gain
            self.total_coins_earned += gain
        return gain, elapsed


def offline_message(gain, seconds):
    """Texto do aviso ao reabrir um save (gain/seconds vêm de claim_offline_earnings)."""
    return tr("Welcome back! +$%s earned while you were away (%s).", format_number(gain), format_playtime(seconds))
