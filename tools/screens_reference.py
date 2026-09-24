"""Python counterpart of `lucky-verities --shots DIR`: renders the same screens headless.

Run with SDL_VIDEODRIVER=dummy SDL_AUDIODRIVER=dummy and the same LUCKY_VERITIES_SAVE_DIR as the
Rust run, from anywhere:  python3 tools/screens_reference.py /path/to/lucky-sahurs/python OUT_DIR
"""

import os
import sys
import time

GAME_DIR, OUT = sys.argv[1], sys.argv[2]
sys.path.insert(0, GAME_DIR)
os.chdir(GAME_DIR)

FAKE_NOW = 1_790_000_000.0
time.time = lambda: FAKE_NOW

import pygame  # noqa: E402
import main  # noqa: E402

g = main.Game()
g.screen = pygame.Surface((1422, 800))
g.gpu_scaled = False
g.recompute_layout()
g.mouse_canvas = lambda: (-100, -100)
pygame.time.get_ticks = lambda: 0


def shot(name):
    g.draw()
    pygame.image.save(g.canvas, os.path.join(OUT, name + ".png"))


shot("title")
for name, fn in (("options", g.toggle_options), ("credits", g.toggle_credits),
                 ("updatelog", g.toggle_update_log), ("leaderboard", g.toggle_leaderboard),
                 ("feedback", g.toggle_feedback)):
    fn()
    shot(name)
    g.close_overlays()
    g.leaderboard_open = False
g.open_account()
shot("account")
g.close_account()
# the "New version available" screen, in each phase
g.upd_info = {"tag": "v2.5.0", "name": "x", "url": "", "size": 1, "digest": "", "page": ""}
for phase, prog, err in (("available", 0.0, "other"), ("downloading", 0.42, "other"), ("installing", 1.0, "other"),
                         ("error", 0.0, "net"), ("error", 0.0, "perm"), ("error", 0.0, "other")):
    g.upd_phase, g.upd_progress, g.upd_error, g.upd_error_detail = phase, prog, err, "boom"
    shot("upd_%s_%s" % (phase, err) if phase == "error" else "upd_" + phase)
g.upd_phase = "idle"

# ---- in game: a deterministic save built by rolling
import random  # noqa: E402
from core.pets import RARITIES, TIER_TRANSCENDENT  # noqa: E402
from core.upgrades import UPGRADE_DEFS  # noqa: E402

g.start_slot(1)
random.seed(7)
st = g.state
for _ in range(4000):
    st.roll()
st.coins = 5e8
for key in UPGRADE_DEFS:
    st.buy_upgrade_bulk(key, "max")
for _ in range(3000):
    st.roll()
while st.trait_charges > 0:
    st.roll_trait()
if st.last_trait_roll is not None:
    st.equip_trait(st.last_trait_roll)
st.equip_best()
g.check_milestones()
g.toast_timer = 0.0
st.coins = 1e9
shot("g_main")
for name, content, sub in (("g_index", "index", None), ("g_index_golden", "index", "golden"),
                           ("g_tree", "tree", None), ("g_tree_luck", "tree", "luck"),
                           ("g_milestones", "milestones", None), ("g_milestones_rolls", "milestones", "rolls"),
                           ("g_daily", "daily", None)):
    g.close_overlays()
    g.right_panel.open(content)
    if content == "index" and sub:
        g.index_tab = sub
    elif content == "tree":
        g.tree_selected_category = sub
    elif content == "milestones":
        g.milestones_selected_category = sub
    g.right_panel.update(5.0)
    shot(name)
g.right_panel.close()
g.right_panel.update(5.0)
g.left_panel.open("bag")
g.left_panel.update(5.0)
shot("g_bag")
g.toggle_bag_view()
shot("g_bag_inventory")
g.left_panel.close()
g.left_panel.update(5.0)
g.toggle_traits()
shot("g_traits")
g.toggle_rebirth()
shot("g_rebirth")
g.close_overlays()
g.toggle_stats()
shot("g_stats")
g.close_overlays()
for tab in ("gameplay", "interface", "volume", "sfx", "game"):
    g.open_options()
    g.set_options_tab(tab)
    shot("g_options_" + tab)
g.close_overlays()
g.start_cutscene(next(i for i, r in enumerate(RARITIES) if r["tier"] == TIER_TRANSCENDENT), "golden")
g.cutscene_elapsed = 1.0
shot("g_cutscene")
g.end_cutscene()

# ---- online screens with made-up data (no network: every refresh is pushed into the future)
now = FAKE_NOW
INF = float("inf")
g.back_to_title()
g.open_saves()
shot("o_saves_local")
g.screen_mode = "title"
g.account = {"uid": "u_me", "username": "tommy", "email": "t@x.io", "email_verified": True}


def person(uid, name, pet, mut):
    return {"uid": uid, "username": name, "avatar_pet": pet, "avatar_mut": mut}


g.friends_list = [person("u_a", "alice", 3, "golden"), person("u_b", "bob", None, "normal"),
                  person("u_c", "carol_long_name", 12, "diamond")]
g.friends_incoming = [person("u_d", "dave", 0, "normal")]
g.friends_outgoing = [person("u_e", "eve", None, "normal")]
g.friends_loaded = True
g.friends_next_refresh = INF
g.friends_badge_at = INF
g.friends_profile_pushed = True
g.chat_last["u_a"] = now - 30.0
shot("o_title_logged")
g.open_friends()
shot("o_friends")
g.set_friends_tab("requests")
shot("o_friends_requests")
g.set_friends_tab("add")
g.friends_search.set_text("alice")
g.friends_result = person("u_a", "alice", 3, "golden")
shot("o_friends_add")
g.friend_stats["u_a"] = {"username": "alice", "coins": 123456789.0, "playtime": 98765.0, "rolls": 43210,
                         "rebirths": 7, "updated_at": now - 100.0}
g.friend_view = "u_a"
shot("o_friend_view")
g.friend_view = None
g.avatar_picker = True
shot("o_avatar_picker")
g.avatar_picker = False
g.chat_uid = "u_a"
g.chat_name = "alice"
g.chat_next_poll = INF


def msg(i, frm, text, t):
    return {"id": i, "from_uid": frm, "text": text, "sent_at": t}


g.chat_messages = [
    msg("1", "u_a", "hey! how many rebirths do you have?", now - 7200.0),
    msg("2", "u_me", "seven, going for the transcendent one now", now - 3000.0),
    msg("3", "u_a", "good luck, it took me ages to get a diamond one and a lot of rolls with the auto roller "
                    "running all night long", now - 40.0),
]
g.chat_field.set_text("gl")
g.chat_focus = True
shot("o_chat")
g.close_friends()
g.feedback_next_refresh = INF


def fbk(uid, name, text, t):
    return {"uid": uid, "username": name, "text": text, "updated_at": t}


g.feedback_list = [fbk("u_a", "alice", "More pets please! Also a way to trade them with friends would be really cool.",
                       now - 400.0), fbk("u_b", "bob", "love it", now - 90000.0)]
g.feedback_mine = fbk("u_me", "tommy", "Add a dark mode for the cards.", now - 10.0)
g.feedback_loaded = True
g.open_feedback()
shot("o_feedback")
g.feedback_confirm_delete = True
shot("o_feedback_confirm")
g.start_feedback_edit()
shot("o_feedback_edit")
g.close_feedback()
g.event_current = {"kind": "luck", "mult": 10.0, "ends_at": now + 250.0, "by": "tommy"}
g.event_is_admin = True
g.event_admin_checked = True
g.event_next_poll = INF
g.toggle_event_admin()
shot("o_event_admin")
g.close_event_admin()
g.lb_retry_at = INF
g.lb_data = {
    "period": 0, "fetched_at": now - 200.0,
    "money": [{"username": "alice", "value": 9.9e12}, {"username": "tommy", "value": 1.5e9},
              {"username": "bob", "value": 12345.0}, {"username": "zed", "value": 10.0}],
    "playtime": [{"username": "bob", "value": 360000.0}],
    "rolls": [], "rebirths": [],
}
g.open_leaderboard()
shot("o_leaderboard")
g.set_lb_tab("playtime")
shot("o_leaderboard_playtime")
g.set_lb_tab("rebirths")
shot("o_leaderboard_empty")
g.close_leaderboard()
g.screen_mode = "game"
shot("o_game_event_banner")
g.screen_mode = "title"
g.open_account()
shot("o_account_logged")
g.close_account()
os._exit(0)
