"""Ecrã de conta (registo / login)."""

import time
import pygame

from config import SAVE_SLOTS
from i18n import tr
from online.firebase import MIN_PASSWORD, USERNAME_RE, online_error_text
from theme import (
    ACCENT, ACCENT_HOVER, BAD, BLACK,
    GREY, GREY_DIM, OUTLINE, PANEL,
    PANEL_LIGHT, PANEL_LIGHTER, WHITE,
)
from ui.drawing import draw_panel
from ui.fonts import fit_text, wrap_text
from ui.widgets import clipboard_text


class AccountPanelMixin:
    """Ecrã de conta (registo / login)."""

    # ================================================================ ONLINE: conta (ecrã)
    def start_text_input(self):
        try:
            pygame.key.start_text_input()
            pygame.key.set_repeat(400, 35)
        except (pygame.error, AttributeError):
            pass

    def stop_text_input(self):
        try:
            pygame.key.stop_text_input()
            pygame.key.set_repeat()
        except (pygame.error, AttributeError):
            pass

    def open_account(self):
        self.account_return = self.screen_mode if self.screen_mode in ("title", "saves") else "title"
        self.close_overlays()
        self.acc_msg = None
        self.acc_busy = False
        self.acc_show_pw = False
        self.acc_focus = None if self.account else "username"
        self.screen_mode = "account"
        self.start_text_input()

    def close_account(self):
        self.stop_text_input()
        self.acc_focus = None
        self.screen_mode = self.account_return
        if self.screen_mode == "saves":
            self.open_saves()

    def set_acc_tab(self, tab):
        self.acc_tab = tab
        self.acc_msg = None
        self.acc_focus = "username"

    def set_focus(self, key):
        self.acc_focus = key

    def acc_visible_fields(self):
        return ["username", "password", "confirm"] if self.acc_tab == "register" else ["username", "password"]

    def handle_account_key(self, event):
        """Teclas do ecrã de conta. Devolve True se a tecla foi usada."""
        k = event.key
        keys = self.acc_visible_fields()
        if k == pygame.K_TAB and not self.account:
            step = -1 if (event.mod & pygame.KMOD_SHIFT) else 1
            i = (keys.index(self.acc_focus) + step) % len(keys) if self.acc_focus in keys else 0
            self.acc_focus = keys[i]
            return True
        if k in (pygame.K_RETURN, pygame.K_KP_ENTER):
            if not self.account:
                self.submit_account()
            return True
        f = self.acc_fields.get(self.acc_focus) if self.acc_focus else None
        if f is None:
            return False
        if k == pygame.K_BACKSPACE:
            f.backspace()
            return True
        if k == pygame.K_v and (event.mod & (pygame.KMOD_CTRL | pygame.KMOD_META)):
            f.add(clipboard_text())
            return True
        return False

    def submit_account(self):
        if self.acc_busy or not self.client or self.account:
            return
        register = self.acc_tab == "register"
        name = self.acc_fields["username"].text.strip().lower()
        pw = self.acc_fields["password"].text
        if not USERNAME_RE.match(name):
            self.acc_msg = (tr("Username: 3 to 16 characters, only letters, numbers and _"), BAD)
            return
        if len(pw) < MIN_PASSWORD:
            self.acc_msg = (tr("Password must have at least %d characters.", MIN_PASSWORD), BAD)
            return
        if register and pw != self.acc_fields["confirm"].text:
            self.acc_msg = (tr("The two passwords don't match."), BAD)
            return
        self.acc_busy = True
        self.acc_msg = (tr("Please wait..."), GREY)
        client = self.client
        job = (lambda: client.sign_up(name, pw)) if register else (lambda: client.sign_in(name, pw))
        self.worker.run(job, lambda _r: self.on_auth_ok(register), self.on_auth_err)

    def on_auth_ok(self, registered):
        self.acc_busy = False
        self.account = {"uid": self.client.uid, "username": self.client.username}
        self.pub_last_time = 0.0
        self.save_session_file()
        for f in self.acc_fields.values():
            f.text = ""
        self.acc_msg = None
        self.acc_show_pw = False
        self.stop_text_input()
        self.acc_focus = None
        self.refresh_slot_info()
        self.open_saves()
        has_local = any(self.slot_info.get(i) for i in range(1, SAVE_SLOTS + 1))
        if registered and has_local:
            self.show_toast(tr("Account created. Your local saves weren't touched - import them below."))
        elif registered:
            self.show_toast(tr("Account created. Welcome, %s!", self.account["username"]))
        else:
            self.show_toast(tr("Welcome back, %s!", self.account["username"]))

    def on_auth_err(self, e):
        self.acc_busy = False
        self.acc_msg = (online_error_text(e), BAD)

    def draw_field(self, rect, key):
        f = self.acc_fields[key]
        focused = self.acc_focus == key
        pygame.draw.rect(self.canvas, (22, 22, 25), rect, border_radius=10)
        pygame.draw.rect(self.canvas, ACCENT if focused else OUTLINE, rect, width=3, border_radius=10)
        shown = ("*" * len(f.text)) if (f.kind == "password" and not self.acc_show_pw) else f.text
        max_w = rect.width - 28
        while len(shown) > 1 and self.font_med.size(shown)[0] > max_w:
            shown = shown[1:]                       # mostra o fim do texto (onde estás a escrever)
        if shown:
            txt = self.font_med.render(shown, True, WHITE)
        else:
            placeholder = {"username": tr("your username"), "password": tr("your password"),
                           "confirm": tr("repeat the password")}[key]
            txt = self.font_med.render(placeholder, True, GREY_DIM)
        pos = (rect.x + 14, rect.centery - txt.get_height() // 2)
        self.canvas.blit(txt, pos)
        if focused and int(time.time() * 2) % 2 == 0:
            cx = pos[0] + (txt.get_width() + 2 if shown else 0)
            pygame.draw.line(self.canvas, WHITE, (cx, rect.y + 10), (cx, rect.bottom - 10), 2)
        self.register_button(rect, lambda k=key: self.set_focus(k), None)

    def draw_account(self, mouse_pos):
        cx = self.vw // 2
        head = self.font_huge.render(tr("Account"), True, WHITE)
        self.canvas.blit(head, head.get_rect(center=(cx, 70)))
        card_w = 480
        x0 = cx - card_w // 2

        def back_button(y):
            self.button(pygame.Rect(cx - 110, y, 220, 48), tr("Back"), self.font_med, mouse_pos,
                        PANEL_LIGHT, PANEL_LIGHTER, WHITE, callback=self.close_account, radius=12)

        if not self.client:
            rect = pygame.Rect(x0, 130, card_w, 200)
            draw_panel(self.canvas, rect, PANEL, radius=16)
            y = rect.y + 30
            for line in wrap_text(tr("Online features aren't set up yet. Add your Firebase keys to the script "
                                     "(or to firebase_config.json) - see FIREBASE_SETUP.md."),
                                  self.font_small, card_w - 56):
                self.canvas.blit(self.font_small.render(line, True, GREY), (rect.x + 28, y))
                y += 24
            back_button(rect.bottom + 24)
            return

        if self.account:
            rect = pygame.Rect(x0, 130, card_w, 300)
            draw_panel(self.canvas, rect, PANEL, radius=16)
            lbl = self.font_small.render(tr("Logged in as"), True, GREY)
            self.canvas.blit(lbl, lbl.get_rect(center=(cx, rect.y + 44)))
            name = self.font_huge.render(fit_text(self.font_huge, self.account["username"], card_w - 60),
                                         True, ACCENT)
            self.canvas.blit(name, name.get_rect(center=(cx, rect.y + 92)))
            y = rect.y + 142
            for line in wrap_text(tr("Your saves are stored in your account and sync to the cloud while you play. "
                                     "Local saves on this PC are never deleted."), self.font_small, card_w - 60):
                t = self.font_small.render(line, True, GREY)
                self.canvas.blit(t, t.get_rect(center=(cx, y)))
                y += 22
            self.button(pygame.Rect(cx - 120, rect.bottom - 76, 240, 48), tr("Log out"), self.font_med, mouse_pos,
                        (150, 60, 60), BAD, WHITE, callback=self.log_out_clicked, radius=12)
            back_button(rect.bottom + 24)
            return

        register = self.acc_tab == "register"
        keys = self.acc_visible_fields()
        card_h = 84 + 86 * len(keys) + 44 + 50 + 16 + 48 + 22
        rect = pygame.Rect(x0, 118, card_w, card_h)
        draw_panel(self.canvas, rect, PANEL, radius=16)

        tab_w = (card_w - 48 - 12) // 2
        for i, (key, label) in enumerate((("login", tr("Log in")), ("register", tr("Register")))):
            trect = pygame.Rect(rect.x + 24 + i * (tab_w + 12), rect.y + 22, tab_w, 42)
            active = self.acc_tab == key
            self.button(trect, label, self.font_med, mouse_pos,
                        ACCENT if active else PANEL_LIGHT, ACCENT_HOVER if active else PANEL_LIGHTER,
                        BLACK if active else WHITE, callback=lambda k=key: self.set_acc_tab(k), radius=10)

        y = rect.y + 84
        labels = {"username": tr("Username"), "password": tr("Password"), "confirm": tr("Repeat password")}
        for key in keys:
            self.canvas.blit(self.font_small_b.render(labels[key], True, GREY), (rect.x + 26, y))
            self.draw_field(pygame.Rect(rect.x + 24, y + 24, card_w - 48, 46), key)
            if key == "password":
                toggle = pygame.Rect(rect.right - 24 - 84, y - 3, 84, 26)

                def toggle_pw():
                    self.acc_show_pw = not self.acc_show_pw
                self.button(toggle, tr("Hide") if self.acc_show_pw else tr("Show"), self.font_tiny, mouse_pos,
                            PANEL_LIGHT, PANEL_LIGHTER, WHITE, callback=toggle_pw, radius=8, sfx=None)
            y += 86

        if self.acc_msg:
            text, color = self.acc_msg
            my = y - 4
            for line in wrap_text(text, self.font_small, card_w - 48)[:2]:
                t = self.font_small.render(line, True, color)
                self.canvas.blit(t, t.get_rect(center=(cx, my + 10)))
                my += 20
        submit = pygame.Rect(cx - 150, y + 44 - 4, 300, 50)
        if self.acc_busy:
            label = tr("Creating account...") if register else tr("Logging in...")
        else:
            label = tr("Create account") if register else tr("Log in")
        self.button(submit, label, self.font_med, mouse_pos, ACCENT, ACCENT_HOVER, BLACK,
                    callback=self.submit_account, radius=12, enabled=not self.acc_busy)

        note = tr("No email needed. There is no password recovery, so remember your password. "
                  "Your local saves are never touched.") if register else \
               tr("Log in to use your cloud saves and join the leaderboard.")
        ny = submit.bottom + 14
        for line in wrap_text(note, self.font_tiny, card_w - 56):
            t = self.font_tiny.render(line, True, GREY)
            self.canvas.blit(t, t.get_rect(center=(cx, ny + 8)))
            ny += 16
        back_button(rect.bottom + 24)

    def draw_account_chip(self, mouse_pos):
        """Canto superior direito do menu / dos saves: 'Log in' ou o nome da conta."""
        w = 220
        rect = pygame.Rect(self.vw - w - 24, 20, w, 44)
        if self.account:
            label = fit_text(self.font_med, self.account["username"], w - 24)
            self.button(rect, label, self.font_med, mouse_pos, PANEL_LIGHT, PANEL_LIGHTER, ACCENT,
                        callback=self.open_account, radius=12)
        else:
            self.button(rect, tr("Log in"), self.font_med, mouse_pos, ACCENT, ACCENT_HOVER, BLACK,
                        callback=self.open_account, radius=12)
