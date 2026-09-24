"""Recipe do pygame para o Android: a mesma do python-for-android, mas numa versao recente.

Porque: a recipe que vem com o p4a esta presa ao pygame 2.1.0, que nao compila com o Python 3.11+
("fatal error: 'longintrepr.h' file not found" - esse header saiu do Python em 3.11). O 2.6.1 compila,
desde que o Cython seja o 3.1 ou mais novo (so esse gera codigo para o Python 3.14 do p4a).

O pygame-ce nao serve aqui: ja passou para o meson, que nao cruza-compila com esta recipe.

So o Android usa isto; no computador o jogo continua a usar o pygame-ce (ver requirements.txt).
Toda a API usada pelo jogo existe nos dois.
"""

from os.path import join

import sh
from pythonforandroid.recipes.pygame import Pygame2Recipe
from pythonforandroid.toolchain import current_directory, shprint

# Linha do modulo "surface" no Setup do Android, tal como vem no pygame 2.6.1.
SURFACE_LINE = "surface src_c/surface.c src_c/alphablit.c src_c/surface_fill.c"
# Os blitters SIMD faltam nessa linha. No arm64 o pygame liga-os na mesma (o simd_blitters.h ativa o
# PG_ENABLE_ARM_NEON, que traduz o SSE2 para NEON), e sem eles o import do pygame.display rebenta com
# 'cannot locate symbol "alphablit_alpha_sse2_argb_surf_alpha"'.
SIMD_SOURCES = "src_c/simd_blitters_sse2.c src_c/simd_blitters_avx2.c"


class PygameRecentRecipe(Pygame2Recipe):
    version = "2.6.1"
    # O setup.py do pygame gera o codigo C com o Cython, que nao vem no tarball.
    hostpython_prerequisites = ["setuptools", "cython>=3.1"]

    def prebuild_arch(self, arch):
        with current_directory(self.get_build_dir(arch.arch)):
            # O Cython 3.1 recusa atribuir ao __dict__ de uma cdef class (passou a trata-lo sozinho).
            # O jogo nao usa o pygame.sprite, mas o _sprite faz parte do Setup do Android e tem de compilar.
            shprint(sh.sed, "-i", "/^        self\\.__dict__ = {}$/d",
                    join("src_c", "cython", "pygame", "_sprite.pyx"))
            shprint(sh.sed, "-i",
                    "s|^{}|{} {}|".format(SURFACE_LINE, SURFACE_LINE, SIMD_SOURCES),
                    join("buildconfig", "Setup.Android.SDL2.in"))
        super().prebuild_arch(arch)


recipe = PygameRecentRecipe()
