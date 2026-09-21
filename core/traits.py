"""Traits: tabela de traits e lógica de cargas / roll / equipar."""

import random

from theme import BLACK, WHITE


# Cada roll (manual ou do Auto Roller) tem uma chance-base de 1/TRAIT_CHARGE_ONE_IN de
# dar uma "carga" de trait. Essa carga gasta-se na página de Traits para rolar 1 trait,
# que fica na tua coleção (Index de Traits) mas só a trait EQUIPADA é que dá os buffs.
# Só a última trait (a mais rara) dá todos os tipos de buff ao mesmo tempo.
# ("mutation" = mais chance de Golden e de Diamond. A melhor trait dá +25%: ver as contas em core/pets.py.)
TRAIT_CHARGE_ONE_IN = 250

TRAITS = [
    {"name": "Lucky",           "color": (110, 130, 145), "text": WHITE, "one_in": 3,
     "buffs": {"money": 0.05}},
    {"name": "Sharp-Eyed",         "color": (60, 150, 195),  "text": WHITE, "one_in": 8,
     "buffs": {"money": 0.08, "luck": 0.06}},
    {"name": "Instinctive",        "color": (55, 115, 225),  "text": WHITE, "one_in": 20,
     "buffs": {"money": 0.12, "luck": 0.10, "charge_chance": 0.15}},
    {"name": "Visionary",        "color": (110, 75, 220),  "text": WHITE, "one_in": 60,
     "buffs": {"money": 0.16, "luck": 0.14, "charge_chance": 0.22, "mutation": 0.02}},
    {"name": "Prodigious",        "color": (165, 55, 210),  "text": WHITE, "one_in": 200,
     "buffs": {"money": 0.22, "luck": 0.18, "charge_chance": 0.30, "mutation": 0.03, "auto_speed": 0.15}},
    {"name": "Radiant",          "color": (220, 75, 160),  "text": WHITE, "one_in": 800,
     "buffs": {"money": 0.30, "luck": 0.24, "charge_chance": 0.40, "mutation": 0.04, "auto_speed": 0.22}},
    {"name": "Ascendant",        "color": (200, 60, 45),   "text": WHITE, "one_in": 3000,
     "buffs": {"money": 0.40, "luck": 0.30, "charge_chance": 0.50, "mutation": 0.06, "auto_speed": 0.30,
               "secret_luck": 0.25}},
    {"name": "Stellar",          "color": (230, 150, 40),  "text": BLACK, "one_in": 12000,
     "buffs": {"money": 0.55, "luck": 0.38, "charge_chance": 0.65, "mutation": 0.08, "auto_speed": 0.42,
               "secret_luck": 0.45}},
    {"name": "Absolute",          "color": (240, 205, 60),  "text": BLACK, "one_in": 60000,
     "buffs": {"money": 0.75, "luck": 0.50, "charge_chance": 0.85, "mutation": 0.12, "auto_speed": 0.60,
               "secret_luck": 0.70}},
    {"name": "Luck Deity", "color": (250, 250, 250), "text": (90, 20, 55), "one_in": 300000,
     "buffs": {"money": 1.00, "luck": 0.70, "charge_chance": 1.10, "mutation": 0.16, "auto_speed": 0.85,
               "secret_luck": 1.20}},
    {"name": "Immortal", "color": (40, 215, 195), "text": BLACK, "one_in": 1500000,
     "buffs": {"money": 1.35, "luck": 0.90, "charge_chance": 1.40, "mutation": 0.20, "auto_speed": 1.10,
               "secret_luck": 1.70}},
    {"name": "Omnipotent", "color": (22, 20, 34), "text": (255, 214, 90), "one_in": 8000000,
     "buffs": {"money": 1.75, "luck": 1.10, "charge_chance": 1.75, "mutation": 0.25, "auto_speed": 1.40,
               "secret_luck": 2.30}},
]


class TraitMixin:
    """Traits: cargas, roll de traits e trait equipada."""

    # ---------------- traits ----------------
    def trait_buff(self, key):
        """Só a trait EQUIPADA dá o seu buff; traits só na coleção não fazem nada."""
        if self.equipped_trait is None:
            return 0.0
        return TRAITS[self.equipped_trait]["buffs"].get(key, 0.0)

    def trait_charge_chance(self):
        chance = (1.0 / TRAIT_CHARGE_ONE_IN) * (1.0 + self.trait_buff("charge_chance"))
        chance *= (1.0 + 0.06 * self.upgrade_level("trait_charge_luck"))
        chance *= (1.0 + 0.10 * self.upgrade_level("trait_charge_luck_2"))
        chance *= (1.0 + self.rebirth_bonus("charge_chance"))
        return min(1.0, chance)

    def trait_roll_weights(self):
        rarity_mult = ((1.0 + 0.09 * self.upgrade_level("trait_rarity_luck")) *
                       (1.0 + 0.15 * self.upgrade_level("trait_rarity_luck_2")) *
                       (1.0 + self.milestone_bonus("trait_luck")) *
                       (1.0 + self.rebirth_bonus("trait_luck")))
        weights = []
        for i, t in enumerate(TRAITS):
            w = 1.0 / t["one_in"]
            if i >= 2:   # Instintivo (3ª trait) em diante
                w *= rarity_mult
            weights.append(w)
        return weights

    def trait_chance(self, trait_index):
        weights = self.trait_roll_weights()
        total = sum(weights)
        return weights[trait_index] / total if total else 0.0

    def roll_trait(self):
        """Consome 1 carga e dá uma trait aleatória (mais rara == mais difícil)."""
        if self.trait_charges <= 0:
            return None
        self.trait_charges -= 1
        weights = self.trait_roll_weights()
        total = sum(weights)
        roll_val = random.random() * total
        cum = 0.0
        idx = len(TRAITS) - 1
        for i, w in enumerate(weights):
            cum += w
            if roll_val <= cum:
                idx = i
                break
        self.owned_traits.add(idx)
        self.last_trait_roll = idx
        self.total_traits_rolled += 1
        self.daily_add_progress("traits")
        return idx

    def equip_trait(self, idx):
        if idx not in self.owned_traits:
            return False
        self.equipped_trait = None if self.equipped_trait == idx else idx
        return True