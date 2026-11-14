//! Ventana principal: inicio, mapa de niveles, ejercicios, lienzo libre, estudio con referencia,
//! diario, presión y guía de Krita.

mod program_ui;
mod study_ui;

use crate::brush::{BrushKind, BrushPreset, presets};
use crate::canvas::{Canvas, H, PAPER, W, to8};
use crate::curve::Curve;
use crate::grading::{Grade, error_marks, grade_copy, grade_hatch, grade_paint, grade_perspective, grade_points, grade_trace};
use crate::lessons::{Exercise, Guide, Kind, Module, Region, Role, Scene, modules};
use crate::painting::PaintMode;
use crate::pressure::{MouseMode, PressureSettings, SpeedSmoother, simulated};
use crate::program::{Program, Review};
use crate::progress::Progress;
use crate::smoothing::{PInfo, Segment, SmoothingType, Smoother};
use crate::study::Topic;
use crate::theme::{self, Btn, Icon};
use crate::view::{View, Xf};
use egui::{
    Align2, Color32, ColorImage, Event, FontId, Pos2, Rect, RichText, Sense, Shape, Stroke, StrokeKind, TextureHandle, TextureOptions, Ui,
    pos2, vec2,
};
use std::collections::HashMap;
use std::sync::Arc;
use study_ui::{DiaryItem, StudyState};
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Instant;

const ACCENT: Color32 = theme::ACCENT;
const GOLD: Color32 = theme::GOLD;

/// Icono y nombre de cada tipo de ejercicio.
fn kind_info(kind: &Kind) -> (Icon, &'static str) {
    match kind {
        Kind::Trace { .. } => (Icon::Trace, "Calcar"),
        Kind::Points { .. } => (Icon::Points, "Punto a punto"),
        Kind::Copy { .. } => (Icon::Copy, "Copia"),
        Kind::Perspective(_) => (Icon::Perspective, "Perspectiva"),
        Kind::Hatch { .. } => (Icon::Hatch, "Rayado"),
        Kind::Paint(_) => (Icon::Paint, "Pintura"),
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Screen {
    Home,
    Program,
    Map,
    Gallery,
    Exercise(usize, usize),
    Free,
    Study,
    Diary,
    Pressure,
    Guide,
}

/// Cada modo con lienzo tiene el suyo: al cambiar de modo, el anterior espera aparcado.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
enum Slot {
    Exercise,
    Free,
    Study,
}

struct PaintRef {
    target: Vec<[f32; 3]>,
    mask: Vec<bool>,
    start: Option<Vec<[f32; 3]>>,
    mode: PaintMode,
    reference: TextureHandle,
    outline: TextureHandle,
}

struct Calibration {
    stage: usize,
    samples: [Vec<f32>; 3],
    pad_strokes: Vec<Vec<(Pos2, f32)>>,
}

pub struct Screenshot {
    pub path: std::path::PathBuf,
    pub requested: bool,
    pub done: Arc<AtomicBool>,
}

pub struct TrazoApp {
    modules: Vec<Module>,
    brushes: Vec<BrushPreset>,
    brush: usize,
    eraser_return: Option<usize>,
    canvas: Canvas,
    smoother: Smoother,
    speed: SpeedSmoother,
    progress: Progress,
    screen: Screen,
    start: Instant,
    drawing: bool,
    last_force: Option<(f32, f64)>,
    raw_pressure: f32,
    curved_pressure: f32,
    grade: Option<Grade>,
    paint_ref: Option<PaintRef>,
    ex_started: f64,
    show_guide: bool,
    calib: Calibration,
    curve_drag: Option<usize>,
    message: Option<(String, f64)>,
    pub screenshot: Option<Screenshot>,
    frame: u64,
    persist: bool,
    /// Modo demostración: los ejercicios se abren sin variación (semilla 0).
    demo: bool,
    seeds: u64,
    /// Errores marcados en el lienzo tras calificar: punto y gravedad (0..1).
    marks: Vec<(Pos2, f32)>,
    show_marks: bool,
    /// Módulo al que desplazar el mapa al abrirlo.
    map_focus: Option<usize>,
    /// Se acabó el tiempo del ejercicio: ya se calificó y no se puede seguir dibujando.
    time_up: bool,
    view: View,
    /// Área del lienzo en el último fotograma (para los atajos de zoom).
    last_area: Rect,
    grid: bool,
    thirds: bool,
    /// Los lienzos de los otros modos, aparcados mientras se usa `canvas`.
    parked: HashMap<Slot, Canvas>,
    canvas_slot: Slot,
    /// Imagen de referencia del lienzo libre (se carga arrastrando un archivo a la ventana).
    reference: Option<TextureHandle>,
    gallery: Vec<GalleryItem>,
    session: Option<Session>,
    study: StudyState,
    diary: Vec<DiaryItem>,
    diary_loaded: bool,
    diary_filter: Option<Topic>,
    /// Programa del año (contenido fijo) y la semana que se está leyendo, si no es la actual.
    program: Program,
    program_view: Option<usize>,
    review: Review,
    review_open: bool,
    /// El ejercicio o el estudio abierto viene de la pantalla «Hoy».
    from_program: bool,
    /// Última foto elegida al azar de la biblioteca, para no repetirla.
    last_ref: Option<std::path::PathBuf>,
}

/// Un dibujo guardado, con su miniatura cuando ya se cargó.
struct GalleryItem {
    path: std::path::PathBuf,
    name: String,
    tex: Option<TextureHandle>,
    failed: bool,
}

/// Sesión de gestos: varias poses seguidas, cada una con el mismo tiempo.
struct Session {
    queue: Vec<(usize, usize)>,
    secs: f32,
    total: usize,
    scores: Vec<f32>,
    /// Cuándo pasar a la siguiente pose tras acabarse el tiempo.
    next_at: Option<f64>,
}

impl TrazoApp {
    pub fn new(cc: &eframe::CreationContext<'_>, screenshot: Option<Screenshot>) -> Self {
        theme::install(&cc.egui_ctx);
        let persist = screenshot.is_none();
        let progress = if persist { Progress::load() } else { Progress::default() };
        let brushes = presets();
        let brush = brushes.iter().position(|b| b.id == "basico5").unwrap_or(0);
        Self {
            modules: modules(),
            brushes,
            brush,
            eraser_return: None,
            canvas: Canvas::new(),
            smoother: Smoother::new(progress.smoothing.clone()),
            speed: SpeedSmoother::default(),
            progress,
            screen: Screen::Home,
            start: Instant::now(),
            drawing: false,
            last_force: None,
            raw_pressure: 0.0,
            curved_pressure: 0.0,
            grade: None,
            paint_ref: None,
            ex_started: 0.0,
            show_guide: true,
            calib: Calibration { stage: 0, samples: [Vec::new(), Vec::new(), Vec::new()], pad_strokes: Vec::new() },
            curve_drag: None,
            message: None,
            screenshot,
            frame: 0,
            persist,
            demo: std::env::var_os("TRAZO_DEMO").is_some(),
            seeds: 0,
            marks: Vec::new(),
            show_marks: true,
            map_focus: None,
            time_up: false,
            view: View::default(),
            last_area: Rect::from_min_size(pos2(0.0, 0.0), vec2(W as f32, H as f32)),
            grid: false,
            thirds: false,
            parked: HashMap::new(),
            canvas_slot: Slot::Exercise,
            reference: None,
            gallery: Vec::new(),
            session: None,
            study: StudyState::default(),
            diary: Vec::new(),
            diary_loaded: false,
            diary_filter: None,
            program: Program::load(),
            program_view: None,
            review: Review::default(),
            review_open: false,
            from_program: false,
            last_ref: None,
        }
    }

    /// El ejercicio que toca: el primero sin estrellas de un módulo abierto; si no hay, el primero
    /// que aún no tiene tres estrellas.
    fn next_target(&self) -> Option<(usize, usize)> {
        let mut fallback = None;
        for (m, md) in self.modules.iter().enumerate() {
            if !self.module_unlocked(m) {
                continue;
            }
            for (e, ex) in md.exercises.iter().enumerate() {
                let s = self.progress.stars(&ex.id);
                if s == 0 {
                    return Some((m, e));
                }
                if s < 3 && fallback.is_none() {
                    fallback = Some((m, e));
                }
            }
        }
        fallback
    }

    fn total_stars_possible(&self) -> u32 {
        self.modules.iter().map(|m| m.exercises.len() as u32 * 3).sum()
    }

    /// Olvida la calificación y las marcas de error (el dibujo cambió).
    fn clear_result(&mut self) {
        self.marks.clear();
    }

    /// Semilla distinta de 0 para la variación de cada intento.
    fn next_seed(&mut self) -> u64 {
        self.seeds += 1;
        let t = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0, |d| d.as_nanos() as u64);
        (t ^ self.seeds.wrapping_mul(0x9E37_79B9_7F4A_7C15)) | 1
    }

    fn now_ms(&self) -> f64 {
        self.start.elapsed().as_secs_f64() * 1000.0
    }

    fn save(&mut self) {
        self.progress.smoothing = self.smoother.opts.clone();
        if self.persist {
            self.progress.save();
        }
    }

    fn current_exercise(&self) -> Option<&Exercise> {
        match self.screen {
            Screen::Exercise(m, e) => self.modules.get(m).and_then(|md| md.exercises.get(e)),
            _ => None,
        }
    }

    fn flat_index(&self, m: usize, e: usize) -> usize {
        self.modules[..m].iter().map(|md| md.exercises.len()).sum::<usize>() + e
    }

    fn by_flat(&self, i: usize) -> Option<(usize, usize)> {
        let mut k = i;
        for (m, md) in self.modules.iter().enumerate() {
            if k < md.exercises.len() {
                return Some((m, k));
            }
            k -= md.exercises.len();
        }
        None
    }

    /// Ejercicios del módulo con al menos una estrella.
    fn module_done(&self, m: usize) -> usize {
        self.modules[m].exercises.iter().filter(|e| self.progress.stars(&e.id) > 0).count()
    }

    /// Ejercicios con estrella que hacen falta en un módulo para abrir el siguiente (la mitad).
    fn module_needed(&self, m: usize) -> usize {
        self.modules[m].exercises.len().div_ceil(2)
    }

    /// Un módulo se abre al conseguir estrella en la mitad de los ejercicios del anterior.
    /// Dentro de un módulo abierto, todos sus ejercicios están disponibles.
    fn module_unlocked(&self, m: usize) -> bool {
        self.progress.unlock_all || m == 0 || self.module_done(m - 1) >= self.module_needed(m - 1)
    }

    fn unlocked(&self, m: usize, _e: usize) -> bool {
        self.module_unlocked(m)
    }

    fn open_exercise(&mut self, ctx: &egui::Context, m: usize, e: usize) {
        // Cada intento es una variación nueva; en modo demostración, siempre la de referencia.
        let seed = if self.demo { 0 } else { self.next_seed() };
        let built = self.modules[m].exercises[e].build(seed);
        self.modules[m].exercises[e].kind = built;
        let ex = self.modules[m].exercises[e].clone();
        self.use_canvas(Slot::Exercise);
        self.from_program = false;
        self.screen = Screen::Exercise(m, e);
        self.grade = None;
        self.clear_result();
        self.time_up = false;
        self.show_guide = true;
        self.ex_started = self.now_ms();
        if let Some(i) = self.brushes.iter().position(|b| b.id == ex.brush) {
            self.brush = i;
            self.eraser_return = None;
        }
        self.canvas.size_mult = 1.0;
        self.canvas.opacity_mult = 1.0;
        if let Some(c) = ex.color.or(self.brushes[self.brush].default_color) {
            self.canvas.color = c;
        } else {
            self.canvas.color = [0.1, 0.1, 0.12];
        }
        if let Some(s) = ex.smoothing {
            self.smoother.opts.kind = s;
        }
        match ex.kind {
            Kind::Paint(paint) => {
                let job = paint.render();
                self.canvas.reset_with(job.start.clone());
                let reference = ctx.load_texture("referencia", to_color_image(&job.target), TextureOptions::LINEAR);
                let outline = ctx.load_texture("contorno", outline_image(&job.mask), TextureOptions::LINEAR);
                self.paint_ref = Some(PaintRef { target: job.target, mask: job.mask, start: job.start, mode: job.mode, reference, outline });
            }
            _ => {
                self.canvas.reset_with(None);
                self.paint_ref = None;
            }
        }
    }

    /// Pone en `canvas` el lienzo de un modo y aparca el que estaba, sin perderlo.
    fn use_canvas(&mut self, slot: Slot) {
        if self.canvas_slot == slot {
            return;
        }
        let next = self.parked.remove(&slot).unwrap_or_else(|| match slot {
            Slot::Exercise => Canvas::new(),
            Slot::Free | Slot::Study => Canvas::new_free(),
        });
        let prev = std::mem::replace(&mut self.canvas, next);
        self.parked.insert(self.canvas_slot, prev);
        self.canvas_slot = slot;
        self.canvas.mark_all();
    }

    fn open_free(&mut self) {
        self.use_canvas(Slot::Free);
        self.screen = Screen::Free;
        self.grade = None;
        self.clear_result();
        self.paint_ref = None;
    }

    fn grade_now(&mut self) {
        let Some(ex) = self.current_exercise().cloned() else { return };
        let strokes = &self.canvas.strokes;
        let g = match &ex.kind {
            Kind::Trace { guides, tol } => {
                // Las guías de contexto ya están dibujadas: no se califican.
                let graded: Vec<Guide> = guides.iter().filter(|g| g.role != Role::Context).cloned().collect();
                grade_trace(strokes, &graded, *tol, self.smoother.opts.magnet)
            }
            Kind::Points { guides, tol } => grade_points(strokes, guides, *tol),
            Kind::Copy { guides, tol } => grade_copy(strokes, guides, *tol),
            Kind::Perspective(scene) => grade_perspective(strokes, scene),
            Kind::Hatch { regions, tol } => grade_hatch(strokes, regions, *tol),
            Kind::Paint(_) => match &self.paint_ref {
                Some(r) => grade_paint(&self.canvas, &r.target, &r.mask, r.mode),
                None => return,
            },
        };
        self.marks = error_marks(&ex.kind, &self.canvas.strokes);
        self.show_marks = true;
        self.progress.record(&ex.id, g.total, g.stars);
        // Cuenta para el calentamiento de hoy en el programa del año.
        self.progress.program.record_exercise(&ex.id, &crate::program::today());
        self.grade = Some(g);
        self.save();
    }

    fn next_exercise(&mut self, ctx: &egui::Context) {
        if let Screen::Exercise(m, e) = self.screen {
            match self.by_flat(self.flat_index(m, e) + 1) {
                Some((nm, ne)) if self.unlocked(nm, ne) => self.open_exercise(ctx, nm, ne),
                _ => self.screen = Screen::Map,
            }
        }
    }

    // ------------------------------------------------------------------ entrada del lápiz

    fn magnet(&self, p: Pos2) -> Pos2 {
        let strength = self.smoother.opts.magnet;
        if strength <= 0.0 || !self.guide_visible() {
            return p;
        }
        let Some(Exercise { kind: Kind::Trace { guides, .. }, .. }) = self.current_exercise() else { return p };
        let mut best = (f32::MAX, p);
        // Solo atraen las guías que se ven y se repasan.
        for g in guides.iter().filter(|g| g.role == Role::Trace) {
            for w in g.pts.windows(2) {
                let ab = w[1] - w[0];
                let t = ((p - w[0]).dot(ab) / ab.length_sq().max(1e-6)).clamp(0.0, 1.0);
                let q = w[0] + ab * t;
                let d = (p - q).length();
                if d < best.0 {
                    best = (d, q);
                }
            }
        }
        let radius = 40.0;
        if best.0 > radius {
            return p;
        }
        let fall = 1.0 - best.0 / radius;
        p + (best.1 - p) * (strength * fall)
    }

    /// Ejercicios con tiempo límite: al acabarse, califica lo dibujado y bloquea el lienzo.
    fn check_timer(&mut self) {
        let Some(limit) = self.time_limit() else { return };
        if !self.time_up && self.now_ms() - self.ex_started >= limit as f64 * 1000.0 {
            self.time_up = true;
            self.grade_now();
        }
    }

    /// Tiempo del ejercicio actual: el de la sesión de gestos si hay una, o el del propio ejercicio.
    fn time_limit(&self) -> Option<f32> {
        let ex = self.current_exercise()?;
        match &self.session {
            Some(s) => Some(s.secs),
            None => ex.time_limit,
        }
    }

    /// Empieza una sesión con `count` poses al azar de los ejercicios de gesto, de `secs` segundos.
    fn start_session(&mut self, ctx: &egui::Context, count: usize, secs: f32) {
        let mut pool: Vec<(usize, usize)> = Vec::new();
        for (m, md) in self.modules.iter().enumerate() {
            for (e, ex) in md.exercises.iter().enumerate() {
                if ex.time_limit.is_some_and(|t| t >= 30.0) && matches!(ex.kind, Kind::Copy { .. }) {
                    pool.push((m, e));
                }
            }
        }
        if pool.is_empty() {
            return;
        }
        // Barajar (Fisher-Yates) con la misma fuente de semillas que las variaciones.
        let mut z = self.next_seed();
        for i in (1..pool.len()).rev() {
            z = z.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
            pool.swap(i, (z >> 33) as usize % (i + 1));
        }
        let mut queue: Vec<(usize, usize)> = pool.iter().cycle().take(count).copied().collect();
        queue.reverse();
        let first = queue.pop().expect("al menos una pose");
        self.session = Some(Session { queue, secs, total: count, scores: Vec::new(), next_at: None });
        self.open_exercise(ctx, first.0, first.1);
    }

    /// Avanza la sesión de gestos: al acabarse el tiempo de una pose, espera un momento y abre la
    /// siguiente; al terminar, deja un resumen.
    fn session_step(&mut self, ctx: &egui::Context) {
        if self.session.is_none() {
            return;
        }
        if !matches!(self.screen, Screen::Exercise(..)) {
            self.session = None;
            return;
        }
        if !self.time_up {
            return;
        }
        let now = self.now_ms();
        let total = self.grade.as_ref().map_or(0.0, |g| g.total);
        let s = self.session.as_mut().expect("sesión");
        let at = *s.next_at.get_or_insert_with(|| {
            s.scores.push(total);
            now + 2500.0
        });
        if now < at {
            return;
        }
        s.next_at = None;
        match s.queue.pop() {
            Some((m, e)) => self.open_exercise(ctx, m, e),
            None => {
                let n = s.scores.len();
                let mean = s.scores.iter().sum::<f32>() / n.max(1) as f32;
                self.message = Some((format!("Sesión terminada: {n} poses, media {mean:.0}/100."), now));
                self.session = None;
                // Cuenta como el gesto del día en el recorrido.
                self.progress.program.record_gesture(&crate::program::today());
                self.save();
            }
        }
    }

    fn guide_visible(&self) -> bool {
        self.show_guide && self.memory_visible()
    }

    /// En los ejercicios de memoria, las ayudas solo se ven los primeros segundos.
    fn memory_visible(&self) -> bool {
        match self.current_exercise().and_then(|e| e.memory) {
            Some(sec) => self.now_ms() - self.ex_started < sec as f64 * 1000.0,
            None => true,
        }
    }

    fn pressure_from(&mut self, force: Option<f32>, speed: f32, now: f64) -> f32 {
        if let Some(f) = force {
            self.last_force = Some((f, now));
        }
        let tablet = self.last_force.filter(|(_, t)| now - t < 400.0).map(|(f, _)| f);
        let raw = tablet.unwrap_or_else(|| {
            simulated(self.progress.pressure.mouse_mode, self.progress.pressure.mouse_constant, speed)
        });
        self.raw_pressure = raw;
        self.curved_pressure = self.progress.pressure.curve.value(raw);
        self.curved_pressure
    }

    fn tablet_active(&self) -> bool {
        self.last_force.is_some_and(|(_, t)| self.now_ms() - t < 2000.0)
    }

    /// `allow`: falso mientras se mueve o gira la vista (no se empieza a dibujar).
    fn handle_canvas_input(&mut self, ui: &Ui, resp: &egui::Response, xf: Xf, allow: bool) {
        self.smoother.zoom = xf.scale;
        let to_canvas = |p: Pos2| xf.to_canvas(p);
        let now = self.now_ms();
        let (moves, force) = ui.ctx().input(|i| {
            let mut moves = Vec::new();
            let mut force = None;
            for ev in &i.events {
                match ev {
                    Event::PointerMoved(p) => moves.push(*p),
                    Event::Touch { force: Some(f), .. } => force = Some(*f),
                    _ => {}
                }
            }
            (moves, force)
        });
        let brush = self.brushes[self.brush].clone();
        let mut segs: Vec<Segment> = Vec::new();
        let picking = ui.input(|i| i.modifiers.ctrl);
        if picking && allow && (resp.drag_started_by(egui::PointerButton::Primary) || resp.clicked()) {
            if let Some(p) = resp.interact_pointer_pos() {
                let q = to_canvas(p);
                if q.x >= 0.0 && q.y >= 0.0 && q.x < W as f32 && q.y < H as f32 {
                    // Se toma el color real, aunque la vista esté en grises.
                    self.canvas.color = self.canvas.pixel(q.x as usize, q.y as usize);
                }
            }
            return;
        }
        if resp.drag_started_by(egui::PointerButton::Primary) && !self.time_up && allow {
            if let Some(p) = resp.interact_pointer_pos() {
                self.speed.reset();
                let cp = self.magnet(to_canvas(p));
                let sp = self.speed.next(cp, now);
                let pr = self.pressure_from(force, sp, now);
                self.clear_result();
                self.canvas.begin_stroke();
                self.smoother.begin(PInfo { pos: cp, pressure: pr, time: now, speed: sp }, &mut segs);
                self.drawing = true;
            }
        }
        if self.drawing {
            let n = moves.len();
            for (k, p) in moves.into_iter().enumerate() {
                let t = now - (n - k - 1) as f64 * 4.0;
                let cp = self.magnet(to_canvas(p));
                let sp = self.speed.next(cp, t);
                let pr = self.pressure_from(force, sp, t);
                self.smoother.add(PInfo { pos: cp, pressure: pr, time: t, speed: sp }, &mut segs);
            }
            self.smoother.tick(now, &mut segs);
            let released = resp.drag_stopped() || !ui.ctx().input(|i| i.pointer.primary_down());
            if released {
                self.smoother.end(&mut segs);
            }
            for s in &segs {
                self.canvas.paint(s, &brush);
            }
            if released {
                self.canvas.end_stroke();
                self.drawing = false;
            }
        } else {
            for s in &segs {
                self.canvas.paint(s, &brush);
            }
        }
    }

    /// Modo demostración (variable TRAZO_DEMO=<id de ejercicio>): abre el ejercicio, traza
    /// sobre las guías con un pulso algo tembloroso a través del asistente y califica.
    fn demo_step(&mut self, ctx: &egui::Context) {
        if self.frame == 2 {
            match std::env::var("TRAZO_PANTALLA").as_deref() {
                Ok("presion") => self.screen = Screen::Pressure,
                Ok("guia") => self.screen = Screen::Guide,
                Ok("libre") => self.open_free(),
                Ok("mapa") => self.screen = Screen::Map,
                Ok("galeria") => self.screen = Screen::Gallery,
                Ok("sesion") => self.start_session(ctx, 5, 60.0),
                Ok("diario") => self.screen = Screen::Diary,
                Ok("hoy") => self.demo_program(ctx),
                Ok("estudio") => self.demo_study(ctx),
                _ => {}
            }
            // TRAZO_VISTA=giro o espejo: vista girada (y volteada) para comprobar la transformación.
            if let Ok(v) = std::env::var("TRAZO_VISTA") {
                self.view = View { zoom: 1.2, pan: vec2(-40.0, 10.0), rot: 0.35, mirror: v == "espejo" };
            }
        }
        let Ok(id) = std::env::var("TRAZO_DEMO") else { return };
        if self.frame == 2 {
            for (m, md) in self.modules.iter().enumerate() {
                if let Some(e) = md.exercises.iter().position(|x| x.id == id) {
                    self.open_exercise(ctx, m, e);
                    break;
                }
            }
        }
        if self.frame != 6 {
            if self.frame == 100 {
                self.grade_now();
            }
            return;
        }
        let Some(ex) = self.current_exercise().cloned() else { return };
        // Pintura: el objetivo exacto, para ver en la captura cómo queda la escena.
        if let (Kind::Paint(_), Some(r)) = (&ex.kind, &self.paint_ref) {
            self.canvas.base = r.target.clone();
            self.canvas.mark_all();
            return;
        }
        let brush = self.brushes[self.brush].clone();
        let mut t = self.now_ms();
        // Recorre la polilínea con un pulso algo tembloroso.
        let shaky = |pts: &[Pos2], pressure: Option<&Vec<f32>>| -> Vec<(Pos2, f32)> {
            pts.iter()
                .enumerate()
                .map(|(i, p)| {
                    let wob = (i as f32 * 0.7).sin() * 2.5 + (i as f32 * 2.3).cos() * 1.2;
                    let pr = pressure.map_or(0.7, |pp| pp[i]);
                    (*p + vec2(wob, -wob * 0.6), pr)
                })
                .collect()
        };
        let strokes: Vec<Vec<(Pos2, f32)>> = match &ex.kind {
            Kind::Trace { guides, .. } | Kind::Points { guides, .. } => {
                guides.iter().filter(|g| g.role != Role::Context).map(|g| shaky(&g.pts, g.pressure.as_ref())).collect()
            }
            Kind::Copy { guides, .. } => {
                // Copia más pequeña y desplazada, como haría un alumno.
                let c = pos2(W as f32 / 2.0, H as f32 / 2.0);
                guides
                    .iter()
                    .map(|g| {
                        let pts: Vec<Pos2> = g.pts.iter().map(|p| c + (*p - c) * 0.7 + vec2(-60.0, 30.0)).collect();
                        shaky(&pts, None)
                    })
                    .collect()
            }
            Kind::Perspective(scene) => scene
                .tasks
                .iter()
                .map(|t| {
                    let (a, b) = scene.target(t);
                    let n = ((b - a).length() / 3.0).ceil().max(2.0) as usize;
                    let pts: Vec<Pos2> = (0..=n).map(|i| a + (b - a) * (i as f32 / n as f32)).collect();
                    shaky(&pts, None)
                })
                .collect(),
            Kind::Hatch { regions, .. } => regions
                .iter()
                .flat_map(|r| r.ideal_lines())
                .map(|(a, b)| {
                    let n = ((b - a).length() / 3.0).ceil().max(2.0) as usize;
                    let pts: Vec<Pos2> = (0..=n).map(|i| a + (b - a) * (i as f32 / n as f32)).collect();
                    shaky(&pts, None)
                })
                .collect(),
            Kind::Paint(_) => (0..12)
                .map(|k| (0..60).map(|i| (pos2(150.0 + i as f32 * 11.0, 230.0 + k as f32 * 16.0), 0.3 + 0.05 * k as f32)).collect())
                .collect(),
        };
        for s in strokes {
            let mut segs = Vec::new();
            self.canvas.begin_stroke();
            for (i, (p, pr)) in s.iter().enumerate() {
                let info = PInfo { pos: *p, pressure: *pr, time: t, speed: 0.2 };
                if i == 0 {
                    self.smoother.begin(info, &mut segs);
                } else {
                    self.smoother.add(info, &mut segs);
                    self.smoother.tick(t, &mut segs);
                }
                t += 8.0;
            }
            self.smoother.end(&mut segs);
            for sg in &segs {
                self.canvas.paint(sg, &brush);
            }
            self.canvas.end_stroke();
        }
    }

    fn shortcuts(&mut self, ctx: &egui::Context) {
        if ctx.egui_wants_keyboard_input() {
            return;
        }
        let (undo, bigger, smaller, eraser) = ctx.input(|i| {
            (
                i.modifiers.command && i.key_pressed(egui::Key::Z),
                i.key_pressed(egui::Key::CloseBracket),
                i.key_pressed(egui::Key::OpenBracket),
                !i.modifiers.command && i.key_pressed(egui::Key::E),
            )
        });
        if undo && !self.drawing {
            self.clear_result();
            self.canvas.undo();
        }
        if bigger {
            self.canvas.size_mult = (self.canvas.size_mult * 1.15).min(6.0);
        }
        if smaller {
            self.canvas.size_mult = (self.canvas.size_mult / 1.15).max(0.15);
        }
        if eraser {
            self.toggle_eraser();
        }
        // Vista del lienzo: solo en las pantallas con lienzo.
        if !matches!(self.screen, Screen::Exercise(..) | Screen::Free | Screen::Study) {
            return;
        }
        let (mirror, zin, zout, fit, left, right, straight) = ctx.input(|i| {
            let plain = !i.modifiers.command && !i.modifiers.alt;
            (
                plain && i.key_pressed(egui::Key::M),
                plain && (i.key_pressed(egui::Key::Plus) || i.key_pressed(egui::Key::Equals)),
                plain && i.key_pressed(egui::Key::Minus),
                plain && i.key_pressed(egui::Key::Num0),
                plain && i.key_pressed(egui::Key::Num4),
                plain && i.key_pressed(egui::Key::Num6),
                plain && i.key_pressed(egui::Key::Num5),
            )
        });
        let area = self.last_area;
        if mirror {
            self.view.mirror = !self.view.mirror;
        }
        if zin {
            self.view.zoom_at(area, area.center(), 1.25);
        }
        if zout {
            self.view.zoom_at(area, area.center(), 1.0 / 1.25);
        }
        if fit {
            self.view = View::default();
        }
        let c = area.center() + self.view.pan;
        if left {
            self.view.rotate_at(area, c, -15f32.to_radians());
        }
        if right {
            self.view.rotate_at(area, c, 15f32.to_radians());
        }
        if straight {
            self.view.rotate_at(area, c, -self.view.rot);
        }
    }

    fn toggle_eraser(&mut self) {
        if let Some(prev) = self.eraser_return.take() {
            self.brush = prev;
        } else if let Some(e) = self.brushes.iter().position(|b| b.kind == BrushKind::Eraser) {
            if self.brush != e {
                self.eraser_return = Some(self.brush);
                self.brush = e;
            }
        }
    }

    // ------------------------------------------------------------------ interfaz

    // ------------------------------------------------------------------ interfaz

    fn top_bar(&mut self, ui: &mut Ui) {
        egui::Panel::top("barra")
            .frame(egui::Frame::new().fill(theme::BG).inner_margin(egui::Margin::symmetric(16, 8)))
            .show(ui, |ui| {
                ui.horizontal(|ui| {
                    logo(ui);
                    ui.label(theme::strong("Trazo Academia", 18.0));
                    ui.add_space(18.0);
                    let tabs = [
                        (Screen::Home, Icon::Home, "Inicio"),
                        (Screen::Program, Icon::Play, "Hoy"),
                        (Screen::Map, Icon::Map, "Niveles"),
                        (Screen::Free, Icon::Brush, "Lienzo libre"),
                        (Screen::Study, Icon::Copy, "Estudio"),
                        (Screen::Diary, Icon::Check, "Diario"),
                        (Screen::Gallery, Icon::Image, "Galería"),
                        (Screen::Pressure, Icon::Pressure, "Presión"),
                        (Screen::Guide, Icon::Book, "Guía de Krita"),
                    ];
                    for (s, icon, label) in tabs {
                        let selected = match (self.screen, s) {
                            (Screen::Exercise(..), Screen::Map) => true,
                            (a, b) => a == b,
                        };
                        if theme::button(ui, Some(icon), label, if selected { Btn::Selected } else { Btn::Ghost }).clicked() {
                            match s {
                                Screen::Free => self.open_free(),
                                Screen::Study => self.open_study(),
                                other => self.screen = other,
                            }
                        }
                    }
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        let txt = format!("{} / {}", self.progress.total_stars(), self.total_stars_possible());
                        ui.label(theme::strong(txt, theme::LEAD).color(GOLD));
                        theme::icon_widget(ui, Icon::Star, 18.0, GOLD);
                    });
                });
            });
    }

    fn home_screen(&mut self, ui: &mut Ui) {
        let ctx = ui.ctx().clone();
        let mut open: Option<(usize, usize)> = None;
        let mut go: Option<Screen> = None;
        let mut start: Option<(usize, f32)> = None;
        egui::CentralPanel::default().frame(egui::Frame::new().fill(theme::BG).inner_margin(28)).show(ui, |ui| {
            egui::ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
                centered(ui, 1080.0, |ui| {
                    ui.label(theme::strong("Aprende a dibujar en digital", theme::DISPLAY));
                    ui.label(theme::body("Ejercicios guiados con los pinceles y el suavizado de Krita. Lo que practicas aquí funciona igual en Krita."));
                    ui.add_space(20.0);
                    if self.home_today_card(ui) {
                        go = Some(Screen::Program);
                    }
                    ui.add_space(14.0);
                    match self.next_target() {
                        Some((m, e)) => {
                            if self.continue_card(ui, m, e) {
                                open = Some((m, e));
                            }
                        }
                        None => {
                            theme::card(theme::RAISED).inner_margin(22).show(ui, |ui| {
                                ui.set_width(ui.available_width());
                                ui.label(theme::strong("¡Tienes tres estrellas en todo!", theme::TITLE));
                                ui.label(theme::body("Repite tus ejercicios favoritos: cada intento es una variación nueva. O dibuja en el lienzo libre."));
                            });
                        }
                    }
                    ui.add_space(16.0);
                    let total_ex: usize = self.modules.iter().map(|m| m.exercises.len()).sum();
                    let done_ex: usize = (0..self.modules.len()).map(|m| self.module_done(m)).sum();
                    let open_mods = (0..self.modules.len()).filter(|m| self.module_unlocked(*m)).count();
                    let (stars, max) = (self.progress.total_stars(), self.total_stars_possible());
                    let n_mods = self.modules.len();
                    ui.columns(3, |cols| {
                        stat_card(&mut cols[0], Icon::Star, GOLD, &stars.to_string(), &format!("de {max} estrellas"), stars as f32 / max.max(1) as f32);
                        stat_card(&mut cols[1], Icon::Check, theme::GOOD, &done_ex.to_string(), &format!("de {total_ex} ejercicios superados"), done_ex as f32 / total_ex.max(1) as f32);
                        stat_card(&mut cols[2], Icon::Map, ACCENT, &open_mods.to_string(), &format!("de {n_mods} módulos abiertos"), open_mods as f32 / n_mods.max(1) as f32);
                    });
                    ui.add_space(20.0);
                    theme::section(ui, "Módulos");
                    for m in 0..self.modules.len() {
                        if self.module_row(ui, m).clicked() {
                            self.map_focus = Some(m);
                            go = Some(Screen::Map);
                        }
                    }
                    ui.add_space(20.0);
                    theme::section(ui, "Estudia del natural");
                    ui.columns(2, |cols| {
                        if action_card(&mut cols[0], Icon::Copy, "Estudio con referencia", "Dibuja a partir de una foto, compárala con tu dibujo y critícalo tú mismo.").clicked() {
                            go = Some(Screen::Study);
                        }
                        if action_card(&mut cols[1], Icon::Check, "Diario de estudios", "Tus estudios con su referencia, tu autocrítica y lo que querías mejorar.").clicked() {
                            go = Some(Screen::Diary);
                        }
                    });
                    ui.add_space(20.0);
                    theme::section(ui, "Practica a tu aire");
                    ui.columns(4, |cols| {
                        if action_card(&mut cols[0], Icon::Brush, "Lienzo libre", "Dibuja sin guías, con capas, referencia y rejilla.").clicked() {
                            go = Some(Screen::Free);
                        }
                        if action_card(&mut cols[1], Icon::Play, "Sesión de gestos", "Cinco poses al azar, un minuto cada una.").clicked() {
                            start = Some((5, 60.0));
                        }
                        if action_card(&mut cols[2], Icon::Play, "Gestos rápidos", "Ocho poses al azar, treinta segundos cada una.").clicked() {
                            start = Some((8, 30.0));
                        }
                        if action_card(&mut cols[3], Icon::Pressure, "Calibrar la presión", "Ajusta la curva del lápiz a tu mano, como en Krita.").clicked() {
                            go = Some(Screen::Pressure);
                        }
                    });
                    ui.add_space(12.0);
                });
            });
        });
        if let Some((n, secs)) = start {
            self.start_session(&ctx, n, secs);
        } else if let Some((m, e)) = open {
            self.open_exercise(&ctx, m, e);
        } else if let Some(s) = go {
            match s {
                Screen::Free => self.open_free(),
                Screen::Study => self.open_study(),
                s => self.screen = s,
            }
        }
    }

    /// Tarjeta grande del inicio con el ejercicio que toca. Devuelve si se pulsó Continuar.
    fn continue_card(&self, ui: &mut Ui, m: usize, e: usize) -> bool {
        let md = &self.modules[m];
        let ex = &md.exercises[e];
        let (icon, kind) = kind_info(&ex.kind);
        let stars = self.progress.stars(&ex.id);
        let mut go = false;
        theme::card(theme::RAISED).stroke(Stroke::new(1.0, md.accent.gamma_multiply(0.6))).inner_margin(22).show(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.horizontal(|ui| {
                ui.vertical(|ui| {
                    ui.set_width((ui.available_width() - 230.0).max(240.0));
                    theme::section(ui, if stars == 0 { "Siguiente ejercicio" } else { "Para mejorar" });
                    ui.horizontal(|ui| {
                        theme::chip(ui, None, &format!("{} · {}", m + 1, md.title), md.accent);
                        theme::chip(ui, Some(icon), kind, theme::TEXT_2);
                    });
                    ui.add_space(4.0);
                    ui.label(theme::strong(ex.title, 24.0));
                    ui.label(theme::body(ex.goal));
                });
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if theme::button_ex(ui, Some(Icon::Play), "Continuar", Btn::Primary, 190.0, false).clicked() {
                        go = true;
                    }
                });
            });
        });
        go
    }

    /// Fila de un módulo en el inicio: número, título y progreso.
    fn module_row(&self, ui: &mut Ui, m: usize) -> egui::Response {
        let md = &self.modules[m];
        let unlocked = self.module_unlocked(m);
        let n = md.exercises.len();
        let done = self.module_done(m);
        let stars: u32 = md.exercises.iter().map(|e| self.progress.stars(&e.id) as u32).sum();
        let (rect, resp) = ui.allocate_exact_size(vec2(ui.available_width(), 50.0), Sense::click());
        let hovered = resp.hovered();
        let p = ui.painter();
        p.rect(rect, 10, if hovered { theme::HOVER } else { theme::RAISED }, Stroke::new(1.0, theme::BORDER), StrokeKind::Inside);
        badge(p, pos2(rect.min.x + 28.0, rect.center().y), m + 1, if unlocked { md.accent } else { Color32::from_gray(80) });
        p.text(
            pos2(rect.min.x + 56.0, rect.center().y),
            Align2::LEFT_CENTER,
            md.title,
            FontId::new(theme::LEAD, theme::semibold()),
            if unlocked { theme::TEXT } else { theme::TEXT_3 },
        );
        let x = rect.max.x - 380.0;
        if unlocked {
            theme::paint_progress(p, Rect::from_min_size(pos2(x, rect.center().y - 3.0), vec2(180.0, 6.0)), stars as f32 / (3 * n).max(1) as f32, md.accent);
            p.text(pos2(x + 196.0, rect.center().y), Align2::LEFT_CENTER, format!("{done} de {n} superados"), FontId::proportional(theme::CAPTION), theme::TEXT_2);
        } else {
            theme::paint_icon(p, Rect::from_center_size(pos2(x + 8.0, rect.center().y), vec2(15.0, 15.0)), Icon::Lock, theme::TEXT_3);
            p.text(pos2(x + 24.0, rect.center().y), Align2::LEFT_CENTER, "Bloqueado", FontId::proportional(theme::CAPTION), theme::TEXT_3);
        }
        theme::paint_icon(p, Rect::from_center_size(pos2(rect.max.x - 22.0, rect.center().y), vec2(13.0, 13.0)), Icon::Next, theme::TEXT_3);
        if hovered {
            ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
        }
        resp
    }

    fn map_screen(&mut self, ui: &mut Ui) {
        let ctx = ui.ctx().clone();
        let mut open: Option<(usize, usize)> = None;
        egui::CentralPanel::default().frame(egui::Frame::new().fill(theme::BG).inner_margin(24)).show(ui, |ui| {
            egui::ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
                centered(ui, 1180.0, |ui| {
                    ui.label(theme::strong("Niveles", theme::TITLE + 4.0));
                    ui.label(theme::body(
                        "Consigue al menos una estrella en la mitad de los ejercicios de un módulo para abrir el siguiente. Dentro de un módulo abierto puedes hacerlos en el orden que quieras.",
                    ));
                    if ui.checkbox(&mut self.progress.unlock_all, "Desbloquear todo (modo profesor)").changed() {
                        self.save();
                    }
                    ui.add_space(12.0);
                    let next = self.next_target();
                    for m in 0..self.modules.len() {
                        let (o, rect) = self.module_card(ui, m, next);
                        if o.is_some() {
                            open = o;
                        }
                        if self.map_focus == Some(m) {
                            ui.scroll_to_rect(rect, Some(egui::Align::TOP));
                            self.map_focus = None;
                        }
                        ui.add_space(14.0);
                    }
                });
            });
        });
        if let Some((m, e)) = open {
            self.open_exercise(&ctx, m, e);
        }
    }

    /// Tarjeta de un módulo en el mapa. Devuelve el ejercicio pulsado y el rectángulo de la tarjeta.
    fn module_card(&self, ui: &mut Ui, m: usize, next: Option<(usize, usize)>) -> (Option<(usize, usize)>, Rect) {
        let unlocked = self.module_unlocked(m);
        let md = &self.modules[m];
        let accent = md.accent;
        let n = md.exercises.len();
        let done = self.module_done(m);
        let stars: u32 = md.exercises.iter().map(|e| self.progress.stars(&e.id) as u32).sum();
        let mut open = None;
        let resp = theme::card(if unlocked { theme::RAISED } else { theme::SURFACE }).inner_margin(18).show(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.horizontal(|ui| {
                let (r, _) = ui.allocate_exact_size(vec2(34.0, 34.0), Sense::hover());
                badge(ui.painter(), r.center(), m + 1, if unlocked { accent } else { Color32::from_gray(80) });
                ui.vertical(|ui| {
                    ui.label(theme::strong(md.title, 19.0).color(if unlocked { theme::TEXT } else { theme::TEXT_2 }));
                    ui.label(theme::body(md.subtitle));
                });
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if unlocked {
                        ui.label(theme::caption(format!("{done} de {n} superados")));
                        theme::progress(ui, stars as f32 / (3 * n).max(1) as f32, accent, 140.0, 6.0);
                    } else {
                        theme::chip(ui, Some(Icon::Lock), "Bloqueado", theme::TEXT_3);
                    }
                });
            });
            if !unlocked && m > 0 {
                let need = self.module_needed(m - 1).saturating_sub(self.module_done(m - 1));
                let s = if need == 1 { "" } else { "s" };
                ui.label(RichText::new(format!("Consigue al menos una estrella en {need} ejercicio{s} más del módulo {m} para abrirlo.")).size(theme::CAPTION).color(GOLD));
            }
            egui::CollapsingHeader::new(RichText::new("Lección").size(theme::BODY).color(accent)).id_salt(("leccion", m)).show(ui, |ui| {
                ui.label(theme::body(md.lesson));
            });
            ui.add_space(4.0);
            ui.horizontal_wrapped(|ui| {
                ui.spacing_mut().item_spacing = vec2(10.0, 10.0);
                for (e, ex) in md.exercises.iter().enumerate() {
                    let st = self.progress.stars(&ex.id);
                    if exercise_tile(ui, e + 1, ex, st, unlocked, accent, next == Some((m, e))).clicked() && unlocked {
                        open = Some((m, e));
                    }
                }
            });
        });
        (open, resp.response.rect)
    }

    fn tools_panel(&mut self, ui: &mut Ui, width: f32) {
        egui::Panel::left("herramientas")
            .resizable(false)
            .exact_size(width)
            .frame(egui::Frame::new().fill(theme::SURFACE).inner_margin(14))
            .show(ui, |ui| {
                egui::ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
                    theme::section(ui, "Pincel");
                    let mut last_cat = "";
                    for i in 0..self.brushes.len() {
                        let b = &self.brushes[i];
                        if b.category != last_cat {
                            last_cat = b.category;
                            ui.add_space(2.0);
                            ui.label(theme::caption(b.category));
                        }
                        let (name, hover, default_color) = (b.name, format!("En Krita: {}\n\n{}", b.krita_name, b.description), b.default_color);
                        let w = ui.available_width();
                        let kind = if self.brush == i { Btn::Selected } else { Btn::Ghost };
                        if theme::button_ex(ui, None, name, kind, w, true).on_hover_text(hover).clicked() {
                            self.brush = i;
                            self.eraser_return = None;
                            if let Some(c) = default_color {
                                if self.current_exercise().and_then(|e| e.color).is_none() {
                                    self.canvas.color = c;
                                }
                            }
                        }
                    }
                    ui.add_space(4.0);
                    let b = &self.brushes[self.brush];
                    ui.label(RichText::new(b.description).size(theme::CAPTION).color(theme::TEXT_2));
                    ui.label(RichText::new(format!("En Krita: {}", b.krita_name)).size(theme::CAPTION).color(ACCENT));
                    theme::section(ui, "Ajustes");
                    // Rótulo encima y deslizador a todo el ancho, para que quepa en el panel.
                    ui.spacing_mut().slider_width = (ui.available_width() - 66.0).max(60.0);
                    ui.label(theme::caption("Tamaño"));
                    ui.add(egui::Slider::new(&mut self.canvas.size_mult, 0.15..=6.0).logarithmic(true).max_decimals(2));
                    ui.label(theme::caption("Opacidad"));
                    ui.add(egui::Slider::new(&mut self.canvas.opacity_mult, 0.05..=1.0).max_decimals(2));
                    ui.label(theme::caption("Atajos como en Krita: [ y ] tamaño, E borrador, Ctrl+Z deshacer, Ctrl + clic toma un color del lienzo."));
                    theme::section(ui, "Color");
                    let mut rgb = self.canvas.color;
                    ui.horizontal(|ui| {
                        ui.color_edit_button_rgb(&mut rgb);
                        let c = Color32::from_rgb(to8(rgb[0]), to8(rgb[1]), to8(rgb[2]));
                        ui.label(RichText::new(format!("#{:02X}{:02X}{:02X}", c.r(), c.g(), c.b())).monospace().color(theme::TEXT_2));
                    });
                    // Grises primero: el dibujo en blanco y negro es lo principal.
                    let swatches: [[f32; 3]; 12] = [
                        [0.05, 0.05, 0.07], [0.3, 0.3, 0.32], [0.55, 0.55, 0.57], [0.8, 0.8, 0.8], [0.985, 0.98, 0.965], [0.85, 0.35, 0.2],
                        [0.95, 0.75, 0.25], [0.35, 0.65, 0.3], [0.2, 0.45, 0.85], [0.45, 0.3, 0.7], [0.8, 0.3, 0.5], [0.45, 0.3, 0.2],
                    ];
                    ui.horizontal_wrapped(|ui| {
                        ui.spacing_mut().item_spacing = vec2(6.0, 6.0);
                        for s in swatches {
                            let (r, resp) = ui.allocate_exact_size(vec2(24.0, 22.0), Sense::click());
                            let current = s == rgb;
                            ui.painter().rect_filled(r, 5, Color32::from_rgb(to8(s[0]), to8(s[1]), to8(s[2])));
                            ui.painter().rect_stroke(r, 5, Stroke::new(if current { 2.0 } else { 1.0 }, if current { ACCENT } else { theme::BORDER }), StrokeKind::Inside);
                            if resp.clicked() {
                                rgb = s;
                            }
                        }
                    });
                    self.canvas.color = rgb;
                    theme::section(ui, "Lienzo");
                    let w = ui.available_width();
                    let can_undo = self.canvas.can_undo();
                    ui.add_enabled_ui(can_undo, |ui| {
                        if theme::button_ex(ui, Some(Icon::Undo), "Deshacer", Btn::Outline, w, true).clicked() {
                            self.clear_result();
                            self.canvas.undo();
                        }
                    });
                    let clear_label = if matches!(self.screen, Screen::Free | Screen::Study) { "Vaciar la capa" } else { "Borrar todo" };
                    if theme::button_ex(ui, Some(Icon::Trash), clear_label, Btn::Outline, w, true).clicked() {
                        if let Some(r) = &self.paint_ref {
                            let start = r.start.clone();
                            self.canvas.reset_with(start);
                        } else {
                            self.canvas.clear();
                        }
                        self.grade = None;
                        self.clear_result();
                    }
                });
            });
    }

    fn assist_panel(&mut self, ui: &mut Ui, width: f32) {
        egui::Panel::right("asistente")
            .resizable(false)
            .exact_size(width)
            .frame(egui::Frame::new().fill(theme::SURFACE).inner_margin(16))
            .show(ui, |ui| {
                egui::ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
                    if let Some(ex) = self.current_exercise().cloned() {
                        let Screen::Exercise(m, e) = self.screen else { return };
                        let (md_title, accent, count) = (self.modules[m].title, self.modules[m].accent, self.modules[m].exercises.len());
                        let (icon, kind) = kind_info(&ex.kind);
                        ui.horizontal_wrapped(|ui| {
                            theme::chip(ui, None, &format!("{} · {}", m + 1, md_title), accent);
                            theme::chip(ui, Some(icon), kind, theme::TEXT_2);
                        });
                        ui.add_space(2.0);
                        if let Some(sn) = &self.session {
                            let n = sn.total - sn.queue.len();
                            ui.label(theme::strong(format!("Sesión de gestos: pose {n} de {}", sn.total), theme::BODY).color(GOLD));
                        }
                        ui.label(theme::caption(format!("Ejercicio {} de {}", e + 1, count)));
                        ui.label(theme::strong(ex.title, theme::TITLE));
                        ui.label(RichText::new(ex.goal).size(theme::BODY).color(theme::TEXT));
                        if let Some(sec) = self.time_limit() {
                            let left = (sec as f64 - (self.now_ms() - self.ex_started) / 1000.0).max(0.0);
                            if self.time_up {
                                ui.label(theme::strong("¡Tiempo!", theme::LEAD).color(GOLD));
                                ui.label(theme::caption(if self.session.is_some() { "La siguiente pose sale en un momento." } else { "Pulsa Repetir para otra pose." }));
                            } else {
                                ui.horizontal(|ui| {
                                    theme::icon_widget(ui, Icon::Play, 16.0, GOLD);
                                    ui.label(theme::strong(format!("Tiempo: {:.0} s", left.ceil()), theme::LEAD).color(GOLD));
                                });
                                let frac = (left / sec as f64) as f32;
                                let w = ui.available_width();
                                theme::progress(ui, frac, GOLD, w, 6.0);
                            }
                        }
                        if let Some(sec) = ex.memory {
                            let left = sec as f64 - (self.now_ms() - self.ex_started) / 1000.0;
                            if left > 0.0 {
                                ui.label(RichText::new(format!("Las ayudas se ocultan en {:.0} s", left.ceil())).size(theme::BODY).color(GOLD));
                            } else if theme::button(ui, Some(Icon::Retry), "Ver las ayudas otra vez", Btn::Outline).clicked() {
                                self.ex_started = self.now_ms();
                            }
                        }
                        if let Kind::Copy { guides, .. } = &ex.kind {
                            theme::section(ui, "Referencia");
                            draw_reference(ui, guides);
                        }
                        if let (Kind::Paint(_), Some(r)) = (&ex.kind, &self.paint_ref) {
                            theme::section(ui, "Referencia");
                            let w = ui.available_width();
                            ui.add(egui::Image::new((r.reference.id(), vec2(w, w * H as f32 / W as f32))).corner_radius(8));
                        }
                        ui.add_space(10.0);
                        let w = ui.available_width();
                        if theme::button_ex(ui, Some(Icon::Check), "Calificar", Btn::Primary, w, false).clicked() {
                            self.grade_now();
                        }
                        let toggle = match &ex.kind {
                            Kind::Trace { .. } if ex.memory.is_none() => Some("Mostrar guía"),
                            Kind::Perspective(s) if s.hints && ex.memory.is_none() => Some("Mostrar ayuda"),
                            _ => None,
                        };
                        if let Some(label) = toggle {
                            ui.checkbox(&mut self.show_guide, label);
                        }
                        // La nota va aquí y no sobre el lienzo, para no tapar el dibujo ni las marcas.
                        self.result_card(ui, m, e);
                        ui.add_space(8.0);
                        krita_box(ui, ex.krita_tip);
                        if let Some(b) = self.progress.results.get(&ex.id).copied() {
                            ui.horizontal(|ui| {
                                ui.label(theme::caption(format!("Mejor: {:.0} · {} intento{}", b.best, b.attempts, if b.attempts == 1 { "" } else { "s" })));
                                let (r, _) = ui.allocate_exact_size(vec2(52.0, 16.0), Sense::hover());
                                theme::stars(ui.painter(), r.center(), 6.0, self.progress.stars(&ex.id));
                            });
                        }
                        ui.add_space(8.0);
                        egui::CollapsingHeader::new(theme::strong("Asistente de trazo", theme::BODY)).id_salt("suavizado_ej").show(ui, |ui| {
                            self.smoothing_ui(ui, matches!(ex.kind, Kind::Trace { .. }));
                        });
                        egui::CollapsingHeader::new(theme::strong("Presión", theme::BODY)).id_salt("presion_ej").show(ui, |ui| {
                            self.pressure_meter(ui);
                        });
                    } else {
                        ui.label(theme::strong("Lienzo libre", theme::TITLE));
                        ui.label(theme::body("Dibuja lo que quieras con los pinceles y el asistente de trazo, sin guías ni calificación."));
                        ui.add_space(8.0);
                        let w = ui.available_width();
                        if theme::button_ex(ui, Some(Icon::Save), "Guardar como PNG", Btn::Primary, w, false).clicked() {
                            self.save_png();
                        }
                        ui.label(theme::caption("Se guarda en Imágenes › TrazoAcademia y aparece en la Galería."));
                        ui.add_space(4.0);
                        self.layers_ui(ui);
                        self.reference_ui(ui);
                        ui.add_space(8.0);
                        egui::CollapsingHeader::new(theme::strong("Asistente de trazo", theme::BODY)).id_salt("suavizado_libre").default_open(true).show(ui, |ui| {
                            self.smoothing_ui(ui, false);
                        });
                        egui::CollapsingHeader::new(theme::strong("Presión", theme::BODY)).id_salt("presion_libre").default_open(true).show(ui, |ui| {
                            self.pressure_meter(ui);
                        });
                    }
                    if let Some((msg, t)) = &self.message {
                        if self.now_ms() - t < 6000.0 {
                            ui.add_space(8.0);
                            ui.label(RichText::new(msg).size(theme::BODY).color(GOLD));
                        }
                    }
                });
            });
    }

    fn exercise_or_free_screen(&mut self, ui: &mut Ui) {
        let ctx = ui.ctx().clone();
        let full = ui.available_width();
        let (lw, rw) = if full < 1320.0 { (200.0, 300.0) } else { (228.0, 348.0) };
        self.tools_panel(ui, lw);
        self.assist_panel(ui, rw);
        let mut nav: Option<(usize, usize)> = None;
        let mut back = false;
        egui::CentralPanel::default().frame(egui::Frame::new().fill(theme::STAGE).inner_margin(12)).show(ui, |ui| {
            ui.horizontal(|ui| {
                if let Screen::Exercise(..) = self.screen {
                    if theme::button(ui, Some(Icon::Map), "Niveles", Btn::Ghost).clicked() {
                        back = true;
                    }
                    ui.add_space(8.0);
                }
                self.view_controls(ui);
                if let Screen::Exercise(m, e) = self.screen {
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        let i = self.flat_index(m, e);
                        let next = self.by_flat(i + 1).filter(|&(nm, ne)| self.unlocked(nm, ne));
                        let prev = if i > 0 { self.by_flat(i - 1) } else { None };
                        ui.add_enabled_ui(next.is_some(), |ui| {
                            if theme::button(ui, Some(Icon::Next), "", Btn::Ghost).on_hover_text("Ejercicio siguiente").clicked() {
                                nav = next;
                            }
                        });
                        ui.add_enabled_ui(prev.is_some(), |ui| {
                            if theme::button(ui, Some(Icon::Prev), "", Btn::Ghost).on_hover_text("Ejercicio anterior").clicked() {
                                nav = prev;
                            }
                        });
                    });
                }
            });
            ui.add_space(4.0);
            let area = ui.available_rect_before_wrap();
            self.last_area = area;
            let resp = ui.interact(area, egui::Id::unique("lienzo"), Sense::drag());
            let navigating = self.navigate_view(ui, &resp, area);
            let xf = self.view.xf(area);
            self.handle_canvas_input(ui, &resp, xf, !navigating);
            let tex = self.canvas.texture(&ctx).id();
            let painter = ui.painter_at(area);
            painter.add(xf.image(tex));
            painter.add(xf.frame(Color32::TRANSPARENT, Stroke::new(1.0, Color32::from_gray(45))));
            if let Some(r) = &self.paint_ref {
                painter.add(xf.image(r.outline.id()));
            }
            if self.screen == Screen::Free {
                self.draw_free_aids(&painter, &xf);
            }
            // Después de calificar se muestran la guía y las ayudas para comparar.
            let reveal = self.grade.is_some() && self.show_marks;
            if let Some(ex) = self.current_exercise() {
                let aids = self.memory_visible() || reveal;
                match &ex.kind {
                    Kind::Trace { guides, .. } => {
                        let size = self.brushes[self.brush].size * self.canvas.size_mult;
                        draw_guides(&painter, guides, &xf, size, self.guide_visible() || reveal, reveal);
                        if ex.numbered && (self.guide_visible() || reveal) {
                            draw_order(&painter, guides, &xf);
                        }
                    }
                    Kind::Hatch { regions, .. } => draw_hatch(&painter, regions, &xf),
                    Kind::Points { guides, .. } if aids => draw_points(&painter, guides, &xf),
                    Kind::Perspective(scene) => {
                        let hints = aids && ((self.show_guide && scene.hints) || reveal);
                        draw_scene(&painter, scene, &xf, aids, hints);
                    }
                    _ => {}
                }
            }
            if self.show_marks {
                for (q, sev) in &self.marks {
                    if *sev < 0.35 {
                        continue;
                    }
                    let t = ((sev - 0.35) / 0.65).clamp(0.0, 1.0);
                    painter.circle_filled(xf.to_screen(*q), 2.5 + 2.0 * t, mix(GOLD, theme::BAD, t));
                }
            }
            // Cursor del pincel y posición estabilizada.
            if !navigating {
                if let Some(hp) = resp.hover_pos().or(resp.interact_pointer_pos()) {
                    let d = (self.brushes[self.brush].size * self.canvas.size_mult * xf.scale).max(4.0);
                    painter.circle_stroke(hp, d * 0.5, Stroke::new(1.0, Color32::from_gray(60)));
                    if let Some(bp) = self.smoother.brush_position() {
                        if self.smoother.opts.kind == SmoothingType::Estabilizador && self.drawing {
                            let sp = xf.to_screen(bp);
                            painter.line_segment([sp, hp], Stroke::new(1.0, ACCENT.gamma_multiply(0.7)));
                            painter.circle_filled(sp, 3.0, ACCENT);
                        }
                    }
                }
            }
            if resp.hovered() {
                ctx.set_cursor_icon(if navigating { egui::CursorIcon::Grabbing } else { egui::CursorIcon::Crosshair });
            }
        });
        if back {
            self.screen = Screen::Map;
        }
        if let Some((m, e)) = nav {
            self.open_exercise(&ctx, m, e);
        }
    }

    /// Mover (espacio o botón central + arrastrar), girar (Mayús + espacio + arrastrar) y acercar
    /// (rueda) la vista. Devuelve si hay un gesto de vista en curso, para no dibujar mientras.
    fn navigate_view(&mut self, ui: &Ui, resp: &egui::Response, area: Rect) -> bool {
        let (space, shift, wheel, pinch, delta, middle, ptr) = ui.input(|i| {
            (
                i.key_down(egui::Key::Space),
                i.modifiers.shift,
                i.smooth_scroll_delta().y,
                i.zoom_delta(),
                i.pointer.delta(),
                i.pointer.middle_down(),
                i.pointer.hover_pos(),
            )
        });
        let typing = ui.ctx().egui_wants_keyboard_input();
        // Con el ratón sobre el lienzo, el foco pasa a él: así la barra espaciadora no pulsa el
        // último botón usado.
        if resp.hovered() && !typing {
            resp.request_focus();
        }
        if resp.hovered() {
            if let Some(at) = ptr {
                if wheel != 0.0 {
                    self.view.zoom_at(area, at, (wheel * 0.0015).exp());
                }
                if pinch != 1.0 {
                    self.view.zoom_at(area, at, pinch);
                }
            }
        }
        let gesture = ((space && !typing) || middle) && !self.drawing;
        if gesture && resp.dragged() {
            if space && shift {
                if let Some(p) = ptr {
                    let c = area.center() + self.view.pan;
                    let (a0, a1) = ((p - delta - c).angle(), (p - c).angle());
                    let mut da = a1 - a0;
                    if da > std::f32::consts::PI {
                        da -= std::f32::consts::TAU;
                    } else if da < -std::f32::consts::PI {
                        da += std::f32::consts::TAU;
                    }
                    self.view.rotate_at(area, c, da);
                }
            } else {
                self.view.pan += delta;
            }
        }
        gesture
    }

    /// Controles de la vista sobre el lienzo.
    fn view_controls(&mut self, ui: &mut Ui) {
        let area = self.last_area;
        if theme::button(ui, None, "−", Btn::Ghost).on_hover_text("Alejar (tecla −)").clicked() {
            self.view.zoom_at(area, area.center(), 1.0 / 1.25);
        }
        ui.label(theme::caption(format!("{:.0} %", self.view.zoom * 100.0)));
        if theme::button(ui, None, "+", Btn::Ghost).on_hover_text("Acercar (tecla +)").clicked() {
            self.view.zoom_at(area, area.center(), 1.25);
        }
        if self.view.rot != 0.0 {
            ui.label(theme::caption(format!("{:.0}°", self.view.degrees())));
        }
        let gray = if self.canvas.gray { Btn::Selected } else { Btn::Ghost };
        if theme::button(ui, None, "Grises", gray)
            .on_hover_text("Ver el lienzo en grises para comprobar los valores (solo en pantalla). En Krita se consigue con una capa de relleno negra en modo Color encima de todo.")
            .clicked()
        {
            self.canvas.gray = !self.canvas.gray;
            self.canvas.mark_all();
        }
        let mirror = if self.view.mirror { Btn::Selected } else { Btn::Ghost };
        if theme::button(ui, None, "Espejo", mirror)
            .on_hover_text("Voltear la vista para ver los errores con ojos frescos (M, como en Krita)")
            .clicked()
        {
            self.view.mirror = !self.view.mirror;
        }
        if self.view != View::default()
            && theme::button(ui, None, "Ajustar", Btn::Ghost).on_hover_text("Ver el lienzo entero y derecho (tecla 0)").clicked()
        {
            self.view = View::default();
        }
        if self.screen == Screen::Free {
            ui.add_space(8.0);
            let g = if self.grid { Btn::Selected } else { Btn::Ghost };
            if theme::button(ui, None, "Rejilla", g).on_hover_text("Cuadrícula de construcción (no se dibuja en la imagen)").clicked() {
                self.grid = !self.grid;
            }
            let t = if self.thirds { Btn::Selected } else { Btn::Ghost };
            if theme::button(ui, None, "Tercios", t).on_hover_text("Líneas de los tercios, para componer").clicked() {
                self.thirds = !self.thirds;
            }
        }
        ui.label(theme::caption("Espacio: mover · Mayús + espacio: girar · rueda: zoom")).on_hover_text(
            "Espacio + arrastrar mueve la vista y Mayús + espacio + arrastrar la gira, como en Krita. El botón central del ratón también mueve. Teclas 4 y 6: girar 15°; 5: enderezar.",
        );
    }

    /// Rejilla y tercios del lienzo libre (solo en pantalla, no en la imagen).
    fn draw_free_aids(&self, p: &egui::Painter, xf: &Xf) {
        let (w, h) = (W as f32, H as f32);
        if self.grid {
            let col = Stroke::new(1.0, Color32::from_rgba_unmultiplied(70, 150, 230, 45));
            let step = 40.0;
            let mut x = step;
            while x < w {
                p.line_segment([xf.to_screen(pos2(x, 0.0)), xf.to_screen(pos2(x, h))], col);
                x += step;
            }
            let mut y = step;
            while y < h {
                p.line_segment([xf.to_screen(pos2(0.0, y)), xf.to_screen(pos2(w, y))], col);
                y += step;
            }
        }
        if self.thirds {
            let col = Stroke::new(1.5, Color32::from_rgba_unmultiplied(230, 120, 70, 110));
            for k in 1..3 {
                let (x, y) = (w * k as f32 / 3.0, h * k as f32 / 3.0);
                p.line_segment([xf.to_screen(pos2(x, 0.0)), xf.to_screen(pos2(x, h))], col);
                p.line_segment([xf.to_screen(pos2(0.0, y)), xf.to_screen(pos2(w, y))], col);
            }
        }
    }

    /// Nota del último intento: estrellas, métricas, consejo y botones para seguir.
    fn result_card(&mut self, ui: &mut Ui, m: usize, e: usize) {
        let Some(g) = self.grade.clone() else { return };
        let ctx = ui.ctx().clone();
        let mut action = None;
        ui.add_space(6.0);
        theme::card(theme::RAISED).stroke(Stroke::new(1.0, ACCENT.gamma_multiply(0.45))).inner_margin(14).show(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.horizontal(|ui| {
                ui.label(theme::strong(format!("{:.0}", g.total), 38.0));
                ui.label(theme::caption("/100"));
                let (r, _) = ui.allocate_exact_size(vec2(84.0, 30.0), Sense::hover());
                theme::stars(ui.painter(), r.center(), 11.0, g.stars);
            });
            grade_metrics(ui, &g);
            ui.add_space(4.0);
            ui.label(RichText::new(&g.advice).size(theme::BODY).color(theme::TEXT));
            if !self.marks.is_empty() {
                ui.checkbox(&mut self.show_marks, "Marcar errores en el lienzo");
            }
            ui.add_space(4.0);
            ui.horizontal(|ui| {
                if theme::button(ui, Some(Icon::Retry), "Repetir", Btn::Outline).on_hover_text("Otra variación del mismo ejercicio").clicked() {
                    action = Some(0);
                }
                if g.stars > 0 && theme::button(ui, Some(Icon::Next), "Siguiente", Btn::Outline).clicked() {
                    action = Some(1);
                }
                if self.from_program && theme::button(ui, Some(Icon::Play), "Volver a Hoy", Btn::Primary).clicked() {
                    action = Some(2);
                }
            });
        });
        let from_program = self.from_program;
        match action {
            Some(0) => self.open_exercise(&ctx, m, e),
            Some(1) => self.next_exercise(&ctx),
            Some(2) => self.screen = Screen::Program,
            _ => {}
        }
        if action.is_some_and(|a| a < 2) {
            self.from_program = from_program;
        }
    }

    /// Panel de capas del lienzo libre.
    fn layers_ui(&mut self, ui: &mut Ui) {
        theme::section(ui, "Capas");
        let n = self.canvas.layers.len();
        for k in (0..n).rev() {
            ui.horizontal(|ui| {
                let mut vis = self.canvas.layers[k].visible;
                if ui.checkbox(&mut vis, "").on_hover_text("Mostrar u ocultar la capa").changed() {
                    self.canvas.layers[k].visible = vis;
                    self.canvas.mark_all();
                }
                let name = self.canvas.layers[k].name.clone();
                let kind = if self.canvas.active == Some(k) { Btn::Selected } else { Btn::Ghost };
                let w = ui.available_width();
                if theme::button_ex(ui, None, &name, kind, w, true).clicked() {
                    self.canvas.active = Some(k);
                }
            });
        }
        ui.horizontal(|ui| {
            ui.add_space(26.0);
            let kind = if self.canvas.active.is_none() { Btn::Selected } else { Btn::Ghost };
            let w = ui.available_width();
            if theme::button_ex(ui, None, "Fondo (papel)", kind, w, true).on_hover_text("La capa de abajo, opaca").clicked() {
                self.canvas.active = None;
            }
        });
        ui.horizontal_wrapped(|ui| {
            if theme::button(ui, None, "+ Capa", Btn::Outline).on_hover_text("Capa nueva encima de la elegida").clicked() {
                self.canvas.add_layer();
            }
            let has = self.canvas.active.is_some();
            ui.add_enabled_ui(has, |ui| {
                if theme::button(ui, None, "Subir", Btn::Ghost).clicked() {
                    self.canvas.move_active_layer(true);
                }
                if theme::button(ui, None, "Bajar", Btn::Ghost).clicked() {
                    self.canvas.move_active_layer(false);
                }
                if theme::button(ui, None, "Eliminar", Btn::Ghost).on_hover_text("Quitar la capa elegida").clicked() {
                    self.canvas.remove_active_layer();
                }
            });
        });
        if let Some(k) = self.canvas.active {
            ui.label(theme::caption("Opacidad de la capa"));
            ui.spacing_mut().slider_width = (ui.available_width() - 66.0).max(60.0);
            if ui.add(egui::Slider::new(&mut self.canvas.layers[k].opacity, 0.0..=1.0).max_decimals(2)).changed() {
                self.canvas.mark_all();
            }
        }
        ui.label(theme::caption("Como en Krita: boceto en una capa con poca opacidad y la línea final en otra encima. Deshacer afecta a la capa elegida."));
    }

    /// Imagen de referencia del lienzo libre.
    fn reference_ui(&mut self, ui: &mut Ui) {
        theme::section(ui, "Referencia");
        match &self.reference {
            Some(tex) => {
                let w = ui.available_width();
                let [tw, th] = tex.size();
                let h = w * th as f32 / tw.max(1) as f32;
                ui.add(egui::Image::new((tex.id(), vec2(w, h))).corner_radius(8));
                if theme::button(ui, None, "Quitar la referencia", Btn::Ghost).clicked() {
                    self.reference = None;
                }
            }
            None => {
                ui.label(theme::caption("Arrastra una imagen (PNG o JPG) a la ventana para tenerla aquí mientras dibujas, como la herramienta de imagen de referencia de Krita."));
            }
        }
    }

    /// Archivos soltados en la ventana: en el lienzo libre se usan como referencia.
    fn handle_drops(&mut self, ctx: &egui::Context) {
        let files = ctx.input(|i| i.raw.dropped_files.clone());
        let Some(f) = files.first() else { return };
        let img = image::open(f.path()).ok().or_else(|| f.bytes().ok().and_then(|b| image::load_from_memory(&b).ok()));
        let now = self.now_ms();
        if self.screen == Screen::Study {
            match img {
                Some(img) => {
                    self.set_study_reference(ctx, img);
                    self.message = Some(("Referencia del estudio cargada.".into(), now));
                }
                None => self.message = Some(("No pude abrir ese archivo como imagen (usa PNG o JPG).".into(), now)),
            }
            return;
        }
        match img {
            Some(img) => {
                // Limitar el tamaño de la textura: la referencia se ve pequeña en el panel.
                let img = img.thumbnail(1600, 1600).to_rgba8();
                self.reference = Some(ctx.load_texture("referencia_libre", color_image(&img), TextureOptions::LINEAR));
                if self.screen != Screen::Free {
                    self.open_free();
                }
                self.message = Some(("Referencia cargada en el lienzo libre.".into(), now));
            }
            None => self.message = Some(("No pude abrir ese archivo como imagen (usa PNG o JPG).".into(), now)),
        }
    }

    /// Lee la carpeta de dibujos guardados, del más nuevo al más antiguo.
    fn load_gallery(&mut self) {
        let mut items: Vec<(std::time::SystemTime, std::path::PathBuf)> = std::fs::read_dir(drawings_dir())
            .map(|rd| {
                rd.filter_map(|e| e.ok())
                    .map(|e| e.path())
                    .filter(|p| p.extension().is_some_and(|x| x.eq_ignore_ascii_case("png")))
                    .map(|p| (std::fs::metadata(&p).and_then(|m| m.modified()).unwrap_or(std::time::UNIX_EPOCH), p))
                    .collect()
            })
            .unwrap_or_default();
        items.sort_by(|a, b| b.0.cmp(&a.0));
        self.gallery = items
            .into_iter()
            .map(|(_, path)| GalleryItem {
                name: path.file_stem().map(|s| s.to_string_lossy().into_owned()).unwrap_or_default(),
                path,
                tex: None,
                failed: false,
            })
            .collect();
    }

    fn gallery_screen(&mut self, ui: &mut Ui) {
        let ctx = ui.ctx().clone();
        if self.gallery.is_empty() {
            self.load_gallery();
        }
        // Miniaturas poco a poco, para no congelar la ventana con muchos dibujos.
        for it in self.gallery.iter_mut().filter(|g| g.tex.is_none() && !g.failed).take(2) {
            match image::open(&it.path) {
                Ok(img) => {
                    let thumb = img.thumbnail(330, 220).to_rgba8();
                    it.tex = Some(ctx.load_texture(it.path.to_string_lossy(), color_image(&thumb), TextureOptions::LINEAR));
                }
                Err(_) => it.failed = true,
            }
        }
        let mut open: Option<std::path::PathBuf> = None;
        egui::CentralPanel::default().frame(egui::Frame::new().fill(theme::BG).inner_margin(24)).show(ui, |ui| {
            egui::ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
                centered(ui, 1180.0, |ui| {
                    ui.label(theme::strong("Galería", theme::TITLE + 4.0));
                    ui.label(theme::body("Tus dibujos guardados desde el lienzo libre, en Imágenes › TrazoAcademia. Ábrelos como capa para seguir dibujando o para calcar encima."));
                    ui.horizontal(|ui| {
                        if theme::button(ui, Some(Icon::Retry), "Actualizar", Btn::Outline).clicked() {
                            self.gallery.clear();
                        }
                        if theme::button(ui, None, "Abrir la carpeta", Btn::Ghost).clicked() {
                            let dir = drawings_dir();
                            let _ = std::fs::create_dir_all(&dir);
                            let _ = std::process::Command::new("explorer").arg(dir).spawn();
                        }
                    });
                    ui.add_space(12.0);
                    if self.gallery.is_empty() {
                        theme::card(theme::RAISED).show(ui, |ui| {
                            ui.set_width(ui.available_width());
                            ui.label(theme::strong("Aún no hay dibujos", theme::LEAD));
                            ui.label(theme::body("Dibuja algo en el lienzo libre y pulsa «Guardar como PNG»: aparecerá aquí."));
                        });
                        return;
                    }
                    ui.horizontal_wrapped(|ui| {
                        ui.spacing_mut().item_spacing = vec2(14.0, 14.0);
                        for it in &self.gallery {
                            theme::card(theme::RAISED).inner_margin(10).show(ui, |ui| {
                                ui.set_width(330.0);
                                match &it.tex {
                                    Some(tex) => {
                                        let [w, h] = tex.size();
                                        ui.add(egui::Image::new((tex.id(), vec2(w as f32, h as f32))).corner_radius(6));
                                    }
                                    None => {
                                        ui.allocate_exact_size(vec2(330.0, 220.0), Sense::hover());
                                    }
                                }
                                ui.label(theme::caption(&it.name));
                                if !it.failed && theme::button(ui, Some(Icon::Brush), "Abrir como capa", Btn::Outline).clicked() {
                                    open = Some(it.path.clone());
                                }
                            });
                        }
                    });
                });
            });
        });
        if let Some(path) = open {
            self.open_in_free(&path);
        }
    }

    /// Abre un dibujo guardado como capa nueva del lienzo libre.
    fn open_in_free(&mut self, path: &std::path::Path) {
        let now = self.now_ms();
        let Ok(img) = image::open(path) else {
            self.message = Some(("No pude abrir ese dibujo.".into(), now));
            return;
        };
        let img = img.resize_exact(W as u32, H as u32, image::imageops::FilterType::Triangle).to_rgba8();
        let px: Vec<[f32; 4]> = img.pixels().map(|p| [0, 1, 2, 3].map(|k| p.0[k] as f32 / 255.0)).collect();
        self.open_free();
        let name = path.file_stem().map(|s| s.to_string_lossy().into_owned()).unwrap_or_else(|| "Dibujo".into());
        self.canvas.add_image_layer(&name, &px);
        self.message = Some((format!("«{name}» abierto como capa nueva. Bájale la opacidad para dibujar encima."), now));
    }

    fn save_png(&mut self) {
        let dir = drawings_dir();
        let _ = std::fs::create_dir_all(&dir);
        let stamp = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0);
        let path = dir.join(format!("dibujo-{stamp}.png"));
        let ok = image::save_buffer(&path, &self.canvas.to_rgba(), W as u32, H as u32, image::ColorType::Rgba8).is_ok();
        let msg = if ok { format!("Guardado en {}", path.display()) } else { "No se pudo guardar la imagen.".into() };
        self.gallery.clear();
        self.message = Some((msg, self.now_ms()));
    }

    fn smoothing_ui(&mut self, ui: &mut Ui, allow_magnet: bool) {
        let o = &mut self.smoother.opts;
        let before = o.kind;
        egui::ComboBox::from_id_salt("suavizado")
            .selected_text(o.kind.label())
            .width(180.0)
            .show_ui(ui, |ui| {
                for k in SmoothingType::ALL {
                    ui.selectable_value(&mut o.kind, k, k.label());
                }
            });
        if before != o.kind && self.drawing {
            o.kind = before;
        }
        ui.label(RichText::new(o.kind.help()).small().color(Color32::from_gray(180)));
        match o.kind {
            SmoothingType::Ponderado => {
                ui.add(egui::Slider::new(&mut o.distance_max, 3.0..=500.0).logarithmic(true).text("Distancia"));
                o.distance_min = o.distance_max;
                ui.add(egui::Slider::new(&mut o.tail_aggressiveness, 0.0..=1.0).text("Agresividad de cola"));
                ui.checkbox(&mut o.smooth_pressure, "Suavizar presión");
            }
            SmoothingType::Estabilizador => {
                ui.add(egui::Slider::new(&mut o.distance_max, 3.0..=500.0).logarithmic(true).text("Distancia"));
                o.distance_min = o.distance_max;
                ui.checkbox(&mut o.use_delay, "Retardo");
                if o.use_delay {
                    ui.add(egui::Slider::new(&mut o.delay_distance, 0.0..=200.0).text("Distancia de retardo"));
                }
                ui.checkbox(&mut o.finish_stabilized, "Terminar línea");
                ui.checkbox(&mut o.stabilize_sensors, "Estabilizar sensores");
            }
            _ => {}
        }
        if ui.small_button("Valores por defecto de Krita").clicked() {
            let kind = o.kind;
            *o = crate::smoothing::SmoothingOptions { kind, magnet: o.magnet, ..Default::default() };
        }
        if allow_magnet {
            ui.add_space(4.0);
            ui.add(egui::Slider::new(&mut o.magnet, 0.0..=1.0).text("Guía magnética"))
                .on_hover_text("Ayuda para principiantes: atrae el trazo hacia la guía. Resta hasta un 25 % de la nota.");
        }
    }

    fn pressure_meter(&mut self, ui: &mut Ui) {
        let tablet = self.tablet_active();
        let src = if tablet {
            "Tableta detectada: presión real".to_string()
        } else {
            match self.progress.pressure.mouse_mode {
                MouseMode::PorVelocidad => "Ratón: presión simulada por velocidad (lento = más)".into(),
                MouseMode::Constante => format!("Ratón: presión fija {:.0} %", self.progress.pressure.mouse_constant * 100.0),
            }
        };
        ui.label(RichText::new(src).small().color(if tablet { Color32::LIGHT_GREEN } else { Color32::from_gray(180) }));
        bar(ui, "Entrada", self.raw_pressure, Color32::from_gray(140));
        bar(ui, "Tras la curva", self.curved_pressure, ACCENT);
    }

    fn pressure_screen(&mut self, ui: &mut Ui) {
        let now = self.now_ms();
        egui::CentralPanel::default().frame(egui::Frame::new().fill(theme::BG).inner_margin(18)).show(ui, |ui| {
            egui::ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
                ui.heading(RichText::new("Presión adaptada").size(24.0).color(Color32::WHITE));
                ui.label("Krita lee la presión del lápiz y la pasa por una curva global (Configuración › Configurar Krita › Ajustes de tableta). Aquí haces lo mismo, y además puedes calibrarla a tu mano.");
                ui.add_space(12.0);
                ui.horizontal_top(|ui| {
                    ui.vertical(|ui| {
                        ui.set_width(320.0);
                        theme::section(ui, "Curva de presión");
                        let changed = curve_editor(ui, &mut self.progress.pressure.curve, &mut self.curve_drag);
                        ui.label(RichText::new("Arrastra los puntos. Clic en vacío añade uno; clic derecho sobre un punto lo quita.").small().color(Color32::from_gray(170)));
                        ui.label(RichText::new(format!("Formato Krita: {}", self.progress.pressure.curve.to_krita_string())).small().monospace());
                        ui.horizontal_wrapped(|ui| {
                            let presets: [(&str, &str); 4] = [
                                ("Lineal (Krita)", "0,0;1,1;"),
                                ("Mano ligera", "0,0;0.3,0.55;1,1;"),
                                ("Mano firme", "0,0;0.6,0.3;1,1;"),
                                ("Sin extremos", "0,0.1;1,0.9;"),
                            ];
                            for (name, c) in presets {
                                if ui.button(name).clicked() {
                                    self.progress.pressure.curve = Curve::parse(c);
                                    self.save();
                                }
                            }
                        });
                        if changed {
                            self.save();
                        }
                        ui.add_space(10.0);
                        theme::section(ui, "Sin tableta (ratón)");
                        let pm = &mut self.progress.pressure;
                        ui.radio_value(&mut pm.mouse_mode, MouseMode::PorVelocidad, "Simular con la velocidad (lento = más presión)");
                        ui.radio_value(&mut pm.mouse_mode, MouseMode::Constante, "Presión fija");
                        if pm.mouse_mode == MouseMode::Constante {
                            ui.add(egui::Slider::new(&mut pm.mouse_constant, 0.05..=1.0).text("Presión"));
                        }
                    });
                    ui.add_space(20.0);
                    ui.vertical(|ui| {
                        theme::section(ui, "Calibración personal");
                        let steps = ["suave (como si acariciaras)", "normal (tu trazo habitual)", "fuerte (presionando con ganas)"];
                        if self.calib.stage < 3 {
                            ui.label(RichText::new(format!("Paso {} de 3: haz un trazo {} en el recuadro.", self.calib.stage + 1, steps[self.calib.stage])).size(15.0).color(GOLD));
                        } else {
                            ui.label(RichText::new("¡Calibrado! Tu curva se ajustó a tu rango natural de presión.").color(Color32::LIGHT_GREEN));
                        }
                        let (resp, painter) = ui.allocate_painter(vec2(560.0, 300.0), Sense::drag());
                        let r = resp.rect;
                        painter.rect_filled(r, 8, Color32::from_rgb(248, 247, 242));
                        let force = ui.ctx().input(|i| i.events.iter().find_map(|e| if let Event::Touch { force: Some(f), .. } = e { Some(*f) } else { None }));
                        if resp.drag_started() {
                            self.speed.reset();
                            self.calib.pad_strokes.push(Vec::new());
                        }
                        if resp.dragged() {
                            if let Some(p) = resp.interact_pointer_pos() {
                                let sp = self.speed.next(p, now);
                                let pr = self.pressure_from(force, sp, now);
                                if let Some(s) = self.calib.pad_strokes.last_mut() {
                                    s.push((p, pr));
                                }
                                if self.calib.stage < 3 {
                                    self.calib.samples[self.calib.stage].push(self.raw_pressure);
                                }
                            }
                        }
                        if resp.drag_stopped() && self.calib.stage < 3 && self.calib.samples[self.calib.stage].len() > 5 {
                            self.calib.stage += 1;
                            if self.calib.stage == 3 {
                                let mean = |v: &Vec<f32>| v.iter().sum::<f32>() / v.len().max(1) as f32;
                                let c = [mean(&self.calib.samples[0]), mean(&self.calib.samples[1]), mean(&self.calib.samples[2])];
                                self.progress.pressure.calibration = Some(c);
                                self.progress.pressure.curve = PressureSettings::curve_from_calibration(c);
                                self.save();
                            }
                        }
                        for s in &self.calib.pad_strokes {
                            for w in s.windows(2) {
                                let width = 1.0 + w[1].1 * 14.0;
                                painter.line_segment([w[0].0, w[1].0], Stroke::new(width, Color32::from_rgb(30, 30, 40)));
                            }
                        }
                        ui.horizontal(|ui| {
                            if ui.button("Empezar calibración de nuevo").clicked() {
                                self.calib = Calibration { stage: 0, samples: [Vec::new(), Vec::new(), Vec::new()], pad_strokes: Vec::new() };
                            }
                            if ui.button("Limpiar recuadro").clicked() {
                                self.calib.pad_strokes.clear();
                            }
                        });
                        if let Some(c) = self.progress.pressure.calibration {
                            ui.label(RichText::new(format!("Tu presión media: suave {:.0} %, normal {:.0} %, fuerte {:.0} %", c[0] * 100.0, c[1] * 100.0, c[2] * 100.0)).small());
                        }
                        theme::section(ui, "Presión ahora");
                        self.pressure_meter(ui);
                        ui.label(RichText::new("Si tienes tableta y el indicador no cambia, revisa que el controlador de la tableta esté instalado (Windows Ink activado).").small().color(Color32::from_gray(160)));
                    });
                });
            });
        });
    }

    fn guide_screen(&mut self, ui: &mut Ui) {
        egui::CentralPanel::default().frame(egui::Frame::new().fill(theme::BG).inner_margin(18)).show(ui, |ui| {
            egui::ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
                ui.set_max_width(980.0);
                ui.heading(RichText::new("Guía rápida de Krita").size(24.0).color(Color32::WHITE));
                ui.label("Lo que aprendes en Trazo Academia funciona igual en Krita. Esta guía te dice dónde está cada cosa.");
                ui.add_space(10.0);
                for (title, body) in KRITA_GUIDE {
                    egui::Frame::new().fill(theme::RAISED).corner_radius(8).inner_margin(12).show(ui, |ui| {
                        ui.set_width(ui.available_width());
                        ui.label(RichText::new(*title).size(17.0).strong().color(ACCENT));
                        ui.label(RichText::new(*body).color(Color32::from_gray(215)));
                    });
                    ui.add_space(8.0);
                }
                egui::Frame::new().fill(theme::RAISED).corner_radius(8).inner_margin(12).show(ui, |ui| {
                    ui.set_width(ui.available_width());
                    ui.label(RichText::new("Pinceles de esta app y su equivalente en Krita").size(17.0).strong().color(ACCENT));
                    egui::Grid::new("pinceles").striped(true).num_columns(3).spacing([16.0, 6.0]).show(ui, |ui| {
                        for b in &self.brushes {
                            ui.label(RichText::new(b.name).strong());
                            ui.label(RichText::new(b.krita_name).monospace().color(Color32::from_gray(180)));
                            ui.label(b.description);
                            ui.end_row();
                        }
                    });
                });
            });
        });
    }
}

const KRITA_GUIDE: &[(&str, &str)] = &[
    ("La pantalla de Krita", "En el centro está el lienzo. A la izquierda, la Caja de herramientas. A la derecha, los acoplables: Selector de color, Preajustes de pincel, Capas y Opciones de herramienta. Arriba, la barra con el pincel actual, su tamaño, opacidad y modo de mezcla."),
    ("Pincel a mano alzada (B)", "Es la herramienta principal para dibujar y pintar. Tamaño con [ y ], o manteniendo Mayús y arrastrando sobre el lienzo. Ctrl+Z deshace. La tecla E alterna el modo borrador del pincel actual."),
    ("Suavizado del pincel", "En Opciones de herramienta del Pincel a mano alzada. Modos: Ninguno, Básico (por defecto), Ponderado, Estabilizador y Pixel. Krita trae Distancia 50, Agresividad de cola 0.15, Retardo activado con 50 píxeles, Terminar línea y Estabilizar sensores activados, y Suavizar presión desactivado. Son los mismos valores que usa esta app."),
    ("Curva de presión global", "Configuración › Configurar Krita › Ajustes de tableta. Por defecto es una línea recta (0,0 a 1,1). Si llegas al máximo con poco esfuerzo, baja la curva; si te cuesta llegar, súbela. La calibración de esta app te da una curva de partida."),
    ("Editor de pinceles (F5)", "Cada opción del pincel (Tamaño, Opacidad, Flujo…) puede depender de sensores: Presión, Velocidad, Inclinación, Rotación… y cada sensor tiene su curva. Por ejemplo, la tinta d) Ink-2 Fineliner usa Tamaño = Presión (de 0.75 a 1) multiplicado por Velocidad (de 1 a 0)."),
    ("Capas", "Trabaja siempre en capas: boceto, tinta y color separados. Baja la opacidad del boceto para entintar encima. Bloquear alfa evita pintar fuera de lo ya pintado. El lienzo libre de esta app también tiene capas con opacidad, para practicar el mismo flujo."),
    ("Asistentes de dibujo", "La herramienta de Asistentes coloca reglas, elipses y puntos de fuga que atraen tu trazo. Son la versión profesional de la guía magnética de esta app."),
    ("Mover, girar y voltear la vista", "Espacio + arrastrar mueve la vista, Mayús + espacio + arrastrar la gira y la tecla M la voltea en espejo para detectar errores de proporción. Aquí funcionan igual, en los ejercicios y en el lienzo libre."),
];

// ------------------------------------------------------------------ dibujo auxiliar

/// Columna centrada de ancho máximo `max_w`.
fn centered<R>(ui: &mut Ui, max_w: f32, add: impl FnOnce(&mut Ui) -> R) -> R {
    let w = ui.available_width().min(max_w);
    let pad = ((ui.available_width() - w) / 2.0).max(0.0);
    ui.horizontal_top(|ui| {
        ui.add_space(pad);
        ui.vertical(|ui| {
            ui.set_width(w);
            add(ui)
        })
        .inner
    })
    .inner
}

fn mix(a: Color32, b: Color32, t: f32) -> Color32 {
    let l = |x: u8, y: u8| (x as f32 + (y as f32 - x as f32) * t).round() as u8;
    Color32::from_rgb(l(a.r(), b.r()), l(a.g(), b.g()), l(a.b(), b.b()))
}

/// Círculo de color con el número del módulo.
fn badge(p: &egui::Painter, c: Pos2, n: usize, color: Color32) {
    p.circle_filled(c, 15.0, color);
    p.text(c, Align2::CENTER_CENTER, n.to_string(), FontId::new(15.0, theme::semibold()), theme::ON_ACCENT);
}

fn stat_card(ui: &mut Ui, icon: Icon, color: Color32, value: &str, label: &str, frac: f32) {
    theme::card(theme::RAISED).show(ui, |ui| {
        ui.set_width(ui.available_width());
        ui.horizontal(|ui| {
            theme::icon_widget(ui, icon, 22.0, color);
            ui.label(theme::strong(value, theme::DISPLAY));
        });
        ui.label(theme::caption(label));
        let w = ui.available_width();
        theme::progress(ui, frac, color, w, 6.0);
    });
}

fn action_card(ui: &mut Ui, icon: Icon, title: &str, text: &str) -> egui::Response {
    let w = ui.available_width();
    let (rect, resp) = ui.allocate_exact_size(vec2(w, 98.0), Sense::click());
    let p = ui.painter();
    p.rect(rect, 12, if resp.hovered() { theme::HOVER } else { theme::RAISED }, Stroke::new(1.0, theme::BORDER), StrokeKind::Inside);
    theme::paint_icon(p, Rect::from_min_size(rect.min + vec2(16.0, 15.0), vec2(22.0, 22.0)), icon, ACCENT);
    p.text(rect.min + vec2(48.0, 26.0), Align2::LEFT_CENTER, title, FontId::new(theme::LEAD, theme::semibold()), theme::TEXT);
    let g = theme::layout_wrapped(p, text, FontId::proportional(theme::CAPTION + 1.0), theme::TEXT_2, w - 32.0, 2);
    p.galley(rect.min + vec2(16.0, 48.0), g, theme::TEXT_2);
    if resp.hovered() {
        ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
    }
    resp
}

/// Mosaico de un ejercicio en el mapa: tipo, número, título completo y estrellas.
fn exercise_tile(ui: &mut Ui, n: usize, ex: &Exercise, stars: u8, unlocked: bool, accent: Color32, is_next: bool) -> egui::Response {
    let (rect, resp) = ui.allocate_exact_size(vec2(176.0, 100.0), Sense::click());
    let hovered = resp.hovered() && unlocked;
    let p = ui.painter();
    let stroke = if is_next && unlocked { Stroke::new(2.0, accent) } else { Stroke::new(1.0, theme::BORDER) };
    p.rect(rect, 10, if hovered { theme::HOVER } else { theme::SURFACE }, stroke, StrokeKind::Inside);
    let (icon, kind) = kind_info(&ex.kind);
    let fg = if unlocked { theme::TEXT } else { theme::TEXT_3 };
    let col = if unlocked { accent } else { theme::TEXT_3 };
    theme::paint_icon(p, Rect::from_min_size(rect.min + vec2(12.0, 11.0), vec2(16.0, 16.0)), icon, col);
    p.text(rect.min + vec2(34.0, 19.0), Align2::LEFT_CENTER, n.to_string(), FontId::new(theme::CAPTION, theme::semibold()), col);
    if !unlocked {
        theme::paint_icon(p, Rect::from_min_size(pos2(rect.max.x - 26.0, rect.min.y + 12.0), vec2(14.0, 14.0)), Icon::Lock, theme::TEXT_3);
    } else if is_next {
        p.text(pos2(rect.max.x - 12.0, rect.min.y + 19.0), Align2::RIGHT_CENTER, "Siguiente", FontId::new(12.0, theme::semibold()), accent);
    }
    let g = theme::layout_wrapped(p, ex.title, FontId::proportional(14.0), fg, rect.width() - 24.0, 2);
    p.galley(rect.min + vec2(12.0, 32.0), g, fg);
    theme::stars(p, pos2(rect.min.x + 33.0, rect.max.y - 15.0), 6.0, stars);
    if hovered {
        ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
    }
    resp.on_hover_text(format!("{kind}: {}", ex.goal))
}

fn krita_box(ui: &mut Ui, text: &str) {
    egui::Frame::new()
        .fill(Color32::from_rgb(27, 43, 55))
        .stroke(Stroke::new(1.0, ACCENT.gamma_multiply(0.45)))
        .corner_radius(10)
        .inner_margin(12)
        .show(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.label(RichText::new("EN KRITA").family(theme::semibold()).size(12.0).color(ACCENT));
            ui.label(RichText::new(text).size(theme::BODY - 1.0).color(theme::TEXT));
        });
}

fn grade_metrics(ui: &mut Ui, g: &Grade) {
    for m in &g.metrics {
        let c = if m.value >= 0.85 { theme::GOOD } else if m.value >= 0.6 { GOLD } else { theme::BAD };
        ui.horizontal(|ui| {
            // Columna de nombres de ancho fijo para que todas las barras empiecen a la vez.
            let (r, _) = ui.allocate_exact_size(vec2(100.0, 20.0), Sense::hover());
            ui.painter().text(r.left_center(), Align2::LEFT_CENTER, m.name, FontId::proportional(theme::CAPTION), theme::TEXT_2);
            let w = (ui.available_width() - 36.0).max(40.0);
            let (r, resp) = ui.allocate_exact_size(vec2(w, 8.0), Sense::hover());
            theme::paint_progress(ui.painter(), r, m.value, c);
            resp.on_hover_text(m.help);
            ui.label(RichText::new(format!("{:.0}", m.value * 100.0)).size(theme::CAPTION).color(theme::TEXT));
        });
    }
}

/// Carpeta de los dibujos guardados (Imágenes › TrazoAcademia).
fn drawings_dir() -> std::path::PathBuf {
    let base = std::env::var_os("USERPROFILE").map(std::path::PathBuf::from).unwrap_or_default();
    base.join("Pictures").join("TrazoAcademia")
}

fn color_image(img: &image::RgbaImage) -> ColorImage {
    ColorImage::from_rgba_unmultiplied([img.width() as usize, img.height() as usize], img.as_raw())
}

fn bar(ui: &mut Ui, label: &str, v: f32, color: Color32) {
    ui.horizontal(|ui| {
        let (r, _) = ui.allocate_exact_size(vec2(90.0, 18.0), Sense::hover());
        ui.painter().text(r.left_center(), Align2::LEFT_CENTER, label, FontId::proportional(theme::CAPTION), theme::TEXT_2);
        let (r, _) = ui.allocate_exact_size(vec2((ui.available_width() - 44.0).max(40.0), 8.0), Sense::hover());
        theme::paint_progress(ui.painter(), r, v, color);
        ui.label(RichText::new(format!("{:.0} %", v * 100.0)).size(theme::CAPTION).color(theme::TEXT));
    });
}

fn logo(ui: &mut Ui) {
    let (r, _) = ui.allocate_exact_size(vec2(30.0, 30.0), Sense::hover());
    let p = ui.painter();
    p.circle_filled(r.center(), 14.0, ACCENT);
    let pts: Vec<Pos2> = (0..=20)
        .map(|i| {
            let t = i as f32 / 20.0;
            pos2(r.min.x + 7.0 + t * 16.0, r.center().y + 5.0 * (t * 6.28).sin())
        })
        .collect();
    p.add(Shape::line(pts, Stroke::new(3.0, Color32::WHITE)));
}

/// Guías de calcar. Las de contexto se ven siempre, como líneas ya dibujadas; las de repasar,
/// si `show`; las ocultas, solo al revisar la nota (`reveal`).
fn draw_guides(p: &egui::Painter, guides: &[Guide], xf: &Xf, brush_size: f32, show: bool, reveal: bool) {
    let to = |q: Pos2| xf.to_screen(q);
    let scale = xf.scale;
    let guide_col = Color32::from_rgba_unmultiplied(70, 150, 230, 120);
    for g in guides {
        match g.role {
            Role::Context => {
                p.add(Shape::line(g.pts.iter().map(|q| to(*q)).collect(), Stroke::new(2.2, INK)));
                continue;
            }
            Role::Hidden if !reveal => continue,
            Role::Hidden => {
                let pts: Vec<Pos2> = g.pts.iter().map(|q| to(*q)).collect();
                p.add(Shape::dashed_line(&pts, Stroke::new(2.0, Color32::from_rgba_unmultiplied(110, 200, 120, 170)), 8.0, 5.0));
                continue;
            }
            Role::Trace if !show => continue,
            Role::Trace => {}
        }
        if let Some(pr) = &g.pressure {
            // Cinta de grosor variable: muestra la presión esperada.
            let maxw = (brush_size * scale).max(8.0);
            for (i, w) in g.pts.windows(2).enumerate() {
                let width = 1.5 + pr[i] * maxw;
                p.line_segment([to(w[0]), to(w[1])], Stroke::new(width, Color32::from_rgba_unmultiplied(255, 170, 70, 70)));
            }
        }
        let pts: Vec<Pos2> = g.pts.iter().map(|q| to(*q)).collect();
        p.add(Shape::dashed_line(&pts, Stroke::new(2.0, guide_col), 8.0, 5.0));
        if let (Some(a), Some(b)) = (pts.first(), pts.get(4.min(pts.len().saturating_sub(1)))) {
            p.circle_filled(*a, 5.0, Color32::from_rgb(90, 200, 110));
            let d = *b - *a;
            if d.length() > 0.5 {
                p.arrow(*a, d.normalized() * 18.0, Stroke::new(2.0, Color32::from_rgb(90, 200, 110)));
            }
        }
    }
}

const START_GREEN: Color32 = Color32::from_rgb(90, 200, 110);
const INK: Color32 = Color32::from_rgb(45, 45, 52);
const VP_RED: Color32 = Color32::from_rgb(220, 80, 70);

/// Punto a punto: cada par de extremos con su número.
fn draw_points(p: &egui::Painter, guides: &[Guide], xf: &Xf) {
    let to = |q: Pos2| xf.to_screen(q);
    for (i, g) in guides.iter().enumerate() {
        let (Some(a), Some(b)) = (g.pts.first(), g.pts.last()) else { continue };
        for q in [to(*a), to(*b)] {
            p.circle_filled(q, 6.0, START_GREEN);
            p.circle_stroke(q, 6.0, Stroke::new(1.5, Color32::from_rgb(30, 90, 45)));
            p.text(q + vec2(9.0, -9.0), Align2::LEFT_BOTTOM, (i + 1).to_string(), FontId::proportional(14.0), INK);
        }
    }
}

/// Escena en perspectiva: horizonte, puntos de fuga, líneas dadas, puntos de salida y ayuda.
fn draw_scene(p: &egui::Painter, s: &Scene, xf: &Xf, aids: bool, hints: bool) {
    let to = |q: Pos2| xf.to_screen(q);
    let (w, h) = (W as f32, H as f32);
    if aids {
        if let Some(hy) = s.horizon {
            if (0.0..=h).contains(&hy) {
                p.line_segment([to(pos2(0.0, hy)), to(pos2(w, hy))], Stroke::new(1.5, Color32::from_rgb(120, 140, 170)));
                // A un cuarto del ancho: los puntos de fuga suelen estar en los extremos o en el centro.
                p.text(to(pos2(w * 0.25, hy)) + vec2(0.0, -3.0), Align2::CENTER_BOTTOM, "Horizonte", FontId::proportional(12.0), Color32::from_rgb(90, 110, 140));
            }
        }
        // Margen de 16 px de pantalla, en unidades del lienzo.
        let m = 16.0 / xf.scale;
        let inner = Rect::from_min_max(pos2(m, m), pos2(w - m, h - m));
        for (i, vp) in s.vps.iter().enumerate() {
            let label = format!("PF{}", i + 1);
            let q = to(*vp);
            if (0.0..=w).contains(&vp.x) && (0.0..=h).contains(&vp.y) {
                p.circle_stroke(q, 7.0, Stroke::new(2.0, VP_RED));
                p.line_segment([q - vec2(10.0, 0.0), q + vec2(10.0, 0.0)], Stroke::new(1.5, VP_RED));
                p.line_segment([q - vec2(0.0, 10.0), q + vec2(0.0, 10.0)], Stroke::new(1.5, VP_RED));
                p.text(q + vec2(10.0, 8.0), Align2::LEFT_TOP, label, FontId::proportional(13.0), VP_RED);
            } else {
                // Fuera del lienzo: flecha en el borde apuntando hacia él.
                let c = to(inner.clamp(*vp));
                let d = (q - c).normalized() * 14.0;
                p.arrow(c - d, d, Stroke::new(2.5, VP_RED));
                let (anchor, off) = if d.y < 0.0 { (Align2::CENTER_TOP, vec2(0.0, 20.0)) } else { (Align2::CENTER_BOTTOM, vec2(0.0, -20.0)) };
                p.text(c + off, anchor, format!("{label} (fuera)"), FontId::proportional(12.0), VP_RED);
            }
        }
    }
    for g in &s.given {
        let pts: Vec<Pos2> = g.pts.iter().map(|q| to(*q)).collect();
        p.add(Shape::line(pts, Stroke::new(2.5, INK)));
    }
    if hints {
        let col = Color32::from_rgba_unmultiplied(70, 150, 230, 120);
        for t in &s.tasks {
            let (a, b) = s.target(t);
            p.add(Shape::dashed_line(&[to(a), to(b)], Stroke::new(2.0, col), 8.0, 5.0));
        }
    }
    for t in &s.tasks {
        let q = to(t.from);
        p.circle_filled(q, 5.0, START_GREEN);
        p.circle_stroke(q, 5.0, Stroke::new(1.2, Color32::from_rgb(30, 90, 45)));
    }
}

/// Número de cada guía junto a su inicio: el orden en que se construye la figura.
fn draw_order(p: &egui::Painter, guides: &[Guide], xf: &Xf) {
    let mut placed: Vec<Pos2> = Vec::new();
    for (i, g) in guides.iter().filter(|g| g.role == Role::Trace).enumerate() {
        let Some(a) = g.pts.first() else { continue };
        let mut c = xf.to_screen(*a) + vec2(-13.0, -13.0);
        // Si varias guías empiezan en el mismo punto, apila los números para que se vean todos.
        while placed.iter().any(|q| (*q - c).length() < 18.0) {
            c.x -= 20.0;
        }
        placed.push(c);
        p.circle_filled(c, 9.0, theme::ACCENT);
        p.text(c, Align2::CENTER_CENTER, (i + 1).to_string(), FontId::new(12.0, theme::semibold()), theme::ON_ACCENT);
    }
}

/// Zonas de rayado: contorno y, al lado, una muestra con la dirección y la densidad pedidas.
fn draw_hatch(p: &egui::Painter, regions: &[Region], xf: &Xf) {
    let to = |q: Pos2| xf.to_screen(q);
    let col = Color32::from_rgba_unmultiplied(70, 150, 230, 150);
    for r in regions {
        let mut pts: Vec<Pos2> = r.pts.iter().map(|q| to(*q)).collect();
        pts.push(pts[0]);
        p.add(Shape::dashed_line(&pts, Stroke::new(2.0, col), 8.0, 5.0));
        // Muestra: arriba, a la izquierda, a la derecha o debajo de la zona, en el primer sitio que
        // quepa en el lienzo sin pisar ninguna zona; si no hay ninguno libre, arriba.
        let bbox = |pts: &[Pos2]| {
            let (mut lo, mut hi) = (Pos2::new(f32::MAX, f32::MAX), Pos2::new(f32::MIN, f32::MIN));
            for q in pts {
                lo = lo.min(*q);
                hi = hi.max(*q);
            }
            Rect::from_min_max(lo, hi)
        };
        let b = bbox(&r.pts);
        let side = 40.0;
        // La caja de la muestra mide `side` más 14 px de rótulo debajo.
        let candidates = [
            pos2(b.min.x, b.min.y - side - 22.0),
            pos2(b.min.x - side - 8.0, b.min.y),
            pos2(b.max.x + 8.0, b.min.y),
            pos2(b.min.x, b.max.y + 8.0),
        ];
        let canvas = Rect::from_min_max(pos2(4.0, 4.0), pos2(W as f32 - 4.0, H as f32 - 4.0));
        let origin = candidates
            .iter()
            .copied()
            .find(|o| {
                let s = Rect::from_min_size(*o, vec2(side, side + 14.0));
                canvas.contains_rect(s) && regions.iter().all(|other| !bbox(&other.pts).shrink(2.0).intersects(s))
            })
            .unwrap_or(candidates[0]);
        let sample = Region {
            pts: vec![origin, origin + vec2(side, 0.0), origin + vec2(side, side), origin + vec2(0.0, side)],
            angle: Some(r.angle.unwrap_or(45.0)),
            spacing: r.spacing,
            cross: r.cross,
            label: None,
        };
        let corners = [origin, origin + vec2(side, 0.0), origin + vec2(side, side), origin + vec2(0.0, side)].map(to);
        p.add(Shape::convex_polygon(corners.to_vec(), theme::PAPER, Stroke::new(1.0, col)));
        for (a, b) in sample.ideal_lines() {
            p.line_segment([to(a), to(b)], Stroke::new(1.4, Color32::from_rgb(60, 60, 70)));
        }
        if r.angle.is_none() {
            p.text(to(origin + vec2(side / 2.0, side)) + vec2(0.0, 2.0), Align2::CENTER_TOP, "dirección libre", FontId::proportional(11.0), Color32::from_rgb(90, 110, 140));
        }
        if let Some(label) = &r.label {
            p.text(to(r.centroid()), Align2::CENTER_CENTER, label, FontId::new(14.0, theme::semibold()), col);
        }
    }
}

/// Miniatura de la referencia para los ejercicios de copia.
fn draw_reference(ui: &mut Ui, guides: &[Guide]) {
    let w = ui.available_width();
    let (r, _) = ui.allocate_exact_size(vec2(w, w * H as f32 / W as f32), Sense::hover());
    let p = ui.painter_at(r);
    p.rect_filled(r, 8, theme::PAPER);
    let s = w / W as f32;
    for g in guides {
        let pts: Vec<Pos2> = g.pts.iter().map(|q| r.min + q.to_vec2() * s).collect();
        p.add(Shape::line(pts, Stroke::new(1.8, INK)));
    }
}

fn curve_editor(ui: &mut Ui, curve: &mut Curve, drag: &mut Option<usize>) -> bool {
    let (resp, painter) = ui.allocate_painter(vec2(300.0, 300.0), Sense::click_and_drag());
    let r = resp.rect.shrink(10.0);
    painter.rect_filled(resp.rect, 6, Color32::from_rgb(30, 33, 36));
    for i in 0..=4 {
        let t = i as f32 / 4.0;
        let x = r.min.x + t * r.width();
        let y = r.min.y + t * r.height();
        painter.line_segment([pos2(x, r.min.y), pos2(x, r.max.y)], Stroke::new(1.0, Color32::from_gray(50)));
        painter.line_segment([pos2(r.min.x, y), pos2(r.max.x, y)], Stroke::new(1.0, Color32::from_gray(50)));
    }
    let to = |x: f32, y: f32| pos2(r.min.x + x * r.width(), r.max.y - y * r.height());
    let from = |p: Pos2| [((p.x - r.min.x) / r.width()).clamp(0.0, 1.0), ((r.max.y - p.y) / r.height()).clamp(0.0, 1.0)];
    let mut changed = false;
    if let Some(p) = resp.interact_pointer_pos() {
        let nearest = curve
            .points
            .iter()
            .enumerate()
            .map(|(i, q)| (i, (to(q[0], q[1]) - p).length()))
            .min_by(|a, b| a.1.total_cmp(&b.1));
        if resp.drag_started() {
            *drag = nearest.filter(|n| n.1 < 12.0).map(|n| n.0);
        }
        if resp.dragged() {
            if let Some(i) = *drag {
                let mut v = from(p);
                let n = curve.points.len();
                if i == 0 {
                    v[0] = 0.0;
                } else if i == n - 1 {
                    v[0] = 1.0;
                } else {
                    v[0] = v[0].clamp(curve.points[i - 1][0] + 0.01, curve.points[i + 1][0] - 0.01);
                }
                curve.points[i] = v;
                curve.rebuild();
                changed = true;
            }
        }
        if resp.clicked() && nearest.is_none_or(|n| n.1 >= 12.0) {
            curve.points.push(from(p));
            curve.rebuild();
            changed = true;
        }
        if resp.secondary_clicked() {
            if let Some((i, d)) = nearest {
                if d < 12.0 && i != 0 && i != curve.points.len() - 1 {
                    curve.points.remove(i);
                    curve.rebuild();
                    changed = true;
                }
            }
        }
    }
    if resp.drag_stopped() {
        *drag = None;
    }
    let line: Vec<Pos2> = (0..=100).map(|i| {
        let x = i as f32 / 100.0;
        to(x, curve.value(x))
    }).collect();
    painter.add(Shape::line(line, Stroke::new(2.5, ACCENT)));
    for q in &curve.points {
        painter.circle_filled(to(q[0], q[1]), 6.0, Color32::WHITE);
        painter.circle_stroke(to(q[0], q[1]), 6.0, Stroke::new(1.5, ACCENT));
    }
    painter.text(pos2(r.min.x, r.max.y + 2.0), Align2::LEFT_TOP, "entrada ›", FontId::proportional(10.0), Color32::GRAY);
    changed
}

fn to_color_image(img: &[[f32; 3]]) -> ColorImage {
    let px = img.iter().map(|c| Color32::from_rgb(to8(c[0]), to8(c[1]), to8(c[2]))).collect();
    ColorImage::new([W, H], px)
}

fn outline_image(mask: &[bool]) -> ColorImage {
    let mut px = vec![Color32::TRANSPARENT; W * H];
    for y in 1..H - 1 {
        for x in 1..W - 1 {
            let i = y * W + x;
            if mask[i] && (!mask[i - 1] || !mask[i + 1] || !mask[i - W] || !mask[i + W]) {
                let c = Color32::from_rgba_unmultiplied(70, 150, 230, 200);
                for (dx, dy) in [(0i32, 0i32), (1, 0), (0, 1)] {
                    let j = ((y as i32 + dy) as usize) * W + (x as i32 + dx) as usize;
                    px[j] = c;
                }
            }
        }
    }
    let _ = PAPER;
    ColorImage::new([W, H], px)
}

impl eframe::App for TrazoApp {
    fn ui(&mut self, ui: &mut Ui, _frame: &mut eframe::Frame) {
        let ctx = ui.ctx().clone();
        self.frame += 1;
        self.demo_step(&ctx);
        self.check_timer();
        self.session_step(&ctx);
        self.study_step(&ctx);
        self.handle_drops(&ctx);
        self.shortcuts(&ctx);
        self.top_bar(ui);
        match self.screen {
            Screen::Home => self.home_screen(ui),
            Screen::Program => self.program_screen(ui),
            Screen::Map => self.map_screen(ui),
            Screen::Gallery => self.gallery_screen(ui),
            Screen::Exercise(..) | Screen::Free => self.exercise_or_free_screen(ui),
            Screen::Study => self.study_screen(ui),
            Screen::Diary => self.diary_screen(ui),
            Screen::Pressure => self.pressure_screen(ui),
            Screen::Guide => self.guide_screen(ui),
        }
        let busy = self.drawing
            || self.session.is_some()
            || self.gallery.iter().any(|g| g.tex.is_none() && !g.failed) && self.screen == Screen::Gallery
            || self.current_exercise().is_some_and(|e| e.memory.is_some())
            || (self.time_limit().is_some() && !self.time_up)
            || self.screen == Screen::Study && (self.study.timer.is_some() || self.study.blink || self.message.is_some())
            || self.screen == Screen::Diary && self.diary.iter().any(|d| d.draw_tex.is_none() && !d.failed);
        if let Some(shot) = &mut self.screenshot {
            ctx.request_repaint();
            if self.frame >= 120 && !shot.requested {
                shot.requested = true;
                let path = shot.path.clone();
                let done = shot.done.clone();
                ctx.request_screenshot(move |img| {
                    let mut rgba = Vec::with_capacity(img.pixels.len() * 4);
                    for p in &img.pixels {
                        rgba.extend_from_slice(&p.to_srgba_unmultiplied());
                    }
                    if let Some(dir) = path.parent() {
                        let _ = std::fs::create_dir_all(dir);
                    }
                    let _ = image::save_buffer(&path, &rgba, img.size[0] as u32, img.size[1] as u32, image::ColorType::Rgba8);
                    done.store(true, Ordering::SeqCst);
                });
            }
            if shot.done.load(Ordering::SeqCst) {
                ctx.send_viewport_cmd(egui::ViewportCommand::Close);
            }
        } else if busy {
            ctx.request_repaint();
        }
    }

    fn on_exit(&mut self, _gl: Option<&eframe::glow::Context>) {
        self.save();
    }
}
