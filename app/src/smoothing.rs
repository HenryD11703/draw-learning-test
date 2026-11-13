//! Asistente de trazo: los modos de suavizado de Krita (Ninguno, Básico, Ponderado,
//! Estabilizador, Pixel) reescritos a partir de la hoja MECHANICS.md §1, más una guía
//! magnética propia para principiantes.

use egui::{Pos2, Vec2, pos2};
use serde::{Deserialize, Serialize};
use std::collections::VecDeque;

#[derive(Clone, Copy, Debug)]
pub struct PInfo {
    pub pos: Pos2,
    pub pressure: f32,
    /// Milisegundos desde el inicio de la app.
    pub time: f64,
    /// Velocidad de dibujo normalizada 0..1.
    pub speed: f32,
}

impl PInfo {
    fn mix(&self, other: &PInfo, t: f32, with_sensors: bool) -> PInfo {
        PInfo {
            pos: self.pos + (other.pos - self.pos) * t,
            pressure: if with_sensors { self.pressure + (other.pressure - self.pressure) * t } else { self.pressure },
            time: self.time,
            speed: if with_sensors { self.speed + (other.speed - self.speed) * t } else { self.speed },
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum SmoothingType {
    Ninguno,
    Basico,
    Ponderado,
    Estabilizador,
    Pixel,
}

impl SmoothingType {
    pub const ALL: [SmoothingType; 5] = [
        SmoothingType::Ninguno,
        SmoothingType::Basico,
        SmoothingType::Ponderado,
        SmoothingType::Estabilizador,
        SmoothingType::Pixel,
    ];
    pub fn label(self) -> &'static str {
        match self {
            SmoothingType::Ninguno => "Ninguno",
            SmoothingType::Basico => "Básico",
            SmoothingType::Ponderado => "Ponderado",
            SmoothingType::Estabilizador => "Estabilizador",
            SmoothingType::Pixel => "Pixel",
        }
    }
    pub fn help(self) -> &'static str {
        match self {
            SmoothingType::Ninguno => "Cada movimiento se pinta tal cual. Muestra tu pulso real.",
            SmoothingType::Basico => "Une los puntos con curvas suaves según la velocidad. Es el modo por defecto de Krita.",
            SmoothingType::Ponderado => "Promedia los últimos puntos con una campana gaussiana: la línea va un poco detrás del cursor y sale más limpia.",
            SmoothingType::Estabilizador => "El trazo persigue al cursor como si tirara de una cuerda. Ideal para curvas largas y entintado.",
            SmoothingType::Pixel => "Ajusta cada punto al centro de un píxel y evita los píxeles de esquina. Para pixel art.",
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SmoothingOptions {
    pub kind: SmoothingType,
    pub distance_min: f32,
    pub distance_max: f32,
    pub tail_aggressiveness: f32,
    pub smooth_pressure: bool,
    pub scalable_distance: bool,
    pub use_delay: bool,
    pub delay_distance: f32,
    pub finish_stabilized: bool,
    pub stabilize_sensors: bool,
    /// Guía magnética (diseño propio): 0 = apagada, 1 = máxima.
    pub magnet: f32,
}

impl Default for SmoothingOptions {
    fn default() -> Self {
        // Valores por defecto de KisConfig (MECHANICS.md §1).
        Self {
            kind: SmoothingType::Basico,
            distance_min: 50.0,
            distance_max: 50.0,
            tail_aggressiveness: 0.15,
            smooth_pressure: false,
            scalable_distance: true,
            use_delay: true,
            delay_distance: 50.0,
            finish_stabilized: true,
            stabilize_sensors: true,
            magnet: 0.0,
        }
    }
}

/// Lo que el lienzo debe pintar.
#[derive(Clone, Copy, Debug)]
pub enum Segment {
    Dot(PInfo),
    Line(PInfo, PInfo),
    Bezier(PInfo, Pos2, Pos2, PInfo),
}

const STABILIZER_INTERVAL_MS: f64 = 50.0;
const BASIC_TIMEOUT_MS: f64 = 100.0;

pub struct Smoother {
    pub opts: SmoothingOptions,
    /// 1/zoom del lienzo (Krita divide las distancias por el zoom efectivo).
    pub zoom: f32,
    active: bool,
    history: Vec<PInfo>,
    distances: Vec<f32>,
    previous: Option<PInfo>,
    older: Option<PInfo>,
    have_tangent: bool,
    previous_tangent: Vec2,
    last_input_time: f64,
    // Estabilizador
    queue: VecDeque<PInfo>,
    pending: Vec<PInfo>,
    last_event: Option<PInfo>,
    last_tick: f64,
    // Pixel
    pixel_last: Option<(i32, i32)>,
    pixel_pending: Option<(i32, i32, PInfo)>,
}

impl Smoother {
    pub fn new(opts: SmoothingOptions) -> Self {
        Self {
            opts,
            zoom: 1.0,
            active: false,
            history: Vec::new(),
            distances: Vec::new(),
            previous: None,
            older: None,
            have_tangent: false,
            previous_tangent: Vec2::ZERO,
            last_input_time: 0.0,
            queue: VecDeque::new(),
            pending: Vec::new(),
            last_event: None,
            last_tick: 0.0,
            pixel_last: None,
            pixel_pending: None,
        }
    }

    #[allow(dead_code)]
    pub fn is_active(&self) -> bool {
        self.active
    }

    /// La "posición del pincel" que dibuja la app como cursor del trazo (útil en el estabilizador).
    pub fn brush_position(&self) -> Option<Pos2> {
        if !self.active {
            return None;
        }
        self.previous.map(|p| p.pos)
    }

    fn distance_for(&self, speed: f32) -> f32 {
        (1.0 - speed) * self.opts.distance_max + self.opts.distance_min * speed
    }

    pub fn begin(&mut self, info: PInfo, out: &mut Vec<Segment>) {
        self.active = true;
        self.history.clear();
        self.distances.clear();
        self.have_tangent = false;
        self.previous_tangent = Vec2::ZERO;
        self.older = Some(info);
        self.previous = Some(info);
        self.last_input_time = info.time;
        self.pixel_last = None;
        self.pixel_pending = None;
        match self.opts.kind {
            SmoothingType::Estabilizador => {
                let n = (self.distance_for(info.speed).round() as i32).max(3) as usize;
                self.queue = std::iter::repeat_n(info, n).collect();
                self.pending.clear();
                self.last_event = Some(info);
                self.last_tick = info.time;
                out.push(Segment::Dot(info));
            }
            SmoothingType::Pixel => {
                let px = (info.pos.x.floor() as i32, info.pos.y.floor() as i32);
                let snapped = PInfo { pos: pos2(px.0 as f32 + 0.5, px.1 as f32 + 0.5), ..info };
                out.push(Segment::Dot(snapped));
                self.pixel_last = Some(px);
                self.previous = Some(snapped);
            }
            _ => out.push(Segment::Dot(info)),
        }
    }

    pub fn add(&mut self, mut info: PInfo, out: &mut Vec<Segment>) {
        if !self.active {
            return;
        }
        self.last_input_time = info.time;
        match self.opts.kind {
            SmoothingType::Ninguno => {
                let prev = self.previous.unwrap_or(info);
                out.push(Segment::Line(prev, info));
                self.previous = Some(info);
            }
            SmoothingType::Basico => {
                self.basic(info, out);
                self.previous = Some(info);
            }
            SmoothingType::Ponderado => {
                self.weighted(&mut info);
                self.basic(info, out);
                self.previous = Some(info);
            }
            SmoothingType::Estabilizador => {
                self.pending.push(info);
                self.last_event = Some(info);
            }
            SmoothingType::Pixel => self.pixel(info, out),
        }
    }

    /// Se llama cada cuadro mientras el trazo está activo (temporizadores de Krita).
    pub fn tick(&mut self, now: f64, out: &mut Vec<Segment>) {
        if !self.active {
            return;
        }
        match self.opts.kind {
            SmoothingType::Basico | SmoothingType::Ponderado => {
                if self.have_tangent && now - self.last_input_time > BASIC_TIMEOUT_MS {
                    self.finish_basic(out);
                }
            }
            SmoothingType::Estabilizador => {
                while now - self.last_tick >= STABILIZER_INTERVAL_MS {
                    self.last_tick += STABILIZER_INTERVAL_MS;
                    self.stabilizer_poll(out);
                }
            }
            _ => {}
        }
    }

    pub fn end(&mut self, out: &mut Vec<Segment>) {
        if !self.active {
            return;
        }
        match self.opts.kind {
            SmoothingType::Basico | SmoothingType::Ponderado => self.finish_basic(out),
            SmoothingType::Estabilizador => {
                self.stabilizer_poll(out);
                if self.opts.finish_stabilized {
                    if let Some(last) = self.last_event {
                        // Seguir promediando hasta alcanzar el último punto.
                        for _ in 0..self.queue.len() + 1 {
                            self.pending.push(last);
                            self.stabilizer_poll_inner(out, true);
                        }
                        let prev = self.previous.unwrap_or(last);
                        out.push(Segment::Line(prev, last));
                    }
                }
            }
            SmoothingType::Pixel => {
                if let Some((x, y, info)) = self.pixel_pending.take() {
                    let p = PInfo { pos: pos2(x as f32 + 0.5, y as f32 + 0.5), ..info };
                    let prev = self.previous.unwrap_or(p);
                    out.push(Segment::Line(prev, p));
                }
            }
            SmoothingType::Ninguno => {}
        }
        self.active = false;
    }

    // ---------------------------------------------------------------- Básico (Bézier)

    fn basic(&mut self, info: PInfo, out: &mut Vec<Segment>) {
        let previous = self.previous.unwrap_or(info);
        let older = self.older.unwrap_or(previous);
        if !self.have_tangent {
            self.have_tangent = true;
            let dt = (info.time - previous.time).max(1.0) as f32;
            self.previous_tangent = (info.pos - previous.pos) / dt;
        } else {
            let dt = (info.time - older.time).max(1.0) as f32;
            let new_tangent = (info.pos - older.pos) / dt;
            if new_tangent == Vec2::ZERO || self.previous_tangent == Vec2::ZERO {
                out.push(Segment::Line(previous, info));
            } else if let Some(seg) = bezier_segment(older, previous, self.previous_tangent, new_tangent) {
                out.push(seg);
            }
            self.previous_tangent = new_tangent;
        }
        self.older = Some(previous);
    }

    fn finish_basic(&mut self, out: &mut Vec<Segment>) {
        if !self.have_tangent {
            return;
        }
        self.have_tangent = false;
        if let (Some(older), Some(previous)) = (self.older, self.previous) {
            let dt = (previous.time - older.time).max(1.0) as f32;
            let new_tangent = (previous.pos - older.pos) / dt;
            if new_tangent == Vec2::ZERO || self.previous_tangent == Vec2::ZERO {
                out.push(Segment::Line(older, previous));
            } else if let Some(seg) = bezier_segment(older, previous, self.previous_tangent, new_tangent) {
                out.push(seg);
            } else {
                out.push(Segment::Line(older, previous));
            }
            self.older = Some(previous);
        }
    }

    // ---------------------------------------------------------------- Ponderado

    fn weighted(&mut self, info: &mut PInfo) {
        if self.opts.distance_min <= 0.0 && self.opts.distance_max <= 0.0 {
            return;
        }
        let prev_pos = self
            .history
            .last()
            .map(|p| p.pos)
            .or(self.previous.map(|p| p.pos))
            .unwrap_or(info.pos);
        self.distances.push((info.pos - prev_pos).length());
        self.history.push(*info);
        // Límite práctico del historial (Krita lo limpia al cortar por peso).
        if self.history.len() > 400 {
            self.history.drain(0..200);
            self.distances.drain(0..200);
        }
        let n = self.history.len();
        if n <= 3 {
            return;
        }
        let scale = if self.opts.scalable_distance { 1.0 / self.zoom.max(0.01) } else { 1.0 };
        let speed = self.previous.map(|p| p.speed).unwrap_or(info.speed);
        let sigma = scale * self.distance_for(speed) / 3.0;
        let gaussian_weight = 1.0 / (2.506_628_3 * sigma);
        let gaussian_weight2 = sigma * sigma;
        let tail = self.opts.tail_aggressiveness * 40.0;
        let mut distance_sum = 0.0f32;
        let mut scale_sum = 0.0f32;
        let mut x = 0.0f32;
        let mut y = 0.0f32;
        let mut pressure = 0.0f32;
        let mut base_rate = 0.0f32;
        for i in (0..n).rev() {
            let p = self.history[i];
            let mut rate = self.distances[i];
            if i + 1 < n {
                let grad = p.pressure - self.history[i + 1].pressure;
                if grad > 0.0 {
                    rate += grad * tail * (1.0 - p.pressure) * 3.0 * sigma;
                }
            }
            let mut w = 0.0;
            if gaussian_weight2 != 0.0 {
                distance_sum += rate;
                w = (-distance_sum * distance_sum / (2.0 * gaussian_weight2)).exp() * gaussian_weight;
            }
            if i == n - 1 {
                base_rate = w;
            } else if w <= 0.0 || base_rate / w > 100.0 {
                break;
            }
            scale_sum += w;
            x += p.pos.x * w;
            y += p.pos.y * w;
            if self.opts.smooth_pressure {
                pressure += p.pressure * w;
            }
        }
        if scale_sum > 0.0 {
            x /= scale_sum;
            y /= scale_sum;
            pressure /= scale_sum;
            if x != 0.0 && y != 0.0 {
                info.pos = pos2(x, y);
                if self.opts.smooth_pressure {
                    info.pressure = pressure;
                }
                if let Some(last) = self.history.last_mut() {
                    *last = *info;
                }
            }
        }
    }

    // ---------------------------------------------------------------- Estabilizador

    fn stabilizer_poll(&mut self, out: &mut Vec<Segment>) {
        if self.pending.is_empty() {
            // Sin eventos nuevos: Krita repite el último para que el trazo alcance al cursor.
            if let Some(last) = self.last_event {
                self.pending.push(last);
            }
        }
        self.stabilizer_poll_inner(out, false);
    }

    fn stabilizer_poll_inner(&mut self, out: &mut Vec<Segment>, finishing: bool) {
        let events: Vec<PInfo> = std::mem::take(&mut self.pending);
        for ev in events {
            let last_painted = self.previous.unwrap_or(ev);
            if self.opts.use_delay && !finishing {
                let dead = self.opts.delay_distance / self.zoom.max(0.01);
                if (ev.pos - last_painted.pos).length() <= dead {
                    // Dentro de la zona muerta el pincel no avanza y la cola se llena con él.
                    for q in self.queue.iter_mut() {
                        *q = last_painted;
                    }
                    continue;
                }
            }
            let stabilized = self.stabilized_info(ev);
            out.push(Segment::Line(last_painted, stabilized));
            self.previous = Some(stabilized);
            self.queue.pop_front();
            self.queue.push_back(ev);
        }
    }

    fn stabilized_info(&self, current: PInfo) -> PInfo {
        let mut result = current;
        if self.queue.len() > 1 {
            let mut k = 1.0f32;
            for item in self.queue.iter().skip(1) {
                // Media móvil: Krita mezcla con peso k/(k+1) para lo acumulado.
                result = result.mix(item, 1.0 / (k + 1.0), self.opts.stabilize_sensors);
                k += 1.0;
            }
        }
        result.time = current.time;
        result
    }

    // ---------------------------------------------------------------- Pixel

    fn pixel(&mut self, info: PInfo, out: &mut Vec<Segment>) {
        let px = (info.pos.x.floor() as i32, info.pos.y.floor() as i32);
        let Some(last) = self.pixel_last else {
            self.pixel_last = Some(px);
            return;
        };
        if px == last {
            return;
        }
        let adjacent = |a: (i32, i32), b: (i32, i32)| (a.0 - b.0).abs().max((a.1 - b.1).abs()) == 1;
        match self.pixel_pending {
            None => {
                if adjacent(px, last) {
                    self.pixel_pending = Some((px.0, px.1, info));
                } else {
                    self.pixel_paint(px, info, out);
                }
            }
            Some((qx, qy, qi)) => {
                if px == (qx, qy) {
                    return;
                }
                if adjacent(px, last) {
                    // El pendiente era un píxel de esquina: se salta.
                    self.pixel_pending = Some((px.0, px.1, info));
                } else {
                    self.pixel_pending = None;
                    self.pixel_paint((qx, qy), qi, out);
                    if adjacent(px, (qx, qy)) {
                        self.pixel_pending = Some((px.0, px.1, info));
                    } else {
                        self.pixel_paint(px, info, out);
                    }
                }
            }
        }
    }

    fn pixel_paint(&mut self, px: (i32, i32), info: PInfo, out: &mut Vec<Segment>) {
        let p = PInfo { pos: pos2(px.0 as f32 + 0.5, px.1 as f32 + 0.5), ..info };
        let prev = self.previous.unwrap_or(p);
        out.push(Segment::Line(prev, p));
        self.previous = Some(p);
        self.pixel_last = Some(px);
    }
}

/// Puntos de control de la Bézier entre p1 y p2 con sus tangentes (MECHANICS.md §1, Básico).
fn bezier_segment(pi1: PInfo, pi2: PInfo, t1: Vec2, t2: Vec2) -> Option<Segment> {
    if t1 == Vec2::ZERO || t2 == Vec2::ZERO {
        return None;
    }
    let p1 = pi1.pos;
    let p2 = pi2.pos;
    let mut ct1 = p1 + t1;
    let mut ct2 = p2 - t2;
    if segments_intersect(ct1, ct2, p1, p2) {
        let dist = (p2 - p1).length() * 0.5;
        if t1.length() > 0.0 {
            ct1 = p1 + t1.normalized() * dist;
        }
        if t2.length() > 0.0 {
            ct2 = p2 - t2.normalized() * dist;
        }
    } else {
        let x = match line_intersection(p1, p1 + t1, p2, p2 - t2) {
            Some(x) if x.x.abs() + x.y.abs() <= 1.0e6 => x,
            _ => pos2((p1.x + p2.x) * 0.5, (p1.y + p2.y) * 0.5),
        };
        ct1 = x;
        ct2 = x;
    }
    let v1 = t1.length();
    let v2 = t2.length();
    let ratio = (v1 / v2).min(v2 / v1).max(1.0e-6);
    let k = (1.0 - (ratio - 0.8).max(0.0)) * 0.8;
    let (k1, k2) = if v1 > v2 { (k, ratio * k) } else { (ratio * k, k) };
    let c1 = p1 + (ct1 - p1) * k1;
    let c2 = p2 + (ct2 - p2) * k2;
    Some(Segment::Bezier(pi1, c1, c2, pi2))
}

fn cross(a: Vec2, b: Vec2) -> f32 {
    a.x * b.y - a.y * b.x
}

fn line_intersection(a1: Pos2, a2: Pos2, b1: Pos2, b2: Pos2) -> Option<Pos2> {
    let r = a2 - a1;
    let s = b2 - b1;
    let d = cross(r, s);
    if d.abs() < 1e-9 {
        return None;
    }
    let t = cross(b1 - a1, s) / d;
    Some(a1 + r * t)
}

fn segments_intersect(a1: Pos2, a2: Pos2, b1: Pos2, b2: Pos2) -> bool {
    let r = a2 - a1;
    let s = b2 - b1;
    let d = cross(r, s);
    if d.abs() < 1e-9 {
        return false;
    }
    let t = cross(b1 - a1, s) / d;
    let u = cross(b1 - a1, r) / d;
    (0.0..=1.0).contains(&t) && (0.0..=1.0).contains(&u)
}
