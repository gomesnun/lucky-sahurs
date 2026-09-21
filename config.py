"""Constantes gerais do jogo (janela, versão, pastas de saves, intervalos)."""

import os
import sys


VIRTUAL_H = 800            # altura "de desenho" fixa; a largura adapta-se ao ecrã
VW_MIN, VW_MAX = 1000, 2400

TOPBAR_H = 70
HINT_H = 0
FPS = 60

GAME_TITLE = "Lucky Sahurs"
# Número da versão (vX.Y.Z): quem o escreve é o GitHub Actions, a partir da tag da Release (ver
# .github/workflows/build.yml). Assim o programa sabe sempre que versão é, e as atualizações não fazem ciclos.
# Sem esse ficheiro ("python main.py" ou um .exe feito à mão) fica "dev": não há verificação de atualizações.
try:
    from build_version import BUILD_VERSION
except ImportError:
    BUILD_VERSION = "dev"
VERSION = BUILD_VERSION if BUILD_VERSION != "dev" else "v7"       # o que aparece no menu

# Atualizações automáticas (ver online/updater.py)
UPDATE_REPO = "gomesnun/lucky-sahurs"       # dono/repositório onde estão as Releases
UPDATE_CHECK_INTERVAL = 30 * 60             # de quanto em quanto tempo volta a perguntar ao GitHub (segundos)
UPDATE_RETRY_AFTER_ERROR = 5 * 60           # se o GitHub não respondeu (sem net...): tenta outra vez daqui a isto


def _pasta_de_dados_do_utilizador():
    """A pasta de dados "oficial" de cada sistema, onde ficam os saves:
    Windows: %APPDATA%\\LuckySahurs   macOS: ~/Library/Application Support/LuckySahurs
    Linux: $XDG_DATA_HOME/LuckySahurs (por omissão ~/.local/share/LuckySahurs)."""
    home = os.path.expanduser("~")
    if sys.platform == "win32":
        base = os.environ.get("APPDATA") or home
    elif sys.platform == "darwin":
        base = os.path.join(home, "Library", "Application Support")
    else:
        base = os.environ.get("XDG_DATA_HOME") or os.path.join(home, ".local", "share")
    return os.path.join(base, "LuckySahurs")


def _dentro_de_uma_app_do_mac():
    """True se isto for o jogo empacotado como .app do macOS (nunca se grava dentro da .app)."""
    exe = os.path.abspath(sys.executable).replace("\\", "/")
    return sys.platform == "darwin" and getattr(sys, "frozen", False) and ".app/Contents/" in exe


def _pasta_do_jogo():
    """A pasta onde está o jogo: a do main.py, ou a do executável (PyInstaller)."""
    if getattr(sys, "frozen", False):
        return os.path.dirname(os.path.abspath(sys.executable))   # com o .exe, __file__ é uma pasta temporária
    return os.path.dirname(os.path.abspath(__file__))


# Pasta do jogo. Serve para ficheiros que o jogador pode pôr "ao lado do jogo" (uma pasta icons/ ou fonts/ para
# trocar as imagens / a letra). Os SAVES já não ficam aqui: ver _pasta_dos_saves().
GAME_DIR = _pasta_do_jogo()

# Ficheiros pessoais que as versões antigas guardavam ao lado do jogo.
_FICHEIROS_ANTIGOS = ("savegame*.json", "lucky_sahurs_*.json", "firebase_config.json")


def _pode_escrever(folder):
    try:
        os.makedirs(folder, exist_ok=True)
        teste = os.path.join(folder, ".escrita_teste")
        with open(teste, "w") as f:
            f.write("ok")
        os.remove(teste)
        return True
    except OSError:
        return False


def _migrar_saves_antigos(antiga, nova):
    """Uma única vez: quem já tinha saves ao lado do jogo passa a tê-los também na pasta nova.
    COPIA (nunca apaga nem sobrescreve): se os ficheiros antigos ficarem lá, podes apagá-los depois."""
    marca = os.path.join(nova, ".migrado")
    if os.path.exists(marca) or os.path.abspath(antiga) == os.path.abspath(nova):
        return
    try:
        import glob
        import shutil
        if not glob.glob(os.path.join(nova, "savegame*.json")):        # a pasta nova ainda não tem saves
            for padrao in _FICHEIROS_ANTIGOS:
                for src in glob.glob(os.path.join(antiga, padrao)):
                    dst = os.path.join(nova, os.path.basename(src))
                    if not os.path.exists(dst):
                        shutil.copy2(src, dst)
            cache = os.path.join(antiga, "cloud_cache")
            if os.path.isdir(cache) and not os.path.exists(os.path.join(nova, "cloud_cache")):
                shutil.copytree(cache, os.path.join(nova, "cloud_cache"))
        with open(marca, "w") as f:
            f.write("ok")
    except OSError:
        pass


def _pasta_dos_saves():
    """Onde ficam os saves, as definições e a sessão: SEMPRE na pasta de dados do utilizador do sistema
    (Windows: %APPDATA%\\LuckySahurs, macOS: ~/Library/Application Support/LuckySahurs, Linux:
    ~/.local/share/LuckySahurs) e não ao lado do jogo. Assim não estão à vista na pasta do jogo.
    Para usar outra pasta (testes, pen drive): variável de ambiente LUCKY_SAHURS_SAVE_DIR.
    Último recurso, se a pasta de dados não deixar escrever: a pasta do jogo."""
    override = os.environ.get("LUCKY_SAHURS_SAVE_DIR")
    if override:
        folder = os.path.abspath(os.path.expanduser(override))
        _pode_escrever(folder)
        return folder
    dados = _pasta_de_dados_do_utilizador()
    if _pode_escrever(dados):
        if not _dentro_de_uma_app_do_mac():
            _migrar_saves_antigos(GAME_DIR, dados)
        return dados
    return GAME_DIR


SAVE_DIR = _pasta_dos_saves()
SAVE_PATH = os.path.join(SAVE_DIR, "savegame.json")   # save antigo (v1-v4) — só usado para migrar
SAVE_SLOTS = 3


AUTOSAVE_INTERVAL = 10.0

# Atalho de ecrã inteiro mostrado no menu. No macOS o F11 costuma estar ocupado pelo sistema (e precisa da
# tecla Fn nos portáteis), por isso lá também funciona Cmd+F (ver main.py).
FULLSCREEN_HINT = "F11 / Cmd+F" if sys.platform == "darwin" else "F11"
