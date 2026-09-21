# Lucky Sahurs - empacotamento para Android (buildozer + python-for-android).
#
# COMO CRIAR O APK
#   python -m pip install --user buildozer cython
#   buildozer android debug            -> bin/luckysahurs-<versao>-debug.apk
# A primeira vez descarrega o SDK/NDK do Android (alguns GB) e demora bastante; as seguintes sao rapidas.
# Precisa de um JDK 17 (variavel JAVA_HOME) e, em Linux, de: git zip unzip autoconf libtool pkg-config.
#
# Para publicar (APK/AAB assinado): "buildozer android release", depois assinar com o apksigner do SDK.

[app]
title = Lucky Sahurs
package.name = luckysahurs
package.domain = io.github.gomesnun

source.dir = .
source.include_exts = py,png,jpg,ttf,otf,ogg,wav,txt,json,ico,icns
source.include_patterns = icons/*,sounds/*,fonts/*
# Nada do que e so do computador entra no APK (scripts de build, site, workflows, saves de teste).
source.exclude_dirs = website,.github,android,bin,.buildozer,.venv,dist,build,__pycache__
source.exclude_patterns = build_exe.bat,build_linux_mac.sh,run_linux_mac.sh,*.spec,rules.txt,read-problems.md,savegame*.json,lucky_sahurs_*.json

# Numero da versao do APK. Acompanhar as tags das Releases (v1.0.4 -> 1.0.4).
version = 1.0.4

# pygame-ce (recipe "pygame"), HTTPS com certificados proprios (a conta / leaderboard / saves na nuvem).
requirements = python3,pygame,certifi,openssl

icon.filename = %(source.dir)s/icons/tung.png
presplash.filename = %(source.dir)s/icons/tung.png
android.presplash_color = #12121a

orientation = landscape
fullscreen = 1

# INTERNET: conta, saves na nuvem e leaderboard (Firebase). O jogo funciona na mesma sem rede.
android.permissions = android.permission.INTERNET,android.permission.ACCESS_NETWORK_STATE

android.api = 34
android.minapi = 24
android.ndk_api = 24
android.archs = arm64-v8a,armeabi-v7a
android.allow_backup = True
android.accept_sdk_license = True

# O jogo desenha para um canvas fixo e faz smoothscale para o ecra: um unico SDL2 window chega.
p4a.bootstrap = sdl2
# Recipe propria do pygame (ver android/p4a-recipes/pygame): a do p4a esta presa a uma versao
# que nao compila com o Python 3.11+.
p4a.local_recipes = ./android/p4a-recipes

[buildozer]
log_level = 2
warn_on_root = 1
