//! Omarchy/pimarchy palette detection, shared by every app in the pi suite.
//!
//! This is the pitype/pifile superset variant: surface + hover + muted
//! support, candidate-path fallbacks, and derived colors when the theme file
//! omits them. Apps that only need the base four colors can ignore the extra
//! fields; apps that need more (piwrite's selection alpha work) extend it.

use std::fs;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct RgbaColor {
    pub r: f32,
    pub g: f32,
    pub b: f32,
}

impl RgbaColor {
    pub fn luminance(self) -> f32 {
        0.299 * self.r + 0.587 * self.g + 0.114 * self.b
    }
}

pub fn parse_hex_color(value: &str) -> Option<RgbaColor> {
    let value = value.trim().trim_matches(|c| c == '"' || c == '\'');
    let hex = value.strip_prefix('#').unwrap_or(value);
    let (r, g, b) = match hex.len() {
        3 => {
            let r = u8::from_str_radix(&hex[0..1].repeat(2), 16).ok()?;
            let g = u8::from_str_radix(&hex[1..2].repeat(2), 16).ok()?;
            let b = u8::from_str_radix(&hex[2..3].repeat(2), 16).ok()?;
            (r, g, b)
        }
        6 => {
            let r = u8::from_str_radix(&hex[0..2], 16).ok()?;
            let g = u8::from_str_radix(&hex[2..4], 16).ok()?;
            let b = u8::from_str_radix(&hex[4..6], 16).ok()?;
            (r, g, b)
        }
        _ => return None,
    };
    Some(RgbaColor {
        r: r as f32 / 255.0,
        g: g as f32 / 255.0,
        b: b as f32 / 255.0,
    })
}

#[derive(Debug, Clone, PartialEq)]
pub struct OmarchyPalette {
    pub dark: bool,
    pub background: String,
    pub foreground: String,
    pub accent: String,
    pub selection: String,
    /// Secondary text color: the theme's `muted`, derived when absent.
    pub muted: String,
    /// Raised panel color (sidebar, toolbar, status bar): the theme's
    /// `lighter_background` (dark) / `darker_background` (light), derived
    /// from background/foreground when absent.
    pub surface: String,
    /// Hover surface: inverse elevation of `surface`.
    pub hover: String,
}

impl OmarchyPalette {
    pub fn fallback(dark: bool) -> Self {
        if dark {
            Self {
                dark: true,
                background: "#101010".into(),
                foreground: "#eeeeee".into(),
                accent: "#5584aa".into(),
                selection: "#186a9a".into(),
                muted: "#909191".into(),
                surface: "#1c1c1c".into(),
                hover: "#2a2a2a".into(),
            }
        } else {
            Self {
                dark: false,
                background: "#ffffff".into(),
                foreground: "#222324".into(),
                accent: "#2077b2".into(),
                selection: "#2077b2".into(),
                muted: "#aeb1b5".into(),
                surface: "#f4f4f4".into(),
                hover: "#e8e8e8".into(),
            }
        }
    }

    /// Read the first theme file found among the candidate paths, falling
    /// back to the built-in palette.
    pub fn load(dark_hint: bool) -> Self {
        let mut palette = Self::fallback(dark_hint);
        for candidate in colors_candidates() {
            if let Some(loaded) = Self::from_colors_file(&candidate, palette.dark) {
                palette = loaded;
                break;
            }
        }
        palette
    }

    /// Parse a theme file. A known `mode` decides light or dark; otherwise
    /// the background's luminance does. `lighter_background`/`darker_background`
    /// map to surface/hover per mode; `muted` is taken as-is.
    pub fn from_colors_file(path: &Path, dark_hint: bool) -> Option<Self> {
        let raw = fs::read_to_string(path).ok()?;
        let mut palette = Self::fallback(dark_hint);
        let mut mode = String::new();
        let mut lighter = None;
        let mut darker = None;
        for line in raw.lines() {
            let line = line.trim();
            if line.is_empty() || line.starts_with('#') {
                continue;
            }
            let Some((key, value)) = line.split_once('=') else {
                continue;
            };
            let key = key.trim();
            let value = unquote(value.trim());
            match key {
                "mode" => mode = value,
                "background" => palette.background = value,
                "foreground" => palette.foreground = value,
                "accent" => palette.accent = value,
                "selection" => palette.selection = value,
                "muted" => palette.muted = value,
                "lighter_background" => lighter = Some(value),
                "darker_background" => darker = Some(value),
                _ => {}
            }
        }
        palette.dark = match mode.as_str() {
            "dark" => true,
            "light" => false,
            _ => parse_hex_color(&palette.background)
                .map(|bg| bg.luminance() < 0.5)
                .unwrap_or(dark_hint),
        };
        palette.surface = if palette.dark {
            lighter
                .clone()
                .unwrap_or_else(|| OmarchyPalette::fallback(true).surface)
        } else {
            darker
                .clone()
                .unwrap_or_else(|| OmarchyPalette::fallback(false).surface)
        };
        palette.hover = if palette.dark {
            darker
                .clone()
                .unwrap_or_else(|| OmarchyPalette::fallback(true).hover)
        } else {
            lighter
                .clone()
                .unwrap_or_else(|| OmarchyPalette::fallback(false).hover)
        };
        Some(palette)
    }
}

/// Theme files to try, in order. `PISWEEP_THEME_DIR` wins for tests and
/// local overrides; then pimarchy's current theme, then Omarchy's.
pub fn colors_candidates() -> Vec<PathBuf> {
    let mut paths = Vec::new();
    if let Some(state) = std::env::var_os("PISWEEP_THEME_DIR") {
        paths.push(PathBuf::from(state).join("colors.toml"));
    }
    let home = home_dir();
    paths.push(home.join(".local/state/pimarchy/current/theme/colors.toml"));
    paths.push(home.join(".local/state/omarchy/current/theme/colors.toml"));
    paths.push(home.join(".config/omarchy/current/theme/colors.toml"));
    paths
}

/// Paths to watch for live re-tinting. Omarchy replaces `current/theme` with
/// `rm -rf` + `mv`, which drops inotify watches, so watchers rearm when a
/// watched path disappears.
pub fn omarchy_watch_paths() -> Vec<PathBuf> {
    let current = home_dir().join(".local/state/omarchy/current");
    vec![
        current.clone(),
        current.join("theme"),
        current.join("theme/colors.toml"),
    ]
}

pub fn sanitized_text_scale(value: f32) -> f32 {
    if value <= 0.0 {
        1.0
    } else {
        value.clamp(0.5, 3.0)
    }
}

/// Desktop text size (`gsettings` `text-scaling-factor`), clamped to 0.5..3.
pub fn detect_text_scale() -> f32 {
    if let Ok(output) = std::process::Command::new("gsettings")
        .args(["get", "org.gnome.desktop.interface", "text-scaling-factor"])
        .output()
    {
        if output.status.success() {
            let raw = String::from_utf8_lossy(&output.stdout);
            if let Ok(value) = raw.trim().parse::<f32>() {
                return sanitized_text_scale(value);
            }
        }
    }
    1.0
}

pub fn detect_system_dark() -> bool {
    if let Ok(output) = std::process::Command::new("gsettings")
        .args(["get", "org.gnome.desktop.interface", "color-scheme"])
        .output()
    {
        if output.status.success() {
            let raw = String::from_utf8_lossy(&output.stdout);
            if raw.contains("prefer-dark") {
                return true;
            }
            if raw.contains("prefer-light") {
                return false;
            }
        }
    }
    true
}

fn unquote(value: &str) -> String {
    let value = value.trim();
    if value.len() >= 2
        && ((value.starts_with('"') && value.ends_with('"'))
            || (value.starts_with('\'') && value.ends_with('\'')))
    {
        value[1..value.len() - 1].to_string()
    } else {
        value.to_string()
    }
}

fn home_dir() -> PathBuf {
    directories::BaseDirs::new()
        .map(|d| d.home_dir().to_path_buf())
        .unwrap_or_else(|| PathBuf::from("."))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn palette_from(name: &str, dark_hint: bool, body: &str) -> OmarchyPalette {
        let dir = std::env::temp_dir().join(format!("pisweep-theme-{name}-{}", std::process::id()));
        let _ = fs::create_dir_all(&dir);
        let path = dir.join("colors.toml");
        fs::write(&path, body).unwrap();
        let palette = OmarchyPalette::from_colors_file(&path, dark_hint).unwrap();
        let _ = fs::remove_dir_all(&dir);
        palette
    }

    #[test]
    fn parses_full_palette() {
        let palette = palette_from(
            "full",
            false,
            "mode = \"dark\"\nbackground = \"#222822\"\nforeground = \"#e8d5b7\"\naccent = \"#4ade80\"\nselection = \"#2d3830\"\nmuted = \"#7f897f\"\nlighter_background = \"#2d3830\"\ndarker_background = \"#141814\"\n",
        );
        assert!(palette.dark);
        assert_eq!(palette.background, "#222822");
        assert_eq!(palette.foreground, "#e8d5b7");
        assert_eq!(palette.accent, "#4ade80");
        assert_eq!(palette.surface, "#2d3830");
        assert_eq!(palette.hover, "#141814");
    }

    #[test]
    fn mode_key_wins_over_luminance() {
        let palette = palette_from(
            "mode-wins",
            true,
            "mode = \"light\"\nbackground = \"#101010\"\n",
        );
        assert!(!palette.dark);
    }

    #[test]
    fn luminance_fallback_without_mode() {
        let palette = palette_from("luminance", false, "background = \"#1a1a1a\"\n");
        assert!(palette.dark);
    }

    #[test]
    fn missing_file_yields_none() {
        let missing = std::env::temp_dir().join("pisweep-nonexistent-colors.toml");
        assert!(OmarchyPalette::from_colors_file(&missing, true).is_none());
    }

    #[test]
    fn hex_parsing() {
        assert!(parse_hex_color("#4ade80").is_some());
        assert!(parse_hex_color("fff").is_some());
        assert!(parse_hex_color("nope").is_none());
    }

    #[test]
    fn text_scale_sanitized() {
        assert_eq!(sanitized_text_scale(0.0), 1.0);
        assert_eq!(sanitized_text_scale(9.0), 3.0);
        assert_eq!(sanitized_text_scale(1.25), 1.25);
    }

    #[test]
    fn watch_paths_cover_current_theme() {
        let paths = omarchy_watch_paths();
        assert_eq!(paths.len(), 3);
        assert!(paths[0].ends_with("current"));
        assert!(paths[1].ends_with("theme"));
        assert!(paths[2].ends_with("colors.toml"));
    }
}
