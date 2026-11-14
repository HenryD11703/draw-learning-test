//! Objetivos de los ejercicios de pintura. Una escena es una lista de figuras con relleno de valor
//! (gris) o de color, incluidas esferas y cilindros sombreados según el principio de la forma de
//! James Gurney («Color and Light»): lado iluminado con medio tono, sombra propia, luz reflejada en
//! el borde de la sombra y brillo. La escena da la imagen objetivo y la máscara de lo que se califica.

use crate::canvas::{H, PAPER, W};
use crate::lessons::{PaintTarget, paint_target, starting_canvas};
use serde::Deserialize;
use std::sync::Arc;

/// Qué se compara al calificar.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PaintMode {
    /// Solo el valor (lo claro y lo oscuro).
    Value,
    /// Tono, saturación y valor.
    Color,
    /// Que la zona quede cubierta, sin importar el tono.
    Coverage,
}

/// Relleno: un gris (0 negro, 1 blanco) o un color RGB de 0 a 1.
#[derive(Clone, Copy, Debug, Deserialize)]
#[serde(untagged)]
pub enum Fill {
    Value(f32),
    Color([f32; 3]),
}

impl Fill {
    pub fn rgb(self) -> [f32; 3] {
        match self {
            Fill::Value(v) => [v, v, v],
            Fill::Color(c) => c,
        }
    }
}

/// Figuras de una escena, en orden de dibujo (las de después tapan a las de antes).
#[derive(Clone, Debug, Deserialize)]
#[serde(tag = "prim", rename_all = "snake_case", deny_unknown_fields)]
pub enum Prim {
    Rect { rect: [f32; 4], fill: Fill },
    Ellipse { c: [f32; 2], rx: f32, ry: f32, fill: Fill },
    /// Polígono (par-impar, sirve también para no convexos).
    Poly { pts: Vec<[f32; 2]>, fill: Fill },
    /// Degradado lineal de `from` a `to`, horizontal o vertical.
    Gradient {
        rect: [f32; 4],
        from: Fill,
        to: Fill,
        #[serde(default)]
        vertical: bool,
    },
    /// Esfera. `light` es la dirección hacia la luz en pantalla (x a la derecha, y hacia abajo).
    /// `reflected` (0..0.4) aclara el borde de la sombra; `highlight` (0..1), el brillo.
    /// `steps` > 0 cuantiza el sombreado en esos niveles (versión «por planos»).
    Sphere {
        c: [f32; 2],
        r: f32,
        light: [f32; 2],
        lit: Fill,
        shadow: Fill,
        #[serde(default)]
        reflected: f32,
        #[serde(default)]
        highlight: f32,
        #[serde(default)]
        steps: u8,
    },
    /// Cilindro vertical (solo el cuerpo). `light` es la x de la luz, de -1 (izquierda) a 1.
    Cylinder {
        rect: [f32; 4],
        light: f32,
        lit: Fill,
        shadow: Fill,
        #[serde(default)]
        reflected: f32,
        #[serde(default)]
        steps: u8,
    },
}

/// Una escena para pintar: el objetivo y, opcionalmente, con qué empieza el lienzo.
#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PaintScene {
    pub mode: PaintMode,
    pub scene: Vec<Prim>,
    #[serde(default)]
    pub start: Vec<Prim>,
}

/// Objetivo de un ejercicio de pintura, sin calcular todavía (se calcula al abrirlo).
#[derive(Clone, Debug)]
pub enum Paint {
    /// Los primeros objetivos, escritos en código.
    Legacy(PaintTarget),
    Scene(Arc<PaintScene>),
}

/// Objetivo calculado: imagen, máscara de lo que se califica, lienzo inicial y modo.
pub struct Job {
    pub target: Vec<[f32; 3]>,
    pub mask: Vec<bool>,
    pub start: Option<Vec<[f32; 3]>>,
    pub mode: PaintMode,
}

impl Paint {
    pub fn render(&self) -> Job {
        match self {
            Paint::Legacy(t) => {
                let (target, mask) = paint_target(*t);
                let mode = match t {
                    PaintTarget::Silhouette => PaintMode::Coverage,
                    PaintTarget::ColorBlend => PaintMode::Color,
                    _ => PaintMode::Value,
                };
                Job { target, mask, start: starting_canvas(*t), mode }
            }
            Paint::Scene(s) => {
                let mut target = vec![PAPER; W * H];
                let mut mask = vec![false; W * H];
                for p in &s.scene {
                    draw(p, &mut target, Some(&mut mask));
                }
                let start = (!s.start.is_empty()).then(|| {
                    let mut img = vec![PAPER; W * H];
                    for p in &s.start {
                        draw(p, &mut img, None);
                    }
                    img
                });
                Job { target, mask, start, mode: s.mode }
            }
        }
    }
}

fn lerp3(a: [f32; 3], b: [f32; 3], t: f32) -> [f32; 3] {
    [0, 1, 2].map(|k| a[k] + (b[k] - a[k]) * t)
}

/// Caja de píxeles que cubre un rectángulo, recortada al lienzo.
fn pixel_box(x0: f32, y0: f32, x1: f32, y1: f32) -> (usize, usize, usize, usize) {
    let c = |v: f32, max: usize| (v.max(0.0) as usize).min(max);
    (c(x0.floor(), W), c(y0.floor(), H), c(x1.ceil(), W), c(y1.ceil(), H))
}

/// Sombreado de una superficie con normal `n` (unitaria): familia de luz y familia de sombra
/// separadas por el terminador, como las describe Gurney.
fn shade(n: [f32; 3], l: [f32; 3], lit: [f32; 3], shadow: [f32; 3], reflected: f32, highlight: f32, steps: u8) -> [f32; 3] {
    let lam = n[0] * l[0] + n[1] * l[1] + n[2] * l[2];
    let mut t = if lam > 0.0 {
        // Lado iluminado: del medio tono (junto al terminador) a la luz plena.
        0.3 + 0.7 * lam
    } else {
        // Sombra propia, algo más clara hacia el borde por la luz reflejada.
        reflected * (1.0 - n[2]) * (-lam).sqrt()
    };
    if steps > 0 {
        let s = steps as f32;
        t = ((t * s).floor() / (s - 1.0).max(1.0)).min(1.0);
    }
    let mut c = lerp3(shadow, lit, t.clamp(0.0, 1.0));
    if highlight > 0.0 && lam > 0.0 {
        // Reflejo especular hacia el observador (0, 0, 1).
        let rz = 2.0 * lam * n[2] - l[2];
        let h = highlight * rz.max(0.0).powf(30.0);
        c = lerp3(c, [1.0, 1.0, 1.0], h.clamp(0.0, 1.0));
    }
    c
}

fn light3(dx: f32, dy: f32) -> [f32; 3] {
    let len = (dx * dx + dy * dy + 0.6 * 0.6).sqrt().max(1e-6);
    [dx / len, dy / len, 0.6 / len]
}

fn draw(p: &Prim, img: &mut [[f32; 3]], mut mask: Option<&mut Vec<bool>>) {
    let mut put = |x: usize, y: usize, c: [f32; 3]| {
        img[y * W + x] = c;
        if let Some(m) = mask.as_deref_mut() {
            m[y * W + x] = true;
        }
    };
    match p {
        Prim::Rect { rect: [x0, y0, x1, y1], fill } => {
            let (a, b, c, d) = pixel_box(*x0, *y0, *x1, *y1);
            for y in b..d {
                for x in a..c {
                    put(x, y, fill.rgb());
                }
            }
        }
        Prim::Ellipse { c, rx, ry, fill } => {
            let (a, b, cc, d) = pixel_box(c[0] - rx, c[1] - ry, c[0] + rx, c[1] + ry);
            for y in b..d {
                for x in a..cc {
                    let (u, v) = ((x as f32 + 0.5 - c[0]) / rx, (y as f32 + 0.5 - c[1]) / ry);
                    if u * u + v * v <= 1.0 {
                        put(x, y, fill.rgb());
                    }
                }
            }
        }
        Prim::Poly { pts, fill } => {
            let (mut lo, mut hi) = ([f32::MAX; 2], [f32::MIN; 2]);
            for q in pts {
                lo = [lo[0].min(q[0]), lo[1].min(q[1])];
                hi = [hi[0].max(q[0]), hi[1].max(q[1])];
            }
            let (a, b, c, d) = pixel_box(lo[0], lo[1], hi[0], hi[1]);
            let n = pts.len();
            for y in b..d {
                for x in a..c {
                    let (px, py) = (x as f32 + 0.5, y as f32 + 0.5);
                    let mut inside = false;
                    for i in 0..n {
                        let (p0, p1) = (pts[i], pts[(i + 1) % n]);
                        if (p0[1] > py) != (p1[1] > py) && px < p0[0] + (p1[0] - p0[0]) * (py - p0[1]) / (p1[1] - p0[1]) {
                            inside = !inside;
                        }
                    }
                    if inside {
                        put(x, y, fill.rgb());
                    }
                }
            }
        }
        Prim::Gradient { rect: [x0, y0, x1, y1], from, to, vertical } => {
            let (a, b, c, d) = pixel_box(*x0, *y0, *x1, *y1);
            for y in b..d {
                for x in a..c {
                    let t = if *vertical { (y as f32 + 0.5 - y0) / (y1 - y0) } else { (x as f32 + 0.5 - x0) / (x1 - x0) };
                    put(x, y, lerp3(from.rgb(), to.rgb(), t.clamp(0.0, 1.0)));
                }
            }
        }
        Prim::Sphere { c, r, light, lit, shadow, reflected, highlight, steps } => {
            let l = light3(light[0], light[1]);
            let (a, b, cc, d) = pixel_box(c[0] - r, c[1] - r, c[0] + r, c[1] + r);
            for y in b..d {
                for x in a..cc {
                    let (nx, ny) = ((x as f32 + 0.5 - c[0]) / r, (y as f32 + 0.5 - c[1]) / r);
                    let d2 = nx * nx + ny * ny;
                    if d2 <= 1.0 {
                        let n = [nx, ny, (1.0 - d2).sqrt()];
                        put(x, y, shade(n, l, lit.rgb(), shadow.rgb(), *reflected, *highlight, *steps));
                    }
                }
            }
        }
        Prim::Cylinder { rect: [x0, y0, x1, y1], light, lit, shadow, reflected, steps } => {
            let l = light3(*light, -0.35);
            let (a, b, c, d) = pixel_box(*x0, *y0, *x1, *y1);
            let (cx, hw) = ((x0 + x1) / 2.0, (x1 - x0) / 2.0);
            for y in b..d {
                for x in a..c {
                    let u = ((x as f32 + 0.5 - cx) / hw).clamp(-1.0, 1.0);
                    let n = [u, 0.0, (1.0 - u * u).max(0.0).sqrt()];
                    put(x, y, shade(n, l, lit.rgb(), shadow.rgb(), *reflected, 0.0, *steps));
                }
            }
        }
    }
}

/// Tono (grados), saturación y valor (HSV) de un color.
pub fn hsv(c: [f32; 3]) -> (f32, f32, f32) {
    let (mx, mn) = (c[0].max(c[1]).max(c[2]), c[0].min(c[1]).min(c[2]));
    let d = mx - mn;
    let s = if mx <= 1e-6 { 0.0 } else { d / mx };
    let h = if d <= 1e-6 {
        0.0
    } else if mx == c[0] {
        60.0 * ((c[1] - c[2]) / d).rem_euclid(6.0)
    } else if mx == c[1] {
        60.0 * ((c[2] - c[0]) / d + 2.0)
    } else {
        60.0 * ((c[0] - c[1]) / d + 4.0)
    };
    (h, s, mx)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::canvas::luminance;

    fn at(img: &[[f32; 3]], x: usize, y: usize) -> f32 {
        luminance(img[y * W + x])
    }

    #[test]
    fn sphere_has_light_and_shadow_families() {
        let s = PaintScene {
            mode: PaintMode::Value,
            scene: vec![Prim::Sphere {
                c: [480.0, 320.0],
                r: 200.0,
                light: [-0.7, -0.7],
                lit: Fill::Value(0.95),
                shadow: Fill::Value(0.15),
                reflected: 0.25,
                highlight: 0.0,
                steps: 0,
            }],
            start: vec![],
        };
        let job = Paint::Scene(Arc::new(s)).render();
        let lit = at(&job.target, 400, 240);
        let core = at(&job.target, 560, 400);
        let rim = at(&job.target, 615, 455);
        assert!(lit > 0.7, "lado iluminado claro: {lit}");
        assert!(core < 0.25, "sombra propia oscura: {core}");
        assert!(rim > core, "la luz reflejada aclara el borde de la sombra: {rim} vs {core}");
        assert!(job.mask[320 * W + 480] && !job.mask[10 * W + 10]);
        assert!(job.start.is_none());
    }

    #[test]
    fn hsv_reads_hue_saturation_and_value() {
        let (h, s, v) = hsv([1.0, 0.0, 0.0]);
        assert!(h.abs() < 1e-3 && (s - 1.0).abs() < 1e-3 && (v - 1.0).abs() < 1e-3);
        let (h, _, _) = hsv([0.0, 0.0, 1.0]);
        assert!((h - 240.0).abs() < 1e-3);
        assert_eq!(hsv([0.5, 0.5, 0.5]).1, 0.0);
    }
}
