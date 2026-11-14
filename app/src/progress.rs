//! Progreso del alumno y ajustes guardados en disco.

use crate::pressure::PressureSettings;
use crate::smoothing::SmoothingOptions;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;

#[derive(Clone, Copy, Debug, Default, Serialize, Deserialize)]
pub struct Result {
    pub best: f32,
    pub stars: u8,
    pub attempts: u32,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct Progress {
    pub results: HashMap<String, Result>,
    pub pressure: PressureSettings,
    pub smoothing: SmoothingOptions,
    pub unlock_all: bool,
    /// Dónde va el alumno en el programa del año.
    #[serde(default)]
    pub program: crate::program::ProgramState,
}

fn path() -> Option<PathBuf> {
    let base = std::env::var_os("APPDATA").or_else(|| std::env::var_os("HOME"))?;
    Some(PathBuf::from(base).join("TrazoAcademia").join("progreso.json"))
}

impl Progress {
    pub fn load() -> Self {
        let mut p: Progress = path()
            .and_then(|p| std::fs::read_to_string(p).ok())
            .and_then(|s| serde_json::from_str(&s).ok())
            .unwrap_or_default();
        p.pressure.curve.rebuild();
        p
    }

    pub fn save(&self) {
        if let Some(p) = path() {
            if let Some(dir) = p.parent() {
                let _ = std::fs::create_dir_all(dir);
            }
            if let Ok(s) = serde_json::to_string_pretty(self) {
                let _ = std::fs::write(p, s);
            }
        }
    }

    pub fn stars(&self, id: &str) -> u8 {
        self.results.get(id).map_or(0, |r| r.stars)
    }

    pub fn record(&mut self, id: &str, score: f32, stars: u8) {
        let r = self.results.entry(id.to_string()).or_default();
        r.attempts += 1;
        if score > r.best {
            r.best = score;
        }
        if stars > r.stars {
            r.stars = stars;
        }
    }

    pub fn total_stars(&self) -> u32 {
        self.results.values().map(|r| r.stars as u32).sum()
    }
}
