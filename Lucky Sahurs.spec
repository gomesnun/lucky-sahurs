# -*- mode: python ; coding: utf-8 -*-
# Nota: os scripts build_exe.bat (Windows) e build_linux_mac.sh (Linux/macOS) chamam o PyInstaller diretamente
# e já põem o icone do Tung. Este .spec faz o mesmo para quem preferir "pyinstaller Lucky Sahurs.spec".
import sys

# icone do executavel: .ico no Windows, .icns no macOS (o Linux nao tem icone no ficheiro; a janela usa o Tung na mesma)
ICON = {'win32': 'icons/tung.ico', 'darwin': 'icons/tung.icns'}.get(sys.platform)

a = Analysis(
    ['main.py'],
    pathex=[],
    binaries=[],
    datas=[('icons', 'icons'), ('sounds', 'sounds'), ('fonts', 'fonts')],
    hiddenimports=[],
    hookspath=[],
    hooksconfig={},
    runtime_hooks=[],
    excludes=[],
    noarchive=False,
    optimize=0,
)
pyz = PYZ(a.pure)

exe = EXE(
    pyz,
    a.scripts,
    a.binaries,
    a.datas,
    [],
    name='Lucky Sahurs',
    debug=False,
    bootloader_ignore_signals=False,
    strip=False,
    upx=True,
    upx_exclude=[],
    runtime_tmpdir=None,
    console=False,
    disable_windowed_traceback=False,
    argv_emulation=False,
    target_arch=None,
    codesign_identity=None,
    entitlements_file=None,
    icon=ICON,
)
