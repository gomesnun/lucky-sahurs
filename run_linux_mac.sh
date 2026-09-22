#!/usr/bin/env bash
# Lucky Verities - jogar no Linux ou no macOS sem criar executavel (usa o Python).
# Na 1a vez instala o pygame-ce num ambiente virtual (.venv) dentro desta pasta; nas seguintes abre logo.
cd "$(dirname "$0")"

PY="$(command -v python3 || command -v python || true)"
if [ -z "$PY" ]; then
    echo "Nao encontrei o Python 3. Instala-o (python.org, brew install python, ou o gestor de pacotes do Linux)."
    exit 1
fi

if [ ! -d .venv ]; then
    echo "A preparar o jogo (so na primeira vez)..."
    "$PY" -m venv .venv || { echo "Falhou a criar o ambiente virtual. No Ubuntu/Debian:  sudo apt install python3-venv"; exit 1; }
fi
. .venv/bin/activate
python -c "import pygame" 2>/dev/null || python -m pip install pygame-ce || exit 1

exec python main.py
