//! Testable Minesweeper core, ported from omamine (Game.js + Solver.js).
//!
//! No UI imports. The binary owns the GPUI Kit window.

pub mod board;
pub mod cli;
pub mod icons;
pub mod solver;
pub mod stats;
pub mod theme;

pub use board::{Difficulty, Game};
pub use cli::{parse_cli, CliAction};
pub use icons::SweepIcon;
pub use solver::Hint;
pub use theme::{parse_hex_color, OmarchyPalette, RgbaColor};
