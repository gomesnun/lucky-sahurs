<h1 align="center">Lucky Sahurs</h1>

<p align="center">
  <b>An idle pet-collecting game built on pure Italian-brainrot chaos.</b><br>
  Roll for pets from <i>Common</i> to <i>Transcendent</i>, stack income, rebirth for permanent power,
  and climb the global leaderboard — solo, on any desktop.
</p>

<p align="center">
  <a href="https://gomesnun.github.io/lucky-sahurs/website/"><b>🌐 Website &amp; live roll simulator</b></a> ·
  <a href="https://github.com/gomesnun/lucky-sahurs/releases/latest"><b>⬇ Download latest release</b></a> ·
  <a href="LEIA-ME.md"><b>🇵🇹 Versão portuguesa</b></a>
</p>

<p align="center">
  <img alt="Windows" src="https://img.shields.io/badge/Windows-.exe-0078D6?logo=windows&logoColor=white">
  <img alt="macOS" src="https://img.shields.io/badge/macOS-Apple%20Silicon%20%2B%20Intel-000000?logo=apple&logoColor=white">
  <img alt="Linux" src="https://img.shields.io/badge/Linux-x86__64-FCC624?logo=linux&logoColor=black">
  <img alt="Python" src="https://img.shields.io/badge/Python-3.10%2B-3776AB?logo=python&logoColor=white">
  <img alt="pygame-ce" src="https://img.shields.io/badge/pygame--ce-engine-green">
</p>

---

## What it is

Press **ROLL**. Get a pet. The rare ones pay better. The rarest one is a **1-in-500,000,000,000**.

| | |
|---|---|
| **11** rarity tiers | Common → Uncommon → Rare → Epic → Legendary → Mythic → Exotic → Secret → Divine → Cosmic → Transcendent |
| **22** collectible pets | each with its own income curve |
| **3** mutations | Golden / Diamond / and the one you find yourself |
| **12** traits | permanent modifiers rolled on top of pets |
| **Milestones** | long-run reward tracks across every category |
| **Rebirths** | wipe progress, keep permanent multipliers |
| **Daily missions** | rotating objectives, offline earnings while you sleep |
| **Cloud saves** | 3 slots, Firebase-backed, optional account |
| **Leaderboard** | global ranking |
| **Languages** | English + Português |

Everything runs offline too — the account is optional.

---

## Install

Pick your platform. Fastest path is the prebuilt download; no Python needed.

### 🪟 Windows

**Option A — download (recommended)**
1. Open the [latest release](https://github.com/gomesnun/lucky-sahurs/releases/latest).
2. Download `Lucky-Sahurs-Windows.exe`.
3. Double-click it. SmartScreen may warn about an unknown publisher → **More info → Run anyway** (the build is unsigned).

**Option B — run from source**
1. Install [Python 3.10+](https://www.python.org/downloads/) — tick **Add Python to PATH** during setup.
2. In this folder, open a terminal and run:
   ```bat
   pip install -r requirements.txt
   python main.py
   ```

**Option C — build your own `.exe`**
1. Double-click `build_exe.bat`.
2. Wait 1–2 minutes.
3. Result: `dist\Lucky Sahurs.exe` — a single file you can send to friends.

### 🍎 macOS (Apple Silicon &amp; Intel)

**Option A — download (recommended)**
1. Open the [latest release](https://github.com/gomesnun/lucky-sahurs/releases/latest).
2. Download `Lucky-Sahurs-macOS-AppleSilicon.zip` (M1/M2/M3/M4) or `Lucky-Sahurs-macOS-Intel.zip`.
3. Unzip it, then **right-click the app → Open** the first time (the app is unsigned, so a plain double-click is blocked).

**Option B — run from source**
1. Install Python 3: `brew install python` (or from python.org).
2. In this folder:
   ```bash
   chmod +x run_linux_mac.sh
   ./run_linux_mac.sh
   ```
   First launch creates a `.venv` and installs `pygame-ce` automatically.

**Option C — build your own `.app`**
```bash
./build_linux_mac.sh
```
Result: `dist/Lucky Sahurs.app`. Zip it to share it.

### 🐧 Linux

**Option A — download (recommended)**
1. Open the [latest release](https://github.com/gomesnun/lucky-sahurs/releases/latest).
2. Download the Linux `.tar.gz` and extract it.
3. Make it runnable and start it:
   ```bash
   chmod +x "Lucky Sahurs"
   ./"Lucky Sahurs"
   ```
   The binary needs **glibc ≥ 2.39** (Ubuntu 24.04+, Fedora 40+, Mint 22+, Arch). On older distros use Option B.

**Option B — run from source**
1. Ensure Python 3 and venv support: `sudo apt install python3 python3-venv` (Debian/Ubuntu).
2. In this folder:
   ```bash
   chmod +x run_linux_mac.sh
   ./run_linux_mac.sh
   ```

**Option C — build your own binary**
```bash
./build_linux_mac.sh
```
Result: `dist/Lucky Sahurs` (single file).

> PyInstaller cannot cross-compile: each platform's executable must be built on that platform (or by CI, below).

---

## Releasing (maintainers)

`.github/workflows/build.yml` builds all four targets on GitHub's runners and attaches them to a Release:

```bash
git tag v1.0.0
git push origin v1.0.0
```

5–10 minutes later, Windows `.exe`, Linux `.tar.gz` and both macOS `.zip` files are on the Releases page.
To only test a build without publishing: **Actions → Build Lucky Sahurs → Run workflow** (files land under *Artifacts*).

---

## Project layout

```
main.py          entry point + game loop
config.py        tunables, paths, constants
core/            game rules — pets, rarities, traits, upgrades, rebirths, milestones, economy
ui/              pygame screens and panels (title, game, shop, pets, leaderboard, options…)
online/          Firebase auth, cloud saves, leaderboard, updater
icons/ sounds/ fonts/   assets bundled into the executables
website/         the landing page + roll simulator
rules.txt        Firestore security rules
```

---

## Tech

Python 3 · [pygame-ce](https://pyga.me/) · PyInstaller · Firebase (Auth + Firestore) · GitHub Actions.

## License

All rights reserved unless stated otherwise. Ask before redistributing builds.
