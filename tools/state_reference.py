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
# v2.7-v2.9: end-game upgrades, the Shop, potions, selling and bulk rolls
from core.shop import DICE, POTION_TYPES
s.coins = 1e30
s.rebirths = max(s.rebirths, 12)
for k in UPGRADE_DEFS:
    got = s.buy_upgrade_bulk(k, "max")
    if got:
        log.append("buy %s %d %r" % (k, got, s.coins))
now = 1800000000.0 + seed * 600
for d in DICE:
    log.append("dice %s %s" % (d["key"], s.buy_dice(d["key"], now)))
for p in POTION_TYPES:
    for lvl in range(1, 6):
        log.append("potion %s %d %s %r" % (p["key"], lvl, s.buy_potion(p["key"], lvl, now), s.coins))
s.potions["luck_1"] = s.potions.get("luck_1", 0) + 11
log.append("combine %s %s" % (s.combine_potions("luck", 1), s.combine_potions("luck", 1)))
log.append("use %s %s %s" % (s.use_potion("luck", 2), s.use_potion("luck", 1), s.use_potion("money", 1)))
s.equip_dice("iron")
log.append("x %r %r %r %r" % (s.flat_luck(), s.money_multiplier(), s.mutation_chances(), s.luck_multiplier(40)))
log.append("p %s" % " ".join(repr(x) for x in s.pet_probs()[-12:]))
for i in range(400):
    r = s.roll()
    log.append("%d %s %d %d" % (r[0], r[1], r[2], r[3]))
log.append("bulk %s" % (s.roll_bulk(10 ** 7),))
s.trait_charges += 2000000
res = s.roll_traits_bulk(10 ** 6)
log.append("tb %s %d" % (" ".join("%d:%d" % kv for kv in sorted(res.items())), s.trait_charges))
res = s.roll_traits_bulk(300)
log.append("tb %s %d" % (" ".join("%d:%d" % kv for kv in sorted(res.items())), s.trait_charges))
log.append("sell %s %s" % (s.sell_pets(0, "normal", 5), s.sell_pets(1, "golden", 10 ** 9)))
s.tick_potions(700)
s.coins = 1e20
s.do_rebirth()
log.append("auto %d %r" % (s.auto_upgrade_step(), s.coins))
for m in s.check_milestones():
    log.append("ms %s %d" % (m[0], m[1]))
print("\n".join(log))
d = s.to_dict()
print(json.dumps(d))
s2 = GameState(); s2.slot = 1; s2.load_dict(json.loads(json.dumps(d)))
print(json.dumps(s2.to_dict()) == json.dumps(d))
