# Code Review Findings

Full review of the game codebase (`main.py`, `core/`, `online/`, `ui/`, `storage.py`, `config.py`, `theme.py`, `i18n.py`/`i18n_pt.py` — everything outside `.venv`). Three bugs found. Everything else, especially the money and cloud-sync logic, held up well under scrutiny (race conditions, conflict resolution, confirm-dialog timers, click-dispatch z-order, idempotency of claim/reward paths — all correct).

---

## 1. Overlay guard drift (`main.py`)

**What's wrong:** `handle_events()` has two places that check "is any full-page overlay currently open" before deciding whether a keyboard shortcut or mouse-wheel scroll should be handled normally. Both checks are hand-maintained lists of boolean flags, and both are missing three flags: `leaderboard_open`, `credits_open`, `update_log_open`. The equivalent list in `close_overlay_on_outside_click()` (used when you click outside a panel) *does* have all of them — so this isn't a case of the flags not existing, it's that two of the three "which overlay is open" checks fell out of sync with the third.

**Effect:** While the Leaderboard, Credits, or Update Log screen is open, pressing a shortcut key like U (Upgrades), T (Traits), I (Index), B (Bag), or M (Milestones) silently closes the open overlay and opens the requested panel instead of being ignored. Same issue for scrolling the mouse wheel over those three screens — it can leak through to whatever's underneath.

**Severity:** Cosmetic/UX only. No data loss, no crash — `close_leaderboard()` and friends have no side effects that would be lost by this.

**Fix:** Add `leaderboard_open`, `credits_open`, `update_log_open` to the same boolean list already used in both guards, matching `close_overlay_on_outside_click()`.

---

## 2. Silent save-failure swallow during auto-update (`ui/update_panel.py`)

**What's wrong:** `upd_finish()` runs right after the game downloads and prepares an update, just before relaunching into the new version:

```python
def upd_finish(self, plan):
    try:
        self.flush_cloud_blocking()
        self.state.save()
        save_settings(self.settings)
    except Exception:
        pass
    try:
        updater.launch(plan)
    except Exception as e:
        self.upd_fail("other", str(e))
        return
    pygame.quit()
    sys.exit()
```

The save sequence is wrapped in a bare `except Exception: pass`. Compare this to the *normal* quit path, `main.py:quit_game()`:

```python
def quit_game(self):
    self.flush_cloud_blocking()
    self.state.save()
    save_settings(self.settings)
    pygame.quit()
    sys.exit()
```

No try/except at all — if a save fails here, the exception propagates up to the top-level handler in `main.py`, which writes it to `crash_log.txt` before the process exits. That's the only diagnostic trail this game has (it ships without a console window).

**Effect:** `flush_cloud_blocking()` already catches its own network errors internally (`OnlineError`), so it won't raise those. But `state.save()` is a direct local disk write, and can genuinely raise on a full disk, a permissions problem, or an antivirus lock — all plausible right after a multi-megabyte update download landed on the same disk. If that happens during `upd_finish()`, the exception is silently discarded, the game relaunches into the new version anyway, and the player's progress since their last successful save is gone with *no* crash log entry to show it ever happened. The normal-quit path would at least leave a trace.

**Severity:** Low likelihood (needs an IO error at that exact moment), but worse-than-normal outcome when it hits: silent data loss with zero diagnostics, during the one operation (auto-update) where the game can't just be relaunched to try again.

**Fix:** Drop the `except Exception: pass` around the save sequence (or narrow it to log to the same crash-log mechanism `main.py` uses) so a save failure here behaves at least as safely as it does on normal quit.

---

## 3. Stale rarity-tier text after a language switch (`ui/cards.py`)

**What's wrong:** `render_pet_card()` caches its rendered card surfaces in a module-level dict, keyed like this:

```python
key = (rarity["pet"], mutation, w, h, tuple(tuple(tuple(c) for c in p) for p in plates), footer_h, locked)
surf = _CARD_CACHE.get(key)
if surf is not None:
    return surf
...
pill = _pill(self.font_at(pill_size, heavy=True).render(tr(rarity["name"]), True, accent), edge=accent)
```

The rarity pill's text comes from `tr(rarity["name"])` — e.g. "Legendary" in English, "Lendário" in Português — but `rarity["name"]`/`rarity["key"]` is **not** part of the cache key. Nothing else invalidates this cache on a language switch either: `set_language()` (`ui/options_panel.py`) doesn't call `clear_card_cache()`. That function is only wired up to the dark/light theme switch (`main.py:302-304`, alongside `clear_font_caches()`/`clear_drawing_caches()` — the comment there is explicitly about outline color, not language).

The sibling function right below it, `render_trait_card()`, does this correctly:

```python
key = ("trait", trait_index, w, h, owned, equipped, chance_text, tr("Chance"), tr("EQUIPPED"))
```

Here the translated strings themselves are baked into the key, so a language switch naturally produces a different key and forces a fresh render. This makes the omission in `render_pet_card()` look like an oversight rather than an intentional difference — the fix was already invented once, just not applied to both functions.

**Effect:** Play in English, view pet cards (Pet Index, Bag) so they get cached, then switch language to Português via Options. Revisiting those same cards, the rarity pill keeps showing the English tier name ("Common", "Rare", "Legendary"...) instead of the Portuguese one, until that specific `(pet, mutation, size, plates, footer_h, locked)` combination happens to fall out of the cache — which only happens when the 320-entry cache limit is hit and gets wholesale-cleared, or the game restarts. In practice, for anyone who's already looked at most of their collection, switching language leaves most rarity pills stuck in the old language for the rest of the session.

**Severity:** Cosmetic/localization only, but reliably reproducible.

**Fix:** Add `rarity["key"]` (or `tr(rarity["name"])` directly) to the cache key tuple in `render_pet_card()`, mirroring what `render_trait_card()` already does.
