//! Lienzo de píxeles: pinta manchas ("dabs") a lo largo de los segmentos que entrega el
//! asistente de trazo, acumula cada trazo con flujo hasta su opacidad y lo funde al soltar.

use crate::brush::{BrushKind, BrushPreset};
use crate::smoothing::{PInfo, Segment};
use egui::{Color32, ColorImage, Pos2, TextureHandle, TextureOptions};

pub const W: usize = 960;
pub const H: usize = 640;
pub const PAPER: [f32; 3] = [0.985, 0.98, 0.965];

/// Capa transparente sobre el fondo (solo en el lienzo libre).
pub struct Layer {
    pub name: String,
    pub rgb: Vec<[f32; 3]>,
    pub alpha: Vec<f32>,
    pub opacity: f32,
    pub visible: bool,
}

impl Layer {
    pub fn new(name: impl Into<String>) -> Self {
        Self { name: name.into(), rgb: vec![[0.0; 3]; W * H], alpha: vec![0.0; W * H], opacity: 1.0, visible: true }
    }
}

/// Copia para deshacer: el fondo o una capa.
enum Snapshot {
    Base(Vec<[f32; 3]>),
    Layer(usize, Vec<[f32; 3]>, Vec<f32>),
}

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
    undo: Vec<(Snapshot, usize)>,
    /// Capas sobre el fondo, de abajo arriba.
    pub layers: Vec<Layer>,
    /// Dónde se pinta: `None` es el fondo; `Some(k)`, la capa k.
    pub active: Option<usize>,
    /// El trazo en curso borra (en una capa, quita transparencia).
    stroke_erase: bool,
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
            layers: Vec::new(),
            active: None,
            stroke_erase: false,
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

    /// Lienzo libre: fondo de papel y una capa para dibujar.
    pub fn new_free() -> Self {
        let mut c = Self::new();
        c.layers.push(Layer::new("Capa 1"));
        c.active = Some(0);
        c
    }

    /// Borra lo que haya en la capa activa (o deja el fondo en blanco).
    pub fn clear(&mut self) {
        self.push_undo();
        match self.active {
            None => self.base.fill(PAPER),
            Some(k) => self.layers[k].alpha.fill(0.0),
        }
        self.strokes.clear();
        self.mark_all();
    }

    /// Añade una capa encima de la activa y la deja activa.
    pub fn add_layer(&mut self) {
        let at = self.active.map_or(0, |k| k + 1);
        let n = self.layers.len() + 1;
        self.layers.insert(at, Layer::new(format!("Capa {n}")));
        self.active = Some(at);
        self.structure_changed();
    }

    pub fn remove_active_layer(&mut self) {
        if let Some(k) = self.active {
            self.layers.remove(k);
            self.active = if self.layers.is_empty() { None } else { Some(k.saturating_sub(1).min(self.layers.len() - 1)) };
            self.structure_changed();
        }
    }

    /// Sube (`up`) o baja la capa activa un puesto.
    pub fn move_active_layer(&mut self, up: bool) {
        let Some(k) = self.active else { return };
        let to = if up { k + 1 } else { k.wrapping_sub(1) };
        if to < self.layers.len() {
            self.layers.swap(k, to);
            self.active = Some(to);
            self.structure_changed();
        }
    }

    /// Añade una imagen como capa nueva (por ejemplo, un dibujo de la galería).
    pub fn add_image_layer(&mut self, name: &str, rgba: &[[f32; 4]]) {
        let mut l = Layer::new(name);
        for (i, px) in rgba.iter().enumerate().take(W * H) {
            l.rgb[i] = [px[0], px[1], px[2]];
            l.alpha[i] = px[3];
        }
        let at = self.active.map_or(0, |k| k + 1);
        self.layers.insert(at, l);
        self.active = Some(at);
        self.structure_changed();
    }

    /// Las copias para deshacer apuntan a capas por su posición: al cambiar el orden ya no valen.
    fn structure_changed(&mut self) {
        self.undo.clear();
        self.mark_all();
    }

    /// Reemplaza el lienzo con una imagen inicial (ejercicios de limpieza, etc.).
    pub fn reset_with(&mut self, pixels: Option<Vec<[f32; 3]>>) {
        self.base = pixels.unwrap_or_else(|| vec![PAPER; W * H]);
        self.stroke_a.fill(0.0);
        self.stroke_box = None;
        self.undo.clear();
        self.layers.clear();
        self.active = None;
        self.strokes.clear();
        self.mark_all();
    }

    fn push_undo(&mut self) {
        let snap = match self.active {
            None => Snapshot::Base(self.base.clone()),
            Some(k) => Snapshot::Layer(k, self.layers[k].rgb.clone(), self.layers[k].alpha.clone()),
        };
        self.undo.push((snap, self.strokes.len()));
        if self.undo.len() > 20 {
            self.undo.remove(0);
        }
    }

    pub fn can_undo(&self) -> bool {
        !self.undo.is_empty()
    }

    pub fn undo(&mut self) {
        if let Some((snap, n)) = self.undo.pop() {
            match snap {
                Snapshot::Base(img) => self.base = img,
                Snapshot::Layer(k, rgb, alpha) => {
                    if let Some(l) = self.layers.get_mut(k) {
                        l.rgb = rgb;
                        l.alpha = alpha;
                    }
                }
            }
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
                        let c = self.stroke_c[i];
                        match self.active {
                            None => {
                                let b = self.base[i];
                                self.base[i] = [b[0] + (c[0] - b[0]) * a, b[1] + (c[1] - b[1]) * a, b[2] + (c[2] - b[2]) * a];
                            }
                            Some(k) => {
                                let l = &mut self.layers[k];
                                if self.stroke_erase {
                                    l.alpha[i] *= 1.0 - a;
                                } else {
                                    // El trazo encima de lo que ya hay en la capa.
                                    let (lc, la) = (l.rgb[i], l.alpha[i]);
                                    let na = a + la * (1.0 - a);
                                    let mix = |s: f32, d: f32| (s * a + d * la * (1.0 - a)) / na;
                                    l.rgb[i] = [mix(c[0], lc[0]), mix(c[1], lc[1]), mix(c[2], lc[2])];
                                    l.alpha[i] = na;
                                }
                            }
                        }
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
                    let c = self.pixel(x as usize, y as usize);
                    for k in 0..3 {
                        acc[k] += c[k];
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
        self.stroke_erase = brush.kind == BrushKind::Eraser;
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

    /// Color visible de un píxel: fondo, capas visibles y el trazo en curso en la capa activa.
    pub fn pixel(&self, x: usize, y: usize) -> [f32; 3] {
        let i = y * W + x;
        let sa = self.stroke_a[i];
        let sc = self.stroke_c[i];
        let over = |d: [f32; 3], s: [f32; 3], a: f32| [d[0] + (s[0] - d[0]) * a, d[1] + (s[1] - d[1]) * a, d[2] + (s[2] - d[2]) * a];
        let mut c = self.base[i];
        if self.active.is_none() && sa > 0.0 {
            c = over(c, sc, sa);
        }
        for (k, l) in self.layers.iter().enumerate() {
            if !l.visible {
                continue;
            }
            let (mut lc, mut la) = (l.rgb[i], l.alpha[i]);
            if self.active == Some(k) && sa > 0.0 {
                if self.stroke_erase {
                    la *= 1.0 - sa;
                } else {
                    let na = sa + la * (1.0 - sa);
                    lc = [0, 1, 2].map(|j| (sc[j] * sa + lc[j] * la * (1.0 - sa)) / na);
                    la = na;
                }
            }
            if la > 0.0 {
                c = over(c, lc, la * l.opacity);
            }
        }
        c
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::brush::presets;
    use egui::pos2;

    fn stroke(c: &mut Canvas, brush: &BrushPreset) {
        let info = |x: f32| PInfo { pos: pos2(x, 300.0), pressure: 1.0, time: 0.0, speed: 0.0 };
        c.begin_stroke();
        c.paint(&Segment::Line(info(200.0), info(600.0)), brush);
        c.end_stroke();
    }

    #[test]
    fn layers_paint_hide_erase_and_undo() {
        let brushes = presets();
        let pen = brushes.iter().find(|b| b.id == "basico5").expect("pincel").clone();
        let eraser = brushes.iter().find(|b| b.kind == BrushKind::Eraser).expect("borrador").clone();
        let mut c = Canvas::new_free();
        c.color = [0.0, 0.0, 0.0];
        stroke(&mut c, &pen);
        let dark = c.pixel(400, 300);
        assert!(luminance(dark) < 0.3, "la línea se ve: {dark:?}");
        assert_eq!(c.base[300 * W + 400], PAPER, "el fondo no se toca al pintar en una capa");
        // Ocultar la capa la quita de la imagen.
        c.layers[0].visible = false;
        assert_eq!(c.pixel(400, 300), PAPER);
        c.layers[0].visible = true;
        // El borrador quita la transparencia de la capa y deja ver el papel.
        stroke(&mut c, &eraser);
        assert!((luminance(c.pixel(400, 300)) - luminance(PAPER)).abs() < 0.05);
        // Deshacer devuelve la línea.
        c.undo();
        assert!(luminance(c.pixel(400, 300)) < 0.3);
        // Una capa a media opacidad da un gris intermedio.
        c.layers[0].opacity = 0.5;
        let half = luminance(c.pixel(400, 300));
        assert!(half > 0.35 && half < 0.65, "media opacidad: {half}");
    }

    #[test]
    fn exercises_keep_painting_on_the_background() {
        let pen = presets().into_iter().find(|b| b.id == "basico5").expect("pincel");
        let mut c = Canvas::new();
        c.color = [0.0, 0.0, 0.0];
        stroke(&mut c, &pen);
        assert!(c.layers.is_empty() && c.active.is_none());
        assert!(luminance(c.base[300 * W + 400]) < 0.3, "sin capas se pinta en el fondo, como antes");
    }
}
