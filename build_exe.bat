@echo off
cd /d "%~dp0"
echo ================================================
echo   Lucky Sahurs - criar o executavel (.exe)
echo ================================================
echo.

python --version >nul 2>&1
if errorlevel 1 (
    echo Nao encontrei o Python. Instala-o em python.org e marca "Add Python to PATH".
    pause
    exit /b 1
)

echo [1/3] A verificar o pygame...
python -c "import pygame" >nul 2>&1
if errorlevel 1 (
    echo Nao tens o pygame instalado. A instalar o pygame-ce...
    python -m pip install pygame-ce
    if errorlevel 1 (
        echo A instalacao do pygame-ce falhou. Verifica a ligacao a internet.
        pause
        exit /b 1
    )
) else (
    echo pygame ja esta instalado - vou usar esse mesmo.
)

echo.
echo [2/3] A instalar / atualizar o pyinstaller...
python -m pip install --upgrade pyinstaller
if errorlevel 1 (
    echo A instalacao do pyinstaller falhou. Verifica a ligacao a internet.
    pause
    exit /b 1
)

echo.
echo [3/3] A criar o .exe com o icone do Tung (demora 1 a 2 minutos)...
python -m PyInstaller --noconfirm --clean --onefile --windowed --icon "icons\tung.ico" --add-data "icons;icons" --add-data "sounds;sounds" --name "Lucky Sahurs" main.py
if errorlevel 1 (
    echo A criacao do .exe falhou. Copia a mensagem de erro de cima e manda-a.
    pause
    exit /b 1
)

echo.
echo Pronto! O jogo esta em:  %~dp0dist\Lucky Sahurs.exe
echo Manda so esse ficheiro aos teus amigos.
pause
