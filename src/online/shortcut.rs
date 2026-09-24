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
fn make_shortcut(target: &Path, _desktop: &Path) {
    windows_shortcuts(target);
}

/// Standard base64 (for PowerShell's -EncodedCommand).
#[cfg_attr(not(any(windows, test)), allow(dead_code))]
fn base64(data: &[u8]) -> String {
    const T: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::with_capacity(data.len().div_ceil(3) * 4);
    for c in data.chunks(3) {
        let n = (c[0] as u32) << 16 | (*c.get(1).unwrap_or(&0) as u32) << 8 | *c.get(2).unwrap_or(&0) as u32;
        for i in 0..4 {
            out.push(if i <= c.len() { T[(n >> (18 - 6 * i) & 63) as usize] as char } else { '=' });
        }
    }
    out
}

/// A PowerShell script that makes "Lucky Verities" shortcuts to `target` on the Desktop and in the Start menu. The
/// shell finds the real folders (OneDrive, other languages), and the script goes as UTF-16 (-EncodedCommand), so
/// paths with accents work - the old VBS couldn't handle them.
#[cfg_attr(not(any(windows, test)), allow(dead_code))]
fn shortcut_script(target: &Path) -> String {
    let q = |p: &Path| format!("'{}'", p.display().to_string().replace('\'', "''"));
    let dir = target.parent().unwrap_or(target);
    format!(
        "$w = New-Object -ComObject WScript.Shell; \
         foreach ($f in @([Environment]::GetFolderPath('Desktop'), [Environment]::GetFolderPath('Programs'))) {{ \
           if ($f) {{ $s = $w.CreateShortcut((Join-Path $f 'Lucky Verities.lnk')); $s.TargetPath = {exe}; \
           $s.WorkingDirectory = {dir}; $s.IconLocation = {exe} + ',0'; $s.Description = 'Lucky Verities'; $s.Save() }} }}",
        exe = q(target),
        dir = q(dir)
    )
}

#[cfg(windows)]
fn windows_shortcuts(target: &Path) {
    use std::os::windows::process::CommandExt;
    let utf16: Vec<u8> = shortcut_script(target).encode_utf16().flat_map(|u| u.to_le_bytes()).collect();
    let _ = Command::new("powershell")
        .args(["-NoProfile", "-NonInteractive", "-WindowStyle", "Hidden", "-EncodedCommand", &base64(&utf16)])
        .creation_flags(0x0800_0000) // no console window
        .status();
}

/// Where the game installs itself on Windows (no admin needed): %LOCALAPPDATA%\Programs\Lucky Verities.
#[cfg(windows)]
fn windows_install_path() -> Option<PathBuf> {
    let base = std::env::var_os("LOCALAPPDATA").map(PathBuf::from)?;
    Some(base.join("Programs").join("Lucky Verities").join("Lucky Verities.exe"))
}

/// v3.0.1, Windows: the file people download ("Lucky-Verities-Windows.exe", in Downloads) installs itself as
/// "Lucky Verities.exe" (see windows_install_path), makes the Desktop and Start menu shortcuts, opens that copy
/// and returns true (this one then closes). From the installed copy (or on any error) it returns false and the
/// game just starts. Release builds only.
#[cfg(windows)]
pub fn install_windows() -> bool {
    if BUILD_VERSION == "dev" || std::env::var_os("LUCKY_VERITIES_NO_INSTALL").is_some() {
        return false;
    }
    let (Ok(exe), Some(target)) = (std::env::current_exe(), windows_install_path()) else { return false };
    let same = |a: &Path, b: &Path| a.to_string_lossy().to_lowercase() == b.to_string_lossy().to_lowercase();
    let canon = |p: &Path| std::fs::canonicalize(p).unwrap_or_else(|_| p.to_path_buf());
    if same(&canon(&exe), &canon(&target)) {
        return false; // this is the installed copy
    }
    let Some(dir) = target.parent() else { return false };
    if std::fs::create_dir_all(dir).is_err() || std::fs::copy(&exe, &target).is_err() {
        return false; // e.g. the installed copy is open right now: just play this one
    }
    windows_shortcuts(&target);
    let _ = std::fs::create_dir_all(save_dir());
    let _ = std::fs::write(marker_path(), "1");
    let args: Vec<std::ffi::OsString> = std::env::args_os().skip(1).collect();
    Command::new(&target).args(args).current_dir(dir).spawn().is_ok()
}

#[cfg(not(windows))]
pub fn install_windows() -> bool {
    false
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

#[cfg(test)]
mod windows_helper_tests {
    use super::*;

    #[test]
    fn base64_matches_the_standard() {
        assert_eq!(base64(b"Man"), "TWFu");
        assert_eq!(base64(b"Ma"), "TWE=");
        assert_eq!(base64(b"M"), "TQ==");
        assert_eq!(base64(b""), "");
        assert_eq!(base64("Olá".as_bytes()), "T2zDoQ==");
    }

    #[test]
    fn shortcut_script_quotes_paths() {
        let s = shortcut_script(Path::new("C:/Users/Tomás O'Neil/AppData/Local/Programs/Lucky Verities/Lucky Verities.exe"));
        assert!(s.contains("$s.TargetPath = 'C:/Users/Tomás O''Neil/AppData/Local/Programs/Lucky Verities/Lucky Verities.exe'"));
        assert!(s.contains("GetFolderPath('Desktop')") && s.contains("GetFolderPath('Programs')"));
        assert!(s.contains("'Lucky Verities.lnk'"));
    }
}

#[cfg(test)]
#[test]
#[ignore] // prints the encoded command, to check it with a real PowerShell (see the v3.0.1 notes)
fn print_encoded_shortcut_command() {
    let utf16: Vec<u8> = shortcut_script(Path::new("C:\\Users\\Tomás O'Neil\\AppData\\Local\\Programs\\Lucky Verities\\Lucky Verities.exe")).encode_utf16().flat_map(|u| u.to_le_bytes()).collect();
    println!("ENCODED={}", base64(&utf16));
}
