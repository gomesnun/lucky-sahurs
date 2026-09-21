"""Milestones: tabela de metas e recompensas, e a lógica que as calcula."""

from core.pets import INDEX_ENTRIES


# Cada categoria tem uma lista de (limite, valor_do_bónus). A recompensa é um bónus
# PERMANENTE numa stat (sorte, sorte de traits, sorte dourada, sorte de diamante,
# dinheiro %, velocidade do auto), que fica sempre ativo depois de resgatado.
# Cada categoria dá sempre o mesmo tipo de bónus.
MILESTONE_REWARD_LABELS = {
    "luck": "Luck", "trait_luck": "Trait Luck",
    "golden_luck": "Golden Luck", "diamond_luck": "Diamond Luck",
    "money": "Money", "auto_speed": "Auto Speed",
    "secret_luck": "Secret Luck", "divine_luck": "Divine Luck",
    "cosmic_luck": "Cosmic Luck", "transcendent_luck": "Transcendent Luck",
    "index_luck": "Luck",
}


def _tiers(thresholds, values):
    return list(zip(thresholds, values))


MILESTONE_DEFS = {
    "rolls": {
        "label": "Total Rolls", "reward_type": "luck",
        "tiers": _tiers(
            [50, 150, 400, 1000, 2500, 6000, 15000, 40000, 100000, 250000, 600000,
             1500000, 4000000, 10000000, 25000000, 50000000, 100000000],
            [.02, .03, .04, .05, .06, .07, .08, .10, .12, .14, .16,
             .18, .20, .23, .26, .30, .35]),
    },
    "traits_rolled": {
        "label": "Traits Rolled", "reward_type": "trait_luck",
        "tiers": _tiers(
            [5, 15, 30, 60, 120, 250, 500, 1000, 2000, 4000, 8000, 15000, 30000, 60000, 120000],
            [.03, .05, .07, .10, .13, .17, .22, .28, .35, .43, .52, .62, .73, .85, 1.00]),
    },
    "golden_rolled": {
        "label": "Golden Pets Rolled", "reward_type": "golden_luck",
        "tiers": _tiers(
            [5, 15, 35, 75, 150, 300, 600, 1200, 2500, 5000, 10000, 20000, 40000, 80000, 150000],
            [.01, .01, .01, .02, .02, .03, .03, .04, .04, .05, .05, .06, .07, .08, .08]),
    },
    "diamond_rolled": {
        "label": "Diamond Pets Rolled", "reward_type": "diamond_luck",
        "tiers": _tiers(
            [2, 6, 15, 35, 75, 150, 300, 600, 1200, 2500, 5000, 10000, 20000, 40000, 80000],
            [.01, .01, .01, .02, .02, .03, .03, .04, .04, .05, .05, .06, .07, .08, .08]),
    },
    "secret_rolled": {
        "label": "Secret Pets Rolled", "reward_type": "secret_luck",     # vale para Secret e melhores
        "tiers": _tiers(
            [1, 3, 5, 10, 25, 50, 100, 250, 500, 1000, 2500, 5000, 10000],
            [.02, .03, .04, .05, .07, .09, .12, .15, .19, .24, .30, .38, .50]),
    },
    "divine_rolled": {
        "label": "Divine Pets Rolled", "reward_type": "divine_luck",     # vale para Divine e melhores
        "tiers": _tiers(
            [1, 2, 3, 5, 10, 20, 40, 80, 150, 300, 600, 1200, 2500, 5000],
            [.03, .04, .05, .06, .08, .10, .12, .14, .17, .20, .24, .29, .35, .42]),
    },
    "cosmic_rolled": {
        "label": "Cosmic Pets Rolled", "reward_type": "cosmic_luck",     # vale para Cosmic e Transcendent
        "tiers": _tiers(
            [1, 2, 3, 5, 10, 20, 40, 80, 150, 300, 600, 1200],
            [.04, .05, .06, .08, .10, .12, .15, .18, .22, .26, .30, .35]),
    },
    "transcendent_rolled": {
        "label": "Transcendent Pets Rolled", "reward_type": "transcendent_luck",   # só para Transcendent
        "tiers": _tiers(
            [1, 2, 3, 5, 10, 20, 40, 80, 150, 300],
            [.05, .06, .08, .10, .12, .15, .19, .24, .30, .40]),
    },
    "indexed_pets": {
        # conta cartas DIFERENTES do Index (pet + mutação); ter 1 milhão do mesmo pet conta só 1
        "label": "Indexed Pets", "reward_type": "index_luck",     # sorte extra para todas as raridades Rare+
        "tiers": _tiers(
            [2, 4, 7, 10, 14, 19, 25, 32, 40, 49, 57, INDEX_ENTRIES],
            [.01, .02, .02, .03, .03, .04, .05, .06, .07, .08, .09, .10]),
    },
    "coins": {
        "label": "Total Coins Earned", "reward_type": "money",
        "tiers": _tiers(
            [1e3, 5e3, 2.5e4, 1e5, 5e5, 2.5e6, 1e7, 5e7, 2.5e8, 1e9, 5e9, 2.5e10, 1e11, 5e11, 1e12],
            [.03, .04, .06, .08, .10, .13, .16, .20, .25, .30, .37, .45, .55, .70, .90]),
    },
    "playtime": {
        "label": "Playtime", "reward_type": "auto_speed",
        "tiers": _tiers(
            [300, 900, 1800, 3600, 7200, 14400, 28800, 57600, 86400, 172800, 345600,
             604800, 1209600, 2592000],
            [.03, .05, .07, .09, .12, .15, .19, .24, .30, .38, .48, .60, .75, 1.00]),
    },
}
MILESTONE_CAT_ORDER = ["rolls", "traits_rolled", "golden_rolled", "diamond_rolled",
                       "secret_rolled", "divine_rolled", "cosmic_rolled", "transcendent_rolled",
                       "indexed_pets", "coins", "playtime"]


class MilestoneMixin:
    """Milestones: métricas, bónus e atribuição das recompensas."""

    def milestone_metric(self, category):
        if category == "indexed_pets":
            return self.indexed_pets_count()
        if category == "secret_rolled":
            return self.total_secret_rolled
        if category == "divine_rolled":
            return self.total_divine_rolled
        if category == "cosmic_rolled":
            return self.total_cosmic_rolled
        if category == "transcendent_rolled":
            return self.total_transcendent_rolled
        if category == "rolls":
            return self.total_rolls
        if category == "traits_rolled":
            return self.total_traits_rolled
        if category == "golden_rolled":
            return self.total_golden_rolled
        if category == "diamond_rolled":
            return self.total_diamond_rolled
        if category == "coins":
            return self.total_coins_earned
        if category == "playtime":
            return self.playtime
        return 0

    def recalc_milestone_bonus(self):
        """Recalcula (e guarda em cache) o bónus total de cada tipo de recompensa."""
        totals = {}
        for category, d in MILESTONE_DEFS.items():
            rt = d["reward_type"]
            for i, (_threshold, value) in enumerate(d["tiers"]):
                if ("%s:%d" % (category, i)) in self.milestones_claimed:
                    totals[rt] = totals.get(rt, 0.0) + value
        self._ms_bonus = totals

    def milestone_bonus(self, reward_type):
        """Bónus (percentagem, ex: 0.05 = +5%) de todas as metas já resgatadas que
        dão este tipo de recompensa. É permanente."""
        return self._ms_bonus.get(reward_type, 0.0)

    def check_milestones(self):
        """Atribui automaticamente qualquer meta recém-atingida (fica logo com o
        bónus permanente). Devolve a lista das que acabaram de ser resgatadas
        (categoria, índice, limite, valor_do_bónus)."""
        newly = []
        for category, d in MILESTONE_DEFS.items():
            metric = self.milestone_metric(category)
            for i, (threshold, value) in enumerate(d["tiers"]):
                mkey = "%s:%d" % (category, i)
                if mkey not in self.milestones_claimed and metric >= threshold:
                    self.milestones_claimed.add(mkey)
                    newly.append((category, i, threshold, value))
        if newly:
            self.recalc_milestone_bonus()
        return newly
