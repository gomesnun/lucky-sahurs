"""Som: mixer, efeitos gerados por código e volume."""

import array
import math
import time
import pygame

from core.pets import RARITIES, TIER_SECRET
from storage import save_settings
from ui.icons import find_asset


def make_tone_bytes(freq=440, duration=0.12, volume=0.35, sample_rate=22050,
                    freq_end=None, channels=2):
    """Gera um som simples (bip/sweep) em bytes PCM 16-bit, sem precisar de
    nenhum ficheiro de áudio nem ligação à internet."""
    n = max(1, int(sample_rate * duration))
    buf = array.array("h")
    amp = int(32767 * volume)
    fade = max(1, int(sample_rate * 0.015))
    for i in range(n):
        t = i / sample_rate
        f = freq if freq_end is None else freq + (freq_end - freq) * (i / max(1, n - 1))
        env = 1.0
        if i < fade:
            env = i / fade
        elif i > n - fade:
            env = (n - i) / fade
        sample = int(amp * env * math.sin(2 * math.pi * f * t))
        for _ in range(channels):
            buf.append(sample)
    return buf.tobytes()


# Sons que vêm de ficheiros (pasta sounds/): nome do efeito -> (ficheiro, volume relativo 0..1).
# Quem quiser trocar um som só tem de substituir o ficheiro (ou apagá-lo: volta o bip gerado por código).
# Os créditos de cada um estão em sounds/CREDITS.txt e no botão Credits do menu principal.
SFX_FILES = {
    "click": ("click.ogg", 0.75),             # abrir a mochila / traits / rebirth / painéis... (quase todos os botões)
    "buy": ("upgrade.ogg", 0.55),             # comprar upgrades
    "milestone": ("achievement.ogg", 0.50),   # conquistas (milestones)
    "rebirth": ("rebirth.ogg", 0.90),         # fazer um Rebirth (só 1.5 s do "Holy Protection Skill Buff")
}
MUSIC_FILE = "music.ogg"                      # música de fundo (em loop)
MUSIC_SCALE = 0.60                            # a música fica sempre mais baixa do que os efeitos (com o slider
                                              # Music a 50%, que é o normal, dá os mesmos 0.30 de antes)

# A que grupo do menu Options > SFX pertence cada som (grupos: ver SFX_CATEGORIES em storage.py).
# Um som que não esteja aqui toca sempre (só depende dos volumes).
SFX_CATEGORY_OF = {
    "click": "click", "equip": "click",
    "roll": "roll", "cyclic": "roll", "mut_golden": "roll", "mut_diamond": "roll",
    "trait_charge": "traits", "trait_roll": "traits",
    "buy": "upgrades",
    "milestone": "milestones",
    "rebirth": "rebirth",
}
for _tier in range(2, 11):
    SFX_CATEGORY_OF["rar_%d" % _tier] = "roll"       # o som de cada raridade rolada

# Som que se ouve como "amostra" quando se volta a ligar um grupo nas Options.
SFX_PREVIEW = {"click": "click", "roll": "roll", "traits": "trait_charge",
               "upgrades": "buy", "milestones": "milestone", "rebirth": "rebirth"}

# Sliders das Options: nome -> definição guardada. "master" é o volume que já existia.
SLIDER_SETTINGS = {"master": "volume", "music": "music_volume", "sfx": "sfx_volume"}
SLIDER_KNOB_R = 10        # raio da bolinha (usado ao desenhar e ao ler o rato)

# Auto Roller muito rápido (mais rolls por segundo do que isto): fica em silêncio, porque seria um barulho
# constante. Só toca o som de um pet Secret ou melhor (tier >= AUTO_QUIET_MIN_TIER). Para o silêncio ser total,
# põe AUTO_QUIET_MIN_TIER = 99.
AUTO_QUIET_RPS = 3.0
AUTO_QUIET_MIN_TIER = TIER_SECRET


def _level(value, default):
    try:
        return max(0.0, min(1.0, float(value)))
    except (TypeError, ValueError):
        return default


class AudioMixin:
    """Som: mixer, efeitos gerados por código e volume."""

    # ---------------------------------------------------------------- som
    def init_mixer(self):
        try:
            if pygame.mixer.get_init() is None:
                pygame.mixer.init(22050, -16, 2, 512)
            info = pygame.mixer.get_init()
            return info is not None and info[1] in (-16, 32784)
        except pygame.error:
            return False

    def build_sounds(self):
        """Cria os efeitos sonoros (tudo gerado por código, sem ficheiros)."""
        self.sounds = {}
        self.sfx_gain = {}
        if not self.mixer_ok:
            return
        try:
            rate, _fmt, ch = pygame.mixer.get_init()

            def tone(f, d, v=0.30, f2=None):
                return make_tone_bytes(f, d, v, rate, f2, ch)

            def seq(notes, d=0.08, v=0.30):
                return b"".join(tone(n, d, v) for n in notes)

            raw = {
                "click": tone(720, 0.04, 0.22),
                "equip": tone(520, 0.05, 0.24, 700),
                "roll": tone(300, 0.09, 0.26, 520),
                "buy": seq([660, 880], 0.07),
                "milestone": seq([523, 659, 784, 1047], 0.10),
                "rebirth": seq([392, 523, 659, 784, 1047], 0.09),
                "trait_charge": seq([880, 1175], 0.07),
                "trait_roll": tone(400, 0.16, 0.28, 900),
                "cyclic": seq([784, 988, 1175], 0.06, 0.26),
                "mut_golden": seq([880, 1109, 1319], 0.06),
                "mut_diamond": seq([1047, 1319, 1568, 2093], 0.06),
                "rar_2": seq([523, 659], 0.07),
                "rar_3": seq([523, 659, 784], 0.08),
                "rar_4": seq([523, 659, 784, 1047], 0.08),
                "rar_5": seq([440, 554, 659, 880, 1108], 0.08),
                "rar_6": seq([523, 659, 784, 1047, 1319], 0.08),
                "rar_7": seq([392, 494, 587, 784, 988, 1175], 0.09),
                "rar_8": seq([523, 659, 784, 1047, 1319, 1568, 2093], 0.10),
                "rar_9": seq([392, 523, 659, 784, 1047, 1319, 1568, 2093], 0.10),
                "rar_10": seq([523, 659, 784, 1047, 1319, 1568, 2093, 2637, 3136], 0.11),
            }
            for name, data in raw.items():
                self.sounds[name] = pygame.mixer.Sound(buffer=data)
        except (pygame.error, ValueError, TypeError):
            self.sounds = {}
        self.load_sound_files()

    def load_sound_files(self):
        """Troca os bips gerados por código pelos sons da pasta sounds/ (se existirem e carregarem bem)."""
        for name, (filename, gain) in SFX_FILES.items():
            path = find_asset("sounds", filename)
            if not path:
                continue
            try:
                self.sounds[name] = pygame.mixer.Sound(path)
                self.sfx_gain[name] = gain
            except (pygame.error, OSError):
                pass          # fica o som gerado por código (se houver)

    def start_music(self):
        """Música de fundo (sounds/music.ogg) em loop, desde o menu. Sem ficheiro ou sem áudio: silêncio."""
        self.music_ok = False
        if not self.mixer_ok:
            return
        path = find_asset("sounds", MUSIC_FILE)
        if not path:
            return
        try:
            pygame.mixer.music.load(path)
            pygame.mixer.music.play(-1)
            self.music_ok = True
        except pygame.error:
            return
        self.apply_music_volume()

    def apply_music_volume(self):
        if not getattr(self, "music_ok", False):
            return
        on = self.settings.get("sound_on", True) and self.settings.get("music_on", True)
        vol = _level(self.settings.get("volume", 0.6), 0.6)                  # volume mestre
        mus = _level(self.settings.get("music_volume", 0.5), 0.5)            # volume da música
        try:
            pygame.mixer.music.set_volume(vol * mus * MUSIC_SCALE if on else 0.0)
        except pygame.error:
            pass

    def toggle_music(self):
        self.settings["music_on"] = not self.settings.get("music_on", True)
        save_settings(self.settings)
        self.apply_music_volume()

    def apply_volume(self):
        vol = _level(self.settings.get("volume", 0.6), 0.6) * _level(self.settings.get("sfx_volume", 1.0), 1.0)
        for name, snd in self.sounds.items():
            snd.set_volume(min(1.0, vol * self.sfx_gain.get(name, 1.0)))
        self.apply_music_volume()

    def play(self, name, min_gap=0.0):
        cfg = self.settings
        if not cfg.get("sound_on", True) or not cfg.get("sfx_on", True):
            return
        if cfg.get("volume", 0.0) <= 0.0 or cfg.get("sfx_volume", 1.0) <= 0.0:
            return
        category = SFX_CATEGORY_OF.get(name)
        if category and not cfg.get("sfx_" + category, True):
            return          # este grupo de sons está desligado nas Options > SFX
        snd = self.sounds.get(name)
        if snd is None:
            return
        now = time.time()
        if min_gap and now - self.last_sfx.get(name, 0.0) < min_gap:
            return
        self.last_sfx[name] = now
        try:
            snd.play()
        except pygame.error:
            pass

    def play_roll_sfx(self, r_idx, mutation, manual=True, quiet=False):
        tier = RARITIES[r_idx]["tier"]
        if not manual and tier < 3 and mutation != "diamond":
            return      # o Auto Roller só faz barulho para coisas boas (senão era um chinfrim)
        if not manual and quiet and tier < AUTO_QUIET_MIN_TIER:
            return      # Auto Roller rapidíssimo: só se ouve o que for mesmo raro
        if tier >= 2:
            name = "rar_%d" % tier
        elif mutation != "normal":
            name = "mut_" + mutation
        else:
            name = "roll"
        self.play(name, min_gap=0.0 if manual else 0.35)

    def toggle_sound(self):
        self.settings["sound_on"] = not self.settings.get("sound_on", True)
        save_settings(self.settings)
        self.apply_music_volume()          # "Sound: Off" cala também a música
        if self.settings["sound_on"]:
            self.play("click")

    def toggle_sfx(self):
        """Liga/desliga TODOS os efeitos sonoros (a música não é afetada)."""
        self.settings["sfx_on"] = not self.settings.get("sfx_on", True)
        save_settings(self.settings)
        if self.settings["sfx_on"]:
            self.play("click")

    def toggle_sfx_category(self, category):
        """Liga/desliga um grupo de sons (Options > SFX). Ao ligar, toca uma amostra desse grupo."""
        key = "sfx_" + category
        self.settings[key] = not self.settings.get(key, True)
        save_settings(self.settings)
        if self.settings[key]:
            self.play(SFX_PREVIEW.get(category, "click"))

    def set_slider_from_pos(self, slider, canvas_pos):
        """Arrastar um slider das Options (master / music / sfx). A bolinha anda entre as pontas da barra."""
        bar = self.slider_bars.get(slider)
        span = 0 if bar is None else bar.width - 2 * SLIDER_KNOB_R
        if span <= 0:
            return
        frac = (canvas_pos[0] - (bar.x + SLIDER_KNOB_R)) / float(span)
        self.settings[SLIDER_SETTINGS[slider]] = round(max(0.0, min(1.0, frac)), 2)
        self.apply_volume()
