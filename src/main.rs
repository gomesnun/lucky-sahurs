// no console window behind the game on Windows (the PyInstaller build used --windowed)
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod assets;
mod audio;
mod config;
mod core;
mod game;
mod gfx;
mod i18n;
mod i18n_pt;
mod online;
mod pyfmt;
mod pyjson;
mod pyrand;
mod storage;
mod theme;
mod ui;

use gfx::{Color, Rect, Surface, draw, transform};

fn dump(s: &Surface, path: &str) {
    let mut out = Vec::with_capacity(s.px.len() * 4 + 8);
    out.extend_from_slice(&(s.w as u32).to_le_bytes());
    out.extend_from_slice(&(s.h as u32).to_le_bytes());
    for p in &s.px {
        let c = Color::from_argb(*p);
        out.extend_from_slice(&[c.r, c.g, c.b, if s.alpha { c.a } else { 255 }]);
    }
    std::fs::write(path, out).unwrap();
}

fn save_png(s: &Surface, path: &std::path::Path) {
    let mut img = image::RgbaImage::new(s.w as u32, s.h as u32);
    for (i, p) in s.px.iter().enumerate() {
        let c = Color::from_argb(*p);
        img.as_mut()[i * 4..i * 4 + 4].copy_from_slice(&[c.r, c.g, c.b, if s.alpha { c.a } else { 255 }]);
    }
    img.save(path).unwrap();
}

fn selftest(dir: &str) {
    // 1: primitives on an opaque surface
    let mut s = Surface::new(300, 200);
    s.fill(Color::rgb(30, 40, 50), None);
    draw::rect(&mut s, Color::rgb(200, 100, 50), Rect::new(10, 10, 80, 50), 0, 12);
    draw::rect(&mut s, Color::rgb(8, 8, 10), Rect::new(10, 10, 80, 50), 3, 12);
    draw::rect(&mut s, Color::rgb(255, 205, 60), Rect::new(100, 10, 60, 40), 4, 0);
    draw::rect(&mut s, Color::rgb(90, 200, 90), Rect::new(170, 10, 40, 40), 2, 20);
    draw::circle(&mut s, Color::rgb(250, 250, 250), (50, 120), 30, 0);
    draw::circle(&mut s, Color::rgb(250, 50, 50), (120, 120), 25, 4);
    draw::circle(&mut s, Color::rgb(50, 50, 250), (180, 120), 20, 1);
    draw::line(&mut s, Color::rgb(255, 255, 0), (10, 180), (290, 150), 3);
    draw::line(&mut s, Color::rgb(0, 255, 255), (220, 20), (290, 90), 1);
    draw::polygon(&mut s, Color::rgb(160, 60, 220), &[(220, 100), (290, 110), (270, 170), (230, 160)], 0);
    draw::polygon(&mut s, Color::rgb(60, 160, 220), &[(200, 180), (240, 195), (210, 199)], 0);
    draw::ellipse(&mut s, Color::rgb(0, 0, 0), Rect::new(240, 20, 50, 26), 0);
    draw::ellipse(&mut s, Color::rgb(255, 128, 0), Rect::new(10, 160, 60, 30), 3);
    draw::arc(&mut s, Color::rgb(255, 255, 255), Rect::new(130, 150, 60, 40), 0.3, 2.8, 4);
    dump(&s, &format!("{dir}/t1.bin"));

    // 2: alpha surfaces, blits, blend modes, smoothscale, rotozoom
    let mut a = Surface::new_alpha(120, 90);
    draw::rect(&mut a, Color::rgba(200, 60, 60, 180), Rect::new(0, 0, 120, 90), 0, 16);
    draw::circle(&mut a, Color::rgba(20, 220, 90, 90), (60, 45), 30, 0);
    let mut mask = Surface::new_alpha(120, 90);
    draw::rect(&mut mask, Color::rgba(255, 255, 255, 255), Rect::new(0, 0, 120, 90), 0, 30);
    a.blit_ex(&mask, 0, 0, None, gfx::BLEND_RGBA_MIN);
    let mut base = Surface::new(300, 200);
    base.fill(Color::rgb(240, 235, 225), None);
    base.blit(&a, 10, 10);
    let mut b = a.clone();
    b.set_alpha(120);
    base.blit(&b, 150, 20);
    let sm = transform::smoothscale(&a, 57, 41);
    base.blit(&sm, 20, 120);
    let big = transform::smoothscale(&a, 150, 70);
    base.blit(&big, 140, 120);
    let rz = transform::rotozoom(&a, 17.0, 0.6);
    base.blit(&rz, 90, 100);
    let mut m2 = a.clone();
    m2.fill_blend(Color::rgba(24, 26, 44, 255), gfx::BLEND_RGB_MULT);
    base.blit(&m2, 200, 5);
    let mut sa = Surface::new_alpha(100, 60);
    sa.blit(&a, -10, -10);
    sa.blit(&m2, 20, 15);
    base.blit(&sa, 5, 5);
    dump(&base, &format!("{dir}/t2.bin"));
    dump(&sa, &format!("{dir}/t2b.bin"));

    // 3: text
    let f = gfx::text::RawFont::open_static(assets::embedded("fonts/Fredoka-Bold.ttf").unwrap(), 19).unwrap();
    let t = f.render("Roll pets, equip them! $1.23M", Color::rgb(255, 184, 48));
    println!("size {:?} h {} render {}x{}", f.size("Roll pets, equip them! $1.23M"), f.get_height(), t.w, t.h);
    dump(&t, &format!("{dir}/t3.bin"));
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    if args.len() > 2 && args[1] == "--selftest" {
        selftest(&args[2]);
        return;
    }
    if args.len() > 3 && args[1] == "--statetest" {
        statetest(args[2].parse().unwrap(), args[3].parse().unwrap());
        return;
    }
    if args.len() > 2 && args[1] == "--shots" {
        shots(&args[2]);
        return;
    }
    if args.len() > 1 && args[1] == "--nettest" {
        // read-only: the public leaderboards
        let (key, pid) = online::firebase::load_firebase_config();
        let client = std::sync::Arc::new(online::firebase::FirebaseClient::new(&key, &pid));
        let period = online::cloud_cache::lb_fetch_period(core::state::now_ts());
        match online::cloud_cache::fetch_leaderboards(&client, period) {
            Ok(v) => {
                for (tab, _) in online::cloud_cache::LB_FIELDS {
                    println!("{}: {} entries", tab, v.get(tab).and_then(|a| a.as_array()).map(|a| a.len()).unwrap_or(0));
                }
            }
            Err(e) => println!("error: {:?}", e),
        }
        return;
    }
    if args.len() > 1 && args[1] == "--updatetest" {
        // the updater without the window: check, and with "install" also download, swap and relaunch
        println!("version {} enabled={} asset={}", config::BUILD_VERSION, online::updater::updates_enabled(), online::updater::asset_name());
        match online::updater::fetch_latest() {
            Ok(Some(info)) => {
                println!("new version {} ({} bytes)", info.tag, info.size);
                if args.get(2).map(|a| a.as_str()) == Some("install") {
                    let r = online::updater::prepare(&info, &None).and_then(|plan| online::updater::launch(&plan));
                    println!("install: {:?}", r);
                }
            }
            Ok(None) => println!("up to date"),
            Err(e) => println!("error: {:?}", e),
        }
        return;
    }
    if args.len() > 1 && args[1] == "--bench" {
        bench();
        return;
    }
    if args.len() > 2 && args[1] == "--sim" {
        sim(&args[2]);
        return;
    }
    // desktop shortcut (1st time, release builds only): on its own thread so it never delays the start
    std::thread::spawn(online::shortcut::create_desktop_shortcut_once);
    let sdl = sdl2::init().expect("SDL init failed");
    let mut g = game::Game::new(Some(&sdl));
    g.run(&sdl);
}

/// Milliseconds per draw() of a few screens: `--bench`.
fn bench() {
    use core::state::{RNG, set_fake_time};
    set_fake_time(Some(1_790_000_000.0));
    RNG.with(|r| *r.borrow_mut() = pyrand::PyRandom::from_int(7));
    let mut g = game::Game::headless(1422, 800);
    let m = (-100.0, -100.0);
    let mut run = |g: &mut game::Game, name: &str| {
        for _ in 0..5 {
            g.draw(m);
        }
        let n = 200;
        let s = std::time::Instant::now();
        for _ in 0..n {
            g.draw(m);
        }
        println!("{:<14} {:.3} ms", name, s.elapsed().as_secs_f64() * 1000.0 / n as f64);
    };
    run(&mut g, "title");
    g.start_slot(1);
    for _ in 0..4000 {
        g.state.roll();
    }
    g.state.equip_best();
    run(&mut g, "game");
    g.right_panel.open("index");
    g.right_panel.update(5.0);
    run(&mut g, "index");
    g.right_panel.close();
    g.right_panel.update(5.0);
    g.left_panel.open("bag");
    g.left_panel.update(5.0);
    run(&mut g, "bag");
    g.left_panel.close();
    g.left_panel.update(5.0);
    g.toggle_traits();
    run(&mut g, "traits");
    g.close_overlays();
    let s = std::time::Instant::now();
    let bg = g.bg_surface.clone();
    for _ in 0..200 {
        g.canvas.blit(&bg, 0, 0);
    }
    println!("{:<14} {:.3} ms", "bg blit", s.elapsed().as_secs_f64() * 1000.0 / 200.0);
    let t = |name: &str, f: &mut dyn FnMut()| {
        let s = std::time::Instant::now();
        for _ in 0..200 {
            f();
        }
        println!("{:<14} {:.3} ms", name, s.elapsed().as_secs_f64() * 1000.0 / 200.0);
    };
    let ov = ui::drawing::dim_overlay(g.vw, config::VIRTUAL_H, 170);
    t("dim overlay", &mut || g.canvas.blit(&ov, 0, 0));
    t("panel 1080x700", &mut || ui::drawing::draw_panel(&mut g.canvas, Rect::new(100, 80, 1080, 700), Some(theme::panel()), 16, true, None));
    let f = g.f.small.clone();
    t("text x50", &mut || {
        for i in 0..50 {
            let s = f.render("Every pet roll has a chance", theme::WHITE);
            g.canvas.blit(&s, 10, 10 + i);
        }
    });
    t("trait cards", &mut || {
        for i in 0..12 {
            let s = g.trait_card(i, 360, 92, true, false);
            g.canvas.blit(&s, 100, 100 + i as i32 * 50);
        }
    });
}

/// The scripted input of `--sim` (tools/sim_reference.py has the same list).
fn sim_script() -> Vec<(u32, Option<(f64, f64)>, Option<sdl2::keyboard::Keycode>)> {
    use sdl2::keyboard::Keycode;
    let mut s = vec![(3, Some((710.0, 392.0)), None), (8, Some((359.0, 447.0)), None)];
    s.extend((20..1220).step_by(4).map(|f| (f, Some((711.0, 572.0)), None)));
    s.push((1230, Some((1373.0, 337.0)), None));
    s.push((1300, Some((1185.0, 170.0)), None));
    s.extend((1320..1400).step_by(5).map(|f| (f, Some((1185.0, 311.0)), None)));
    s.extend((1420..1800).step_by(4).map(|f| (f, Some((476.0, 572.0)), None)));
    s.push((1810, None, Some(Keycode::Escape)));
    s.push((1830, Some((49.0, 287.0)), None));
    s.push((1880, Some((185.0, 203.0)), None));
    s.push((1890, Some((185.0, 161.0)), None));
    s.push((1900, None, Some(Keycode::Escape)));
    s.push((2400, Some((1373.0, 337.0)), None));
    s.push((2460, Some((1185.0, 170.0)), None));
    s.extend((2500..3500).step_by(30).map(|f| (f, Some((1185.0, 311.0)), None)));
    s.push((3550, None, Some(Keycode::Escape)));
    s.push((3560, Some((1327.0, 35.0)), None));
    s.push((3570, Some((709.0, 277.0)), None));
    s.push((3580, Some((600.0, 350.0)), None));
    s.push((3600, None, Some(Keycode::Escape)));
    s
}

/// Scripted play (clicks, keys, 1/60 s frames on a fake clock), then the save as JSON: `--sim <dir>`.
fn sim(dir: &str) {
    use core::state::{RNG, set_fake_time};
    let t0 = 1_790_000_000.0;
    set_fake_time(Some(t0));
    RNG.with(|r| *r.borrow_mut() = pyrand::PyRandom::from_int(11));
    let mut g = game::Game::headless(1422, 800);
    let sdl = sdl2::init().expect("SDL init failed");
    g.mixer = sdl.audio().ok().and_then(|a| audio::Mixer::open(&a));
    g.mixer_ok = g.mixer.is_some();
    g.build_sounds();
    let script = sim_script();
    let dt = 1.0 / 60.0;
    let m = (-100.0, -100.0);
    let mut t = t0;
    for frame in 0..3700u32 {
        t += dt;
        set_fake_time(Some(t));
        g.poll_worker();
        g.tick_updater();
        g.tick_theme(dt);
        for (_, click, key) in script.iter().filter(|s| s.0 == frame) {
            if let Some(p) = click {
                g.mouse_down(*p);
            }
            if let Some(k) = key {
                g.on_key(game::KeyEv { key: *k, ctrl: false, shift: false });
            }
        }
        g.tick(dt);
        g.draw(m);
        if frame == 1400 {
            save_png(&g.canvas, &std::path::Path::new(dir).join("sim_1400.png"));
        }
    }
    save_png(&g.canvas, &std::path::Path::new(dir).join("sim_end.png"));
    std::fs::write(std::path::Path::new(dir).join("sim_state.json"), pyjson::dumps(&g.state.to_dict())).unwrap();
}

/// Headless screenshots of the main screens (no window): `--shots <dir>`.
fn shots(dir: &str) {
    use core::state::set_fake_time;
    set_fake_time(Some(1_790_000_000.0));
    let mut g = game::Game::headless(1422, 800);
    // the audio device (SDL_AUDIODRIVER=dummy in tests), as the real game opens it
    let sdl = sdl2::init().expect("SDL init failed");
    g.mixer = sdl.audio().ok().and_then(|a| audio::Mixer::open(&a));
    g.mixer_ok = g.mixer.is_some();
    g.build_sounds();
    let save = |g: &game::Game, name: &str| {
        let p = std::path::Path::new(dir).join(format!("{}.png", name));
        save_png(&g.canvas, &p);
    };
    let m = (-100.0, -100.0);
    g.draw(m);
    save(&g, "title");
    for (name, f) in [
        ("options", game::Game::toggle_options as fn(&mut game::Game)),
        ("credits", game::Game::toggle_credits),
        ("updatelog", game::Game::toggle_update_log),
        ("leaderboard", game::Game::toggle_leaderboard),
        ("feedback", game::Game::toggle_feedback),
    ] {
        f(&mut g);
        g.draw(m);
        save(&g, name);
        g.close_overlays();
        g.leaderboard_open = false;
    }
    g.open_account();
    g.draw(m);
    save(&g, "account");
    g.close_account();
    // the "New version available" screen, in each phase
    g.upd.info = Some(online::updater::ReleaseInfo {
        tag: "v2.5.0".into(),
        name: "x".into(),
        url: String::new(),
        size: 1,
        digest: String::new(),
        page: String::new(),
    });
    for (phase, prog, err) in [
        ("available", 0.0, "other"),
        ("downloading", 0.42, "other"),
        ("installing", 1.0, "other"),
        ("error", 0.0, "net"),
        ("error", 0.0, "perm"),
        ("error", 0.0, "other"),
    ] {
        g.upd.phase = phase;
        *g.upd.progress.lock().unwrap() = prog;
        g.upd.error = err;
        g.upd.error_detail = "boom".into();
        g.draw(m);
        save(&g, &if phase == "error" { format!("upd_{phase}_{err}") } else { format!("upd_{phase}") });
    }
    g.upd.phase = "idle";

    // ---- in game: a deterministic save built by rolling
    use core::data::{TIER_TRANSCENDENT, tier_first_pet, upgrade_defs};
    use core::state::RNG;
    g.start_slot(1);
    RNG.with(|r| *r.borrow_mut() = pyrand::PyRandom::from_int(7));
    for _ in 0..4000 {
        g.state.roll();
    }
    g.state.coins = 5e8;
    for u in upgrade_defs() {
        g.state.buy_upgrade_bulk(u.key, 0);
    }
    for _ in 0..3000 {
        g.state.roll();
    }
    while g.state.trait_charges > 0 {
        g.state.roll_trait();
    }
    if let Some(t) = g.state.last_trait_roll {
        g.state.equip_trait(t);
    }
    g.state.equip_best();
    g.check_milestones();
    g.toast_timer = 0.0;
    g.state.coins = 1e9;
    g.draw(m);
    save(&g, "g_main");
    let right: [(&str, &str, Option<&str>); 7] = [
        ("g_index", "index", None),
        ("g_index_golden", "index", Some("golden")),
        ("g_tree", "tree", None),
        ("g_tree_luck", "tree", Some("luck")),
        ("g_milestones", "milestones", None),
        ("g_milestones_rolls", "milestones", Some("rolls")),
        ("g_daily", "daily", None),
    ];
    for (name, content, sub) in right {
        g.close_overlays();
        g.right_panel.open(content);
        match (content, sub) {
            ("index", Some(t)) => g.index_tab = if t == "golden" { "golden" } else { "normal" },
            ("tree", s) => g.tree_selected_category = s.map(|_| "luck"),
            ("milestones", s) => g.milestones_selected_category = s.map(|_| "rolls"),
            _ => {}
        }
        g.right_panel.update(5.0);
        g.draw(m);
        save(&g, name);
    }
    g.right_panel.close();
    g.right_panel.update(5.0);
    g.left_panel.open("bag");
    g.left_panel.update(5.0);
    g.draw(m);
    save(&g, "g_bag");
    g.toggle_bag_view();
    g.draw(m);
    save(&g, "g_bag_inventory");
    g.left_panel.close();
    g.left_panel.update(5.0);
    g.toggle_traits();
    g.draw(m);
    save(&g, "g_traits");
    g.toggle_rebirth();
    g.draw(m);
    save(&g, "g_rebirth");
    g.close_overlays();
    g.toggle_stats();
    g.draw(m);
    save(&g, "g_stats");
    g.close_overlays();
    for tab in ["gameplay", "interface", "volume", "sfx", "game"] {
        g.open_options();
        g.set_options_tab(tab);
        g.draw(m);
        save(&g, &format!("g_options_{}", tab));
    }
    g.close_overlays();
    g.start_cutscene(tier_first_pet(TIER_TRANSCENDENT), "golden");
    g.cutscene_elapsed = 1.0;
    g.draw(m);
    save(&g, "g_cutscene");
    g.end_cutscene();

    // ---- v2.6-v2.9 screens
    for key in ["etereo", "celestial", "absoluto", "primordial", "paradoxo"] {
        g.start_cutscene(tier_first_pet(core::data::tier_index(key)), "rainbow");
        g.cutscene_elapsed = 1.0;
        g.draw(m);
        save(&g, &format!("n_cutscene_{}", key));
        g.end_cutscene();
    }
    g.state.rebirths = g.state.rebirths.max(1);
    g.state.coins = 1e12;
    let shop_now = online::firebase::server_now();
    g.state.buy_dice("wood", shop_now);
    *g.state.shop.potions.entry("luck_2".into()).or_insert(0) += 6;
    *g.state.shop.potions.entry("money_1".into()).or_insert(0) += 2;
    g.state.use_potion("money", 1);
    g.draw(m);
    save(&g, "n_main_dice");
    // v3.0 effects
    let tnow = core::state::now_ts();
    // verities raining behind the roll screen (run the rain for a while so the screen fills up) + a roll pop
    for _ in 0..240 {
        g.update_verity_fx(0.05);
    }
    g.last_click_pos = Some({
        let c = g.main_card_rect();
        (c.centerx() as f64, (c.bottom() + 50) as f64)
    });
    g.spawn_roll_pop(tier_first_pet(core::data::tier_index("absoluto")), "rainbow");
    g.update_verity_fx(0.3);
    g.draw(m);
    save(&g, "v3_rain");
    let roll_c = {
        let c = g.main_card_rect();
        (c.centerx() as f64 - 40.0, (c.bottom() + 50) as f64)
    };
    g.press_fx = Some((roll_c, tnow - 0.12));
    g.draw(roll_c);
    save(&g, "v3_hover_ripple");
    g.press_fx = None;
    g.too_fast_until = f64::INFINITY;
    g.too_fast_best = Some((tier_first_pet(core::data::tier_index("absoluto")), "diamond"));
    g.draw(m);
    save(&g, "n_too_fast");
    g.too_fast_until = 0.0;
    g.toggle_index_page();
    g.draw(m);
    save(&g, "n_index_page");
    g.index_tab = "rainbow";
    g.index_scroll = 1e9;
    g.draw(m);
    g.draw(m);
    save(&g, "n_index_page_rainbow_end");
    g.close_overlays();
    g.toggle_shop();
    g.draw(m);
    save(&g, "n_shop_dice");
    g.shop.tab = "potions";
    g.draw(m);
    save(&g, "n_shop_potions");
    g.close_overlays();
    g.left_panel.open("bag");
    g.left_panel.update(5.0);
    g.show_bag_potions();
    g.draw(m);
    save(&g, "n_bag_potions");
    g.bag_view = "inventory";
    g.draw(m);
    save(&g, "n_bag_inventory");
    g.open_sell(0, "normal");
    g.sell.field.set_text("3");
    g.draw(m);
    save(&g, "n_sell");
    g.close_overlays();
    g.left_panel.close();
    g.left_panel.update(5.0);
    g.right_panel.open("milestones");
    g.right_panel.update(5.0);
    g.milestones_selected_category = None;
    g.draw(m);
    save(&g, "n_milestones");
    g.select_milestone_group(Some("rarities"));
    g.draw(m);
    save(&g, "n_milestones_rarities");
    g.right_panel.close();
    g.right_panel.update(5.0);
    g.close_overlays();

    // ---- online screens with made-up data (no network: every refresh is pushed into the future)
    use online::firebase::{Event, Feedback, Message, Person};
    let now = 1_790_000_000.0;
    g.back_to_title();
    g.open_saves();
    g.draw(m);
    save(&g, "o_saves_local");
    g.screen_mode = "title";
    g.account = Some(game::Account { uid: "u_me".into(), username: "tommy".into(), email: Some("t@x.io".into()), email_verified: true });
    let person = |uid: &str, name: &str, pet: Option<i64>, m: &str| Person { uid: uid.into(), username: name.into(), avatar_pet: pet, avatar_mut: m.into(), time: None, last_seen: None, title: None };
    g.fr.list = vec![person("u_a", "alice", Some(3), "golden"), person("u_b", "bob", None, "normal"), person("u_c", "carol_long_name", Some(12), "diamond")];
    let snow = online::firebase::server_now();
    g.fr.list[0].last_seen = Some(snow - 30.0);
    g.fr.list[1].last_seen = Some(snow - 7300.0);
    g.fr.list[0].title = Some("top1".into());
    g.fr.list[2].title = Some("rarity_hunter".into());
    g.fr.incoming = vec![person("u_d", "dave", Some(0), "normal")];
    g.fr.outgoing = vec![person("u_e", "eve", None, "normal")];
    g.fr.loaded = true;
    g.fr.next_refresh = f64::INFINITY;
    g.fr.badge_at = f64::INFINITY;
    g.fr.profile_pushed = true;
    g.chat.last.insert("u_a".into(), now - 30.0);
    g.draw(m);
    save(&g, "o_title_logged");
    g.open_friends();
    g.draw(m);
    save(&g, "o_friends");
    g.ev.is_admin = true;
    g.state.title = Some("owner");
    g.draw(m);
    save(&g, "t_friends_titled");
    g.open_title_picker();
    g.draw(m);
    save(&g, "t_title_picker");
    g.titles.picker = false;
    g.state.title = None;
    g.ev.is_admin = false;
    g.set_friends_tab("requests");
    g.draw(m);
    save(&g, "o_friends_requests");
    g.set_friends_tab("add");
    g.fr.search.set_text("alice");
    g.fr.result = Some(Some(person("u_a", "alice", Some(3), "golden")));
    g.draw(m);
    save(&g, "o_friends_add");
    g.fr.stats.insert("u_a".into(), Some(online::firebase::PublicStats { username: "alice".into(), coins: 123456789.0, playtime: 98765.0, rolls: 43210, rebirths: 7, updated_at: Some(now - 100.0) }));
    g.fr.view = Some("u_a".into());
    g.draw(m);
    save(&g, "o_friend_view");
    g.fr.view = None;
    g.fr.avatar_picker = true;
    g.draw(m);
    save(&g, "o_avatar_picker");
    g.fr.avatar_picker = false;
    g.chat.uid = Some("u_a".into());
    g.chat.name = "alice".into();
    g.chat.next_poll = f64::INFINITY;
    let msg = |id: &str, from: &str, text: &str, t: f64| Message { id: id.into(), from_uid: from.into(), text: text.into(), sent_at: Some(t) };
    g.chat.messages = vec![
        msg("1", "u_a", "hey! how many rebirths do you have?", now - 7200.0),
        msg("2", "u_me", "seven, going for the transcendent one now", now - 3000.0),
        msg("3", "u_a", "good luck, it took me ages to get a diamond one and a lot of rolls with the auto roller running all night long", now - 40.0),
    ];
    g.chat.field.set_text("gl");
    g.chat.focus = true;
    g.draw(m);
    save(&g, "o_chat");
    g.close_chat();
    // trades: a slot tied to the account so trades_ready() is true
    g.state.cloud_uid = Some("u_me".into());
    g.trades.next_refresh = f64::INFINITY;
    g.trades.loaded = true;
    g.trades.received = vec![online::firebase::Trade {
        id: "t1".into(),
        from_uid: "u_a".into(),
        to_uid: "u_me".into(),
        offer: vec![("3_golden".into(), 2)],
        request: vec![("0_normal".into(), 50), ("1_normal".into(), 5)],
    }];
    g.trades.sent = vec![online::firebase::Trade { id: "t2".into(), from_uid: "u_me".into(), to_uid: "u_b".into(), offer: vec![("2_diamond".into(), 1)], request: vec![("9_normal".into(), 1)] }];
    g.set_friends_tab("trades");
    g.draw(m);
    save(&g, "o_trades");
    g.trades.target = Some("u_a".into());
    g.trades.target_name = "alice".into();
    g.trades.their_wallet = Some([("3_golden".to_string(), 4i64), ("5_normal".to_string(), 12)].into_iter().collect());
    g.set_trade_offer_qty("offer", "0_normal", 20);
    g.set_trade_offer_qty("request", "3_golden", 2);
    g.draw(m);
    save(&g, "o_trade_propose");
    g.close_trade_propose();
    g.state.cloud_uid = None;
    g.close_friends();
    g.fb.next_refresh = f64::INFINITY;
    let fbk = |uid: &str, name: &str, text: &str, t: f64| Feedback { uid: uid.into(), username: name.into(), text: text.into(), updated_at: Some(t) };
    g.fb.list = vec![fbk("u_a", "alice", "More pets please! Also a way to trade them with friends would be really cool.", now - 400.0), fbk("u_b", "bob", "love it", now - 90000.0)];
    g.fb.mine = Some(fbk("u_me", "tommy", "Add a dark mode for the cards.", now - 10.0));
    g.fb.loaded = true;
    g.open_feedback();
    g.draw(m);
    save(&g, "o_feedback");
    g.fb.confirm_delete = true;
    g.draw(m);
    save(&g, "o_feedback_confirm");
    g.start_feedback_edit();
    g.draw(m);
    save(&g, "o_feedback_edit");
    g.close_feedback();
    g.ev.current = vec![Event { kind: "luck".into(), mult: 10.0, ends_at: now + 250.0, by: "tommy".into() }];
    g.ev.is_admin = true;
    g.ev.admin_checked = true;
    g.ev.next_poll = f64::INFINITY;
    g.toggle_event_admin();
    g.draw(m);
    save(&g, "o_event_admin");
    g.close_event_admin();
    g.toggle_admin_menu();
    g.draw(m);
    save(&g, "o_admin_menu");
    g.adm.menu_open = false;
    g.adm.ban_open = true;
    g.adm.list_loaded = true;
    g.adm.list = vec![online::firebase::Ban { uid: "u_x".into(), username: "cheater".into(), reason: "Edited the save".into(), by: "tommy".into(), at: Some(now - 3600.0) }];
    g.adm.found = Some(person("u_b", "bob", None, "normal"));
    g.draw(m);
    save(&g, "o_ban_admin");
    g.adm.ban_open = false;
    g.adm.info = Some(online::firebase::Ban { uid: "u_me".into(), username: "tommy".into(), reason: "Testing the ban screen".into(), by: "admin".into(), at: Some(now - 86400.0) });
    g.draw(m);
    save(&g, "o_ban_screen");
    g.adm.info = None;
    g.adm.checked_uid = Some("u_me".into());
    g.adm.check_at = f64::INFINITY;
    g.close_event_admin();
    g.lb_retry_at = f64::INFINITY;
    g.lb_data = Some(serde_json::json!({
        "period": 0, "fetched_at": now - 200.0,
        "money": [{"username": "alice", "value": 9.9e12}, {"username": "tommy", "value": 1.5e9}, {"username": "bob", "value": 12345.0}, {"username": "zed", "value": 10.0}],
        "playtime": [{"username": "bob", "value": 360000.0}],
        "rolls": [], "rebirths": []
    }));
    g.open_leaderboard();
    g.draw(m);
    save(&g, "o_leaderboard");
    g.set_lb_tab("playtime");
    g.draw(m);
    save(&g, "o_leaderboard_playtime");
    g.set_lb_tab("rebirths");
    g.draw(m);
    save(&g, "o_leaderboard_empty");
    g.close_leaderboard();
    g.screen_mode = "game";
    g.draw(m);
    save(&g, "o_game_event_banner");
    g.screen_mode = "title";
    g.open_account();
    g.draw(m);
    save(&g, "o_account_logged");
    g.close_account();
}

fn statetest(seed: u64, n: usize) {
    use core::data::upgrade_defs;
    use core::state::{GameState, RNG};
    use pyfmt::py_float_str as r;
    RNG.with(|g| *g.borrow_mut() = pyrand::PyRandom::from_int(seed));
    let mut s = GameState::new();
    s.slot = Some(1);
    let mut log: Vec<String> = Vec::new();
    for i in 0..n {
        let (a, b, c, d) = s.roll();
        log.push(format!("{} {} {} {}", a, b, c as i32, d as i32));
        let inc = s.income_per_second() * 3.7 + 11.0;
        s.coins += inc;
        let inc = s.income_per_second() * 3.7 + 11.0;
        s.total_coins_earned += inc;
        s.playtime += 1.5;
        if i % 37 == 0 {
            for u in upgrade_defs() {
                let got = s.buy_upgrade_bulk(u.key, [0, 1, 10][i % 3]);
                if got > 0 {
                    log.push(format!("buy {} {} {}", u.key, got, r(s.coins)));
                }
            }
        }
        if s.trait_charges > 0 && i % 5 == 0 {
            let t = s.roll_trait();
            log.push(format!("trait {}", t.map(|x| x.to_string()).unwrap_or("None".into())));
            if let Some(t) = t {
                s.equip_trait(t);
            }
        }
        for m in s.check_milestones() {
            log.push(format!("ms {} {}", m.0, m.1));
        }
        if i % 11 == 0 {
            s.equip_best();
        }
        if i % 97 == 0 && s.do_rebirth() {
            log.push(format!("rebirth {}", s.rebirths));
        }
        if i % 50 == 0 {
            let (g, d, rb) = s.mutation_chances();
            log.push(format!("w {} {} {} ({}, {}, {})", r(s.luck_multiplier(20)), r(s.money_multiplier()), r(s.auto_rolls_per_second()), r(g), r(d), r(rb)));
        }
    }
    for k in 0..3 {
        log.push(format!("claim {}", s.claim_daily_mission(k)));
    }
    // v2.7-v2.9: end-game upgrades, the Shop, potions, selling and bulk rolls
    let pyb = |b: bool| if b { "True" } else { "False" };
    s.coins = 1e30;
    s.rebirths = s.rebirths.max(12);
    for u in upgrade_defs() {
        let got = s.buy_upgrade_bulk(u.key, 0);
        if got > 0 {
            log.push(format!("buy {} {} {}", u.key, got, r(s.coins)));
        }
    }
    let now = 1800000000.0 + seed as f64 * 600.0;
    for d in core::shop::DICE.iter() {
        let ok = s.buy_dice(d.key, now);
        log.push(format!("dice {} {}", d.key, pyb(ok)));
    }
    for p in core::shop::POTION_TYPES.iter() {
        for lvl in 1..=5 {
            let ok = s.buy_potion(p.key, lvl, now);
            log.push(format!("potion {} {} {} {}", p.key, lvl, pyb(ok), r(s.coins)));
        }
    }
    *s.shop.potions.entry("luck_1".into()).or_insert(0) += 11;
    let (c1, c2) = (s.combine_potions("luck", 1), s.combine_potions("luck", 1));
    log.push(format!("combine {} {}", pyb(c1), pyb(c2)));
    let (u1, u2, u3) = (s.use_potion("luck", 2), s.use_potion("luck", 1), s.use_potion("money", 1));
    log.push(format!("use {} {} {}", pyb(u1), pyb(u2), pyb(u3)));
    s.equip_dice("iron");
    let (g, d, rb) = s.mutation_chances();
    log.push(format!("x {} {} ({}, {}, {}) {}", r(s.flat_luck(1.0)), r(s.money_multiplier()), r(g), r(d), r(rb), r(s.luck_multiplier(40))));
    let probs = s.pet_probs(1.0, None);
    log.push(format!("p {}", probs[probs.len() - 12..].iter().map(|x| r(*x)).collect::<Vec<_>>().join(" ")));
    for _ in 0..400 {
        let (a, b, c, d) = s.roll();
        log.push(format!("{} {} {} {}", a, b, c as i32, d as i32));
    }
    let (best, ch) = s.roll_bulk(10_000_000);
    log.push(match best {
        Some((i, m)) => format!("bulk ({}, '{}', {})", i, m, ch),
        None => format!("bulk (None, None, {})", ch),
    });
    s.trait_charges += 2_000_000;
    for n in [1_000_000i64, 300] {
        let res = s.roll_traits_bulk(n);
        let mut kv: Vec<(usize, i64)> = res.into_iter().collect();
        kv.sort();
        log.push(format!("tb {} {}", kv.iter().map(|(k, v)| format!("{}:{}", k, v)).collect::<Vec<_>>().join(" "), s.trait_charges));
    }
    let (s1, g1) = s.sell_pets(0, "normal", 5);
    let (s2, g2) = s.sell_pets(1, "golden", 1_000_000_000);
    log.push(format!("sell ({}, {}) ({}, {})", s1, r(g1), s2, r(g2)));
    s.tick_potions(700.0);
    s.coins = 1e20;
    s.do_rebirth();
    let bought = s.auto_upgrade_step();
    log.push(format!("auto {} {}", bought, r(s.coins)));
    for m in s.check_milestones() {
        log.push(format!("ms {} {}", m.0, m.1));
    }
    println!("{}", log.join("\n"));
    let d = s.to_dict();
    let text = pyjson::dumps(&d);
    println!("{}", text);
    let mut s2 = GameState::new();
    s2.slot = Some(1);
    let _ = s2.load_dict(&serde_json::from_str(&text).unwrap());
    let t2 = pyjson::dumps(&s2.to_dict());
    if t2 != text { eprintln!("{}", t2); }
    println!("{}", if t2 == text { "True" } else { "False" });
}
