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
    if args.len() > 2 && args[1] == "--onlinetest" {
        online_test(&args[2]);
        return;
    }
    if args.len() > 2 && args[1] == "--explorevideo" {
        explore_video(&args[2]);
        return;
    }
    if args.len() > 2 && args[1] == "--teaservideo" {
        teaser_video(&args[2]);
        return;
    }
    if args.len() > 2 && args[1] == "--battlevideo" {
        battle_video(&args[2]);
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
    if args.len() > 1 && args[1] == "--webdata" {
        webdata();
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
    // Windows: the downloaded file copies itself to "Lucky Verities.exe" (+ shortcuts) and goes on running
    online::shortcut::install_windows();
    // desktop shortcut (1st time, release builds only): on its own thread so it never delays the start
    std::thread::spawn(online::shortcut::create_desktop_shortcut_once);
    let sdl = sdl2::init().expect("SDL init failed");
    let mut g = game::Game::new(Some(&sdl));
    g.run(&sdl);
}

/// The game's tables as JavaScript for website/index.html (`--webdata`): the site's simulator and lists use the
/// same numbers as the game.
fn webdata() {
    use core::data::*;
    let hex = |c: gfx::Color| format!("'#{:02x}{:02x}{:02x}'", c.r, c.g, c.b);
    let pt = |s: &str| {
        i18n::set_language("pt");
        let t = i18n::tr(s);
        i18n::set_language("en");
        t
    };
    let q = |s: &str| format!("'{}'", s.replace('\\', "\\\\").replace('\'', "\\'"));
    let mut out = String::from("/* ---- generated by `lucky-verities --webdata` from the game's own tables (src/core/data.rs) ---- */\n");
    out += "var TIERS = [\n";
    for t in RARITY_TIERS.iter() {
        out += &format!(
            "  {{key:{}, en:{}, pt:{}, color:{}, color2:{}, text:{}, oneIn:{}, income:{}}},\n",
            q(t.key), q(t.name), q(&pt(t.name)), hex(t.color), t.color2.map(hex).unwrap_or("null".into()), hex(t.text), t.one_in, t.income
        );
    }
    out += "];\nvar TIER_INDEX = {}; TIERS.forEach(function(t,i){ TIER_INDEX[t.key] = i; });\n";
    out += "var PETS = [\n";
    for r in rarities().iter() {
        out += &format!("  {{tierKey:{}, name:{}, income:{}, weight:{:e}}},\n", q(RARITY_TIERS[r.tier].key), q(r.pet), r.income, r.share / r.one_in);
    }
    out += "];\nPETS.forEach(function(p){ p.tierIdx = TIER_INDEX[p.tierKey]; p.tier = TIERS[p.tierIdx]; p.isSecond = false; });\n";
    out += "var TOTAL_WEIGHT = PETS.reduce(function(s,p){ return s + p.weight; }, 0);\n";
    out += &format!(
        "var MUTATIONS = {{normal:{{mult:1,en:'',pt:''}}, golden:{{mult:{},en:'Golden',pt:{}}}, diamond:{{mult:{},en:'Diamond',pt:{}}}, rainbow:{{mult:{},en:'Rainbow',pt:{}}}}};\n",
        mutation("golden").unwrap().mult, q(&pt("Golden")), mutation("diamond").unwrap().mult, q(&pt("Diamond")), mutation("rainbow").unwrap().mult, q(&pt("Rainbow"))
    );
    out += &format!("var GOLDEN_BASE = {}, DIAMOND_BASE = {}, RAINBOW_BASE = {}; /* the chances right after unlocking each, no upgrades */\n", GOLDEN_BASE, DIAMOND_BASE, RAINBOW_BASE);
    out += "var TRAITS = [\n";
    for t in TRAITS.iter() {
        out += &format!("  {{name:{}, oneIn:{}, money:{}}},\n", q(t.name), t.one_in, (t.buff("money") * 100.0).round() / 100.0);
    }
    out += "];\nvar TRAIT_COLORS = [";
    out += &TRAITS.iter().map(|t| hex(t.color)).collect::<Vec<_>>().join(",");
    out += "];\nvar MILESTONES = [\n";
    for m in MILESTONES.iter() {
        out += &format!("  {{key:{}, en:{}, pt:{}}},\n", q(m.key), q(m.label), q(&pt(m.label)));
    }
    out += "];\n/* ---- end of generated data ---- */\n";
    print!("{}", out);
}

/// Milliseconds per draw() of a few screens: `--bench`.
fn bench() {
    use core::state::{RNG, set_fake_time};
    set_fake_time(Some(1_790_000_000.0));
    RNG.with(|r| *r.borrow_mut() = pyrand::PyRandom::from_int(7));
    let mut g = game::Game::headless(1422, 800);
    let m = (-100.0, -100.0);
    let run = |g: &mut game::Game, name: &str| {
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
    // v3.0 effects: the verity rain running (update + draw each frame, like the real loop)
    for _ in 0..3000 {
        g.update_verity_fx(1.0 / 60.0);
    }
    {
        let n = 300;
        let s = std::time::Instant::now();
        for _ in 0..n {
            g.update_verity_fx(1.0 / 60.0);
            g.draw(m);
        }
        println!("{:<14} {:.3} ms  ({} verities falling)", "game+rain", s.elapsed().as_secs_f64() * 1000.0 / n as f64, g.vfx_drop_count());
    }
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
/// Two games playing an online battle against each other, the moves passed by hand instead of through the
/// server: both must see the same battle (and the one who accepted sees it from their side). Saves a few shots.
fn online_test(dir: &str) {
    use core::battle::Move;
    use core::data::tier_first_pet;
    use core::state::set_fake_time;
    set_fake_time(Some(1_790_000_000.0));
    let doc = online::firebase::BattleDoc {
        id: "test".into(),
        from_uid: "a".into(),
        to_uid: "b".into(),
        from_name: "Tom".into(),
        to_name: "Nuno".into(),
        from_team: format!("{}_normal_3,{}_rainbow_1", tier_first_pet(6), tier_first_pet(9) + 1),
        to_team: format!("{}_golden_2,{}_normal_0", tier_first_pet(8), tier_first_pet(7) + 2),
        seed: 12345,
        status: "live".into(),
        ..Default::default()
    };
    let mut games: Vec<game::Game> = (0..2).map(|_| game::Game::headless(1422, 800)).collect();
    for (me, g) in games.iter_mut().enumerate() {
        g.settings.set_str("last_update_seen", game::tutorial::latest_update());
        g.settings.set_bool("tutorial_done", true);
        g.start_slot(1);
        g.open_battle();
        g.battle.mode = "online";
        g.start_online_battle(&doc, me);
    }
    let m = (-100.0, -100.0);
    let mut turn = [0usize; 2];
    let mut shots = 0;
    for step in 0..6000 {
        for me in 0..2 {
            let g = &mut games[me];
            g.tick_battle(0.1);
            if g.online_my_turn() && !g.battle.busy() {
                let b = g.battle.battle.as_ref().unwrap();
                if b.needs_switch() {
                    let k = (0..b.teams[b.me].len()).find(|&k| b.can_switch_to(b.me, k)).unwrap();
                    g.battle_switch(k);
                } else if b.fighter(b.me).can_transform() {
                    g.battle_transform();
                } else if turn[me] == 1 && me == 1 {
                    turn[me] += 1;
                    g.state.battle_items.insert("iron".into(), 1);
                    g.battle_item(core::battle::Item::Iron);
                } else {
                    let f = b.fighter(b.me).clone();
                    let mv = if turn[me] % 3 == 0 && f.special_pp > 0 { Move::Special } else if turn[me] % 5 == 4 { Move::Guard } else { Move::Strike };
                    turn[me] += 1;
                    g.battle_use(mv);
                }
            }
        }
        // the "server": each game gets the other's moves
        let (a, b) = (games[0].online_moves(), games[1].online_moves());
        games[0].online_feed(&b);
        games[1].online_feed(&a);
        if step % 400 == 5 && shots < 6 {
            for (me, g) in games.iter_mut().enumerate() {
                g.draw(m);
                save_png(&g.canvas, &std::path::Path::new(dir).join(format!("online_{}_{}.png", shots, if me == 0 { "tom" } else { "nuno" })));
            }
            shots += 1;
        }
        let w: Vec<Option<usize>> = games.iter().map(|g| g.battle.battle.as_ref().and_then(|b| b.winner)).collect();
        if w[0].is_some() && w[1].is_some() && !games[0].battle.busy() && !games[1].battle.busy() {
            assert_eq!(w[0], w[1], "both games must agree on the winner");
            let hp: Vec<Vec<i32>> = games.iter().map(|g| g.battle.battle.as_ref().unwrap().teams.iter().flatten().map(|f| f.hp).collect()).collect();
            assert_eq!(hp[0], hp[1], "both games must end with the same HP");
            for (me, g) in games.iter_mut().enumerate() {
                g.tick_battle(0.1);
                g.draw(m);
                save_png(&g.canvas, &std::path::Path::new(dir).join(format!("online_end_{}.png", if me == 0 { "tom" } else { "nuno" })));
            }
            println!("online test: same battle on both sides, winner side {:?}, {} steps", w[0], step);
            return;
        }
    }
    panic!("the online battle never ended");
}

/// The 4.0 teaser, frame by frame (30 fps): an existing save opened for the first time on 4.0.
fn teaser_video(dir: &str) {
    use core::state::set_fake_time;
    set_fake_time(Some(1_790_000_000.0));
    let mut g = game::Game::headless(1422, 800);
    g.settings.set_bool("tutorial_done", true);
    g.start_slot(1);
    for _ in 0..300 {
        g.state.roll();
    }
    g.settings.set_bool(game::teaser::TEASER_SETTING, false);
    g.on_enter_game();
    let m = (-100.0, -100.0);
    let mut i = 0;
    while g.teaser_active() || i < 20 {
        g.tick(1.0 / 30.0);
        g.draw(m);
        g.apply_glow();
        save_png(&g.canvas, &std::path::Path::new(dir).join(format!("frame_{:05}.png", i)));
        i += 1;
        if i > 30 * 30 {
            break;
        }
    }
}

/// v4.0: the Battle hub, then Explore - Steve chases and catches pets in a few worlds (30 fps frames in dir).
fn explore_video(dir: &str) {
    use core::explore::{VerityPet, xp_for_level};
    use core::state::set_fake_time;
    set_fake_time(Some(1_790_000_000.0));
    let mut g = game::Game::headless(1422, 800);
    g.settings.set_str("last_update_seen", game::tutorial::latest_update());
    g.settings.set_bool("tutorial_done", true);
    g.settings.set_bool(game::teaser::TEASER_SETTING, true);
    g.start_slot(1);
    for _ in 0..300 {
        g.state.roll();
    }
    g.state.explore.xp = xp_for_level(21) - 40.0;
    for (dim, r) in [(4, 0.01), (3, 0.6), (1, 0.4)] {
        g.state.explore.add_pet(VerityPet::roll(dim, [r, 0.3, 0.8, r]));
        let n = g.state.explore.pets.len() - 1;
        g.state.explore.toggle_equip(n);
    }
    g.state.explore.hat = 3;
    g.state.explore.shirt = 6;
    let mut i = 0;
    let mut frame = |g: &mut game::Game, m: (f64, f64)| {
        g.tick(1.0 / 30.0);
        g.draw(m);
        g.apply_glow();
        save_png(&g.canvas, &std::path::Path::new(dir).join(format!("frame_{:05}.png", i)));
        i += 1;
    };
    // the hub, the mouse over Explore
    g.open_battle();
    for k in 0..75 {
        let m = if k < 20 { (700.0, 600.0) } else { (330.0, 420.0) };
        frame(&mut g, m);
    }
    g.open_explore();
    let m = (-100.0, -100.0);
    for (dim, secs) in [(0, 9.0), (1, 5.0), (3, 5.0), (4, 6.0)] {
        if dim != 0 {
            g.explore_travel(dim);
        }
        let n = (secs * 30.0) as i32;
        for _ in 0..n {
            if !g.explore_busy() {
                g.explore_chase_nearest();
            }
            frame(&mut g, m);
        }
    }
    g.explore.overlay = "wardrobe";
    for _ in 0..60 {
        frame(&mut g, m);
    }
}

/// Plays a whole battle by itself and saves every frame (30 fps, Glow on) as dir/frame_NNNNN.png - for a video.
/// Team: a Monster, a Rainbow and a Golden verity. Moves: Special and Strike in turn, Rest when low, switches.
fn battle_video(dir: &str) {
    use core::data::tier_first_pet;
    use core::state::set_fake_time;
    set_fake_time(Some(1_790_000_000.0));
    let mut g = game::Game::headless(1422, 800);
    g.settings.set_str("last_update_seen", game::tutorial::latest_update());
    g.settings.set_bool("tutorial_done", true);
    g.start_slot(1);
    let team = [(tier_first_pet(6), "normal", 3usize), (tier_first_pet(9) + 1, "rainbow", 1), (tier_first_pet(5) + 1, "golden", 2)];
    for (p, m, ph) in team {
        g.state.owned.insert(format!("{}_{}", p, m), 5);
        g.state.phases.insert(format!("{}_{}", p, m), ph);
    }
    g.state.battle_items.insert("potion".into(), 2);
    g.state.battle_items.insert("power".into(), 1);
    g.open_battle();
    g.battle.picks = team.iter().map(|t| (t.0, t.1)).collect();
    g.start_battle();
    let fps = 30.0;
    let m = (-100.0, -100.0);
    let mut turn = 0;
    let mut end_frames = None;
    for i in 0..(fps as usize * 120) {
        g.tick_battle(1.0 / fps);
        let busy = g.battle.busy();
        let state = g.battle.battle.as_ref().map(|b| (b.winner, b.needs_switch(), b.fighter(0).clone(), (0..b.teams[0].len()).find(|&k| b.can_switch_to(0, k))));
        if let Some((winner, needs_switch, me, next)) = state {
            if winner.is_some() {
                if !busy && end_frames.is_none() {
                    end_frames = Some(i + fps as usize * 3);
                }
            } else if !busy {
                if needs_switch {
                    if let Some(k) = next {
                        g.battle_switch(k);
                    }
                } else if me.can_transform() {
                    g.battle_transform();
                } else if turn == 0 && g.state.battle_items.get("power").copied().unwrap_or(0) > 0 {
                    turn += 1;
                    g.battle_item(core::battle::Item::Power);
                } else if me.hp * 10 < me.max_hp * 3 && g.state.battle_items.get("potion").copied().unwrap_or(0) > 0 {
                    g.battle_item(core::battle::Item::Potion);
                } else {
                    use core::battle::Move;
                    let mv = if me.hp * 10 < me.max_hp * 4 && me.rest_pp > 0 {
                        Move::Rest
                    } else if turn % 2 == 0 && me.special_pp > 0 {
                        Move::Special
                    } else {
                        Move::Strike
                    };
                    turn += 1;
                    g.battle_use(mv);
                }
            }
        }
        g.draw(m);
        g.apply_glow();
        save_png(&g.canvas, &std::path::Path::new(dir).join(format!("frame_{:05}.png", i)));
        if end_frames.is_some_and(|e| i >= e) {
            break;
        }
    }
}

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
    // the tutorial and the "What's new" pop-up get their own shots below; elsewhere they'd cover every screen
    g.settings.set_str("last_update_seen", game::tutorial::latest_update());
    g.settings.set_bool("tutorial_done", true);
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
    let right: [(&str, &str, Option<&str>); 8] = [
        ("g_index", "index", None),
        ("g_index_golden", "index", Some("golden")),
        ("g_tree", "tree", None),
        ("g_tree_luck", "tree", Some("luck")),
        ("g_tree_misc", "tree", Some("misc")),
        ("g_milestones", "milestones", None),
        ("g_milestones_rolls", "milestones", Some("rolls")),
        ("g_daily", "daily", None),
    ];
    for (name, content, sub) in right {
        g.close_overlays();
        g.right_panel.open(content);
        match (content, sub) {
            ("index", Some(t)) => g.index_tab = if t == "golden" { "golden" } else { "normal" },
            ("tree", s) => g.tree_selected_category = s.map(|c| if c == "misc" { "misc" } else { "luck" }),
            ("milestones", s) => g.milestones_selected_category = s.map(|_| "rolls"),
            _ => {}
        }
        g.right_panel.update(5.0);
        g.right_panel.update(5.0); // switching content: the old panel closes first, then the new one opens
        g.draw(m);
        save(&g, name);
    }
    // v3.0 weekly quests (the first one finished, ready to claim)
    g.quests_tab = "weekly";
    g.state.ensure_weekly_missions();
    let wm = g.state.weekly_missions[0].clone();
    g.state.weekly_counts.insert(wm.mtype.to_string(), wm.target);
    g.draw(m);
    save(&g, "v3_weekly_quests");
    g.quests_tab = "daily";
    g.right_panel.close();
    g.right_panel.update(5.0);
    g.left_panel.open("bag");
    g.left_panel.update(5.0);
    g.draw(m);
    save(&g, "g_bag");
    if let Some((li, lm)) = g.inventory_entries().first().copied() {
        g.state.toggle_lock(li, lm); // v3.0.4: a locked verity
    }
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
    // v3.0 Prestige: the tabs, the page, the picker, and after one
    let rb_before = g.state.rebirths;
    g.state.rebirths = 12;
    g.state.prestige = 0; // at 12 Rebirths without a Prestige: Rebirth is locked
    g.toggle_rebirth();
    g.draw(m);
    save(&g, "v3_rebirth_tabs");
    g.set_rebirth_tab("prestige");
    g.draw(m);
    save(&g, "v3_prestige");
    g.prestige_picking = true;
    g.draw(m);
    save(&g, "v3_prestige_pick");
    g.prestige_picking = false;
    g.do_prestige_clicked();
    g.draw(m);
    save(&g, "v3_prestige_confirm");
    g.do_prestige_clicked();
    g.draw(m);
    save(&g, "v3_prestige_done");
    g.close_rebirth();
    g.draw(m);
    save(&g, "v3_after_prestige");
    // v3.0.4: Auto Rebirth switch (Prestige II)
    let p_before = g.state.prestige;
    g.state.prestige = 2;
    g.state.rebirths = 3;
    g.state.auto_rebirth_on = true;
    g.toggle_rebirth();
    g.set_rebirth_tab("rebirth");
    g.draw(m);
    save(&g, "v3_auto_rebirth");
    g.close_rebirth();
    g.state.auto_rebirth_on = false;
    g.state.prestige = p_before;
    g.state.rebirths = rb_before;
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
    g.shop.tab = "battle";
    g.draw(m);
    save(&g, "n_shop_battle");
    g.close_overlays();
    g.left_panel.open("bag");
    g.left_panel.update(5.0);
    g.show_bag_potions();
    g.draw(m);
    save(&g, "n_bag_potions");
    g.bag_view = "inventory";
    g.draw(m);
    save(&g, "n_bag_inventory");
    g.bag_view = "equipped"; // few slots: the cards sit in the middle
    g.draw(m);
    save(&g, "n_bag_equipped");
    // v3.0.4: the Auto Trait Roller's "New trait!" card
    g.close_overlays();
    g.left_panel.close();
    g.left_panel.update(5.0);
    g.trait_popup = Some((6, 2.0, true));
    g.draw(m);
    save(&g, "n_trait_popup");
    g.trait_popup = None;
    g.left_panel.open("bag");
    g.left_panel.update(5.0);
    g.bag_view = "inventory";
    g.open_sell(0, "normal");
    g.sell.field.set_text("3");
    g.draw(m);
    save(&g, "n_sell");
    g.close_overlays();
    // phases (stacking): one verity in each phase, then the Evolve page ready / maxed
    {
        let (owned0, phases0, equipped0) = (g.state.owned.clone(), g.state.phases.clone(), g.state.equipped.clone());
        let pets = [(0usize, 60i64, 0usize), (1, 20, 1), (2, 3, 2), (3, 2, 3)];
        g.state.equipped.clear();
        for (i, n, ph) in pets {
            g.state.owned.insert(format!("{}_normal", i), n);
            g.state.phases.insert(format!("{}_normal", i), ph);
            g.state.equip_add(i, "normal");
        }
        g.inv_sort = "rarity";
        g.draw(m);
        save(&g, "n_phases_inventory");
        g.bag_view = "equipped";
        g.draw(m);
        save(&g, "n_phases_equipped");
        g.bag_view = "inventory";
        g.open_evolve(0, "normal");
        g.draw(m);
        save(&g, "n_evolve");
        g.press_evolve();
        g.draw(m);
        save(&g, "n_evolve_done");
        g.open_evolve(2, "normal");
        g.draw(m);
        save(&g, "n_evolve_short");
        g.open_evolve(3, "normal");
        g.draw(m);
        save(&g, "n_evolve_monster");
        g.close_overlays();
        // a battle: the team picker, the start, the moves, an attack and the result
        g.close_overlays();
        g.open_battle();
        g.tick_battle(1.3);
        g.draw(m);
        save(&g, "n_battle_hub");
        // v4.0 Explore: every world, a walk, the wardrobe and the pets
        {
            use core::explore::{VerityPet, xp_for_level};
            g.state.explore.xp = xp_for_level(21) + 30.0;
            for (dim, r) in [(0, 0.2), (1, 0.6), (2, 0.4), (3, 0.9), (4, 0.01), (4, 0.5)] {
                g.state.explore.add_pet(VerityPet::roll(dim, [r, 0.3, 0.8, r]));
            }
            g.state.explore.toggle_equip(4);
            g.state.explore.toggle_equip(3);
            g.state.explore.toggle_equip(1);
            g.state.explore.hat = 3;
            g.state.explore.shirt = 6;
            g.open_explore();
            for (dim, name) in [(0, "overworld"), (1, "nether"), (2, "end"), (3, "end_city"), (4, "emerald_city")] {
                g.enter_world(dim);
                g.tick_explore(0.4);
                g.draw(m);
                let t0 = std::time::Instant::now();
                for _ in 0..10 {
                    g.tick_explore(1.0 / 60.0);
                    g.draw(m);
                }
                eprintln!("explore {} frame: {:.1} ms (average of 10)", name, t0.elapsed().as_secs_f64() * 100.0);
                save(&g, &format!("n_explore_{}", name));
            }
            g.enter_world(0);
            let p = g.explore.pos;
            g.explore.target = Some((p.0 + 6.0, p.1 - 5.0));
            for _ in 0..12 {
                g.tick_explore(1.0 / 30.0);
            }
            g.draw(m);
            save(&g, "n_explore_walk");
            g.explore.overlay = "wardrobe";
            g.draw(m);
            save(&g, "n_explore_wardrobe");
            g.explore.overlay = "pets";
            g.draw(m);
            save(&g, "n_explore_pets");
            g.close_explore();
            g.close_battle();
            g.open_left_panel("bag");
            g.bag_view = "verity_pets";
            g.draw(m);
            save(&g, "n_bag_verity_pets");
            g.bag_view = "equipped";
            g.left_panel.close();
            g.open_battle();
        }
        g.battle_mode("cpu");
        g.draw(m);
        save(&g, "n_battle_pick");
        g.start_battle();
        g.tick_battle(0.9);
        g.draw(m);
        save(&g, "n_battle_sendout");
        g.tick_battle(6.0);
        g.tick_battle(2.0);
        let t0 = std::time::Instant::now();
        for _ in 0..10 {
            g.draw(m);
        }
        eprintln!("battle frame: {:.1} ms (average of 10)", t0.elapsed().as_secs_f64() * 100.0);
        save(&g, "n_battle_start");
        g.battle.menu = "fight";
        g.draw(m);
        save(&g, "n_battle_moves");
        // a Special: "used ...!" (1s), then it charges (0.85s), fires, and hits
        g.battle_use(core::battle::Move::Special);
        g.tick_battle(1.5);
        g.draw(m);
        save(&g, "n_battle_charge");
        g.tick_battle(0.55);
        g.draw(m);
        save(&g, "n_battle_fire");
        g.tick_battle(0.35);
        g.draw(m);
        save(&g, "n_battle_hit");
        g.tick_battle(3.0);
        g.draw(m);
        save(&g, "n_battle_attack");
        // a whole turn, frame by frame (to check the animations)
        g.tick_battle(8.0);
        g.battle_use(core::battle::Move::Special);
        for i in 0..20 {
            g.tick_battle(0.45);
            g.draw(m);
            save(&g, &format!("n_battle_seq_{:02}", i));
        }
        for _ in 0..200 {
            g.tick_battle(30.0);
            let Some(b) = g.battle.battle.as_ref() else { break };
            if b.winner.is_some() {
                break;
            }
            if b.needs_switch() {
                let next = (0..b.teams[0].len()).find(|&i| b.can_switch_to(0, i)).unwrap_or(0);
                g.battle_switch(next);
            } else {
                g.battle_use(core::battle::Move::Special);
                g.battle_use(core::battle::Move::Strike);
            }
        }
        g.tick_battle(30.0);
        g.draw(m);
        save(&g, "n_battle_result");
        g.close_battle();
        g.left_panel.close();
        g.left_panel.update(5.0);
        // the tutorial (a few of its steps) and the "What's new" pop-up
        g.start_tutorial();
        g.left_panel.update(5.0);
        for step in [0usize, 1, 2, 4, 6, 11, 12] {
            g.tutorial_step = Some(step);
            g.draw(m);
            save(&g, &format!("n_tutorial_{:02}", step));
        }
        g.tutorial_step = None;
        g.settings.set_str("last_update_seen", "v3.0.0");
        g.whats_new_open = true;
        g.draw(m);
        save(&g, "n_whats_new");
        g.whats_new_open = false;
        g.settings.set_str("last_update_seen", game::tutorial::latest_update());
        g.toggle_options();
        g.options_tab = "game";
        g.draw(m);
        save(&g, "n_options_game");
        g.options_tab = "interface";
        g.draw(m);
        save(&g, "n_options_interface");
        g.close_options();
        // the Glow effect (applied to the finished frame, like the real game does before showing it)
        g.draw(m);
        save(&g, "n_glow_off");
        let t0 = std::time::Instant::now();
        g.apply_glow();
        eprintln!("glow: {:.2} ms", t0.elapsed().as_secs_f64() * 1000.0);
        save(&g, "n_glow_on");
        g.left_panel.open("bag");
        g.left_panel.update(5.0);
        g.bag_view = "equipped";
        g.draw(m);
        save(&g, "n_glow_bag_off");
        g.apply_glow();
        save(&g, "n_glow_bag_on");
        g.left_panel.close();
        g.left_panel.update(5.0);
        g.left_panel.open("bag");
        g.left_panel.update(5.0);
        g.inv_sort = "money";
        g.state.owned = owned0;
        g.state.phases = phases0;
        g.state.equipped = equipped0;
    }
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
    // v3.0.1: the Resets page
    g.open_reset_admin();
    g.adm.search.set_text("friend");
    g.adm.msg = Some((i18n::tr("No player with that name."), theme::BAD));
    g.draw(m);
    save(&g, "v3_resets");
    g.adm.msg = None;
    g.adm.search.set_text("");
    g.adm.page = "ban";
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
    g.lb_data.as_mut().unwrap()["rebirths"] = serde_json::json!([
        {"username": "alice", "value": 4.0, "prestige": 3}, {"username": "tommy", "value": 17.0, "prestige": 1},
        {"username": "bob", "value": 38.0}, {"username": "zed", "value": 2.0}
    ]);
    g.draw(m);
    save(&g, "v3_lb_prestige");
    g.close_leaderboard();
    g.screen_mode = "game";
    g.draw(m);
    save(&g, "o_game_event_banner");
    // online battles: the "vs Friends" tab with a challenge got and one sent
    g.open_battle();
    g.battle.mode = "online";
    g.battle.ch_next = f64::INFINITY;
    g.battle.ch_loaded = true;
    let bd = |id: &str, from: &str, to: &str| online::firebase::BattleDoc { id: id.into(), from_uid: format!("u_{}", from), to_uid: format!("u_{}", to), from_name: from.into(), to_name: to.into(), status: "pending".into(), ..Default::default() };
    g.battle.received = vec![bd("b1", "alice", "tommy")];
    g.battle.sent = vec![bd("b2", "tommy", "bob")];
    g.draw(m);
    save(&g, "o_battle_friends");
    g.battle.mode = "ranked";
    g.state.rank_rating = 1312;
    g.state.rank_wins = 14;
    g.state.rank_losses = 9;
    let re = |n: &str, r: i64| online::firebase::RankEntry { name: n.into(), rating: r, wins: 0, losses: 0 };
    g.battle.ranked.top = vec![re("alice", 1742), re("bob", 1580), re("tommy", 1312), re("carol", 1190), re("dave", 1020)];
    g.draw(m);
    save(&g, "o_battle_ranked");
    g.battle.ranked.searching = true;
    g.battle.ranked.since = online::firebase::now() - 17.0;
    g.draw(m);
    save(&g, "o_battle_ranked_search");
    g.battle.ranked.searching = false;
    g.close_battle();
    g.battle.received.clear();
    g.battle.sent.clear();
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
        log.push(format!("claim {}", s.claim_daily_mission(k).map_or(0, |r| r.charges)));
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
