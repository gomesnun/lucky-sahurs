"""Compare same-named PNGs in two directories; writes <name>_diff.png (red = differing pixels).
Usage: python tools/compare_png.py <dirA> <dirB> [diffdir]"""
import os, sys
os.environ.setdefault("SDL_VIDEODRIVER", "dummy")
import pygame
a, b = sys.argv[1], sys.argv[2]
out = sys.argv[3] if len(sys.argv) > 3 else None
for name in sorted(os.listdir(a)):
    if not name.endswith(".png") or not os.path.exists(os.path.join(b, name)):
        continue
    sa, sb = pygame.image.load(os.path.join(a, name)), pygame.image.load(os.path.join(b, name))
    if sa.get_size() != sb.get_size():
        print("%s: size differs %s vs %s" % (name, sa.get_size(), sb.get_size())); continue
    w, h = sa.get_size()
    ba, bb = pygame.image.tobytes(sa, "RGB"), pygame.image.tobytes(sb, "RGB")
    diff, maxd, box = 0, 0, None
    dimg = pygame.Surface((w, h)) if out else None
    if dimg:
        dimg.blit(sb, (0, 0)); dimg.fill((90, 90, 90), special_flags=pygame.BLEND_RGB_MULT)
    for i in range(0, len(ba), 3):
        d = max(abs(ba[i] - bb[i]), abs(ba[i + 1] - bb[i + 1]), abs(ba[i + 2] - bb[i + 2]))
        if d:
            diff += 1; maxd = max(maxd, d)
            px = i // 3; x, y = px % w, px // w
            box = (min(box[0], x), min(box[1], y), max(box[2], x), max(box[3], y)) if box else (x, y, x, y)
            if dimg:
                dimg.set_at((x, y), (255, 0, 0))
    print("%s: differing px=%d max=%d bbox=%s" % (name, diff, maxd, box))
    if dimg and diff:
        pygame.image.save(dimg, os.path.join(out, name.replace(".png", "_diff.png")))
