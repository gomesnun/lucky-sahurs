"""Builds the OG verities' art (Verity, Lovity, Falsity, Cruelty) as shaded 3D balls.

The originals were flat emoji discs with a thick navy outline; on the game's 3D pet balls that outline showed as a
dark stripe across the middle whenever the pet turned sideways. This keeps the faces (taken from Verity's own
Phase 1-3 art), drops the outline and lights each ball like a sphere, in each OG's colour. Lovity gets its open,
laughing mouth. Also makes Cruelty's Monster art and its "clean" ball (no face, used to colour the 3D monster).

    pip install pillow && python3 tools/og_art.py
"""

import colorsys
import math

from PIL import Image, ImageDraw, ImageFilter

PETS = "icons/pets"
SRC = "tools/og_src"  # the original flat art the faces come from (never overwritten)
SIZE = 512
CX, CY, R = 255.5, 277.5, 143.5  # where every verity's ball sits in its 512x512 art
FACE_R = R * 0.9  # the old outline started here
SRC_BODY = (255, 190, 45)  # Verity's yellow, to un-mix the faces' anti-aliased edges

COLORS = {
    "verity": (255, 196, 48),
    "lovity": (255, 72, 150),
    "falsity": (48, 154, 255),
    "cruelty": (222, 30, 38),
}

LIGHT = (-0.5, -0.62, 0.61)
_l = math.sqrt(sum(c * c for c in LIGHT))
LIGHT = tuple(c / _l for c in LIGHT)
# where the shiny highlight sits: up by the top-left rim, clear of the eyes (it washed the left eye out)
HALF = (-0.42, -0.58, 0.0)
HALF = (HALF[0], HALF[1], math.sqrt(1 - HALF[0] ** 2 - HALF[1] ** 2))


def face_layer(src, keep=lambda x, y: True):
    """The face drawn on Verity's art (eyes, mouth, teeth), as an RGBA layer with the yellow taken out."""
    px = src.load()
    out = Image.new("RGBA", (SIZE, SIZE), (0, 0, 0, 0))
    op = out.load()
    for y in range(SIZE):
        for x in range(SIZE):
            if math.hypot(x + 0.5 - CX, y + 0.5 - CY) > FACE_R or not keep(x, y):
                continue
            r, g, b, a = px[x, y]
            if a == 0:
                continue
            h, s, v = colorsys.rgb_to_hsv(r / 255, g / 255, b / 255)
            # the old painted-on shine at the top left isn't part of the face
            if s < 0.3 and v > 0.8 and y < CY and x < CX - 40:
                continue
            yellow = min(r, g) - b
            alpha = max(0.0, min(1.0, (110 - yellow) / 100))
            if alpha <= 0.02:
                continue
            # un-mix the yellow body out of the edge pixels so there's no yellow halo on other colours
            col = [max(0, min(255, round((c - (1 - alpha) * bc) / alpha))) for c, bc in zip((r, g, b), SRC_BODY)]
            op[x, y] = (col[0], col[1], col[2], round(alpha * 255))
    return out


def lovity_mouth(layer):
    """Lovity laughs: a big open mouth with its tongue out (drawn 4x and scaled down, for smooth edges)."""
    k = 4
    big = Image.new("RGBA", (SIZE * k, SIZE * k), (0, 0, 0, 0))
    d = ImageDraw.Draw(big)
    w, top, depth = 150, CY + 12, 78
    box = [(CX - w / 2) * k, (top - depth) * k, (CX + w / 2) * k, (top + depth) * k]
    d.pieslice(box, 0, 180, fill=(20, 6, 30, 255))
    d.ellipse([(CX - 42) * k, (top + depth - 38) * k, (CX + 42) * k, (top + depth + 10) * k], fill=(255, 120, 170, 255))
    # keep the tongue inside the mouth
    mask = Image.new("L", big.size, 0)
    ImageDraw.Draw(mask).pieslice(box, 0, 180, fill=255)
    tongue_cut = Image.new("RGBA", big.size, (0, 0, 0, 0))
    tongue_cut.paste(big, (0, 0), mask)
    small = tongue_cut.resize((SIZE, SIZE), Image.LANCZOS)
    layer.alpha_composite(small)
    return layer


def ball(color, face):
    """A lit sphere in `color` with `face` painted on it (and lit with it)."""
    out = Image.new("RGBA", (SIZE, SIZE), (0, 0, 0, 0))
    op = out.load()
    fp = face.load()
    for y in range(SIZE):
        for x in range(SIZE):
            dx, dy = (x + 0.5 - CX) / R, (y + 0.5 - CY) / R
            d2 = dx * dx + dy * dy
            dist = math.sqrt(d2) * R
            cover = max(0.0, min(1.0, R - dist + 0.5))  # anti-aliased edge
            if cover <= 0:
                continue
            nz = math.sqrt(max(0.0, 1 - min(d2, 1.0)))
            n = (dx, dy, nz)
            diff = max(0.0, n[0] * LIGHT[0] + n[1] * LIGHT[1] + n[2] * LIGHT[2])
            shade = (0.42 + 0.7 * diff) * (0.78 + 0.22 * nz)
            # a little warm light bouncing back in from the bottom right
            bounce = max(0.0, 0.4 * dx + 0.55 * dy) ** 2 * 0.18
            fr, fg, fb, fa = fp[x, y]
            fa /= 255
            base = [c * (1 - fa) + fc * fa for c, fc in zip(color, (fr, fg, fb))]
            col = [c * shade + c * bounce for c in base]
            spec = max(0.0, n[0] * HALF[0] + n[1] * HALF[1] + n[2] * HALF[2]) ** 60 * 0.85
            col = [c + (255 - c) * spec for c in col]
            op[x, y] = tuple(max(0, min(255, round(c))) for c in col) + (round(255 * cover),)
    # a soft gloss band near the top left, like the other verities' art
    gloss = Image.new("L", (SIZE, SIZE), 0)
    ImageDraw.Draw(gloss).ellipse([CX - R * 0.7, CY - R * 0.95, CX + R * 0.1, CY - R * 0.58], fill=70)
    gloss = gloss.filter(ImageFilter.GaussianBlur(10))
    white = Image.new("RGBA", (SIZE, SIZE), (255, 255, 255, 0))
    disc = Image.new("L", (SIZE, SIZE), 0)
    ImageDraw.Draw(disc).ellipse([CX - R, CY - R, CX + R, CY + R], fill=255)
    white.putalpha(Image.composite(gloss, Image.new("L", (SIZE, SIZE), 0), disc))
    out.alpha_composite(white)
    return out


def recolor_red(img, sat_min=0.12):
    """Cruelty's red, from Lovity's pink art (same shading, hue moved to red)."""
    img = img.convert("RGBA")
    px = img.load()
    for y in range(img.height):
        for x in range(img.width):
            r, g, b, a = px[x, y]
            if a == 0:
                continue
            h, s, v = colorsys.rgb_to_hsv(r / 255, g / 255, b / 255)
            if s < sat_min:
                continue
            r2, g2, b2 = colorsys.hsv_to_rgb(0.995, min(1.0, s * 1.05), v * 0.92)
            px[x, y] = (round(r2 * 255), round(g2 * 255), round(b2 * 255), a)
    return img


def main():
    faces = {}
    for phase in (1, 2, 3):
        src = Image.open(f"{SRC}/verity_p{phase}.png").convert("RGBA")
        faces[phase] = face_layer(src)
    # Lovity keeps Verity's eyes, with its own laughing mouth
    lovity_face = lovity_mouth(face_layer(Image.open(f"{SRC}/verity_p1.png").convert("RGBA"), keep=lambda x, y: y < CY - 5))
    for name, color in COLORS.items():
        for phase in (1, 2, 3):
            face = lovity_face if (name == "lovity" and phase == 1) else faces[phase]
            path = f"{PETS}/{name}.png" if phase == 1 else f"{PETS}/phases/{name}_p{phase}.png"
            ball(color, face).save(path, optimize=True)
            print("wrote", path)
    # Cruelty's Monster art and clean ball, from Lovity's
    recolor_red(Image.open(f"{SRC}/lovity_p4.png")).save(f"{PETS}/phases/cruelty_p4.png", optimize=True)
    recolor_red(Image.open(f"{SRC}/lovity_clean.png")).save(f"{PETS}/clean/cruelty.png", optimize=True)
    print("wrote cruelty_p4 + clean/cruelty")


if __name__ == "__main__":
    main()
