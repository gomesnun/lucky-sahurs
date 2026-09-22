"""Constantes gerais do jogo (janela, versão, pastas de saves, intervalos)."""

import os
import sys


# Android: o python-for-android define estas variáveis de ambiente no arranque da app.
IS_ANDROID = bool(os.environ.get("ANDROID_ARGUMENT") or os.environ.get("ANDROID_PRIVATE"))

# Altura "de desenho" fixa; a largura adapta-se ao ecrã. No telemóvel o ecrã é pequeno e o CPU é
# fraco: desenhar menos píxeis aumenta tudo (botões, letras) e ainda dá mais FPS.
VIRTUAL_H = 560 if IS_ANDROID else 800
VW_MIN, VW_MAX = (700, 1400) if IS_ANDROID else (1000, 2400)

TOPBAR_H = 70
HINT_H = 0
FPS = 60

GAME_TITLE = "Lucky Verities"
# Número da versão (vX.Y.Z): quem o escreve é o GitHub Actions, a partir da tag da Release (ver
# .github/workflows/build.yml). Assim o programa sabe sempre que versão é, e as atualizações não fazem ciclos.
# Sem esse ficheiro ("python main.py" ou um .exe feito à mão) fica "dev": não há verificação de atualizações.
try:
    from build_version import BUILD_VERSION
except ImportError:
    BUILD_VERSION = "dev"
VERSION = BUILD_VERSION if BUILD_VERSION != "dev" else "v2.2.0"       # o que aparece no menu

# Atualizações automáticas (ver online/updater.py)
UPDATE_REPO = "gomesnun/lucky-verities"       # dono/repositório onde estão as Releases
UPDATE_CHECK_INTERVAL = 30 * 60             # de quanto em quanto tempo volta a perguntar ao GitHub (segundos)
UPDATE_RETRY_AFTER_ERROR = 5 * 60           # se o GitHub não respondeu (sem net...): tenta outra vez daqui a isto


# Nome da pasta de dados (dentro da pasta de dados do sistema) e o que tinha o jogo antigo (Lucky Sahurs):
# quem já jogava não perde os saves, porque na 1.ª vez eles são COPIADOS da pasta antiga (ver _pasta_dos_saves).
PASTA_DE_DADOS = "LuckyVerities"
PASTA_DE_DADOS_ANTIGA = "LuckySahurs"


def _pasta_de_dados(nome):
    """A pasta de dados "oficial" de cada sistema, com este nome por dentro, onde ficam os saves:
    Android: a pasta privada da app (ANDROID_PRIVATE), que só esta app lê e que o sistema apaga com ela.
    Windows: %APPDATA%\\<nome>   macOS: ~/Library/Application Support/<nome>
    Linux: $XDG_DATA_HOME/<nome> (por omissão ~/.local/share/<nome>)."""
    if IS_ANDROID:
        privada = os.environ.get("ANDROID_PRIVATE") or os.path.expanduser("~")
        return os.path.join(privada, nome)
    home = os.path.expanduser("~")
    if sys.platform == "win32":
        base = os.environ.get("APPDATA") or home
    elif sys.platform == "darwin":
        base = os.path.join(home, "Library", "Application Support")
    else:
        base = os.environ.get("XDG_DATA_HOME") or os.path.join(home, ".local", "share")
    return os.path.join(base, nome)


def _pasta_de_dados_do_utilizador():
    """Onde ficam os saves do Lucky Verities: %APPDATA%\\LuckyVerities (Windows), etc."""
    return _pasta_de_dados(PASTA_DE_DADOS)


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
_FICHEIROS_ANTIGOS = ("savegame*.json", "lucky_sahurs_*.json", "lucky_verities_*.json", "firebase_config.json")


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


def _nome_novo(nome):
    """Os ficheiros do jogo antigo chamavam-se lucky_sahurs_*.json: na pasta nova passam a lucky_verities_*.json."""
    return nome.replace("lucky_sahurs_", "lucky_verities_", 1) if nome.startswith("lucky_sahurs_") else nome


def _migrar_saves_antigos(antiga, nova, nome_da_marca=".migrado"):
    """Uma única vez: quem já tinha saves noutra pasta (ao lado do jogo, ou na pasta de dados do Lucky Sahurs)
    passa a tê-los também na pasta nova.
    COPIA (nunca apaga nem sobrescreve): se os ficheiros antigos ficarem lá, podes apagá-los depois."""
    marca = os.path.join(nova, nome_da_marca)
    if os.path.exists(marca) or os.path.abspath(antiga) == os.path.abspath(nova):
        return
    try:
        import glob
        import shutil
        if not glob.glob(os.path.join(nova, "savegame*.json")):        # a pasta nova ainda não tem saves
            for padrao in _FICHEIROS_ANTIGOS:
                for src in glob.glob(os.path.join(antiga, padrao)):
                    dst = os.path.join(nova, _nome_novo(os.path.basename(src)))
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
    (Windows: %APPDATA%\\LuckyVerities, macOS: ~/Library/Application Support/LuckyVerities, Linux:
    ~/.local/share/LuckyVerities) e não ao lado do jogo. Assim não estão à vista na pasta do jogo.
    Para usar outra pasta (testes, pen drive): variável de ambiente LUCKY_VERITIES_SAVE_DIR (a antiga
    LUCKY_SAHURS_SAVE_DIR continua a funcionar).
    Último recurso, se a pasta de dados não deixar escrever: a pasta do jogo."""
    override = os.environ.get("LUCKY_VERITIES_SAVE_DIR") or os.environ.get("LUCKY_SAHURS_SAVE_DIR")
    if override:
        folder = os.path.abspath(os.path.expanduser(override))
        _pode_escrever(folder)
        return folder
    dados = _pasta_de_dados_do_utilizador()
    if _pode_escrever(dados):
        # 1.º os saves do jogo antigo (pasta LuckySahurs): são os mais recentes, por isso têm prioridade
        _migrar_saves_antigos(_pasta_de_dados(PASTA_DE_DADOS_ANTIGA), dados, ".migrado_sahurs")
        if not _dentro_de_uma_app_do_mac() and not IS_ANDROID:
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

# No Android o jogo é sempre em ecrã inteiro (não há janela nem F11) e os toques substituem o rato:
# um toque que arrasta mais do que isto (pixéis do ecrã) conta como scroll e já não carrega no botão.
TOUCH_DRAG_SLOP = 16
