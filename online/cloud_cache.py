"""Sessão guardada, cache local dos saves da conta e períodos da leaderboard."""

import json
import os
import re
import time
import uuid

from config import SAVE_DIR
from i18n import tr
from online.firebase import (
    LEADERBOARD_FETCH_DELAY, LEADERBOARD_PERIOD, LEADERBOARD_SIZE, save_summary,
)


SESSION_PATH = os.path.join(SAVE_DIR, "lucky_verities_session.json")
CACHE_DIR = os.path.join(SAVE_DIR, "cloud_cache")
LEADERBOARD_CACHE_PATH = os.path.join(SAVE_DIR, "lucky_verities_leaderboard.json")


# ---- sessão (para não teres de iniciar sessão de cada vez que abres o jogo) ----
def load_session():
    try:
        with open(SESSION_PATH, "r", encoding="utf-8") as f:
            d = json.load(f)
        if all(isinstance(d.get(k), str) and d.get(k) for k in ("uid", "username", "refresh_token")):
            return d
    except (OSError, ValueError, AttributeError):
        pass
    return None


def store_session(uid, username, refresh_token, last_pub_time=0.0):
    try:
        tmp = SESSION_PATH + ".tmp"
        with open(tmp, "w", encoding="utf-8") as f:
            json.dump({"uid": uid, "username": username, "refresh_token": refresh_token,
                       "last_pub_time": float(last_pub_time)}, f)
        os.replace(tmp, SESSION_PATH)
    except OSError:
        pass


def clear_session():
    try:
        if os.path.exists(SESSION_PATH):
            os.remove(SESSION_PATH)
    except OSError:
        pass


# ---- nome deste PROCESSO (para o "uma conta, um jogo de cada vez") ----
INSTALL_ID_RE = re.compile(r"^[A-Za-z0-9_-]{8,64}$")


def install_id():
    """Identificador NOVO a cada arranque do jogo (não fica guardado em disco). Assim, dois .exe
    abertos ao mesmo tempo - mesmo no mesmo PC - contam sempre como dispositivos diferentes, e um
    bloqueia mesmo o outro. Fechar o jogo normalmente larga logo a marca (ver release_session), por
    isso reabrir a seguir no mesmo PC continua instantâneo - só um crash é que obriga a esperar os
    SESSION_STALE segundos, tal como acontecia antes."""
    return uuid.uuid4().hex


# ---- cache local dos saves da conta (o jogo grava aqui de 10 em 10 s e sincroniza com a cloud) ----
def cache_path(uid, slot, suffix=""):
    safe = re.sub(r"[^A-Za-z0-9_-]", "", str(uid)) or "unknown"
    return os.path.join(CACHE_DIR, safe, "slot%d%s.json" % (slot, suffix))


def read_cache(uid, slot):
    try:
        with open(cache_path(uid, slot), "r", encoding="utf-8") as f:
            d = json.load(f)
        if isinstance(d, dict) and isinstance(d.get("state"), dict):
            return {"state": d["state"], "base_time": d.get("base_time"), "dirty": bool(d.get("dirty", True))}
    except (OSError, ValueError, TypeError):
        pass
    return None


def write_cache(uid, slot, base_time, dirty, state_dict, suffix=""):
    path = cache_path(uid, slot, suffix)
    try:
        os.makedirs(os.path.dirname(path), exist_ok=True)
        tmp = path + ".tmp"
        with open(tmp, "w", encoding="utf-8") as f:
            json.dump({"base_time": base_time, "dirty": bool(dirty), "state": state_dict}, f)
        os.replace(tmp, path)
    except OSError:
        pass


def delete_cache(uid, slot):
    try:
        path = cache_path(uid, slot)
        if os.path.exists(path):
            os.remove(path)
    except OSError:
        pass


def peek_cache(uid, slot):
    c = read_cache(uid, slot)
    if not c:
        return None
    s = save_summary(c["state"])
    return {"coins": s["coins"], "total_rolls": s["rolls"], "playtime": s["playtime"], "rebirths": s["rebirths"]}


def backup_state(uid, slot, label, state_dict):
    """Guarda a versão que NÃO foi usada num conflito (nunca é apagada)."""
    write_cache(uid, slot, None, False, state_dict, suffix="_%s_backup_%d" % (label, int(time.time())))


def pick_save(cloud, cache):
    """Decide de onde vem o progresso ao abrir um slot da conta.
    cloud: {"data": dict, "time": str} ou None      cache: {"state", "base_time", "dirty"} ou None
    Devolve (state_dict|None, base_time, dirty, nota, perdedor). 'perdedor' = (nome, dict) da versão que
    NÃO foi usada quando as duas mudaram: fica num ficheiro de backup, nunca se perde."""
    if cloud is None and cache is None:
        return None, None, True, None, None
    if cache is None:
        return cloud["data"], cloud["time"], False, None, None
    if cloud is None:
        return cache["state"], None, True, None, None
    if cache["base_time"] == cloud["time"]:
        if cache["dirty"]:
            return cache["state"], cloud["time"], True, None, None
        return cloud["data"], cloud["time"], False, None, None
    if not cache["dirty"]:
        return cloud["data"], cloud["time"], False, None, None
    # a cloud mudou noutro PC E aqui também há progresso por enviar: ganha quem tem mais playtime
    try:
        local_pt = float(cache["state"].get("playtime", 0.0))
        cloud_pt = float(cloud["data"].get("playtime", 0.0))
    except (TypeError, ValueError, AttributeError):
        local_pt = cloud_pt = 0.0
    if local_pt > cloud_pt:
        return (cache["state"], cloud["time"], True,
                tr("Kept this PC's progress (it was ahead of the cloud). The cloud version was backed up."),
                ("cloud", cloud["data"]))
    return (cloud["data"], cloud["time"], False,
            tr("The cloud save was ahead of this PC. Your local version was backed up."),
            ("local", cache["state"]))


# ---- leaderboard: períodos de 10 min certinhos no relógio ----
def lb_publish_period(now):
    return int(now // LEADERBOARD_PERIOD)


def lb_fetch_period(now):
    return int((now - LEADERBOARD_FETCH_DELAY) // LEADERBOARD_PERIOD)


def lb_next_update(now):
    return (lb_fetch_period(now) + 1) * LEADERBOARD_PERIOD + LEADERBOARD_FETCH_DELAY


# tabelas da leaderboard (chave usada na cache / no ecrã) -> campo do documento /leaderboard/{uid}
LB_FIELDS = (("money", "coins"), ("playtime", "playtime"), ("rolls", "rolls"), ("rebirths", "rebirths"))
LB_KEYS = tuple(k for k, _f in LB_FIELDS)


def fetch_leaderboards(client, period):
    data = {}
    for key, field in LB_FIELDS:
        # rebirths: só entra quem já fez pelo menos 1 (senão a tabela enchia-se de zeros)
        data[key] = client.top_entries(field, LEADERBOARD_SIZE, min_value=1 if key == "rebirths" else None)
    data["period"] = period
    data["fetched_at"] = time.time()
    return data


def lb_data_complete(d):
    """False se a cache for de uma versão antiga (sem as tabelas Rolls / Rebirths) -> pede-se de novo."""
    return isinstance(d, dict) and all(isinstance(d.get(k), list) for k in LB_KEYS)


def load_lb_cache():
    try:
        with open(LEADERBOARD_CACHE_PATH, "r", encoding="utf-8") as f:
            d = json.load(f)
        if isinstance(d, dict) and isinstance(d.get("money"), list) and isinstance(d.get("playtime"), list):
            return d
    except (OSError, ValueError):
        pass
    return None


def store_lb_cache(data):
    try:
        tmp = LEADERBOARD_CACHE_PATH + ".tmp"
        with open(tmp, "w", encoding="utf-8") as f:
            json.dump(data, f)
        os.replace(tmp, LEADERBOARD_CACHE_PATH)
    except OSError:
        pass
