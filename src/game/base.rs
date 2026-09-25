//! Base UI pieces: buttons, clipping, scrollbar, icons, toasts (ui/base.py).

use super::{Button, Cb, Game};
use crate::config::TOPBAR_H;
use crate::gfx::{Color, Rect, Surface, draw, ti};
use crate::theme::*;
use crate::ui::drawing::{draw_panel, draw_state_border, rounded_box};
use crate::ui::fonts::{Font, fit_text};
use crate::ui::icons::load_icon;
use crate::ui::widgets::TextField;
use std::rc::Rc;

/// Which text field a widget edits.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum FieldRef {
    Acc(&'static str),
    FriendsSearch,
    Chat,
    Feedback,
    EventMult,
    EventSeconds,
    BanSearch,
    BanReason,
    SellAmount,
    SlotName,
}

/// Optional arguments of button().
#[derive(Clone)]
pub struct Bo {
    pub radius: i32,
    pub enabled: bool,
    pub border: Option<Color>,
    pub sfx: Option<&'static str>,
    pub icon: Option<&'static str>,
}

impl Default for Bo {
    fn default() -> Self {
        Bo { radius: 10, enabled: true, border: None, sfx: Some("click"), icon: None }
    }
}

impl Bo {
    pub fn r(radius: i32) -> Bo {
        Bo { radius, ..Default::default() }
    }
    pub fn enabled(mut self, e: bool) -> Bo {
        self.enabled = e;
        self
    }
    pub fn border(mut self, c: Option<Color>) -> Bo {
        self.border = c;
        self
    }
    pub fn sfx(mut self, s: Option<&'static str>) -> Bo {
        self.sfx = s;
        self
    }
    pub fn icon(mut self, i: &'static str) -> Bo {
        self.icon = Some(i);
        self
    }
}

pub fn blit_center(canvas: &mut Surface, s: &Surface, c: (i32, i32)) -> Rect {
    let r = Rect::with_center(s.w, s.h, c);
    canvas.blit(s, r.x, r.y);
    r
}

pub fn blit_midtop(canvas: &mut Surface, s: &Surface, c: (i32, i32)) -> Rect {
    let r = Rect::with_midtop(s.w, s.h, c);
    canvas.blit(s, r.x, r.y);
    r
}

pub fn blit_midleft(canvas: &mut Surface, s: &Surface, c: (i32, i32)) -> Rect {
    let r = Rect::with_midleft(s.w, s.h, c);
    canvas.blit(s, r.x, r.y);
    r
}

impl Game {
    pub fn field_mut(&mut self, f: FieldRef) -> &mut TextField {
        match f {
            FieldRef::Acc(k) => self.acc.fields.get_mut(k).expect("account field"),
            FieldRef::FriendsSearch => &mut self.fr.search,
            FieldRef::Chat => &mut self.chat.field,
            FieldRef::Feedback => &mut self.fb.field,
            FieldRef::EventMult => &mut self.ev.mult_field,
            FieldRef::EventSeconds => &mut self.ev.seconds_field,
            FieldRef::BanSearch => &mut self.adm.search,
            FieldRef::BanReason => &mut self.adm.reason,
            FieldRef::SellAmount => &mut self.sell.field,
            FieldRef::SlotName => &mut self.slot_name_field,
        }
    }

    /// ui/widgets.clipboard_text(): first line of the clipboard text, or "".
    pub fn clipboard_text(&self) -> String {
        let Some(w) = self.win.as_ref() else { return String::new() };
        let raw = w.video.clipboard().clipboard_text().unwrap_or_default();
        if raw.trim().is_empty() {
            return String::new();
        }
        raw.replace('\0', "").lines().next().unwrap_or("").to_string()
    }

    /// The editing keys shared by every text box (Backspace/Delete/arrows/Home/End/Ctrl+V).
    /// `vertical`: Up/Down act as Home/End. Returns true if the key was used.
    pub fn edit_field_key(&mut self, field: FieldRef, ev: super::KeyEv, vertical: bool) -> bool {
        use sdl2::keyboard::Keycode as K;
        match ev.key {
            K::Backspace => self.field_mut(field).backspace(),
            K::Delete => self.field_mut(field).delete(),
            K::Left => self.field_mut(field).move_by(-1),
            K::Right => self.field_mut(field).move_by(1),
            K::Home => self.field_mut(field).home(),
            K::End => self.field_mut(field).end(),
            K::Up if vertical => self.field_mut(field).home(),
            K::Down if vertical => self.field_mut(field).end(),
            K::V if ev.ctrl => {
                let t = self.clipboard_text();
                self.field_mut(field).add(&t);
            }
            _ => return false,
        }
        true
    }

    pub fn show_toast(&mut self, text: &str, duration: f64) {
        self.toast_text = Some(text.to_string());
        self.toast_timer = duration;
        self.toast_kind = None;
    }

    pub fn draw_toast(&mut self) {
        let Some(text) = self.toast_text.clone() else { return };
        if self.toast_timer <= 0.0 || text.is_empty() {
            return;
        }
        let lines: Vec<Rc<Surface>> = text.split('\n').map(|p| self.f.med.render(p, WHITE)).collect();
        let line_h = lines[0].h;
        let w = lines.iter().map(|t| t.w).max().unwrap_or(0) + 40;
        let n = lines.len() as i32;
        let h = line_h * n + 4 * (n - 1) + 20;
        let (x0, y0) = (self.vw / 2 - w / 2, TOPBAR_H + 10);
        let mut s = Surface::new_alpha(w, h);
        let r = s.get_rect();
        let p = panel();
        draw::rect(&mut s, Color::rgba(p.r, p.g, p.b, 245), r, 0, 10);
        draw::rect(&mut s, outline(), r, BORDER_W_SMALL, 10);
        for (i, t) in lines.iter().enumerate() {
            s.blit(t, 20, 10 + i as i32 * (line_h + 4));
        }
        self.canvas.blit(&s, x0, y0);
    }

    // ---------------------------------------------------------------- clipping / buttons
    pub fn push_clip(&mut self, rect: Rect) {
        self.clip_stack.push(rect);
        self.canvas.set_clip(Some(rect));
    }

    pub fn pop_clip(&mut self) {
        self.clip_stack.pop();
        let c = self.clip_stack.last().copied();
        self.canvas.set_clip(c);
    }

    pub fn register_button(&mut self, rect: Rect, callback: Cb, sfx: Option<&'static str>) {
        let mut rect = rect;
        if let Some(c) = self.clip_stack.last() {
            rect = rect.clip(c);
            if rect.w <= 0 || rect.h <= 0 {
                return;
            }
        }
        self.buttons.push(Button { rect, cb: callback, sfx, nav: self.nav_mode });
    }

    /// A button that does nothing (clicks inside a page do not close it).
    pub fn register_blocker(&mut self, rect: Rect) {
        self.register_button(rect, Rc::new(|_: &mut Game| {}), None);
    }

    // ---------------------------------------------------------------- text fields
    pub fn draw_text_field(&mut self, rect: Rect, field: FieldRef, display: &str, placeholder: &str, focused: bool, on_focus: Cb) {
        let font = self.f.med.clone();
        draw::rect(&mut self.canvas, Color::rgb(22, 22, 25), rect, 0, 10);
        draw::rect(&mut self.canvas, if focused { accent() } else { outline() }, rect, 3, 10);
        let max_w = rect.w - 28;
        let disp: Vec<char> = display.chars().collect();
        let caret = self.field_mut(field).caret.min(disp.len());
        let width = |sub: &str| -> i32 { if sub.is_empty() { 0 } else { font.render(sub, WHITE).w } };
        let sl = |a: usize, b: usize| -> String { disp[a.min(disp.len())..b.min(disp.len())].iter().collect() };
        let mut start = 0usize;
        while start < caret && width(&sl(start, disp.len())) > max_w {
            start += 1;
        }
        while start > 0 && width(&sl(start - 1, disp.len())) <= max_w {
            start -= 1;
        }
        let mut shown: Vec<char> = disp[start..].to_vec();
        while shown.len() > 1 && width(&shown.iter().collect::<String>()) > max_w {
            shown.pop();
        }
        let txt = if !disp.is_empty() { font.render(&shown.iter().collect::<String>(), WHITE) } else { font.render(placeholder, grey_dim()) };
        let pos = (rect.x + 14, rect.centery() - txt.h / 2);
        self.canvas.blit(&txt, pos.0, pos.1);
        if focused && (self.ticks() / 500) % 2 == 0 {
            let cx = (pos.0 + if !disp.is_empty() { width(&sl(start, caret)) + 1 } else { 0 }).min(rect.right() - 12);
            draw::line(&mut self.canvas, WHITE, (cx, rect.y + 10), (cx, rect.bottom() - 10), 2);
        }
        let d = disp.clone();
        let x0 = pos.0;
        let st = start;
        let fnt = font.clone();
        self.register_button(
            rect,
            Rc::new(move |g: &mut Game| {
                on_focus(g);
                let mx = g.last_click_pos.map(|p| p.0).unwrap_or(x0 as f64);
                let w = |a: usize, b: usize| -> i32 {
                    let s: String = d[a..b].iter().collect();
                    if s.is_empty() { 0 } else { fnt.render(&s, WHITE).w }
                };
                let (mut best, mut best_dx) = (st, (mx - x0 as f64).abs());
                for i in st + 1..=d.len() {
                    let dx = (mx - (x0 + w(st, i)) as f64).abs();
                    if dx > best_dx {
                        break;
                    }
                    best = i;
                    best_dx = dx;
                }
                g.field_mut(field).caret = best;
            }),
            None,
        );
    }

    /// Like wrap_text, but returns (start, end) of each line in the original text (in characters).
    pub fn wrap_indices(text: &[char], font: &Font, max_w: i32) -> Vec<(usize, usize)> {
        let mut lines = Vec::new();
        let (mut start, mut last_space): (usize, i64) = (0, -1);
        let mut i = 0usize;
        while i < text.len() {
            if text[i] == ' ' {
                last_space = i as i64;
            }
            let s: String = text[start..i + 1].iter().collect();
            if font.render(&s, WHITE).w > max_w && i > start {
                let cut = if last_space > start as i64 { last_space as usize + 1 } else { i };
                lines.push((start, cut));
                start = cut;
                last_space = -1;
                continue;
            }
            i += 1;
        }
        lines.push((start, text.len()));
        lines
    }

    pub fn draw_text_area(&mut self, rect: Rect, field: FieldRef, placeholder: &str, focused: bool, on_focus: Cb) {
        let font = self.f.small_b.clone();
        draw::rect(&mut self.canvas, Color::rgb(22, 22, 25), rect, 0, 10);
        draw::rect(&mut self.canvas, if focused { accent() } else { outline() }, rect, 3, 10);
        let (text_s, caret0) = {
            let f = self.field_mut(field);
            (f.text.clone(), f.caret)
        };
        let text: Vec<char> = text_s.chars().collect();
        let max_w = rect.w - 28;
        let line_h = font.get_height() + 4;
        let (x0, y0) = (rect.x + 14, rect.y + 12);
        let caret = caret0.min(text.len());
        if text.is_empty() {
            let ph = font.render(placeholder, grey_dim());
            self.canvas.blit(&ph, x0, y0);
        }
        let spans = Self::wrap_indices(&text, &font, max_w);
        for (n, (a, b)) in spans.iter().enumerate() {
            let y = y0 + n as i32 * line_h;
            if y + line_h > rect.bottom() - 6 {
                break;
            }
            let seg: String = text[*a..*b].iter().collect();
            if !seg.is_empty() {
                let t = font.render(&seg, WHITE);
                self.canvas.blit(&t, x0, y);
            }
            if focused && *a <= caret && caret <= *b && (caret < *b || n == spans.len() - 1 || caret < spans[n + 1].0) && (self.ticks() / 500) % 2 == 0 {
                let pre: String = text[*a..caret].iter().collect();
                let cx = x0 + font.render(&pre, WHITE).w;
                draw::line(&mut self.canvas, WHITE, (cx, y - 1), (cx, y + font.get_height()), 2);
            }
        }
        let sp = spans.clone();
        let t = text.clone();
        self.register_button(
            rect,
            Rc::new(move |g: &mut Game| {
                on_focus(g);
                let Some(pos) = g.last_click_pos else { return };
                let n = (((pos.1 - y0 as f64) / line_h as f64).floor() as i64).clamp(0, sp.len() as i64 - 1) as usize;
                let (a, b) = sp[n];
                let (mut best, mut best_dx) = (a, (pos.0 - x0 as f64).abs());
                for i in a + 1..=b {
                    let s: String = t[a..i].iter().collect();
                    let dx = (pos.0 - (x0 + font.render(&s, WHITE).w) as f64).abs();
                    if dx > best_dx {
                        break;
                    }
                    best = i;
                    best_dx = dx;
                }
                g.field_mut(field).caret = best;
            }),
            None,
        );
    }

    pub fn begin_modal(&mut self) {
        self.buttons.retain(|b| b.nav);
        self.scrollbar_hits.clear();
    }

    pub fn clip_allows(&self, rect: &Rect) -> bool {
        match self.clip_stack.last() {
            None => true,
            Some(c) => c.colliderect(rect),
        }
    }

    #[allow(clippy::too_many_arguments)]
    pub fn button(&mut self, rect: Rect, label: &str, font: &Font, mouse_pos: (f64, f64), base: Color, hover: Color, text: Color, callback: Option<Cb>, o: Bo) -> bool {
        if !self.clip_allows(&rect) {
            return false;
        }
        let hovering = o.enabled && rect.collidepoint(mouse_pos) && self.clip_stack.last().is_none_or(|c| c.collidepoint(mouse_pos));
        let mut color = base;
        if !o.enabled {
            color = super::color_dim(base, 55);
        } else if hovering {
            color = hover;
        }
        let anim = self.animations();
        if hovering && anim && rect.w >= 24 {
            // v3.0: a soft glow in the button's own hover colour
            let g = crate::ui::fx::hover_glow(rect.w, rect.h, o.radius, hover);
            self.canvas.blit(&g, rect.x - 7, rect.y - 7);
        }
        let bx = rounded_box(rect.w, rect.h, color, o.radius, BORDER_W_SMALL);
        self.canvas.blit(&bx, rect.x, rect.y);
        if anim && o.enabled {
            self.draw_button_fx(rect, o.radius, hovering);
        }
        if let Some(bc) = o.border {
            draw_state_border(&mut self.canvas, rect, bc, o.radius, 2);
        }
        if !label.is_empty() || o.icon.is_some() {
            self.draw_button_content(rect, label, font, if o.enabled { text } else { grey_dim() }, o.icon, o.enabled);
        }
        if o.enabled {
            if let Some(c) = callback {
                self.register_button(rect, c, o.sfx);
            }
        }
        hovering
    }

    fn font_list(&self, thin: bool) -> Vec<Font> {
        if thin {
            vec![self.f.small.clone(), self.f.tiny.clone()]
        } else {
            vec![self.f.huge.clone(), self.f.big.clone(), self.f.med.clone(), self.f.small_b.clone(), self.f.tiny_b.clone()]
        }
    }

    /// The fonts smaller than `font` (same style: heavy or thin), biggest first.
    pub fn fonts_menores(&self, font: &Font) -> Vec<Font> {
        let thin = Rc::ptr_eq(font, &self.f.small) || Rc::ptr_eq(font, &self.f.tiny);
        self.font_list(thin).into_iter().filter(|f| f.get_height() < font.get_height()).collect()
    }

    /// Rendered text that fits in max_w: `font`, else smaller fonts, else cut with an ellipsis.
    pub fn texto_que_cabe(&self, label: &str, font: &Font, color: Color, max_w: i32) -> Rc<Surface> {
        let txt = font.render(label, color);
        if txt.w <= max_w {
            return txt;
        }
        let menores = self.fonts_menores(font);
        for f in &menores {
            let t = f.render(label, color);
            if t.w <= max_w {
                return t;
            }
        }
        let ultima = menores.last().cloned().unwrap_or_else(|| font.clone());
        ultima.render(&fit_text(&ultima, label, max_w - 4), color)
    }

    pub fn draw_button_content(&mut self, rect: Rect, label: &str, font: &Font, color: Color, icon: Option<&str>, enabled: bool) {
        let (pad, gap) = (8, 8);
        let img = icon.and_then(|i| load_icon(i, 16.max(rect.h - 12)));
        let mut txt = if !label.is_empty() { Some(font.render(label, color)) } else { None };
        let Some(img) = img else {
            if let Some(mut t) = txt {
                if t.w > rect.w - 2 * pad {
                    t = self.texto_que_cabe(label, font, color, rect.w - 2 * pad);
                }
                blit_center(&mut self.canvas, &t, rect.center());
            }
            return;
        };
        let alpha = if enabled { 255 } else { 110 };
        let Some(t) = txt.take() else {
            let r = Rect::with_center(img.w, img.h, rect.center());
            self.canvas.blit_with_alpha(&img, r.x, r.y, alpha);
            return;
        };
        let icon_left = rect.right() - pad - img.w;
        let (area_l, area_r) = (rect.x + pad, icon_left - gap);
        if area_r - area_l < 24 {
            let total = t.w + gap + img.w;
            let x = rect.centerx() - total / 2;
            blit_midleft(&mut self.canvas, &t, (x, rect.centery()));
            let r = Rect::with_midleft(img.w, img.h, (x + t.w + gap, rect.centery()));
            self.canvas.blit_with_alpha(&img, r.x, r.y, alpha);
            return;
        }
        let t = if t.w > area_r - area_l { self.texto_que_cabe(label, font, color, area_r - area_l) } else { t };
        blit_center(&mut self.canvas, &t, ((area_l + area_r).div_euclid(2), rect.centery()));
        let r = Rect::with_midleft(img.w, img.h, (icon_left, rect.centery()));
        self.canvas.blit_with_alpha(&img, r.x, r.y, alpha);
    }

    // ---------------------------------------------------------------- panel header
    pub fn panel_header(&mut self, rect: Rect, title: &str, mouse_pos: (f64, f64), close_cb: Cb) {
        self.register_blocker(rect);
        draw_panel(&mut self.canvas, rect, Some(panel()), 0, false, Some(BORDER_W));
        let t = self.f.big.render(title, WHITE);
        self.canvas.blit(&t, rect.x + 22, rect.y + 16);
        let close = Rect::new(rect.right() - 44, rect.y + 18, 28, 28);
        let f = self.f.small_b.clone();
        self.button(close, "X", &f, mouse_pos, panel_light(), BAD, WHITE, Some(close_cb), Bo::r(8));
    }

    pub fn draw_scrollbar(&mut self, content: Rect, scroll: f64, content_h: f64, key: Option<&'static str>, mouse_pos: Option<(f64, f64)>) {
        if content_h <= content.h as f64 {
            return;
        }
        let track = Rect::new(content.right() - 12, content.top() + 4, 10, content.h - 8);
        draw::rect(&mut self.canvas, Color::rgb(24, 24, 27), track, 0, 5);
        draw::rect(&mut self.canvas, outline(), track, 2, 5);
        let frac = content.h as f64 / content_h;
        let bar_h = 34.max((track.h as f64 * frac) as i32);
        let max_scroll = content_h - content.h as f64;
        let t = if max_scroll <= 0.0 { 0.0 } else { scroll / max_scroll };
        let bar_y = track.top() + ((track.h - bar_h) as f64 * t) as i32;
        let thumb = Rect::new(track.x, bar_y, track.w, bar_h);
        let dragging = key.is_some() && self.dragging_scrollbar == key;
        let hovering = mouse_pos.is_some_and(|m| self.clip_allows(&track) && track.collidepoint(m));
        let color = if dragging {
            accent_hover()
        } else if hovering {
            accent()
        } else {
            panel_lighter()
        };
        draw::rect(&mut self.canvas, color, thumb, 0, 5);
        draw::rect(&mut self.canvas, outline(), thumb, 2, 5);
        if let Some(k) = key {
            if self.clip_allows(&track) {
                let hit = match self.clip_stack.last() {
                    Some(c) => track.clip(c),
                    None => track,
                };
                if hit.w > 0 && hit.h > 0 {
                    self.scrollbar_hits.insert(k, (track, max_scroll, bar_h));
                }
            }
        }
    }

    // ---------------------------------------------------------------- side buttons
    pub fn draw_icon(&mut self, kind: &str, rect: Rect, color: Color, bg: Option<Color>) {
        let bg = bg.unwrap_or_else(panel_light);
        if let Some(img) = load_icon(kind, rect.w.min(rect.h) - 14) {
            blit_center(&mut self.canvas, &img, rect.center());
            return;
        }
        let (cx, cy) = rect.center();
        let cv = &mut self.canvas;
        match kind {
            "index" => {
                draw::polygon(cv, color, &[(cx - 1, cy - 8), (cx - 14, cy - 12), (cx - 14, cy + 9), (cx - 1, cy + 13)], 0);
                draw::polygon(cv, color, &[(cx + 1, cy - 8), (cx + 14, cy - 12), (cx + 14, cy + 9), (cx + 1, cy + 13)], 0);
                for i in 0..3 {
                    let yy = cy - 5 + i * 5;
                    draw::line(cv, bg, (cx - 11, yy - 1), (cx - 4, yy + 1), 2);
                    draw::line(cv, bg, (cx + 4, yy + 1), (cx + 11, yy - 1), 2);
                }
            }
            "tree" => {
                draw::polygon(cv, color, &[(cx, cy - 15), (cx + 13, cy - 1), (cx - 13, cy - 1)], 0);
                for i in 0..3 {
                    draw::rect(cv, color, Rect::new(cx - 5, cy + 3 + i * 5, 10, 3), 0, 1);
                }
            }
            "bag" => {
                draw::rect(cv, color, Rect::new(cx - 6, cy - 21, 12, 13), 3, 6);
                draw::rect(cv, color, Rect::new(cx - 19, cy + 2, 9, 17), 0, 4);
                draw::rect(cv, color, Rect::new(cx + 10, cy + 2, 9, 17), 0, 4);
                draw::rect(cv, color, Rect::new(cx - 14, cy - 15, 28, 35), 0, 12);
                draw::line(cv, bg, (cx - 14, cy + 3), (cx - 14, cy + 18), 2);
                draw::line(cv, bg, (cx + 13, cy + 3), (cx + 13, cy + 18), 2);
                draw::lines(cv, bg, false, &[(cx - 14, cy - 4), (cx - 1, cy + 1), (cx, cy + 1), (cx + 13, cy - 4)], 2);
                draw::rect(cv, bg, Rect::new(cx - 8, cy + 6, 16, 11), 2, 4);
            }
            "trait" => {
                let mut pts = Vec::new();
                for i in 0..10 {
                    let ang = -std::f64::consts::PI / 2.0 + i as f64 * std::f64::consts::PI / 5.0;
                    let r = if i % 2 == 0 { 12.0 } else { 5.0 };
                    pts.push((ti(cx as f64 + r * ang.cos()), ti(cy as f64 + r * ang.sin())));
                }
                draw::polygon(cv, color, &pts, 0);
            }
            "milestones" => {
                draw::ellipse(cv, color, Rect::new(cx - 17, cy - 11, 10, 11), 3);
                draw::ellipse(cv, color, Rect::new(cx + 7, cy - 11, 10, 11), 3);
                draw::polygon(cv, color, &[(cx - 10, cy - 13), (cx + 10, cy - 13), (cx + 9, cy - 4), (cx + 6, cy + 2), (cx + 2, cy + 4), (cx - 2, cy + 4), (cx - 6, cy + 2), (cx - 9, cy - 4)], 0);
                draw::rect(cv, color, Rect::new(cx - 2, cy + 3, 4, 6), 0, 0);
                draw::rect(cv, color, Rect::new(cx - 8, cy + 9, 16, 4), 0, 2);
            }
            "rebirth" => {
                let arc_rect = Rect::new(cx - 13, cy - 13, 26, 26);
                draw::arc(cv, color, arc_rect, 35f64.to_radians(), 325f64.to_radians(), 4);
                let ang = 35f64.to_radians();
                let tip = (cx as f64 + 13.0 * ang.cos(), cy as f64 - 13.0 * ang.sin());
                draw::polygon(cv, color, &[(ti(tip.0 - 8.0), ti(tip.1 - 3.0)), (ti(tip.0 + 3.0), ti(tip.1 - 9.0)), (ti(tip.0 + 2.0), ti(tip.1 + 5.0))], 0);
            }
            "daily" => {
                draw::rect(cv, color, Rect::new(cx - 14, cy - 11, 28, 23), 3, 4);
                draw::line(cv, color, (cx - 8, cy - 17), (cx - 8, cy - 9), 3);
                draw::line(cv, color, (cx + 8, cy - 17), (cx + 8, cy - 9), 3);
                draw::line(cv, color, (cx - 14, cy - 3), (cx + 14, cy - 3), 2);
                draw::lines(cv, color, false, &[(cx - 6, cy + 4), (cx - 1, cy + 9), (cx + 8, cy - 3)], 3);
            }
            _ => {}
        }
    }

    pub fn draw_badge(&mut self, rect: Rect, count: i64) {
        if count <= 0 {
            return;
        }
        let txt = self.f.small_b.render(&if count < 100 { count.to_string() } else { "99+".into() }, WHITE);
        let h = 26;
        let w = h.max(txt.w + 14);
        let badge = Rect::with_center(w, h, (rect.right() - 3, rect.top() + 3));
        draw::rect(&mut self.canvas, PAUSED_RED, badge, 0, h / 2);
        draw::rect(&mut self.canvas, outline(), badge, BORDER_W_SMALL, h / 2);
        blit_center(&mut self.canvas, &txt, badge.center());
    }

    pub fn draw_alert_mark(&mut self, rect: Rect) {
        let size = 36;
        let center = (rect.right() - 3, rect.top() + 7);
        match load_icon("alert", size) {
            Some(img) => {
                blit_center(&mut self.canvas, &img, center);
            }
            None => {
                draw::circle(&mut self.canvas, PAUSED_RED, center, 13, 0);
                draw::circle(&mut self.canvas, outline(), center, 13, BORDER_W_SMALL);
                let t = self.f.small_b.render("!", WHITE);
                blit_center(&mut self.canvas, &t, center);
            }
        }
    }

    /// v3.0 effects on top of a button: the subtle glare that sweeps across it now and then (faster while hovered)
    /// and the ripple where it was clicked.
    pub fn draw_button_fx(&mut self, rect: Rect, radius: i32, hovering: bool) {
        let t = crate::core::state::now_ts();
        // Potato Mode: skip the sweeping glare (never the text/labels themselves - just this overlay)
        if self.animations() && rect.w >= 40 && rect.h >= 20 {
            let (period, sweep, alpha) = if hovering { (2.4, 0.7, 60) } else { (7.0, 0.9, 34) };
            if let Some(phase) = crate::ui::fx::glare_phase(t, rect, period, sweep) {
                if let Some(g) = crate::ui::fx::glare_band(rect.w, rect.h, radius, phase, alpha) {
                    self.canvas.blit(&g, rect.x, rect.y);
                }
            }
        }
        if let Some((p, t0)) = self.press_fx {
            let k = (t - t0) / 0.35;
            if (0.0..1.0).contains(&k) && rect.collidepoint(p) {
                let at = ((p.0 - rect.x as f64) as i32, (p.1 - rect.y as f64) as i32);
                let r = crate::ui::fx::ripple(rect.w, rect.h, radius, at, k);
                self.canvas.blit(&r, rect.x, rect.y);
            }
        }
    }

    #[allow(clippy::too_many_arguments)]
    pub fn side_button(&mut self, rect: Rect, label: &str, icon: &str, mouse_pos: (f64, f64), active: bool, callback: Cb, label_font: Option<Font>, badge: i64, alert: bool) {
        let hovering = rect.collidepoint(mouse_pos);
        let mut base = if active { accent() } else { panel_light() };
        if hovering && !active {
            base = panel_lighter();
        }
        let anim = self.animations();
        if hovering && anim {
            let g = crate::ui::fx::hover_glow(rect.w, rect.h, 12, if active { accent() } else { accent_hover() });
            self.canvas.blit(&g, rect.x - 7, rect.y - 7);
        }
        draw_panel(&mut self.canvas, rect, Some(base), 12, true, None);
        self.draw_icon(icon, rect, if active { BLACK } else { WHITE }, Some(base));
        if anim {
            self.draw_button_fx(rect, 12, hovering);
        }
        if !label.is_empty() {
            let font = label_font.unwrap_or_else(|| self.f.small_b.clone());
            let t = font.render(label, if hovering || active { accent() } else { WHITE });
            blit_midtop(&mut self.canvas, &t, (rect.centerx(), rect.bottom() + 4));
        }
        self.register_button(rect, callback, Some("click"));
        self.draw_badge(rect, badge);
        if alert {
            self.draw_alert_mark(rect);
        }
    }
}
