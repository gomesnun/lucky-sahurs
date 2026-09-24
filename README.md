<h1 align="center">Lucky Verities</h1>

<p align="center">
  <b>An idle collecting game about Verities: little round creatures with big personalities.</b><br>
  Roll for pets from <i>Common</i> to <i>Transcendent</i>, stack income, rebirth for permanent power,
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

Press **ROLL**. Get a pet. The rare ones pay better. The rarest one is a **1-in-500,000,000,000**.

| | |
|---|---|
| **11** rarity tiers | Common → Uncommon → Rare → Epic → Legendary → Mythic → Exotic → Secret → Divine → Cosmic → Transcendent |
| **22** collectible Verities | each with its own income curve |
| **3** mutations | Golden / Diamond / and the one you find yourself |
| **12** traits | permanent modifiers rolled on top of pets |
| **Milestones** | long-run reward tracks across every category |
| **Rebirths** | wipe progress, keep permanent multipliers |
| **Daily missions** | rotating objectives, offline earnings while you sleep |
| **Cloud saves** | 3 slots, Firebase-backed, optional account |
| **Leaderboard** | global ranking |
| **Friends** | search a player, send a request, follow their stats |
| **Profile photo** | any Verity you own, Golden and Diamond ring included |
| **Languages** | English + Português |

Everything runs offline too — the account is optional.

---

## Install

Pick your platform. Fastest path is the prebuilt download; no Python needed.

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
sudo apt update && sudo apt install -y git curl build-essential pkg-config libsdl2-dev libfreetype-dev libharfbuzz-dev && curl -sSf https://sh.rustup.rs | sh -s -- -y --profile minimal && . "$HOME/.cargo/env" && { git clone --depth 1 https://github.com/gomesnun/lucky-verities ~/lucky-verities || git -C ~/lucky-verities pull; } && cd ~/lucky-verities && cargo build --release && install -Dm755 target/release/lucky-verities ~/.local/bin/lucky-verities
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
