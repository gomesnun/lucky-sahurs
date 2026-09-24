<h1 align="center">Lucky Verities</h1>

<p align="center">
  <b>An idle collecting game about Verities: little round creatures with big personalities.</b><br>
  Roll for pets from <i>Common</i> to <i>Paradox</i>, stack income, rebirth and prestige for permanent power,
  add friends and climb the global leaderboard — on Windows, macOS and Linux.
</p>

<p align="center">
  <a href="https://gomesnun.github.io/lucky-verities/website/"><b>🌐 Website &amp; live roll simulator</b></a> ·
  <a href="https://github.com/gomesnun/lucky-verities/releases/latest"><b>⬇ Download latest release</b></a> ·
  <a href="python/LEIA-ME.md"><b>🇵🇹 Versão portuguesa</b></a>
</p>

<p align="center">
  <img alt="Windows" src="https://img.shields.io/badge/Windows-.exe-0078D6?logo=windows&logoColor=white">
  <img alt="macOS" src="https://img.shields.io/badge/macOS-Apple%20Silicon%20%2B%20Intel-000000?logo=apple&logoColor=white">
  <img alt="Linux" src="https://img.shields.io/badge/Linux-x86__64-FCC624?logo=linux&logoColor=black">
  <img alt="Rust" src="https://img.shields.io/badge/Rust-desktop-B7410E?logo=rust&logoColor=white">
</p>

---

## What it is

Press **ROLL**. Get a pet. The rare ones pay better. The rarest one is a **1-in-5,000,000,000,000,000,000,000** (5 sextillion).

| | |
|---|---|
| **16** rarity tiers | Common → Uncommon → Rare → Epic → Legendary → Mythic → Exotic → Secret → Divine → Cosmic → Transcendent → Ethereal → Celestial → Absolute → Primordial → **Paradox** |
| **64** collectible Verities | each with its own income; the **Index** remembers every one you ever rolled (256 entries with the mutations) |
| **4** mutations | Normal, **Golden** (x3), **Diamond** (x9) and **Rainbow** (x27), plus Golden / Diamond / Rainbow Rolls every so many rolls |
| **17** traits | rolled with trait charges, one equipped at a time; the **Auto Trait Roller** spends your charges for you |
| **Upgrades** | a tree of 48 upgrades: luck for every tier, money, mutation chance, bonus rolls, Auto Roller, offline earnings, slots, the **Auto Upgrader** |
| **Rebirths** | reset your coins for permanent Money and Luck, with 17 rewards along the way |
| **Prestige** | 5 Prestiges (10 / 15 / 20 / 30 / 40 Rebirths) for up to **x200 Money and x60 Luck**; keep 1 verity of your choice |
| **Shop** | 10 dice (permanent luck, and your ROLL button takes their style) and Coin / Speed / Luck potions; new stock every 10 minutes, the same for everyone |
| **Quests** | 3 daily quests (new at 00:00 Lisbon time) and 3 harder weekly quests (every Monday); rewards grow with you |
| **Milestones** | 15 categories of long-run goals, grouped, each with permanent bonuses |
| **Titles** | equip one and everyone sees it: Owner, Top 1 / 2 / 3, Prestiged, Completionist and more |
| **Online** | optional account with 3 cloud save slots, friends (with online status), chat, **trades** between friends, and live events |
| **Leaderboards** | Money, Playtime, Rolls and Rebirths (with each player's Prestige), top 50 |
| **Offline earnings** | you keep earning while the game is closed |
| **Effects** | cutscenes for the rarest pulls, verities raining behind the roll screen, click pops, glowing buttons |
| **Languages** | English + Português |

Everything runs offline too — the account is optional.

---

## Install

Pick your platform. The fastest way is the ready-made download.

### 🪟 Windows

1. Open the [latest release](https://github.com/gomesnun/lucky-verities/releases/latest).
2. Download `Lucky-Verities-Windows.exe`.
3. Double-click it. SmartScreen may warn about an unknown publisher → **More info → Run anyway** (the build is unsigned).


### 🍎 macOS (Apple Silicon &amp; Intel)

1. Open the [latest release](https://github.com/gomesnun/lucky-verities/releases/latest).
2. Download `Lucky-Verities-macOS-AppleSilicon.zip` (M1/M2/M3/M4) or `Lucky-Verities-macOS-Intel.zip`.
3. Unzip it, then **right-click the app → Open** the first time (the app is unsigned, so a plain double-click is blocked).

### 🐧 Linux

1. Open the [latest release](https://github.com/gomesnun/lucky-verities/releases/latest).
2. Download the Linux `.tar.gz` and extract it.
3. Make it runnable and start it:
   ```bash
   chmod +x LuckyVerities
   ./LuckyVerities
   ```
   The binary needs **glibc ≥ 2.39** (Ubuntu 24.04+, Fedora 40+, Mint 22+, Arch).

#### Build it yourself (one command)

Any distro, any glibc: paste the line for yours into a terminal. It installs what the build needs, installs Rust
(through [rustup](https://rustup.rs), in your home folder), downloads the game, builds it and puts the program at
`~/.local/bin/lucky-verities`. Then start it with `lucky-verities` (or `~/.local/bin/lucky-verities`). Running the
same line again updates it. Your saves are kept either way.

**Ubuntu / Debian / Mint / Pop!_OS**
```bash
sudo apt update; sudo apt install -y git curl build-essential pkg-config libsdl2-dev libfreetype-dev libharfbuzz-dev && curl -sSf https://sh.rustup.rs | sh -s -- -y --profile minimal && . "$HOME/.cargo/env" && { git clone --depth 1 https://github.com/gomesnun/lucky-verities ~/lucky-verities || git -C ~/lucky-verities pull; } && cd ~/lucky-verities && cargo build --release && install -Dm755 target/release/lucky-verities ~/.local/bin/lucky-verities
```
**Fedora**
```bash
sudo dnf install -y git curl gcc gcc-c++ make pkgconf-pkg-config SDL2-devel freetype-devel harfbuzz-devel && curl -sSf https://sh.rustup.rs | sh -s -- -y --profile minimal && . "$HOME/.cargo/env" && { git clone --depth 1 https://github.com/gomesnun/lucky-verities ~/lucky-verities || git -C ~/lucky-verities pull; } && cd ~/lucky-verities && cargo build --release && install -Dm755 target/release/lucky-verities ~/.local/bin/lucky-verities
```
**Arch / Manjaro / EndeavourOS**
```bash
sudo pacman -S --needed --noconfirm git curl base-devel pkgconf sdl2 freetype2 harfbuzz && curl -sSf https://sh.rustup.rs | sh -s -- -y --profile minimal && . "$HOME/.cargo/env" && { git clone --depth 1 https://github.com/gomesnun/lucky-verities ~/lucky-verities || git -C ~/lucky-verities pull; } && cd ~/lucky-verities && cargo build --release && install -Dm755 target/release/lucky-verities ~/.local/bin/lucky-verities
```
**openSUSE**
```bash
sudo zypper install -y git curl gcc gcc-c++ make pkg-config SDL2-devel freetype2-devel harfbuzz-devel && curl -sSf https://sh.rustup.rs | sh -s -- -y --profile minimal && . "$HOME/.cargo/env" && { git clone --depth 1 https://github.com/gomesnun/lucky-verities ~/lucky-verities || git -C ~/lucky-verities pull; } && cd ~/lucky-verities && cargo build --release && install -Dm755 target/release/lucky-verities ~/.local/bin/lucky-verities
```
**Void**
```bash
sudo xbps-install -Sy git curl base-devel pkg-config SDL2-devel freetype-devel harfbuzz-devel && curl -sSf https://sh.rustup.rs | sh -s -- -y --profile minimal && . "$HOME/.cargo/env" && { git clone --depth 1 https://github.com/gomesnun/lucky-verities ~/lucky-verities || git -C ~/lucky-verities pull; } && cd ~/lucky-verities && cargo build --release && install -Dm755 target/release/lucky-verities ~/.local/bin/lucky-verities
```
**Alpine**
```bash
sudo apk add git curl build-base pkgconf sdl2-dev freetype-dev harfbuzz-dev && curl -sSf https://sh.rustup.rs | sh -s -- -y --profile minimal && . "$HOME/.cargo/env" && { git clone --depth 1 https://github.com/gomesnun/lucky-verities ~/lucky-verities || git -C ~/lucky-verities pull; } && cd ~/lucky-verities && cargo build --release && install -Dm755 target/release/lucky-verities ~/.local/bin/lucky-verities
```
**Gentoo**
```bash
sudo emerge --noreplace dev-vcs/git net-misc/curl dev-util/pkgconf media-libs/libsdl2 media-libs/freetype media-libs/harfbuzz && curl -sSf https://sh.rustup.rs | sh -s -- -y --profile minimal && . "$HOME/.cargo/env" && { git clone --depth 1 https://github.com/gomesnun/lucky-verities ~/lucky-verities || git -C ~/lucky-verities pull; } && cd ~/lucky-verities && cargo build --release && install -Dm755 target/release/lucky-verities ~/.local/bin/lucky-verities
```
**NixOS** (nothing is installed system-wide)
```bash
{ git clone --depth 1 https://github.com/gomesnun/lucky-verities ~/lucky-verities || git -C ~/lucky-verities pull; } && cd ~/lucky-verities && nix-shell -p cargo rustc gcc pkg-config SDL2 freetype harfbuzz --run 'cargo build --release' && install -Dm755 target/release/lucky-verities ~/.local/bin/lucky-verities
```

Installed copies update themselves: when a newer release is out, the game offers the update on launch.

---

## Source layout

| Path | What |
|---|---|
| `src/`, `Cargo.toml`, `build.rs` | **The game** (Rust). Desktop releases (Windows / macOS / Linux) are built from here. |
| `icons/`, `sounds/`, `fonts/` | Game files, built into the executable. Shared with `python/` and the website. |
| `python/` | The original Python + pygame-ce version, kept as the reference for the parity checks. |
| `tools/` | Parity checks: the Rust build must match `python/` pixel for pixel. |
| `website/`, `backend/` | Landing page and the e-mail backend. |

Run from source (needs Rust, SDL2, FreeType and HarfBuzz from your package manager):

```bash
cargo run --release
```

Parity checks: `lucky-verities --shots DIR` against `tools/screens_reference.py python DIR`, and
`--sim DIR` against `tools/sim_reference.py python DIR`, compared with `tools/compare_png.py`.

Releases: push a tag `vX.Y.Z` (higher than the last one). `.github/workflows/build.yml` builds the desktop
files; the release files keep the old names, so every installed copy (including the
older Python builds) updates itself.

---

## License

All rights reserved unless stated otherwise. Ask before redistributing builds.
