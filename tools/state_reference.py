"""Deterministic GameState simulation (compare with `lucky-verities --statetest SEED N`)."""
import json, os, random, sys
sys.path.insert(0, os.environ.get("LV_PY", "/home/tom/dev/lucky-sahurs"))
os.environ.setdefault("SDL_VIDEODRIVER", "dummy")
from core.game_state import GameState
from core.upgrades import UPGRADE_DEFS

seed, n = int(sys.argv[1]), int(sys.argv[2])
random.seed(seed)
s = GameState()
s.slot = 1
log = []
for i in range(n):
    r = s.roll()
    log.append("%d %s %d %d" % (r[0], r[1], r[2], r[3]))
    s.coins += s.income_per_second() * 3.7 + 11
    s.total_coins_earned += s.income_per_second() * 3.7 + 11
    s.playtime += 1.5
    if i % 37 == 0:
        for k in UPGRADE_DEFS:
            got = s.buy_upgrade_bulk(k, ["max", 1, 10][i % 3])
            if got:
                log.append("buy %s %d %r" % (k, got, s.coins))
    if s.trait_charges and i % 5 == 0:
        t = s.roll_trait()
        log.append("trait %s" % t)
        s.equip_trait(t)
    for m in s.check_milestones():
        log.append("ms %s %d" % (m[0], m[1]))
    if i % 11 == 0:
        s.equip_best()
    if i % 97 == 0 and s.do_rebirth():
        log.append("rebirth %d" % s.rebirths)
    if i % 50 == 0:
        log.append("w %r %r %r %r" % (s.luck_multiplier(20), s.money_multiplier(), s.auto_rolls_per_second(), s.mutation_chances()))
for k in range(3):
    log.append("claim %d" % s.claim_daily_mission(k))
print("\n".join(log))
d = s.to_dict()
print(json.dumps(d))
s2 = GameState(); s2.slot = 1; s2.load_dict(json.loads(json.dumps(d)))
print(json.dumps(s2.to_dict()) == json.dumps(d))
