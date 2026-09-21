"""Recipe do pygame para o Android: a mesma do python-for-android, mas numa versao recente.

Porque: a recipe que vem com o p4a esta presa ao pygame 2.1.0, que nao compila com o Python 3.11+
("fatal error: 'longintrepr.h' file not found" - esse header saiu do Python em 3.11). O 2.6.1 ja compila.

So o Android usa isto; no computador o jogo continua a usar o pygame-ce (ver requirements.txt).
Toda a API usada pelo jogo existe nos dois.
"""

from pythonforandroid.recipes.pygame import Pygame2Recipe


class PygameRecentRecipe(Pygame2Recipe):
    version = "2.6.1"
    # O setup.py do pygame gera codigo com o Cython; o 3.1 rejeita o "cdef dict __dict__" do _sprite.pyx.
    hostpython_prerequisites = ["setuptools", "cython<3.1"]


recipe = PygameRecentRecipe()
