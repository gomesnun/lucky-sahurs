"""Missões diárias: 3 por dia (escolhidas ao acaso, com objetivos que sobem com o teu progresso),
com uma recompensa em cargas de trait quando completas cada uma."""

import datetime
import random

from i18n import tr


# Cada tipo tem 5 níveis de dificuldade (objetivo + recompensa) - o nível usado depende de
# quão avançado já estás (ver mission_stage), para as missões nunca ficarem triviais nem impossíveis.
MISSION_POOL = [
    {"type": "rolls", "label": "Roll %d pets",
     "targets": [150, 300, 600, 1200, 2500], "reward": [3, 4, 5, 6, 8]},
    {"type": "golden", "label": "Get %d Golden pets",
     "targets": [2, 4, 8, 15, 30], "reward": [3, 4, 5, 6, 8]},
    {"type": "diamond", "label": "Get %d Diamond pets",
     "targets": [1, 2, 4, 8, 15], "reward": [4, 5, 6, 8, 10]},
    {"type": "traits", "label": "Roll %d traits",
     "targets": [2, 4, 7, 12, 20], "reward": [3, 4, 5, 6, 8]},
    {"type": "rarity_epico", "label": "Get %d Epic pet(s)",
     "targets": [1, 2, 3, 5, 8], "reward": [3, 4, 5, 6, 7]},
    {"type": "rarity_lendario", "label": "Get %d Legendary pet(s)",
     "targets": [1, 1, 2, 3, 5], "reward": [4, 5, 6, 7, 8]},
    {"type": "rarity_mitico", "label": "Get %d Mythic pet(s)",
     "targets": [1, 1, 1, 2, 3], "reward": [6, 6, 7, 8, 10]},
    {"type": "rarity_secreto", "label": "Get a Secret pet",
     "targets": [1, 1, 1, 1, 1], "reward": [10, 10, 10, 10, 10]},
]
MISSION_BY_TYPE = {m["type"]: m for m in MISSION_POOL}
DAILY_MISSION_COUNT = 3


def mission_stage(state):
    """0..4: aproxima o quão avançado o jogador está (pelas rolls totais), para
    escolher objetivos/recompensas de um nível razoável."""
    r = state.total_rolls
    if r < 250:
        return 0
    if r < 2500:
        return 1
    if r < 25000:
        return 2
    if r < 250000:
        return 3
    return 4


def generate_daily_missions(state, date_str):
    """3 missões ao acaso (mas estáveis no mesmo dia, mesmo se reabrires o jogo)."""
    rng = random.Random("%s:slot%s" % (date_str, state.slot))
    stage = mission_stage(state)
    chosen = rng.sample(MISSION_POOL, k=min(DAILY_MISSION_COUNT, len(MISSION_POOL)))
    missions = []
    for m in chosen:
        idx = min(stage, len(m["targets"]) - 1)
        missions.append({"type": m["type"], "target": m["targets"][idx], "reward": m["reward"][idx]})
    return missions


def mission_label(mission):
    m = MISSION_BY_TYPE.get(mission["type"])
    if not m:
        return "???"
    if "%" in m["label"]:
        return tr(m["label"], mission["target"])
    return tr(m["label"])           # ex.: "Get a Secret pet" (sem número)


class DailyMissionsMixin:
    """Missões diárias: geração automática, progresso (contadores que resetam por dia) e resgate."""

    def _today_str(self):
        return datetime.date.today().isoformat()

    def ensure_daily_missions(self):
        """Gera missões novas se for um dia novo (ou se ainda não houver nenhuma)."""
        today = self._today_str()
        if self.daily_date == today and self.daily_missions:
            return
        self.daily_date = today
        self.daily_missions = generate_daily_missions(self, today)
        self.daily_counts = {}
        self.daily_claimed = set()

    def daily_mission_progress(self, index):
        if not (0 <= index < len(self.daily_missions)):
            return 0
        mtype = self.daily_missions[index]["type"]
        return self.daily_counts.get(mtype, 0)

    def daily_add_progress(self, mtype, n=1):
        """Chamado a cada roll de pet / trait para avançar as missões desse tipo (se existirem hoje)."""
        self.ensure_daily_missions()
        self.daily_counts[mtype] = self.daily_counts.get(mtype, 0) + n

    def claim_daily_mission(self, index):
        """Resgata a missão (dá as cargas de trait). Devolve quantas cargas deu (0 = não deu para resgatar)."""
        self.ensure_daily_missions()
        if not (0 <= index < len(self.daily_missions)) or index in self.daily_claimed:
            return 0
        mission = self.daily_missions[index]
        if self.daily_mission_progress(index) < mission["target"]:
            return 0
        self.daily_claimed.add(index)
        self.trait_charges += mission["reward"]
        return mission["reward"]