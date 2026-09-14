//! Lucide icons for the board and HUD, served alongside gpui-kit's set.
//!
//! Same pattern as pifile: copy the official SVGs into `assets/icons/` and
//! implement `IconNamed` so `Icon::new(SweepIcon::Flag)` works.

use std::borrow::Cow;

use gpui_kit::component::IconNamed;
use gpui_kit::{AssetSource, SharedString};
use rust_embed::RustEmbed;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SweepIcon {
    Flag,
    Bomb,
    X,
    Smile,
    Frown,
    Grin,
    Lightbulb,
    Trophy,
    Timer,
    Info,
}

impl IconNamed for SweepIcon {
    fn path(self) -> SharedString {
        match self {
            Self::Flag => "icons/flag.svg",
            Self::Bomb => "icons/bomb.svg",
            Self::X => "icons/x.svg",
            Self::Smile => "icons/face-slightly-smiling.svg",
            Self::Frown => "icons/face-slightly-frowning.svg",
            Self::Grin => "icons/face-grinning.svg",
            Self::Lightbulb => "icons/lightbulb.svg",
            Self::Trophy => "icons/trophy.svg",
            Self::Timer => "icons/timer.svg",
            Self::Info => "icons/info.svg",
        }
        .into()
    }
}

#[derive(RustEmbed)]
#[folder = "assets/"]
#[include = "icons/*.svg"]
pub struct AppAssets;

impl AssetSource for AppAssets {
    fn load(&self, path: &str) -> anyhow::Result<Option<Cow<'static, [u8]>>> {
        if path.starts_with("icons/") && path.ends_with(".svg") {
            if let Some(data) = Self::get(path) {
                return Ok(Some(data.data));
            }
        }
        gpui_kit::assets::Assets.load(path)
    }

    fn list(&self, path: &str) -> anyhow::Result<Vec<SharedString>> {
        gpui_kit::assets::Assets.list(path)
    }
}
