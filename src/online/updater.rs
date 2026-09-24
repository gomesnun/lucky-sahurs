//! Automatic updates (online/updater.py): only in builds made by GitHub Actions from a vX.Y.Z tag.
//!
//! 1. On start (and every 30 minutes) ask GitHub for the latest Release of UPDATE_REPO.
//! 2. If its number is HIGHER than BUILD_VERSION, the "New version" screen shows up.
//! 3. Updating = download this system's file, check it came whole (size + SHA-256), swap the
//!    program for the new one and open it again.
//!
//! Never locks anyone out: no internet / GitHub down / Release without this system's file => no
//! prompt; "dev" builds never ask; only ever updates to a HIGHER version, so no update loops.
//! The Release asset names are the same ones the Python (PyInstaller) builds used, so those
//! builds update straight into this one.

use crate::config::{BUILD_VERSION, UPDATE_REPO};
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::Duration;

pub const API_BASE: &str = "https://api.github.com";
const API_TIMEOUT: f64 = 10.0;
const DOWNLOAD_TIMEOUT: f64 = 30.0;
const USER_AGENT: &str = "LuckyVerities-Updater";

// Names of each Release's files (must match .github/workflows/build.yml).
pub const ASSET_WINDOWS: &str = "Lucky-Verities-Windows.exe";
pub const ASSET_LINUX: &str = "Lucky-Verities-Linux.tar.gz";
pub const ASSET_MAC_ARM: &str = "Lucky-Verities-macOS-AppleSilicon.zip";
pub const ASSET_MAC_INTEL: &str = "Lucky-Verities-macOS-Intel.zip";
pub const LINUX_BINARY_NAME: &str = "LuckyVerities"; // the file inside the .tar.gz
pub const MAC_APP_NAME: &str = "Lucky Verities.app"; // the folder inside the .zip
const WORK_PREFIX: &str = ".lucky-update-"; // temporary work folders (removed on start)

/// The update failed. code: "net" | "corrupt" | "perm" | "other".
#[derive(Debug, Clone)]
pub struct UpdateError {
    pub code: &'static str,
    pub detail: String,
}

fn uerr(code: &'static str, detail: impl ToString) -> UpdateError {
    UpdateError { code, detail: detail.to_string() }
}

// ---------------------------------------------------------------- versions
/// "v1.2.3" -> (1, 2, 3); "v7" -> (7, 0, 0); "dev" or junk -> None.
pub fn parse_version(text: &str) -> Option<(u64, u64, u64)> {
    let t = text.trim();
    let t = t.strip_prefix('v').unwrap_or(t);
    let parts: Vec<&str> = t.split('.').collect();
    if parts.is_empty() || parts.len() > 3 {
        return None;
    }
    let mut v = [0u64; 3];
    for (i, p) in parts.iter().enumerate() {
        if p.is_empty() || !p.bytes().all(|b| b.is_ascii_digit()) {
            return None;
        }
        v[i] = p.parse().ok()?;
    }
    Some((v[0], v[1], v[2]))
}

/// True only if both versions are valid and `latest` is HIGHER than `current`.
pub fn is_newer(latest: &str, current: &str) -> bool {
    matches!((parse_version(latest), parse_version(current)), (Some(a), Some(b)) if a > b)
}

/// True only in a build made from a vX.Y.Z tag. To turn it off (tests): LUCKY_VERITIES_NO_UPDATE=1
/// (the old LUCKY_SAHURS_NO_UPDATE works too).
pub fn updates_enabled() -> bool {
    let set = |k: &str| std::env::var_os(k).is_some_and(|v| !v.is_empty());
    if set("LUCKY_VERITIES_NO_UPDATE") || set("LUCKY_SAHURS_NO_UPDATE") {
        return false;
    }
    parse_version(BUILD_VERSION).is_some()
}

/// The Release file that fits this system.
pub fn asset_name() -> &'static str {
    if cfg!(target_os = "windows") {
        ASSET_WINDOWS
    } else if cfg!(target_os = "macos") {
        if cfg!(target_arch = "aarch64") { ASSET_MAC_ARM } else { ASSET_MAC_INTEL }
    } else {
        ASSET_LINUX
    }
}

// ---------------------------------------------------------------- asking GitHub
#[derive(Debug, Clone)]
pub struct ReleaseInfo {
    pub tag: String,
    #[allow(dead_code)]
    pub name: String,
    pub url: String,
    pub size: u64,
    pub digest: String,
    pub page: String,
}

/// LUCKY_VERITIES_UPDATE_API replaces https://api.github.com (tests against a local server; plain
/// http is only accepted then).
fn api_base() -> (String, bool) {
    match std::env::var("LUCKY_VERITIES_UPDATE_API") {
        Ok(v) if !v.is_empty() => (v.trim_end_matches('/').to_string(), true),
        _ => (API_BASE.to_string(), false),
    }
}

fn agent(timeout: f64) -> ureq::Agent {
    let t = Some(Duration::from_secs_f64(timeout));
    ureq::Agent::config_builder()
        .timeout_connect(t)
        .timeout_send_request(t)
        .timeout_recv_response(t)
        .http_status_as_error(false)
        .user_agent(USER_AGENT)
        .build()
        .into()
}

/// From GitHub's answer (/releases/latest), the new version for this system, or None.
pub fn pick_release(data: &Value, current: &str, allow_http: bool) -> Option<ReleaseInfo> {
    let obj = data.as_object()?;
    let truthy = |k: &str| obj.get(k).is_some_and(|v| !(v.is_null() || v == &Value::Bool(false)));
    if truthy("draft") || truthy("prerelease") {
        return None;
    }
    let tag = obj.get("tag_name").and_then(|v| v.as_str()).unwrap_or("").to_string();
    if !is_newer(&tag, current) {
        return None;
    }
    let wanted = asset_name();
    for a in obj.get("assets").and_then(|v| v.as_array()).into_iter().flatten() {
        let Some(a) = a.as_object() else { continue };
        if a.get("name").and_then(|v| v.as_str()) != Some(wanted) {
            continue;
        }
        let url = a.get("browser_download_url").and_then(|v| v.as_str()).unwrap_or("").to_string();
        let state = a.get("state").and_then(|v| v.as_str()).unwrap_or("uploaded");
        if state != "uploaded" || !(url.starts_with("https://") || (allow_http && url.starts_with("http://"))) {
            continue;
        }
        return Some(ReleaseInfo {
            tag,
            name: wanted.to_string(),
            url,
            size: a.get("size").and_then(|v| v.as_u64()).unwrap_or(0),
            digest: a.get("digest").and_then(|v| v.as_str()).unwrap_or("").to_string(),
            page: obj.get("html_url").and_then(|v| v.as_str()).unwrap_or("").to_string(),
        });
    }
    None // the Release doesn't have this system's file yet (build still running): don't block
}

/// Ok(None) = on the latest version (or no way to tell). Err("net") if GitHub didn't answer.
pub fn fetch_latest() -> Result<Option<ReleaseInfo>, UpdateError> {
    let (base, allow_http) = api_base();
    let url = format!("{}/repos/{}/releases/latest", base, UPDATE_REPO);
    let resp = agent(API_TIMEOUT)
        .get(&url)
        .header("Accept", "application/vnd.github+json")
        .call()
        .map_err(|e| uerr("net", e))?;
    let status = resp.status().as_u16();
    if status == 404 {
        return Ok(None); // the repository has no Releases yet
    }
    if status >= 400 {
        return Err(uerr("net", format!("HTTP {}", status))); // 403 = GitHub's limit, 5xx = down...
    }
    let text = resp.into_body().read_to_string().map_err(|e| uerr("net", e))?;
    let data: Value = serde_json::from_str(&text).map_err(|e| uerr("net", e))?;
    Ok(pick_release(&data, BUILD_VERSION, allow_http))
}

// ---------------------------------------------------------------- downloading
pub type Progress = Arc<Mutex<f64>>;

fn set_progress(p: &Option<Progress>, v: f64) {
    if let Some(p) = p {
        if let Ok(mut g) = p.lock() {
            *g = v;
        }
    }
}

fn is_perm_error(e: &std::io::Error) -> bool {
    use std::io::ErrorKind::*;
    matches!(e.kind(), PermissionDenied | StorageFull | ReadOnlyFilesystem)
}

/// Downloads info.url to `dest` (first into a .part), checks size and SHA-256 and only then puts
/// the file in place. `progress` goes 0..1 while downloading.
pub fn download(info: &ReleaseInfo, dest: &Path, progress: &Option<Progress>) -> Result<(), UpdateError> {
    let part = PathBuf::from(format!("{}.part", dest.display()));
    let mut f = std::fs::File::create(&part).map_err(|e| uerr("perm", e))?;
    let fail = |code: &'static str, d: String| {
        let _ = std::fs::remove_file(&part);
        uerr(code, d)
    };
    let resp = match agent(DOWNLOAD_TIMEOUT).get(&info.url).header("Accept", "application/octet-stream").call() {
        Ok(r) => r,
        Err(e) => return Err(fail("net", e.to_string())),
    };
    let status = resp.status().as_u16();
    if status >= 400 {
        return Err(fail("net", format!("HTTP {}", status)));
    }
    let total = resp
        .headers()
        .get("content-length")
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.parse::<u64>().ok())
        .filter(|v| *v > 0)
        .unwrap_or(info.size);
    let mut body = resp.into_body();
    let mut reader = body.as_reader();
    let mut h = Sha256::new();
    let mut done: u64 = 0;
    let mut buf = vec![0u8; 64 * 1024];
    loop {
        let n = match reader.read(&mut buf) {
            Ok(0) => break,
            Ok(n) => n,
            Err(e) if e.kind() == std::io::ErrorKind::Interrupted => continue,
            Err(e) => return Err(fail("net", e.to_string())),
        };
        if let Err(e) = f.write_all(&buf[..n]) {
            return Err(fail(if is_perm_error(&e) { "perm" } else { "net" }, e.to_string()));
        }
        h.update(&buf[..n]);
        done += n as u64;
        if total > 0 {
            set_progress(progress, (done as f64 / total as f64).min(1.0));
        }
    }
    drop(f);
    if total > 0 && done < total {
        // the connection dropped halfway: a network problem, not a damaged file
        return Err(fail("net", format!("descarga incompleta ({} de {})", done, total)));
    }
    if info.size > 0 && done != info.size {
        return Err(fail("corrupt", format!("tamanho {} em vez de {}", done, info.size)));
    }
    if let Some(want) = info.digest.strip_prefix("sha256:") {
        let got: String = h.finalize().iter().map(|b| format!("{:02x}", b)).collect();
        if got != want.trim().to_lowercase() {
            return Err(fail("corrupt", "sha256 diferente".into()));
        }
    }
    if done == 0 {
        return Err(fail("corrupt", "ficheiro vazio".into()));
    }
    std::fs::rename(&part, dest).map_err(|e| fail("perm", e.to_string()))?;
    set_progress(progress, 1.0);
    Ok(())
}

/// Checks the first bytes (an HTML error page never passes for an .exe / .zip / .tar.gz).
fn check_magic(path: &Path, magic: &[u8]) -> Result<(), UpdateError> {
    let mut f = std::fs::File::open(path).map_err(|e| uerr("other", e))?;
    let mut head = vec![0u8; magic.len()];
    let ok = f.read_exact(&mut head).is_ok() && head == magic;
    if !ok {
        return Err(uerr("corrupt", "formato inesperado"));
    }
    Ok(())
}

fn unique_name() -> String {
    let t = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_nanos()).unwrap_or(0);
    format!("{}{}-{}", WORK_PREFIX, std::process::id(), t % 1_000_000_000)
}

/// The program's folder has to allow writing (otherwise the game can't swap itself).
fn need_writable(folder: &Path) -> Result<(), UpdateError> {
    let tmp = folder.join(unique_name());
    std::fs::write(&tmp, b"").map_err(|e| uerr("perm", e))?;
    let _ = std::fs::remove_file(&tmp);
    Ok(())
}

fn make_work_dir(folder: &Path) -> Result<PathBuf, UpdateError> {
    let work = folder.join(unique_name());
    std::fs::create_dir(&work).map_err(|e| uerr("perm", e))?;
    Ok(work)
}

fn current_exe() -> Result<PathBuf, UpdateError> {
    let exe = std::env::current_exe().map_err(|e| uerr("other", e))?;
    Ok(std::fs::canonicalize(&exe).unwrap_or(exe))
}

// ---------------------------------------------------------------- preparing the swap (background thread)
#[derive(Debug, Clone)]
pub enum Plan {
    Windows { exe: PathBuf, new: PathBuf },
    Linux { exe: PathBuf },
    Mac { app: PathBuf, new: PathBuf, work: PathBuf },
}

/// Downloads and prepares the swap. Returns a plan for launch().
pub fn prepare(info: &ReleaseInfo, progress: &Option<Progress>) -> Result<Plan, UpdateError> {
    if cfg!(target_os = "windows") {
        prepare_windows(info, progress)
    } else if cfg!(target_os = "macos") {
        prepare_mac(info, progress)
    } else {
        prepare_linux(info, progress)
    }
}

fn prepare_windows(info: &ReleaseInfo, progress: &Option<Progress>) -> Result<Plan, UpdateError> {
    let exe = current_exe()?;
    need_writable(exe.parent().unwrap_or(Path::new(".")))?;
    let new = PathBuf::from(format!("{}.update", exe.display()));
    download(info, &new, progress)?;
    check_magic(&new, b"MZ")?;
    Ok(Plan::Windows { exe, new })
}

fn prepare_linux(info: &ReleaseInfo, progress: &Option<Progress>) -> Result<Plan, UpdateError> {
    let exe = current_exe()?;
    let folder = exe.parent().unwrap_or(Path::new(".")).to_path_buf();
    need_writable(&folder)?;
    let work = make_work_dir(&folder)?; // in the same folder: the swap is atomic
    let r = (|| {
        let archive = work.join("update.tar.gz");
        download(info, &archive, progress)?;
        check_magic(&archive, b"\x1f\x8b")?;
        let new = work.join(LINUX_BINARY_NAME);
        extract_linux_binary(&archive, &new)?;
        check_magic(&new, b"\x7fELF")?;
        std::fs::rename(&new, &exe).map_err(|e| uerr("perm", e)) // replacing a running program is allowed on Linux
    })();
    let _ = std::fs::remove_dir_all(&work);
    r?;
    Ok(Plan::Linux { exe })
}

fn extract_linux_binary(archive: &Path, new: &Path) -> Result<(), UpdateError> {
    let f = std::fs::File::open(archive).map_err(|e| uerr("other", e))?;
    let mut tar = tar::Archive::new(flate2::read::GzDecoder::new(f));
    let entries = tar.entries().map_err(|e| uerr("corrupt", e))?;
    for entry in entries {
        let mut entry = entry.map_err(|e| uerr("corrupt", e))?;
        let path = entry.path().map_err(|e| uerr("corrupt", e))?.to_path_buf();
        if path.as_os_str() != LINUX_BINARY_NAME {
            continue;
        }
        if !entry.header().entry_type().is_file() || entry.header().size().unwrap_or(0) == 0 {
            return Err(uerr("corrupt", "conteúdo inesperado"));
        }
        let mut out = std::fs::File::create(new).map_err(|e| uerr("perm", e))?;
        std::io::copy(&mut entry, &mut out).map_err(|e| uerr("corrupt", e))?;
        drop(out);
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(new, std::fs::Permissions::from_mode(0o755)).map_err(|e| uerr("perm", e))?;
        }
        return Ok(());
    }
    Err(uerr("corrupt", format!("\"{}\" not found in archive", LINUX_BINARY_NAME)))
}

fn mac_app_path(exe: &Path) -> Result<PathBuf, UpdateError> {
    let s = exe.to_string_lossy();
    match s.find(".app/Contents/") {
        Some(i) => Ok(PathBuf::from(&s[..i + 4])),
        None => Err(uerr("perm", "não é uma .app")),
    }
}

fn prepare_mac(info: &ReleaseInfo, progress: &Option<Progress>) -> Result<Plan, UpdateError> {
    let exe = current_exe()?;
    let app = mac_app_path(&exe)?;
    let parent = app.parent().unwrap_or(Path::new("/")).to_path_buf();
    need_writable(&parent)?; // fails for "translocated" apps (opened from a zip / Downloads)
    let work = make_work_dir(&parent)?;
    let r = (|| {
        let archive = work.join("update.zip");
        download(info, &archive, progress)?;
        check_magic(&archive, b"PK")?;
        let out = work.join("new");
        std::fs::create_dir(&out).map_err(|e| uerr("perm", e))?;
        // "ditto" (from macOS itself) keeps the .app's permissions and symlinks
        let r = std::process::Command::new("ditto").arg("-x").arg("-k").arg(&archive).arg(&out).output().map_err(|e| uerr("other", e))?;
        if !r.status.success() {
            let msg: String = String::from_utf8_lossy(&r.stderr).chars().take(200).collect();
            return Err(uerr("corrupt", msg));
        }
        let new_app = out.join(MAC_APP_NAME);
        if !new_app.is_dir() {
            return Err(uerr("corrupt", "a .app não veio no zip"));
        }
        Ok(new_app)
    })();
    match r {
        Ok(new) => Ok(Plan::Mac { app, new, work }),
        Err(e) => {
            let _ = std::fs::remove_dir_all(&work);
            Err(e)
        }
    }
}

// ---------------------------------------------------------------- swap and reopen (after the game closes)
// .bat: keeps trying to swap the program until the game closes (Windows can't replace a running
// .exe) and reopens it. %~1 = current program, %~2 = new program.
const WINDOWS_SCRIPT: &str = "@echo off\r\n\
set /a tries=0\r\n\
:retry\r\n\
move /y \"%~2\" \"%~1\" >nul 2>&1\r\n\
if not exist \"%~2\" goto done\r\n\
set /a tries+=1\r\n\
if %tries% geq 120 goto done\r\n\
ping -n 2 127.0.0.1 >nul\r\n\
goto retry\r\n\
:done\r\n\
cd /d \"%~dp1\"\r\n\
start \"\" \"%~1\"\r\n\
(goto) 2>nul & del \"%~f0\"\r\n";

// Linux: waits for the game to close and opens the (already swapped) program. $1 = pid, $2 = program
const LINUX_SCRIPT: &str = "while kill -0 \"$1\" 2>/dev/null; do sleep 0.2; done; cd \"$(dirname \"$2\")\" && exec \"$2\"";

// macOS: waits for the game to close, swaps the .app (rolling back if it fails) and opens it.
// $1 = pid, $2 = current .app, $3 = new .app, $4 = work folder
const MAC_SCRIPT: &str = "while kill -0 \"$1\" 2>/dev/null; do sleep 0.3; done; \
if mv \"$2\" \"$2.old-update\"; then \
if mv \"$3\" \"$2\"; then rm -rf \"$2.old-update\"; else mv \"$2.old-update\" \"$2\"; fi; \
fi; \
rm -rf \"$4\"; \
open -n \"$2\"";

fn detached(cmd: &mut std::process::Command) -> std::io::Result<()> {
    use std::process::Stdio;
    cmd.stdin(Stdio::null()).stdout(Stdio::null()).stderr(Stdio::null());
    #[cfg(unix)]
    {
        use std::os::unix::process::CommandExt;
        cmd.process_group(0);
    }
    cmd.spawn().map(|_| ())
}

/// Starts the helper that swaps the program after the game closes and reopens it. Does NOT close
/// the game: the caller saves everything and exits right after.
pub fn launch(plan: &Plan) -> Result<(), UpdateError> {
    let pid = std::process::id().to_string();
    let r = match plan {
        Plan::Windows { exe, new } => {
            let folder = std::env::temp_dir().join(unique_name().trim_start_matches('.'));
            std::fs::create_dir_all(&folder).map_err(|e| uerr("perm", e))?;
            let bat = folder.join("update.bat");
            std::fs::write(&bat, WINDOWS_SCRIPT).map_err(|e| uerr("perm", e))?;
            let mut cmd = std::process::Command::new("cmd.exe");
            #[cfg(windows)]
            {
                use std::os::windows::process::CommandExt;
                cmd.raw_arg(format!("/d /s /c \"\"{}\" \"{}\" \"{}\"\"", bat.display(), exe.display(), new.display()));
                // CREATE_NO_WINDOW | CREATE_NEW_PROCESS_GROUP
                cmd.creation_flags(0x0800_0000 | 0x0000_0200);
            }
            #[cfg(not(windows))]
            let _ = (exe, new);
            detached(&mut cmd)
        }
        Plan::Linux { exe } => detached(std::process::Command::new("/bin/sh").arg("-c").arg(LINUX_SCRIPT).arg("sh").arg(&pid).arg(exe)),
        Plan::Mac { app, new, work } => detached(
            std::process::Command::new("/bin/sh").arg("-c").arg(MAC_SCRIPT).arg("sh").arg(&pid).arg(app).arg(new).arg(work),
        ),
    };
    r.map_err(|e| uerr("other", e))
}

/// On start: removes leftovers of updates that stopped halfway (.update / .part files and work folders).
pub fn cleanup_leftovers() {
    let Ok(exe) = current_exe() else { return };
    let mut folders = vec![exe.parent().unwrap_or(Path::new(".")).to_path_buf()];
    if cfg!(target_os = "macos") {
        if let Ok(app) = mac_app_path(&exe) {
            if let Some(p) = app.parent() {
                folders.push(p.to_path_buf());
            }
        }
    }
    for suffix in [".update", ".update.part"] {
        let _ = std::fs::remove_file(format!("{}{}", exe.display(), suffix));
    }
    for folder in folders {
        let Ok(rd) = std::fs::read_dir(&folder) else { continue };
        for e in rd.flatten() {
            if e.file_name().to_string_lossy().starts_with(WORK_PREFIX) {
                let p = e.path();
                if p.is_dir() {
                    let _ = std::fs::remove_dir_all(&p);
                } else {
                    let _ = std::fs::remove_file(&p);
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn versions() {
        assert_eq!(parse_version("v1.2.3"), Some((1, 2, 3)));
        assert_eq!(parse_version("v7"), Some((7, 0, 0)));
        assert_eq!(parse_version("2.5"), Some((2, 5, 0)));
        assert_eq!(parse_version("dev"), None);
        assert_eq!(parse_version("v1.2.3.4"), None);
        assert_eq!(parse_version("v1..2"), None);
        assert!(is_newer("v2.5.0", "v2.4.1"));
        assert!(!is_newer("v2.4.1", "v2.4.1"));
        assert!(!is_newer("v2.5.0", "dev"));
    }
}
