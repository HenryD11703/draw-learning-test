//! Lienzo de píxeles: pinta manchas ("dabs") a lo largo de los segmentos que entrega el
//! asistente de trazo, acumula cada trazo con flujo hasta su opacidad y lo funde al soltar.

use crate::brush::{BrushKind, BrushPreset};
use crate::smoothing::{PInfo, Segment};
use egui::{Color32, ColorImage, Pos2, TextureHandle, TextureOptions};

pub const W: usize = 960;
pub const H: usize = 640;
pub const PAPER: [f32; 3] = [0.985, 0.98, 0.965];

#[derive(Clone, Copy, Debug)]
pub struct StrokePoint {
    pub pos: Pos2,
    pub pressure: f32,
}

pub struct Canvas {
    pub base: Vec<[f32; 3]>,
    stroke_a: Vec<f32>,
    stroke_c: Vec<[f32; 3]>,
    stroke_box: Option<[usize; 4]>,
    dirty: Option<[usize; 4]>,
    display: Vec<Color32>,
    texture: Option<TextureHandle>,
    undo: Vec<(Vec<[f32; 3]>, usize)>,
    redo: Vec<(Vec<[f32; 3]>, usize)>,
    next_dab: f32,
    pub strokes: Vec<Vec<StrokePoint>>,
    pub color: [f32; 3],
    pub size_mult: f32,
    pub opacity_mult: f32,
    pub last_dab_size: f32,
}

impl Canvas {
    pub fn new() -> Self {
        let mut c = Self {
            base: vec![PAPER; W * H],
            stroke_a: vec![0.0; W * H],
            stroke_c: vec![[0.0; 3]; W * H],
            stroke_box: None,
            dirty: None,
            display: vec![Color32::WHITE; W * H],
            texture: None,
            undo: Vec::new(),
            redo: Vec::new(),
            next_dab: 0.0,
            strokes: Vec::new(),
            color: [0.1, 0.1, 0.12],
            size_mult: 1.0,
            opacity_mult: 1.0,
            last_dab_size: 0.0,
        };
        c.mark_all();
        c
    }

    pub fn mark_all(&mut self) {
        self.dirty = Some([0, 0, W, H]);
    }

    fn mark(&mut self, b: [usize; 4]) {
        self.dirty = Some(match self.dirty {
            None => b,
            Some(d) => [d[0].min(b[0]), d[1].min(b[1]), d[2].max(b[2]), d[3].max(b[3])],
        });
        self.stroke_box = Some(match self.stroke_box {
            None => b,
            Some(d) => [d[0].min(b[0]), d[1].min(b[1]), d[2].max(b[2]), d[3].max(b[3])],
        });
    }

    pub fn clear(&mut self) {
        self.push_undo();
        self.base.fill(PAPER);
        self.strokes.clear();
        self.mark_all();
    }

    /// Reemplaza el lienzo con una imagen inicial (ejercicios de limpieza, etc.).
    pub fn reset_with(&mut self, pixels: Option<Vec<[f32; 3]>>) {
        self.base = pixels.unwrap_or_else(|| vec![PAPER; W * H]);
        self.stroke_a.fill(0.0);
        self.stroke_box = None;
        self.undo.clear();
        self.redo.clear();
        self.strokes.clear();
        self.mark_all();
    }

    fn push_undo(&mut self) {
        self.undo.push((self.base.clone(), self.strokes.len()));
        if self.undo.len() > 20 {
            self.undo.remove(0);
        }
        self.redo.clear();
    }

    pub fn can_undo(&self) -> bool {
        !self.undo.is_empty()
    }

    pub fn undo(&mut self) {
        if let Some((img, n)) = self.undo.pop() {
            let cur = std::mem::replace(&mut self.base, img);
            self.redo.push((cur, self.strokes.len()));
            self.strokes.truncate(n);
            self.mark_all();
        }
    }

    pub fn begin_stroke(&mut self) {
        self.push_undo();
        self.next_dab = 0.0;
        self.strokes.push(Vec::new());
    }

    pub fn end_stroke(&mut self) {
        if let Some([x0, y0, x1, y1]) = self.stroke_box.take() {
            for y in y0..y1 {
                for x in x0..x1 {
                    let i = y * W + x;
                    let a = self.stroke_a[i];
                    if a > 0.0 {
                        let b = self.base[i];
                        let c = self.stroke_c[i];
                        self.base[i] = [b[0] + (c[0] - b[0]) * a, b[1] + (c[1] - b[1]) * a, b[2] + (c[2] - b[2]) * a];
                        self.stroke_a[i] = 0.0;
                    }
                }
            }
            self.dirty = Some(match self.dirty {
                None => [x0, y0, x1, y1],
                Some(d) => [d[0].min(x0), d[1].min(y0), d[2].max(x1), d[3].max(y1)],
            });
        }
        if self.strokes.last().is_some_and(|s| s.is_empty()) {
            self.strokes.pop();
        }
    }

    pub fn paint(&mut self, seg: &Segment, brush: &BrushPreset) {
        match *seg {
            Segment::Dot(p) => {
                self.dab(p, brush);
                self.next_dab = self.spacing(p, brush);
            }
            Segment::Line(a, b) => self.line(a, b, brush),
            Segment::Bezier(a, c1, c2, b) => {
                let approx = (c1 - a.pos).length() + (c2 - c1).length() + (b.pos - c2).length();
                let n = ((approx / 2.0).ceil() as usize).clamp(1, 400);
                let mut prev = a;
                for k in 1..=n {
                    let t = k as f32 / n as f32;
                    let u = 1.0 - t;
                    let pos = (a.pos.to_vec2() * (u * u * u)
                        + c1.to_vec2() * (3.0 * u * u * t)
                        + c2.to_vec2() * (3.0 * u * t * t)
                        + b.pos.to_vec2() * (t * t * t))
                        .to_pos2();
                    let cur = PInfo {
                        pos,
                        pressure: a.pressure + (b.pressure - a.pressure) * t,
                        time: a.time + (b.time - a.time) * t as f64,
                        speed: a.speed + (b.speed - a.speed) * t,
                    };
                    self.line(prev, cur, brush);
                    prev = cur;
                }
            }
        }
    }

    fn spacing(&self, p: PInfo, brush: &BrushPreset) -> f32 {
        let d = self.diameter(p, brush);
        (d * brush.spacing).max(if brush.kind == BrushKind::Pixel { 1.0 } else { 0.7 })
    }

    fn diameter(&self, p: PInfo, brush: &BrushPreset) -> f32 {
        (brush.size * self.size_mult * brush.size_factor(p.pressure, p.speed)).max(1.0)
    }

    fn line(&mut self, a: PInfo, b: PInfo, brush: &BrushPreset) {
        let len = (b.pos - a.pos).length();
        if len <= 0.0001 {
            return;
        }
        let mut s = self.next_dab;
        let mut guard = 0;
        while s <= len && guard < 20000 {
            let t = s / len;
            let p = PInfo {
                pos: a.pos + (b.pos - a.pos) * t,
                pressure: a.pressure + (b.pressure - a.pressure) * t,
                time: a.time,
                speed: a.speed + (b.speed - a.speed) * t,
            };
            self.dab(p, brush);
            s += self.spacing(p, brush);
            guard += 1;
        }
        self.next_dab = s - len;
    }

    fn sample_color(&self, pos: Pos2, radius: f32) -> [f32; 3] {
        let r = radius.max(1.0) as i32;
        let cx = pos.x as i32;
        let cy = pos.y as i32;
        let mut acc = [0.0f32; 3];
        let mut n = 0.0;
        let step = (r / 3).max(1);
        let mut y = cy - r;
        while y <= cy + r {
            let mut x = cx - r;
            while x <= cx + r {
                if x >= 0 && y >= 0 && (x as usize) < W && (y as usize) < H {
                    let i = y as usize * W + x as usize;
                    let a = self.stroke_a[i];
                    let b = self.base[i];
                    let c = self.stroke_c[i];
                    for k in 0..3 {
                        acc[k] += b[k] + (c[k] - b[k]) * a;
                    }
                    n += 1.0;
                }
                x += step;
            }
            y += step;
        }
        if n == 0.0 { self.color } else { [acc[0] / n, acc[1] / n, acc[2] / n] }
    }

    fn dab(&mut self, p: PInfo, brush: &BrushPreset) {
        let diam = self.diameter(p, brush);
        self.last_dab_size = diam;
        let radius = diam * 0.5;
        let opacity = (brush.opacity_at(p.pressure) * self.opacity_mult).clamp(0.0, 1.0);
        let flow = brush.flow_at(p.pressure).clamp(0.0, 1.0);
        let mut color = match brush.kind {
            BrushKind::Eraser => PAPER,
            _ => self.color,
        };
        if brush.smudge > 0.0 {
            let under = self.sample_color(p.pos, radius * 0.6);
            for k in 0..3 {
                color[k] = color[k] + (under[k] - color[k]) * brush.smudge_at(p.pressure);
            }
        }
        if let Some(s) = self.strokes.last_mut() {
            s.push(StrokePoint { pos: p.pos, pressure: p.pressure });
        }
        let pixel = brush.kind == BrushKind::Pixel;
        let reach = radius + 1.0;
        let x0 = ((p.pos.x - reach).floor().max(0.0)) as usize;
        let y0 = ((p.pos.y - reach).floor().max(0.0)) as usize;
        let x1 = ((p.pos.x + reach).ceil().max(0.0) as usize).min(W);
        let y1 = ((p.pos.y + reach).ceil().max(0.0) as usize).min(H);
        if x0 >= x1 || y0 >= y1 {
            return;
        }
        let hard = brush.hardness.clamp(0.0, 1.0);
        for y in y0..y1 {
            for x in x0..x1 {
                let dx = x as f32 + 0.5 - p.pos.x;
                let dy = y as f32 + 0.5 - p.pos.y;
                let mut mask = if pixel {
                    if dx.abs() < radius.max(0.5) && dy.abs() < radius.max(0.5) { 1.0 } else { 0.0 }
                } else {
                    let d = (dx * dx + dy * dy).sqrt();
                    if d > radius + 0.5 {
                        0.0
                    } else {
                        // Borde: zona dura + caída suave, con 1 px de antialias.
                        let edge = ((radius + 0.5 - d).clamp(0.0, 1.0)).min(1.0);
                        let inner = radius * hard;
                        let fall = if d <= inner || radius - inner < 0.01 {
                            1.0
                        } else {
                            let t = ((d - inner) / (radius - inner)).clamp(0.0, 1.0);
                            1.0 - t * t * (3.0 - 2.0 * t)
                        };
                        edge * fall
                    }
                };
                if mask <= 0.0 {
                    continue;
                }
                if brush.grain > 0.0 {
                    mask *= 1.0 - brush.grain * grain(x, y);
                }
                let i = y * W + x;
                let a0 = self.stroke_a[i];
                if opacity <= a0 {
                    continue;
                }
                let a1 = a0 + (opacity - a0) * (mask * flow).min(1.0);
                if a1 <= a0 {
                    continue;
                }
                let w_new = (a1 - a0) / a1;
                let c0 = self.stroke_c[i];
                self.stroke_c[i] = [
                    c0[0] + (color[0] - c0[0]) * w_new,
                    c0[1] + (color[1] - c0[1]) * w_new,
                    c0[2] + (color[2] - c0[2]) * w_new,
                ];
                self.stroke_a[i] = a1;
            }
        }
        self.mark([x0, y0, x1, y1]);
    }

    /// Color visible de un píxel (lienzo + trazo en curso).
    pub fn pixel(&self, x: usize, y: usize) -> [f32; 3] {
        let i = y * W + x;
        let a = self.stroke_a[i];
        let b = self.base[i];
        let c = self.stroke_c[i];
        [b[0] + (c[0] - b[0]) * a, b[1] + (c[1] - b[1]) * a, b[2] + (c[2] - b[2]) * a]
    }

    pub fn texture(&mut self, ctx: &egui::Context) -> &TextureHandle {
        if let Some([x0, y0, x1, y1]) = self.dirty.take() {
            for y in y0..y1 {
                for x in x0..x1 {
                    let c = self.pixel(x, y);
                    self.display[y * W + x] = Color32::from_rgb(to8(c[0]), to8(c[1]), to8(c[2]));
                }
            }
            let img = ColorImage::new([W, H], self.display.clone());
            match &mut self.texture {
                Some(t) => t.set(img, TextureOptions::LINEAR),
                None => self.texture = Some(ctx.load_texture("lienzo", img, TextureOptions::LINEAR)),
            }
        }
        if self.texture.is_none() {
            let img = ColorImage::new([W, H], self.display.clone());
            self.texture = Some(ctx.load_texture("lienzo", img, TextureOptions::LINEAR));
        }
        self.texture.as_ref().expect("texture")
    }

    pub fn to_rgba(&self) -> Vec<u8> {
        let mut out = Vec::with_capacity(W * H * 4);
        for y in 0..H {
            for x in 0..W {
                let c = self.pixel(x, y);
                out.extend_from_slice(&[to8(c[0]), to8(c[1]), to8(c[2]), 255]);
            }
        }
        out
    }
}

pub fn to8(v: f32) -> u8 {
    (v.clamp(0.0, 1.0) * 255.0 + 0.5) as u8
}

/// Grano de papel determinista (ruido de valor de dos escalas).
fn grain(x: usize, y: usize) -> f32 {
    fn h(x: u32, y: u32) -> f32 {
        let mut n = x.wrapping_mul(374_761_393) ^ y.wrapping_mul(668_265_263);
        n = (n ^ (n >> 13)).wrapping_mul(1_274_126_177);
        ((n ^ (n >> 16)) & 0xffff) as f32 / 65535.0
    }
    let fine = h(x as u32, y as u32);
    let coarse = h(x as u32 / 3, y as u32 / 3);
    (fine * 0.55 + coarse * 0.45).powf(1.6)
}

pub fn luminance(c: [f32; 3]) -> f32 {
    0.2126 * c[0] + 0.7152 * c[1] + 0.0722 * c[2]
}
