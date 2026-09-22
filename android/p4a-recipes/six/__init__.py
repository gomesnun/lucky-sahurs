"""Recipe do six (o pyjnius precisa dele) - so para o p4a nao ter de o ir buscar com o pip.

Porque: o six e um unico ficheiro .py, mas nao vem com recipe no python-for-android. Sem recipe, o p4a
cria um "venv" a parte, corre la dentro "pip install -U pip" e so depois instala o modulo. Esse pip novo
instalado por cima de si proprio fica partido:

    ImportError: cannot import name 'BuildDependencyInstallError' from 'pip._internal.exceptions'

e o build morre ali. Com esta recipe nao ha nenhum requisito sem recipe, o p4a salta o venv todo, e o
six e copiado como qualquer outro modulo em Python puro.
"""

from pythonforandroid.recipe import PythonRecipe


class SixRecipe(PythonRecipe):
    version = "1.17.0"
    url = "https://pypi.org/packages/source/s/six/six-{version}.tar.gz"
    depends = []
    call_hostpython_via_targetpython = False
    install_in_hostpython = True


recipe = SixRecipe()
