"""Phase 2 / Phase 3 art for every verity (icons/pets/<slug>.png -> icons/pets/phases/<slug>_p2.png, _p3.png).

The verity keeps its own face "effects" (eyes, stars, brows, blush, tears...): only the mouth is swapped, the rest is
changed on top of it. The face is split into pieces (connected components) and the mouth is refilled from the ball.
  phase 2  wide creepy grin full of teeth instead of the mouth, dark bags behind the verity's own eyes
  phase 3  worn out: heavy droopy lids over the verity's own eyes, bags under them, flat wavy mouth
A face without separate eyes (closed ^ ^ eyes...) gets drawn ones instead.
Every image shares the same ball geometry (ui/cards.rs BALL_FRAC / BALL_CY), so faces land in the same place.
Run from the repo root: python3 tools/phases/faces.py
"""
import os
import numpy as np
from scipy import ndimage
from PIL import Image, ImageDraw, ImageFilter

SRC = "icons/pets"
DST = "icons/pets/phases"
BALL_FRAC, BALL_CY = 0.56, 0.542
SS = 4  # supersampling for the drawn faces


def _box(a, r, axis):
    """Box blur of radius r along an axis (edge-padded), via cumulative sums."""
    pad = [(0, 0)] * a.ndim
    pad[axis] = (r + 1, r)
    c = np.cumsum(np.pad(a, pad, mode="edge"), axis=axis)
    n = a.shape[axis]
    hi = np.take(c, np.arange(2 * r + 1, 2 * r + 1 + n), axis=axis)
    lo = np.take(c, np.arange(0, n), axis=axis)
    return (hi - lo) / (2 * r + 1)


def gblur(a, sigma):
    """Gaussian-like blur (3 box passes per axis) of an HxW or HxWxC float array."""
    r = max(1, int(round(sigma * 0.93)))
    for axis in (0, 1):
        for _ in range(3):
            a = _box(a, r, axis)
    return a


def peel(rgb, known, todo):
    """Fill the 'todo' pixels ring by ring with the mean of their known neighbours, then smooth them a little."""
    rgb = rgb.copy()
    known = known.copy()
    todo = todo & ~known
    while todo.any():
        num = gblur_box(rgb * known[..., None], 2)
        den = gblur_box(known.astype(np.float32), 2)
        ring = todo & (den > 0.12)
        if not ring.any():
            break
        rgb[ring] = num[ring] / den[ring][:, None]
        known = known | ring
        todo = todo & ~ring
    sm = gblur(rgb, 3.0)
    return sm


def gblur_box(a, r):
    for axis in (0, 1):
        a = _box(a, r, axis)
    return a


def erase_face(img, cx, cy, R):
    a = np.asarray(img, dtype=np.float32)
    rgb, alpha = a[..., :3], a[..., 3]
    h, w = alpha.shape
    yy, xx = np.mgrid[0:h, 0:w]
    ball = ((xx - cx) ** 2 + (yy - cy) ** 2) <= (0.96 * R) ** 2
    fx, fy, rx, ry = cx, cy + 0.12 * R, 0.72 * R, 0.50 * R
    zone = (((xx - fx) / rx) ** 2 + ((yy - fy) / ry) ** 2) <= 1.0
    # onion-peel fill: grow the ball's colours inward from the zone's edge (keeps split / rainbow balls apart)
    fill = peel(rgb, ball & ~zone, zone & ball)
    # which pixels in the zone are "face" (far from that fill)? eyes, mouth, brows, blush
    diff = np.sqrt(((rgb - fill) ** 2).sum(-1))
    feat = (diff > 34) & zone & ball
    fimg = Image.fromarray((feat * 255).astype(np.uint8)).filter(ImageFilter.MaxFilter(11))
    feat = (np.asarray(fimg) > 0) & ball
    # second pass: only the face pixels are unknown now, so the fill follows the ball more closely
    fill = peel(rgb, ball & ~feat, feat)
    # the face's strong strokes against that closer fill, to split it into separate pieces (eyes, mouth, blush...)
    diff = np.sqrt(((rgb - fill) ** 2).sum(-1))
    lum_w = np.array([0.299, 0.587, 0.114], dtype=np.float32)
    raw = (np.abs((rgb - fill) @ lum_w) > 50) & feat  # brightness strokes: a colourful texture alone doesn't count
    f = gblur(feat.astype(np.float32), 2.0)[..., None]
    out = rgb * (1 - f) + fill * f
    res = np.dstack([out, alpha]).clip(0, 255).astype(np.uint8)
    lum = float((fill[zone & ball] * [0.299, 0.587, 0.114]).sum(-1).mean())
    base = fill[zone & ball].mean(0)
    return {"clean": Image.fromarray(res, "RGBA"), "lum": lum, "base": base, "raw": raw, "feat": feat, "fill": fill, "rgb": rgb, "diff": diff}


def curve(x0, x1, fn, n=40):
    return [(x0 + (x1 - x0) * i / n, fn(x0 + (x1 - x0) * i / n)) for i in range(n + 1)]


def segment(e, cx, cy, R):
    """Splits the face: the eyes (kept and restyled), the mouth (replaced) and the rest (kept: brows, blush, tears,
    stars...). Eyes are the biggest stroke on each side of the upper face; the mouth is everything in the middle
    below them."""
    raw = e["raw"]
    h, w = raw.shape
    yy, xx = np.mgrid[0:h, 0:w]
    eyes = []
    for side in (-1, 1):
        area = raw & (side * (xx - cx) > 0.06 * R) & (np.abs(xx - cx) < 0.56 * R) & (yy > cy - 0.34 * R) & (yy < cy + 0.14 * R)
        labels, n = ndimage.label(area)
        best = None
        for i in range(1, n + 1):
            ys, xs = np.nonzero(labels == i)
            bw, bh = xs.max() - xs.min() + 1, ys.max() - ys.min() + 1
            # not a shadow cut by the top of the search area (headbands, hats), not a long flat strip
            if len(xs) < 60 or ys.min() <= cy - 0.33 * R or bw > 3.2 * bh:
                continue
            if best is None or len(xs) > len(best[0]):
                best = (xs, ys)
        if best is None:
            continue
        xs, ys = best
        # a thin stroke (a closed ^ eye, an arc) only gets a lid line, not a lid
        thin = len(xs) / max(1, (xs.max() - xs.min() + 1) * (ys.max() - ys.min() + 1)) < 0.38
        eyes.append((xs.min(), xs.max(), ys.min(), ys.max(), xs.mean(), ys.mean(), thin))
    if len(eyes) < 2:
        eyes = []
    eye_bottom = max(q[3] for q in eyes) if eyes else cy + 0.05 * R
    # the mouth: every face pixel in the middle band below the eyes (its inside included)
    band = (yy > eye_bottom + 0.03 * R) & (np.abs(xx - cx) < 0.44 * R)
    labels, n = ndimage.label(e["feat"] & band)
    mouth = np.zeros_like(raw)
    for i in range(1, n + 1):
        comp = labels == i
        ys, xs = np.nonzero(comp)
        # blush and tears sit to the sides and are soft: they stay
        if abs(xs.mean() - cx) > 0.34 * R and float(e["diff"][ys, xs].mean()) < 90:
            continue
        if (comp & raw).sum() >= 8:
            mouth |= comp
    mouth = ndimage.binary_dilation(ndimage.binary_fill_holes(mouth), iterations=3)
    return mouth, eyes


def ink_fix(layer, clean):
    """Drawn black lines turn light where the ball behind them is dark (and white glints turn dark)."""
    rgb = layer[..., :3]
    bg = np.asarray(clean, dtype=np.float32)[..., :3]
    bg_dark = gblur((bg * [0.299, 0.587, 0.114]).sum(-1), 6.0) < 75
    ink = (rgb.max(-1) < 40) & (layer[..., 3] > 0) & bg_dark
    glint = (rgb.min(-1) > 252) & bg_dark
    rgb[ink] = (238, 234, 250)
    rgb[glint] = (20, 14, 26)
    return layer


def canvas(size):
    S = size * SS
    L = Image.new("RGBA", (S, S), (0, 0, 0, 0))
    return L, ImageDraw.Draw(L)


def down(L, size):
    return np.asarray(L.resize((size, size), Image.LANCZOS)).astype(np.float32)


def over(dst, layer):
    """dst (HxWx3 float) with an RGBA float layer on top."""
    a = layer[..., 3:4] / 255.0
    return dst * (1 - a) + layer[..., :3] * a


def bags_layer(size, cx, cy, R, phase, eyes, base, dark):
    L, d = canvas(size)
    k = SS
    col = (150, 70, 190, 150) if dark else (int(base[0] * 0.45 + 40), int(base[1] * 0.25), int(base[2] * 0.25 + 10), 150)
    spots = [((x0 + x1) / 2, y0, y1, (x1 - x0) / 2) for x0, x1, y0, y1, *_ in eyes] or [
        (cx + sx * 0.3 * R, cy - 0.2 * R, cy + 0.05 * R, 0.09 * R) for sx in (-1, 1)]
    for ex, y0, y1, hw in spots:
        rw = max(hw * 1.6, 0.16 * R)
        if phase == 2:  # dark rings around the whole eye
            d.ellipse(((ex - rw) * k, (y0 - 0.08 * R) * k, (ex + rw) * k, (y1 + 0.10 * R) * k), fill=col)
        else:  # heavy bags under it
            d.ellipse(((ex - rw) * k, ((y0 + y1) / 2) * k, (ex + rw) * k, (y1 + 0.13 * R) * k), fill=col)
    L = L.filter(ImageFilter.GaussianBlur(0.06 * R * k))
    return down(L, size)


def grin_layer(size, cx, cy, R):
    L, d = canvas(size)
    k = SS
    X = lambda u: (cx + u * R) * k
    Y = lambda v: (cy + v * R) * k
    ink = (14, 10, 12, 255)
    W = 0.66
    up = lambda u: 0.20 + 0.06 * (1 - (u / W) ** 2) - 0.26 * (u / W) ** 2
    lo = lambda u: 0.20 + 0.40 * (1 - (u / W) ** 2) - 0.26 * (u / W) ** 2
    top = curve(-W, W, up)
    bot = curve(W, -W, lo)
    o = 0.045  # outline thickness
    d.polygon([(X(u), Y(v - o)) for u, v in top] + [(X(u), Y(v + o)) for u, v in bot], fill=ink)
    d.polygon([(X(u), Y(v)) for u, v in top + bot], fill=(70, 12, 24, 255))
    # teeth: two rows split by a gum curve
    mid = lambda u: (up(u) + lo(u)) / 2 + 0.01
    n, gap = 11, 0.012
    for row in (0, 1):
        for i in range(n):
            u0 = -W + 2 * W * i / n + gap
            u1 = -W + 2 * W * (i + 1) / n - gap
            a, b = (up, lambda u: mid(u) - 0.012) if row == 0 else (lambda u: mid(u) + 0.012, lo)
            if b((u0 + u1) / 2) - a((u0 + u1) / 2) < 0.03:
                continue
            pts = curve(u0, u1, lambda u: a(u) + 0.01, 6) + curve(u1, u0, lambda u: b(u) - 0.01, 6)
            d.polygon([(X(u), Y(v)) for u, v in pts], fill=(250, 248, 240, 255))
    # jagged corners
    for sx in (-1, 1):
        u, v = sx * W, up(W)
        for du, dv in ((0.07, -0.13), (0.11, -0.05), (0.03, -0.16)):
            d.line((X(u - sx * 0.02), Y(v + 0.02), X(u + sx * du), Y(v + dv)), fill=ink, width=int(0.035 * R * k))
    return down(L, size)


def plain_eyes_layer(size, cx, cy, R, phase):
    """Eyes for a face whose own eyes couldn't be told apart (closed ^ ^ eyes and such)."""
    L, d = canvas(size)
    k = SS
    X = lambda u: (cx + u * R) * k
    Y = lambda v: (cy + v * R) * k
    ink = (14, 10, 12, 255)
    for sx in (-1, 1):
        if phase == 2:
            d.ellipse((X(sx * 0.30 - 0.085), Y(-0.24), X(sx * 0.30 + 0.085), Y(0.03)), fill=ink)
        else:
            ex = sx * 0.31
            lid = lambda u: -0.02 + sx * 0.02 * (u - ex) / 0.17
            low = lambda u: 0.07 - 0.075 * ((u - ex) / 0.17) ** 2
            d.polygon([(X(u), Y(v)) for u, v in curve(ex - 0.17, ex + 0.17, lid) + curve(ex + 0.17, ex - 0.17, low)], fill=ink)
            d.line([(X(u), Y(v - 0.01)) for u, v in curve(ex - 0.20, ex + 0.20, lid, 12)], fill=ink, width=int(0.05 * R * k), joint="curve")
            d.ellipse((X(ex - 0.03), Y(0.005), X(ex + 0.03), Y(0.065)), fill=(255, 255, 255, 255))
    return down(L, size)


def lids(size, cx, cy, R, eyes):
    """Heavy droopy lids over the verity's own eyes: a mask to paint with the ball colour, and the lid lines."""
    M, dm = canvas(size)
    L, dl = canvas(size)
    k = SS
    ink = (14, 10, 12, 255)
    for x0, x1, y0, y1, ex, ey, thin in eyes:
        outer = 1 if ex > cx else -1  # the outer corner droops (sad, tired)
        hgt = min(y1 - y0, 0.30 * R)
        pad = 0.03 * R
        xa, xb = x0 - pad, x1 + pad
        drop = 0.08 * R
        yin = y0 + (0.30 if thin else 0.50) * hgt
        yout = yin + drop
        ya, yb = (yout, yin) if outer < 0 else (yin, yout)
        if not thin:
            dm.polygon([(xa * k, (y0 - 0.02 * R) * k), (xb * k, (y0 - 0.02 * R) * k), (xb * k, yb * k), (xa * k, ya * k)], fill=(255, 255, 255, 255))
        dl.line([((xa - outer * 0.02 * R) * k, ya * k), ((xb + outer * 0.02 * R) * k, yb * k)], fill=ink, width=int(0.055 * R * k))
    return down(M, size)[..., 3] / 255.0, down(L, size)


def mouth3_layer(size, cx, cy, R):
    L, d = canvas(size)
    k = SS
    X = lambda u: (cx + u * R) * k
    Y = lambda v: (cy + v * R) * k
    ink = (14, 10, 12, 255)
    wav = lambda u: 0.45 + 0.02 * np.sin(u * 16) + 0.03 * (u / 0.22) ** 2
    d.line([(X(u), Y(v)) for u, v in curve(-0.22, 0.22, wav, 30)], fill=ink, width=int(0.06 * R * k), joint="curve")
    for sx in (-1, 1):
        u = sx * 0.22
        d.ellipse((X(u) - 0.03 * R * k, Y(wav(u)) - 0.03 * R * k, X(u) + 0.03 * R * k, Y(wav(u)) + 0.03 * R * k), fill=ink)
    return down(L, size)


def make(slug):
    img = Image.open(f"{SRC}/{slug}.png").convert("RGBA")
    size = img.size[0]
    R, cx, cy = BALL_FRAC * size / 2, size / 2, BALL_CY * size
    e = erase_face(img, cx, cy, R)
    clean, base, dark = e["clean"], e["base"], e["lum"] < 70
    mouth, eyes = segment(e, cx, cy, R)
    # the verity's own face minus its mouth, feathered
    keep = gblur((e["feat"] & ~mouth).astype(np.float32), 1.5)[..., None]
    own = np.dstack([e["rgb"], keep[..., 0] * 255])
    alpha = np.asarray(clean, dtype=np.float32)[..., 3]
    base_rgb = np.asarray(clean, dtype=np.float32)[..., :3]
    outs = {}
    for phase in (2, 3):
        out = over(base_rgb, bags_layer(size, cx, cy, R, phase, eyes, base, dark))
        out = over(out, own)
        if phase == 2:
            if not eyes:
                out = over(out, ink_fix(plain_eyes_layer(size, cx, cy, R, 2), clean))
            out = over(out, ink_fix(grin_layer(size, cx, cy, R), clean))
        else:
            if eyes:
                m, lines = lids(size, cx, cy, R, eyes)
                m = m * gblur(e["feat"].astype(np.float32), 1.5)  # only over the eye itself: the ball around it stays
                out = out * (1 - m[..., None]) + e["fill"] * m[..., None]
                out = over(out, ink_fix(lines, clean))
            else:
                out = over(out, ink_fix(plain_eyes_layer(size, cx, cy, R, 3), clean))
            out = over(out, ink_fix(mouth3_layer(size, cx, cy, R), clean))
        outs[phase] = Image.fromarray(np.dstack([out, alpha]).clip(0, 255).astype(np.uint8), "RGBA")
    return outs, clean, base


if __name__ == "__main__":
    import sys
    os.makedirs(DST, exist_ok=True)
    os.makedirs("tools/phases/clean", exist_ok=True)  # the balls without a face: the Monster's skin (monster.mjs)
    slugs = sys.argv[1:] or sorted(f[:-4] for f in os.listdir(SRC) if f.endswith(".png"))
    import json
    colors = {}
    for s in slugs:
        outs, clean, base = make(s)
        colors[s] = [int(c) for c in base]
        clean.save(f"tools/phases/clean/{s}.png")
        for p, o in outs.items():
            o.save(f"{DST}/{s}_p{p}.png", optimize=True)
        print(s, "ok")
    if len(slugs) > 1:
        json.dump(colors, open("tools/phases/colors.json", "w"), indent=0)
