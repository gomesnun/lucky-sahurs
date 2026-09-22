"""Ecrã de conta (registo / login / recuperação / verificação de email)."""

import random
import re
import time
import pygame

from config import SAVE_SLOTS
from i18n import tr
from online.firebase import (
    MIN_PASSWORD, OnlineError, USERNAME_RE, online_error_text,
    server_clock_synced, server_now, username_to_email,
)
from online.mailer import send_verification_code
from theme import (
    ACCENT, ACCENT_HOVER, BAD, BLACK,
    GOOD, GREY, GREY_DIM, OUTLINE, PANEL,
    PANEL_LIGHT, PANEL_LIGHTER, WHITE,
)
from ui.drawing import draw_panel
from ui.fonts import fit_text, wrap_text
from ui.widgets import clipboard_text

EMAIL_RE = re.compile(r"^[^@\s]+@[^@\s]+\.[^@\s]+$")
CODE_RESEND_COOLDOWN = 20.0        # segundos entre "reenviar código"
CODE_EXPIRY = 600                  # 10 minutos (tem de bater certo com a mensagem mostrada ao jogador)


class AccountPanelMixin:
    """Ecrã de conta (registo / login / recuperação / verificação de email)."""

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
        if not (self.account and self.acc_stage in ("verify", "link_email", "reauth")):
            self.acc_stage = "form"
            if not self.account:
                self.acc_tab = "login"
        if not self.account and self.acc_stage == "form":
            self.acc_focus = self.acc_visible_fields()[0]
        self.screen_mode = "account"
        self.start_text_input()

    def close_account(self):
        self.stop_text_input()
        self.acc_focus = None
        if self.acc_stage in ("verify", "reauth", "link_email", "forgot", "forgot_sent"):
            self.acc_stage = "form"        # "Back" também serve para saltar a verificação, se quiseres
        self.screen_mode = self.account_return
        if self.screen_mode == "saves":
            self.open_saves()

    def set_acc_tab(self, tab):
        self.acc_tab = tab
        self.acc_msg = None
        self.acc_focus = self.acc_visible_fields()[0]

    def set_focus(self, key):
        self.acc_focus = key

    def acc_visible_fields(self):
        if self.acc_stage == "verify":
            return ["code"]
        if self.acc_stage == "reauth":
            return ["password"]
        if self.acc_stage == "link_email":
            return ["email"]
        if self.acc_stage == "forgot":
            return ["username"]
        if self.acc_tab == "register":
            return ["username", "email", "password", "confirm"]
        if self.acc_tab == "recover":
            return ["username", "password"]      # contas antigas, sem email associado
        return ["email", "password"]              # "login": contas normais entram pelo email

    def submit_current_stage(self):
        if self.acc_stage == "verify":
            self.submit_code()
        elif self.acc_stage == "reauth":
            self.submit_reauth()
        elif self.acc_stage == "link_email":
            self.submit_link_email()
        elif self.acc_stage == "forgot":
            self.submit_forgot()
        elif not self.account:
            self.submit_account()

    def handle_account_key(self, event):
        """Teclas do ecrã de conta. Devolve True se a tecla foi usada."""
        k = event.key
        keys = self.acc_visible_fields()
        if k == pygame.K_TAB:
            step = -1 if (event.mod & pygame.KMOD_SHIFT) else 1
            i = (keys.index(self.acc_focus) + step) % len(keys) if self.acc_focus in keys else 0
            self.acc_focus = keys[i]
            return True
        if k in (pygame.K_RETURN, pygame.K_KP_ENTER):
            self.submit_current_stage()
            return True
        f = self.acc_fields.get(self.acc_focus) if self.acc_focus else None
        if f is None:
            return False
        if k == pygame.K_BACKSPACE:
            f.backspace()
            return True
        if k == pygame.K_DELETE:
            f.delete()
            return True
        if k == pygame.K_LEFT:
            f.move(-1)
            return True
        if k == pygame.K_RIGHT:
            f.move(1)
            return True
        if k in (pygame.K_HOME, pygame.K_UP):
            f.home()
            return True
        if k in (pygame.K_END, pygame.K_DOWN):
            f.end()
            return True
        if k == pygame.K_v and (event.mod & (pygame.KMOD_CTRL | pygame.KMOD_META)):
            f.add(clipboard_text())
            return True
        return False

    # ---------------------------------------------------------------- registo / login
    def submit_account(self):
        if self.acc_busy or not self.client or self.account:
            return
        register = self.acc_tab == "register"
        via_recover = self.acc_tab == "recover"
        pw = self.acc_fields["password"].text.strip()
        if len(pw) < MIN_PASSWORD:
            self.acc_msg = (tr("Password must have at least %d characters.", MIN_PASSWORD), BAD)
            return
        client = self.client

        if register:
            name = self.acc_fields["username"].text.strip().lower()
            if not USERNAME_RE.match(name):
                self.acc_msg = (tr("Username: 3 to 16 characters, only letters, numbers and _"), BAD)
                return
            email = self.acc_fields["email"].text.strip()
            if not EMAIL_RE.match(email):
                self.acc_msg = (tr("Enter a valid email address."), BAD)
                return
            if pw != self.acc_fields["confirm"].text.strip():
                self.acc_msg = (tr("The two passwords don't match."), BAD)
                return
            job = lambda: client.sign_up(name, pw)
        elif via_recover:
            # contas antigas, sem email associado: continuam a entrar por username
            name = self.acc_fields["username"].text.strip().lower()
            if not USERNAME_RE.match(name):
                self.acc_msg = (tr("Username: 3 to 16 characters, only letters, numbers and _"), BAD)
                return
            email = ""
            job = lambda: client.sign_in(name, pw)
        else:
            # login normal: pelo email já confirmado (por baixo continua a resolver para o username)
            email = self.acc_fields["email"].text.strip()
            if not EMAIL_RE.match(email):
                self.acc_msg = (tr("Enter a valid email address."), BAD)
                return
            job = lambda: client.sign_in_by_email(email, pw)

        self.acc_busy = True
        self.acc_msg = (tr("Please wait..."), GREY)
        self.worker.run(job, lambda _r: self.on_auth_ok(register, email, via_recover), self.on_auth_err)

    def on_auth_ok(self, registered, email="", via_recover=False):
        self.acc_busy = False
        self.account = {"uid": self.client.uid, "username": self.client.username,
                        "email": None, "email_verified": False}
        self.pub_last_time = 0.0
        self.save_session_file()
        self.acc_msg = None
        self.acc_show_pw = False
        for f in self.acc_fields.values():
            f.set_text()
        if registered:
            # conta nova: já tem email (campo obrigatório no registo) -> manda logo o código
            self.begin_email_verification(email, welcome_new=True)
        else:
            # login: pode ser uma conta já verificada, uma a meio da verificação, ou uma antiga sem email
            self.acc_busy = True
            self.acc_msg = (tr("Checking your account..."), GREY)
            self.worker.run(self.client.get_profile,
                            lambda p: self.on_login_profile(p, via_recover), self.on_login_profile_err)

    def on_auth_err(self, e):
        self.acc_busy = False
        msg = (e.message or "").upper() if isinstance(e, OnlineError) else ""
        if self.acc_tab == "login" and any(k in msg for k in
                                           ("EMAIL_NOT_FOUND", "INVALID_PASSWORD", "INVALID_LOGIN_CREDENTIALS")):
            self.acc_msg = (tr("Wrong email or password."), BAD)
        else:
            self.acc_msg = (online_error_text(e) if isinstance(e, OnlineError) else str(e), BAD)

    def on_login_profile(self, profile, via_recover=False):
        self.acc_busy = False
        if via_recover and profile and profile.get("email_verified"):
            # o separador "Old account" é só para contas que nunca chegaram a migrar; esta já concluiu
            # a migração (email confirmado) - não é uma "conta antiga", tem de entrar pelo "Log in".
            # (uma confirmação a meio/pendente ainda pode - e tem de poder - continuar por aqui)
            self.log_out()
            self.acc_tab = "login"
            self.acc_msg = (tr("This account already has an email. Use \"Log in\" instead."), BAD)
            return
        if profile and profile.get("email_verified"):
            self.account["email_verified"] = True
            self.account["email"] = profile.get("email")
            self.finish_login_flow()
        elif profile and profile.get("email"):
            # já tinha começado a associar um email mas nunca confirmou o código
            self.account["email"] = profile.get("email")
            self.acc_pending_email = profile.get("email")
            self.enter_verify_stage()
            self.show_toast(tr("You still need to confirm your email."))
        else:
            self.enter_link_email_stage()
            self.show_toast(tr("Add an email to your account so you can recover it later."))

    def on_login_profile_err(self, _e):
        # sem rede para verificar: não bloqueia o jogador por causa disto - os saves locais/cache
        # continuam a funcionar. Na próxima vez que houver rede, volta a perguntar.
        self.acc_busy = False
        self.finish_login_flow()

    def finish_login_flow(self):
        self.stop_text_input()
        self.acc_focus = None
        self.acc_stage = "form"
        self.refresh_slot_info()
        self.open_saves()
        self.show_toast(tr("Welcome back, %s!", self.account["username"]))

    def require_email_link(self):
        """Chamado quando descobrimos (ex.: ao abrir o jogo com sessão guardada) que a conta ainda
        não tem um email verificado."""
        if self.screen_mode == "account" and self.acc_stage in ("verify", "link_email"):
            return
        self.account_return = "title"
        self.close_overlays()
        self.acc_msg = None
        self.acc_busy = False
        self.screen_mode = "account"
        self.enter_verify_stage() if self.acc_pending_email else self.enter_link_email_stage()
        self.show_toast(tr("Add an email to your account so you can recover it later."))

    # ---------------------------------------------------------------- verificação por código
    def enter_link_email_stage(self):
        self.acc_stage = "link_email"
        self.acc_fields["email"].set_text()
        self.acc_focus = "email"
        self.acc_msg = None
        self.start_text_input()

    def enter_verify_stage(self):
        self.acc_stage = "verify"
        self.acc_fields["code"].set_text()
        self.acc_focus = "code"
        self.acc_msg = None
        self.start_text_input()

    def submit_link_email(self):
        if self.acc_busy:
            return
        email = self.acc_fields["email"].text.strip()
        if not EMAIL_RE.match(email):
            self.acc_msg = (tr("Enter a valid email address."), BAD)
            return
        self.begin_email_verification(email)

    def begin_email_verification(self, email, welcome_new=False):
        """Gera um código de 6 dígitos, guarda-o na conta e manda-o por email."""
        if self.acc_busy:
            return
        self.acc_busy = True
        self.acc_msg = (tr("Sending the code..."), GREY)
        code = "%06d" % random.randint(0, 999999)
        client = self.client

        def job():
            client.set_pending_code(email, code)
            send_verification_code(email, code)
        self.worker.run(job, lambda _r: self.on_code_sent(email, welcome_new), self.on_code_sent_err)

    def on_code_sent(self, email, welcome_new):
        self.acc_busy = False
        self.acc_pending_email = email
        self.acc_code_sent_at = time.time()
        self.enter_verify_stage()
        if welcome_new:
            self.show_toast(tr("Account created! We sent a code to %s.", email))
        else:
            self.show_toast(tr("Code sent to %s.", email))

    def on_code_sent_err(self, e):
        self.acc_busy = False
        self.acc_msg = (online_error_text(e), BAD)

    def resend_code(self):
        if self.acc_busy:
            return
        if time.time() - self.acc_code_sent_at < CODE_RESEND_COOLDOWN:
            self.acc_msg = (tr("Please wait a moment before resending."), GREY)
            return
        if self.acc_pending_email:
            self.begin_email_verification(self.acc_pending_email)

    def submit_code(self):
        if self.acc_busy:
            return
        entered = self.acc_fields["code"].text.strip()
        if len(entered) != 6:
            self.acc_msg = (tr("Enter the 6-digit code."), BAD)
            return
        self.acc_busy = True
        self.acc_msg = (tr("Checking..."), GREY)
        client = self.client
        username = self.account["username"]

        def job():
            profile = client.get_profile()
            if not profile or not profile.get("pending_code"):
                raise OnlineError("bad_request", "NO_PENDING_CODE")
            if profile["pending_code"] != entered:
                raise OnlineError("bad_request", "WRONG_CODE")
            created = profile.get("code_created")
            if created is not None and server_clock_synced() and server_now() - created > CODE_EXPIRY:
                raise OnlineError("bad_request", "CODE_EXPIRED")
            email = profile["email"]
            client.confirm_email()
            client.publish_username_email(username, email)      # tem de ir ANTES de trocar o email de login
            try:
                client.change_login_email(email)
            except OnlineError as e:
                if "CREDENTIAL_TOO_OLD_LOGIN_AGAIN" in (e.message or "").upper():
                    # confirm_email/publish já ficaram feitos; só falta esta troca, que exige um login
                    # "fresco" (o token restaurado do disco é velho demais para a Google aceitar isto).
                    raise OnlineError("reauth_required", email)
                raise
            return email
        self.worker.run(job, self.on_verify_ok, self.on_verify_err)

    def on_verify_ok(self, email):
        self.acc_busy = False
        self.account["email_verified"] = True
        self.account["email"] = email
        self.acc_pending_email = None
        self.acc_reauth_email = None
        self.save_session_file()             # se passou pelo re-login, grava o refresh_token novo
        self.stop_text_input()
        self.acc_focus = None
        self.acc_stage = "form"
        self.refresh_slot_info()
        self.open_saves()
        self.show_toast(tr("Email confirmed! Your account can now be recovered."))

    def on_verify_err(self, e):
        self.acc_busy = False
        if isinstance(e, OnlineError) and e.code == "reauth_required":
            # e.message aqui é o email (já confirmado) - só falta re-autenticar para o aplicar
            self.enter_reauth_stage(e.message)
            return
        msg = (e.message or "").upper() if isinstance(e, OnlineError) else ""
        if "WRONG_CODE" in msg:
            self.acc_msg = (tr("Wrong code. Check your email and try again."), BAD)
        elif "CODE_EXPIRED" in msg:
            self.acc_msg = (tr("This code expired. Tap Resend to get a new one."), BAD)
        elif "NO_PENDING_CODE" in msg:
            self.acc_msg = (tr("No pending code. Tap Resend to get one."), BAD)
        else:
            self.acc_msg = (online_error_text(e) if isinstance(e, OnlineError) else str(e), BAD)

    # ---------------------------------------------------------------- reautenticar (sessão velha demais)
    def enter_reauth_stage(self, email):
        """O email já ficou confirmado no Firestore; só falta trocar o email de login no Firebase Auth,
        e isso exige um login "fresco" com a password (a sessão restaurada do disco é velha demais)."""
        self.acc_reauth_email = email
        self.acc_stage = "reauth"
        self.acc_fields["password"].set_text()
        self.acc_focus = "password"
        self.acc_msg = (tr("Your session is a bit old. Enter your password to finish confirming your email."), GREY)
        self.start_text_input()

    def submit_reauth(self):
        if self.acc_busy:
            return
        pw = self.acc_fields["password"].text.strip()
        if len(pw) < MIN_PASSWORD:
            self.acc_msg = (tr("Password must have at least %d characters.", MIN_PASSWORD), BAD)
            return
        self.acc_busy = True
        self.acc_msg = (tr("Checking..."), GREY)
        client = self.client
        username = self.account["username"]
        email = self.acc_reauth_email

        def job():
            # email interno falso, não o novo (ver reauth_pending_link) - renova o token com um login
            # a sério -> auth_time fresco
            client.reauth_pending_link(username, pw)
            client.change_login_email(email)
            return email
        self.worker.run(job, self.on_verify_ok, self.on_reauth_err)

    def on_reauth_err(self, e):
        self.acc_busy = False
        self.acc_msg = (online_error_text(e) if isinstance(e, OnlineError) else str(e), BAD)

    # ---------------------------------------------------------------- esqueci-me da password
    def open_forgot(self):
        self.acc_stage = "forgot"
        self.acc_focus = "username"
        self.acc_msg = None

    def close_forgot(self):
        self.acc_stage = "form"
        self.acc_msg = None

    def submit_forgot(self):
        if self.acc_busy:
            return
        username = self.acc_fields["username"].text.strip().lower()
        if not USERNAME_RE.match(username):
            self.acc_msg = (tr("Enter your username."), BAD)
            return
        self.acc_busy = True
        self.acc_msg = (tr("Please wait..."), GREY)
        client = self.client

        def job():
            email = client.resolve_login_email(username)
            if email == username_to_email(username):
                raise OnlineError("bad_request", "NO_EMAIL_LINKED")
            client.send_password_reset(email)
            return email
        self.worker.run(job, self.on_forgot_ok, self.on_forgot_err)

    def on_forgot_ok(self, email):
        self.acc_busy = False
        self.acc_msg = None
        self.acc_forgot_email = email
        self.acc_stage = "forgot_sent"

    def on_forgot_err(self, e):
        self.acc_busy = False
        msg = (e.message or "").upper() if isinstance(e, OnlineError) else ""
        if "NO_EMAIL_LINKED" in msg:
            self.acc_msg = (tr("This account has no confirmed email yet. Log in with your password "
                              "first, then add one."), BAD)
        elif isinstance(e, OnlineError) and e.code == "not_found":
            self.acc_msg = (tr("Username not found."), BAD)
        else:
            self.acc_msg = (online_error_text(e) if isinstance(e, OnlineError) else str(e), BAD)

    # ---------------------------------------------------------------- desenho: peças partilhadas
    def draw_field(self, rect, key):
        f = self.acc_fields[key]
        display = ("*" * len(f.text)) if (f.kind == "password" and not self.acc_show_pw) else f.text
        placeholder = {"username": tr("your username"), "password": tr("your password"),
                       "confirm": tr("repeat the password"), "email": tr("your email"),
                       "code": tr("6-digit code")}[key]
        self.draw_text_field(rect, f, display, placeholder, self.acc_focus == key,
                             lambda k=key: self.set_focus(k))

    def draw_acc_message(self, cx, y, card_w):
        if not self.acc_msg:
            return y
        text, color = self.acc_msg
        for line in wrap_text(text, self.font_small, card_w - 48)[:3]:
            t = self.font_small.render(line, True, color)
            self.canvas.blit(t, t.get_rect(center=(cx, y + 10)))
            y += 20
        return y

    # ---------------------------------------------------------------- desenho: ecrã principal
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

        if self.account and self.acc_stage == "verify":
            self.draw_verify_stage(mouse_pos, cx, x0, card_w, back_button)
            return
        if self.account and self.acc_stage == "reauth":
            self.draw_reauth_stage(mouse_pos, cx, x0, card_w, back_button)
            return
        if self.account and self.acc_stage == "link_email":
            self.draw_link_email_stage(mouse_pos, cx, x0, card_w, back_button)
            return

        if self.account:
            self.draw_logged_in(mouse_pos, cx, x0, card_w, back_button)
            return

        if self.acc_stage == "forgot":
            self.draw_forgot_stage(mouse_pos, cx, x0, card_w, back_button)
            return
        if self.acc_stage == "forgot_sent":
            self.draw_forgot_sent_stage(mouse_pos, cx, x0, card_w, back_button)
            return

        self.draw_login_register(mouse_pos, cx, x0, card_w, back_button)

    # ---------------------------------------------------------------- desenho: já com sessão iniciada
    def draw_logged_in(self, mouse_pos, cx, x0, card_w, back_button):
        verified = self.account.get("email_verified")
        card_h = 300 if verified else 356
        rect = pygame.Rect(x0, 130, card_w, card_h)
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
        if not verified:
            y += 6
            warn = fit_text(self.font_small_b, tr("Your account has no confirmed email yet."), card_w - 60)
            t = self.font_small_b.render(warn, True, BAD)
            self.canvas.blit(t, t.get_rect(center=(cx, y)))
            y += 30
            fix_rect = pygame.Rect(cx - 150, y, 300, 42)

            def go_fix():
                self.enter_verify_stage() if self.acc_pending_email else self.enter_link_email_stage()
            self.button(fix_rect, tr("Add / confirm email"), self.font_small_b, mouse_pos,
                        ACCENT, ACCENT_HOVER, BLACK, callback=go_fix, radius=10)
            y = fix_rect.bottom + 12
        self.button(pygame.Rect(cx - 120, rect.bottom - 76, 240, 48), tr("Log out"), self.font_med, mouse_pos,
                    (150, 60, 60), BAD, WHITE, callback=self.log_out_clicked, radius=12)
        back_button(rect.bottom + 24)

    # ---------------------------------------------------------------- desenho: login / registo / recuperar
    def draw_login_register(self, mouse_pos, cx, x0, card_w, back_button):
        register = self.acc_tab == "register"
        keys = self.acc_visible_fields()
        card_h = 130 + 86 * len(keys) + 44 + 50 + 16 + 40 + 22
        rect = pygame.Rect(x0, 118, card_w, card_h)
        draw_panel(self.canvas, rect, PANEL, radius=16)

        tabs = (("login", tr("Log in")), ("register", tr("Register")), ("recover", tr("Old account")))
        gap = 8
        tab_w = (card_w - 48 - gap * 2) // 3
        for i, (key, label) in enumerate(tabs):
            trect = pygame.Rect(rect.x + 24 + i * (tab_w + gap), rect.y + 22, tab_w, 42)
            active = self.acc_tab == key
            self.button(trect, fit_text(self.font_small_b, label, tab_w - 10), self.font_small_b, mouse_pos,
                        ACCENT if active else PANEL_LIGHT, ACCENT_HOVER if active else PANEL_LIGHTER,
                        BLACK if active else WHITE, callback=lambda k=key: self.set_acc_tab(k), radius=10)

        y = rect.y + 84
        labels = {"username": tr("Username"), "password": tr("Password"), "confirm": tr("Repeat password"),
                 "email": tr("Email")}
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

        y = self.draw_acc_message(cx, y - 4, card_w)
        submit = pygame.Rect(cx - 150, y + 44 - 4, 300, 50)
        if self.acc_busy:
            label = tr("Creating account...") if register else tr("Logging in...")
        else:
            label = tr("Create account") if register else tr("Log in")
        self.button(submit, label, self.font_med, mouse_pos, ACCENT, ACCENT_HOVER, BLACK,
                    callback=self.submit_account, radius=12, enabled=not self.acc_busy)

        ny = submit.bottom + 12
        if self.acc_tab in ("login", "recover"):
            forgot_rect = pygame.Rect(cx - 100, ny, 200, 26)
            self.button(forgot_rect, tr("Forgot password?"), self.font_tiny, mouse_pos,
                        PANEL, PANEL_LIGHT, ACCENT, callback=self.open_forgot, radius=6, sfx=None)
            ny = forgot_rect.bottom + 8

        if register:
            note = tr("An email is required so you can recover your account later. We'll send you a "
                      "6-digit code to confirm it. Your local saves are never touched.")
        elif self.acc_tab == "recover":
            note = tr("For accounts made before emails were required and never confirmed one - "
                      "username and password, same as always. You'll be asked to add an email right after.")
        else:
            note = tr("Log in with the email you confirmed for your account.")
        for line in wrap_text(note, self.font_tiny, card_w - 56):
            t = self.font_tiny.render(line, True, GREY)
            self.canvas.blit(t, t.get_rect(center=(cx, ny + 8)))
            ny += 16
        back_button(rect.bottom + 24)

    # ---------------------------------------------------------------- desenho: confirmar código
    def draw_verify_stage(self, mouse_pos, cx, x0, card_w, back_button):
        rect = pygame.Rect(x0, 118, card_w, 372)
        draw_panel(self.canvas, rect, PANEL, radius=16)
        title = self.font_big.render(tr("Confirm your email"), True, WHITE)
        self.canvas.blit(title, title.get_rect(center=(cx, rect.y + 34)))
        email = self.acc_pending_email or (self.account or {}).get("email") or ""
        y = rect.y + 66
        for line in wrap_text(tr("We sent a 6-digit code to %s. It expires in 10 minutes.", email),
                              self.font_small, card_w - 56):
            t = self.font_small.render(line, True, GREY)
            self.canvas.blit(t, t.get_rect(center=(cx, y)))
            y += 20

        y += 12
        self.canvas.blit(self.font_small_b.render(tr("Code"), True, GREY), (rect.x + 26, y))
        code_rect = pygame.Rect(cx - 110, y + 24, 220, 52)
        self.draw_field(code_rect, "code")
        y = code_rect.bottom + 12

        y = self.draw_acc_message(cx, y, card_w)
        confirm_rect = pygame.Rect(cx - 150, y + 20, 300, 50)
        self.button(confirm_rect, tr("Confirming...") if self.acc_busy else tr("Confirm"), self.font_med,
                    mouse_pos, ACCENT, ACCENT_HOVER, BLACK, callback=self.submit_code, radius=12,
                    enabled=not self.acc_busy)

        cooldown = max(0.0, CODE_RESEND_COOLDOWN - (time.time() - self.acc_code_sent_at))
        resend_label = tr("Resend code (%ds)", int(cooldown) + 1) if cooldown > 0 else tr("Resend code")
        resend_rect = pygame.Rect(cx - 110, confirm_rect.bottom + 12, 220, 32)
        self.button(resend_rect, resend_label, self.font_small, mouse_pos, PANEL_LIGHT, PANEL_LIGHTER, WHITE,
                    callback=self.resend_code, radius=8, enabled=(not self.acc_busy and cooldown <= 0), sfx=None)
        back_button(rect.bottom + 24)

    # ---------------------------------------------------------------- desenho: re-autenticar (sessão velha)
    def draw_reauth_stage(self, mouse_pos, cx, x0, card_w, back_button):
        rect = pygame.Rect(x0, 118, card_w, 372)
        draw_panel(self.canvas, rect, PANEL, radius=16)
        title = self.font_big.render(tr("Confirm your password"), True, WHITE)
        self.canvas.blit(title, title.get_rect(center=(cx, rect.y + 34)))
        y = rect.y + 66
        for line in wrap_text(tr("Your email was confirmed, but your session is a bit old. Enter your "
                                 "password once more to finish."), self.font_small, card_w - 56):
            t = self.font_small.render(line, True, GREY)
            self.canvas.blit(t, t.get_rect(center=(cx, y)))
            y += 20

        y += 12
        self.canvas.blit(self.font_small_b.render(tr("Password"), True, GREY), (rect.x + 26, y))
        pw_rect = pygame.Rect(rect.x + 24, y + 24, card_w - 48, 46)
        self.draw_field(pw_rect, "password")
        toggle = pygame.Rect(pw_rect.right - 84, pw_rect.y - 27, 84, 26)

        def toggle_pw():
            self.acc_show_pw = not self.acc_show_pw
        self.button(toggle, tr("Hide") if self.acc_show_pw else tr("Show"), self.font_tiny, mouse_pos,
                    PANEL_LIGHT, PANEL_LIGHTER, WHITE, callback=toggle_pw, radius=8, sfx=None)
        y = pw_rect.bottom + 12

        y = self.draw_acc_message(cx, y, card_w)
        confirm_rect = pygame.Rect(cx - 150, y + 20, 300, 50)
        self.button(confirm_rect, tr("Confirming...") if self.acc_busy else tr("Confirm"), self.font_med,
                    mouse_pos, ACCENT, ACCENT_HOVER, BLACK, callback=self.submit_reauth, radius=12,
                    enabled=not self.acc_busy)
        back_button(confirm_rect.bottom + 24)

    # ---------------------------------------------------------------- desenho: associar email (conta antiga)
    def draw_link_email_stage(self, mouse_pos, cx, x0, card_w, back_button):
        rect = pygame.Rect(x0, 118, card_w, 322)
        draw_panel(self.canvas, rect, PANEL, radius=16)
        title = self.font_big.render(tr("Add an email"), True, WHITE)
        self.canvas.blit(title, title.get_rect(center=(cx, rect.y + 34)))
        y = rect.y + 66
        for line in wrap_text(tr("This account doesn't have an email yet. Add one so you can recover "
                                 "your account and reset your password later."), self.font_small, card_w - 56):
            t = self.font_small.render(line, True, GREY)
            self.canvas.blit(t, t.get_rect(center=(cx, y)))
            y += 20

        y += 12
        self.canvas.blit(self.font_small_b.render(tr("Email"), True, GREY), (rect.x + 26, y))
        self.draw_field(pygame.Rect(rect.x + 24, y + 24, card_w - 48, 46), "email")
        y += 24 + 46 + 12

        y = self.draw_acc_message(cx, y, card_w)
        submit = pygame.Rect(cx - 150, y + 20, 300, 50)
        self.button(submit, tr("Sending...") if self.acc_busy else tr("Send code"), self.font_med, mouse_pos,
                    ACCENT, ACCENT_HOVER, BLACK, callback=self.submit_link_email, radius=12,
                    enabled=not self.acc_busy)
        back_button(rect.bottom + 24)

    # ---------------------------------------------------------------- desenho: esqueci-me da password
    def draw_forgot_stage(self, mouse_pos, cx, x0, card_w, back_button):
        rect = pygame.Rect(x0, 118, card_w, 306)
        draw_panel(self.canvas, rect, PANEL, radius=16)
        title = self.font_big.render(tr("Forgot password"), True, WHITE)
        self.canvas.blit(title, title.get_rect(center=(cx, rect.y + 34)))
        y = rect.y + 66
        for line in wrap_text(tr("Enter your username. If your account has a confirmed email, we'll "
                                 "send it a link to set a new password."), self.font_small, card_w - 56):
            t = self.font_small.render(line, True, GREY)
            self.canvas.blit(t, t.get_rect(center=(cx, y)))
            y += 20

        y += 12
        self.canvas.blit(self.font_small_b.render(tr("Username"), True, GREY), (rect.x + 26, y))
        self.draw_field(pygame.Rect(rect.x + 24, y + 24, card_w - 48, 46), "username")
        y += 24 + 46 + 12

        y = self.draw_acc_message(cx, y, card_w)
        submit = pygame.Rect(cx - 150, y + 20, 300, 50)
        self.button(submit, tr("Sending...") if self.acc_busy else tr("Send reset link"), self.font_med,
                    mouse_pos, ACCENT, ACCENT_HOVER, BLACK, callback=self.submit_forgot, radius=12,
                    enabled=not self.acc_busy)
        cancel = pygame.Rect(cx - 100, submit.bottom + 10, 200, 32)
        self.button(cancel, tr("Cancel"), self.font_small, mouse_pos, PANEL_LIGHT, PANEL_LIGHTER, WHITE,
                    callback=self.close_forgot, radius=8, sfx=None)
        back_button(rect.bottom + 24)

    def draw_forgot_sent_stage(self, mouse_pos, cx, x0, card_w, back_button):
        rect = pygame.Rect(x0, 130, card_w, 220)
        draw_panel(self.canvas, rect, PANEL, radius=16)
        title = self.font_big.render(tr("Check your email"), True, GOOD)
        self.canvas.blit(title, title.get_rect(center=(cx, rect.y + 40)))
        email = getattr(self, "acc_forgot_email", "") or ""
        y = rect.y + 80
        for line in wrap_text(tr("We sent a password reset link to %s. Open it to set a new password, "
                                 "then come back and log in.", email), self.font_small, card_w - 56):
            t = self.font_small.render(line, True, GREY)
            self.canvas.blit(t, t.get_rect(center=(cx, y)))
            y += 20
        ok = pygame.Rect(cx - 130, rect.bottom - 60, 260, 44)
        self.button(ok, tr("Back to login"), self.font_med, mouse_pos, ACCENT, ACCENT_HOVER, BLACK,
                    callback=self.close_forgot, radius=10)
        back_button(rect.bottom + 24)

    def draw_account_chip(self, mouse_pos):
        """Canto superior direito do menu / dos saves: 'Log in' ou o nome da conta."""
        w = 220
        rect = pygame.Rect(self.vw - w - 24, 20, w, 44)
        if self.account:
            label = fit_text(self.font_med, self.account["username"], w - 24)
            color = ACCENT if self.account.get("email_verified") else BAD
            self.button(rect, label, self.font_med, mouse_pos, PANEL_LIGHT, PANEL_LIGHTER, color,
                        callback=self.open_account, radius=12)
        else:
            self.button(rect, tr("Log in"), self.font_med, mouse_pos, ACCENT, ACCENT_HOVER, BLACK,
                        callback=self.open_account, radius=12)