//! The game: puts every piece together (sound, screens, panels, online) and runs the main loop
//! (main.py). Each former Python mixin is an `impl Game` block in its own file.

pub mod account;
pub mod admin;
pub mod audio;
pub mod base;
pub mod chat_panel;
pub mod cloud;
pub mod credits;
pub mod cutscene;
pub mod daily_panel;
pub mod events;
pub mod feedback;
pub mod friends;
pub mod game_screen;
pub mod gameplay;
pub mod leaderboard;
pub mod milestones_panel;
pub mod options;
pub mod pets_panel;
pub mod rebirth_panel;
pub mod season;
pub mod prestige_panel;
pub mod sell_panel;
pub mod evolve_panel;
pub mod battle_panel;
pub mod battle_online;
pub mod battle3d;
pub mod monster3d;
pub mod tutorial;
pub mod teaser;
pub mod shop_panel;
pub mod title_saves;
pub mod trades;
pub mod titles_panel;
pub mod traits_panel;
pub mod update_panel;
pub mod verity_fx;
pub mod updatelog;
pub mod upgrades_panel;

use crate::audio::{Chunk, Mixer};
use crate::config::{AUTOSAVE_INTERVAL, FPS, GAME_TITLE, VIRTUAL_H, VW_MAX, VW_MIN};
use crate::core::state::{GameState, now_ts};
use crate::gfx::{Color, Rect, Surface, transform};
use crate::i18n::{set_language, tr};
use crate::online::firebase::{Client, OnlineError, Res, Worker};
use crate::storage::{Settings, load_settings, migrate_legacy_save, save_settings};
use crate::theme;
use crate::ui::drawing::{clear_drawing_caches, make_game_background};
use crate::ui::fonts::{Font, clear_font_caches, make_font};
use crate::ui::widgets::{Particle, SlidePanel};
use indexmap::IndexMap;
use sdl2::event::{Event, WindowEvent};
use sdl2::keyboard::{Keycode, Mod};
use std::collections::HashMap;
use std::rc::Rc;
use std::sync::Arc;

pub type Cb = Rc<dyn Fn(&mut Game)>;

pub fn cb(f: impl Fn(&mut Game) + 'static) -> Option<Cb> {
    Some(Rc::new(f))
}

pub struct Button {
    pub rect: Rect,
    pub cb: Cb,
    pub sfx: Option<&'static str>,
    pub nav: bool,
}

pub struct Fonts {
    pub tiny: Font,
    pub tiny_b: Font,
    pub small: Font,
    pub small_b: Font,
    pub med: Font,
    pub big: Font,
    pub huge: Font,
    pub title: Font,
}

#[derive(Clone, Copy)]
pub struct KeyEv {
    pub key: Keycode,
    pub ctrl: bool,
    pub shift: bool,
}

#[derive(Clone, Debug)]
pub struct Account {
    pub uid: String,
    pub username: String,
    pub email: Option<String>,
    pub email_verified: bool,
}

/// The SDL window side (absent in headless screenshot runs).
pub struct Win {
    pub canvas: sdl2::render::WindowCanvas,
    pub creator: sdl2::render::TextureCreator<sdl2::video::WindowContext>,
    pub tex: Option<(sdl2::render::Texture, sdl2::render::Texture, i32)>,
    pub video: sdl2::VideoSubsystem,
}

pub struct Game {
    pub settings: Settings,
    pub theme_check_timer: f64,
    pub mixer: Option<Mixer>,
    pub mixer_ok: bool,
    pub music_ok: bool,
    pub f: Fonts,

    pub state: GameState,
    /// states replaced while a network callback still holds them (see Python closures over `st`)
    pub orphan_states: HashMap<u64, GameState>,
    pub screen_mode: &'static str,
    pub slot_info: HashMap<i64, Option<crate::storage::SlotPeek>>,
    pub delete_confirm_slot: Option<i64>,
    pub delete_confirm_timer: f64,

    pub fullscreen: bool,
    pub win: Option<Win>,
    pub real_size: (i32, i32),
    pub canvas: Surface,
    pub bg_surface: Rc<Surface>,
    pub vw: i32,
    pub scale_x: f64,
    pub scale_y: f64,
    pub right_w: i32,
    pub left_w: i32,

    pub options_open: bool,
    pub stats_open: bool,
    pub credits_open: bool,
    pub update_log_open: bool,
    /// scratch buffers for the Glow effect (gfx/glow.rs)
    pub glow_buf: crate::gfx::glow::GlowBuf,
    /// the first-time tutorial's step (None = not showing)
    pub tutorial_step: Option<usize>,
    /// the "What's new" pop-up after an update
    pub whats_new_open: bool,
    /// the 4.0 teaser's clock (None = not playing)
    pub teaser_t: Option<f64>,
    pub update_log_scroll: f64,
    pub update_log_max_scroll: f64,
    pub update_log_list_rect: Rect,
    pub dragging_slider: Option<&'static str>,
    pub slider_bars: HashMap<&'static str, Rect>,
    pub slider_hits: IndexMap<&'static str, Rect>,
    pub options_tab: &'static str,

    pub cutscene_active: Option<(usize, &'static str)>,
    pub cutscene_elapsed: f64,
    pub cutscene_queue: Vec<(usize, &'static str)>,
    pub cutscene_particles: Vec<Particle>,
    pub cutscene_ambient_accum: f64,

    // reset_ui
    pub right_panel: SlidePanel,
    pub left_panel: SlidePanel,
    pub index_tab: &'static str,
    pub bag_view: &'static str,
    pub inv_sort: &'static str,
    pub inv_high_first: bool,
    pub inv_mut: &'static str,
    pub inv_tier: Option<usize>,
    pub traits_open: bool,
    pub index_open: bool,
    pub index_scroll: f64,
    pub index_max_scroll: f64,
    pub index_list_rect: Rect,
    pub traits_scroll: f64,
    pub traits_max_scroll: f64,
    pub traits_list_rect: Rect,
    pub rebirth_open: bool,
    pub rebirth_confirm: bool,
    pub rebirth_confirm_timer: f64,
    pub rebirth_scroll: f64,
    /// v3.0: "rebirth" or "prestige" (the tabs beside the Rebirth page)
    pub rebirth_tab: &'static str,
    /// v3.0: "daily" or "weekly" (the Quests panel's tabs)
    pub quests_tab: &'static str,
    /// the verity a Prestige keeps (None = your best one)
    pub prestige_keep: Option<crate::core::state::Pet>,
    pub prestige_picking: bool,
    pub rebirth_max_scroll: f64,
    pub rebirth_list_rect: Rect,
    pub milestones_selected_category: Option<&'static str>,
    pub milestones_selected_group: Option<&'static str>,
    pub tree_selected_category: Option<&'static str>,
    pub roll_anim_start: f64,
    pub particles: Vec<Particle>,
    pub auto_accum: f64,
    pub auto_upgrade_timer: f64,
    pub prefs_timer: f64,
    pub auto_trait_timer: f64,
    pub auto_rebirth_timer: f64,
    /// the Auto Roller rolls faster than the card can show until this time (see draw_too_fast_card)
    pub too_fast_until: f64,
    /// the best pet since it got that fast
    pub too_fast_best: Option<crate::core::state::Pet>,
    pub right_rect: Rect,
    pub left_rect: Rect,

    pub toast_text: Option<String>,
    pub toast_timer: f64,
    pub toast_kind: Option<&'static str>,
    pub trait_toast_count: i64,
    /// v3.0.4: the Auto Trait Roller's "New trait!" card (trait, time left, equipped by it)
    pub trait_popup: Option<(usize, f64, bool)>,

    pub frame_dt: f64,
    pub roll_rate: f64,
    pub rate_prev: Option<i64>,
    pub bar_display: HashMap<&'static str, f64>,
    pub bar_continuous: HashMap<&'static str, bool>,

    pub autosave_timer: f64,
    pub buttons: Vec<Button>,
    pub last_click_pos: Option<(f64, f64)>,
    /// v3.0: where and when a button was last clicked (for its ripple)
    pub press_fx: Option<((f64, f64), f64)>,
    /// v3.0 (Cookie Clicker style): verities drifting down behind the roll screen, and the ones popping up
    /// from clicks
    pub vfx: verity_fx::VerityFx,
    /// v3.0: the coins shown in the top bar count up smoothly to the real value
    pub coins_display: Option<f64>,
    pub nav_mode: bool,
    pub clip_stack: Vec<Rect>,
    pub dragging_scrollbar: Option<&'static str>,
    pub scrollbar_hits: IndexMap<&'static str, (Rect, f64, i32)>,

    pub sounds: HashMap<String, Arc<Chunk>>,
    pub sfx_gain: HashMap<String, f64>,
    pub last_sfx: HashMap<String, f64>,
    pub last_manual_roll: Option<f64>,
    pub start_instant: std::time::Instant,
    pub quit_requested: bool,

    // title screen caches
    pub title_shadow: Option<(usize, Rc<Surface>)>,
    pub mascot_pad: Option<(usize, Rc<Surface>)>,
    pub mascot_frames: HashMap<(i64, i64), (Rc<Surface>, (i32, i32))>,
    pub mascot_still: Option<(Rc<Surface>, (i32, i32))>,
    pub mascot_shadows: HashMap<i32, Rc<Surface>>,

    // ---------------- online ----------------
    pub client: Option<Client>,
    pub worker: Worker<Game>,
    pub upd: update_panel::UpdaterUi,
    pub account: Option<Account>,
    pub account_return: &'static str,
    pub acc: account::AccountUi,
    pub cache_info: HashMap<i64, crate::online::cloud_cache::CachePeek>,
    pub cloud_slots: HashMap<i64, crate::online::firebase::SaveSummary>,
    pub cloud_slots_loaded: bool,
    pub cloud_slots_loading: bool,
    pub cloud_slots_error: Option<String>,
    pub cloud_slots_error_msg: String,
    pub slots_retry_at: f64,
    pub slot_starting: bool,
    pub import_busy: bool,
    pub upload_inflight: bool,
    pub sync_timer: f64,
    pub sync_error: Option<String>,
    pub sync_last_ok: Option<f64>,
    pub install_id: String,
    pub session_held: bool,
    pub session_timer: f64,
    pub session_inflight: bool,
    pub pub_inflight: bool,
    pub pub_last_time: f64,
    pub pub_retry_at: f64,
    /// the /leaderboard rules refused the "prestige" field (not published yet): send without it
    pub lb_no_prestige: bool,
    pub pub_jitter: f64,
    pub leaderboard_open: bool,
    pub lb_tab: &'static str,
    pub lb_scroll: f64,
    pub lb_max_scroll: f64,
    pub lb_list_rect: Rect,
    pub lb_data: Option<serde_json::Value>,
    pub lb_loading: bool,
    pub lb_error: Option<String>,
    pub lb_retry_at: f64,
    pub lb_refetch_done: f64,
    pub fr: friends::FriendsUi,
    pub fb: feedback::FeedbackUi,
    pub chat: chat_panel::ChatUi,
    pub ev: events::EventsUi,
    pub adm: admin::AdminUi,
    pub trades: trades::TradesUi,
    pub shop: shop_panel::ShopUi,
    pub sell: sell_panel::SellUi,
    pub evolve: evolve_panel::EvolveUi,
    pub battle: battle_panel::BattleUi,
    pub titles: titles_panel::TitlesUi,
    /// v3.0.1: season resets (see season.rs)
    pub season: season::SeasonUi,
    pub text_input_on: bool,
}

impl Game {
    pub fn new(sdl: Option<&sdl2::Sdl>) -> Game {
        let settings = load_settings();
        set_language(&settings.get_str("language", "en"));
        theme::set_dark(theme::resolve_dark(&settings.get_str("theme_mode", theme::DEFAULT_THEME_MODE)));

        let mixer = sdl.and_then(|s| s.audio().ok()).and_then(|a| Mixer::open(&a));
        let mixer_ok = mixer.is_some();

        let f = Fonts {
            tiny: make_font(13, 1, false),
            tiny_b: make_font(12, 1, true),
            small: make_font(15, 1, false),
            small_b: make_font(15, 1, true),
            med: make_font(19, 2, true),
            big: make_font(27, 2, true),
            huge: make_font(40, 3, true),
            title: make_font(96, 5, true),
        };

        migrate_legacy_save();
        let fullscreen = settings.get_bool("fullscreen", true);
        let (key, pid) = crate::online::firebase::load_firebase_config();
        let client = if !key.is_empty() && !pid.is_empty() { Some(Arc::new(crate::online::firebase::FirebaseClient::new(&key, &pid))) } else { None };

        let mut g = Game {
            settings,
            theme_check_timer: 0.0,
            mixer,
            mixer_ok,
            music_ok: false,
            f,
            state: GameState::new(),
            orphan_states: HashMap::new(),
            screen_mode: "title",
            slot_info: HashMap::new(),
            delete_confirm_slot: None,
            delete_confirm_timer: 0.0,
            fullscreen,
            win: None,
            real_size: (1422, 800),
            canvas: Surface::new(1422, VIRTUAL_H),
            bg_surface: Rc::new(Surface::new(1, 1)),
            vw: 1422,
            scale_x: 1.0,
            scale_y: 1.0,
            right_w: 0,
            left_w: 0,
            options_open: false,
            stats_open: false,
            credits_open: false,
            update_log_open: false,
            glow_buf: Default::default(),
            tutorial_step: None,
            whats_new_open: false,
            teaser_t: None,
            update_log_scroll: 0.0,
            update_log_max_scroll: 0.0,
            update_log_list_rect: Rect::ZERO,
            dragging_slider: None,
            slider_bars: HashMap::new(),
            slider_hits: IndexMap::new(),
            options_tab: "gameplay",
            cutscene_active: None,
            cutscene_elapsed: 0.0,
            cutscene_queue: Vec::new(),
            cutscene_particles: Vec::new(),
            cutscene_ambient_accum: 0.0,
            right_panel: SlidePanel::new("right"),
            left_panel: SlidePanel::new("left"),
            index_tab: "normal",
            bag_view: "equipped",
            inv_sort: "money",
            inv_high_first: true,
            inv_mut: "all",
            inv_tier: None,
            traits_open: false,
            index_open: false,
            index_scroll: 0.0,
            index_max_scroll: 0.0,
            index_list_rect: Rect::ZERO,
            traits_scroll: 0.0,
            traits_max_scroll: 0.0,
            traits_list_rect: Rect::ZERO,
            rebirth_open: false,
            rebirth_confirm: false,
            rebirth_confirm_timer: 0.0,
            rebirth_scroll: 0.0,
            rebirth_tab: "rebirth",
            quests_tab: "daily",
            prestige_keep: None,
            prestige_picking: false,
            rebirth_max_scroll: 0.0,
            rebirth_list_rect: Rect::ZERO,
            milestones_selected_category: None,
            milestones_selected_group: None,
            tree_selected_category: None,
            roll_anim_start: 0.0,
            particles: Vec::new(),
            auto_accum: 0.0,
            auto_upgrade_timer: 0.0,
            prefs_timer: 0.0,
            auto_trait_timer: 0.0,
            auto_rebirth_timer: 0.0,
            too_fast_until: 0.0,
            too_fast_best: None,
            right_rect: Rect::ZERO,
            left_rect: Rect::ZERO,
            toast_text: None,
            toast_timer: 0.0,
            toast_kind: None,
            trait_toast_count: 0,
            trait_popup: None,
            frame_dt: 1.0 / FPS as f64,
            roll_rate: 0.0,
            rate_prev: None,
            bar_display: HashMap::new(),
            bar_continuous: HashMap::new(),
            autosave_timer: 0.0,
            buttons: Vec::new(),
            last_click_pos: None,
            press_fx: None,
            vfx: verity_fx::VerityFx::default(),
            coins_display: None,
            nav_mode: false,
            clip_stack: Vec::new(),
            dragging_scrollbar: None,
            scrollbar_hits: IndexMap::new(),
            sounds: HashMap::new(),
            sfx_gain: HashMap::new(),
            last_sfx: HashMap::new(),
            last_manual_roll: None,
            start_instant: std::time::Instant::now(),
            quit_requested: false,
            title_shadow: None,
            mascot_pad: None,
            mascot_frames: HashMap::new(),
            mascot_still: None,
            mascot_shadows: HashMap::new(),
            client,
            worker: Worker::new(),
            upd: Default::default(),
            account: None,
            account_return: "title",
            acc: account::AccountUi::new(),
            cache_info: HashMap::new(),
            cloud_slots: HashMap::new(),
            cloud_slots_loaded: false,
            cloud_slots_loading: false,
            cloud_slots_error: None,
            cloud_slots_error_msg: String::new(),
            slots_retry_at: 0.0,
            slot_starting: false,
            import_busy: false,
            upload_inflight: false,
            sync_timer: 0.0,
            sync_error: None,
            sync_last_ok: None,
            install_id: crate::online::cloud_cache::install_id(),
            session_held: false,
            session_timer: 0.0,
            session_inflight: false,
            pub_inflight: false,
            pub_last_time: 0.0,
            pub_retry_at: 0.0,
            lb_no_prestige: false,
            pub_jitter: crate::core::state::rand_uniform(0.0, crate::online::firebase::LEADERBOARD_PUBLISH_JITTER),
            leaderboard_open: false,
            lb_tab: "money",
            lb_scroll: 0.0,
            lb_max_scroll: 0.0,
            lb_list_rect: Rect::ZERO,
            lb_data: crate::online::cloud_cache::load_lb_cache(),
            lb_loading: false,
            lb_error: None,
            lb_retry_at: 0.0,
            lb_refetch_done: 0.0,
            fr: friends::FriendsUi::new(),
            fb: feedback::FeedbackUi::new(),
            chat: chat_panel::ChatUi::new(),
            ev: events::EventsUi::new(),
            adm: admin::AdminUi::new(),
            trades: trades::TradesUi::new(),
            shop: shop_panel::ShopUi::new(),
            sell: sell_panel::SellUi::new(),
            evolve: evolve_panel::EvolveUi::new(),
            battle: battle_panel::BattleUi::new(),
            titles: titles_panel::TitlesUi::new(),
            season: Default::default(),
            text_input_on: true,
        };
        if let Some(sdl) = sdl {
            g.open_window(sdl);
        }
        g.set_fullscreen(fullscreen, false);
        g.reset_ui();
        g.build_sounds();
        g.start_music();
        g.apply_volume();
        g.restore_session();
        g.init_updater(); // checks for new versions (see update_panel.rs)
        g
    }

    /// Headless game with a fixed virtual width (screenshot tests).
    pub fn headless(real_w: i32, real_h: i32) -> Game {
        let mut g = Game::new(None);
        g.real_size = (real_w, real_h);
        g.recompute_layout();
        g
    }

    pub fn animations(&self) -> bool {
        self.settings.get_bool("animations", true)
    }

    /// The Glow effect (Options > Interface), on by default.
    pub fn glow_on(&self) -> bool {
        self.settings.get_bool("glow", true)
    }

    /// Bright parts of the finished frame bleed a soft light (only when Glow is on).
    pub fn apply_glow(&mut self) {
        if self.glow_on() {
            crate::gfx::glow::apply(&mut self.canvas, &mut self.glow_buf, 1.3);
        }
    }

    /// pygame.time.get_ticks()
    pub fn ticks(&self) -> i64 {
        if crate::core::state::fake_time_active() {
            return 0;
        }
        self.start_instant.elapsed().as_millis() as i64
    }

    pub fn reset_ui(&mut self) {
        self.right_panel = SlidePanel::new("right");
        self.left_panel = SlidePanel::new("left");
        self.index_tab = "normal";
        self.bag_view = "equipped";
        self.inv_sort = "money";
        self.inv_high_first = true;
        self.inv_mut = "all";
        self.inv_tier = None;
        self.traits_open = false;
        self.index_open = false;
        self.index_scroll = 0.0;
        self.index_max_scroll = 0.0;
        self.index_list_rect = Rect::ZERO;
        self.traits_scroll = 0.0;
        self.traits_max_scroll = 0.0;
        self.traits_list_rect = Rect::ZERO;
        self.rebirth_open = false;
        self.rebirth_confirm = false;
        self.rebirth_confirm_timer = 0.0;
        self.rebirth_scroll = 0.0;
        self.rebirth_max_scroll = 0.0;
        self.rebirth_list_rect = Rect::ZERO;
        self.rebirth_tab = "rebirth";
        self.prestige_keep = None;
        self.prestige_picking = false;
        self.milestones_selected_category = None;
        self.milestones_selected_group = None;
        self.tree_selected_category = None;
        self.roll_anim_start = 0.0;
        self.particles = Vec::new();
        self.auto_accum = 0.0;
        self.auto_upgrade_timer = 0.0;
        self.right_rect = Rect::ZERO;
        self.left_rect = Rect::ZERO;
    }

    /// Replaces the current save state (keeping the old one alive for network callbacks that hold it).
    pub fn replace_state(&mut self, st: GameState) {
        let old = std::mem::replace(&mut self.state, st);
        if self.upload_inflight {
            self.orphan_states.insert(old.obj_id, old);
        }
        self.apply_save_prefs();
        self.enforce_season_state(); // a save from before a reset of everyone starts again from 0
    }

    /// v3.0: the settings kept in the save (from any PC) become this PC's settings. Fullscreen stays per PC.
    pub fn apply_save_prefs(&mut self) {
        let Some(prefs) = self.state.prefs.clone() else { return };
        let mut changed = false;
        for (k, v) in prefs {
            if k == "fullscreen" || !self.settings.map.contains_key(&k) {
                continue;
            }
            if self.settings.map.get(&k) != Some(&v) {
                self.settings.map.insert(k, v);
                changed = true;
            }
        }
        if changed {
            save_settings(&self.settings);
            set_language(&self.settings.get_str("language", "en"));
            theme::set_dark(theme::resolve_dark(&self.settings.get_str("theme_mode", theme::DEFAULT_THEME_MODE)));
            self.apply_music_volume();
        }
    }

    /// The other way: this PC's settings go into the save (and so to the cloud) when they change.
    pub fn store_save_prefs(&mut self) {
        let mut prefs = self.settings.map.clone();
        prefs.remove("fullscreen");
        if self.state.prefs.as_ref() != Some(&prefs) {
            self.state.prefs = Some(prefs);
            self.state.dirty = true;
        }
    }

    pub fn state_by_id(&mut self, id: u64) -> Option<&mut GameState> {
        if self.state.obj_id == id {
            return Some(&mut self.state);
        }
        self.orphan_states.get_mut(&id)
    }

    // ---------------------------------------------------------------- network jobs
    pub fn run_job<T: Send + 'static>(
        &mut self,
        job: impl FnOnce() -> Res<T> + Send + 'static,
        ok: impl FnOnce(&mut Game, T) + 'static,
        err: impl FnOnce(&mut Game, OnlineError) + 'static,
    ) {
        self.worker.run(job, Some(Box::new(ok)), Some(Box::new(err)));
    }

    pub fn run_job_quiet<T: Send + 'static>(&mut self, job: impl FnOnce() -> Res<T> + Send + 'static) {
        self.worker.run::<T>(job, None, None);
    }

    pub fn poll_worker(&mut self) {
        for cb in self.worker.take_done() {
            cb(self);
        }
        if !self.upload_inflight {
            self.orphan_states.clear();
        }
    }

    // ---------------------------------------------------------------- screen
    fn open_window(&mut self, sdl: &sdl2::Sdl) {
        let Ok(video) = sdl.video() else { return };
        let _ = crate::ui::icons::load_icon("verity", 64);
        let Ok(mut window) = video.window(GAME_TITLE, 1280, 720).resizable().position_centered().build() else { return };
        if let Some(icon) = crate::assets::read("icons/verity.png").and_then(|b| crate::gfx::load_png_bytes(&b)) {
            let small = transform::smoothscale(&icon, 64, 64);
            let mut bytes: Vec<u8> = Vec::with_capacity(small.px.len() * 4);
            for p in &small.px {
                bytes.extend_from_slice(&p.to_le_bytes());
            }
            if let Ok(s) = sdl2::surface::Surface::from_data(&mut bytes, 64, 64, 256, sdl2::pixels::PixelFormatEnum::ARGB8888) {
                window.set_icon(s);
            }
        }
        let canvas = match window.into_canvas().accelerated().build() {
            Ok(c) => c,
            Err(_) => return,
        };
        let creator = canvas.texture_creator();
        sdl2::hint::set("SDL_MOUSE_FOCUS_CLICKTHROUGH", "1");
        video.text_input().start();
        self.win = Some(Win { canvas, creator, tex: None, video });
    }

    pub fn set_fullscreen(&mut self, fs: bool, announce: bool) {
        let _ = announce;
        let mut fs = fs;
        if let Some(w) = self.win.as_mut() {
            let window = w.canvas.window_mut();
            if fs {
                if window.set_fullscreen(sdl2::video::FullscreenType::Desktop).is_err() {
                    fs = false;
                }
            } else {
                let _ = window.set_fullscreen(sdl2::video::FullscreenType::Off);
                let screen_w = w.video.current_display_mode(0).map(|m| m.w).unwrap_or(1280);
                let ww = (1500.0f64).min((1000.0f64).max(screen_w as f64 * 0.8)) as i32;
                let hh = (ww as f64 * 9.0 / 16.0) as i32;
                let _ = window.set_size(ww as u32, hh as u32);
                window.set_position(sdl2::video::WindowPos::Centered, sdl2::video::WindowPos::Centered);
            }
            let (ww, hh) = window.size();
            self.real_size = (ww as i32, hh as i32);
        }
        self.fullscreen = fs;
        self.settings.set_bool("fullscreen", fs);
        self.recompute_layout();
    }

    pub fn toggle_fullscreen(&mut self) {
        let fs = !self.fullscreen;
        self.set_fullscreen(fs, true);
        save_settings(&self.settings);
        self.show_toast(&if self.fullscreen { tr("Fullscreen on") } else { tr("Windowed mode") }, 1.8);
    }

    pub fn recompute_layout(&mut self) {
        let real_w = self.real_size.0.max(320);
        let real_h = self.real_size.1.max(240);
        let vw = crate::core::formatting::py_round(VIRTUAL_H as f64 * real_w as f64 / real_h as f64) as i32;
        self.vw = vw.clamp(VW_MIN, VW_MAX);
        self.canvas = Surface::new(self.vw, VIRTUAL_H);
        self.bg_surface = Rc::new(make_game_background(self.vw, VIRTUAL_H, theme::bg_top(), theme::bg_bottom(), theme::dark_mode()));
        self.scale_x = real_w as f64 / self.vw as f64;
        self.scale_y = real_h as f64 / VIRTUAL_H as f64;
        self.right_w = (470.0f64).min(self.vw as f64 * 0.34) as i32;
        self.left_w = (370.0f64).min(self.vw as f64 * 0.28) as i32;
        if let Some(w) = self.win.as_mut() {
            if let Some((a, b, _)) = w.tex.take() {
                unsafe {
                    a.destroy();
                    b.destroy();
                }
            }
        }
    }

    // ---------------------------------------------------------------- theme
    pub fn theme_mode_label(&self) -> String {
        match self.settings.get_str("theme_mode", theme::DEFAULT_THEME_MODE).as_str() {
            "dark" => tr("Dark"),
            "light" => tr("Light"),
            _ => tr(theme::SYSTEM_LABEL),
        }
    }

    pub fn apply_dark(&mut self, dark: bool) {
        if dark == theme::dark_mode() {
            return;
        }
        theme::set_dark(dark);
        clear_font_caches();
        clear_drawing_caches();
        crate::ui::cards::clear_card_cache();
        self.title_shadow = None;
        self.recompute_layout();
    }

    pub fn set_theme_mode(&mut self, mode: &str, announce: bool) {
        let mode = if theme::THEME_MODES.contains(&mode) { mode } else { theme::DEFAULT_THEME_MODE };
        self.settings.set_str("theme_mode", mode);
        self.theme_check_timer = 0.0;
        self.apply_dark(theme::resolve_dark(mode));
        save_settings(&self.settings);
        if announce {
            let t = crate::tr!("Theme: %s", self.theme_mode_label());
            self.show_toast(&t, 1.8);
        }
    }

    pub fn cycle_theme_mode(&mut self) {
        let modes = theme::THEME_MODES;
        let cur = self.settings.get_str("theme_mode", theme::DEFAULT_THEME_MODE);
        let next = match modes.iter().position(|m| *m == cur) {
            Some(i) => modes[(i + 1) % modes.len()],
            None => modes[0],
        };
        self.set_theme_mode(next, true);
    }

    pub fn screen_to_canvas(&self, pos: (i32, i32)) -> (f64, f64) {
        (pos.0 as f64 / self.scale_x, pos.1 as f64 / self.scale_y)
    }

    pub fn mouse_canvas(&self, mouse: (i32, i32)) -> (f64, f64) {
        self.screen_to_canvas(mouse)
    }

    // ---------------------------------------------------------------- draggable scrollbars
    pub fn scroll_from_track(&self, key: &str, canvas_y: f64) -> Option<f64> {
        let (track, max_scroll, bar_h) = *self.scrollbar_hits.get(key)?;
        let span = track.h - bar_h;
        if span <= 0 || max_scroll <= 0.0 {
            return Some(0.0);
        }
        let frac = (canvas_y - track.top() as f64 - bar_h as f64 / 2.0) / span as f64;
        Some((frac * max_scroll).min(max_scroll).max(0.0))
    }

    pub fn apply_scroll(&mut self, key: &str, value: f64) {
        let clamp = |v: f64, m: f64| v.min(m).max(0.0);
        match key {
            "right" => self.right_panel.set_scroll_abs(value),
            "left" => self.left_panel.set_scroll_abs(value),
            "shop" => self.shop.scroll = clamp(value, self.shop.max_scroll),
            "index" => self.index_scroll = clamp(value, self.index_max_scroll),
            "traits" => self.traits_scroll = clamp(value, self.traits_max_scroll),
            "rebirth" => self.rebirth_scroll = clamp(value, self.rebirth_max_scroll),
            "leaderboard" => self.lb_scroll = clamp(value, self.lb_max_scroll),
            "friends" => self.fr.scroll = clamp(value, self.fr.max_scroll),
            "feedback" => self.fb.scroll = clamp(value, self.fb.max_scroll),
            "update_log" => self.update_log_scroll = clamp(value, self.update_log_max_scroll),
            "chat" => self.chat.scroll = clamp(self.chat.max_scroll - value, self.chat.max_scroll),
            _ => {}
        }
    }

    pub fn start_scrollbar_drag(&mut self, pos: (f64, f64)) -> bool {
        let hit = self.scrollbar_hits.iter().find(|(_, (t, _, _))| t.collidepoint(pos)).map(|(k, _)| *k);
        if let Some(key) = hit {
            self.dragging_scrollbar = Some(key);
            if let Some(v) = self.scroll_from_track(key, pos.1) {
                self.apply_scroll(key, v);
            }
            return true;
        }
        false
    }

    pub fn close_overlays(&mut self) {
        if self.sell.target.is_some() {
            self.close_sell();
        }
        self.close_evolve();
        self.adm.menu_open = false;
        if self.adm.ban_open {
            self.close_ban_admin();
        }
        if self.options_open {
            self.close_options();
        }
        self.stats_open = false;
        self.credits_open = false;
        self.update_log_open = false;
        if self.fb.open {
            self.close_feedback();
        }
        if self.ev.admin_open {
            self.close_event_admin();
        }
        self.traits_open = false;
        self.index_open = false;
        self.shop.open = false;
        self.rebirth_open = false;
        self.rebirth_confirm = false;
        self.leaderboard_open = false;
        if self.fr.open {
            self.close_friends();
        }
    }

    pub fn close_overlay_on_outside_click(&mut self) {
        if self.sell.target.is_some() {
            self.close_sell();
        } else if self.evolve.target.is_some() {
            self.close_evolve();
        } else if self.adm.menu_open {
            self.close_admin_menu();
        } else if self.adm.ban_open {
            self.close_ban_admin();
        } else if self.ev.admin_open {
            self.close_event_admin();
        } else if self.fr.open {
            self.close_friends();
        } else if self.fb.open {
            self.close_feedback();
        } else if self.leaderboard_open {
            self.close_leaderboard();
        } else if self.options_open {
            self.close_options();
        } else if self.stats_open {
            self.close_stats();
        } else if self.credits_open {
            self.close_credits();
        } else if self.update_log_open {
            self.close_update_log();
        } else if self.traits_open && self.screen_mode == "game" {
            self.close_traits();
        } else if self.index_open && self.screen_mode == "game" {
            self.close_index_page();
        } else if self.shop.open && self.screen_mode == "game" {
            self.close_shop();
        } else if self.rebirth_open && self.screen_mode == "game" {
            self.close_rebirth();
        } else if self.left_panel.is_open() && self.screen_mode == "game" {
            self.left_panel.close(); // the Bag page
        } else {
            return;
        }
        self.play("click", 0.0);
    }

    // ---------------------------------------------------------------- main loop
    /// One frame of game logic (after the events).
    pub fn tick(&mut self, dt: f64) {
        if self.screen_mode == "game" {
            let gain = self.state.income_per_second() * dt;
            self.state.coins += gain;
            self.state.total_coins_earned += gain;
            self.state.playtime += dt;
            self.state.last_seen = Some(now_ts());
            self.update_auto(dt);
            self.update_auto_upgrade(dt);
            self.update_auto_trait(dt);
            self.update_auto_rebirth(dt);
            self.state.tick_potions(dt); // active potions only use up time with the game open
            self.update_cutscenes(dt);
            self.update_roll_rate(dt);
            self.check_milestones();
            self.state.ensure_daily_missions();
            self.right_panel.update(dt);
            self.left_panel.update(dt);
            self.autosave_timer += dt;
            self.prefs_timer += dt;
            if self.prefs_timer >= 1.0 {
                self.prefs_timer = 0.0;
                self.store_save_prefs();
            }
            if self.autosave_timer >= AUTOSAVE_INTERVAL {
                self.autosave_timer = 0.0;
                self.state.save();
            }
            self.tick_online(dt);
            self.tick_titles(dt);
        } else {
            self.rate_prev = None;
            self.roll_rate = 0.0;
        }
        if self.delete_confirm_slot.is_some() {
            self.delete_confirm_timer -= dt;
            if self.delete_confirm_timer <= 0.0 {
                self.delete_confirm_slot = None;
            }
        }
        self.tick_sell(dt);
        self.tick_battle(dt);
        self.tick_teaser(dt);
        if self.rebirth_confirm {
            self.rebirth_confirm_timer -= dt;
            if self.rebirth_confirm_timer <= 0.0 {
                self.rebirth_confirm = false;
            }
        }
        self.update_animations(dt);
        self.frame_dt = dt;
    }

    pub fn tick_theme(&mut self, dt: f64) {
        // (the Python only follows the system theme live on Windows)
        let _ = dt;
    }

    pub fn run(&mut self, sdl: &sdl2::Sdl) {
        let mut pump = sdl.event_pump().expect("event pump");
        let frame = std::time::Duration::from_secs_f64(1.0 / FPS as f64);
        let mut last = std::time::Instant::now();
        let mut running = true;
        while running {
            // clock.tick(FPS)
            let elapsed = last.elapsed();
            if elapsed < frame {
                std::thread::sleep(frame - elapsed);
            }
            let now = std::time::Instant::now();
            let dt = now.duration_since(last).as_secs_f64().min(0.1);
            last = now;
            self.poll_worker();
            self.tick_updater(); // now and then checks GitHub for a new version
            self.tick_ban(now_ts()); // was this account banned? (see admin.rs)
            self.tick_season(dt); // did an admin reset everyone's progress? (see season.rs)
            self.tick_theme(dt);
            running = self.handle_events(&mut pump);
            if self.quit_requested {
                break;
            }
            self.tick(dt);
            let ms = pump.mouse_state();
            let mouse = self.mouse_canvas((ms.x(), ms.y()));
            self.draw(mouse);
            self.apply_glow();
            self.present();
        }
        self.save_everything();
    }

    /// pump.poll_iter(), minus the event types the sdl2 crate cannot decode (sdl2-compat on SDL3
    /// sends some newer ones, and decoding them aborts the process).
    fn poll_events_safe(_pump: &mut sdl2::EventPump) -> Vec<Event> {
        let mut out = Vec::new();
        loop {
            let mut raw = std::mem::MaybeUninit::<sdl2::sys::SDL_Event>::uninit();
            if unsafe { sdl2::sys::SDL_PollEvent(raw.as_mut_ptr()) } == 0 {
                break;
            }
            let raw = unsafe { raw.assume_init() };
            let ty = unsafe { raw.type_ };
            let known = matches!(ty, 0x100..=0x107 | 0x200 | 0x300..=0x303 | 0x400..=0x403 | 0x700..=0x702 | 0x900 | 0x1000..=0x1001);
            if known {
                out.push(Event::from_ll(raw));
            }
        }
        out
    }

    fn handle_events(&mut self, pump: &mut sdl2::EventPump) -> bool {
        let events: Vec<Event> = Self::poll_events_safe(pump);
        for event in events {
            match event {
                Event::Quit { .. } => return false,
                Event::Window { win_event: WindowEvent::Resized(w, h) | WindowEvent::SizeChanged(w, h), .. } => {
                    if (w, h) != self.real_size {
                        self.real_size = (w, h);
                        self.recompute_layout();
                    }
                }
                // pygame drops SDL's auto-repeat KEYDOWNs (no key.set_repeat); without this, holding a key
                // (or a key-up lost while Wayland switches fullscreen) re-triggers it: F11 flickers forever
                Event::KeyDown { repeat: true, .. } => {}
                Event::KeyDown { keycode: Some(key), keymod, .. } => {
                    let ctrl = keymod.intersects(Mod::LCTRLMOD | Mod::RCTRLMOD | Mod::LGUIMOD | Mod::RGUIMOD);
                    let shift = keymod.intersects(Mod::LSHIFTMOD | Mod::RSHIFTMOD);
                    self.on_key(KeyEv { key, ctrl, shift });
                    if self.quit_requested {
                        return false;
                    }
                }
                Event::TextInput { text, .. } => self.on_text(&text),
                Event::MouseWheel { y, precise_y, .. } => {
                    let _ = precise_y;
                    let (mx, my) = (pump.mouse_state().x(), pump.mouse_state().y());
                    let pos = self.mouse_canvas((mx, my));
                    self.scroll_at(pos, -(y as f64) * 60.0);
                }
                Event::MouseMotion { x, y, .. } => {
                    if let Some(s) = self.dragging_slider {
                        let p = self.screen_to_canvas((x, y));
                        self.set_slider_from_pos(s, p);
                    } else if let Some(key) = self.dragging_scrollbar {
                        let p = self.screen_to_canvas((x, y));
                        match self.scroll_from_track(key, p.1) {
                            None => self.dragging_scrollbar = None,
                            Some(v) => self.apply_scroll(key, v),
                        }
                    }
                }
                Event::MouseButtonUp { mouse_btn: sdl2::mouse::MouseButton::Left, .. } => {
                    if self.dragging_slider.is_some() {
                        self.dragging_slider = None;
                        save_settings(&self.settings);
                        self.play("click", 0.0);
                    }
                    self.dragging_scrollbar = None;
                }
                Event::MouseButtonDown { mouse_btn: sdl2::mouse::MouseButton::Left, x, y, .. } => {
                    let p = self.screen_to_canvas((x, y));
                    self.mouse_down(p);
                    if self.quit_requested {
                        return false;
                    }
                }
                _ => {}
            }
        }
        true
    }

    pub fn mouse_down(&mut self, p: (f64, f64)) {
        if self.options_open && !self.update_modal_active() && self.cutscene_active.is_none() {
            let hit = self.slider_hits.iter().find(|(_, r)| r.collidepoint(p)).map(|(k, _)| *k);
            if let Some(h) = hit {
                self.dragging_slider = Some(h);
                self.set_slider_from_pos(h, p);
                return;
            }
        }
        if self.start_scrollbar_drag(p) {
            return;
        }
        self.dispatch_click(p);
    }

    pub fn on_text(&mut self, text: &str) {
        if self.ban_screen_active() {
        } else if self.adm.ban_open && self.adm.focus.is_some() && !self.update_modal_active() {
            self.ban_type(text);
        } else if self.sell.target.is_some() && self.sell.focus && !self.update_modal_active() {
            self.sell_type(text);
        } else if self.fr.open && self.fr.focus && !self.update_modal_active() {
            self.fr.search.add(text);
        } else if self.chat.uid.is_some() && self.chat.focus && !self.update_modal_active() {
            self.chat.field.add(text);
        } else if self.fb.open && self.fb.focus && !self.update_modal_active() {
            self.fb.field.add(text);
        } else if self.ev.admin_open && self.ev.admin_focus.is_some() && !self.update_modal_active() {
            if self.ev.admin_focus == Some("mult") {
                self.ev.mult_field.add(text);
            } else {
                self.ev.seconds_field.add(text);
            }
        } else if self.screen_mode == "account" && !self.update_modal_active() {
            if let Some(f) = self.acc.focus {
                if let Some(fl) = self.acc.fields.get_mut(f) {
                    fl.add(text);
                }
            }
        }
    }

    pub fn on_key(&mut self, ev: KeyEv) {
        let k = ev.key;
        if (self.update_modal_active() || self.ban_screen_active()) && k != Keycode::F11 {
        } else if self.cutscene_active.is_some() && k != Keycode::F11 {
            self.skip_cutscene();
        } else if self.screen_mode == "account" && self.handle_account_key(ev) {
        } else if self.screen_mode == "game" && (self.handle_teaser_key(ev) || self.handle_tutorial_key(ev) || self.handle_whats_new_key(ev) || self.handle_battle_key(ev)) {
        } else if self.handle_sell_key(ev)
            || self.handle_evolve_key(ev)
            || self.handle_ban_key(ev)
            || self.handle_chat_key(ev)
            || self.handle_friends_key(ev)
            || self.handle_feedback_key(ev)
            || self.handle_event_admin_key(ev)
        {
        } else if k == Keycode::F11 || (k == Keycode::F && ev.ctrl) {
            self.toggle_fullscreen();
        } else if k == Keycode::Escape || k == Keycode::AcBack {
            if self.screen_mode == "account" {
                self.close_account();
            } else if self.adm.menu_open {
                self.close_admin_menu();
            } else if self.ev.admin_open {
                self.close_event_admin();
            } else if self.fr.open {
                self.close_friends();
            } else if self.fb.open {
                self.close_feedback();
            } else if self.leaderboard_open {
                self.close_leaderboard();
            } else if self.options_open {
                self.close_options();
            } else if self.stats_open {
                self.close_stats();
            } else if self.credits_open {
                self.close_credits();
            } else if self.update_log_open {
                self.close_update_log();
            } else if self.screen_mode == "saves" {
                self.back_to_title();
            } else if self.screen_mode == "title" {
            } else if self.traits_open {
                self.traits_open = false;
            } else if self.index_open {
                self.close_index_page();
            } else if self.shop.open {
                self.close_shop();
            } else if self.rebirth_open {
                self.close_rebirth();
            } else if self.right_panel.is_open() {
                self.right_panel.close();
            } else if self.left_panel.is_open() {
                self.left_panel.close();
            }
        } else if self.options_open
            || self.stats_open
            || self.traits_open
            || self.index_open
            || self.shop.open
            || self.rebirth_open
            || self.leaderboard_open
            || self.credits_open
            || self.update_log_open
            || self.fr.open
            || self.fb.open
            || self.ev.admin_open
            || self.adm.menu_open
            || self.adm.ban_open
            || self.sell.target.is_some()
            || self.evolve.target.is_some()
            || self.tutorial_active()
            || self.whats_new_open
            || self.battle.open
            || self.screen_mode != "game"
        {
        } else if k == Keycode::U || k == Keycode::T {
            self.open_right_panel("tree");
        } else if k == Keycode::I {
            self.toggle_index_page();
        } else if k == Keycode::B {
            self.open_left_panel("bag");
        } else if k == Keycode::M {
            self.open_right_panel("milestones");
        }
    }

    pub fn dispatch_click(&mut self, pos: (f64, f64)) -> bool {
        self.last_click_pos = Some(pos);
        let hit = self.buttons.iter().rev().find(|b| b.rect.collidepoint(pos)).map(|b| (b.cb.clone(), b.sfx));
        if let Some((cbk, sfx)) = hit {
            self.press_fx = Some((pos, now_ts()));
            if let Some(s) = sfx {
                self.play(s, 0.0);
            }
            cbk(self);
            return true;
        }
        if !self.update_modal_active() {
            self.close_overlay_on_outside_click();
        }
        false
    }

    pub fn scroll_at(&mut self, pos: (f64, f64), step: f64) -> bool {
        let clamp = |v: f64, m: f64| v.min(m).max(0.0);
        if self.ban_screen_active() || self.adm.menu_open {
            return false;
        }
        if self.adm.ban_open {
            if self.adm.list_rect.collidepoint(pos) {
                self.adm.scroll = clamp(self.adm.scroll + step, self.adm.max_scroll);
                return true;
            }
            return false;
        }
        if self.fb.open {
            if self.fb.list_rect.collidepoint(pos) {
                self.fb.scroll = clamp(self.fb.scroll + step, self.fb.max_scroll);
                return true;
            }
            return false;
        }
        if self.fr.open {
            if self.chat.uid.is_some() {
                if self.chat.list_rect.collidepoint(pos) {
                    self.chat.scroll = clamp(self.chat.scroll - step, self.chat.max_scroll);
                    return true;
                }
                return false;
            }
            if self.fr.list_rect.collidepoint(pos) || self.fr.avatar_picker || self.titles.picker || self.trades.target.is_some() {
                self.fr.scroll = clamp(self.fr.scroll + step, self.fr.max_scroll);
                return true;
            }
            return false;
        }
        if self.update_log_open {
            if self.update_log_list_rect.collidepoint(pos) {
                self.update_log_scroll = clamp(self.update_log_scroll + step, self.update_log_max_scroll);
                return true;
            }
            return false;
        }
        if self.screen_mode != "game"
            || self.options_open
            || self.stats_open
            || self.sell.target.is_some()
            || self.evolve.target.is_some()
            || self.tutorial_active()
            || self.whats_new_open
            || self.battle.open
            || self.leaderboard_open
            || self.credits_open
            || self.update_modal_active()
            || self.cutscene_active.is_some()
        {
            return false;
        }
        if self.shop.open {
            if self.shop.list_rect.collidepoint(pos) {
                self.shop.scroll = clamp(self.shop.scroll + step, self.shop.max_scroll);
                return true;
            }
            return false;
        }
        if self.index_open {
            if self.index_list_rect.collidepoint(pos) {
                self.index_scroll = clamp(self.index_scroll + step, self.index_max_scroll);
                return true;
            }
            return false;
        }
        if self.traits_open && self.traits_list_rect.collidepoint(pos) {
            self.traits_scroll = clamp(self.traits_scroll + step, self.traits_max_scroll);
        } else if self.rebirth_open && self.rebirth_list_rect.collidepoint(pos) {
            self.rebirth_scroll = clamp(self.rebirth_scroll + step, self.rebirth_max_scroll);
        } else if self.right_panel.visible() && self.right_rect.collidepoint(pos) {
            self.right_panel.add_scroll(step);
        } else if self.left_panel.visible() && self.left_rect.collidepoint(pos) {
            self.left_panel.add_scroll(step);
        } else {
            return false;
        }
        true
    }

    pub fn draw(&mut self, mouse_pos: (f64, f64)) {
        let bg = self.bg_surface.clone();
        self.canvas.set_clip(None);
        self.canvas.blit(&bg, 0, 0);
        self.buttons.clear();
        self.nav_mode = false;
        self.clip_stack.clear();
        self.scrollbar_hits.clear();
        self.canvas.set_clip(None);
        match self.screen_mode {
            "title" => {
                self.draw_title(mouse_pos);
                self.draw_toast();
            }
            "saves" => {
                self.draw_saves(mouse_pos);
                self.draw_toast();
            }
            "account" => {
                self.draw_account(mouse_pos);
                self.draw_toast();
            }
            _ => self.draw_game_screen(mouse_pos),
        }
        if self.stats_open {
            self.begin_modal();
            self.draw_stats(mouse_pos);
        }
        if self.options_open {
            self.begin_modal();
            self.draw_options(mouse_pos);
        }
        if self.fr.open {
            self.begin_modal();
            self.draw_friends(mouse_pos);
        }
        if self.leaderboard_open {
            self.begin_modal();
            self.draw_leaderboard(mouse_pos);
        }
        if self.credits_open {
            self.begin_modal();
            self.draw_credits(mouse_pos);
        }
        if self.update_log_open {
            self.begin_modal();
            self.draw_update_log(mouse_pos);
        }
        if self.fb.open {
            self.begin_modal();
            self.draw_feedback(mouse_pos);
        }
        if self.ev.admin_open {
            self.begin_modal();
            self.draw_event_admin(mouse_pos);
        }
        if self.adm.menu_open {
            self.begin_modal();
            self.draw_admin_menu(mouse_pos);
        }
        if self.adm.ban_open {
            self.begin_modal();
            self.draw_ban_admin(mouse_pos);
        }
        if self.sell.target.is_some() {
            self.begin_modal();
            self.draw_sell_page(mouse_pos);
        }
        if self.evolve.target.is_some() {
            self.begin_modal();
            self.draw_evolve_page(mouse_pos);
        }
        if self.battle.open {
            self.draw_battle(mouse_pos);
        }
        if self.whats_new_open && !self.tutorial_active() {
            self.draw_whats_new(mouse_pos);
        }
        if self.tutorial_active() {
            self.draw_tutorial(mouse_pos);
        }
        if self.cutscene_active.is_some() {
            self.draw_cutscene(mouse_pos);
        }
        if self.ban_screen_active() && !self.update_modal_active() {
            // banned account: on top of everything, it can only log out or quit
            self.draw_ban_screen(mouse_pos);
        }
        if self.update_modal_active() {
            // "New version available": on top of EVERYTHING, blocks the rest
            self.draw_update_modal(mouse_pos);
        }
    }

    /// Scales the canvas to the window (linear filtering with animations on, like smoothscale; nearest
    /// otherwise, like transform.scale) and shows it.
    fn present(&mut self) {
        let anim = self.animations();
        let (vw, vh) = (self.vw, VIRTUAL_H);
        let Some(w) = self.win.as_mut() else { return };
        if w.tex.as_ref().map(|t| t.2) != Some(vw) {
            if let Some((a, b, _)) = w.tex.take() {
                unsafe {
                    a.destroy();
                    b.destroy();
                }
            }
            sdl2::hint::set("SDL_RENDER_SCALE_QUALITY", "1");
            let lin = w.creator.create_texture_streaming(sdl2::pixels::PixelFormatEnum::ARGB8888, vw as u32, vh as u32);
            sdl2::hint::set("SDL_RENDER_SCALE_QUALITY", "0");
            let near = w.creator.create_texture_streaming(sdl2::pixels::PixelFormatEnum::ARGB8888, vw as u32, vh as u32);
            match (lin, near) {
                (Ok(a), Ok(b)) => w.tex = Some((a, b, vw)),
                _ => return,
            }
        }
        let Some((lin, near, _)) = w.tex.as_mut() else { return };
        let tex = if anim { lin } else { near };
        let px = &self.canvas.px;
        let bytes: &[u8] = unsafe { std::slice::from_raw_parts(px.as_ptr() as *const u8, px.len() * 4) };
        let _ = tex.update(None, bytes, (vw * 4) as usize);
        w.canvas.set_draw_color(sdl2::pixels::Color::RGB(0, 0, 0));
        w.canvas.clear();
        let _ = w.canvas.copy(tex, None, None);
        w.canvas.present();
    }

    pub fn save_everything(&mut self) {
        self.flush_cloud_blocking();
        self.state.save();
        save_settings(&self.settings);
    }

    pub fn quit_game(&mut self) {
        self.save_everything();
        std::process::exit(0);
    }

    // ---------------------------------------------------------------- text input (keyboard on phones)
    pub fn start_text_input(&mut self) {
        if let Some(w) = self.win.as_ref() {
            w.video.text_input().start();
        }
        self.text_input_on = true;
    }
    pub fn stop_text_input(&mut self) {
        // desktop pygame keeps text input on; nothing to do
    }
}

pub fn color_dim(c: Color, d: i32) -> Color {
    Color::rgb((c.r as i32 - d).max(0) as u8, (c.g as i32 - d).max(0) as u8, (c.b as i32 - d).max(0) as u8)
}

#[cfg(test)]
mod prefs_tests {
    use super::*;

    /// One temporary save folder for the whole test run (the save folder is read once).
    fn test_save_dir() -> std::path::PathBuf {
        static DIR: std::sync::OnceLock<std::path::PathBuf> = std::sync::OnceLock::new();
        DIR.get_or_init(|| {
            let dir = std::env::temp_dir().join(format!("lv-game-test-{}", std::process::id()));
            // SAFETY: set once, before anything reads the save folder
            unsafe { std::env::set_var("LUCKY_VERITIES_SAVE_DIR", &dir) };
            dir
        })
        .clone()
    }

    #[test]
    fn a_new_season_wipes_only_older_saves() {
        test_save_dir();
        let mut g = Game::headless(1422, 800);
        g.season.server = Some(1);
        // the save being played, from before the reset: back to 0, same slot, settings kept
        let mut st = GameState::new();
        st.slot = Some(1);
        st.total_rolls = 5000;
        st.coins = 1e9;
        st.rebirths = 7;
        st.playtime = 3600.0;
        let mut p = serde_json::Map::new();
        p.insert("animations".into(), serde_json::json!(false));
        st.prefs = Some(p.clone());
        g.replace_state(st);
        assert_eq!((g.state.total_rolls, g.state.coins, g.state.rebirths, g.state.season), (0, 0.0, 0, 1));
        assert_eq!((g.state.slot, g.state.prefs.clone()), (Some(1), Some(p)));
        assert_eq!(g.state.playtime, 3600.0); // the playtime stays
        // progress made after the reset is never touched
        let mut st = GameState::new();
        st.slot = Some(2);
        st.season = 1;
        st.total_rolls = 42;
        g.replace_state(st);
        assert_eq!(g.state.total_rolls, 42);
        // the other slots on this PC: the old one goes, the new-season one stays
        let mut old = GameState::new();
        old.slot = Some(2);
        old.total_rolls = 99;
        old.save();
        let mut new = GameState::new();
        new.slot = Some(3);
        new.season = 1;
        new.total_rolls = 7;
        new.save();
        g.replace_state(GameState::new()); // playing nothing
        g.on_season_known();
        assert!(!crate::storage::save_slot_path(2).exists());
        assert!(crate::storage::save_slot_path(3).exists());
        // no season yet (0): nothing happens
        let mut g2 = Game::headless(1422, 800);
        g2.season.server = Some(0);
        let mut st = GameState::new();
        st.slot = Some(1);
        st.total_rolls = 10;
        g2.replace_state(st);
        assert_eq!(g2.state.total_rolls, 10);
    }

    #[test]
    fn a_personal_reset_wipes_only_that_accounts_saves() {
        test_save_dir();
        let mut g = Game::headless(1422, 800);
        g.account = Some(Account { uid: "u1".into(), username: "tommy".into(), email: None, email_verified: false });
        g.season.server = Some(0);
        g.season.my_reset = Some(1);
        let cloud = |uid: &str, rolls: i64, reset: i64| {
            let mut st = GameState::new();
            st.slot = Some(1);
            st.cloud_uid = Some(uid.into());
            st.total_rolls = rolls;
            st.player_reset = reset;
            st
        };
        g.replace_state(cloud("u1", 800, 0)); // this account's save from before the reset: wiped
        assert_eq!((g.state.total_rolls, g.state.player_reset, g.state.cloud_uid.as_deref()), (0, 1, Some("u1")));
        g.replace_state(cloud("u1", 30, 1)); // made after the reset: kept
        assert_eq!(g.state.total_rolls, 30);
        let mut offline = GameState::new(); // an offline save on this PC: not this account's, kept
        offline.slot = Some(2);
        offline.total_rolls = 55;
        g.replace_state(offline);
        assert_eq!(g.state.total_rolls, 55);
    }

    #[test]
    fn settings_follow_the_save_to_another_pc() {
        let dir = test_save_dir();
        // PC 1: turns animations and Secret cutscenes off; the settings go into the save
        let mut pc1 = Game::headless(1422, 800);
        pc1.settings.set_bool("animations", false);
        pc1.settings.set_bool("cutscenes_secreto", false);
        pc1.settings.set_bool("fullscreen", false);
        pc1.store_save_prefs();
        let save = pc1.state.to_dict();
        // PC 2: default settings, loads that save (as if from the cloud)
        let mut pc2 = Game::headless(1422, 800);
        pc2.settings = Settings::defaults();
        let mut st = GameState::new();
        st.load_dict(&save).unwrap();
        pc2.replace_state(st);
        assert!(!pc2.settings.get_bool("animations", true));
        assert!(!pc2.settings.get_bool("cutscenes_secreto", true));
        assert!(pc2.settings.get_bool("fullscreen", false)); // fullscreen stays per PC
        let _ = dir;
    }
}
