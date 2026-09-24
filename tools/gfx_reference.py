"""Reference renderer: same operations as `lucky-verities --selftest`, done with pygame-ce.
Usage: python tools/gfx_reference.py <outdir>   (then tools/compare.py compares the dumps)"""
import os, struct, sys
os.environ["SDL_VIDEODRIVER"] = "dummy"
os.environ["SDL_AUDIODRIVER"] = "dummy"
import pygame
pygame.init()
pygame.display.set_mode((10, 10))

def dump(s, path):
    w, h = s.get_size()
    alpha = bool(s.get_flags() & pygame.SRCALPHA)
    out = bytearray(struct.pack("<II", w, h))
    for y in range(h):
        for x in range(w):
            c = s.get_at((x, y))
            out += bytes((c.r, c.g, c.b, c.a if alpha else 255))
    open(path, "wb").write(out)

d = sys.argv[1]
s = pygame.Surface((300, 200))
s.fill((30, 40, 50))
pygame.draw.rect(s, (200, 100, 50), (10, 10, 80, 50), border_radius=12)
pygame.draw.rect(s, (8, 8, 10), (10, 10, 80, 50), width=3, border_radius=12)
pygame.draw.rect(s, (255, 205, 60), (100, 10, 60, 40), width=4)
pygame.draw.rect(s, (90, 200, 90), (170, 10, 40, 40), width=2, border_radius=20)
pygame.draw.circle(s, (250, 250, 250), (50, 120), 30)
pygame.draw.circle(s, (250, 50, 50), (120, 120), 25, 4)
pygame.draw.circle(s, (50, 50, 250), (180, 120), 20, 1)
pygame.draw.line(s, (255, 255, 0), (10, 180), (290, 150), 3)
pygame.draw.line(s, (0, 255, 255), (220, 20), (290, 90), 1)
pygame.draw.polygon(s, (160, 60, 220), [(220, 100), (290, 110), (270, 170), (230, 160)])
pygame.draw.polygon(s, (60, 160, 220), [(200, 180), (240, 195), (210, 199)])
pygame.draw.ellipse(s, (0, 0, 0), (240, 20, 50, 26))
pygame.draw.ellipse(s, (255, 128, 0), (10, 160, 60, 30), 3)
pygame.draw.arc(s, (255, 255, 255), (130, 150, 60, 40), 0.3, 2.8, 4)
dump(s, d + "/t1.bin")

a = pygame.Surface((120, 90), pygame.SRCALPHA)
pygame.draw.rect(a, (200, 60, 60, 180), (0, 0, 120, 90), border_radius=16)
pygame.draw.circle(a, (20, 220, 90, 90), (60, 45), 30)
mask = pygame.Surface((120, 90), pygame.SRCALPHA)
pygame.draw.rect(mask, (255, 255, 255, 255), (0, 0, 120, 90), border_radius=30)
a.blit(mask, (0, 0), special_flags=pygame.BLEND_RGBA_MIN)
base = pygame.Surface((300, 200))
base.fill((240, 235, 225))
base.blit(a, (10, 10))
b = a.copy(); b.set_alpha(120)
base.blit(b, (150, 20))
base.blit(pygame.transform.smoothscale(a, (57, 41)), (20, 120))
base.blit(pygame.transform.smoothscale(a, (150, 70)), (140, 120))
base.blit(pygame.transform.rotozoom(a, 17.0, 0.6), (90, 100))
m2 = a.copy(); m2.fill((24, 26, 44, 255), special_flags=pygame.BLEND_RGB_MULT)
base.blit(m2, (200, 5))
sa = pygame.Surface((100, 60), pygame.SRCALPHA)
sa.blit(a, (-10, -10)); sa.blit(m2, (20, 15))
base.blit(sa, (5, 5))
dump(base, d + "/t2.bin")
dump(sa, d + "/t2b.bin")

f = pygame.font.Font(os.path.join(os.path.dirname(__file__), "..", "assets", "fonts", "Fredoka-Bold.ttf"), 19)
t = f.render("Roll pets, equip them! $1.23M", True, (255, 184, 48))
print("size", f.size("Roll pets, equip them! $1.23M"), "h", f.get_height(), "render", t.get_size())
dump(t, d + "/t3.bin")
