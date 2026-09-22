#!/usr/bin/env bash
# Lucky Verities - criar o executavel no Linux ou no macOS.
# IMPORTANTE: o PyInstaller so cria o executavel do sistema em que corre (um .exe faz-se no Windows,
# uma .app faz-se num Mac, o binario Linux faz-se em Linux). Nao se faz "cross-compile".
set -e
cd "$(dirname "$0")"

PY="$(command -v python3 || command -v python || true)"
if [ -z "$PY" ]; then
    echo "Nao encontrei o Python 3. Instala-o (python.org, brew install python, ou o gestor de pacotes do Linux)."
    exit 1
fi

echo "[1/3] A preparar um ambiente virtual (.venv)..."
[ -d .venv ] || "$PY" -m venv .venv
. .venv/bin/activate

echo "[2/3] A instalar pygame-ce e pyinstaller..."
python -m pip install --upgrade pip >/dev/null
python -m pip install pygame-ce pyinstaller

echo "[3/3] A criar o jogo (demora 1 a 2 minutos)..."
if [ "$(uname)" = "Darwin" ]; then
    # macOS: uma .app (pasta) com o icone do Verity
    python -m PyInstaller --noconfirm --clean --windowed --icon "icons/verity.icns" \
        --add-data "icons:icons" --add-data "sounds:sounds" --add-data "fonts:fonts" --name "Lucky Verities" main.py
    echo
    echo "Pronto! O jogo esta em:  dist/Lucky Verities.app"
    echo "Para enviar a amigos: comprime essa .app num .zip. (Ao abrir pela 1a vez, o macOS pode avisar que o"
    echo "programador nao e verificado: botao direito na app > Abrir.)"
else
    # Linux: um unico ficheiro
    python -m PyInstaller --noconfirm --clean --onefile --windowed \
        --add-data "icons:icons" --add-data "sounds:sounds" --add-data "fonts:fonts" --name "Lucky Verities" main.py
    echo
    echo "Pronto! O jogo esta em:  dist/Lucky Verities"
    echo "(Se nao abrir com duplo clique:  chmod +x \"dist/Lucky Verities\")"
fi
