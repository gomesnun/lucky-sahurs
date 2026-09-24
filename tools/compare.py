"""Compare two directories of raw RGBA dumps. Usage: python tools/compare.py <dirA> <dirB>"""
import os, struct, sys
a, b = sys.argv[1], sys.argv[2]
for name in sorted(os.listdir(a)):
    if not name.endswith(".bin"):
        continue
    pa, pb = open(os.path.join(a, name), "rb").read(), open(os.path.join(b, name), "rb").read()
    wa, ha = struct.unpack("<II", pa[:8]); wb, hb = struct.unpack("<II", pb[:8])
    if (wa, ha) != (wb, hb):
        print("%s: size differs %s vs %s" % (name, (wa, ha), (wb, hb))); continue
    diff = 0; maxd = 0; first = None
    for i in range(8, len(pa), 4):
        d = max(abs(pa[i + k] - pb[i + k]) for k in range(4))
        if d:
            diff += 1; maxd = max(maxd, d)
            if first is None:
                px = (i - 8) // 4; first = (px % wa, px // wa, tuple(pa[i:i+4]), tuple(pb[i:i+4]))
    print("%s: %dx%d  differing px=%d  max=%d  first=%s" % (name, wa, ha, diff, maxd, first))
