"""Cliente do Firebase (contas + Firestore por REST), erros de rede e o Worker de fundo."""

import calendar
import email.utils
import http.client
import json
import os
import queue
import re
import threading
import time
import traceback
import uuid
import urllib.error
import urllib.parse
import urllib.request

from config import SAVE_DIR
from i18n import tr
from online.tls import ssl_context


# Não precisa de nenhuma biblioteca extra: fala com o Firebase por HTTPS (REST) com o urllib.
# Para ativar: preenche as duas chaves aqui em baixo (ou cria o firebase_config.json) — ver
# FIREBASE_SETUP.md. Com as chaves vazias o jogo funciona como antes (só saves locais) e os
# botões online avisam que ainda não estão configurados.
FIREBASE_API_KEY = "AIzaSyDDRAionwoM0sZ0RWGwqNk14RY77zY1KvI"        # Project settings -> General -> "Web API Key"
# (O projeto Firebase e o domínio abaixo mantêm o nome antigo de propósito: as contas e as rules do Firestore
# dependem deles. NÃO os mudes, senão as contas existentes deixam de entrar.)
FIREBASE_PROJECT_ID = "lucky-sahurs"     # Project settings -> General -> "Project ID"
FIREBASE_CONFIG_PATH = os.path.join(SAVE_DIR, "firebase_config.json")   # {"api_key": "...", "project_id": "..."}

USERNAME_DOMAIN = "luckysahurs.invalid"      # o "email" interno de cada conta é <username>@<isto>
USERNAME_RE = re.compile(r"^[a-z0-9_]{3,16}$")
MIN_PASSWORD = 6
NET_TIMEOUT = 8.0

LEADERBOARD_PERIOD = 10 * 60        # a leaderboard atualiza de 10 em 10 minutos (relógio certo: :00, :10, :20, ...)
LEADERBOARD_PUBLISH_JITTER = 30     # cada conta publica entre 0 e 30 s depois da hora certa (para não ser tudo no mesmo segundo)
LEADERBOARD_FETCH_DELAY = 60        # quem vê espera 60 s depois da hora certa: já toda a gente publicou (jitter 30 s + rede)
LEADERBOARD_MIN_GAP = 6 * 60        # mínimo entre duas publicações da mesma conta. As regras do Firestore têm de exigir
                                    # MENOS do que isto: 'duration.value(5, 'm')' na regra "allow update" de /leaderboard
LEADERBOARD_SIZE = 50
CLOUD_SYNC_INTERVAL = 90.0          # segundos entre uploads do save para a cloud (a cache local grava de 10 em 10 s)

# ---- "uma conta, um jogo de cada vez" ----
SESSION_HEARTBEAT = 30.0            # de quanto em quanto tempo se renova a marca de "estou a jogar"
SESSION_STALE = 90.0                # sem renovar durante isto, a marca vale zero (jogo fechado à bruta, luz que foi
                                    # abaixo...). As regras do Firestore têm de usar o MESMO valor em
                                    # 'duration.value(90, 's')' na regra "allow update" de /users/{uid}/session/current


def load_firebase_config():
    key, pid = FIREBASE_API_KEY.strip(), FIREBASE_PROJECT_ID.strip()
    if not (key and pid) and os.path.exists(FIREBASE_CONFIG_PATH):
        try:
            with open(FIREBASE_CONFIG_PATH, "r", encoding="utf-8") as f:
                d = json.load(f)
            key = key or str(d.get("api_key", "")).strip()
            pid = pid or str(d.get("project_id", "")).strip()
        except (OSError, ValueError, AttributeError):
            pass
    return key, pid


def username_to_email(username):
    return "%s@%s" % (username.lower(), USERNAME_DOMAIN)


class OnlineError(Exception):
    """code: 'offline' | 'auth' | 'denied' | 'not_found' | 'conflict' | 'bad_request' | 'server'."""

    def __init__(self, code, message="", status=0):
        Exception.__init__(self, "%s: %s" % (code, message))
        self.code = code
        self.message = message
        self.status = status


AUTH_MESSAGES = (
    ("EMAIL_EXISTS", "That username is already taken."),
    ("INVALID_LOGIN_CREDENTIALS", "Wrong username or password."),
    ("INVALID_PASSWORD", "Wrong username or password."),
    ("EMAIL_NOT_FOUND", "Wrong username or password."),
    ("WEAK_PASSWORD", "Password must have at least 6 characters."),
    ("TOO_MANY_ATTEMPTS", "Too many attempts. Try again in a few minutes."),
    ("USER_DISABLED", "This account has been disabled."),
    ("OPERATION_NOT_ALLOWED", "Email/Password sign-in is not enabled in the Firebase project."),
    ("CONFIGURATION_NOT_FOUND", "Firebase Authentication isn't set up in this project yet."),
    ("API KEY", "The Firebase API key is not valid."),
)


def online_error_text(e):
    """Texto simples (no idioma atual do jogo) para mostrar ao jogador."""
    if e.code == "offline":
        return tr("Can't reach the server. Check your internet connection.")
    if e.code == "denied":
        return tr("The server refused this. Check the Firestore rules.")
    if getattr(e, "status", 0) == 429 or "RESOURCE_EXHAUSTED" in (e.message or "").upper():
        return tr("The server's free daily limit was reached. It comes back on its own later today.")
    if e.code == "auth":
        return tr("Session expired. Please log in again.")
    if e.code == "conflict":
        return tr("This was changed somewhere else. Try again.")
    msg = (e.message or "").upper()
    for key, text in AUTH_MESSAGES:
        if key in msg:
            return tr(text)
    return tr("Something went wrong (%s).", e.message or e.code)


def classify_http_error(e):
    status = e.code
    try:
        payload = json.loads(e.read().decode("utf-8"))
    except Exception:
        payload = {}
    err = payload.get("error", {}) if isinstance(payload, dict) else {}
    if not isinstance(err, dict):
        err = {"message": str(err)}
    message = str(err.get("message", ""))
    fs_status = str(err.get("status", ""))
    short = message.split(" : ")[0].strip()
    if status in (409, 412) or fs_status in ("ABORTED", "ALREADY_EXISTS", "FAILED_PRECONDITION"):
        return OnlineError("conflict", short, status)
    if status == 404 or fs_status == "NOT_FOUND":
        return OnlineError("not_found", short, status)
    if status == 401 or fs_status == "UNAUTHENTICATED":
        return OnlineError("auth", short, status)
    if status == 403 or fs_status == "PERMISSION_DENIED":
        return OnlineError("denied", short, status)
    if status == 400:
        return OnlineError("bad_request", short, status)
    return OnlineError("server", short or ("HTTP %d" % status), status)


# ---- relógio do servidor ----
# O relógio do PC do jogador pode estar completamente trocado (fuso mal posto, pilha da motherboard gasta).
# Como a marca de sessão é gravada com a hora do SERVIDOR, comparar com time.time() daria disparates: ou
# bloqueava o jogador para sempre, ou nunca bloqueava ninguém. Por isso guarda-se a diferença entre os dois
# relógios, lida do cabeçalho "Date" que vem em cada resposta HTTP.
_clock_lock = threading.Lock()
_clock_offset = 0.0                 # hora do servidor - hora deste PC
_clock_synced = False


def note_server_date(headers):
    global _clock_offset, _clock_synced
    try:
        stamp = email.utils.parsedate_to_datetime(headers.get("Date")).timestamp()
    except (AttributeError, TypeError, ValueError):
        return
    with _clock_lock:
        _clock_offset = stamp - time.time()
        _clock_synced = True


def server_now():
    """Hora do servidor (aproximada ao segundo: chega bem para uma janela de 90 s)."""
    with _clock_lock:
        return time.time() + _clock_offset


def server_clock_synced():
    with _clock_lock:
        return _clock_synced


def parse_timestamp(text):
    """'2026-09-21T12:34:56.789Z' -> segundos desde a época. None se não der para ler."""
    try:
        head = text.strip().rstrip("Z")
    except AttributeError:
        return None
    frac = 0.0
    if "." in head:
        head, dot = head.split(".", 1)
        try:
            frac = float("0." + re.sub(r"[^0-9]", "", dot))
        except ValueError:
            frac = 0.0
    try:
        return calendar.timegm(time.strptime(head, "%Y-%m-%dT%H:%M:%S")) + frac
    except (ValueError, TypeError):
        return None


def http_json(method, url, body=None, headers=None, form=False, timeout=NET_TIMEOUT):
    hdrs = {"Accept": "application/json"}
    data = None
    if body is not None:
        if form:
            data = urllib.parse.urlencode(body).encode("utf-8")
            hdrs["Content-Type"] = "application/x-www-form-urlencoded"
        else:
            data = json.dumps(body).encode("utf-8")
            hdrs["Content-Type"] = "application/json"
    if headers:
        hdrs.update(headers)
    req = urllib.request.Request(url, data=data, headers=hdrs, method=method)
    try:
        with urllib.request.urlopen(req, timeout=timeout, context=ssl_context()) as resp:
            raw = resp.read()
            note_server_date(resp.headers)
    except urllib.error.HTTPError as e:
        note_server_date(e.headers)
        raise classify_http_error(e)
    except (urllib.error.URLError, OSError, ValueError, http.client.HTTPException) as e:
        raise OnlineError("offline", str(e))
    if not raw:
        return {}
    try:
        return json.loads(raw.decode("utf-8"))
    except ValueError:
        raise OnlineError("server", "invalid response")


# ---- valores do Firestore (formato REST) ----
def fs_encode(v):
    if isinstance(v, bool):
        return {"booleanValue": v}
    if isinstance(v, int):
        return {"integerValue": str(v)}
    if isinstance(v, float):
        if v != v:
            v = 0.0
        v = max(-1.7e308, min(1.7e308, v))       # o JSON não aceita inf / nan
        return {"doubleValue": v}
    return {"stringValue": str(v)}


def fs_decode(value):
    if not isinstance(value, dict):
        return None
    if "stringValue" in value:
        return value["stringValue"]
    if "doubleValue" in value:
        return float(value["doubleValue"])
    if "integerValue" in value:
        return int(value["integerValue"])
    if "booleanValue" in value:
        return bool(value["booleanValue"])
    if "timestampValue" in value:
        return value["timestampValue"]
    return None


def iso_timestamp(secs):
    """segundos desde a epoca -> '2026-09-22T12:34:56Z' (o formato que o Firestore aceita)."""
    return time.strftime("%Y-%m-%dT%H:%M:%SZ", time.gmtime(float(secs)))


def fs_fields(doc):
    return {k: fs_decode(v) for k, v in (doc.get("fields") or {}).items()}


def new_doc_id():
    """Id novo para um documento (o mesmo formato que o Firestore usa: 20 caracteres)."""
    return uuid.uuid4().hex[:20]


def event_from_doc(doc):
    """Documento /events/current -> {"kind", "mult", "ends_at", "by"} (ends_at em segundos)."""
    f = fs_fields(doc)
    ends = f.get("ends_at")
    if isinstance(ends, str):
        ends = parse_timestamp(ends)
    return {
        "kind": f.get("kind") or "luck",
        "mult": float(f.get("mult") or 1.0),
        "ends_at": ends or 0.0,
        "by": f.get("by") or "",
    }


def feedback_from_doc(doc):
    """/feedback/{uid} -> {"uid", "username", "text", "updated_at"}."""
    f = fs_fields(doc)
    return {"uid": str(doc.get("name", "")).rsplit("/", 1)[-1],
            "username": str(f.get("username") or "?"),
            "text": str(f.get("text") or ""),
            "updated_at": parse_timestamp(f.get("updated_at"))}


def profile_from_doc(doc):
    """/profiles/{uid} -> {"uid", "username", "avatar_pet" (None = sem foto), "avatar_mut"}."""
    f = fs_fields(doc)
    pet = f.get("avatar_pet")
    try:
        pet = int(pet)
    except (TypeError, ValueError):
        pet = -1
    return {"uid": str(doc.get("name", "")).rsplit("/", 1)[-1],
            "username": str(f.get("username") or "?"),
            "avatar_pet": None if pet < 0 else pet,
            "avatar_mut": str(f.get("avatar_mut") or "normal")}


def save_summary(d):
    """Resumo de um save (dict do GameState.to_dict) para mostrar no ecrã de Saves e na leaderboard."""
    def num(key):
        try:
            v = float(d.get(key, 0.0))
        except (TypeError, ValueError, AttributeError):
            return 0.0
        return v if v == v else 0.0
    coins = num("coins")
    return {"coins": coins, "total_earned": max(coins, num("total_coins_earned")),
            "playtime": max(0.0, num("playtime")), "rolls": int(max(0.0, min(num("total_rolls"), 9e18))),
            "rebirths": int(max(0.0, min(num("rebirths"), 1e9)))}


class FirebaseClient:
    """Cliente mínimo do Firebase Auth + Firestore (REST). Seguro para usar a partir de threads."""

    SUMMARY_FIELDS = ("coins", "total_earned", "playtime", "rolls", "rebirths")

    def __init__(self, api_key, project_id,
                 identity_base="https://identitytoolkit.googleapis.com/v1",
                 token_base="https://securetoken.googleapis.com/v1",
                 firestore_base="https://firestore.googleapis.com/v1"):
        self.api_key = api_key
        self.project_id = project_id
        self.identity_base = identity_base
        self.token_base = token_base
        self.firestore_base = firestore_base
        self.docs_root = "projects/%s/databases/(default)/documents" % project_id
        self._lock = threading.RLock()
        self.uid = None
        self.username = None
        self.refresh_token = None
        self.id_token = None
        self.expires_at = 0.0

    @property
    def signed_in(self):
        return bool(self.uid and self.refresh_token)

    # ---- contas ----
    def _set_tokens(self, uid, username, refresh_token, id_token, expires_in):
        with self._lock:
            self.uid = uid
            self.username = username
            self.refresh_token = refresh_token
            self.id_token = id_token
            try:
                self.expires_at = time.time() + int(expires_in)
            except (TypeError, ValueError):
                self.expires_at = time.time() + 3000

    def _auth_call(self, action, email, password, username):
        body = {"email": email, "password": password, "returnSecureToken": True}
        url = "%s/accounts:%s?key=%s" % (self.identity_base, action, urllib.parse.quote(self.api_key))
        r = http_json("POST", url, body)
        try:
            self._set_tokens(r["localId"], username, r["refreshToken"], r["idToken"], r.get("expiresIn", "3600"))
        except (KeyError, TypeError):
            raise OnlineError("server", "unexpected login response")

    def sign_up(self, username, password):
        """Conta nova: o email de login no Firebase continua a ser o falso (<username>@...invalid) até
        o jogador verificar um email a sério (ver confirm_email / change_login_email)."""
        self._auth_call("signUp", username_to_email(username), password, username)

    def resolve_login_email(self, username):
        """O email de login ATUAL desta conta: o de verdade, se já tiver sido associado e verificado
        (ver /usernames/{username} no Firestore), ou o falso de sempre, para contas antigas/não migradas."""
        try:
            doc = self._fs("GET", "/usernames/%s" % username)
        except OnlineError as e:
            if e.code == "not_found":
                return username_to_email(username)
            raise
        email = fs_fields(doc).get("email")
        return email if email else username_to_email(username)

    def sign_in(self, username, password):
        email = self.resolve_login_email(username)
        self._auth_call("signInWithPassword", email, password, username)

    def find_username_by_email(self, email):
        """Procura no /usernames o username cujo email de login ATUAL é este. Serve para o ecrã de
        "Log in" poder pedir o email em vez do username: continuamos, por baixo, a autenticar-nos por
        username como sempre - só invertemos a pesquisa. Leitura pública (mesma regra do resolve_login_email),
        por isso funciona mesmo antes de teres sessão."""
        body = {"structuredQuery": {
            "from": [{"collectionId": "usernames"}],
            "where": {"fieldFilter": {"field": {"fieldPath": "email"}, "op": "EQUAL",
                                      "value": {"stringValue": email}}},
            "limit": 1,
        }}
        url = "%s/%s:runQuery?key=%s" % (self.firestore_base, self.docs_root, urllib.parse.quote(self.api_key))
        results = http_json("POST", url, body)
        for item in (results or []):
            doc = item.get("document") if isinstance(item, dict) else None
            if doc and doc.get("name"):
                return doc["name"].rsplit("/", 1)[-1]      # o id do documento = o username
        return None

    def sign_in_by_email(self, email, password):
        """Login "normal" (ecrã Log in): pede o email, mas resolve para o username e faz o login de
        sempre. Só funciona para contas já migradas (com email confirmado) - é para essas que existe
        uma entrada em /usernames. Contas antigas sem email continuam a entrar pelo separador "Old account"."""
        username = self.find_username_by_email(email)
        if not username:
            raise OnlineError("bad_request", "EMAIL_NOT_FOUND")
        self.sign_in(username, password)
        return username

    def reauth_pending_link(self, username, password):
        """Renova o login a meio da associação de email (ver change_login_email). Usa SEMPRE o email
        falso interno, nunca resolve_login_email: nesta fase o Firestore (/usernames) já pode estar a
        apontar para o email novo (publish_username_email corre antes de change_login_email), mas o
        Firebase Auth só passa a aceitar esse email novo se change_login_email tiver mesmo sucedido -
        e é precisamente porque isso falhou que aqui estamos."""
        self._auth_call("signInWithPassword", username_to_email(username), password, username)

    # ---- email a sério: verificação por código + troca do email de login ----
    def get_profile(self):
        """{"email", "email_verified", "pending_code", "code_created"} desta conta, ou None se ainda
        não existir (conta antiga que nunca associou um email)."""
        uid = self._need_uid()
        try:
            doc = self._fs("GET", "/users/%s/account/profile" % uid)
        except OnlineError as e:
            if e.code == "not_found":
                return None
            raise
        f = fs_fields(doc)
        return {"email": f.get("email"), "email_verified": bool(f.get("email_verified")),
                "pending_code": f.get("pending_code") or None,
                "code_created": parse_timestamp(f.get("code_created"))}

    def set_pending_code(self, email, code):
        """Grava o email (ainda por confirmar) e o código de 6 dígitos pendente. A hora do código é a
        do SERVIDOR (para a expiração não depender do relógio do jogador)."""
        uid = self._need_uid()
        name = "%s/users/%s/account/profile" % (self.docs_root, uid)
        body = {"writes": [{
            "update": {"name": name, "fields": {
                "email": fs_encode(email), "email_verified": fs_encode(False), "pending_code": fs_encode(str(code)),
            }},
            "updateTransforms": [{"fieldPath": "code_created", "setToServerValue": "REQUEST_TIME"}],
        }]}
        self._fs("POST", ":commit", body)

    def confirm_email(self):
        """Marca o email como verificado e apaga o código (já usado)."""
        uid = self._need_uid()
        name = "%s/users/%s/account/profile" % (self.docs_root, uid)
        body = {"writes": [{
            "update": {"name": name, "fields": {"email_verified": fs_encode(True), "pending_code": fs_encode("")}},
            "updateMask": {"fieldPaths": ["email_verified", "pending_code"]},
        }]}
        self._fs("POST", ":commit", body)

    def publish_username_email(self, username, email):
        """/usernames/{username} = o email de login ATUAL desta conta (para o login por username
        continuar a funcionar depois de troc-armos o email de verdade no Auth). Escrito ENQUANTO o
        token ainda tem o email falso: é isso que as rules usam para confirmar que o dono é mesmo quem
        diz ser (só quem está autenticado como <username>@...invalid pode escrever aqui)."""
        uid = self._need_uid()
        name = "%s/usernames/%s" % (self.docs_root, username)
        body = {"writes": [{"update": {"name": name, "fields": {"uid": fs_encode(uid), "email": fs_encode(email)}}}]}
        self._fs("POST", ":commit", body)

    def change_login_email(self, new_email):
        """Troca o email de login desta conta no Firebase Auth (de <username>@...invalid para o email
        de verdade, já verificado pelo nosso código). Faz-se DEPOIS do publish_username_email."""
        uid = self._need_uid()
        url = "%s/accounts:update?key=%s" % (self.identity_base, urllib.parse.quote(self.api_key))
        body = {"idToken": self.ensure_token(), "email": new_email, "returnSecureToken": True}
        r = http_json("POST", url, body)
        try:
            self._set_tokens(uid, self.username, r.get("refreshToken", self.refresh_token),
                             r["idToken"], r.get("expiresIn", "3600"))
        except (KeyError, TypeError):
            raise OnlineError("server", "unexpected update response")

    def send_password_reset(self, email):
        """Pede ao Firebase para mandar o EMAIL OFICIAL de reset de password (link seguro, gerado e
        validado só por eles) para este endereço."""
        url = "%s/accounts:sendOobCode?key=%s" % (self.identity_base, urllib.parse.quote(self.api_key))
        http_json("POST", url, {"requestType": "PASSWORD_RESET", "email": email})

    def restore(self, uid, username, refresh_token):
        with self._lock:
            self.uid = uid
            self.username = username
            self.refresh_token = refresh_token
            self.id_token = None
            self.expires_at = 0.0

    def sign_out(self):
        with self._lock:
            self.uid = self.username = self.refresh_token = self.id_token = None
            self.expires_at = 0.0

    def ensure_token(self, force=False):
        with self._lock:
            if not self.refresh_token:
                raise OnlineError("auth", "not signed in")
            if not force and self.id_token and time.time() < self.expires_at - 60:
                return self.id_token
            url = "%s/token?key=%s" % (self.token_base, urllib.parse.quote(self.api_key))
            try:
                r = http_json("POST", url, {"grant_type": "refresh_token",
                                            "refresh_token": self.refresh_token}, form=True)
            except OnlineError as e:
                if e.code in ("bad_request", "auth", "denied"):
                    raise OnlineError("auth", e.message, e.status)
                raise
            try:
                self.id_token = r["id_token"]
            except (KeyError, TypeError):
                raise OnlineError("server", "unexpected token response")
            self.refresh_token = r.get("refresh_token", self.refresh_token)
            self.uid = r.get("user_id", self.uid)
            try:
                self.expires_at = time.time() + int(r.get("expires_in", 3600))
            except (TypeError, ValueError):
                self.expires_at = time.time() + 3000
            return self.id_token

    # ---- Firestore ----
    def _fs(self, method, suffix, body=None, params=None):
        query = list(params or [])
        query.append(("key", self.api_key))
        headers = {}
        if self.signed_in:
            headers["Authorization"] = "Bearer " + self.ensure_token()
        url = "%s/%s%s?%s" % (self.firestore_base, self.docs_root, suffix, urllib.parse.urlencode(query))
        return http_json(method, url, body, headers)

    def _need_uid(self):
        if not self.signed_in:
            raise OnlineError("auth", "not signed in")
        return self.uid

    def put_save(self, slot, state_dict, base_time):
        """Envia o save. base_time = "updateTime" da cloud em que este estado se baseia (None = ainda não
        existe). Se a cloud mudou entretanto (outro PC), o servidor recusa -> OnlineError('conflict')
        e NADA é sobrescrito."""
        uid = self._need_uid()
        fields = dict(save_summary(state_dict))
        fields["data"] = json.dumps(state_dict)
        params = [("updateMask.fieldPaths", k) for k in sorted(fields)]
        if base_time:
            params.append(("currentDocument.updateTime", base_time))
        else:
            params.append(("currentDocument.exists", "false"))
        body = {"fields": {k: fs_encode(v) for k, v in fields.items()}}
        doc = self._fs("PATCH", "/users/%s/saves/slot%d" % (uid, slot), body, params)
        return doc.get("updateTime")

    def get_save(self, slot):
        uid = self._need_uid()
        try:
            doc = self._fs("GET", "/users/%s/saves/slot%d" % (uid, slot))
        except OnlineError as e:
            if e.code == "not_found":
                return None
            raise
        f = fs_fields(doc)
        try:
            data = json.loads(f["data"])
            if not isinstance(data, dict):
                raise ValueError("not a dict")
        except (KeyError, ValueError, TypeError):
            raise OnlineError("server", "the cloud save is unreadable")
        return {"data": data, "time": doc.get("updateTime")}

    def list_saves(self):
        uid = self._need_uid()
        params = [("mask.fieldPaths", k) for k in self.SUMMARY_FIELDS] + [("pageSize", "10")]
        res = self._fs("GET", "/users/%s/saves" % uid, None, params)
        out = {}
        for doc in res.get("documents", []) if isinstance(res, dict) else []:
            name = str(doc.get("name", "")).rsplit("/", 1)[-1]
            if not (name.startswith("slot") and name[4:].isdigit()):
                continue
            f = fs_fields(doc)
            out[int(name[4:])] = {"coins": float(f.get("coins") or 0.0),
                                  "total_earned": float(f.get("total_earned") or 0.0),
                                  "playtime": float(f.get("playtime") or 0.0),
                                  "rolls": int(f.get("rolls") or 0),
                                  "rebirths": int(f.get("rebirths") or 0),      # saves antigos na cloud não têm este campo
                                  "time": doc.get("updateTime")}
        return out

    def delete_save(self, slot):
        uid = self._need_uid()
        try:
            self._fs("DELETE", "/users/%s/saves/slot%d" % (uid, slot))
        except OnlineError as e:
            if e.code != "not_found":
                raise

    # ---- sessão: uma conta, um jogo de cada vez ----
    def read_session(self):
        """A marca de "estou a jogar" da conta, ou None se não existir."""
        uid = self._need_uid()
        try:
            doc = self._fs("GET", "/users/%s/session/current" % uid)
        except OnlineError as e:
            if e.code == "not_found":
                return None
            raise
        f = fs_fields(doc)
        return {"session_id": str(f.get("session_id") or ""),
                "active": bool(f.get("active")),
                "heartbeat": parse_timestamp(f.get("heartbeat"))}

    def session_blocker(self, session_id):
        """A marca de OUTRO PC que ainda está a jogar esta conta, ou None se o caminho estiver livre.
        Na dúvida devolve None: mais vale deixar jogar do que trancar alguém fora da própria conta."""
        lock = self.read_session()
        if not lock or lock["session_id"] == session_id or not lock["active"]:
            return None
        if lock["heartbeat"] is None or not server_clock_synced():
            return None
        if server_now() - lock["heartbeat"] > SESSION_STALE:
            return None                                  # o outro PC desapareceu sem se despedir
        return lock

    def write_session(self, session_id, active):
        """Marca (ou larga) a conta como "a jogar neste PC". A hora é posta pelo SERVIDOR, para não
        haver forma de fingir uma marca fresca nem uma marca velha."""
        uid = self._need_uid()
        name = "%s/users/%s/session/current" % (self.docs_root, uid)
        body = {"writes": [{
            "update": {"name": name, "fields": {"session_id": fs_encode(str(session_id)),
                                                "active": fs_encode(bool(active))}},
            "updateTransforms": [{"fieldPath": "heartbeat", "setToServerValue": "REQUEST_TIME"}],
        }]}
        self._fs("POST", ":commit", body)

    # ---- leaderboard ----
    def publish_score(self, coins, playtime, rolls=0, rebirths=0):
        uid = self._need_uid()
        name = "%s/leaderboard/%s" % (self.docs_root, uid)
        # rolls e rebirths vão como inteiros (as regras do Firestore exigem "is int")
        rolls = int(max(0, min(rolls, 9e18)))
        rebirths = int(max(0, min(rebirths, 1e9)))
        body = {"writes": [{
            "update": {"name": name, "fields": {
                "username": fs_encode(self.username),
                "coins": fs_encode(float(coins)),
                "playtime": fs_encode(float(playtime)),
                "rolls": fs_encode(rolls),
                "rebirths": fs_encode(rebirths),
            }},
            "updateTransforms": [{"fieldPath": "updated_at", "setToServerValue": "REQUEST_TIME"}],
        }]}
        self._fs("POST", ":commit", body)

    # ---- perfil público (avatar) e amigos ----
    # /profiles/{uid} = a parte pública da conta: username + a foto (um verity que o jogador tem).
    # Fica à parte de /usernames/{username} de propósito: esse documento guarda o email de login e
    # ninguém além do dono tem de o poder ler para procurar um amigo.
    def publish_profile(self, avatar_pet=None, avatar_mut="normal"):
        uid = self._need_uid()
        name = "%s/profiles/%s" % (self.docs_root, uid)
        body = {"writes": [{
            "update": {"name": name, "fields": {
                "username": fs_encode(self.username or ""),
                # -1 = sem foto escolhida (ainda não rolou nenhum verity, ou tirou a foto)
                "avatar_pet": fs_encode(-1 if avatar_pet is None else int(avatar_pet)),
                "avatar_mut": fs_encode(str(avatar_mut or "normal")),
            }},
            "updateTransforms": [{"fieldPath": "updated_at", "setToServerValue": "REQUEST_TIME"}],
        }]}
        self._fs("POST", ":commit", body)

    def find_profile(self, username):
        """O perfil público de quem tem este username, ou None se não existir."""
        query = {
            "from": [{"collectionId": "profiles"}],
            "where": {"fieldFilter": {"field": {"fieldPath": "username"}, "op": "EQUAL",
                                      "value": fs_encode(str(username))}},
            "limit": 1,
        }
        res = self._fs("POST", ":runQuery", {"structuredQuery": query})
        for item in res if isinstance(res, list) else []:
            doc = item.get("document") if isinstance(item, dict) else None
            if doc:
                return profile_from_doc(doc)
        # Sem perfil publicado (a conta ainda nao abriu a pagina dos Amigos nesta versao): procura-se
        # na leaderboard, que tambem e publica e tem o username, e so depois no mapa /usernames.
        return self._find_profile_fallback(username)

    def _find_profile_fallback(self, username):
        query = {
            "from": [{"collectionId": "leaderboard"}],
            "where": {"fieldFilter": {"field": {"fieldPath": "username"}, "op": "EQUAL",
                                      "value": fs_encode(str(username))}},
            "limit": 1,
        }
        uid = ""
        try:
            res = self._fs("POST", ":runQuery", {"structuredQuery": query})
            for item in res if isinstance(res, list) else []:
                doc = item.get("document") if isinstance(item, dict) else None
                if doc:
                    uid = str(doc.get("name", "")).rsplit("/", 1)[-1]
                    break
        except OnlineError:
            uid = ""
        if not uid:
            # Ultimo recurso: /usernames/{username}. Este documento tambem guarda o email de login da
            # conta - le-se so o "uid" e mais nada, que o email nunca sai daqui.
            try:
                doc = self._fs("GET", "/usernames/%s" % username)
            except OnlineError:
                return None
            uid = str(fs_fields(doc).get("uid") or "")
        if not uid:
            return None
        return {"uid": uid, "username": str(username), "avatar_pet": None, "avatar_mut": "normal"}

    def get_public_profile(self, uid):
        try:
            doc = self._fs("GET", "/profiles/%s" % uid)
        except OnlineError as e:
            if e.code == "not_found":
                return None
            raise
        return profile_from_doc(doc)

    def get_public_stats(self, uid):
        """A linha da leaderboard de outra conta (é o que se mostra nas stats de um amigo).
        None se essa conta ainda nunca publicou."""
        try:
            doc = self._fs("GET", "/leaderboard/%s" % uid)
        except OnlineError as e:
            if e.code == "not_found":
                return None
            raise
        f = fs_fields(doc)
        return {"username": str(f.get("username") or ""),
                "coins": float(f.get("coins") or 0.0),
                "playtime": float(f.get("playtime") or 0.0),
                "rolls": int(f.get("rolls") or 0),
                "rebirths": int(f.get("rebirths") or 0),
                "updated_at": parse_timestamp(f.get("updated_at"))}

    def _request_path(self, to_uid, from_uid):
        # o id do pedido é "destino__origem": assim o mesmo pedido nunca aparece duas vezes
        return "/friend_requests/%s__%s" % (to_uid, from_uid)

    def send_friend_request(self, to_uid, to_username):
        uid = self._need_uid()
        if to_uid == uid:
            raise OnlineError("bad_request", "that is your own account")
        name = self.docs_root + self._request_path(to_uid, uid)
        body = {"writes": [{
            "update": {"name": name, "fields": {
                "from_uid": fs_encode(uid), "from_username": fs_encode(self.username or ""),
                "to_uid": fs_encode(to_uid), "to_username": fs_encode(str(to_username or "")),
            }},
            "updateTransforms": [{"fieldPath": "created_at", "setToServerValue": "REQUEST_TIME"}],
        }]}
        self._fs("POST", ":commit", body)

    def list_friend_requests(self, incoming=True, limit=60):
        """Pedidos recebidos (incoming=True) ou enviados. Sem orderBy de propósito: com o filtro por
        uid, ordenar por created_at obrigaria a criar um índice composto no Firestore."""
        uid = self._need_uid()
        field = "to_uid" if incoming else "from_uid"
        query = {
            "from": [{"collectionId": "friend_requests"}],
            "where": {"fieldFilter": {"field": {"fieldPath": field}, "op": "EQUAL", "value": fs_encode(uid)}},
            "limit": limit,
        }
        res = self._fs("POST", ":runQuery", {"structuredQuery": query})
        out = []
        for item in res if isinstance(res, list) else []:
            doc = item.get("document") if isinstance(item, dict) else None
            if not doc:
                continue
            f = fs_fields(doc)
            other_uid = f.get("from_uid") if incoming else f.get("to_uid")
            other_name = f.get("from_username") if incoming else f.get("to_username")
            if not other_uid:
                continue
            out.append({"uid": str(other_uid), "username": str(other_name or "?"),
                        "created_at": parse_timestamp(f.get("created_at"))})
        return out

    def delete_friend_request(self, to_uid, from_uid):
        try:
            self._fs("DELETE", self._request_path(to_uid, from_uid))
        except OnlineError as e:
            if e.code != "not_found":
                raise

    def accept_friend_request(self, from_uid, from_username):
        """Aceitar: as duas listas de amigos e o apagar do pedido vão num único :commit (ou acontece
        tudo, ou não acontece nada - nunca fica um amigo só de um lado)."""
        uid = self._need_uid()
        mine = "%s/users/%s/friends/%s" % (self.docs_root, uid, from_uid)
        theirs = "%s/users/%s/friends/%s" % (self.docs_root, from_uid, uid)
        body = {"writes": [
            {"update": {"name": mine, "fields": {"username": fs_encode(str(from_username or "?"))}},
             "updateTransforms": [{"fieldPath": "since", "setToServerValue": "REQUEST_TIME"}]},
            {"update": {"name": theirs, "fields": {"username": fs_encode(self.username or "")}},
             "updateTransforms": [{"fieldPath": "since", "setToServerValue": "REQUEST_TIME"}]},
            {"delete": self.docs_root + self._request_path(uid, from_uid)},
        ]}
        self._fs("POST", ":commit", body)

    def list_friends(self, limit=200):
        uid = self._need_uid()
        res = self._fs("GET", "/users/%s/friends" % uid, None, [("pageSize", str(limit))])
        out = []
        for doc in (res.get("documents", []) if isinstance(res, dict) else []):
            fuid = str(doc.get("name", "")).rsplit("/", 1)[-1]
            if not fuid:
                continue
            f = fs_fields(doc)
            out.append({"uid": fuid, "username": str(f.get("username") or "?"),
                        "since": parse_timestamp(f.get("since"))})
        out.sort(key=lambda e: e["username"])
        return out

    def remove_friend(self, friend_uid):
        """Apaga dos dois lados: um amigo que só desaparece de uma das listas ficava lá para sempre."""
        uid = self._need_uid()
        body = {"writes": [
            {"delete": "%s/users/%s/friends/%s" % (self.docs_root, uid, friend_uid)},
            {"delete": "%s/users/%s/friends/%s" % (self.docs_root, friend_uid, uid)},
        ]}
        self._fs("POST", ":commit", body)

    # ---------------------------------------------------------------- chat entre amigos
    def chat_id(self, other_uid):
        """Id da conversa: os dois uid por ordem alfabética, separados por "__". Assim os dois lados
        chegam sempre ao mesmo sítio e as regras conseguem confirmar quem lá pode entrar."""
        uid = self._need_uid()
        a, b = sorted([str(uid), str(other_uid)])
        return "%s__%s" % (a, b)

    def send_message(self, other_uid, text):
        cid = self.chat_id(other_uid)
        uid = self._need_uid()
        name = "%s/chats/%s/messages" % (self.docs_root, cid)
        body = {"writes": [
            {"update": {"name": "%s/%s" % (name, new_doc_id()), "fields": {
                "from_uid": fs_encode(uid),
                "to_uid": fs_encode(str(other_uid)),
                "text": fs_encode(str(text)),
            }},
             "updateTransforms": [{"fieldPath": "sent_at", "setToServerValue": "REQUEST_TIME"}]},
            # resumo da conversa: é só isto que se lê para saber se há mensagem por ver
            # (senão era preciso ler as mensagens de todos os amigos)
            {"update": {"name": "%s/chats/%s" % (self.docs_root, cid), "fields": {
                "last_from": fs_encode(uid),
            }},
             "updateTransforms": [{"fieldPath": "last_at", "setToServerValue": "REQUEST_TIME"}]},
        ]}
        self._fs("POST", ":commit", body)

    def get_chat_summary(self, other_uid):
        """{"last_at", "last_from"} da conversa com este amigo, ou None se nunca falaram."""
        cid = self.chat_id(other_uid)
        try:
            doc = self._fs("GET", "/chats/%s" % cid)
        except OnlineError as e:
            if e.code == "not_found":
                return None
            raise
        f = fs_fields(doc)
        return {"last_at": parse_timestamp(f.get("last_at")), "last_from": str(f.get("last_from") or "")}

    def list_messages(self, other_uid, limit=50):
        """As mensagens mais recentes desta conversa, da mais antiga para a mais nova."""
        cid = self.chat_id(other_uid)
        query = {
            "from": [{"collectionId": "messages"}],
            "orderBy": [{"field": {"fieldPath": "sent_at"}, "direction": "DESCENDING"}],
            "limit": int(limit),
        }
        res = self._fs("POST", "/chats/%s:runQuery" % cid, {"structuredQuery": query})
        out = []
        for item in res if isinstance(res, list) else []:
            doc = item.get("document") if isinstance(item, dict) else None
            if not doc:
                continue
            f = fs_fields(doc)
            out.append({"id": str(doc.get("name", "")).rsplit("/", 1)[-1],
                        "from_uid": str(f.get("from_uid") or ""),
                        "text": str(f.get("text") or ""),
                        "sent_at": parse_timestamp(f.get("sent_at"))})
        out.reverse()
        return out

    # ---------------------------------------------------------------- feedback (um por conta)
    # ================================================================ eventos globais (admin)
    def is_admin(self):
        """True se esta conta tem um documento em /admins/{uid}. Quem pode comecar eventos globais
        decide-se no Console (basta criar/apagar o documento), nunca no codigo do jogo."""
        uid = self._need_uid()
        try:
            self._fs("GET", "/admins/%s" % uid)
        except OnlineError as e:
            if e.code == "not_found":
                return False
            raise
        return True

    def get_event(self):
        """O evento global a decorrer, ou None se nao houver nenhum."""
        try:
            doc = self._fs("GET", "/events/current")
        except OnlineError as e:
            if e.code == "not_found":
                return None
            raise
        return event_from_doc(doc)

    def start_event(self, kind, mult, seconds):
        """Comeca (ou substitui) o evento global. So passa se as regras deixarem este uid escrever."""
        uid = self._need_uid()
        ends = time.time() + float(seconds)
        fields = {
            "kind": fs_encode(str(kind)),
            "mult": fs_encode(float(mult)),
            "by": fs_encode(self.username or uid),
            "ends_at": {"timestampValue": iso_timestamp(ends)},
        }
        params = [("updateMask.fieldPaths", k) for k in sorted(fields)]
        self._fs("PATCH", "/events/current", {"fields": fields}, params)
        return {"kind": str(kind), "mult": float(mult), "ends_at": ends,
                "by": self.username or uid}

    def stop_event(self):
        try:
            self._fs("DELETE", "/events/current")
        except OnlineError as e:
            if e.code != "not_found":
                raise

    def publish_feedback(self, text):
        """Cria ou reescreve o feedback desta conta. O id do documento e o uid, por isso cada conta
        so pode ter um; apagar e escrever outra vez volta a criar."""
        uid = self._need_uid()
        name = "%s/feedback/%s" % (self.docs_root, uid)
        body = {"writes": [{
            "update": {"name": name, "fields": {
                "username": fs_encode(self.username or ""),
                "text": fs_encode(str(text)),
            }},
            "updateTransforms": [{"fieldPath": "updated_at", "setToServerValue": "REQUEST_TIME"}],
        }]}
        self._fs("POST", ":commit", body)

    def get_my_feedback(self):
        uid = self._need_uid()
        try:
            doc = self._fs("GET", "/feedback/%s" % uid)
        except OnlineError as e:
            if e.code == "not_found":
                return None
            raise
        return feedback_from_doc(doc)

    def delete_feedback(self):
        uid = self._need_uid()
        try:
            self._fs("DELETE", "/feedback/%s" % uid)
        except OnlineError as e:
            if e.code != "not_found":
                raise

    def list_feedback(self, limit=60):
        """Os feedbacks mais recentes (a pagina mostra-os a toda a gente)."""
        query = {
            "from": [{"collectionId": "feedback"}],
            "orderBy": [{"field": {"fieldPath": "updated_at"}, "direction": "DESCENDING"}],
            "limit": int(limit),
        }
        res = self._fs("POST", ":runQuery", {"structuredQuery": query})
        out = []
        for item in res if isinstance(res, list) else []:
            doc = item.get("document") if isinstance(item, dict) else None
            if doc:
                out.append(feedback_from_doc(doc))
        return out

    def top_entries(self, field, limit, min_value=None):
        """Top 'limit' contas por 'field'. min_value: ignora quem tem menos que isto (ex.: rebirths >= 1,
        para a tabela não se encher de zeros). Contas antigas sem o campo nunca aparecem."""
        query = {
            "from": [{"collectionId": "leaderboard"}],
            "orderBy": [{"field": {"fieldPath": field}, "direction": "DESCENDING"}],
            "limit": limit,
        }
        if min_value is not None:
            query["where"] = {"fieldFilter": {"field": {"fieldPath": field}, "op": "GREATER_THAN_OR_EQUAL",
                                              "value": fs_encode(min_value)}}
        res = self._fs("POST", ":runQuery", {"structuredQuery": query})
        entries = []
        for item in res if isinstance(res, list) else []:
            doc = item.get("document") if isinstance(item, dict) else None
            if not doc:
                continue
            f = fs_fields(doc)
            try:
                entries.append({"username": str(f.get("username", "?")), "value": float(f.get(field) or 0.0)})
            except (TypeError, ValueError):
                continue
        return entries


class Worker:
    """Corre as chamadas de rede numa thread (o jogo nunca congela) e entrega o resultado ao loop
    principal: os callbacks on_ok / on_err correm sempre na thread principal, dentro de poll()."""

    def __init__(self):
        self.done = queue.Queue()

    def run(self, fn, on_ok=None, on_err=None):
        def target():
            try:
                res = fn()
            except OnlineError as e:
                self.done.put((on_err, e))
            except Exception as e:      # qualquer outro erro vira um erro "server" em vez de rebentar a thread
                self.done.put((on_err, OnlineError("server", str(e))))
            else:
                self.done.put((on_ok, res))
        threading.Thread(target=target, daemon=True).start()

    def poll(self):
        while True:
            try:
                cb, val = self.done.get_nowait()
            except queue.Empty:
                return
            if cb is None:
                continue
            try:
                cb(val)
            except Exception:
                traceback.print_exc()
