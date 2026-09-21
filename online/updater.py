"""Atualizações automáticas do jogo (só nos programas empacotados: .exe / .app / binário de Linux).

COMO FUNCIONA
  1. Ao abrir (e de 30 em 30 minutos) o jogo pergunta ao GitHub qual é a última Release de UPDATE_REPO.
  2. Se o número dessa versão for MAIOR que o do jogo (BUILD_VERSION, escrito pelo GitHub Actions a partir da
     tag), aparece o ecrã "Nova versão" com [Atualizar agora] e [Não (fecha o jogo)].
  3. Atualizar = descarregar o ficheiro do sistema, confirmar que veio inteiro (tamanho + SHA-256), trocar o
     programa pelo novo e voltar a abri-lo.

NUNCA DEIXA NINGUÉM PRESO
  - sem internet / GitHub em baixo / Release ainda sem o ficheiro deste sistema  =>  NÃO bloqueia;
  - "python main.py" e programas feitos à mão (sem BUILD_VERSION) nunca pedem atualização;
  - só se atualiza para uma versão MAIOR (nunca para trás), por isso não há ciclos de atualização.

Este ficheiro não usa o pygame: dá para testá-lo sozinho (ver as funções fetch_latest / prepare / launch).
"""

import errno
import hashlib
import http.client
import json
import os
import platform
import re
import shutil
import subprocess
import sys
import tarfile
import tempfile
import urllib.error
import urllib.request

import config
from online.tls import ssl_context

API_BASE = "https://api.github.com"
API_TIMEOUT = 10.0
DOWNLOAD_TIMEOUT = 30.0
USER_AGENT = "LuckySahurs-Updater"

# Nomes dos ficheiros de cada Release (têm de ser iguais aos do .github/workflows/build.yml).
ASSET_WINDOWS = "Lucky-Sahurs-Windows.exe"
ASSET_LINUX = "Lucky-Sahurs-Linux.tar.gz"
ASSET_MAC_ARM = "Lucky-Sahurs-macOS-AppleSilicon.zip"
ASSET_MAC_INTEL = "Lucky-Sahurs-macOS-Intel.zip"
LINUX_BINARY_NAME = "LuckySahurs"      # o ficheiro que vai dentro do .tar.gz
MAC_APP_NAME = "Lucky Sahurs.app"      # a pasta que vai dentro do .zip
WORK_PREFIX = ".lucky-update-"         # pastas de trabalho temporárias (apagadas ao arrancar)


class UpdateError(Exception):
    """Falhou a atualização. code: "net" (rede) | "corrupt" (ficheiro estragado) | "perm" (sem permissão) | "other"."""

    def __init__(self, code, detail=""):
        Exception.__init__(self, "%s: %s" % (code, detail))
        self.code = code
        self.detail = detail


# ---------------------------------------------------------------- versões
_VERSION_RE = re.compile(r"^v?(\d+)(?:\.(\d+))?(?:\.(\d+))?$")


def parse_version(text):
    """'v1.2.3' -> (1, 2, 3); 'v7' -> (7, 0, 0); 'dev' ou lixo -> None."""
    m = _VERSION_RE.match(str(text or "").strip())
    if not m:
        return None
    return tuple(int(g) if g else 0 for g in m.groups())


def is_newer(latest, current):
    """True só se as duas versões forem válidas e 'latest' for MAIOR que 'current'."""
    a, b = parse_version(latest), parse_version(current)
    return a is not None and b is not None and a > b


def updates_enabled(current=None):
    """True só no programa empacotado (PyInstaller) feito a partir de uma tag vX.Y.Z pelo GitHub Actions.
    Para desligar (testes): variável de ambiente LUCKY_SAHURS_NO_UPDATE=1."""
    if os.environ.get("LUCKY_SAHURS_NO_UPDATE"):
        return False
    if config.IS_ANDROID:
        return False        # no Android quem atualiza e a loja / o APK, nao o jogo
    if not getattr(sys, "frozen", False):
        return False
    return parse_version(config.BUILD_VERSION if current is None else current) is not None


def asset_name():
    """O nome do ficheiro da Release que serve a este sistema."""
    if sys.platform == "win32":
        return ASSET_WINDOWS
    if sys.platform == "darwin":
        return ASSET_MAC_ARM if platform.machine().lower() in ("arm64", "aarch64") else ASSET_MAC_INTEL
    return ASSET_LINUX


# ---------------------------------------------------------------- perguntar ao GitHub
def _get(url, timeout, accept):
    req = urllib.request.Request(url, headers={"User-Agent": USER_AGENT, "Accept": accept})
    return urllib.request.urlopen(req, timeout=timeout, context=ssl_context())


def pick_release(data, current, allow_http=False):
    """Da resposta do GitHub (/releases/latest) tira a versão nova para este sistema, ou None."""
    if not isinstance(data, dict) or data.get("draft") or data.get("prerelease"):
        return None
    tag = str(data.get("tag_name") or "")
    if not is_newer(tag, current):
        return None
    wanted = asset_name()
    for a in data.get("assets") or []:
        if not isinstance(a, dict) or a.get("name") != wanted:
            continue
        url = str(a.get("browser_download_url") or "")
        if a.get("state", "uploaded") != "uploaded" or not (url.startswith("https://") or (allow_http and url.startswith("http://"))):
            continue
        return {"tag": tag, "name": wanted, "url": url, "size": int(a.get("size") or 0),
                "digest": a.get("digest"), "page": str(data.get("html_url") or "")}
    return None        # a Release ainda não tem o ficheiro para este sistema (build a meio): não bloqueia


def fetch_latest(current=None, repo=None, api_base=API_BASE, allow_http=False):
    """None = estás na última versão (ou não dá para saber). Um dict = há versão nova:
    {"tag", "name", "url", "size", "digest", "page"}. Levanta UpdateError("net") se o GitHub não respondeu."""
    current = config.BUILD_VERSION if current is None else current
    repo = repo or config.UPDATE_REPO
    url = "%s/repos/%s/releases/latest" % (api_base, repo)
    try:
        with _get(url, API_TIMEOUT, "application/vnd.github+json") as resp:
            data = json.loads(resp.read().decode("utf-8"))
    except urllib.error.HTTPError as e:
        if e.code == 404:                      # o repositório ainda não tem Releases
            return None
        raise UpdateError("net", "HTTP %d" % e.code)        # 403 = limite do GitHub, 5xx = em baixo...
    except (urllib.error.URLError, OSError, ValueError, http.client.HTTPException) as e:
        raise UpdateError("net", str(e))
    return pick_release(data, current, allow_http)


# ---------------------------------------------------------------- descarregar
def _remove(path):
    try:
        os.remove(path)
    except OSError:
        pass


def download(info, dest, progress=None):
    """Descarrega info["url"] para 'dest' (primeiro num .part), confirma o tamanho e o SHA-256 e só então põe
    o ficheiro no sítio. progress(0..1) é chamado enquanto descarrega. Levanta UpdateError."""
    part = dest + ".part"
    try:
        f = open(part, "wb")
    except OSError as e:
        raise UpdateError("perm", str(e))
    h = hashlib.sha256()
    done = 0
    total = 0
    try:
        with f:
            with _get(info["url"], DOWNLOAD_TIMEOUT, "application/octet-stream") as resp:
                total = int(resp.headers.get("Content-Length") or info.get("size") or 0)
                while True:
                    chunk = resp.read(64 * 1024)
                    if not chunk:
                        break
                    f.write(chunk)
                    h.update(chunk)
                    done += len(chunk)
                    if progress and total:
                        progress(min(1.0, done / float(total)))
    except urllib.error.HTTPError as e:
        _remove(part)
        raise UpdateError("net", "HTTP %d" % e.code)
    except (urllib.error.URLError, http.client.HTTPException, OSError) as e:
        _remove(part)
        if isinstance(e, OSError) and getattr(e, "errno", None) in (errno.ENOSPC, errno.EACCES, errno.EPERM):
            raise UpdateError("perm", str(e))
        raise UpdateError("net", str(e))
    if total and done < total:                 # a ligação caiu a meio: é um problema de rede, não de ficheiro
        _remove(part)
        raise UpdateError("net", "descarga incompleta (%d de %d)" % (done, total))
    size = int(info.get("size") or 0)
    if size and done != size:
        _remove(part)
        raise UpdateError("corrupt", "tamanho %d em vez de %d" % (done, size))
    digest = str(info.get("digest") or "")
    if digest.startswith("sha256:") and h.hexdigest() != digest[7:].strip().lower():
        _remove(part)
        raise UpdateError("corrupt", "sha256 diferente")
    if done == 0:
        _remove(part)
        raise UpdateError("corrupt", "ficheiro vazio")
    os.replace(part, dest)
    if progress:
        progress(1.0)


def _check_magic(path, magic):
    """Confirma os primeiros bytes (uma página de erro em HTML nunca passa por um .exe / .zip / .tar.gz)."""
    try:
        with open(path, "rb") as f:
            ok = f.read(len(magic)) == magic
    except OSError as e:
        raise UpdateError("other", str(e))
    if not ok:
        raise UpdateError("corrupt", "formato inesperado")


def _need_writable(folder):
    """A pasta onde está o programa tem de deixar escrever (senão o jogo não consegue trocar-se a si próprio)."""
    try:
        fd, tmp = tempfile.mkstemp(prefix=WORK_PREFIX, dir=folder)
        os.close(fd)
        os.remove(tmp)
    except OSError as e:
        raise UpdateError("perm", str(e))


# ---------------------------------------------------------------- preparar a troca (numa thread de fundo)
def prepare(info, progress=None):
    """Descarrega e prepara a troca do programa. Devolve um "plano" para launch(). Levanta UpdateError."""
    if sys.platform == "win32":
        return _prepare_windows(info, progress)
    if sys.platform == "darwin":
        return _prepare_mac(info, progress)
    return _prepare_linux(info, progress)


def _prepare_windows(info, progress):
    exe = os.path.abspath(sys.executable)
    _need_writable(os.path.dirname(exe))
    new = exe + ".update"
    download(info, new, progress)
    _check_magic(new, b"MZ")
    return {"kind": "windows", "exe": exe, "new": new}


def _prepare_linux(info, progress):
    exe = os.path.abspath(sys.executable)
    folder = os.path.dirname(exe)
    _need_writable(folder)
    work = tempfile.mkdtemp(prefix=WORK_PREFIX, dir=folder)         # na mesma pasta: a troca é atómica
    try:
        archive = os.path.join(work, "update.tar.gz")
        download(info, archive, progress)
        _check_magic(archive, b"\x1f\x8b")
        new = os.path.join(work, LINUX_BINARY_NAME)
        try:
            with tarfile.open(archive, "r:gz") as tar:
                member = tar.getmember(LINUX_BINARY_NAME)
                if not member.isfile() or member.size <= 0:
                    raise UpdateError("corrupt", "conteúdo inesperado")
                with tar.extractfile(member) as src, open(new, "wb") as dst:
                    shutil.copyfileobj(src, dst)
        except (tarfile.TarError, KeyError, EOFError) as e:
            raise UpdateError("corrupt", str(e))
        os.chmod(new, 0o755)
        _check_magic(new, b"\x7fELF")
        try:
            os.replace(new, exe)         # trocar um programa que está a correr é permitido no Linux
        except OSError as e:
            raise UpdateError("perm", str(e))
    finally:
        shutil.rmtree(work, ignore_errors=True)
    return {"kind": "linux", "exe": exe}


def _mac_app_path(exe):
    i = exe.find(".app/Contents/")
    if i < 0:
        raise UpdateError("perm", "não é uma .app")
    return exe[:i + 4]


def _prepare_mac(info, progress):
    exe = os.path.abspath(sys.executable)
    app = _mac_app_path(exe)
    parent = os.path.dirname(app)
    _need_writable(parent)             # falha em apps "translocadas" (abertas de um zip/transferência)
    work = tempfile.mkdtemp(prefix=WORK_PREFIX, dir=parent)
    try:
        archive = os.path.join(work, "update.zip")
        download(info, archive, progress)
        _check_magic(archive, b"PK")
        out = os.path.join(work, "new")
        os.mkdir(out)
        # o "ditto" (do próprio macOS) preserva permissões e ligações simbólicas da .app; o zipfile do Python não
        r = subprocess.run(["ditto", "-x", "-k", archive, out], capture_output=True, timeout=300)
        if r.returncode != 0:
            raise UpdateError("corrupt", r.stderr.decode("utf-8", "replace")[:200])
        new_app = os.path.join(out, MAC_APP_NAME)
        if not os.path.isdir(new_app):
            raise UpdateError("corrupt", "a .app não veio no zip")
    except BaseException:
        shutil.rmtree(work, ignore_errors=True)
        raise
    return {"kind": "mac", "app": app, "new": new_app, "work": work}


# ---------------------------------------------------------------- trocar e reabrir (depois de o jogo fechar)
def clean_env():
    """Ambiente para o programa novo. O PyInstaller deixa variáveis que fariam o programa novo tentar usar a
    pasta temporária do velho (que desaparece ao fechar): tiram-se, e o PYINSTALLER_RESET_ENVIRONMENT diz-lhe
    para arrancar como um programa novo."""
    env = dict(os.environ)
    env["PYINSTALLER_RESET_ENVIRONMENT"] = "1"
    for k in ("_MEIPASS2", "_PYI_ARCHIVE_FILE", "_PYI_APPLICATION_HOME_DIR", "_PYI_PARENT_PROCESS_LEVEL",
              "_PYI_LINUX_PROCESS_NAME"):
        env.pop(k, None)
    for var in ("LD_LIBRARY_PATH", "DYLD_LIBRARY_PATH"):
        orig = env.pop(var + "_ORIG", None)
        if orig is not None:
            env[var] = orig
        elif getattr(sys, "frozen", False):
            env.pop(var, None)
    return env


# .bat: tenta trocar o programa até o jogo fechar (o Windows não deixa trocar um .exe que está a correr) e reabre-o.
# %~1 = programa atual, %~2 = programa novo (vêm como argumentos para funcionar com nomes com acentos).
WINDOWS_SCRIPT = (
    "@echo off\r\n"
    "set /a tries=0\r\n"
    ":retry\r\n"
    "move /y \"%~2\" \"%~1\" >nul 2>&1\r\n"
    "if not exist \"%~2\" goto done\r\n"
    "set /a tries+=1\r\n"
    "if %tries% geq 120 goto done\r\n"
    "ping -n 2 127.0.0.1 >nul\r\n"
    "goto retry\r\n"
    ":done\r\n"
    "cd /d \"%~dp1\"\r\n"
    "start \"\" \"%~1\"\r\n"
    "(goto) 2>nul & del \"%~f0\"\r\n"
)

# Linux: espera que o jogo feche e abre o programa (já trocado). $1 = pid do jogo, $2 = programa
LINUX_SCRIPT = ('while kill -0 "$1" 2>/dev/null; do sleep 0.2; done; '
                'cd "$(dirname "$2")" && exec "$2"')

# macOS: espera que o jogo feche, troca a .app (com marcha-atrás se falhar) e abre-a.
# $1 = pid, $2 = .app atual, $3 = .app nova, $4 = pasta de trabalho
MAC_SCRIPT = (
    'while kill -0 "$1" 2>/dev/null; do sleep 0.3; done; '
    'if mv "$2" "$2.old-update"; then '
    'if mv "$3" "$2"; then rm -rf "$2.old-update"; else mv "$2.old-update" "$2"; fi; '
    'fi; '
    'rm -rf "$4"; '
    'open -n "$2"'
)


def _detached(cmd, env, **kw):
    return subprocess.Popen(cmd, env=env, stdin=subprocess.DEVNULL, stdout=subprocess.DEVNULL,
                            stderr=subprocess.DEVNULL, close_fds=True, **kw)


def launch(plan):
    """Arranca o ajudante que troca o programa depois de o jogo fechar e o reabre. NÃO fecha o jogo: quem
    chama guarda tudo e faz sys.exit() logo a seguir."""
    env = clean_env()
    kind = plan["kind"]
    if kind == "windows":
        folder = tempfile.mkdtemp(prefix="lucky-update-")
        bat = os.path.join(folder, "update.bat")
        with open(bat, "w", encoding="ascii", newline="") as f:
            f.write(WINDOWS_SCRIPT)
        cmd = 'cmd.exe /d /s /c ""%s" "%s" "%s""' % (bat, plan["exe"], plan["new"])
        # 0x08000000 = CREATE_NO_WINDOW, 0x00000200 = CREATE_NEW_PROCESS_GROUP
        return _detached(cmd, env, creationflags=0x08000000 | 0x00000200)
    if kind == "mac":
        return _detached(["/bin/sh", "-c", MAC_SCRIPT, "sh", str(os.getpid()), plan["app"], plan["new"],
                          plan["work"]], env, start_new_session=True)
    return _detached(["/bin/sh", "-c", LINUX_SCRIPT, "sh", str(os.getpid()), plan["exe"]], env,
                     start_new_session=True)


def cleanup_leftovers():
    """Ao arrancar: apaga restos de atualizações que ficaram a meio (ficheiros .update / .part e pastas de trabalho)."""
    try:
        exe = os.path.abspath(sys.executable)
        folders = {os.path.dirname(exe)}
        if sys.platform == "darwin" and ".app/Contents/" in exe:
            folders.add(os.path.dirname(_mac_app_path(exe)))
        for name in (exe + ".update", exe + ".update.part"):
            _remove(name)
        for folder in folders:
            for entry in os.listdir(folder):
                if entry.startswith(WORK_PREFIX):
                    shutil.rmtree(os.path.join(folder, entry), ignore_errors=True)
    except OSError:
        pass
