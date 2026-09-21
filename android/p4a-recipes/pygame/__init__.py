"""Recipe do pygame para o Android: a mesma do python-for-android, mas com o pygame-ce.

Porque: a recipe que vem com o p4a esta presa ao pygame 2.1.0 e o pygame classico (2.6.1) nao
compila com o Python 3.14 que o p4a usa hoje ("fatal error: 'longintrepr.h' file not found").
O pygame-ce 2.5.7 suporta o 3.14 e e o mesmo pygame que o jogo usa no computador.
"""

from pythonforandroid.recipes.pygame import Pygame2Recipe


class PygameCeRecipe(Pygame2Recipe):
    version = "2.5.7"
    url = "https://github.com/pygame-community/pygame-ce/archive/refs/tags/{version}.tar.gz"
    # O setup.py cythoniza as fontes; o pygame-ce exige um Cython ate ao 3.2.4.
    hostpython_prerequisites = ["setuptools", "cython<=3.2.4"]


recipe = PygameCeRecipe()
