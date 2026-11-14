//! Tipos de los módulos y ejercicios, geometría de las guías y objetivos de pintura.
//! El contenido (qué ejercicios hay) vive en los archivos JSON de `content/`; ver `content.rs`.

use crate::canvas::{H, PAPER, W};
use crate::content::{KindSpec, Vary};
use crate::smoothing::SmoothingType;
use egui::{Pos2, Vec2, pos2, vec2};
use serde::Deserialize;
use std::f32::consts::{PI, TAU};

#[derive(Clone, Debug)]
pub struct Guide {
    pub pts: Vec<Pos2>,
    /// Presión objetivo en cada punto (0..1) si el ejercicio la califica.
    pub pressure: Option<Vec<f32>>,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug, Deserialize)]
pub enum PaintTarget {
    ValueSteps,
    Gradient,
    Sphere,
    Cylinder,
    Silhouette,
    Cleanup,
    ColorBlend,
}

#[derive(Clone, Debug)]
pub enum Kind {
    /// Repasar las guías.
    Trace { guides: Vec<Guide>, tol: f32 },
    /// Solo se ven los extremos de cada guía: unirlos con un trazo.
    Points { guides: Vec<Guide>, tol: f32 },
    /// La guía se ve al lado como referencia y se dibuja en cualquier sitio y tamaño.
    Copy { guides: Vec<Guide>, tol: f32 },
    /// Horizonte, puntos de fuga y puntos de salida: trazar hacia donde corresponda.
    Perspective(Scene),
    Paint { target: PaintTarget },
}

/// Hacia dónde debe ir una línea en perspectiva.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Dir {
    Vp(usize),
    Vertical,
    Horizontal,
}

/// Una línea que el alumno debe trazar: sale de `from` en la dirección `dir` durante `len` px.
#[derive(Clone, Copy, Debug)]
pub struct Task {
    pub from: Pos2,
    pub dir: Dir,
    pub len: f32,
}

#[derive(Clone, Debug)]
pub struct Scene {
    pub horizon: Option<f32>,
    pub vps: Vec<Pos2>,
    /// Líneas ya dibujadas (contexto).
    pub given: Vec<Guide>,
    pub tasks: Vec<Task>,
    pub tol: f32,
    /// Si se pueden mostrar las líneas objetivo como ayuda.
    pub hints: bool,
}

impl Scene {
    /// Dirección unitaria que debe seguir una línea que pasa por `at`.
    pub fn dir_at(&self, at: Pos2, d: Dir) -> Vec2 {
        match d {
            Dir::Vp(i) => {
                let v = self.vps[i] - at;
                if v.length() < 1e-3 { vec2(1.0, 0.0) } else { v.normalized() }
            }
            Dir::Vertical => vec2(0.0, 1.0),
            Dir::Horizontal => vec2(1.0, 0.0),
        }
    }

    /// Segmento objetivo de una tarea.
    pub fn target(&self, t: &Task) -> (Pos2, Pos2) {
        (t.from, t.from + self.dir_at(t.from, t.dir) * t.len)
    }

    /// Direcciones válidas en la escena (una por cada tipo usado en las tareas).
    pub fn dirs(&self) -> Vec<Dir> {
        let mut v: Vec<Dir> = Vec::new();
        for t in &self.tasks {
            if !v.contains(&t.dir) {
                v.push(t.dir);
            }
        }
        v
    }
}

#[derive(Clone)]
pub struct Exercise {
    pub id: String,
    pub title: &'static str,
    pub goal: &'static str,
    pub krita_tip: &'static str,
    pub brush: &'static str,
    pub smoothing: Option<SmoothingType>,
    pub color: Option<[f32; 3]>,
    /// Instancia actual (se reconstruye con otra semilla en cada intento).
    pub kind: Kind,
    /// Segundos que se ve la guía antes de ocultarse (ejercicios de memoria).
    pub memory: Option<f32>,
    pub spec: KindSpec,
    pub vary: Vary,
}

impl Exercise {
    /// Construye el ejercicio. La semilla 0 da siempre la versión de referencia, sin variación.
    pub fn build(&self, seed: u64) -> Kind {
        self.spec.build(&self.vary, seed)
    }
}

pub struct Module {
    pub title: &'static str,
    pub subtitle: &'static str,
    pub lesson: &'static str,
    pub accent: egui::Color32,
    pub exercises: Vec<Exercise>,
}

// ------------------------------------------------------------------ geometría

const STEP: f32 = 3.0;

pub(crate) fn resample(pts: &[Pos2]) -> Vec<Pos2> {
    let mut out = Vec::new();
    if pts.is_empty() {
        return out;
    }
    out.push(pts[0]);
    let mut carry = 0.0;
    for w in pts.windows(2) {
        let (a, b) = (w[0], w[1]);
        let len = (b - a).length();
        let mut s = STEP - carry;
        while s <= len {
            out.push(a + (b - a) * (s / len));
            s += STEP;
        }
        carry = len - (s - STEP);
    }
    if out.last() != pts.last() {
        out.push(*pts.last().unwrap());
    }
    out
}

pub(crate) fn ellipse(c: Pos2, rx: f32, ry: f32, rot: f32, a0: f32, a1: f32) -> Vec<Pos2> {
    let n = 160;
    let (s, co) = rot.sin_cos();
    (0..=n)
        .map(|i| {
            let t = a0 + (a1 - a0) * i as f32 / n as f32;
            let x = rx * t.cos();
            let y = ry * t.sin();
            pos2(c.x + x * co - y * s, c.y + x * s + y * co)
        })
        .collect()
}

pub(crate) fn circle(c: Pos2, r: f32) -> Vec<Pos2> {
    ellipse(c, r, r, 0.0, -PI / 2.0, -PI / 2.0 + TAU)
}

pub(crate) fn bezier(p0: Pos2, p1: Pos2, p2: Pos2, p3: Pos2) -> Vec<Pos2> {
    (0..=80)
        .map(|i| {
            let t = i as f32 / 80.0;
            let u = 1.0 - t;
            (p0.to_vec2() * (u * u * u) + p1.to_vec2() * (3.0 * u * u * t) + p2.to_vec2() * (3.0 * u * t * t) + p3.to_vec2() * (t * t * t)).to_pos2()
        })
        .collect()
}

pub(crate) fn spiral(c: Pos2, r0: f32, r1: f32, turns: f32) -> Vec<Pos2> {
    let n = (turns * 120.0) as usize;
    (0..=n)
        .map(|i| {
            let t = i as f32 / n as f32;
            let a = t * turns * TAU;
            let r = r0 + (r1 - r0) * t;
            pos2(c.x + r * a.cos(), c.y + r * a.sin())
        })
        .collect()
}

pub(crate) fn wave(x0: f32, x1: f32, y: f32, amp: f32, periods: f32) -> Vec<Pos2> {
    (0..=200)
        .map(|i| {
            let t = i as f32 / 200.0;
            pos2(x0 + (x1 - x0) * t, y + amp * (t * periods * TAU).sin())
        })
        .collect()
}

pub(crate) fn star(c: Pos2, r_out: f32, r_in: f32) -> Vec<Pos2> {
    let mut v = Vec::new();
    for i in 0..=10 {
        let a = -PI / 2.0 + i as f32 * PI / 5.0;
        let r = if i % 2 == 0 { r_out } else { r_in };
        v.push(pos2(c.x + r * a.cos(), c.y + r * a.sin()));
    }
    v
}

// ------------------------------------------------------------------ objetivos de pintura

pub const CELL: usize = 16;

/// Imagen objetivo y máscara (dónde se califica) de un ejercicio de pintura.
pub fn paint_target(t: PaintTarget) -> (Vec<[f32; 3]>, Vec<bool>) {
    let mut img = vec![PAPER; W * H];
    let mut mask = vec![false; W * H];
    let set = |img: &mut Vec<[f32; 3]>, mask: &mut Vec<bool>, x: usize, y: usize, c: [f32; 3]| {
        img[y * W + x] = c;
        mask[y * W + x] = true;
    };
    match t {
        PaintTarget::ValueSteps => {
            let vals = [0.92, 0.72, 0.5, 0.3, 0.1];
            for (k, v) in vals.iter().enumerate() {
                let x0 = 120 + k * 150;
                for y in 220..400 {
                    for x in x0..x0 + 120 {
                        set(&mut img, &mut mask, x, y, [*v; 3]);
                    }
                }
            }
        }
        PaintTarget::Gradient => {
            for y in 240..400 {
                for x in 120..840 {
                    let t = (x - 120) as f32 / 720.0;
                    let v = 0.12 + 0.82 * t;
                    set(&mut img, &mut mask, x, y, [v; 3]);
                }
            }
        }
        PaintTarget::Sphere => {
            let (cx, cy, r) = (480.0, 320.0, 190.0);
            let light = vec2(-0.55, -0.6);
            let lz = (1.0 - light.length_sq()).max(0.0).sqrt();
            for y in 0..H {
                for x in 0..W {
                    let dx = (x as f32 - cx) / r;
                    let dy = (y as f32 - cy) / r;
                    let d2 = dx * dx + dy * dy;
                    if d2 <= 1.0 {
                        let nz = (1.0 - d2).sqrt();
                        let lam = (dx * light.x + dy * light.y + nz * lz).max(0.0);
                        let v = 0.12 + 0.8 * lam;
                        set(&mut img, &mut mask, x, y, [v; 3]);
                    }
                }
            }
        }
        PaintTarget::Cylinder => {
            let (x0, x1, y0, y1) = (330.0, 630.0, 150.0, 500.0);
            for y in y0 as usize..y1 as usize {
                for x in x0 as usize..x1 as usize {
                    let u = (x as f32 - x0) / (x1 - x0) * 2.0 - 1.0;
                    let nz = (1.0 - u * u).max(0.0).sqrt();
                    let lam = (-0.6 * u + 0.8 * nz).max(0.0);
                    let v = 0.14 + 0.78 * lam;
                    set(&mut img, &mut mask, x, y, [v; 3]);
                }
            }
        }
        PaintTarget::Silhouette => {
            // Una hoja: intersección de dos círculos.
            let c1 = pos2(480.0, 470.0);
            let c2 = pos2(480.0, 170.0);
            let r = 260.0;
            for y in 0..H {
                for x in 0..W {
                    let p = pos2(x as f32 + 0.5, y as f32 + 0.5);
                    let rot = pos2(480.0 + (p.x - 480.0) * 0.9 - (p.y - 320.0) * 0.44, 320.0 + (p.x - 480.0) * 0.44 + (p.y - 320.0) * 0.9);
                    if (rot - c1).length() < r && (rot - c2).length() < r {
                        set(&mut img, &mut mask, x, y, [0.2, 0.45, 0.85]);
                    }
                }
            }
        }
        PaintTarget::Cleanup => {
            for y in 140..500 {
                for x in 200..760 {
                    set(&mut img, &mut mask, x, y, PAPER);
                }
            }
        }
        PaintTarget::ColorBlend => {
            let a = [0.85, 0.35, 0.2];
            let b = [0.2, 0.4, 0.8];
            for y in 220..420 {
                for x in 140..820 {
                    let t = ((x - 140) as f32 / 680.0).clamp(0.0, 1.0);
                    let s = t * t * (3.0 - 2.0 * t);
                    set(&mut img, &mut mask, x, y, [a[0] + (b[0] - a[0]) * s, a[1] + (b[1] - a[1]) * s, a[2] + (b[2] - a[2]) * s]);
                }
            }
        }
    }
    (img, mask)
}

/// Lienzo inicial de un ejercicio (por ejemplo, manchas para limpiar o colores para mezclar).
pub fn starting_canvas(t: PaintTarget) -> Option<Vec<[f32; 3]>> {
    match t {
        PaintTarget::Cleanup => {
            let mut img = vec![PAPER; W * H];
            let blobs = [(260.0, 200.0, 40.0), (420.0, 330.0, 55.0), (640.0, 230.0, 35.0), (560.0, 430.0, 45.0), (330.0, 440.0, 30.0), (700.0, 380.0, 28.0)];
            for y in 0..H {
                for x in 0..W {
                    for &(bx, by, br) in &blobs {
                        let d = ((x as f32 - bx).powi(2) + (y as f32 - by).powi(2)).sqrt();
                        if d < br {
                            img[y * W + x] = [0.25, 0.22, 0.3];
                        }
                    }
                }
            }
            Some(img)
        }
        PaintTarget::ColorBlend => {
            let mut img = vec![PAPER; W * H];
            for y in 220..420 {
                for x in 140..820 {
                    img[y * W + x] = if x < 480 { [0.85, 0.35, 0.2] } else { [0.2, 0.4, 0.8] };
                }
            }
            Some(img)
        }
        _ => None,
    }
}

// ------------------------------------------------------------------ contenido

/// Todos los módulos, en orden, leídos de `content/`.
pub fn modules() -> Vec<Module> {
    crate::content::modules()
}

#[allow(dead_code)]
pub fn all_exercise_ids(mods: &[Module]) -> Vec<String> {
    mods.iter().flat_map(|m| m.exercises.iter().map(|e| e.id.clone())).collect()
}
