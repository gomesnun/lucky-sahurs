"""Economia: dinheiro por segundo, multiplicadores e auto-roll."""

from core.pets import MUTATIONS, RARITIES
from core.upgrades import AUTO_BASE_RPS, BASE_SLOTS


class EconomyMixin:
    """Dinheiro: multiplicadores, rendimento dos pets equipados e velocidade do auto-roll."""

    def max_slots(self):
        return (BASE_SLOTS + self.upgrade_level("slots") + self.upgrade_level("slots_plus")
                + int(self.rebirth_bonus("slots")))

    def money_multiplier(self):
        base = (1.0 + 0.10 * self.upgrade_level("money")) * (1.0 + 0.25 * self.upgrade_level("money_prism"))
        return (base * (1.0 + self.trait_buff("money")) * (1.0 + self.milestone_bonus("money"))
                * self.rebirth_money_mult() * (1.0 + self.rebirth_bonus("money")))

    def auto_unlocked(self):
        return self.upgrade_level("auto_unlock") >= 1

    def auto_rolls_per_second(self):
        if not self.auto_unlocked():
            return 0.0
        base = AUTO_BASE_RPS * (1 + 0.40 * self.upgrade_level("auto_speed"))
        base *= (1.0 + 0.50 * self.upgrade_level("auto_turbo"))
        return (base * (1.0 + self.trait_buff("auto_speed")) * (1.0 + self.milestone_bonus("auto_speed"))
                * (1.0 + self.rebirth_bonus("auto_speed")))

    def pet_income(self, rarity_index, mutation):
        return RARITIES[rarity_index]["income"] * MUTATIONS[mutation]["mult"] * self.money_multiplier()

    def income_per_second(self):
        total = 0.0
        for rarity_index, mutation in self.equipped:
            total += RARITIES[rarity_index]["income"] * MUTATIONS[mutation]["mult"]
        return total * self.money_multiplier()