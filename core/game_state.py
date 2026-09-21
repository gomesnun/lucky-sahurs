"""GameState: o estado completo de um save (juntando pets, upgrades, traits, milestones e economia)."""

import json
import os

from core.daily_missions import MISSION_BY_TYPE, DailyMissionsMixin
from core.economy import EconomyMixin
from core.milestones import MILESTONE_DEFS, MilestoneMixin
from core.offline import OfflineMixin
from core.pets import MUTATIONS, PetMixin, RARITIES
from core.rebirths import RebirthMixin
from core.traits import TRAITS, TraitMixin
from core.upgrades import UPGRADE_DEFS, UpgradeMixin
from online.cloud_cache import write_cache
from storage import save_slot_path


class GameState(EconomyMixin, UpgradeMixin, PetMixin, TraitMixin, MilestoneMixin,
                RebirthMixin, DailyMissionsMixin, OfflineMixin):
    """Estado completo de um save. As regras estão nos mixins (economia, upgrades, pets, traits,
    milestones, rebirths, missões diárias, ganhos offline); aqui ficam só a criação do estado
    e o guardar / carregar."""

    def __init__(self):
        self.slot = None         # número do slot de save (1..3); None = ainda sem slot (não grava)
        self.coins = 0.0
        self.owned = {}          # "{indice_raridade}_{mutacao}" -> quantidade
        self.equipped = []       # lista de [indice_raridade, mutacao] (pode repetir)
        self.upgrades = {k: 0 for k in UPGRADE_DEFS}
        self.last_roll = None
        self.total_rolls = 0
        self.auto_on = True

        self.trait_charges = 0
        self.owned_traits = set()
        self.equipped_trait = None
        self.last_trait_roll = None
        self.last_trait_batch = None     # {indice_trait: quantidade} do último "Usar Todas as Cargas"
        self.total_traits_rolled = 0     # para a meta "Traits Rolled"

        self.total_coins_earned = 0.0    # para as metas de dinheiro (nunca desce ao gastar)
        self.playtime = 0.0              # segundos de jogo, para as metas de tempo
        self.total_golden_rolled = 0     # para a meta "Golden Pets Rolled"
        self.total_diamond_rolled = 0    # para a meta "Diamond Pets Rolled"
        self.total_secret_rolled = 0     # para a meta "Secret Pets Rolled"
        self.total_divine_rolled = 0     # para a meta "Divine Pets Rolled"
        self.total_cosmic_rolled = 0     # para a meta "Cosmic Pets Rolled"
        self.total_transcendent_rolled = 0   # para a meta "Transcendent Pets Rolled"
        self.milestones_claimed = set()  # chaves "categoria:indice"
        self._ms_bonus = {}              # cache: tipo de recompensa -> bónus total

        self.cyclic_roll_count = 0       # rolls desde o último Golden Roll
        self.cyclic_bonus_ready = False  # True = a próxima roll vem com o bónus do Golden Roll
        self.diamond_roll_count = 0      # rolls desde o último Diamond Roll
        self.diamond_bonus_ready = False
        self.rainbow_roll_count = 0      # rolls desde o último Rainbow Roll
        self.rainbow_bonus_ready = False
        # ciclos pausados (clicar na barra): um ciclo pausado fica congelado - não conta rolls e não dispara
        self.cycle_paused = {"golden": False, "diamond": False, "rainbow": False}

        self.auto_equip_best_on = True   # só tem efeito depois de "auto_equip_unlock" comprado

        # ---- conta na cloud (None enquanto o slot for só local) ----
        self.cloud_uid = None             # uid da conta dona deste save (None = save local, ficheiro no PC)
        self.cloud_base_time = None       # "updateTime" da cloud em que este estado se baseia
        self.dirty = False                # True = há progresso ainda não enviado para a cloud
        self.sync_conflict = False        # True = a cloud mudou noutro PC; parou de enviar até reabrir o slot
        self.save_seq = 0                 # incrementa a cada save() enquanto é um save de conta

        # ---- rebirths ----
        self.rebirths = 0                 # nº de rebirths feitos (sem limite); dá bónus permanente

        # ---- ganhos offline ----
        self.last_seen = None             # timestamp (time.time()) da última vez que isto foi guardado

        # ---- missões diárias ----
        self.daily_date = None            # dia (YYYY-MM-DD) para o qual as missões abaixo são válidas
        self.daily_missions = []          # lista de {"type", "target", "reward"} (sempre 3)
        self.daily_counts = {}            # tipo -> quantidade feita HOJE (reseta com o dia)
        self.daily_claimed = set()        # índices (0..2) já resgatados hoje
        # (as missões só são geradas mais tarde, quando o slot já está atribuído - ver ensure_daily_missions)

    # ---------------- save / load ----------------
    def to_dict(self):
        return {
            "coins": self.coins,
            "owned": self.owned,
            "equipped": [list(p) for p in self.equipped],
            "upgrades": self.upgrades,
            "total_rolls": self.total_rolls,
            "settings": {"auto_on": self.auto_on, "auto_equip_best_on": self.auto_equip_best_on},
            "traits": {
                "charges": self.trait_charges,
                "owned": sorted(self.owned_traits),
                "equipped": self.equipped_trait,
                "total_rolled": self.total_traits_rolled,
            },
            "total_coins_earned": self.total_coins_earned,
            "playtime": self.playtime,
            "total_golden_rolled": self.total_golden_rolled,
            "total_diamond_rolled": self.total_diamond_rolled,
            "total_secret_rolled": self.total_secret_rolled,
            "total_divine_rolled": self.total_divine_rolled,
            "total_cosmic_rolled": self.total_cosmic_rolled,
            "total_transcendent_rolled": self.total_transcendent_rolled,
            "milestones_claimed": sorted(self.milestones_claimed),
            "cyclic_roll_count": self.cyclic_roll_count,
            "cyclic_bonus_ready": self.cyclic_bonus_ready,
            "diamond_roll_count": self.diamond_roll_count,
            "diamond_bonus_ready": self.diamond_bonus_ready,
            "rainbow_roll_count": self.rainbow_roll_count,
            "rainbow_bonus_ready": self.rainbow_bonus_ready,
            "cycle_paused": dict(self.cycle_paused),
            "rebirths": self.rebirths,
            "last_seen": self.last_seen,
            "daily": {
                "date": self.daily_date,
                "missions": self.daily_missions,
                "counts": self.daily_counts,
                "claimed": sorted(self.daily_claimed),
            },
        }

    def load_dict(self, d):
        self.coins = float(d.get("coins", 0.0))
        self.owned = {str(k): int(v) for k, v in d.get("owned", {}).items()}
        self.equipped = []
        for p in d.get("equipped", []):
            try:
                idx, mut = int(p[0]), str(p[1])
            except (TypeError, ValueError, IndexError):
                continue
            if 0 <= idx < len(RARITIES) and mut in MUTATIONS:
                self.equipped.append([idx, mut])
        # os rebirths têm de ser lidos ANTES do limite de slots abaixo (as recompensas de Rebirth dão slots extra)
        self.rebirths = max(0, int(d.get("rebirths", 0)))
        loaded_up = d.get("upgrades", {})
        for k in UPGRADE_DEFS:
            lvl = int(loaded_up.get(k, 0))
            self.upgrades[k] = max(0, min(lvl, UPGRADE_DEFS[k]["max_level"]))
        self.total_rolls = int(d.get("total_rolls", 0))
        st = d.get("settings", {})
        self.auto_on = bool(st.get("auto_on", True))
        self.auto_equip_best_on = bool(st.get("auto_equip_best_on", True))
        # segurança: nunca mais slots equipados do que os disponíveis
        del self.equipped[self.max_slots():]

        tr = d.get("traits", {})
        self.trait_charges = max(0, int(tr.get("charges", 0)))
        self.owned_traits = {int(i) for i in tr.get("owned", []) if 0 <= int(i) < len(TRAITS)}
        eq = tr.get("equipped")
        self.equipped_trait = int(eq) if (eq is not None and int(eq) in self.owned_traits) else None
        self.total_traits_rolled = max(0, int(tr.get("total_rolled", 0)))

        self.total_coins_earned = max(self.coins, float(d.get("total_coins_earned", 0.0)))
        self.playtime = max(0.0, float(d.get("playtime", 0.0)))
        self.total_golden_rolled = max(0, int(d.get("total_golden_rolled", 0)))
        self.total_diamond_rolled = max(0, int(d.get("total_diamond_rolled", 0)))
        # saves antigos não tinham estes contadores: usa os pets que já tens (não se vendem)
        self.total_secret_rolled = max(0, int(d.get("total_secret_rolled", self.owned_of_rarity("secreto"))))
        self.total_divine_rolled = max(0, int(d.get("total_divine_rolled", self.owned_of_rarity("divino"))))
        self.total_cosmic_rolled = max(0, int(d.get("total_cosmic_rolled", self.owned_of_rarity("cosmico"))))
        self.total_transcendent_rolled = max(0, int(d.get("total_transcendent_rolled",
                                                          self.owned_of_rarity("transcendente"))))
        # só aceita chaves de metas que ainda existem (o jogo mudou de versão)
        claimed = set()
        for k in d.get("milestones_claimed", []):
            k = str(k)
            try:
                cat, idx = k.split(":")
                if cat in MILESTONE_DEFS and 0 <= int(idx) < len(MILESTONE_DEFS[cat]["tiers"]):
                    claimed.add(k)
            except ValueError:
                continue
        self.milestones_claimed = claimed
        self.recalc_milestone_bonus()
        self.cyclic_roll_count = max(0, min(int(d.get("cyclic_roll_count", 0)), self.golden_roll_every() - 1))
        self.cyclic_bonus_ready = bool(d.get("cyclic_bonus_ready", False))
        self.diamond_roll_count = max(0, min(int(d.get("diamond_roll_count", 0)), self.diamond_roll_every() - 1))
        self.diamond_bonus_ready = bool(d.get("diamond_bonus_ready", False))
        self.rainbow_roll_count = max(0, min(int(d.get("rainbow_roll_count", 0)), self.rainbow_roll_every() - 1))
        self.rainbow_bonus_ready = bool(d.get("rainbow_bonus_ready", False))
        paused = d.get("cycle_paused", {})
        if not isinstance(paused, dict):
            paused = {}
        self.cycle_paused = {k: bool(paused.get(k, False)) for k in ("golden", "diamond", "rainbow")}

        last_seen = d.get("last_seen")
        self.last_seen = float(last_seen) if isinstance(last_seen, (int, float)) else None

        daily = d.get("daily", {})
        if not isinstance(daily, dict):
            daily = {}
        self.daily_date = daily.get("date") if isinstance(daily.get("date"), str) else None
        self.daily_missions = []
        for m in daily.get("missions", []) if isinstance(daily.get("missions"), list) else []:
            try:
                mtype, target, reward = str(m["type"]), int(m["target"]), int(m["reward"])
                if mtype in MISSION_BY_TYPE and target > 0 and reward > 0:
                    self.daily_missions.append({"type": mtype, "target": target, "reward": reward})
            except (KeyError, TypeError, ValueError):
                continue
        self.daily_counts = {}
        counts = daily.get("counts", {})
        if isinstance(counts, dict):
            for k, v in counts.items():
                try:
                    self.daily_counts[str(k)] = max(0, int(v))
                except (TypeError, ValueError):
                    continue
        self.daily_claimed = set()
        for i in daily.get("claimed", []) if isinstance(daily.get("claimed"), list) else []:
            try:
                ii = int(i)
                if 0 <= ii < len(self.daily_missions):
                    self.daily_claimed.add(ii)
            except (TypeError, ValueError):
                continue
        self.ensure_daily_missions()      # gera missões novas se o dia mudou (ou save antigo sem nenhuma)

    def save(self):
        if self.slot is None:
            return
        if self.cloud_uid:
            # save de conta: nunca escreve no ficheiro local do slot (esse é só para saves sem conta) -
            # fica na cache por-uid e é marcado como "por enviar"; o upload real acontece à parte.
            self.save_seq += 1
            self.dirty = True
            write_cache(self.cloud_uid, self.slot, self.cloud_base_time, True, self.to_dict())
            return
        path = save_slot_path(self.slot)
        try:
            tmp = path + ".tmp"
            with open(tmp, "w", encoding="utf-8") as f:
                json.dump(self.to_dict(), f)
            os.replace(tmp, path)
        except OSError:
            pass

    def load(self):
        if self.slot is None:
            return
        path = save_slot_path(self.slot)
        if os.path.exists(path):
            try:
                with open(path, "r", encoding="utf-8") as f:
                    self.load_dict(json.load(f))
            except (OSError, ValueError, KeyError, TypeError, AttributeError):
                pass