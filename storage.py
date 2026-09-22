"""Saves locais (slots) e definições (som, volume, ecrã inteiro...)."""

import json
import os

from config import SAVE_DIR, SAVE_PATH, SAVE_SLOTS
from i18n import DEFAULT_LANGUAGE, LANGUAGE_CODES
from theme import DEFAULT_THEME_MODE, THEME_MODES


def save_slot_path(slot):
    return os.path.join(SAVE_DIR, "savegame_slot%d.json" % slot)


def migrate_legacy_save():
    """Se já existir um save da versão antiga (ficheiro único) e ainda não houver
    nenhum slot novo, copia-o para o Slot 1 — para ninguém perder o progresso."""
    try:
        if not os.path.exists(SAVE_PATH):
            return
        if any(os.path.exists(save_slot_path(i)) for i in range(1, SAVE_SLOTS + 1)):
            return
        with open(SAVE_PATH, "r", encoding="utf-8") as f:
            data = f.read()
        with open(save_slot_path(1), "w", encoding="utf-8") as f:
            f.write(data)
    except OSError:
        pass


def peek_slot(slot):
    """Lê só o resumo de um slot (sem afetar o jogo atual) para mostrar no ecrã de Saves."""
    path = save_slot_path(slot)
    if not os.path.exists(path):
        return None
    try:
        with open(path, "r", encoding="utf-8") as f:
            d = json.load(f)
        return {
            "coins": float(d.get("coins", 0.0)),
            "total_rolls": int(d.get("total_rolls", 0)),
            "playtime": float(d.get("playtime", 0.0)),
        }
    except (OSError, ValueError, TypeError, AttributeError):
        # ficheiro existe mas está estragado -> mostra como slot usado, mas vazio de dados
        return {"coins": 0.0, "total_rolls": 0, "playtime": 0.0}


def delete_slot(slot):
    try:
        path = save_slot_path(slot)
        if os.path.exists(path):
            os.remove(path)
    except OSError:
        pass


# ----------------------------------------------------------------------------------
# OPÇÕES GLOBAIS (não dependem do save)
# ----------------------------------------------------------------------------------
SETTINGS_PATH = os.path.join(SAVE_DIR, "lucky_verities_settings.json")
# Sons (SFX) que se podem ligar/desligar um a um nas Options (chave, nome mostrado).
# A definição guardada chama-se "sfx_<chave>" (ex.: "sfx_click"). Que som pertence a que grupo: ui/audio.py.
SFX_CATEGORIES = (
    ("click", "Clicks"),
    ("roll", "Rolling"),
    ("traits", "Traits"),
    ("upgrades", "Upgrades"),
    ("milestones", "Milestones"),
    ("rebirth", "Rebirth"),
)

# Cutscenes (ver ui/cutscene_panel.py): uma ligada/desligada por raridade, para dar para escolher
# só as que se quer ver (ex.: só Transcendent). A chave guardada é "cutscenes_<raridade>".
CUTSCENE_RARITIES = ("secreto", "divino", "cosmico", "transcendente")

DEFAULT_SETTINGS = {"sound_on": True, "volume": 0.6, "animations": True, "fullscreen": True,
                    "trait_notifications": True, "buy_mode": 1, "music_on": True,
                    "music_volume": 0.5, "sfx_volume": 1.0, "sfx_on": True,
                    "language": DEFAULT_LANGUAGE, "theme_mode": DEFAULT_THEME_MODE}
for _key, _label in SFX_CATEGORIES:
    DEFAULT_SETTINGS["sfx_" + _key] = True
for _key in CUTSCENE_RARITIES:
    DEFAULT_SETTINGS["cutscenes_" + _key] = True


def load_settings():
    s = dict(DEFAULT_SETTINGS)
    if os.path.exists(SETTINGS_PATH):
        try:
            with open(SETTINGS_PATH, "r", encoding="utf-8") as f:
                d = json.load(f)
            s["sound_on"] = bool(d.get("sound_on", True))
            s["volume"] = max(0.0, min(1.0, float(d.get("volume", 0.6))))
            s["animations"] = bool(d.get("animations", True))
            s["fullscreen"] = bool(d.get("fullscreen", True))
            s["trait_notifications"] = bool(d.get("trait_notifications", True))
            # migração do interruptor único antigo ("cutscenes_enabled"): quem tinha desligado tudo
            # fica com as 4 raridades desligadas por omissão; o resto (ou quem nunca teve a chave
            # antiga) fica com o normal, todas ligadas.
            legado_default = bool(d.get("cutscenes_enabled", True))
            for key in CUTSCENE_RARITIES:
                s["cutscenes_" + key] = bool(d.get("cutscenes_" + key, legado_default))
            s["music_on"] = bool(d.get("music_on", True))
            # tema: "system" (segue o Windows / sistema), "dark" ou "light". Contas antigas nao tem esta chave
            # (ou tinham o interruptor antigo "dark_mode"), por isso ficam todas em "system".
            mode = d.get("theme_mode", DEFAULT_THEME_MODE)
            s["theme_mode"] = mode if mode in THEME_MODES else DEFAULT_THEME_MODE
            # "volume" = volume mestre (o que já existia). A música e os SFX têm o seu próprio volume por cima.
            s["music_volume"] = max(0.0, min(1.0, float(d.get("music_volume", 0.5))))
            s["sfx_volume"] = max(0.0, min(1.0, float(d.get("sfx_volume", 1.0))))
            s["sfx_on"] = bool(d.get("sfx_on", True))
            lang = d.get("language", DEFAULT_LANGUAGE)          # idioma do jogo (ver i18n.py)
            s["language"] = lang if lang in LANGUAGE_CODES else DEFAULT_LANGUAGE
            for key, _label in SFX_CATEGORIES:
                s["sfx_" + key] = bool(d.get("sfx_" + key, True))
            bm = d.get("buy_mode", 1)          # Upgrades: comprar 1, 10 ou o máximo de níveis de uma vez
            s["buy_mode"] = bm if bm in (1, 10, "max") else 1
        except (OSError, ValueError, TypeError, AttributeError):
            pass
    return s


def save_settings(settings):
    try:
        tmp = SETTINGS_PATH + ".tmp"
        with open(tmp, "w", encoding="utf-8") as f:
            json.dump(settings, f)
        os.replace(tmp, SETTINGS_PATH)
    except OSError:
        pass