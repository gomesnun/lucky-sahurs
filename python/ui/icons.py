"""Ícones PNG (pasta icons/): carregados uma vez e guardados em cache. Se faltar um ficheiro,
o jogo usa o ícone desenhado por código (por isso podes trocar / apagar ícones à vontade)."""

import os
import sys
import pygame

from config import GAME_DIR, SHARED_DIR


def _icon_dirs():
    """Onde procurar, por ordem: ao lado do jogo / .exe (para poderes trocar os ícones), a pasta do
    projeto e, dentro do .exe (PyInstaller), a pasta embutida."""
    dirs = [os.path.join(GAME_DIR, "icons"),
            os.path.join(os.path.dirname(os.path.dirname(os.path.abspath(__file__))), "icons"),
            os.path.join(SHARED_DIR, "icons")]
    bundled = getattr(sys, "_MEIPASS", None)
    if bundled:
        dirs.append(os.path.join(bundled, "icons"))
    return dirs


_cache = {}


def load_icon(name, size):
    """Surface quadrada (size x size, com transparência) do ficheiro icons/<name>.png, ou None se não existir."""
    key = (name, size)
    if key in _cache:
        return _cache[key]
    surf = None
    for folder in _icon_dirs():
        path = os.path.join(folder, name + ".png")
        if not os.path.isfile(path):
            continue
        try:
            img = pygame.image.load(path).convert_alpha()
            surf = pygame.transform.smoothscale(img, (size, size))
            break
        except (pygame.error, OSError):
            continue
    _cache[key] = surf
    return surf


_pet_cache = {}


def load_pet_image(pet_name, size, silhouette=False):
    """Imagem do verity (icons/pets/<nome em minúsculas>.png) numa surface quadrada size x size com
    transparência, ou None se o ficheiro não existir (o cartão volta então ao desenho só com texto).
    silhouette=True devolve a mesma imagem escurecida (para os verities que ainda não tens no Index).
    O resultado fica em cache: não o alteres."""
    key = (pet_name, size, silhouette)
    if key in _pet_cache:
        return _pet_cache[key]
    surf = None
    slug = "".join(c for c in pet_name.lower() if c.isalnum())
    for folder in _icon_dirs():
        path = os.path.join(folder, "pets", slug + ".png")
        if not os.path.isfile(path):
            continue
        try:
            img = pygame.image.load(path).convert_alpha()
            surf = pygame.transform.smoothscale(img, (size, size))
            if silhouette:
                surf.fill((24, 26, 44, 255), special_flags=pygame.BLEND_RGB_MULT)
            break
        except (pygame.error, OSError):
            continue
    if len(_pet_cache) > 400:
        _pet_cache.clear()
    _pet_cache[key] = surf
    return surf


def set_window_icon(name="verity", size=64):
    """Põe o smiley do Verity (icons/<name>.png) como ícone da janela / da barra de tarefas, no lugar do ícone por
    defeito do pygame. Chama-se ANTES de criar a janela (por isso não se usa convert_alpha aqui).
    O ícone do próprio .exe / .app é outra coisa: vem do --icon do PyInstaller (ver build_exe.bat e o .spec)."""
    for folder in _icon_dirs():
        path = os.path.join(folder, name + ".png")
        if not os.path.isfile(path):
            continue
        try:
            img = pygame.image.load(path)
            try:
                img = pygame.transform.smoothscale(img, (size, size))
            except (pygame.error, ValueError):
                pass                      # sem o redimensionamento: o SDL ajusta o tamanho sozinho
            pygame.display.set_icon(img)
            return True
        except (pygame.error, OSError):
            continue
    return False


def find_asset(folder, filename):
    """Caminho de <folder>/<filename> (sons, por exemplo): ao lado do jogo / .exe, na pasta do projeto
    ou embutido no .exe (PyInstaller). Devolve None se o ficheiro não existir em nenhum sítio."""
    roots = [GAME_DIR, os.path.dirname(os.path.dirname(os.path.abspath(__file__))), SHARED_DIR]
    bundled = getattr(sys, "_MEIPASS", None)
    if bundled:
        roots.append(bundled)
    for root in roots:
        path = os.path.join(root, folder, filename)
        if os.path.isfile(path):
            return path
    return None
