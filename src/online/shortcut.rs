//! Desktop shortcut, made by itself the first time the packaged game runs (online/shortcut.py).
//!
//! Happens only once (marked with a file in the saves folder) and only in a release build - a dev build never
//! creates anything. Any failure (no desktop, no permission, unsupported OS...) is silently ignored: this must
//! never stop the game from opening.

use crate::config::{BUILD_VERSION, save_dir};
use std::path::{Path, PathBuf};
use std::process::Command;

const MARKER_NAME: &str = "desktop_shortcut_made";

fn marker_path() -> PathBuf {
    save_dir().join(MARKER_NAME)
}

fn desktop_dir() -> Option<PathBuf> {
    let home = std::env::var_os(if cfg!(windows) { "USERPROFILE" } else { "HOME" }).map(PathBuf::from)?;
    #[cfg(windows)]
    {
        // the desktop may have been moved (OneDrive, etc.): ask the shell for the real one
        let out = Command::new("powershell")
            .args(["-NoProfile", "-Command", "[Environment]::GetFolderPath('Desktop')"])
            .output()
            .ok();
        if let Some(o) = out {
            let p = PathBuf::from(String::from_utf8_lossy(&o.stdout).trim().to_string());
            if p.is_dir() {
                return Some(p);
            }
        }
    }
    let p = home.join("Desktop");
    p.is_dir().then_some(p)
}

#[cfg(windows)]
fn make_shortcut(target: &Path, desktop: &Path) {
    // a .lnk through a small VBS script (WScript.Shell)
    let link = desktop.join("Lucky Verities.lnk");
    if link.exists() {
        return;
    }
    let dir = target.parent().unwrap_or(target);
    let vbs = format!(
        "Set oWS = WScript.CreateObject(\"WScript.Shell\")\nSet oLink = oWS.CreateShortcut(\"{}\")\noLink.TargetPath = \"{}\"\noLink.WorkingDirectory = \"{}\"\noLink.IconLocation = \"{}, 0\"\noLink.Description = \"Lucky Verities\"\noLink.Save\n",
        link.display(),
        target.display(),
        dir.display(),
        target.display()
    );
    let path = std::env::temp_dir().join(format!("lv-shortcut-{}.vbs", std::process::id()));
    if std::fs::write(&path, vbs).is_ok() {
        use std::os::windows::process::CommandExt;
        let _ = Command::new("cscript").arg("//nologo").arg(&path).creation_flags(0x0800_0000).status();
        let _ = std::fs::remove_file(&path);
    }
}

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
