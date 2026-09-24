"""Python counterpart of `lucky-verities --sim DIR`: the same scripted clicks / keys on a fake clock.

Run with SDL_VIDEODRIVER=dummy SDL_AUDIODRIVER=dummy and an empty LUCKY_VERITIES_SAVE_DIR:
    python3 tools/sim_reference.py /path/to/lucky-sahurs/python OUT_DIR
"""

import json
import os
import random
import sys
import time

GAME_DIR, OUT = sys.argv[1], sys.argv[2]
sys.path.insert(0, GAME_DIR)
os.chdir(GAME_DIR)

T0 = 1_790_000_000.0
clock = [T0]
time.time = lambda: clock[0]
time.perf_counter = lambda: clock[0]
random.seed(11)

import pygame  # noqa: E402
import main  # noqa: E402
from config import AUTOSAVE_INTERVAL  # noqa: E402

g = main.Game()
g.screen = pygame.Surface((1422, 800))
g.gpu_scaled = False
g.recompute_layout()
g.mouse_canvas = lambda: (-100, -100)
pygame.time.get_ticks = lambda: 0

script = [(3, (710, 392), None), (8, (359, 447), None)]
script += [(f, (711, 572), None) for f in range(20, 1220, 4)]
script += [(1230, (1373, 337), None), (1300, (1185, 170), None)]
script += [(f, (1185, 311), None) for f in range(1320, 1400, 5)]
script += [(f, (476, 572), None) for f in range(1420, 1800, 4)]
script += [(1810, None, pygame.K_ESCAPE), (1830, (49, 287), None), (1880, (185, 203), None),
           (1890, (185, 161), None), (1900, None, pygame.K_ESCAPE)]
script += [(2400, (1373, 337), None), (2460, (1185, 170), None)]
script += [(f, (1185, 311), None) for f in range(2500, 3500, 30)]
script += [(3550, None, pygame.K_ESCAPE), (3560, (1327, 35), None), (3570, (709, 277), None),
           (3580, (600, 350), None), (3600, None, pygame.K_ESCAPE)]


def frame(dt):
    """main.Game.run(), one iteration (without the clock and the perf log)."""
    g.worker.poll()
    g.tick_updater()
    g.tick_theme(dt)
    g.handle_events()
    if g.screen_mode == "game":
        gain = g.state.income_per_second() * dt
        g.state.coins += gain
        g.state.total_coins_earned += gain
        g.state.playtime += dt
        g.state.last_seen = time.time()
        g.update_auto(dt)
        g.update_cutscenes(dt)
        g.update_roll_rate(dt)
        g.check_milestones()
        g.state.ensure_daily_missions()
        g.right_panel.update(dt)
        g.left_panel.update(dt)
        g.autosave_timer += dt
        if g.autosave_timer >= AUTOSAVE_INTERVAL:
            g.autosave_timer = 0.0
            g.state.save()
        g.tick_online(dt)
    else:
        g._rate_prev = None
        g.roll_rate = 0.0
    if g.delete_confirm_slot is not None:
        g.delete_confirm_timer -= dt
        if g.delete_confirm_timer <= 0:
            g.delete_confirm_slot = None
    if g.rebirth_confirm:
        g.rebirth_confirm_timer -= dt
        if g.rebirth_confirm_timer <= 0:
            g.rebirth_confirm = False
    g.update_animations(dt)
    g.frame_dt = dt
    g.draw()


dt = 1.0 / 60.0
pygame.event.clear()
for n in range(3700):
    clock[0] += dt
    for f, click, key in script:
        if f != n:
            continue
        if click:
            pygame.event.post(pygame.event.Event(pygame.MOUSEBUTTONDOWN, button=1, pos=click))
        if key:
            pygame.event.post(pygame.event.Event(pygame.KEYDOWN, key=key, mod=0, unicode="", scancode=0))
    frame(dt)
    if n == 1400:
        pygame.image.save(g.canvas, os.path.join(OUT, "sim_1400.png"))
pygame.image.save(g.canvas, os.path.join(OUT, "sim_end.png"))
with open(os.path.join(OUT, "sim_state.json"), "w") as fh:
    fh.write(json.dumps(g.state.to_dict()))
os._exit(0)
