//! Desktop shortcut, made by itself the first time the packaged game runs (online/shortcut.py).
//!
//! Happens only once (marked with a file in the saves folder) and only in a release build - a dev build never
//! creates anything. Any failure (no desktop, no permission, unsupported OS...) is silently ignored: this must
//! never stop the game from opening.

use crate::config::{BUILD_VERSION, save_dir};
use std::path::{Path, PathBuf};
use std::process::Command;

/// v3.0.1: a new name - the old Python version wrote "desktop_shortcut_made" with the same meaning, so on a PC that
/// had it the new game never made its own shortcut.
const MARKER_NAME: &str = "desktop_shortcut_v3";

fn marker_path() -> PathBuf {
    save_dir().join(MARKER_NAME)
}

fn desktop_dir() -> Option<PathBuf> {
    #[cfg(windows)]
    {
        // the desktop may have been moved (OneDrive, another language...): the shell knows the real one
        if let Some(p) = dirs::desktop_dir().filter(|p| p.is_dir()) {
            return Some(p);
        }
    }
    let home = std::env::var_os(if cfg!(windows) { "USERPROFILE" } else { "HOME" }).map(PathBuf::from)?;
    let p = home.join("Desktop");
    p.is_dir().then_some(p)
}

#[cfg(windows)]
fn make_shortcut(target: &Path, _desktop: &Path) {
    windows_shortcuts(target);
}

/// v3.0.3: the "Lucky Verities" shortcuts (Desktop and Start menu) are written straight as .lnk files. Before, a
/// hidden PowerShell made them, and antivirus (Avast) took the game for a virus because of that.
#[cfg(windows)]
fn windows_shortcuts(target: &Path) {
    let dir = target.parent().unwrap_or(target);
    let start_menu = std::env::var_os("APPDATA").map(|a| PathBuf::from(a).join("Microsoft").join("Windows").join("Start Menu").join("Programs"));
    for folder in [desktop_dir(), start_menu].into_iter().flatten() {
        if !folder.is_dir() {
            continue;
        }
        let Ok(mut link) = mslnk::ShellLink::new(target) else { return };
        link.set_working_dir(Some(dir.display().to_string()));
        link.set_icon_location(Some(target.display().to_string()));
        link.set_name(Some("Lucky Verities".into()));
        let _ = link.create_lnk(folder.join("Lucky Verities.lnk"));
    }
}

/// Where the game installs itself on Windows (no admin needed): %LOCALAPPDATA%\Programs\Lucky Verities.
#[cfg(windows)]
fn windows_install_path() -> Option<PathBuf> {
    let base = std::env::var_os("LOCALAPPDATA").map(PathBuf::from)?;
    Some(base.join("Programs").join("Lucky Verities").join("Lucky Verities.exe"))
}

/// Windows: the file people download ("Lucky-Verities-Windows.exe", in Downloads) puts a copy of itself at
/// windows_install_path ("Lucky Verities.exe") and makes the Desktop and Start menu shortcuts to it, then just goes
/// on running (v3.0.3: it no longer opens that copy and closes - antivirus blocked the copy and the game "closed").
/// Only when there's no installed copy yet; release builds only; any failure is ignored.
#[cfg(windows)]
pub fn install_windows() {
    if BUILD_VERSION == "dev" || std::env::var_os("LUCKY_VERITIES_NO_INSTALL").is_some() {
        return;
    }
    let (Ok(exe), Some(target)) = (std::env::current_exe(), windows_install_path()) else { return };
    let same = |a: &Path, b: &Path| a.to_string_lossy().to_lowercase() == b.to_string_lossy().to_lowercase();
    let canon = |p: &Path| std::fs::canonicalize(p).unwrap_or_else(|_| p.to_path_buf());
    if target.exists() || same(&canon(&exe), &canon(&target)) {
        return; // already installed (the updater keeps that copy up to date) / this is the installed copy
    }
    let Some(dir) = target.parent() else { return };
    if std::fs::create_dir_all(dir).is_err() || std::fs::copy(&exe, &target).is_err() {
        return;
    }
    windows_shortcuts(&target);
    let _ = std::fs::create_dir_all(save_dir());
    let _ = std::fs::write(marker_path(), "1");
}

#[cfg(not(windows))]
pub fn install_windows() {}

#[cfg(target_os = "macos")]
fn make_shortcut(target: &Path, desktop: &Path) {
    // inside a .app the executable is deep down (Contents/MacOS/...): the alias points at the .app itself
    let Some(app) = target.ancestors().find(|p| p.extension().is_some_and(|e| e == "app")) else { return };
    if desktop.join("Lucky Verities").exists() {
        return;
    }
    let script = format!(
        "tell application \"Finder\"\n  make alias file to POSIX file \"{}\" at desktop\n  set name of result to \"Lucky Verities\"\nend tell",
        app.display()
    );
    let _ = Command::new("osascript").arg("-e").arg(script).status();
}

#[cfg(all(unix, not(target_os = "macos")))]
fn make_shortcut(target: &Path, desktop: &Path) {
    // a .desktop file, marked executable (otherwise the desktop shows it as "untrusted" and won't open it)
    let link = desktop.join("Lucky Verities.desktop");
    if link.exists() {
        return;
    }
    let content = desktop_entry(target);
    if std::fs::write(&link, content).is_ok() {
        use std::os::unix::fs::PermissionsExt;
        let _ = std::fs::set_permissions(&link, std::fs::Permissions::from_mode(0o755));
        let _ = Command::new("gio").arg("set").arg(&link).arg("metadata::trusted").arg("true").status();
    }
}

/// The icon, written out of the executable (the icons are built into it) to ~/.local/share/icons.
#[cfg(all(unix, not(target_os = "macos")))]
fn linux_icon() -> Option<PathBuf> {
    let dir = std::env::var_os("XDG_DATA_HOME").map(PathBuf::from).or_else(|| std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".local").join("share")))?;
    let icon = dir.join("icons").join("lucky-verities.png");
    if !icon.exists() {
        let data = crate::assets::read("icons/verity.png")?;
        std::fs::create_dir_all(icon.parent()?).ok()?;
        std::fs::write(&icon, data).ok()?;
    }
    Some(icon)
}

#[cfg(all(unix, not(target_os = "macos")))]
fn desktop_entry(target: &Path) -> String {
    let icon = linux_icon().map(|p| p.display().to_string()).unwrap_or_default();
    format!(
        "[Desktop Entry]\nType=Application\nName=Lucky Verities\nComment=Roll for Verities\nExec=\"{}\"\nIcon={}\nTerminal=false\nCategories=Game;\n",
        target.display(),
        icon
    )
}

/// Linux: puts the game in the app menu (~/.local/share/applications) with its icon, and keeps it pointing at this
/// executable (it may have moved). Cheap: only writes when something changed. Every start of a release build.
#[cfg(all(unix, not(target_os = "macos")))]
pub fn ensure_app_menu_entry() {
    if BUILD_VERSION == "dev" || std::env::var_os("LUCKY_VERITIES_NO_SHORTCUT").is_some() {
        return;
    }
    let Ok(target) = std::env::current_exe() else { return };
    let Some(dir) = std::env::var_os("XDG_DATA_HOME").map(PathBuf::from).or_else(|| std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".local").join("share"))) else { return };
    let apps = dir.join("applications");
    let file = apps.join("lucky-verities.desktop");
    let content = desktop_entry(&target);
    if std::fs::read_to_string(&file).ok().as_deref() == Some(content.as_str()) {
        return;
    }
    let _ = std::fs::create_dir_all(&apps);
    let _ = std::fs::write(&file, content);
}

#[cfg(not(all(unix, not(target_os = "macos"))))]
pub fn ensure_app_menu_entry() {}

/// Call once at start (on a separate thread). Only does something in a release build, and only the first time.
pub fn create_desktop_shortcut_once() {
    ensure_app_menu_entry();
    if BUILD_VERSION == "dev" || std::env::var_os("LUCKY_VERITIES_NO_SHORTCUT").is_some() || marker_path().exists() {
        return;
    }
    if let (Some(desktop), Ok(target)) = (desktop_dir(), std::env::current_exe()) {
        make_shortcut(&target, &desktop);
    }
    let _ = std::fs::create_dir_all(save_dir());
    let _ = std::fs::write(marker_path(), "1");
}

#[cfg(all(test, unix, not(target_os = "macos")))]
mod tests {
    use super::*;

    #[test]
    fn menu_entry_has_the_icon_and_the_game() {
        let dir = std::env::temp_dir().join(format!("lv-menu-test-{}", std::process::id()));
        // SAFETY: only this test reads XDG_DATA_HOME
        unsafe { std::env::set_var("XDG_DATA_HOME", &dir) };
        let entry = desktop_entry(Path::new("/opt/lv/LuckyVerities"));
        let icon = dir.join("icons").join("lucky-verities.png");
        assert!(icon.exists() && std::fs::metadata(&icon).unwrap().len() > 1000);
        assert!(entry.contains("Exec=\"/opt/lv/LuckyVerities\""));
        assert!(entry.contains(&format!("Icon={}", icon.display())));
        assert!(entry.contains("Categories=Game;"));
        let _ = std::fs::remove_dir_all(&dir);
    }
}
