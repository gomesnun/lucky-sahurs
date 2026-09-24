"""Cores e espessuras do estilo "cartoon" usado em toda a interface.

Suporta modo claro e escuro: chama set_dark(True/False) para trocar a paleta em tempo real.
Cada ficheiro faz "from theme import PANEL, ..." (o Python copia o valor nessa altura), por isso
set_dark() nao so muda as cores aqui como tambem as espalha por todos os modulos ja importados
que as usam (ver sync_theme) - sem isto, eles ficavam presos a cor antiga para sempre.
"""

import subprocess
import sys

WHITE = (255, 255, 255)
BLACK = (12, 12, 14)

BORDER_W = 4                   # espessura do contorno dos paineis grandes
BORDER_W_SMALL = 3             # botoes, cartoes e linhas
GOOD = (100, 225, 130)
BAD = (235, 95, 95)
GOLD_BORDER = (255, 205, 60)
DIAMOND_BORDER = (150, 235, 255)
PAUSED_RED = (225, 45, 45)     # contorno das barras Golden/Diamond/Rainbow Roll quando estao pausadas

# a primeira que existir no PC e usada: Windows (segoeui, verdana, arial), macOS (helveticaneue, helvetica, arial)
# e Linux (liberationsans, dejavusans, notosans...). Se nenhuma existir, o pygame usa a fonte que traz consigo.
FONT_NAMES = "segoeui,verdana,arial,helveticaneue,helvetica,liberationsans,dejavusans,notosans,freesans"

# --- paleta "clara": fundo creme, paineis escuros com contorno preto grosso ---
_LIGHT = dict(
    BG_TOP=(246, 243, 236), BG_BOTTOM=(226, 222, 212),
    OUTLINE=(8, 8, 10),
    PANEL=(34, 34, 37), PANEL_LIGHT=(52, 52, 57), PANEL_LIGHTER=(76, 76, 83),
    ACCENT=(255, 184, 48), ACCENT_HOVER=(255, 212, 110),
    GREY=(200, 200, 208), GREY_DIM=(152, 152, 160),
)

# --- paleta "escura": fundo espacial (com estrelinhas, ver ui/drawing.py) ---
_DARK = dict(
    BG_TOP=(23, 24, 39), BG_BOTTOM=(12, 13, 21),
    OUTLINE=(6, 6, 9),
    PANEL=(41, 42, 59), PANEL_LIGHT=(59, 61, 83), PANEL_LIGHTER=(82, 85, 110),
    ACCENT=(255, 184, 48), ACCENT_HOVER=(255, 212, 110),
    GREY=(205, 208, 226), GREY_DIM=(146, 149, 172),
)

_PALETTE_NAMES = list(_LIGHT)      # nomes de cores que mudam consoante o tema
DARK_MODE = False                  # o valor real vem das settings (ver main.py); isto e so o arranque


def _apply(values):
    globals().update(values)


_apply(_LIGHT)      # modo claro por defeito enquanto ninguem chamar set_dark()


# --- tema do sistema (Windows / macOS / Linux) ---
THEME_MODES = ("system", "dark", "light")     # o que o jogador escolhe nas Options; "system" = segue o sistema
DEFAULT_THEME_MODE = "system"
SYSTEM_LABEL = "Windows default" if sys.platform == "win32" else "System default"


def system_prefers_dark(default=True):
    """True se o sistema estiver em modo escuro. Windows: registo (AppsUseLightTheme); macOS: AppleInterfaceStyle;
    Linux: gsettings (GNOME). Se nao conseguir descobrir, devolve 'default' (nunca dá erro)."""
    try:
        if sys.platform == "win32":
            import winreg
            key = winreg.OpenKey(winreg.HKEY_CURRENT_USER,
                                 r"Software\Microsoft\Windows\CurrentVersion\Themes\Personalize")
            try:
                value, _kind = winreg.QueryValueEx(key, "AppsUseLightTheme")
            finally:
                winreg.CloseKey(key)
            return int(value) == 0
        if sys.platform == "darwin":
            out = subprocess.run(["defaults", "read", "-g", "AppleInterfaceStyle"],
                                 capture_output=True, text=True, timeout=1.5)
            return "dark" in out.stdout.lower()          # sem valor (modo claro) o comando falha e nao escreve nada
        out = subprocess.run(["gsettings", "get", "org.gnome.desktop.interface", "color-scheme"],
                             capture_output=True, text=True, timeout=1.5)
        text = out.stdout.lower()
        if "dark" in text:
            return True
        if "light" in text or "default" in text:
            return False
    except Exception:
        pass
    return default


def resolve_dark(mode):
    """'dark' -> True, 'light' -> False, 'system' (ou qualquer outra coisa) -> o que o sistema estiver a usar."""
    if mode == "dark":
        return True
    if mode == "light":
        return False
    return system_prefers_dark()


def sync_theme():
    """Espalha as cores ATUAIS deste modulo por todos os ficheiros ja importados que fizeram
    'from theme import <cor>'. Chamar sempre depois de mudar a paleta (ver set_dark)."""
    here = sys.modules[__name__]
    for mod in list(sys.modules.values()):
        if mod is None or mod is here:
            continue
        try:
            ns = vars(mod)
            for name in _PALETTE_NAMES:
                if name in ns and isinstance(ns[name], tuple):     # so as cores que vieram daqui (tuplos RGB)
                    ns[name] = getattr(here, name)
        except TypeError:
            continue


def set_dark(dark):
    """Muda para o modo escuro (dark=True) ou claro (dark=False) e atualiza o jogo todo (as
    cores ja desenhadas ficam como estavam; o resto do codigo volta a chamar-nos a cada frame)."""
    global DARK_MODE
    DARK_MODE = bool(dark)
    _apply(_DARK if DARK_MODE else _LIGHT)
    sync_theme()
