"""Python counterpart of `lucky-verities --bench`: ms per draw of a few screens (canvas only)."""
import os, random, sys, time
GAME_DIR = sys.argv[1]
sys.path.insert(0, GAME_DIR); os.chdir(GAME_DIR)
FAKE = 1_790_000_000.0
real_perf = time.perf_counter
time.time = lambda: FAKE
random.seed(7)
import pygame, main
g = main.Game()
g.screen = pygame.Surface((1422, 800)); g.gpu_scaled = False; g.recompute_layout()
g.mouse_canvas = lambda: (-100, -100)
pygame.display.flip = lambda: None
g.canvas_is_screen = False
def draw_canvas_only():
    scr = g.screen
    g.screen = g.canvas   # skip the final smoothscale to the window, like --bench in Rust
    g.draw()
    g.screen = scr
def run(name):
    for _ in range(5): draw_canvas_only()
    n = 200; s = real_perf()
    for _ in range(n): draw_canvas_only()
    print("%-14s %.3f ms" % (name, (real_perf() - s) * 1000 / n))
run("title")
g.start_slot(1)
for _ in range(4000): g.state.roll()
g.state.equip_best()
run("game")
g.right_panel.open("index"); g.right_panel.update(5.0); run("index")
g.right_panel.close(); g.right_panel.update(5.0)
g.left_panel.open("bag"); g.left_panel.update(5.0); run("bag")
g.left_panel.close(); g.left_panel.update(5.0)
g.toggle_traits(); run("traits")
sys.stdout.flush(); os._exit(0)
