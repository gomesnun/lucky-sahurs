//! Cartoon-style colours and border widths, with light / dark palettes switchable at runtime.

use crate::gfx::Color;
use std::cell::Cell;

pub const WHITE: Color = Color::rgb(255, 255, 255);
pub const BLACK: Color = Color::rgb(12, 12, 14);
pub const BORDER_W: i32 = 4;
pub const BORDER_W_SMALL: i32 = 3;
pub const GOOD: Color = Color::rgb(100, 225, 130);
pub const BAD: Color = Color::rgb(235, 95, 95);
pub const GOLD_BORDER: Color = Color::rgb(255, 205, 60);
pub const DIAMOND_BORDER: Color = Color::rgb(150, 235, 255);
pub const PAUSED_RED: Color = Color::rgb(225, 45, 45);

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Palette {
    pub bg_top: Color,
    pub bg_bottom: Color,
    pub outline: Color,
    pub panel: Color,
    pub panel_light: Color,
    pub panel_lighter: Color,
    pub accent: Color,
    pub accent_hover: Color,
    pub grey: Color,
    pub grey_dim: Color,
}

pub const LIGHT: Palette = Palette {
    bg_top: Color::rgb(246, 243, 236),
    bg_bottom: Color::rgb(226, 222, 212),
    outline: Color::rgb(8, 8, 10),
    panel: Color::rgb(34, 34, 37),
    panel_light: Color::rgb(52, 52, 57),
    panel_lighter: Color::rgb(76, 76, 83),
    accent: Color::rgb(255, 184, 48),
    accent_hover: Color::rgb(255, 212, 110),
    grey: Color::rgb(200, 200, 208),
    grey_dim: Color::rgb(152, 152, 160),
};

pub const DARK: Palette = Palette {
    bg_top: Color::rgb(23, 24, 39),
    bg_bottom: Color::rgb(12, 13, 21),
    outline: Color::rgb(6, 6, 9),
    panel: Color::rgb(41, 42, 59),
    panel_light: Color::rgb(59, 61, 83),
    panel_lighter: Color::rgb(82, 85, 110),
    accent: Color::rgb(255, 184, 48),
    accent_hover: Color::rgb(255, 212, 110),
    grey: Color::rgb(205, 208, 226),
    grey_dim: Color::rgb(146, 149, 172),
};

thread_local! {
    static PAL: Cell<Palette> = const { Cell::new(LIGHT) };
    static DARK_MODE: Cell<bool> = const { Cell::new(false) };
}

#[inline]
pub fn pal() -> Palette {
    PAL.with(|p| p.get())
}
#[inline]
pub fn dark_mode() -> bool {
    DARK_MODE.with(|d| d.get())
}

// current-palette accessors (Python modules read the synced globals at call time)
#[inline]
pub fn bg_top() -> Color {
    pal().bg_top
}
#[inline]
pub fn bg_bottom() -> Color {
    pal().bg_bottom
}
#[inline]
pub fn outline() -> Color {
    pal().outline
}
#[inline]
pub fn panel() -> Color {
    pal().panel
}
#[inline]
pub fn panel_light() -> Color {
    pal().panel_light
}
#[inline]
pub fn panel_lighter() -> Color {
    pal().panel_lighter
}
#[inline]
pub fn accent() -> Color {
    pal().accent
}
#[inline]
pub fn accent_hover() -> Color {
    pal().accent_hover
}
#[inline]
pub fn grey() -> Color {
    pal().grey
}
#[inline]
pub fn grey_dim() -> Color {
    pal().grey_dim
}

pub const THEME_MODES: [&str; 3] = ["system", "dark", "light"];
pub const DEFAULT_THEME_MODE: &str = "system";
pub const SYSTEM_LABEL: &str = if cfg!(target_os = "windows") { "Windows default" } else { "System default" };

/// True if the desktop prefers a dark theme (GNOME color-scheme); `default` when unknown.
pub fn system_prefers_dark(default: bool) -> bool {
    #[cfg(target_os = "macos")]
    {
        if let Ok(out) = std::process::Command::new("defaults").args(["read", "-g", "AppleInterfaceStyle"]).output() {
            return String::from_utf8_lossy(&out.stdout).to_lowercase().contains("dark");
        }
        return default;
    }
    #[cfg(not(target_os = "macos"))]
    {
        let out = std::process::Command::new("gsettings")
            .args(["get", "org.gnome.desktop.interface", "color-scheme"])
            .stderr(std::process::Stdio::null())
            .output();
        if let Ok(out) = out {
            let text = String::from_utf8_lossy(&out.stdout).to_lowercase();
            if text.contains("dark") {
                return true;
            }
            if text.contains("light") || text.contains("default") {
                return false;
            }
        }
        default
    }
}

pub fn resolve_dark(mode: &str) -> bool {
    match mode {
        "dark" => true,
        "light" => false,
        _ => system_prefers_dark(true),
    }
}

pub fn set_dark(dark: bool) {
    DARK_MODE.with(|d| d.set(dark));
    PAL.with(|p| p.set(if dark { DARK } else { LIGHT }));
}
