//! Presión adaptada: curva global de la tableta (como en Krita), velocidad de dibujo,
//! presión simulada para ratón y calibración personal (MECHANICS.md §2).

use crate::curve::Curve;
use egui::Pos2;
use serde::{Deserialize, Serialize};
use std::collections::VecDeque;

/// Krita: maxAllowedSpeedValue = 30, speedValueSmoothing = 3.
pub const MAX_ALLOWED_SPEED: f32 = 30.0;
pub const SPEED_SMOOTHING: usize = 3;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum MouseMode {
    Constante,
    PorVelocidad,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PressureSettings {
    pub curve: Curve,
    pub mouse_mode: MouseMode,
    pub mouse_constant: f32,
    /// Resultado de la última calibración: presión media suave / normal / fuerte.
    pub calibration: Option<[f32; 3]>,
}

impl Default for PressureSettings {
    fn default() -> Self {
        Self {
            curve: Curve::linear(),
            mouse_mode: MouseMode::PorVelocidad,
            mouse_constant: 0.7,
            calibration: None,
        }
    }
}

impl PressureSettings {
    /// Curva construida a partir de la calibración: el rango natural de la mano cubre 0..1.
    pub fn curve_from_calibration(c: [f32; 3]) -> Curve {
        let mut s = c;
        s.sort_by(|a, b| a.total_cmp(b));
        let soft = s[0].clamp(0.02, 0.9);
        let normal = s[1].clamp(soft + 0.03, 0.95);
        let strong = s[2].clamp(normal + 0.03, 0.98);
        Curve::new(vec![[0.0, 0.0], [soft, 0.2], [normal, 0.5], [strong, 0.9], [1.0, 1.0]])
    }
}

#[derive(Default)]
pub struct SpeedSmoother {
    samples: VecDeque<(f32, f64)>,
    last: Option<(Pos2, f64)>,
}

impl SpeedSmoother {
    pub fn reset(&mut self) {
        self.samples.clear();
        self.last = None;
    }

    /// Velocidad normalizada 0..1, como `min(1, velocidad / 30)` en Krita.
    pub fn next(&mut self, pos: Pos2, time_ms: f64) -> f32 {
        if let Some((p, t)) = self.last {
            let dt = (time_ms - t).max(0.5);
            self.samples.push_back(((pos - p).length(), dt));
            if self.samples.len() > SPEED_SMOOTHING {
                self.samples.pop_front();
            }
        }
        self.last = Some((pos, time_ms));
        let (d, t) = self.samples.iter().fold((0.0f32, 0.0f64), |acc, s| (acc.0 + s.0, acc.1 + s.1));
        if t <= 0.0 {
            return 0.0;
        }
        let px_per_ms = d / t as f32;
        (px_per_ms / MAX_ALLOWED_SPEED * 10.0).min(1.0)
    }
}

/// Presión simulada para ratón por velocidad (diseño propio): lento = más presión.
pub fn simulated(mode: MouseMode, constant: f32, speed: f32) -> f32 {
    match mode {
        MouseMode::Constante => constant,
        MouseMode::PorVelocidad => (1.0 - 0.7 * (speed * 1.6).min(1.0)).clamp(0.05, 1.0),
    }
}
