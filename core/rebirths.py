"""Rebirths: reseta dinheiro (e, até ao Rebirth 10, também as upgrades) a troco de um bónus PERMANENTE de
sorte e dinheiro, e desbloqueia recompensas em certos números de rebirths (REBIRTH_REWARDS).
Não há limite de rebirths - o custo só sobe (exponencial) a cada um."""

from core import balance as B
from i18n import tr

REBIRTH_BASE_COST = B.REBIRTH_BASE_COST        # custo do 1º rebirth (coins na mão, não total_coins_earned)
REBIRTH_COST_MULT = B.REBIRTH_COST_MULT        # cada rebirth seguinte custa isto vezes mais
REBIRTH_MONEY_PER = B.REBIRTH_MONEY_PER        # +x% dinheiro/seg por rebirth (permanente, empilha com tudo o resto)
REBIRTH_LUCK_PER = B.REBIRTH_LUCK_PER          # +x% peso em pets Raros ou melhores por rebirth (permanente)

# ----------------------------------------------------------------------------------
# RECOMPENSAS DE REBIRTH (aparecem na página de Rebirth)
# ----------------------------------------------------------------------------------
# (rebirths precisos, nome, {tipo de bónus: valor}). Ficam ativas para sempre assim que chegas ao número
# (não se gravam no save: vêm do nº de rebirths). Tipos:
#   money / luck / secret_luck / auto_speed / trait_luck / charge_chance / golden_luck / diamond_luck /
#   offline_rate  -> percentagem (0.10 = +10%; offline_rate soma pontos à % de ganhos offline)
#   slots         -> slots de equipar a mais
#   offline_time  -> horas a mais de ganhos offline
#   keep_upgrades -> a partir daqui um Rebirth só reseta o dinheiro (as upgrades ficam)
# Golden/Diamond: as 5 recompensas somam +25% cada (parte das contas do teto de mutação em core/pets.py).
REBIRTH_REWARDS = [
    (2,  "Second Wind",     {"money": 0.10}),
    (4,  "Lucky Start",     {"luck": 0.05}),
    (6,  "Full House",      {"slots": 1}),
    (8,  "Cash Flow",       {"money": 0.15}),
    (10, "Rebirth Master",  {"keep_upgrades": 1}),
    (12, "Shiny Hunter",    {"golden_luck": 0.05, "diamond_luck": 0.05}),
    (14, "Night Owl",       {"offline_rate": 0.10}),
    (16, "Trait Collector", {"charge_chance": 0.15}),
    (18, "Trait Whisperer", {"trait_luck": 0.10}),
    (20, "Glitter Storm",   {"golden_luck": 0.05, "diamond_luck": 0.05}),
    (23, "Extra Seat",      {"slots": 1}),
    (26, "Shiny Expert",    {"golden_luck": 0.05, "diamond_luck": 0.05}),
    (29, "Secret Keeper",   {"secret_luck": 0.10}),
    (32, "Shiny Master",    {"golden_luck": 0.05, "diamond_luck": 0.05}),
    (35, "Sleepless",       {"offline_time": 2, "money": 0.15}),
    (38, "Turbo Roller",    {"auto_speed": 0.10, "luck": 0.05}),
    (40, "Shiny Legend",    {"golden_luck": 0.05, "diamond_luck": 0.05, "money": 0.25}),
]

# BALANCE: as recompensas de dinheiro / sorte / velocidade do auto são multiplicadas por B.REBIRTH_REWARD_SCALE
# (aqui, uma vez, por isso a página de Rebirth mostra os valores finais).
REBIRTH_REWARDS = [
    (need, name, {k: (round(v * B.REBIRTH_REWARD_SCALE.get(k, 1.0), 2) if k in B.REBIRTH_REWARD_SCALE else v)
                  for k, v in rewards.items()})
    for need, name, rewards in REBIRTH_REWARDS
]

REBIRTH_REWARD_LABELS = {
    "money": "Money", "luck": "Luck", "secret_luck": "Secret+ Luck", "auto_speed": "Auto Speed",
    "trait_luck": "Trait Luck", "charge_chance": "Trait Charge Chance",
    "golden_luck": "Golden Luck", "diamond_luck": "Diamond Luck", "offline_rate": "Offline Earnings",
}


def format_rebirth_reward(rewards):
    """Texto de uma recompensa (já no idioma atual), ex.: '+5% Golden Luck, +5% Diamond Luck'."""
    parts = []
    for kind, value in rewards.items():
        if kind == "keep_upgrades":
            parts.append(tr("Rebirth only resets your coins from now on"))
        elif kind == "slots":
            parts.append(tr("+%d Equip Slot", value) if value == 1 else tr("+%d Equip Slots", value))
        elif kind == "offline_time":
            parts.append(tr("+%g hours max offline time", value))
        else:
            parts.append(tr("+%.0f%% %s", value * 100, tr(REBIRTH_REWARD_LABELS.get(kind, kind))))
    return ", ".join(parts)


class RebirthMixin:
    """Rebirths: custo, bónus permanentes, recompensas e o próprio reset."""

    def rebirth_cost(self, n=None):
        """Custo para ires de 'n' rebirths para 'n+1' (por omissão, o próximo)."""
        n = self.rebirths if n is None else n
        return REBIRTH_BASE_COST * (REBIRTH_COST_MULT ** n)

    def rebirth_available(self):
        return self.coins >= self.rebirth_cost()

    def rebirth_money_mult(self):
        return 1.0 + REBIRTH_MONEY_PER * self.rebirths

    def rebirth_luck_mult(self):
        return 1.0 + REBIRTH_LUCK_PER * self.rebirths

    # ---------------- recompensas ----------------
    def rebirth_bonus(self, kind):
        """Soma do bónus deste tipo em todas as recompensas de Rebirth já desbloqueadas (0 se nenhuma).
        Guardado em cache até o nº de rebirths mudar (isto é chamado muitas vezes por segundo)."""
        cache = getattr(self, "_rb_cache", None)
        if cache is None or cache[0] != self.rebirths:
            totals = {}
            for need, _name, rewards in REBIRTH_REWARDS:
                if self.rebirths >= need:
                    for k, v in rewards.items():
                        totals[k] = totals.get(k, 0) + v
            cache = (self.rebirths, totals)
            self._rb_cache = cache
        return cache[1].get(kind, 0)

    def rebirth_keeps_upgrades(self):
        """True = o próximo Rebirth só reseta o dinheiro (recompensa 'Rebirth Master', a partir do Rebirth 10)."""
        return self.rebirth_bonus("keep_upgrades") > 0

    def next_rebirth_reward(self):
        """Índice (em REBIRTH_REWARDS) da próxima recompensa por desbloquear, ou None se já tens todas."""
        for i, (need, _name, _rewards) in enumerate(REBIRTH_REWARDS):
            if self.rebirths < need:
                return i
        return None

    def do_rebirth(self):
        """Reseta o dinheiro (e as upgrades, enquanto ainda não tens a recompensa 'Rebirth Master') e sobe o
        nº de rebirths (bónus permanente de sorte/dinheiro). Pets, traits, milestones, total_rolls, playtime...
        nada disso é tocado. Devolve True se o rebirth aconteceu (False = não tinhas dinheiro suficiente)."""
        if not self.rebirth_available():
            return False
        keep_upgrades = self.rebirth_keeps_upgrades()      # decidido ANTES de subir o contador
        self.rebirths += 1
        self.coins = 0.0
        if not keep_upgrades:
            for k in self.upgrades:
                self.upgrades[k] = 0
            # com as upgrades de slots resetadas, os slots equipados a mais deixam de caber
            del self.equipped[self.max_slots():]
        return True
