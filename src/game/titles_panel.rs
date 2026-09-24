//! Titles: equipping one, publishing it to /titles/{uid} so other players see it, dropping titles you no longer
//! hold (Top 1/2/3), and the title picker inside the Friends page. The list is in core/titles.rs.

use super::base::Bo;
use super::{Game, cb};
use crate::core::titles::{TITLES, TitleDef, title_def, title_hint, title_unlocked};
use crate::gfx::{Color, Rect, draw, ti};
use crate::i18n::tr;
use crate::online::cloud_cache::LB_FIELDS;
use crate::theme::*;
use crate::tr;
use crate::ui::fonts::{Font, fit_text};

/// how often a title that isn't held any more (e.g. out of the top 3) is taken off
const TITLE_CHECK_EVERY: f64 = 5.0;

pub struct TitlesUi {
    pub picker: bool,
    /// (uid, title) last published, so it's only sent when it changes
    pub pushed: Option<(String, Option<String>)>,
    pub pushing: bool,
    pub check_timer: f64,
}

impl TitlesUi {
    pub fn new() -> TitlesUi {
        TitlesUi { picker: false, pushed: None, pushing: false, check_timer: 0.0 }
    }
}

impl Game {
    /// Your best place on any leaderboard tab (1 = first), from the last snapshot. None = not listed / no data.
    pub fn my_top_rank(&self) -> Option<i64> {
        let name = self.account.as_ref()?.username.clone();
        let data = self.lb_data.as_ref()?;
        let mut best: Option<i64> = None;
        for (tab, _) in LB_FIELDS {
            if let Some(rows) = data.get(tab).and_then(|v| v.as_array()) {
                if let Some(pos) = rows.iter().position(|r| r.get("username").and_then(|u| u.as_str()) == Some(name.as_str())) {
                    let rank = pos as i64 + 1;
                    best = Some(best.map_or(rank, |b| b.min(rank)));
                }
            }
        }
        best
    }

    pub fn title_is_unlocked(&self, t: &TitleDef) -> bool {
        title_unlocked(t, &self.state, self.ev.is_admin, self.my_top_rank())
    }

    pub fn equip_title(&mut self, id: Option<&'static str>) {
        self.state.title = id;
        self.state.dirty = true;
        self.titles.picker = false;
        self.play("equip", 0.0);
        self.titles.check_timer = 0.0;
    }

    /// Every frame in the game: publishes the title when it changes (or another account / save opens) and takes
    /// off a title that isn't held any more.
    pub fn tick_titles(&mut self, dt: f64) {
        self.titles.check_timer -= dt;
        if self.titles.check_timer > 0.0 {
            return;
        }
        self.titles.check_timer = TITLE_CHECK_EVERY;
        if let Some(t) = self.state.title.and_then(title_def) {
            // only decide once the facts are known: the admin check came back / there is leaderboard data
            let known = match t.unlock {
                crate::core::titles::Unlock::Admin => self.ev.admin_checked && !self.ev.loading_admin,
                crate::core::titles::Unlock::TopRank(_) => self.lb_data.is_some() && self.account.is_some(),
                _ => true,
            };
            if known && !self.title_is_unlocked(t) {
                self.state.title = None;
                self.state.dirty = true;
                self.show_toast(&tr!("You no longer hold the title \"%s\".", tr(t.name)), 2.2);
            }
        }
        let (Some(client), Some(acc)) = (self.client.clone(), self.account.as_ref()) else { return };
        if self.titles.pushing {
            return;
        }
        let want = (acc.uid.clone(), self.state.title.map(|s| s.to_string()));
        if self.titles.pushed.as_ref() == Some(&want) {
            return;
        }
        self.titles.pushing = true;
        let title = want.1.clone();
        self.run_job(
            move || client.set_title(title.as_deref()),
            move |g, _: ()| {
                g.titles.pushing = false;
                g.titles.pushed = Some(want);
            },
            |g, _| {
                // no network, or the /titles rule isn't published yet: try again later, quietly
                g.titles.pushing = false;
                g.titles.check_timer = 60.0;
            },
        );
    }

    // ================================================================ drawing
    /// A small coloured pill with the title's name. Returns its width (0 = nothing drawn).
    pub fn draw_title_pill(&mut self, x: i32, y: i32, id: &str, font: &Font, max_w: i32) -> i32 {
        let Some(t) = title_def(id) else { return 0 };
        let text = fit_text(font, &tr(t.name), (max_w - 14).max(10));
        let txt = font.render(&text, t.color);
        let r = Rect::new(x, y, txt.w + 14, txt.h + 4);
        draw::rect(&mut self.canvas, Color::rgb(18, 18, 26), r, 0, r.h / 2);
        draw::rect(&mut self.canvas, t.color, r, 2, r.h / 2);
        self.canvas.blit(&txt, x + 7, y + 2);
        r.w
    }

    pub fn open_title_picker(&mut self) {
        self.titles.picker = true;
        self.fr.scroll = 0.0;
        self.fr.msg = None;
        self.set_friends_focus(false);
        self.ensure_leaderboard(false); // the Top 1/2/3 titles need a fresh leaderboard
    }

    /// The title picker (inside the Friends page, like the photo picker).
    pub fn draw_title_picker(&mut self, rect: Rect, mouse_pos: (f64, f64)) {
        let head = self.f.med.render(&tr("Choose a title"), WHITE);
        self.canvas.blit(&head, rect.x + 4, rect.y);
        let sub = self.f.tiny.render(&tr("Every player sees your title next to your name. Unlock more with Milestones and the leaderboards."), grey_dim());
        self.canvas.blit(&sub, rect.x + 4, rect.y + head.h + 2);

        let sb = self.f.small_b.clone();
        let foot_y = rect.bottom() - 40;
        self.button(Rect::new(rect.x, foot_y, 140, 40), &tr("Back"), &sb, mouse_pos, panel_light(), panel_lighter(), WHITE, cb(|g| g.titles.picker = false), Bo::r(10));
        if self.state.title.is_some() {
            self.button(Rect::new(rect.right() - 160, foot_y, 160, 40), &tr("No title"), &sb, mouse_pos, panel_light(), BAD, WHITE, cb(|g| g.equip_title(None)), Bo::r(10));
        }

        let list = Rect::new(rect.x, rect.y + head.h + 24, rect.w, foot_y - 12 - (rect.y + head.h + 24));
        let row_h = 52;
        let gap = 6;
        let scroll = self.fr.scroll;
        let small = self.f.small.clone();
        let tb = self.f.tiny_b.clone();
        self.push_clip(list);
        let mut y = list.y as f64 - scroll;
        for t in TITLES.iter() {
            let row = Rect::new(list.x, ti(y), list.w - 16, row_h);
            y += (row_h + gap) as f64;
            if row.bottom() < list.top() - 4 || row.top() > list.bottom() + 4 {
                continue;
            }
            let unlocked = self.title_is_unlocked(t);
            let equipped = self.state.title == Some(t.id);
            draw::rect(&mut self.canvas, if equipped { panel_lighter() } else { panel_light() }, row, 0, 10);
            draw::rect(&mut self.canvas, if equipped { t.color } else { outline() }, row, 2, 10);
            let pill_w = self.draw_title_pill(row.x + 12, row.y + 8, t.id, &tb, row.w / 2);
            let _ = pill_w;
            let (fmt, n, label) = title_hint(t);
            let hint = match (n, label) {
                (Some(n), _) => tr!(fmt, n),
                (None, Some(l)) => tr!(fmt, tr(l)),
                _ => tr(fmt),
            };
            let ht = small.render(&fit_text(&small, &hint, row.w - 150), if unlocked { grey() } else { grey_dim() });
            self.canvas.blit(&ht, row.x + 12, row.bottom() - ht.h - 6);
            let btn = Rect::new(row.right() - 122, row.centery() - 16, 110, 32);
            let id = t.id;
            if equipped {
                self.button(btn, &tr("EQUIPPED"), &sb, mouse_pos, GOOD, GOOD, BLACK, None, Bo::r(8));
            } else if unlocked {
                self.button(btn, &tr("Equip"), &sb, mouse_pos, accent(), accent_hover(), BLACK, cb(move |g| g.equip_title(Some(id))), Bo::r(8).sfx(None));
            } else {
                self.button(btn, &tr("Locked"), &sb, mouse_pos, panel(), panel(), grey_dim(), None, Bo::r(8).enabled(false));
            }
        }
        self.pop_clip();
        let content_h = (TITLES.len() as i32 * (row_h + gap) - gap) as f64;
        self.fr.max_scroll = (content_h - list.h as f64).max(0.0);
        self.fr.scroll = self.fr.scroll.clamp(0.0, self.fr.max_scroll);
        self.draw_scrollbar(list, scroll, content_h, Some("friends"), Some(mouse_pos));
    }
}
