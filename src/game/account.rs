//! Account screen: register / log in / recover / email verification (ui/account_panel.py).

use super::base::{Bo, FieldRef};
use super::{Account, Game, KeyEv, cb};
use crate::core::state::{RNG, now_ts};
use crate::gfx::{Color, Rect};
use crate::i18n::tr;
use crate::online::firebase::{MIN_PASSWORD, OnlineError, Res, online_error_text, server_clock_synced, server_now, username_ok, username_to_email};
use crate::online::mailer::send_verification_code;
use crate::theme::*;
use crate::tr;
use crate::ui::drawing::draw_panel;
use crate::ui::fonts::{fit_text, wrap_text};
use crate::ui::widgets::TextField;
use indexmap::IndexMap;
use sdl2::keyboard::Keycode;
use std::rc::Rc;

const CODE_RESEND_COOLDOWN: f64 = 20.0;
const CODE_EXPIRY: f64 = 600.0;

/// ^[^@\s]+@[^@\s]+\.[^@\s]+$
pub fn email_ok(s: &str) -> bool {
    if s.chars().any(char::is_whitespace) {
        return false;
    }
    let mut parts = s.split('@');
    let (Some(local), Some(domain), None) = (parts.next(), parts.next(), parts.next()) else { return false };
    if local.is_empty() {
        return false;
    }
    match domain.rfind('.') {
        Some(i) => i > 0 && i + 1 < domain.len(),
        None => false,
    }
}

fn server_err(e: String) -> OnlineError {
    OnlineError::new("server", e)
}

fn upper_msg(e: &OnlineError) -> String {
    e.message.to_uppercase()
}

pub struct AccountUi {
    pub tab: &'static str,
    pub stage: &'static str,
    pub fields: IndexMap<&'static str, TextField>,
    pub focus: Option<&'static str>,
    pub msg: Option<(String, Color)>,
    pub busy: bool,
    pub show_pw: bool,
    pub pending_email: Option<String>,
    pub reauth_email: Option<String>,
    pub code_sent_at: f64,
    pub forgot_email: Option<String>,
}

impl AccountUi {
    pub fn new() -> AccountUi {
        let mut fields = IndexMap::new();
        fields.insert("username", TextField::new("username"));
        fields.insert("password", TextField::new("password"));
        fields.insert("confirm", TextField::new("password"));
        fields.insert("email", TextField::new("email"));
        fields.insert("code", TextField::new("code"));
        AccountUi {
            tab: "login",
            stage: "form",
            fields,
            focus: None,
            msg: None,
            busy: false,
            show_pw: false,
            pending_email: None,
            reauth_email: None,
            code_sent_at: 0.0,
            forgot_email: None,
        }
    }

    fn text(&self, key: &str) -> String {
        self.fields[key].text.trim().to_string()
    }

    fn clear(&mut self, key: &str) {
        if let Some(f) = self.fields.get_mut(key) {
            f.set_text("");
        }
    }
}

impl Game {
    pub fn open_account(&mut self) {
        self.account_return = if self.screen_mode == "title" || self.screen_mode == "saves" { if self.screen_mode == "title" { "title" } else { "saves" } } else { "title" };
        self.close_overlays();
        self.acc.msg = None;
        self.acc.busy = false;
        self.acc.show_pw = false;
        if !(self.account.is_some() && matches!(self.acc.stage, "verify" | "link_email" | "reauth")) {
            self.acc.stage = "form";
            if self.account.is_none() {
                self.acc.tab = "login";
            }
        }
        if self.account.is_none() && self.acc.stage == "form" {
            self.acc.focus = Some(self.acc_visible_fields()[0]);
        }
        self.screen_mode = "account";
        self.start_text_input();
    }

    pub fn close_account(&mut self) {
        self.stop_text_input();
        self.acc.focus = None;
        if matches!(self.acc.stage, "verify" | "reauth" | "link_email" | "forgot" | "forgot_sent") {
            self.acc.stage = "form";
        }
        self.screen_mode = self.account_return;
        if self.screen_mode == "saves" {
            self.open_saves();
        }
    }

    pub fn set_acc_tab(&mut self, tab: &'static str) {
        self.acc.tab = tab;
        self.acc.msg = None;
        self.acc.focus = Some(self.acc_visible_fields()[0]);
    }

    fn acc_visible_fields(&self) -> Vec<&'static str> {
        match self.acc.stage {
            "verify" => vec!["code"],
            "reauth" => vec!["password"],
            "link_email" => vec!["email"],
            "forgot" => vec!["username"],
            _ => match self.acc.tab {
                "register" => vec!["username", "email", "password", "confirm"],
                "recover" => vec!["username", "password"],
                _ => vec!["email", "password"],
            },
        }
    }

    fn submit_current_stage(&mut self) {
        match self.acc.stage {
            "verify" => self.submit_code(),
            "reauth" => self.submit_reauth(),
            "link_email" => self.submit_link_email(),
            "forgot" => self.submit_forgot(),
            _ => {
                if self.account.is_none() {
                    self.submit_account();
                }
            }
        }
    }

    pub fn handle_account_key(&mut self, ev: KeyEv) -> bool {
        let keys = self.acc_visible_fields();
        if ev.key == Keycode::Tab {
            let step: i64 = if ev.shift { -1 } else { 1 };
            let i = match self.acc.focus.and_then(|f| keys.iter().position(|k| *k == f)) {
                Some(p) => (p as i64 + step).rem_euclid(keys.len() as i64) as usize,
                None => 0,
            };
            self.acc.focus = Some(keys[i]);
            return true;
        }
        if matches!(ev.key, Keycode::Return | Keycode::KpEnter) {
            self.submit_current_stage();
            return true;
        }
        let Some(f) = self.acc.focus.filter(|f| self.acc.fields.contains_key(f)) else { return false };
        self.edit_field_key(FieldRef::Acc(f), ev, true)
    }

    // ---------------------------------------------------------------- register / log in
    fn submit_account(&mut self) {
        if self.acc.busy || self.account.is_some() {
            return;
        }
        let Some(client) = self.client.clone() else { return };
        let register = self.acc.tab == "register";
        let via_recover = self.acc.tab == "recover";
        let pw = self.acc.text("password");
        if pw.chars().count() < MIN_PASSWORD {
            self.acc.msg = Some((tr!("Password must have at least %d characters.", MIN_PASSWORD as i64), BAD));
            return;
        }
        let email;
        let job: Box<dyn FnOnce() -> Res<()> + Send>;
        if register {
            let name = self.acc.text("username").to_lowercase();
            if !username_ok(&name) {
                self.acc.msg = Some((tr("Username: 3 to 16 characters, only letters, numbers and _"), BAD));
                return;
            }
            email = self.acc.text("email");
            if !email_ok(&email) {
                self.acc.msg = Some((tr("Enter a valid email address."), BAD));
                return;
            }
            if pw != self.acc.text("confirm") {
                self.acc.msg = Some((tr("The two passwords don't match."), BAD));
                return;
            }
            job = Box::new(move || client.sign_up(&name, &pw));
        } else if via_recover {
            let name = self.acc.text("username").to_lowercase();
            if !username_ok(&name) {
                self.acc.msg = Some((tr("Username: 3 to 16 characters, only letters, numbers and _"), BAD));
                return;
            }
            email = String::new();
            job = Box::new(move || client.sign_in(&name, &pw));
        } else {
            email = self.acc.text("email");
            if !email_ok(&email) {
                self.acc.msg = Some((tr("Enter a valid email address."), BAD));
                return;
            }
            let e2 = email.clone();
            job = Box::new(move || client.sign_in_by_email(&e2, &pw).map(|_| ()));
        }
        self.acc.busy = true;
        self.acc.msg = Some((tr("Please wait..."), grey()));
        self.run_job(job, move |g, _: ()| g.on_auth_ok(register, email, via_recover), |g, e| g.on_auth_err(e));
    }

    fn on_auth_ok(&mut self, registered: bool, email: String, via_recover: bool) {
        self.acc.busy = false;
        let client = self.client.clone().unwrap();
        self.account = Some(Account { uid: client.uid().unwrap_or_default(), username: client.username().unwrap_or_default(), email: None, email_verified: false });
        self.pub_last_time = 0.0;
        self.save_session_file();
        self.acc.msg = None;
        self.acc.show_pw = false;
        for f in self.acc.fields.values_mut() {
            f.set_text("");
        }
        if registered {
            self.begin_email_verification(email, true);
        } else {
            self.acc.busy = true;
            self.acc.msg = Some((tr("Checking your account..."), grey()));
            self.run_job(
                move || client.get_profile(),
                move |g, p| g.on_login_profile(p, via_recover),
                |g, _| {
                    g.acc.busy = false;
                    g.finish_login_flow();
                },
            );
        }
    }

    fn on_auth_err(&mut self, e: OnlineError) {
        self.acc.busy = false;
        let msg = upper_msg(&e);
        if self.acc.tab == "login" && ["EMAIL_NOT_FOUND", "INVALID_PASSWORD", "INVALID_LOGIN_CREDENTIALS"].iter().any(|k| msg.contains(k)) {
            self.acc.msg = Some((tr("Wrong email or password."), BAD));
        } else {
            self.acc.msg = Some((online_error_text(&e), BAD));
        }
    }

    fn on_login_profile(&mut self, profile: Option<crate::online::firebase::AccountProfile>, via_recover: bool) {
        self.acc.busy = false;
        let verified = profile.as_ref().is_some_and(|p| p.email_verified);
        if via_recover && verified {
            self.log_out();
            self.acc.tab = "login";
            self.acc.msg = Some((tr("This account already has an email. Use \"Log in\" instead."), BAD));
            return;
        }
        let email = profile.as_ref().and_then(|p| p.email.clone()).filter(|e| !e.is_empty());
        if verified {
            if let Some(a) = self.account.as_mut() {
                a.email_verified = true;
                a.email = profile.and_then(|p| p.email);
            }
            self.finish_login_flow();
        } else if let Some(email) = email {
            if let Some(a) = self.account.as_mut() {
                a.email = Some(email.clone());
            }
            self.acc.pending_email = Some(email);
            self.enter_verify_stage();
            self.show_toast(&tr("You still need to confirm your email."), 1.8);
        } else {
            self.enter_link_email_stage();
            self.show_toast(&tr("Add an email to your account so you can recover it later."), 1.8);
        }
    }

    fn finish_login_flow(&mut self) {
        self.stop_text_input();
        self.acc.focus = None;
        self.acc.stage = "form";
        self.refresh_slot_info();
        self.open_saves();
        let name = self.account.as_ref().map(|a| a.username.clone()).unwrap_or_default();
        self.show_toast(&tr!("Welcome back, %s!", name), 1.8);
    }

    pub fn require_email_link(&mut self) {
        if self.screen_mode == "account" && matches!(self.acc.stage, "verify" | "link_email") {
            return;
        }
        self.account_return = "title";
        self.close_overlays();
        self.acc.msg = None;
        self.acc.busy = false;
        self.screen_mode = "account";
        if self.acc.pending_email.is_some() {
            self.enter_verify_stage();
        } else {
            self.enter_link_email_stage();
        }
        self.show_toast(&tr("Add an email to your account so you can recover it later."), 1.8);
    }

    // ---------------------------------------------------------------- code verification
    fn enter_link_email_stage(&mut self) {
        self.acc.stage = "link_email";
        self.acc.clear("email");
        self.acc.focus = Some("email");
        self.acc.msg = None;
        self.start_text_input();
    }

    fn enter_verify_stage(&mut self) {
        self.acc.stage = "verify";
        self.acc.clear("code");
        self.acc.focus = Some("code");
        self.acc.msg = None;
        self.start_text_input();
    }

    fn submit_link_email(&mut self) {
        if self.acc.busy {
            return;
        }
        let email = self.acc.text("email");
        if !email_ok(&email) {
            self.acc.msg = Some((tr("Enter a valid email address."), BAD));
            return;
        }
        self.begin_email_verification(email, false);
    }

    fn begin_email_verification(&mut self, email: String, welcome_new: bool) {
        if self.acc.busy {
            return;
        }
        self.acc.busy = true;
        self.acc.msg = Some((tr("Sending the code..."), grey()));
        let code = format!("{:06}", RNG.with(|r| r.borrow_mut().randint(0, 999999)));
        let client = self.client.clone().unwrap();
        let e2 = email.clone();
        self.run_job(
            move || {
                client.set_pending_code(&e2, &code)?;
                send_verification_code(&e2, &code).map_err(server_err)
            },
            move |g, _: ()| {
                g.acc.busy = false;
                g.acc.pending_email = Some(email.clone());
                g.acc.code_sent_at = now_ts();
                g.enter_verify_stage();
                let t = if welcome_new { tr!("Account created! We sent a code to %s.", email) } else { tr!("Code sent to %s.", email) };
                g.show_toast(&t, 1.8);
            },
            |g, e| {
                g.acc.busy = false;
                g.acc.msg = Some((online_error_text(&e), BAD));
            },
        );
    }

    fn resend_code(&mut self) {
        if self.acc.busy {
            return;
        }
        if now_ts() - self.acc.code_sent_at < CODE_RESEND_COOLDOWN {
            self.acc.msg = Some((tr("Please wait a moment before resending."), grey()));
            return;
        }
        if let Some(e) = self.acc.pending_email.clone() {
            self.begin_email_verification(e, false);
        }
    }

    fn submit_code(&mut self) {
        if self.acc.busy {
            return;
        }
        let entered = self.acc.text("code");
        if entered.chars().count() != 6 {
            self.acc.msg = Some((tr("Enter the 6-digit code."), BAD));
            return;
        }
        self.acc.busy = true;
        self.acc.msg = Some((tr("Checking..."), grey()));
        let client = self.client.clone().unwrap();
        let username = self.account.as_ref().map(|a| a.username.clone()).unwrap_or_default();
        self.run_job(
            move || {
                let profile = client.get_profile()?;
                let Some(profile) = profile.filter(|p| p.pending_code.as_deref().is_some_and(|c| !c.is_empty())) else {
                    return Err(OnlineError::new("bad_request", "NO_PENDING_CODE"));
                };
                if profile.pending_code.as_deref() != Some(entered.as_str()) {
                    return Err(OnlineError::new("bad_request", "WRONG_CODE"));
                }
                if let Some(created) = profile.code_created {
                    if server_clock_synced() && server_now() - created > CODE_EXPIRY {
                        return Err(OnlineError::new("bad_request", "CODE_EXPIRED"));
                    }
                }
                let email = profile.email.unwrap_or_default();
                client.confirm_email()?;
                client.publish_username_email(&username, &email)?;
                if let Err(e) = client.change_login_email(&email) {
                    if upper_msg(&e).contains("CREDENTIAL_TOO_OLD_LOGIN_AGAIN") {
                        return Err(OnlineError::new("reauth_required", email));
                    }
                    return Err(e);
                }
                Ok(email)
            },
            |g, email| g.on_verify_ok(email),
            |g, e| g.on_verify_err(e),
        );
    }

    fn on_verify_ok(&mut self, email: String) {
        self.acc.busy = false;
        if let Some(a) = self.account.as_mut() {
            a.email_verified = true;
            a.email = Some(email);
        }
        self.acc.pending_email = None;
        self.acc.reauth_email = None;
        self.save_session_file();
        self.stop_text_input();
        self.acc.focus = None;
        self.acc.stage = "form";
        self.refresh_slot_info();
        self.open_saves();
        self.show_toast(&tr("Email confirmed! Your account can now be recovered."), 1.8);
    }

    fn on_verify_err(&mut self, e: OnlineError) {
        self.acc.busy = false;
        if e.code == "reauth_required" {
            self.enter_reauth_stage(e.message.clone());
            return;
        }
        let msg = upper_msg(&e);
        self.acc.msg = Some((
            if msg.contains("WRONG_CODE") {
                tr("Wrong code. Check your email and try again.")
            } else if msg.contains("CODE_EXPIRED") {
                tr("This code expired. Tap Resend to get a new one.")
            } else if msg.contains("NO_PENDING_CODE") {
                tr("No pending code. Tap Resend to get one.")
            } else {
                online_error_text(&e)
            },
            BAD,
        ));
    }

    fn enter_reauth_stage(&mut self, email: String) {
        self.acc.reauth_email = Some(email);
        self.acc.stage = "reauth";
        self.acc.clear("password");
        self.acc.focus = Some("password");
        self.acc.msg = Some((tr("Your session is a bit old. Enter your password to finish confirming your email."), grey()));
        self.start_text_input();
    }

    fn submit_reauth(&mut self) {
        if self.acc.busy {
            return;
        }
        let pw = self.acc.text("password");
        if pw.chars().count() < MIN_PASSWORD {
            self.acc.msg = Some((tr!("Password must have at least %d characters.", MIN_PASSWORD as i64), BAD));
            return;
        }
        self.acc.busy = true;
        self.acc.msg = Some((tr("Checking..."), grey()));
        let client = self.client.clone().unwrap();
        let username = self.account.as_ref().map(|a| a.username.clone()).unwrap_or_default();
        let email = self.acc.reauth_email.clone().unwrap_or_default();
        self.run_job(
            move || {
                client.reauth_pending_link(&username, &pw)?;
                client.change_login_email(&email)?;
                Ok(email)
            },
            |g, email| g.on_verify_ok(email),
            |g, e| {
                g.acc.busy = false;
                g.acc.msg = Some((online_error_text(&e), BAD));
            },
        );
    }

    // ---------------------------------------------------------------- forgot password
    fn open_forgot(&mut self) {
        self.acc.stage = "forgot";
        self.acc.focus = Some("username");
        self.acc.msg = None;
    }

    fn close_forgot(&mut self) {
        self.acc.stage = "form";
        self.acc.msg = None;
    }

    fn submit_forgot(&mut self) {
        if self.acc.busy {
            return;
        }
        let username = self.acc.text("username").to_lowercase();
        if !username_ok(&username) {
            self.acc.msg = Some((tr("Enter your username."), BAD));
            return;
        }
        self.acc.busy = true;
        self.acc.msg = Some((tr("Please wait..."), grey()));
        let client = self.client.clone().unwrap();
        self.run_job(
            move || {
                let email = client.resolve_login_email(&username)?;
                if email == username_to_email(&username) {
                    return Err(OnlineError::new("bad_request", "NO_EMAIL_LINKED"));
                }
                client.send_password_reset(&email)?;
                Ok(email)
            },
            |g, email: String| {
                g.acc.busy = false;
                g.acc.msg = None;
                g.acc.forgot_email = Some(email);
                g.acc.stage = "forgot_sent";
            },
            |g, e| {
                g.acc.busy = false;
                let msg = upper_msg(&e);
                g.acc.msg = Some((
                    if msg.contains("NO_EMAIL_LINKED") {
                        tr("This account has no confirmed email yet. Log in with your password first, then add one.")
                    } else if e.code == "not_found" {
                        tr("Username not found.")
                    } else {
                        online_error_text(&e)
                    },
                    BAD,
                ));
            },
        );
    }

    // ================================================================ drawing
    fn draw_field(&mut self, rect: Rect, key: &'static str) {
        let f = &self.acc.fields[key];
        let display = if f.kind == "password" && !self.acc.show_pw { "*".repeat(f.len()) } else { f.text.clone() };
        let placeholder = match key {
            "username" => tr("Your username"),
            "password" => tr("Your password"),
            "confirm" => tr("Repeat the password"),
            "email" => tr("Your email"),
            _ => tr("6-digit code"),
        };
        let focused = self.acc.focus == Some(key);
        self.draw_text_field(rect, FieldRef::Acc(key), &display, &placeholder, focused, Rc::new(move |g: &mut Game| g.acc.focus = Some(key)));
    }

    fn draw_acc_message(&mut self, cx: i32, mut y: i32, card_w: i32) -> i32 {
        let Some((text, color)) = self.acc.msg.clone() else { return y };
        let small = self.f.small.clone();
        for line in wrap_text(&text, &small, card_w - 48).iter().take(3) {
            let t = small.render(line, color);
            let r = Rect::with_center(t.w, t.h, (cx, y + 10));
            self.canvas.blit(&t, r.x, r.y);
            y += 20;
        }
        y
    }

    fn acc_back_button(&mut self, cx: i32, y: i32, mouse_pos: (f64, f64)) {
        let med = self.f.med.clone();
        self.button(Rect::new(cx - 110, y, 220, 48), &tr("Back"), &med, mouse_pos, panel_light(), panel_lighter(), WHITE, cb(|g| g.close_account()), Bo::r(12));
    }

    fn blit_centered_lines(&mut self, text: &str, cx: i32, mut y: i32, width: i32, step: i32) -> i32 {
        let small = self.f.small.clone();
        for line in wrap_text(text, &small, width) {
            let t = small.render(&line, grey());
            let r = Rect::with_center(t.w, t.h, (cx, y));
            self.canvas.blit(&t, r.x, r.y);
            y += step;
        }
        y
    }

    fn blit_title(&mut self, text: &str, color: Color, center: (i32, i32)) {
        let t = self.f.big.render(text, color);
        let r = Rect::with_center(t.w, t.h, center);
        self.canvas.blit(&t, r.x, r.y);
    }

    fn show_pw_toggle(&mut self, rect: Rect, mouse_pos: (f64, f64)) {
        let tiny = self.f.tiny.clone();
        let label = if self.acc.show_pw { tr("Hide") } else { tr("Show") };
        self.button(rect, &label, &tiny, mouse_pos, panel_light(), panel_lighter(), WHITE, cb(|g| g.acc.show_pw = !g.acc.show_pw), Bo::r(8).sfx(None));
    }

    pub fn draw_account(&mut self, mouse_pos: (f64, f64)) {
        let cx = self.vw / 2;
        let head = self.f.huge.render(&tr("Account"), WHITE);
        let r = Rect::with_center(head.w, head.h, (cx, 70));
        self.canvas.blit(&head, r.x, r.y);
        let card_w = 480;
        let x0 = cx - card_w / 2;

        if self.client.is_none() {
            let rect = Rect::new(x0, 130, card_w, 200);
            draw_panel(&mut self.canvas, rect, Some(panel()), 16, true, None);
            let mut y = rect.y + 30;
            let small = self.f.small.clone();
            for line in wrap_text(&tr("Online features aren't set up yet. Add your Firebase keys to the script (or to firebase_config.json) - see FIREBASE_SETUP.md."), &small, card_w - 56) {
                let t = small.render(&line, grey());
                self.canvas.blit(&t, rect.x + 28, y);
                y += 24;
            }
            self.acc_back_button(cx, rect.bottom() + 24, mouse_pos);
            return;
        }
        let logged = self.account.is_some();
        match self.acc.stage {
            "verify" if logged => return self.draw_verify_stage(mouse_pos, cx, x0, card_w),
            "reauth" if logged => return self.draw_reauth_stage(mouse_pos, cx, x0, card_w),
            "link_email" if logged => return self.draw_link_email_stage(mouse_pos, cx, x0, card_w),
            _ => {}
        }
        if logged {
            self.draw_logged_in(mouse_pos, cx, x0, card_w);
        } else if self.acc.stage == "forgot" {
            self.draw_forgot_stage(mouse_pos, cx, x0, card_w);
        } else if self.acc.stage == "forgot_sent" {
            self.draw_forgot_sent_stage(mouse_pos, cx, x0, card_w);
        } else {
            self.draw_login_register(mouse_pos, cx, x0, card_w);
        }
    }

    fn draw_logged_in(&mut self, mouse_pos: (f64, f64), cx: i32, x0: i32, card_w: i32) {
        let acc = self.account.clone().unwrap();
        let verified = acc.email_verified;
        let card_h = if verified { 300 } else { 356 };
        let rect = Rect::new(x0, 130, card_w, card_h);
        draw_panel(&mut self.canvas, rect, Some(panel()), 16, true, None);
        let lbl = self.f.small.render(&tr("Logged in as"), grey());
        let r = Rect::with_center(lbl.w, lbl.h, (cx, rect.y + 44));
        self.canvas.blit(&lbl, r.x, r.y);
        let huge = self.f.huge.clone();
        let name = huge.render(&fit_text(&huge, &acc.username, card_w - 60), accent());
        let r = Rect::with_center(name.w, name.h, (cx, rect.y + 92));
        self.canvas.blit(&name, r.x, r.y);
        let mut y = self.blit_centered_lines(&tr("Your saves are stored in your account and sync to the cloud while you play. Local saves on this PC are never deleted."), cx, rect.y + 142, card_w - 60, 22);
        if !verified {
            y += 6;
            let sb = self.f.small_b.clone();
            let warn = fit_text(&sb, &tr("Your account has no confirmed email yet."), card_w - 60);
            let t = sb.render(&warn, BAD);
            let r = Rect::with_center(t.w, t.h, (cx, y));
            self.canvas.blit(&t, r.x, r.y);
            y += 30;
            let fix_rect = Rect::new(cx - 150, y, 300, 42);
            self.button(
                fix_rect,
                &tr("Add / confirm email"),
                &sb,
                mouse_pos,
                accent(),
                accent_hover(),
                BLACK,
                cb(|g| {
                    if g.acc.pending_email.is_some() {
                        g.enter_verify_stage();
                    } else {
                        g.enter_link_email_stage();
                    }
                }),
                Bo::r(10),
            );
        }
        let med = self.f.med.clone();
        self.button(Rect::new(cx - 120, rect.bottom() - 76, 240, 48), &tr("Log out"), &med, mouse_pos, Color::rgb(150, 60, 60), BAD, WHITE, cb(|g| g.log_out_clicked()), Bo::r(12));
        self.acc_back_button(cx, rect.bottom() + 24, mouse_pos);
    }

    fn draw_login_register(&mut self, mouse_pos: (f64, f64), cx: i32, x0: i32, card_w: i32) {
        let register = self.acc.tab == "register";
        let keys = self.acc_visible_fields();
        let card_h = 130 + 86 * keys.len() as i32 + 44 + 50 + 16 + 40 + 22;
        let rect = Rect::new(x0, 118, card_w, card_h);
        draw_panel(&mut self.canvas, rect, Some(panel()), 16, true, None);

        let sb = self.f.small_b.clone();
        let tiny = self.f.tiny.clone();
        let med = self.f.med.clone();
        let tabs = [("login", tr("Log in")), ("register", tr("Register")), ("recover", tr("Old account"))];
        let gap = 8;
        let tab_w = (card_w - 48 - gap * 2) / 3;
        for (i, (key, label)) in tabs.iter().enumerate() {
            let trect = Rect::new(rect.x + 24 + i as i32 * (tab_w + gap), rect.y + 22, tab_w, 42);
            let active = self.acc.tab == *key;
            let k: &'static str = key;
            self.button(
                trect,
                &fit_text(&sb, label, tab_w - 10),
                &sb,
                mouse_pos,
                if active { accent() } else { panel_light() },
                if active { accent_hover() } else { panel_lighter() },
                if active { BLACK } else { WHITE },
                cb(move |g| g.set_acc_tab(k)),
                Bo::r(10),
            );
        }

        let mut y = rect.y + 84;
        for &key in &keys {
            let label = match key {
                "username" => tr("Username"),
                "password" => tr("Password"),
                "confirm" => tr("Repeat password"),
                _ => tr("Email"),
            };
            let t = sb.render(&label, grey());
            self.canvas.blit(&t, rect.x + 26, y);
            self.draw_field(Rect::new(rect.x + 24, y + 24, card_w - 48, 46), key);
            if key == "password" {
                self.show_pw_toggle(Rect::new(rect.right() - 24 - 84, y - 3, 84, 26), mouse_pos);
            }
            y += 86;
        }

        let y = self.draw_acc_message(cx, y - 4, card_w);
        let submit = Rect::new(cx - 150, y + 44 - 4, 300, 50);
        let busy = self.acc.busy;
        let label = if busy {
            if register { tr("Creating account...") } else { tr("Logging in...") }
        } else if register {
            tr("Create account")
        } else {
            tr("Log in")
        };
        self.button(submit, &label, &med, mouse_pos, accent(), accent_hover(), BLACK, cb(|g| g.submit_account()), Bo::r(12).enabled(!busy));

        let mut ny = submit.bottom() + 12;
        if self.acc.tab == "login" || self.acc.tab == "recover" {
            let forgot_rect = Rect::new(cx - 100, ny, 200, 26);
            self.button(forgot_rect, &tr("Forgot password?"), &tiny, mouse_pos, panel(), panel_light(), accent(), cb(|g| g.open_forgot()), Bo::r(6).sfx(None));
            ny = forgot_rect.bottom() + 8;
        }
        let note = if register {
            tr("An email is required so you can recover your account later. We'll send you a 6-digit code to confirm it. Your local saves are never touched.")
        } else if self.acc.tab == "recover" {
            tr("For accounts made before emails were required and never confirmed one - username and password, same as always. You'll be asked to add an email right after.")
        } else {
            tr("Log in with the email you confirmed for your account.")
        };
        for line in wrap_text(&note, &tiny, card_w - 56) {
            let t = tiny.render(&line, grey());
            let r = Rect::with_center(t.w, t.h, (cx, ny + 8));
            self.canvas.blit(&t, r.x, r.y);
            ny += 16;
        }
        self.acc_back_button(cx, rect.bottom() + 24, mouse_pos);
    }

    fn draw_verify_stage(&mut self, mouse_pos: (f64, f64), cx: i32, x0: i32, card_w: i32) {
        let rect = Rect::new(x0, 118, card_w, 372);
        draw_panel(&mut self.canvas, rect, Some(panel()), 16, true, None);
        self.blit_title(&tr("Confirm your email"), WHITE, (cx, rect.y + 34));
        let email = self.acc.pending_email.clone().or_else(|| self.account.as_ref().and_then(|a| a.email.clone())).unwrap_or_default();
        let mut y = self.blit_centered_lines(&tr!("We sent a 6-digit code to %s. It expires in 10 minutes.", email), cx, rect.y + 66, card_w - 56, 20);
        y += 12;
        let sb = self.f.small_b.clone();
        let t = sb.render(&tr("Code"), grey());
        self.canvas.blit(&t, rect.x + 26, y);
        let code_rect = Rect::new(cx - 110, y + 24, 220, 52);
        self.draw_field(code_rect, "code");
        let y = self.draw_acc_message(cx, code_rect.bottom() + 12, card_w);
        let confirm_rect = Rect::new(cx - 150, y + 20, 300, 50);
        let busy = self.acc.busy;
        let med = self.f.med.clone();
        self.button(confirm_rect, &if busy { tr("Confirming...") } else { tr("Confirm") }, &med, mouse_pos, accent(), accent_hover(), BLACK, cb(|g| g.submit_code()), Bo::r(12).enabled(!busy));

        let cooldown = (CODE_RESEND_COOLDOWN - (now_ts() - self.acc.code_sent_at)).max(0.0);
        let resend_label = if cooldown > 0.0 { tr!("Resend code (%ds)", cooldown as i64 + 1) } else { tr("Resend code") };
        let resend_rect = Rect::new(cx - 110, confirm_rect.bottom() + 12, 220, 32);
        let small = self.f.small.clone();
        self.button(resend_rect, &resend_label, &small, mouse_pos, panel_light(), panel_lighter(), WHITE, cb(|g| g.resend_code()), Bo::r(8).enabled(!busy && cooldown <= 0.0).sfx(None));
        self.acc_back_button(cx, rect.bottom() + 24, mouse_pos);
    }

    fn draw_reauth_stage(&mut self, mouse_pos: (f64, f64), cx: i32, x0: i32, card_w: i32) {
        // the card's height follows the text (the description + an error/notice can take more than one line),
        // otherwise "Confirm" and "Back" end up outside the card
        let small = self.f.small.clone();
        let desc_lines = crate::ui::fonts::wrap_text(&tr("Your email was confirmed, but your session is a bit old. Enter your password once more to finish."), &small, card_w - 56).len() as i32;
        let msg_lines = match &self.acc.msg {
            Some((m, _)) => crate::ui::fonts::wrap_text(m, &small, card_w - 48).len().min(3) as i32,
            None => 0,
        };
        let card_h = 66 + desc_lines * 20 + 12 + 24 + 46 + 12 + msg_lines * 20 + 20 + 50;
        let rect = Rect::new(x0, 118, card_w, card_h);
        draw_panel(&mut self.canvas, rect, Some(panel()), 16, true, None);
        self.blit_title(&tr("Confirm your password"), WHITE, (cx, rect.y + 34));
        let mut y = self.blit_centered_lines(&tr("Your email was confirmed, but your session is a bit old. Enter your password once more to finish."), cx, rect.y + 66, card_w - 56, 20);
        y += 12;
        let t = self.f.small_b.render(&tr("Password"), grey());
        self.canvas.blit(&t, rect.x + 26, y);
        let pw_rect = Rect::new(rect.x + 24, y + 24, card_w - 48, 46);
        self.draw_field(pw_rect, "password");
        self.show_pw_toggle(Rect::new(pw_rect.right() - 84, pw_rect.y - 27, 84, 26), mouse_pos);
        let y = self.draw_acc_message(cx, pw_rect.bottom() + 12, card_w);
        let confirm_rect = Rect::new(cx - 150, y + 20, 300, 50);
        let busy = self.acc.busy;
        let med = self.f.med.clone();
        self.button(confirm_rect, &if busy { tr("Confirming...") } else { tr("Confirm") }, &med, mouse_pos, accent(), accent_hover(), BLACK, cb(|g| g.submit_reauth()), Bo::r(12).enabled(!busy));
        self.acc_back_button(cx, confirm_rect.bottom() + 24, mouse_pos);
    }

    fn draw_link_email_stage(&mut self, mouse_pos: (f64, f64), cx: i32, x0: i32, card_w: i32) {
        let rect = Rect::new(x0, 118, card_w, 322);
        draw_panel(&mut self.canvas, rect, Some(panel()), 16, true, None);
        self.blit_title(&tr("Add an email"), WHITE, (cx, rect.y + 34));
        let mut y = self.blit_centered_lines(&tr("This account doesn't have an email yet. Add one so you can recover your account and reset your password later."), cx, rect.y + 66, card_w - 56, 20);
        y += 12;
        let t = self.f.small_b.render(&tr("Email"), grey());
        self.canvas.blit(&t, rect.x + 26, y);
        self.draw_field(Rect::new(rect.x + 24, y + 24, card_w - 48, 46), "email");
        y += 24 + 46 + 12;
        let y = self.draw_acc_message(cx, y, card_w);
        let submit = Rect::new(cx - 150, y + 20, 300, 50);
        let busy = self.acc.busy;
        let med = self.f.med.clone();
        self.button(submit, &if busy { tr("Sending...") } else { tr("Send code") }, &med, mouse_pos, accent(), accent_hover(), BLACK, cb(|g| g.submit_link_email()), Bo::r(12).enabled(!busy));
        self.acc_back_button(cx, rect.bottom() + 24, mouse_pos);
    }

    fn draw_forgot_stage(&mut self, mouse_pos: (f64, f64), cx: i32, x0: i32, card_w: i32) {
        let rect = Rect::new(x0, 118, card_w, 306);
        draw_panel(&mut self.canvas, rect, Some(panel()), 16, true, None);
        self.blit_title(&tr("Forgot password"), WHITE, (cx, rect.y + 34));
        let mut y = self.blit_centered_lines(&tr("Enter your username. If your account has a confirmed email, we'll send it a link to set a new password."), cx, rect.y + 66, card_w - 56, 20);
        y += 12;
        let t = self.f.small_b.render(&tr("Username"), grey());
        self.canvas.blit(&t, rect.x + 26, y);
        self.draw_field(Rect::new(rect.x + 24, y + 24, card_w - 48, 46), "username");
        y += 24 + 46 + 12;
        let y = self.draw_acc_message(cx, y, card_w);
        let submit = Rect::new(cx - 150, y + 20, 300, 50);
        let busy = self.acc.busy;
        let med = self.f.med.clone();
        let small = self.f.small.clone();
        self.button(submit, &if busy { tr("Sending...") } else { tr("Send reset link") }, &med, mouse_pos, accent(), accent_hover(), BLACK, cb(|g| g.submit_forgot()), Bo::r(12).enabled(!busy));
        let cancel = Rect::new(cx - 100, submit.bottom() + 10, 200, 32);
        self.button(cancel, &tr("Cancel"), &small, mouse_pos, panel_light(), panel_lighter(), WHITE, cb(|g| g.close_forgot()), Bo::r(8).sfx(None));
        self.acc_back_button(cx, rect.bottom() + 24, mouse_pos);
    }

    fn draw_forgot_sent_stage(&mut self, mouse_pos: (f64, f64), cx: i32, x0: i32, card_w: i32) {
        let rect = Rect::new(x0, 130, card_w, 220);
        draw_panel(&mut self.canvas, rect, Some(panel()), 16, true, None);
        self.blit_title(&tr("Check your email"), GOOD, (cx, rect.y + 40));
        let email = self.acc.forgot_email.clone().unwrap_or_default();
        self.blit_centered_lines(&tr!("We sent a password reset link to %s. Open it to set a new password, then come back and log in.", email), cx, rect.y + 80, card_w - 56, 20);
        let ok = Rect::new(cx - 130, rect.bottom() - 60, 260, 44);
        let med = self.f.med.clone();
        self.button(ok, &tr("Back to login"), &med, mouse_pos, accent(), accent_hover(), BLACK, cb(|g| g.close_forgot()), Bo::r(10));
        self.acc_back_button(cx, rect.bottom() + 24, mouse_pos);
    }

    pub fn draw_account_chip(&mut self, mouse_pos: (f64, f64)) {
        let w = 220;
        let rect = Rect::new(self.vw - w - 24, 20, w, 44);
        let med = self.f.med.clone();
        if let Some(acc) = self.account.clone() {
            let label = fit_text(&med, &acc.username, w - 24);
            let color = if acc.email_verified { accent() } else { BAD };
            self.button(rect, &label, &med, mouse_pos, panel_light(), panel_lighter(), color, cb(|g| g.open_account()), Bo::r(12));
        } else {
            self.button(rect, &tr("Log in"), &med, mouse_pos, accent(), accent_hover(), BLACK, cb(|g| g.open_account()), Bo::r(12));
        }
    }
}
