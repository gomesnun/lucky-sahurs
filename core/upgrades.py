"""Upgrades: tabela de upgrades, categorias, constantes dos Golden / Diamond / Rainbow Roll e a compra."""

from core.pets import DIAMOND_STEP_1, DIAMOND_STEP_2, GOLDEN_STEP_1, GOLDEN_STEP_2
from i18n import L


# Ganhos offline (ver core/offline.py): valores de partida e passo de cada nível das upgrades Offline.
OFFLINE_BASE_RATE = 0.30       # 30% do rendimento normal
OFFLINE_RATE_STEP = 0.02       # +2% por nível (Offline Earnings e Offline Earnings II)
OFFLINE_BASE_HOURS = 8         # até 8h de ganhos offline
OFFLINE_TIME_STEP = 1          # +1 hora por nível (Offline Time e Offline Time II)
OFFLINE_MAX_HOURS = 48         # teto de segurança (upgrades + recompensas de Rebirth)

UPGRADE_DEFS = {
    "luck": {
        "name": "Luck", "desc": "+12% weight to all Rare pets or better.",
        "max_level": 40, "base_cost": 50, "cost_mult": 1.55, "requires": None,
    },
    "luck_prism": {
        "name": "Prismatic Luck", "desc": "Stacks: +10% to Epic+, again to Mythic+ and again to Secret+.",
        "max_level": 25, "base_cost": 60000, "cost_mult": 1.6, "requires": "luck",
        "requires_level": 10,
    },
    "luck_cosmic": {
        "name": "Exotic Luck", "desc": "+25% weight to Exotic or better, per level.",
        "max_level": 20, "base_cost": 2000000000, "cost_mult": 1.8, "requires": "luck_prism",
        "requires_level": 15,
    },
    "luck_divine": {
        "name": "Divine Luck", "desc": "+40% weight to Divine or better, per level.",
        "max_level": 15, "base_cost": 100000000000, "cost_mult": 2.0, "requires": "luck_cosmic",
        "requires_level": 10,
    },
    "money": {
        "name": "Money", "desc": "+10% money/sec per level.",
        "max_level": 70, "base_cost": 40, "cost_mult": 1.5, "requires": None,
    },
    "money_prism": {
        "name": "Superior Money", "desc": "+25% money/sec per level (multiplies with regular Money).",
        "max_level": 40, "base_cost": 20000000000, "cost_mult": 1.6, "requires": "money",
        "requires_level": 35,
    },
    "slots": {
        "name": "Equip Slots", "desc": "+1 slot to equip pets.",
        "max_level": 20, "base_cost": 300, "cost_mult": 2.4, "requires": None,
    },
    "slots_plus": {
        "name": "Extra Slots", "desc": "+1 slot to equip pets, once your Slots are maxed out.",
        "max_level": 15, "base_cost": 20000000000, "cost_mult": 2.6, "requires": "slots",
        "requires_level": 20,
    },
    "auto_unlock": {
        "name": "Auto Roller", "desc": "Unlocks the Auto Roller: rolls by itself while turned on.",
        "max_level": 1, "base_cost": 120000, "cost_mult": 1, "requires": None,
    },
    "auto_speed": {
        "name": "Auto Speed", "desc": "+40% Auto Roller speed per level.",
        "max_level": 40, "base_cost": 25000, "cost_mult": 1.42, "requires": "auto_unlock",
    },
    "auto_turbo": {
        "name": "Auto Turbo", "desc": "+50% Auto Roller speed per level (multiplies).",
        "max_level": 25, "base_cost": 50000000000, "cost_mult": 1.5, "requires": "auto_speed",
        "requires_level": 20,
    },
    "golden_unlock": {
        "name": "Unlock Golden", "desc": "Allows Golden mutations to appear on rolls (x3 money).",
        "max_level": 1, "base_cost": 2000, "cost_mult": 1, "requires": None,
    },
    "golden_chance": {
        "name": "Golden Luck", "desc": L("+%g%% chance of Golden mutation per level.", GOLDEN_STEP_1 * 100),
        "max_level": 20, "base_cost": 800, "cost_mult": 1.9, "requires": "golden_unlock",
    },
    "golden_chance_2": {
        "name": "Golden Luck II", "desc": L("+%g%% chance of Golden mutation per level.", GOLDEN_STEP_2 * 100),
        "max_level": 15, "base_cost": 1500000000, "cost_mult": 1.85, "requires": "golden_chance",
        "requires_level": 20,
    },
    "diamond_unlock": {
        "name": "Unlock Diamond", "desc": "Allows Diamond mutations to appear on rolls (x9 money).",
        "max_level": 1, "base_cost": 25000, "cost_mult": 1, "requires": "golden_unlock",
    },
    "diamond_chance": {
        "name": "Diamond Luck", "desc": L("+%g%% chance of Diamond mutation per level.", DIAMOND_STEP_1 * 100),
        "max_level": 22, "base_cost": 10000, "cost_mult": 2.0, "requires": "diamond_unlock",
    },
    "diamond_chance_2": {
        "name": "Diamond Luck II", "desc": L("+%g%% chance of Diamond mutation per level.", DIAMOND_STEP_2 * 100),
        "max_level": 15, "base_cost": 50000000000, "cost_mult": 1.9, "requires": "diamond_chance",
        "requires_level": 22,
    },
    "trait_charge_luck": {
        "name": "Trait Charge Luck", "desc": "+6% extra (relative) chance of getting 1 trait charge "
        "per roll, per level.",
        "max_level": 18, "base_cost": 15000, "cost_mult": 1.7, "requires": None,
    },
    "trait_charge_luck_2": {
        "name": "Trait Charge Luck II", "desc": "+10% extra (relative) chance of getting 1 trait charge "
        "per roll, per level.",
        "max_level": 15, "base_cost": 1000000000, "cost_mult": 1.85, "requires": "trait_charge_luck",
        "requires_level": 18,
    },
    "trait_rarity_luck": {
        "name": "Trait Luck", "desc": "+9% weight to traits from Instinctive (the 3rd) upward "
        "when you roll a trait, per level.",
        "max_level": 20, "base_cost": 20000, "cost_mult": 1.8, "requires": None,
    },
    "trait_rarity_luck_2": {
        "name": "Trait Luck II", "desc": "+15% weight to traits from Instinctive upward "
        "when you roll a trait, per level.",
        "max_level": 15, "base_cost": 8000000000, "cost_mult": 1.9, "requires": "trait_rarity_luck",
        "requires_level": 20,
    },
    "cyclic_every": {
        "name": "Short Cycle", "desc": "-1 roll in the Golden Roll cycle per level (minimum 6 rolls).",
        "max_level": 4, "base_cost": 50000000, "cost_mult": 5.0, "requires": None,
    },
    "cyclic_power": {
        "name": "Strong Golden Roll", "desc": "+1 to the Golden Roll multiplier per level.",
        "max_level": 10, "base_cost": 1000000000, "cost_mult": 2.4, "requires": "cyclic_every",
        "requires_level": 3,
    },
    "diamond_roll_unlock": {
        "name": "Unlock Diamond Roll", "desc": "Unlocks the Diamond Roll cycle: every so many rolls, "
        "the next roll gets a massive luck boost.",
        "max_level": 1, "base_cost": 20000000000, "cost_mult": 1, "requires": "cyclic_power",
        "requires_level": 5,
    },
    "diamond_roll_every": {
        "name": "Diamond Short Cycle", "desc": "-3 rolls in the Diamond Roll cycle per level "
        "(minimum 70 rolls).",
        "max_level": 10, "base_cost": 40000000000, "cost_mult": 2.6, "requires": "diamond_roll_unlock",
    },
    "diamond_roll_power": {
        "name": "Strong Diamond Roll", "desc": "+5 to the Diamond Roll multiplier per level.",
        "max_level": 10, "base_cost": 80000000000, "cost_mult": 2.6, "requires": "diamond_roll_every",
        "requires_level": 5,
    },
    "rainbow_roll_unlock": {
        "name": "Unlock Rainbow Roll", "desc": "Unlocks the Rainbow Roll cycle: every so many rolls, "
        "the next roll gets an insane luck boost.",
        "max_level": 1, "base_cost": 500000000000, "cost_mult": 1, "requires": "diamond_roll_power",
        "requires_level": 5,
    },
    "rainbow_roll_every": {
        "name": "Rainbow Short Cycle", "desc": "-30 rolls in the Rainbow Roll cycle per level "
        "(minimum 700 rolls).",
        "max_level": 10, "base_cost": 1000000000000, "cost_mult": 2.8, "requires": "rainbow_roll_unlock",
    },
    "rainbow_roll_power": {
        "name": "Strong Rainbow Roll", "desc": "+25 to the Rainbow Roll multiplier per level.",
        "max_level": 10, "base_cost": 2000000000000, "cost_mult": 2.8, "requires": "rainbow_roll_every",
        "requires_level": 5,
    },
    "offline_rate": {
        "name": "Offline Earnings", "desc": L("+%g%% offline earnings per level (you start at %g%% of your "
        "money/sec while the game is closed).", OFFLINE_RATE_STEP * 100, OFFLINE_BASE_RATE * 100),
        "max_level": 15, "base_cost": 8000, "cost_mult": 1.6, "requires": None,
    },
    "offline_rate_2": {
        "name": "Offline Earnings II", "desc": L("+%g%% offline earnings per level, once Offline Earnings "
        "is maxed out.", OFFLINE_RATE_STEP * 100),
        "max_level": 5, "base_cost": 4000000000, "cost_mult": 1.9, "requires": "offline_rate",
        "requires_level": 15,
    },
    "offline_time": {
        "name": "Offline Time", "desc": L("+%d hour of max offline time per level (you start at %d hours).",
                                          OFFLINE_TIME_STEP, OFFLINE_BASE_HOURS),
        "max_level": 12, "base_cost": 15000, "cost_mult": 1.65, "requires": None,
    },
    "offline_time_2": {
        "name": "Offline Time II", "desc": L("+%d hour of max offline time per level, once Offline Time "
        "is maxed out.", OFFLINE_TIME_STEP),
        "max_level": 4, "base_cost": 8000000000, "cost_mult": 2.0, "requires": "offline_time",
        "requires_level": 12,
    },
    "auto_equip_unlock": {
        "name": "Auto Equip Best", "desc": "Unlocks the Auto Equip Best toggle in the Bag: automatically "
        "keeps your highest-earning pets equipped as you roll.",
        "max_level": 1, "base_cost": 750000, "cost_mult": 1, "requires": None,
    },
}

# BALANCE: com os Rebirths o jogo ficou fácil demais, por isso todas as upgrades passam a encarecer um pouco
# mais depressa por nível (2%: no nível 10 custam ~1.2x, no 35 ~2x, no 70 ~4x do que custavam antes).
# Para ficar mais fácil/difícil basta mexer neste número (1.0 = como antes).
UPGRADE_COST_GROWTH = 1.02
for _d in UPGRADE_DEFS.values():
    if _d["cost_mult"] != 1:
        _d["cost_mult"] = round(_d["cost_mult"] * UPGRADE_COST_GROWTH, 4)
del _d

# Categorias da Upgrade Tree (o ecrã inicial mostra uma linha por categoria, como nas Milestones).
# As que só teriam 1-2 upgrades ficam juntas em "Misc".
UPGRADE_CATEGORIES = [
    {"key": "luck", "label": "Luck", "desc": "Rarer pets show up more often",
     "upgrades": ["luck", "luck_prism", "luck_cosmic", "luck_divine"]},
    {"key": "mutations", "label": "Mutation Chance", "desc": "Golden and Diamond pets",
     "upgrades": ["golden_unlock", "golden_chance", "golden_chance_2",
                  "diamond_unlock", "diamond_chance", "diamond_chance_2"]},
    {"key": "money", "label": "Money", "desc": "Earn more money per second",
     "upgrades": ["money", "money_prism"]},
    {"key": "traits", "label": "Traits", "desc": "Trait charges and trait rarity",
     "upgrades": ["trait_charge_luck", "trait_charge_luck_2",
                  "trait_rarity_luck", "trait_rarity_luck_2"]},
    {"key": "bonus_rolls", "label": "Bonus Rolls", "desc": "Golden, Diamond and Rainbow Roll",
     "upgrades": ["cyclic_every", "cyclic_power",
                  "diamond_roll_unlock", "diamond_roll_every", "diamond_roll_power",
                  "rainbow_roll_unlock", "rainbow_roll_every", "rainbow_roll_power"]},
    {"key": "auto", "label": "Auto Roller", "desc": "Rolls by itself, faster",
     "upgrades": ["auto_unlock", "auto_speed", "auto_turbo"]},
    {"key": "offline", "label": "Offline", "desc": "Earn more while the game is closed",
     "upgrades": ["offline_rate", "offline_rate_2", "offline_time", "offline_time_2"]},
    {"key": "misc", "label": "Misc", "desc": "Equip Slots and Auto Equip Best",
     "upgrades": ["slots", "slots_plus", "auto_equip_unlock"]},
]
UPGRADE_CAT_BY_KEY = {c["key"]: c for c in UPGRADE_CATEGORIES}
UPGRADE_ORDER = [k for c in UPGRADE_CATEGORIES for k in c["upgrades"]]
# segurança: toda a upgrade tem de estar numa (e só numa) categoria
assert sorted(UPGRADE_ORDER) == sorted(UPGRADE_DEFS), "Há upgrades fora das categorias da Tree"

BASE_SLOTS = 3
AUTO_BASE_RPS = 0.34          # rolls por segundo no nível 0 do auto

# ----------------------------------------------------------------------------------
# GOLDEN / DIAMOND / RAINBOW ROLL (sorte cíclica)
# ----------------------------------------------------------------------------------
# A cada N rolls (sem contar a roll de bónus), a próxima roll vem com um multiplicador
# extra de sorte nas raridades Raras ou melhores. Há 3 ciclos independentes, cada um
# desbloqueado/melhorado na Upgrade Tree:
#   Golden Roll   -> ativo desde o início, melhorado por "cyclic_every" / "cyclic_power"
#   Diamond Roll  -> desbloqueado por "diamond_roll_unlock", melhorado pelos "diamond_roll_*"
#   Rainbow Roll  -> desbloqueado por "rainbow_roll_unlock", melhorado pelos "rainbow_roll_*"
# Se mais do que um ciclo estiver pronto ao mesmo tempo, os multiplicadores multiplicam-se.
CYCLIC_LUCK_EVERY = 10
CYCLIC_LUCK_MIN = 6
CYCLIC_LUCK_MULT = 10.0

DIAMOND_ROLL_EVERY_BASE = 100
DIAMOND_ROLL_EVERY_MIN = 70
DIAMOND_ROLL_EVERY_STEP = 3
DIAMOND_ROLL_MULT_BASE = 50.0
DIAMOND_ROLL_MULT_STEP = 5.0

RAINBOW_ROLL_EVERY_BASE = 1000
RAINBOW_ROLL_EVERY_MIN = 700
RAINBOW_ROLL_EVERY_STEP = 30
RAINBOW_ROLL_MULT_BASE = 250.0
RAINBOW_ROLL_MULT_STEP = 25.0


class UpgradeMixin:
    """Upgrades: níveis, custos, compra e os ciclos de Golden / Diamond / Rainbow Roll."""

    # ---------------- upgrades ----------------
    def upgrade_level(self, key):
        return self.upgrades.get(key, 0)

    def upgrade_cost_at(self, key, lvl):
        """Custo de comprar o nível a seguir a 'lvl' (ou seja, de passar de lvl para lvl+1)."""
        d = UPGRADE_DEFS[key]
        if d["cost_mult"] == 1:
            return d["base_cost"]
        return int(d["base_cost"] * (d["cost_mult"] ** lvl))

    def upgrade_cost(self, key):
        return self.upgrade_cost_at(key, self.upgrade_level(key))

    def upgrade_locked_by(self, key):
        """Devolve a chave do requisito em falta, ou None."""
        d = UPGRADE_DEFS[key]
        req = d.get("requires")
        if req is None:
            return None
        need = d.get("requires_level", 1)
        if self.upgrade_level(req) < need:
            return req
        return None

    def upgrade_available(self, key):
        d = UPGRADE_DEFS[key]
        if self.upgrade_level(key) >= d["max_level"]:
            return False
        return self.upgrade_locked_by(key) is None

    def upgrade_affordable(self, key):
        """True se a upgrade dá para comprar AGORA: desbloqueada, não está no máximo e há moedas para o próximo nível."""
        return self.upgrade_available(key) and self.coins >= self.upgrade_cost(key)

    def affordable_upgrades_count(self):
        """Quantas upgrades diferentes dá para comprar neste momento (número da bolinha vermelha no botão UPGRADES).
        Só conta as que já estão desbloqueadas; as que só abrem depois de comprar outra não entram."""
        return sum(1 for key in UPGRADE_DEFS if self.upgrade_affordable(key))

    def upgrade_bulk_quote(self, key, mode):
        """Quanto custa comprar vários níveis de uma vez. mode = 1, 10 ou "max".
        Devolve (níveis, custo_total, dá_para_pagar).
          - 1 / 10: compra exatamente esses níveis (menos, se já só faltarem alguns para o máximo);
            se não der para pagar tudo, dá_para_pagar = False (o botão fica desativado).
          - "max": compra quantos níveis der para pagar com as moedas de agora; se nem 1 nível
            der, devolve o preço do próximo nível com dá_para_pagar = False."""
        d = UPGRADE_DEFS[key]
        lvl = self.upgrade_level(key)
        remaining = d["max_level"] - lvl
        if remaining <= 0:
            return 0, 0, False
        if mode == "max":
            n, total = 0, 0
            while n < remaining:
                c = self.upgrade_cost_at(key, lvl + n)
                if total + c > self.coins:
                    break
                total += c
                n += 1
            if n == 0:
                return 1, self.upgrade_cost_at(key, lvl), False
            return n, total, True
        n = min(int(mode), remaining)
        total = sum(self.upgrade_cost_at(key, lvl + i) for i in range(n))
        return n, total, self.coins >= total

    def buy_upgrade_bulk(self, key, mode):
        """Compra 1, 10 ou o máximo possível de níveis (ver upgrade_bulk_quote).
        Devolve quantos níveis comprou (0 = não comprou nada)."""
        if not self.upgrade_available(key):
            return 0
        n, total, can_pay = self.upgrade_bulk_quote(key, mode)
        if not can_pay or n <= 0:
            return 0
        self.coins -= total
        self.upgrades[key] += n
        if key == "auto_equip_unlock":      # comprado na Tree: fica ligado e já equipa o melhor
            self.auto_equip_best_on = True
            self.equip_best()
        return n

    def buy_upgrade(self, key):
        return self.buy_upgrade_bulk(key, 1) > 0

    # ---------------- Golden / Diamond / Rainbow Roll ----------------
    def golden_roll_every(self):
        return max(CYCLIC_LUCK_MIN, CYCLIC_LUCK_EVERY - self.upgrade_level("cyclic_every"))

    def golden_roll_mult(self):
        return CYCLIC_LUCK_MULT + 1.0 * self.upgrade_level("cyclic_power")

    def rolls_until_golden_roll(self):
        if self.cyclic_bonus_ready:
            return 0
        return max(0, self.golden_roll_every() - self.cyclic_roll_count)

    def diamond_roll_unlocked(self):
        return self.upgrade_level("diamond_roll_unlock") >= 1

    def diamond_roll_every(self):
        return max(DIAMOND_ROLL_EVERY_MIN,
                   DIAMOND_ROLL_EVERY_BASE - DIAMOND_ROLL_EVERY_STEP * self.upgrade_level("diamond_roll_every"))

    def diamond_roll_mult(self):
        return DIAMOND_ROLL_MULT_BASE + DIAMOND_ROLL_MULT_STEP * self.upgrade_level("diamond_roll_power")

    def rolls_until_diamond_roll(self):
        if not self.diamond_roll_unlocked():
            return None
        if self.diamond_bonus_ready:
            return 0
        return max(0, self.diamond_roll_every() - self.diamond_roll_count)

    def rainbow_roll_unlocked(self):
        return self.upgrade_level("rainbow_roll_unlock") >= 1

    def rainbow_roll_every(self):
        return max(RAINBOW_ROLL_EVERY_MIN,
                   RAINBOW_ROLL_EVERY_BASE - RAINBOW_ROLL_EVERY_STEP * self.upgrade_level("rainbow_roll_every"))

    def rainbow_roll_mult(self):
        return RAINBOW_ROLL_MULT_BASE + RAINBOW_ROLL_MULT_STEP * self.upgrade_level("rainbow_roll_power")

    def rolls_until_rainbow_roll(self):
        if not self.rainbow_roll_unlocked():
            return None
        if self.rainbow_bonus_ready:
            return 0
        return max(0, self.rainbow_roll_every() - self.rainbow_roll_count)

    # ---------------- auto equip best ----------------
    # ---------------- pausar os ciclos ----------------
    def is_cycle_paused(self, kind):
        """kind = "golden" | "diamond" | "rainbow". Pausado = congelado: não conta rolls e não dispara."""
        return bool(self.cycle_paused.get(kind, False))

    def toggle_cycle_pause(self, kind):
        if kind in self.cycle_paused:
            self.cycle_paused[kind] = not self.cycle_paused[kind]

    def auto_equip_unlocked(self):
        return self.upgrade_level("auto_equip_unlock") >= 1
