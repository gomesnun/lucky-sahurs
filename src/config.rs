//! General constants (window, version, save folders, intervals) - config.py.

use std::path::{Path, PathBuf};
use std::sync::OnceLock;

pub const VIRTUAL_H: i32 = 800;
pub const VW_MIN: i32 = 1000;
pub const VW_MAX: i32 = 2400;
pub const TOPBAR_H: i32 = 70;
pub const FPS: u32 = 60;
pub const GAME_TITLE: &str = "Lucky Verities";

/// Version number (vX.Y.Z): GitHub Actions passes the release tag in LV_BUILD_VERSION (see
/// .github/workflows/build.yml), so the game knows which version it is and updates never loop.
/// Without it ("cargo build") it is "dev": no update checks.
pub const BUILD_VERSION: &str = match option_env!("LV_BUILD_VERSION") {
    Some(v) if !v.is_empty() => v,
    _ => "dev",
};
/// What the menu shows.
pub const VERSION: &str = if const_eq(BUILD_VERSION, "dev") { "v3.0.4" } else { BUILD_VERSION };

const fn const_eq(a: &str, b: &str) -> bool {
    let (a, b) = (a.as_bytes(), b.as_bytes());
    if a.len() != b.len() {
        return false;
    }
    let mut i = 0;
    while i < a.len() {
        if a[i] != b[i] {
            return false;
        }
        i += 1;
    }
    true
}

pub const UPDATE_REPO: &str = "gomesnun/lucky-verities";
pub const UPDATE_CHECK_INTERVAL: f64 = 30.0 * 60.0;
pub const UPDATE_RETRY_AFTER_ERROR: f64 = 5.0 * 60.0;

pub const PASTA_DE_DADOS: &str = "LuckyVerities";
pub const PASTA_DE_DADOS_ANTIGA: &str = "LuckySahurs";

pub const SAVE_SLOTS: i64 = 3;
pub const AUTOSAVE_INTERVAL: f64 = 10.0;
pub const FULLSCREEN_HINT: &str = if cfg!(target_os = "macos") { "F11 / Cmd+F" } else { "F11" };

/// os.path.expanduser("~")
fn home() -> PathBuf {
    let var = if cfg!(target_os = "windows") { "USERPROFILE" } else { "HOME" };
    std::env::var_os(var).filter(|v| !v.is_empty()).map(PathBuf::from).unwrap_or_else(|| PathBuf::from("."))
}

fn data_dir(name: &str) -> PathBuf {
    if cfg!(target_os = "windows") {
        let base = std::env::var_os("APPDATA").map(PathBuf::from).unwrap_or_else(home);
        return base.join(name);
    }
    if cfg!(target_os = "macos") {
        return home().join("Library").join("Application Support").join(name);
    }
    let base = std::env::var_os("XDG_DATA_HOME").map(PathBuf::from).unwrap_or_else(|| home().join(".local").join("share"));
    base.join(name)
}

/// Folder the game runs from (where the executable is). Players can drop icons/ fonts/ sounds/ here.
pub fn game_dir() -> &'static Path {
    static D: OnceLock<PathBuf> = OnceLock::new();
    D.get_or_init(|| {
        std::env::current_exe()
            .ok()
            .and_then(|p| p.parent().map(|p| p.to_path_buf()))
            .unwrap_or_else(|| PathBuf::from("."))
    })
}

/// Where bundled assets live: next to the executable (assets/ or the folders directly), in the
/// directory above it (cargo target layout), or in the source tree.
pub fn asset_roots() -> &'static [PathBuf] {
    static R: OnceLock<Vec<PathBuf>> = OnceLock::new();
    R.get_or_init(|| {
        let g = game_dir().to_path_buf();
        let mut v = vec![g.clone(), g.join("assets")];
        let mut up = g.clone();
        for _ in 0..3 {
            if let Some(p) = up.parent() {
                up = p.to_path_buf();
                v.push(up.join("assets"));
            }
        }
        let src = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        v.push(src.join("assets"));
        v.push(src);
        v.push(PathBuf::from("/usr/share/lucky-verities"));
        v
    })
}

fn can_write(folder: &Path) -> bool {
    if std::fs::create_dir_all(folder).is_err() {
        return false;
    }
    let test = folder.join(".escrita_teste");
    if std::fs::write(&test, "ok").is_err() {
        return false;
    }
    let _ = std::fs::remove_file(&test);
    true
}

fn new_name(name: &str) -> String {
    if let Some(rest) = name.strip_prefix("lucky_sahurs_") { format!("lucky_verities_{}", rest) } else { name.to_string() }
}

fn glob_simple(dir: &Path, prefix: &str, suffix: &str) -> Vec<PathBuf> {
    let mut out = Vec::new();
    if let Ok(rd) = std::fs::read_dir(dir) {
        for e in rd.flatten() {
            let n = e.file_name().to_string_lossy().to_string();
            if n.starts_with(prefix) && n.ends_with(suffix) && e.path().is_file() {
                out.push(e.path());
            }
        }
    }
    out.sort();
    out
}

fn copy_tree(src: &Path, dst: &Path) -> std::io::Result<()> {
    std::fs::create_dir_all(dst)?;
    for e in std::fs::read_dir(src)?.flatten() {
        let p = e.path();
        let d = dst.join(e.file_name());
        if p.is_dir() {
            copy_tree(&p, &d)?;
        } else {
            std::fs::copy(&p, &d)?;
        }
    }
    Ok(())
}

/// One-time COPY of saves from an older folder (never deletes or overwrites).
fn migrate_old_saves(old: &Path, new: &Path, marker_name: &str) {
    let marker = new.join(marker_name);
    if marker.exists() {
        return;
    }
    if let (Ok(a), Ok(b)) = (old.canonicalize(), new.canonicalize()) {
        if a == b {
            return;
        }
    }
    let run = || -> std::io::Result<()> {
        if glob_simple(new, "savegame", ".json").is_empty() {
            let patterns = [("savegame", ".json"), ("lucky_sahurs_", ".json"), ("lucky_verities_", ".json"), ("firebase_config", ".json")];
            for (pre, suf) in patterns {
                for src in glob_simple(old, pre, suf) {
                    let name = src.file_name().unwrap().to_string_lossy().to_string();
                    if pre == "firebase_config" && name != "firebase_config.json" {
                        continue;
                    }
                    let dst = new.join(new_name(&name));
                    if !dst.exists() {
                        std::fs::copy(&src, &dst)?;
                    }
                }
            }
            let cache = old.join("cloud_cache");
            if cache.is_dir() && !new.join("cloud_cache").exists() {
                copy_tree(&cache, &new.join("cloud_cache"))?;
            }
        }
        std::fs::write(&marker, "ok")?;
        Ok(())
    };
    let _ = run();
}

pub fn save_dir() -> &'static Path {
    static D: OnceLock<PathBuf> = OnceLock::new();
    D.get_or_init(|| {
        let over = std::env::var("LUCKY_VERITIES_SAVE_DIR").ok().filter(|s| !s.is_empty()).or_else(|| {
            std::env::var("LUCKY_SAHURS_SAVE_DIR").ok().filter(|s| !s.is_empty())
        });
        if let Some(o) = over {
            let expanded = if let Some(rest) = o.strip_prefix("~/") { home().join(rest) } else { PathBuf::from(&o) };
            let folder = std::path::absolute(&expanded).unwrap_or(expanded);
            can_write(&folder);
            return folder;
        }
        let dados = data_dir(PASTA_DE_DADOS);
        if can_write(&dados) {
            migrate_old_saves(&data_dir(PASTA_DE_DADOS_ANTIGA), &dados, ".migrado_sahurs");
            migrate_old_saves(game_dir(), &dados, ".migrado");
            return dados;
        }
        game_dir().to_path_buf()
    })
}

pub fn save_path_legacy() -> PathBuf {
    save_dir().join("savegame.json")
}
