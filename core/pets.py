"""Pets, raridades e mutações: dados, chances base e lógica de roll / equipar."""

import random

from core import balance as B
from theme import BLACK, DIAMOND_BORDER, GOLD_BORDER, WHITE


# Uma RARIDADE (tier) agrupa 1 ou 2 pets. Cores, rendimento e chance são da raridade; a chance
# da raridade reparte-se pelos pets que ela tem, mas NÃO por igual: o 2.º pet de cada raridade é
# mais raro do que o 1.º (ver SECOND_PET_SHARE).
RARITY_TIERS = [
    {"key": "comum",    "name": "Common",    "color": (235, 235, 235), "color2": None,          "text": BLACK,        "one_in": 2,        "income": 1},
    {"key": "incomum",  "name": "Uncommon",  "color": (60, 190, 90),   "color2": None,          "text": BLACK,        "one_in": 4,        "income": 4},
    {"key": "raro",     "name": "Rare",      "color": (45, 120, 230),  "color2": None,          "text": WHITE,        "one_in": 15,       "income": 15},
    {"key": "epico",    "name": "Epic",      "color": (150, 60, 220),  "color2": None,          "text": WHITE,        "one_in": 80,       "income": 60},
    {"key": "lendario", "name": "Legendary", "color": (230, 195, 35),  "color2": None,          "text": BLACK,        "one_in": 800,      "income": 300},
    {"key": "mitico",   "name": "Mythic",    "color": (220, 45, 45),   "color2": None,          "text": WHITE,        "one_in": 10000,    "income": 1800},
    {"key": "exotico",  "name": "Exotic",    "color": (230, 140, 30),  "color2": (60, 190, 90), "text": WHITE,        "one_in": 150000,   "income": 12000},
    {"key": "secreto",  "name": "Secret",    "color": (14, 14, 16),    "color2": (245, 245, 245), "text": WHITE,      "one_in": 2500000,  "income": 90000},
    {"key": "divino",   "name": "Divine",    "color": (240, 145, 190), "color2": (255, 255, 255), "text": (80, 18, 50), "one_in": 50000000, "income": 750000},
    # nebulosa violeta-escura com estrelinhas (ver draw_rarity_bg)
    {"key": "cosmico",  "name": "Cosmic",    "color": (96, 58, 224),   "color2": (14, 8, 48),  "text": WHITE,        "one_in": 3000000000,   "income": 8000000},
    # arco-íris pastel iridescente (ver draw_rarity_bg)
    {"key": "transcendente", "name": "Transcendent", "color": (255, 226, 120), "color2": (120, 235, 255), "text": BLACK, "one_in": 500000000000, "income": 90000000},
]

# Lista PLANA dos pets. ATENÇÃO: os saves guardam o ÍNDICE do pet nesta lista (chaves
# "indice_mutacao" e o equipamento), por isso NUNCA reordenes nem removas pets - só acrescenta no fim.
PET_DEFS = [
    ("comum", "Trippi Troppi"),
    ("incomum", "Bombombini Gusini"),
    ("raro", "Frigo Camelo"),
    ("epico", "Chimpanzini Bananini"),
    ("lendario", "Ballerina Cappuccina"),
    ("mitico", "Bombardiro Crocodilo"),
    ("exotico", "Tralalero Tralala"),
    ("secreto", "Saturno Saturnita"),
    ("divino", "Tung Tung Tung Sahur"),
    # 2.º pet de cada raridade
    ("comum", "Lirili Larila"),
    ("incomum", "Cappuccino Assassino"),
    ("raro", "Brr Brr Patapim"),
    ("epico", "Glorbo Fruttodrillo"),
    ("lendario", "Trulimero Trulicina"),
    ("mitico", "Brri Brri Bicus Dicus Bombicus"),
    ("exotico", "Orcalero Orcala"),
    ("secreto", "Graipuss Medussi"),
    ("divino", "La Vacca Saturno Saturnita"),
    # raridades novas: Cosmic e Transcendent têm 2 pets cada
    ("cosmico", "Girafa Celestre"),
    ("transcendente", "Dragon Cannelloni"),
    ("cosmico", "Tralaledon"),          # 2.º Cosmic (acrescentado no fim para não mexer nos índices dos saves)
    ("transcendente", "Ganganzelli Trulala"),   # 2.º Transcendent (idem: no fim, para não mexer nos índices dos saves)
]

TIER_INDEX = {_t["key"]: _i for _i, _t in enumerate(RARITY_TIERS)}
TIER_SECRET = TIER_INDEX["secreto"]
TIER_DIVINE = TIER_INDEX["divino"]
TIER_COSMIC = TIER_INDEX["cosmico"]
TIER_TRANSCENDENT = TIER_INDEX["transcendente"]

# RARITIES = um dicionário por PET (com os dados da raridade dele + "pet", "tier", "n_pets" e "share").
# É indexado pelo índice do pet, que é o que os saves guardam.
RARITIES = []
for _key, _pet in PET_DEFS:
    _d = dict(RARITY_TIERS[TIER_INDEX[_key]])
    _d["pet"] = _pet
    _d["tier"] = TIER_INDEX[_key]
    RARITIES.append(_d)
# Com 2 pets na mesma raridade, o 2.º (o que vem mais tarde na lista) fica com esta fração da chance
# da raridade e o 1.º com o resto: 1/3 e 2/3 => o 2.º sai 2x menos vezes do que o 1.º.
# Raridades com 1 pet só ficam com a chance toda. A soma dos pets de uma raridade dá sempre 100%.
SECOND_PET_SHARE = 1.0 / 3.0
# Por ser mais raro, o 2.º pet também rende mais dinheiro (mas não chega ao dobro).
SECOND_PET_INCOME_MULT = 1.5
for _i, _d in enumerate(RARITIES):
    _same = [_j for _j, _e in enumerate(RARITIES) if _e["tier"] == _d["tier"]]
    _d["n_pets"] = len(_same)
    if len(_same) == 2:
        _d["share"] = SECOND_PET_SHARE if _i == _same[1] else 1.0 - SECOND_PET_SHARE
        if _i == _same[1]:
            _d["income"] = _d["income"] * SECOND_PET_INCOME_MULT
    else:
        _d["share"] = 1.0 / len(_same)
PET_ORDER = sorted(range(len(RARITIES)), key=lambda _i: (RARITIES[_i]["tier"], _i))   # ordem de exibição
TIER_FIRST_PET = [min(_i for _i, _r in enumerate(RARITIES) if _r["tier"] == _t)
                  for _t in range(len(RARITY_TIERS))]
del _key, _pet, _d, _i, _same

MUTATIONS = {
    "normal":  {"label": "",         "mult": 1, "border": None},
    "golden":  {"label": "Golden",  "mult": 3, "border": GOLD_BORDER},
    "diamond": {"label": "Diamond", "mult": 9, "border": DIAMOND_BORDER},
}
MUT_ORDER = ["normal", "golden", "diamond"]
INDEX_ENTRIES = len(RARITIES) * len(MUT_ORDER)      # cartas do Index: cada pet em Normal, Golden e Diamond

# Chance máxima REAL de cada mutação. Com estes tetos a versão normal é sempre a mais
# comum (com 25% de dourado e 9% de diamante, o normal nunca desce dos ~68%).
GOLDEN_MAX_CHANCE = 0.25
DIAMOND_MAX_CHANCE = 0.09

# Chance-base vinda das upgrades (ANTES dos multiplicadores da trait equipada, das metas e das recompensas
# de Rebirth). Tudo está calibrado para que, com TUDO no máximo (todas as upgrades + todas as metas +
# todas as recompensas de Rebirth + a melhor trait), a chance seja exatamente o teto acima. Assim nunca há
# upgrades "a mais": qualquer nível que compres continua a subir a chance, mesmo com o resto no topo.
# A melhor trait (Omnipotent) dá +25% de mutação, as metas Golden/Diamond dão +60% no total e as recompensas
# de Rebirth +25% no total; se mudares algum destes valores, refaz estas contas.
#   Dourado:  0.10  (upgrades) x 1.25 (Omnipotent) x 1.60 (metas) x 1.25 (Rebirth) = 0.25
#   Diamante: 0.036 (upgrades) x 1.25 x 1.60 x 1.25 = 0.09
GOLDEN_BASE = 0.02         # ao desbloquear
GOLDEN_STEP_1 = 0.0025     # Golden Luck     (20 níveis -> +5%)
GOLDEN_STEP_2 = 0.002      # Golden Luck II  (15 níveis -> +3%)
DIAMOND_BASE = 0.0025      # ao desbloquear
DIAMOND_STEP_1 = 0.0005    # Diamond Luck    (22 níveis -> +1.1%)
DIAMOND_STEP_2 = 0.0015    # Diamond Luck II (15 níveis -> +2.25%)


def base_pet_chance(rarity_index, mutation):
    """Chance-BASE (1 em X) deste pet por roll, como se tivesses acabado de começar: sem luck de
    upgrades, trait ou metas, e com Golden/Diamond só desbloqueados (nenhum nível de Golden Luck /
    Diamond Luck). É o valor que o Index mostra entre parênteses."""
    weights = [_r["share"] / _r["one_in"] for _r in RARITIES]
    total = sum(weights)
    rarity_chance = weights[rarity_index] / total if total else 0.0
    if mutation == "diamond":
        factor = DIAMOND_BASE
    elif mutation == "golden":
        factor = (1.0 - DIAMOND_BASE) * GOLDEN_BASE
    else:
        factor = (1.0 - DIAMOND_BASE) * (1.0 - GOLDEN_BASE)
    return rarity_chance * factor


def cap_chance(raw, cap):
    """Limita a chance ao teto. Como os valores das upgrades/metas/traits estão calibrados
    para a soma máxima dar exatamente o teto, isto só serve de segurança."""
    return max(0.0, min(cap, raw))


class PetMixin:
    """Pets: chances, sorte, roll, coleção (owned) e equipar / desequipar."""

    def equip_best(self):
        """Substitui todo o equipamento pelos pets que mais dinheiro dão (ganancioso:
        enche os slots pelos de maior rendimento para baixo, respeitando quantos tens)."""
        options = []
        for r_idx in range(len(RARITIES)):
            for mutation in MUT_ORDER:
                owned = self.count_owned(r_idx, mutation)
                if owned > 0:
                    options.append((self.pet_income(r_idx, mutation), r_idx, mutation, owned))
        options.sort(key=lambda t: -t[0])
        slots = self.max_slots()
        new_equipped = []
        for _income, r_idx, mutation, owned in options:
            while owned > 0 and len(new_equipped) < slots:
                new_equipped.append([r_idx, mutation])
                owned -= 1
            if len(new_equipped) >= slots:
                break
        self.equipped = new_equipped

    # ---------------- probabilidades ----------------
    def luck_multiplier(self, rarity_index, bonus_mult=1.0):
        """Multiplicador de peso aplicado a cada pet pelos upgrades de sorte + trait + metas.
        Depende da RARIDADE do pet (tier), não do pet em si."""
        tier = RARITIES[rarity_index]["tier"]
        if tier < 2:
            return 1.0
        m = 1.0 + B.LUCK_PER_LEVEL * self.upgrade_level("luck")
        prism = self.upgrade_level("luck_prism")
        if tier >= 3:
            m *= (1.0 + B.LUCK_PRISM_PER_LEVEL * prism)
        if tier >= 5:
            m *= (1.0 + B.LUCK_PRISM_PER_LEVEL * prism)
        if tier >= TIER_SECRET:
            m *= (1.0 + B.LUCK_PRISM_PER_LEVEL * prism)
        if tier >= 6:
            m *= (1.0 + B.LUCK_COSMIC_PER_LEVEL * self.upgrade_level("luck_cosmic"))       # upgrade "Exotic Luck"
        if tier >= TIER_DIVINE:
            m *= (1.0 + B.LUCK_DIVINE_PER_LEVEL * self.upgrade_level("luck_divine"))
        m *= (1.0 + self.trait_buff("luck"))
        if tier >= TIER_SECRET:   # Secret e melhores também ganham a sorte especial da trait
            m *= (1.0 + self.trait_buff("secret_luck"))
        m *= (1.0 + self.milestone_bonus("luck"))
        m *= (1.0 + self.milestone_bonus("index_luck"))      # meta "Indexed Pets"
        if tier >= TIER_SECRET:        # Secret e melhores
            m *= (1.0 + self.milestone_bonus("secret_luck"))
        if tier >= TIER_DIVINE:        # Divine e melhores
            m *= (1.0 + self.milestone_bonus("divine_luck"))
        if tier >= TIER_COSMIC:        # Cosmic e Transcendent
            m *= (1.0 + self.milestone_bonus("cosmic_luck"))
        if tier >= TIER_TRANSCENDENT:  # só Transcendent
            m *= (1.0 + self.milestone_bonus("transcendent_luck"))
        m *= self.rebirth_luck_mult()
        m *= (1.0 + self.rebirth_bonus("luck"))                # recompensas da página de Rebirth
        if tier >= TIER_SECRET:
            m *= (1.0 + self.rebirth_bonus("secret_luck"))
        if bonus_mult != 1.0:
            m *= bonus_mult
        return m

    def roll_weights(self, with_luck=True, bonus_mult=1.0):
        weights = []
        for i, r in enumerate(RARITIES):
            w = r["share"] / r["one_in"]     # a chance da raridade reparte-se pelos pets dela (o 2.º é mais raro)
            if with_luck:
                w *= self.luck_multiplier(i, bonus_mult=bonus_mult)
            weights.append(w)
        return weights

    def roll_chance(self, rarity_index, with_luck=True):
        weights = self.roll_weights(with_luck)
        total = sum(weights)
        return weights[rarity_index] / total if total else 0.0

    def mutation_chances(self):
        g = 0.0
        d = 0.0
        base_mult = 1.0 + self.trait_buff("mutation")
        golden_mult = (base_mult * (1.0 + self.milestone_bonus("golden_luck"))
                       * (1.0 + self.rebirth_bonus("golden_luck")))
        diamond_mult = (base_mult * (1.0 + self.milestone_bonus("diamond_luck"))
                        * (1.0 + self.rebirth_bonus("diamond_luck")))
        if self.upgrade_level("golden_unlock") >= 1:
            g = (GOLDEN_BASE + GOLDEN_STEP_1 * self.upgrade_level("golden_chance")
                 + GOLDEN_STEP_2 * self.upgrade_level("golden_chance_2")) * golden_mult
        if self.upgrade_level("diamond_unlock") >= 1:
            d = (DIAMOND_BASE + DIAMOND_STEP_1 * self.upgrade_level("diamond_chance")
                 + DIAMOND_STEP_2 * self.upgrade_level("diamond_chance_2")) * diamond_mult
        return cap_chance(g, GOLDEN_MAX_CHANCE), cap_chance(d, DIAMOND_MAX_CHANCE)

    def combined_chance(self, rarity_index, mutation, with_luck=True, weights=None):
        """Chance REAL de sair este pet JÁ com esta mutação, por roll, com a tua sorte atual
        (upgrades + trait equipada + metas) e as tuas chances de mutação atuais. Não conta os
        bónus temporários do Golden / Diamond / Rainbow Roll. Dourado/Diamante são sempre mais
        raros do que a versão normal do mesmo pet."""
        if weights is None:      # quem chama em ciclo (Index) passa os pesos já calculados
            weights = self.roll_weights(with_luck=with_luck)
        total = sum(weights)
        rarity_chance = weights[rarity_index] / total if total else 0.0
        gchance, dchance = self.mutation_chances()
        if mutation == "diamond":
            mut_factor = dchance
        elif mutation == "golden":
            mut_factor = (1.0 - dchance) * gchance
        else:
            mut_factor = (1.0 - dchance) * (1.0 - gchance)
        return rarity_chance * mut_factor

    # ---------------- rolls ----------------
    def roll(self):
        golden_paused = self.is_cycle_paused("golden")
        diamond_paused = self.is_cycle_paused("diamond")
        rainbow_paused = self.is_cycle_paused("rainbow")
        golden_active = self.cyclic_bonus_ready and not golden_paused
        diamond_active = self.diamond_bonus_ready and self.diamond_roll_unlocked() and not diamond_paused
        rainbow_active = self.rainbow_bonus_ready and self.rainbow_roll_unlocked() and not rainbow_paused
        bonus_active = golden_active or diamond_active or rainbow_active

        # Golden / Diamond / Rainbow Roll ao mesmo tempo: as sortes SOMAM-SE (não se multiplicam).
        # Ex. sem upgrades: Golden x10 + Diamond x50 + Rainbow x250 = x310 de sorte. Com upgrades, cada
        # um já vem mais alto e a soma sobe com eles.
        active_mults = []
        if golden_active:
            active_mults.append(self.golden_roll_mult())
        if diamond_active:
            active_mults.append(self.diamond_roll_mult())
        if rainbow_active:
            active_mults.append(self.rainbow_roll_mult())
        bonus_mult = sum(active_mults) if active_mults else 1.0

        weights = self.roll_weights(with_luck=True, bonus_mult=bonus_mult)
        total = sum(weights)
        roll_val = random.random() * total
        cum = 0.0
        rarity_index = len(RARITIES) - 1
        for i, w in enumerate(weights):
            cum += w
            if roll_val <= cum:
                rarity_index = i
                break

        gchance, dchance = self.mutation_chances()
        mutation = "normal"
        if dchance > 0 and random.random() < dchance:
            mutation = "diamond"
        elif gchance > 0 and random.random() < gchance:
            mutation = "golden"

        key = "%d_%s" % (rarity_index, mutation)
        self.owned[key] = self.owned.get(key, 0) + 1
        self.last_roll = (rarity_index, mutation)
        self.total_rolls += 1
        self.daily_add_progress("rolls")
        self.daily_add_progress("rarity_%s" % RARITIES[rarity_index]["key"])
        if mutation == "golden":
            self.total_golden_rolled += 1
            self.daily_add_progress("golden")
        elif mutation == "diamond":
            self.total_diamond_rolled += 1
            self.daily_add_progress("diamond")
        if RARITIES[rarity_index]["key"] == "secreto":
            self.total_secret_rolled += 1
        elif RARITIES[rarity_index]["key"] == "divino":
            self.total_divine_rolled += 1
        elif RARITIES[rarity_index]["key"] == "cosmico":
            self.total_cosmic_rolled += 1
        elif RARITIES[rarity_index]["key"] == "transcendente":
            self.total_transcendent_rolled += 1

        # ---- Golden/Diamond/Rainbow Roll: consome o bónus pronto, ou avança o contador ----
        # (um ciclo pausado não faz nada: nem conta este roll, nem gasta o bónus que já estiver pronto)
        if golden_active:
            self.cyclic_bonus_ready = False
            self.cyclic_roll_count = 0
        elif not golden_paused:
            self.cyclic_roll_count += 1
            if self.cyclic_roll_count >= self.golden_roll_every():
                self.cyclic_roll_count = 0
                self.cyclic_bonus_ready = True

        if self.diamond_roll_unlocked():
            if diamond_active:
                self.diamond_bonus_ready = False
                self.diamond_roll_count = 0
            elif not diamond_paused:
                self.diamond_roll_count += 1
                if self.diamond_roll_count >= self.diamond_roll_every():
                    self.diamond_roll_count = 0
                    self.diamond_bonus_ready = True

        if self.rainbow_roll_unlocked():
            if rainbow_active:
                self.rainbow_bonus_ready = False
                self.rainbow_roll_count = 0
            elif not rainbow_paused:
                self.rainbow_roll_count += 1
                if self.rainbow_roll_count >= self.rainbow_roll_every():
                    self.rainbow_roll_count = 0
                    self.rainbow_bonus_ready = True

        gained_charge = False
        if random.random() < self.trait_charge_chance():
            self.trait_charges += 1
            gained_charge = True

        return rarity_index, mutation, gained_charge, bonus_active

    # ---------------- milestones ----------------
    def owned_of_rarity(self, rarity_key):
        """Quantos pets desta raridade (todas as mutações) já saíram."""
        total = 0
        for k, v in self.owned.items():
            try:
                if RARITIES[int(k.split("_")[0])]["key"] == rarity_key:
                    total += int(v)
            except (ValueError, IndexError):
                continue
        return total

    def indexed_pets_count(self):
        """Quantas cartas DIFERENTES do Index já tens (pet + mutação). Repetidos não contam."""
        n = 0
        for k, v in self.owned.items():
            try:
                idx_s, mut = k.split("_", 1)
                if int(v) > 0 and 0 <= int(idx_s) < len(RARITIES) and mut in MUTATIONS:
                    n += 1
            except (ValueError, TypeError):
                continue
        return n

    # ---------------- equipar ----------------
    def count_owned(self, rarity_index, mutation):
        return self.owned.get("%d_%s" % (rarity_index, mutation), 0)

    def equipped_count(self, rarity_index, mutation):
        return sum(1 for p in self.equipped if p[0] == rarity_index and p[1] == mutation)

    def equip_add(self, rarity_index, mutation):
        if (self.equipped_count(rarity_index, mutation) < self.count_owned(rarity_index, mutation)
                and len(self.equipped) < self.max_slots()):
            self.equipped.append([rarity_index, mutation])
            return True
        return False

    def equip_remove_one(self, rarity_index, mutation):
        for i in range(len(self.equipped) - 1, -1, -1):
            if self.equipped[i][0] == rarity_index and self.equipped[i][1] == mutation:
                self.equipped.pop(i)
                return True
        return False

    def remove_slot_at(self, index):
        if 0 <= index < len(self.equipped):
            self.equipped.pop(index)