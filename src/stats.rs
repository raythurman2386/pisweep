//! Best-time persistence. Corrupt files reset; writes are atomic.

use std::fs;
use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use crate::board::Difficulty;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
pub struct Bests {
    #[serde(default)]
    pub beginner: u32,
    #[serde(default)]
    pub intermediate: u32,
    #[serde(default)]
    pub expert: u32,
}

impl Bests {
    pub fn get(&self, d: Difficulty) -> u32 {
        match d {
            Difficulty::Beginner => self.beginner,
            Difficulty::Intermediate => self.intermediate,
            Difficulty::Expert => self.expert,
        }
    }

    /// Record a finished run. Zero elapsed becomes 1s, matching omamine.
    /// Returns true when this run is a new best.
    pub fn record(&mut self, d: Difficulty, elapsed: u32) -> bool {
        let seconds = elapsed.max(1);
        let slot = match d {
            Difficulty::Beginner => &mut self.beginner,
            Difficulty::Intermediate => &mut self.intermediate,
            Difficulty::Expert => &mut self.expert,
        };
        if *slot == 0 || seconds < *slot {
            *slot = seconds;
            true
        } else {
            false
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
pub struct Settings {
    #[serde(default)]
    pub difficulty: String,
    #[serde(default)]
    pub bests: Bests,
}

pub struct Store {
    path: PathBuf,
}

impl Store {
    pub fn new(path: PathBuf) -> Self {
        Self { path }
    }

    pub fn load(&self) -> Settings {
        fs::read_to_string(&self.path)
            .ok()
            .and_then(|raw| serde_json::from_str(&raw).ok())
            .unwrap_or_default()
    }

    pub fn save(&self, settings: &Settings) {
        let Ok(body) = serde_json::to_string_pretty(settings) else {
            return;
        };
        if let Some(parent) = self.path.parent() {
            let _ = fs::create_dir_all(parent);
        }
        let tmp = self.path.with_extension("json.tmp");
        if fs::write(&tmp, body)
            .and_then(|_| fs::File::open(&tmp).and_then(|f| f.sync_all()))
            .is_ok()
        {
            let _ = fs::rename(&tmp, &self.path);
        }
    }
}

pub fn settings_paths() -> (PathBuf, PathBuf) {
    let base = directories::ProjectDirs::from("dev", "pisweep", "pisweep")
        .map(|p| p.data_dir().to_path_buf())
        .unwrap_or_else(|| PathBuf::from(".").join("pisweep-data"));
    let _ = fs::create_dir_all(&base);
    (base.clone(), base.join("stats.json"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn records_and_keeps_faster() {
        let mut b = Bests::default();
        assert!(b.record(Difficulty::Beginner, 12));
        assert_eq!(b.beginner, 12);
        assert!(!b.record(Difficulty::Beginner, 20));
        assert!(b.record(Difficulty::Beginner, 7));
        assert_eq!(b.beginner, 7);
        let mut z = Bests::default();
        assert!(z.record(Difficulty::Beginner, 0));
        assert_eq!(z.beginner, 1);
    }

    #[test]
    fn corrupt_resets() {
        let dir = std::env::temp_dir().join(format!("pisweep-stats-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        let path = dir.join("stats.json");
        fs::write(&path, "nope{{{").unwrap();
        let loaded = Store::new(path).load();
        assert_eq!(loaded, Settings::default());
        let _ = fs::remove_dir_all(dir);
    }
}
