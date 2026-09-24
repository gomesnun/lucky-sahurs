//! Game files (icons/, sounds/, fonts/). A copy is built into the executable (build.rs), so the
//! game is a single file like the PyInstaller --onefile build; a file with the same name next to
//! the game still wins, so players can swap an icon or a sound.

use crate::config::{asset_roots, game_dir};
use std::borrow::Cow;
use std::path::PathBuf;

include!(concat!(env!("OUT_DIR"), "/embedded_assets.rs"));

fn roots() -> Vec<PathBuf> {
    let mut v = vec![game_dir().to_path_buf()];
    v.extend(asset_roots().iter().cloned());
    v
}

/// Built-in copy of "<folder>/<name>" ("icons/pets/amity.png").
pub fn embedded(rel: &str) -> Option<&'static [u8]> {
    EMBEDDED.iter().find(|(p, _)| *p == rel).map(|(_, b)| *b)
}

/// Contents of "<folder>/<name>": a file next to the game first, then the built-in copy.
pub fn read(rel: &str) -> Option<Cow<'static, [u8]>> {
    for r in roots() {
        let p = r.join(rel);
        if p.is_file() {
            if let Ok(b) = std::fs::read(&p) {
                return Some(Cow::Owned(b));
            }
        }
    }
    embedded(rel).map(Cow::Borrowed)
}

/// Built-in file names directly inside <folder>, sorted.
pub fn embedded_names(folder: &str) -> Vec<&'static str> {
    let prefix = format!("{folder}/");
    let mut v: Vec<&'static str> = EMBEDDED
        .iter()
        .filter_map(|(p, _)| p.strip_prefix(prefix.as_str()))
        .filter(|n| !n.contains('/'))
        .collect();
    v.sort();
    v
}
