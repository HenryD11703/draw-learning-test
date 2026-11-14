//! Ventana principal: mapa de niveles, ejercicios, lienzo libre, presión y guía de Krita.

use crate::brush::{BrushKind, BrushPreset, presets};
use crate::canvas::{Canvas, H, PAPER, W, to8};
use crate::curve::Curve;
use crate::grading::{Grade, grade_copy, grade_paint, grade_perspective, grade_points, grade_trace};
use crate::lessons::{Exercise, Guide, Kind, Module, PaintTarget, Scene, modules, paint_target, starting_canvas};
use crate::pressure::{MouseMode, PressureSettings, SpeedSmoother, simulated};
use crate::progress::Progress;
use crate::smoothing::{PInfo, Segment, SmoothingType, Smoother};
use egui::{
    Align2, Color32, ColorImage, CornerRadius, Event, FontId, Pos2, Rect, RichText, Sense, Shape, Stroke, StrokeKind,
    TextureHandle, TextureOptions, Ui, Vec2, pos2, vec2,
};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Instant;

const ACCENT: Color32 = Color32::from_rgb(61, 174, 233);
const PANEL: Color32 = Color32::from_rgb(49, 54, 59);
const DARK: Color32 = Color32::from_rgb(35, 38, 41);
const GOLD: Color32 = Color32::from_rgb(255, 200, 60);

#[derive(Clone, Copy, PartialEq, Eq)]
enum Screen {
    Map,
    Exercise(usize, usize),
    Free,
    Pressure,
    Guide,
}

struct PaintRef {
    target: Vec<[f32; 3]>,
    mask: Vec<bool>,
    kind: PaintTarget,
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
}

impl TrazoApp {
    pub fn new(cc: &eframe::CreationContext<'_>, screenshot: Option<Screenshot>) -> Self {
        setup_style(&cc.egui_ctx);
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
            screen: Screen::Map,
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
        }
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
        self.screen = Screen::Exercise(m, e);
        self.grade = None;
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
            Kind::Paint { target } => {
                self.canvas.reset_with(starting_canvas(target));
                let (img, mask) = paint_target(target);
                let reference = ctx.load_texture("referencia", to_color_image(&img), TextureOptions::LINEAR);
                let outline = ctx.load_texture("contorno", outline_image(&mask), TextureOptions::LINEAR);
                self.paint_ref = Some(PaintRef { target: img, mask, kind: target, reference, outline });
            }
            _ => {
                self.canvas.reset_with(None);
                self.paint_ref = None;
            }
        }
    }

    fn open_free(&mut self) {
        self.screen = Screen::Free;
        self.grade = None;
        self.paint_ref = None;
    }

    fn grade_now(&mut self) {
        let Some(ex) = self.current_exercise().cloned() else { return };
        let strokes = &self.canvas.strokes;
        let g = match &ex.kind {
            Kind::Trace { guides, tol } => grade_trace(strokes, guides, *tol, self.smoother.opts.magnet),
            Kind::Points { guides, tol } => grade_points(strokes, guides, *tol),
            Kind::Copy { guides, tol } => grade_copy(strokes, guides, *tol),
            Kind::Perspective(scene) => grade_perspective(strokes, scene),
            Kind::Paint { .. } => match &self.paint_ref {
                Some(r) => grade_paint(&self.canvas, &r.target, &r.mask, r.kind),
                None => return,
            },
        };
        self.progress.record(&ex.id, g.total, g.stars);
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
        for g in guides {
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

    fn handle_canvas_input(&mut self, ui: &Ui, resp: &egui::Response, rect: Rect) {
        let scale = rect.width() / W as f32;
        self.smoother.zoom = scale;
        let to_canvas = |p: Pos2| pos2((p.x - rect.min.x) / scale, (p.y - rect.min.y) / scale);
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
        if resp.drag_started_by(egui::PointerButton::Primary) {
            if let Some(p) = resp.interact_pointer_pos() {
                self.speed.reset();
                let cp = self.magnet(to_canvas(p));
                let sp = self.speed.next(cp, now);
                let pr = self.pressure_from(force, sp, now);
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
                _ => {}
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
            Kind::Trace { guides, .. } | Kind::Points { guides, .. } => guides.iter().map(|g| shaky(&g.pts, g.pressure.as_ref())).collect(),
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
            Kind::Paint { .. } => (0..12)
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

    fn top_bar(&mut self, ui: &mut Ui) {
        egui::Panel::top("barra")
            .frame(egui::Frame::new().fill(DARK).inner_margin(egui::Margin::symmetric(12, 8)))
            .show(ui, |ui| {
                ui.horizontal(|ui| {
                    logo(ui);
                    ui.label(RichText::new("Trazo Academia").size(20.0).strong().color(Color32::WHITE));
                    ui.label(RichText::new("aprende a dibujar en digital y a usar Krita").color(Color32::GRAY));
                    ui.add_space(24.0);
                    let tabs = [
                        (Screen::Map, "Niveles"),
                        (Screen::Free, "Lienzo libre"),
                        (Screen::Pressure, "Presión"),
                        (Screen::Guide, "Guía de Krita"),
                    ];
                    for (s, label) in tabs {
                        let selected = match (self.screen, s) {
                            (Screen::Exercise(..), Screen::Map) => true,
                            (a, b) => a == b,
                        };
                        if ui.selectable_label(selected, RichText::new(label).size(15.0)).clicked() {
                            match s {
                                Screen::Free => self.open_free(),
                                other => self.screen = other,
                            }
                        }
                    }
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        let total: u32 = self.modules.iter().map(|m| m.exercises.len() as u32 * 3).sum();
                        ui.label(RichText::new(format!("{} / {}", self.progress.total_stars(), total)).size(16.0).color(GOLD));
                        let (r, _) = ui.allocate_exact_size(vec2(20.0, 20.0), Sense::hover());
                        star(ui.painter(), r.center(), 9.0, GOLD);
                    });
                });
            });
    }

    fn map_screen(&mut self, ui: &mut Ui) {
        let ctx = ui.ctx().clone();
        egui::CentralPanel::default().frame(egui::Frame::new().fill(PANEL).inner_margin(16)).show(ui, |ui| {
            egui::ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
                ui.horizontal(|ui| {
                    ui.heading(RichText::new("Mapa de niveles").size(24.0).color(Color32::WHITE));
                    ui.add_space(16.0);
                    ui.label("Consigue al menos una estrella en la mitad de los ejercicios de un módulo para abrir el siguiente.");
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if ui.checkbox(&mut self.progress.unlock_all, "Desbloquear todo (modo profesor)").changed() {
                            self.save();
                        }
                    });
                });
                ui.add_space(10.0);
                let mut open: Option<(usize, usize)> = None;
                let width = ui.available_width();
                let cols = if width > 1100.0 { 2 } else { 1 };
                let card_w = (width - 16.0 * (cols as f32 - 1.0)) / cols as f32;
                for row in (0..self.modules.len()).collect::<Vec<_>>().chunks(cols) {
                    ui.horizontal_top(|ui| {
                        for &m in row {
                            ui.allocate_ui_with_layout(vec2(card_w, 10.0), egui::Layout::top_down(egui::Align::Min), |ui| {
                                ui.set_width(card_w);
                                if let Some(o) = self.module_card(ui, m) {
                                    open = Some(o);
                                }
                            });
                        }
                    });
                    ui.add_space(14.0);
                }
                if let Some((m, e)) = open {
                    self.open_exercise(&ctx, m, e);
                }
            });
        });
    }

    fn module_card(&mut self, ui: &mut Ui, m: usize) -> Option<(usize, usize)> {
        let md = &self.modules[m];
        let accent = md.accent;
        let unlocked = self.module_unlocked(m);
        let status = if unlocked {
            format!("{} de {} ejercicios con estrella", self.module_done(m), md.exercises.len())
        } else {
            let need = self.module_needed(m - 1) - self.module_done(m - 1);
            format!("Bloqueado: consigue estrella en {need} ejercicio{} más del módulo {}", if need == 1 { "" } else { "s" }, m)
        };
        let mut open = None;
        egui::Frame::new()
            .fill(Color32::from_rgb(42, 46, 50))
            .stroke(Stroke::new(1.0, Color32::from_rgb(70, 75, 80)))
            .corner_radius(10)
            .inner_margin(14)
            .show(ui, |ui| {
                ui.set_width(ui.available_width());
                ui.horizontal(|ui| {
                    let (r, _) = ui.allocate_exact_size(vec2(6.0, 40.0), Sense::hover());
                    ui.painter().rect_filled(r, 3, accent);
                    ui.vertical(|ui| {
                        ui.label(RichText::new(format!("{}. {}", m + 1, md.title)).size(18.0).strong().color(Color32::WHITE));
                        ui.label(RichText::new(md.subtitle).color(Color32::from_gray(170)));
                    });
                });
                ui.label(RichText::new(status).small().color(if unlocked { Color32::from_gray(150) } else { GOLD }));
                egui::CollapsingHeader::new(RichText::new("Lección").color(accent))
                    .id_salt(("leccion", m))
                    .show(ui, |ui| {
                        ui.label(RichText::new(md.lesson).color(Color32::from_gray(210)));
                    });
                ui.add_space(6.0);
                ui.horizontal_wrapped(|ui| {
                    for (e, ex) in md.exercises.iter().enumerate() {
                        let unlocked = self.unlocked(m, e);
                        let stars = self.progress.stars(&ex.id);
                        if level_node(ui, e + 1, ex.title, stars, unlocked, accent).clicked() && unlocked {
                            open = Some((m, e));
                        }
                    }
                });
            });
        open
    }

    fn tools_panel(&mut self, ui: &mut Ui) {
        egui::Panel::left("herramientas")
            .resizable(false)
            .exact_size(250.0)
            .frame(egui::Frame::new().fill(PANEL).inner_margin(10))
            .show(ui, |ui| {
                egui::ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
                    section(ui, "Pinceles");
                    let mut last_cat = "";
                    for i in 0..self.brushes.len() {
                        let b = &self.brushes[i];
                        if b.category != last_cat {
                            last_cat = b.category;
                            ui.label(RichText::new(b.category).small().color(Color32::from_gray(150)));
                        }
                        let sel = self.brush == i;
                        let resp = ui
                            .add_sized(
                                [ui.available_width(), 26.0],
                                egui::Button::selectable(sel, RichText::new(b.name).color(if sel { Color32::WHITE } else { Color32::from_gray(215) })),
                            )
                            .on_hover_text(format!("En Krita: {}\n\n{}", b.krita_name, b.description));
                        if resp.clicked() {
                            self.brush = i;
                            self.eraser_return = None;
                            if let Some(c) = self.brushes[i].default_color {
                                if self.current_exercise().and_then(|e| e.color).is_none() {
                                    self.canvas.color = c;
                                }
                            }
                        }
                    }
                    ui.add_space(6.0);
                    let b = &self.brushes[self.brush];
                    ui.label(RichText::new(b.description).small().color(Color32::from_gray(185)));
                    ui.label(RichText::new(format!("En Krita: {}", b.krita_name)).small().color(ACCENT));
                    ui.add_space(8.0);
                    section(ui, "Ajustes");
                    ui.add(egui::Slider::new(&mut self.canvas.size_mult, 0.15..=6.0).logarithmic(true).text("Tamaño ×"));
                    ui.add(egui::Slider::new(&mut self.canvas.opacity_mult, 0.05..=1.0).text("Opacidad"));
                    ui.label(RichText::new("Atajos como en Krita: [ y ] tamaño, E borrador, Ctrl+Z deshacer.").small().color(Color32::from_gray(150)));
                    ui.add_space(8.0);
                    section(ui, "Color");
                    let mut rgb = self.canvas.color;
                    ui.horizontal(|ui| {
                        ui.color_edit_button_rgb(&mut rgb);
                        let c = Color32::from_rgb(to8(rgb[0]), to8(rgb[1]), to8(rgb[2]));
                        ui.label(RichText::new(format!("#{:02X}{:02X}{:02X}", c.r(), c.g(), c.b())).monospace());
                    });
                    let swatches: [[f32; 3]; 12] = [
                        [0.05, 0.05, 0.07], [0.3, 0.3, 0.32], [0.55, 0.55, 0.57], [0.8, 0.8, 0.8], [0.985, 0.98, 0.965], [0.85, 0.35, 0.2],
                        [0.95, 0.75, 0.25], [0.35, 0.65, 0.3], [0.2, 0.45, 0.85], [0.45, 0.3, 0.7], [0.8, 0.3, 0.5], [0.45, 0.3, 0.2],
                    ];
                    ui.horizontal_wrapped(|ui| {
                        for s in swatches {
                            let (r, resp) = ui.allocate_exact_size(vec2(26.0, 22.0), Sense::click());
                            ui.painter().rect_filled(r, 4, Color32::from_rgb(to8(s[0]), to8(s[1]), to8(s[2])));
                            ui.painter().rect_stroke(r, 4, Stroke::new(1.0, Color32::from_gray(90)), StrokeKind::Inside);
                            if resp.clicked() {
                                rgb = s;
                            }
                        }
                    });
                    self.canvas.color = rgb;
                    ui.add_space(8.0);
                    ui.horizontal(|ui| {
                        if ui.add_enabled(self.canvas.can_undo(), egui::Button::new("Deshacer")).clicked() {
                            self.canvas.undo();
                        }
                        if ui.button("Borrar todo").clicked() {
                            if let Some(r) = &self.paint_ref {
                                let start = starting_canvas(r.kind);
                                self.canvas.reset_with(start);
                            } else {
                                self.canvas.clear();
                            }
                            self.grade = None;
                        }
                    });
                    if self.screen == Screen::Free && ui.button("Guardar como PNG").clicked() {
                        self.save_png();
                    }
                });
            });
    }

    fn save_png(&mut self) {
        let base = std::env::var_os("USERPROFILE").map(std::path::PathBuf::from).unwrap_or_default();
        let dir = base.join("Pictures").join("TrazoAcademia");
        let _ = std::fs::create_dir_all(&dir);
        let stamp = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0);
        let path = dir.join(format!("dibujo-{stamp}.png"));
        let ok = image::save_buffer(&path, &self.canvas.to_rgba(), W as u32, H as u32, image::ColorType::Rgba8).is_ok();
        let msg = if ok { format!("Guardado en {}", path.display()) } else { "No se pudo guardar la imagen.".into() };
        self.message = Some((msg, self.now_ms()));
    }

    fn assist_panel(&mut self, ui: &mut Ui) {
        let ctx = ui.ctx().clone();
        egui::Panel::right("asistente")
            .resizable(false)
            .exact_size(330.0)
            .frame(egui::Frame::new().fill(PANEL).inner_margin(12))
            .show(ui, |ui| {
                egui::ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
                    if let Some(ex) = self.current_exercise().cloned() {
                        let Screen::Exercise(m, e) = self.screen else { return };
                        let accent = self.modules[m].accent;
                        ui.label(RichText::new(format!("{}. {}", m + 1, self.modules[m].title)).small().color(accent));
                        ui.label(RichText::new(format!("Ejercicio {} · {}", e + 1, ex.title)).size(19.0).strong().color(Color32::WHITE));
                        ui.add_space(4.0);
                        ui.label(RichText::new(ex.goal).color(Color32::from_gray(220)));
                        if let Some(sec) = ex.memory {
                            let left = sec as f64 - (self.now_ms() - self.ex_started) / 1000.0;
                            if left > 0.0 {
                                ui.label(RichText::new(format!("La guía se ocultará en {:.0} s", left.ceil())).color(GOLD));
                            } else if ui.button("Ver la guía otra vez").clicked() {
                                self.ex_started = self.now_ms();
                            }
                        }
                        ui.add_space(6.0);
                        krita_box(ui, ex.krita_tip);
                        ui.add_space(8.0);
                        if let Kind::Copy { guides, .. } = &ex.kind {
                            section(ui, "Referencia");
                            draw_reference(ui, guides);
                            ui.add_space(8.0);
                        }
                        // La guía magnética solo tiene sentido cuando se calca.
                        self.smoothing_ui(ui, matches!(ex.kind, Kind::Trace { .. }));
                        ui.add_space(8.0);
                        self.pressure_meter(ui);
                        ui.add_space(10.0);
                        if let (Kind::Paint { .. }, Some(r)) = (&ex.kind, &self.paint_ref) {
                            section(ui, "Referencia");
                            let w = ui.available_width();
                            let size = vec2(w, w * H as f32 / W as f32);
                            ui.add(egui::Image::new((r.reference.id(), size)).corner_radius(6));
                            ui.add_space(8.0);
                        }
                        let toggle = match &ex.kind {
                            Kind::Trace { .. } if ex.memory.is_none() => Some("Mostrar guía"),
                            Kind::Perspective(s) if s.hints && ex.memory.is_none() => Some("Mostrar ayuda"),
                            _ => None,
                        };
                        ui.horizontal(|ui| {
                            let calif = egui::Button::new(RichText::new("Calificar").size(16.0).strong().color(Color32::BLACK)).fill(ACCENT);
                            if ui.add_sized([130.0, 34.0], calif).clicked() {
                                self.grade_now();
                            }
                            if let Some(label) = toggle {
                                ui.checkbox(&mut self.show_guide, label);
                            }
                        });
                        let best = self.progress.results.get(&ex.id).copied();
                        if let Some(b) = best {
                            ui.label(RichText::new(format!("Mejor puntuación: {:.0} · intentos: {}", b.best, b.attempts)).small().color(Color32::from_gray(160)));
                        }
                        if let Some(g) = self.grade.clone() {
                            ui.add_space(8.0);
                            grade_box(ui, &g);
                            ui.add_space(6.0);
                            ui.horizontal(|ui| {
                                if ui.button("Repetir").clicked() {
                                    self.open_exercise(&ctx, m, e);
                                }
                                if g.stars > 0 && ui.button(RichText::new("Siguiente ejercicio ›").strong()).clicked() {
                                    self.next_exercise(&ctx);
                                }
                            });
                        }
                    } else {
                        ui.label(RichText::new("Lienzo libre").size(19.0).strong().color(Color32::WHITE));
                        ui.label("Dibuja y pinta lo que quieras con los pinceles y el asistente de trazo. Guarda tu obra como PNG desde el panel de la izquierda.");
                        ui.add_space(8.0);
                        self.smoothing_ui(ui, false);
                        ui.add_space(8.0);
                        self.pressure_meter(ui);
                    }
                    if let Some((msg, t)) = &self.message {
                        if self.now_ms() - t < 6000.0 {
                            ui.add_space(8.0);
                            ui.label(RichText::new(msg).color(GOLD));
                        }
                    }
                });
            });
    }

    fn smoothing_ui(&mut self, ui: &mut Ui, allow_magnet: bool) {
        section(ui, "Asistente de trazo (suavizado)");
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
        section(ui, "Presión");
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

    fn exercise_or_free_screen(&mut self, ui: &mut Ui) {
        self.tools_panel(ui);
        self.assist_panel(ui);
        egui::CentralPanel::default().frame(egui::Frame::new().fill(Color32::from_rgb(30, 32, 35)).inner_margin(14)).show(ui, |ui| {
            let avail = ui.available_size();
            let scale = (avail.x / W as f32).min(avail.y / H as f32).max(0.1);
            let size = vec2(W as f32 * scale, H as f32 * scale);
            let rect = Rect::from_center_size(ui.available_rect_before_wrap().center(), size);
            let resp = ui.interact(rect, egui::Id::unique("lienzo"), Sense::drag());
            self.handle_canvas_input(ui, &resp, rect);
            let ctx = ui.ctx().clone();
            let tex = self.canvas.texture(&ctx).id();
            let painter = ui.painter_at(rect.expand(4.0));
            painter.rect_filled(rect.expand(1.0), 0, Color32::from_gray(20));
            painter.image(tex, rect, Rect::from_min_max(pos2(0.0, 0.0), pos2(1.0, 1.0)), Color32::WHITE);
            if let Some(r) = &self.paint_ref {
                painter.image(r.outline.id(), rect, Rect::from_min_max(pos2(0.0, 0.0), pos2(1.0, 1.0)), Color32::WHITE);
            }
            if let Some(ex) = self.current_exercise() {
                let aids = self.memory_visible();
                match &ex.kind {
                    Kind::Trace { guides, .. } if self.guide_visible() => {
                        let size = self.brushes[self.brush].size * self.canvas.size_mult;
                        draw_guides(&painter, guides, rect, scale, size);
                    }
                    Kind::Points { guides, .. } if aids => draw_points(&painter, guides, rect, scale),
                    Kind::Perspective(scene) => draw_scene(&painter, scene, rect, scale, aids, aids && self.show_guide && scene.hints),
                    _ => {}
                }
            }
            // Cursor del pincel y posición estabilizada.
            if let Some(hp) = resp.hover_pos().or(resp.interact_pointer_pos()) {
                let d = (self.brushes[self.brush].size * self.canvas.size_mult * scale).max(4.0);
                painter.circle_stroke(hp, d * 0.5, Stroke::new(1.0, Color32::from_gray(60)));
                if let Some(bp) = self.smoother.brush_position() {
                    if self.smoother.opts.kind == SmoothingType::Estabilizador && self.drawing {
                        let sp = rect.min + bp.to_vec2() * scale;
                        painter.line_segment([sp, hp], Stroke::new(1.0, ACCENT.gamma_multiply(0.7)));
                        painter.circle_filled(sp, 3.0, ACCENT);
                    }
                }
            }
            if resp.hovered() {
                ctx.set_cursor_icon(egui::CursorIcon::Crosshair);
            }
        });
    }

    fn pressure_screen(&mut self, ui: &mut Ui) {
        let now = self.now_ms();
        egui::CentralPanel::default().frame(egui::Frame::new().fill(PANEL).inner_margin(18)).show(ui, |ui| {
            egui::ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
                ui.heading(RichText::new("Presión adaptada").size(24.0).color(Color32::WHITE));
                ui.label("Krita lee la presión del lápiz y la pasa por una curva global (Configuración › Configurar Krita › Ajustes de tableta). Aquí haces lo mismo, y además puedes calibrarla a tu mano.");
                ui.add_space(12.0);
                ui.horizontal_top(|ui| {
                    ui.vertical(|ui| {
                        ui.set_width(320.0);
                        section(ui, "Curva de presión");
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
                        section(ui, "Sin tableta (ratón)");
                        let pm = &mut self.progress.pressure;
                        ui.radio_value(&mut pm.mouse_mode, MouseMode::PorVelocidad, "Simular con la velocidad (lento = más presión)");
                        ui.radio_value(&mut pm.mouse_mode, MouseMode::Constante, "Presión fija");
                        if pm.mouse_mode == MouseMode::Constante {
                            ui.add(egui::Slider::new(&mut pm.mouse_constant, 0.05..=1.0).text("Presión"));
                        }
                    });
                    ui.add_space(20.0);
                    ui.vertical(|ui| {
                        section(ui, "Calibración personal");
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
                        ui.add_space(8.0);
                        self.pressure_meter(ui);
                        ui.label(RichText::new("Si tienes tableta y el indicador no cambia, revisa que el controlador de la tableta esté instalado (Windows Ink activado).").small().color(Color32::from_gray(160)));
                    });
                });
            });
        });
    }

    fn guide_screen(&mut self, ui: &mut Ui) {
        egui::CentralPanel::default().frame(egui::Frame::new().fill(PANEL).inner_margin(18)).show(ui, |ui| {
            egui::ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
                ui.set_max_width(980.0);
                ui.heading(RichText::new("Guía rápida de Krita").size(24.0).color(Color32::WHITE));
                ui.label("Lo que aprendes en Trazo Academia funciona igual en Krita. Esta guía te dice dónde está cada cosa.");
                ui.add_space(10.0);
                for (title, body) in KRITA_GUIDE {
                    egui::Frame::new().fill(Color32::from_rgb(42, 46, 50)).corner_radius(8).inner_margin(12).show(ui, |ui| {
                        ui.set_width(ui.available_width());
                        ui.label(RichText::new(*title).size(17.0).strong().color(ACCENT));
                        ui.label(RichText::new(*body).color(Color32::from_gray(215)));
                    });
                    ui.add_space(8.0);
                }
                egui::Frame::new().fill(Color32::from_rgb(42, 46, 50)).corner_radius(8).inner_margin(12).show(ui, |ui| {
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
    ("Capas", "Trabaja siempre en capas: boceto, tinta y color separados. Baja la opacidad del boceto para entintar encima. Bloquear alfa evita pintar fuera de lo ya pintado."),
    ("Asistentes de dibujo", "La herramienta de Asistentes coloca reglas, elipses y puntos de fuga que atraen tu trazo. Son la versión profesional de la guía magnética de esta app."),
    ("Ver con ojos frescos", "La tecla M voltea la vista en espejo para detectar errores de proporción. También puedes rotar el lienzo para trazar en tu ángulo más cómodo."),
];

// ------------------------------------------------------------------ dibujo auxiliar

fn setup_style(ctx: &egui::Context) {
    let mut v = egui::Visuals::dark();
    v.panel_fill = PANEL;
    v.window_fill = PANEL;
    v.extreme_bg_color = DARK;
    v.selection.bg_fill = ACCENT.gamma_multiply(0.55);
    v.selection.stroke = Stroke::new(1.0, Color32::WHITE);
    v.hyperlink_color = ACCENT;
    v.widgets.inactive.weak_bg_fill = Color32::from_rgb(60, 65, 71);
    v.widgets.inactive.bg_fill = Color32::from_rgb(60, 65, 71);
    v.widgets.hovered.weak_bg_fill = Color32::from_rgb(75, 82, 89);
    v.widgets.active.weak_bg_fill = ACCENT.gamma_multiply(0.7);
    ctx.set_visuals(v);
    ctx.global_style_mut(|s| {
        s.spacing.item_spacing = vec2(8.0, 6.0);
        s.spacing.button_padding = vec2(8.0, 4.0);
    });
}

fn section(ui: &mut Ui, title: &str) {
    ui.label(RichText::new(title.to_uppercase()).small().strong().color(Color32::from_gray(160)));
    ui.separator();
}

fn krita_box(ui: &mut Ui, text: &str) {
    egui::Frame::new()
        .fill(Color32::from_rgb(33, 52, 66))
        .stroke(Stroke::new(1.0, ACCENT.gamma_multiply(0.6)))
        .corner_radius(6)
        .inner_margin(10)
        .show(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.label(RichText::new("EN KRITA").small().strong().color(ACCENT));
            ui.label(RichText::new(text).color(Color32::from_gray(220)));
        });
}

fn bar(ui: &mut Ui, label: &str, v: f32, color: Color32) {
    ui.horizontal(|ui| {
        ui.add_sized([90.0, 16.0], egui::Label::new(RichText::new(label).small()));
        let (r, _) = ui.allocate_exact_size(vec2(ui.available_width() - 44.0, 12.0), Sense::hover());
        ui.painter().rect_filled(r, 4, Color32::from_gray(30));
        let mut f = r;
        f.set_width(r.width() * v.clamp(0.0, 1.0));
        ui.painter().rect_filled(f, 4, color);
        ui.label(RichText::new(format!("{:.0}%", v * 100.0)).small().monospace());
    });
}

fn grade_box(ui: &mut Ui, g: &Grade) {
    egui::Frame::new().fill(Color32::from_rgb(38, 42, 46)).corner_radius(8).inner_margin(12).show(ui, |ui| {
        ui.set_width(ui.available_width());
        ui.horizontal(|ui| {
            ui.label(RichText::new(format!("{:.0}", g.total)).size(36.0).strong().color(Color32::WHITE));
            ui.label(RichText::new("/100").color(Color32::GRAY));
            ui.add_space(10.0);
            for i in 0..3 {
                let (r, _) = ui.allocate_exact_size(vec2(28.0, 28.0), Sense::hover());
                star(ui.painter(), r.center(), 12.0, if i < g.stars { GOLD } else { Color32::from_gray(70) });
            }
        });
        for m in &g.metrics {
            let c = if m.value >= 0.85 { Color32::from_rgb(110, 210, 120) } else if m.value >= 0.6 { GOLD } else { Color32::from_rgb(235, 100, 90) };
            ui.horizontal(|ui| {
                ui.add_sized([90.0, 16.0], egui::Label::new(RichText::new(m.name).small()));
                let (r, resp) = ui.allocate_exact_size(vec2(ui.available_width() - 44.0, 10.0), Sense::hover());
                ui.painter().rect_filled(r, 4, Color32::from_gray(30));
                let mut f = r;
                f.set_width(r.width() * m.value);
                ui.painter().rect_filled(f, 4, c);
                resp.on_hover_text(m.help);
                ui.label(RichText::new(format!("{:.0}", m.value * 100.0)).small().monospace());
            });
        }
        ui.add_space(4.0);
        ui.label(RichText::new(&g.advice).color(Color32::from_gray(220)));
    });
}

fn star(p: &egui::Painter, c: Pos2, r: f32, color: Color32) {
    let pt = |i: usize, rad: f32| {
        let a = -std::f32::consts::FRAC_PI_2 + i as f32 * std::f32::consts::PI / 5.0;
        pos2(c.x + rad * a.cos(), c.y + rad * a.sin())
    };
    for k in 0..5 {
        let outer = pt(2 * k, r);
        let in1 = pt(2 * k + 9, r * 0.45);
        let in2 = pt(2 * k + 1, r * 0.45);
        p.add(Shape::convex_polygon(vec![c, in1, outer, in2], color, Stroke::NONE));
    }
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

fn level_node(ui: &mut Ui, n: usize, title: &str, stars: u8, unlocked: bool, accent: Color32) -> egui::Response {
    let (r, resp) = ui.allocate_exact_size(vec2(118.0, 92.0), Sense::click());
    let p = ui.painter();
    let c = pos2(r.center().x, r.min.y + 28.0);
    let hovered = resp.hovered() && unlocked;
    let fill = if !unlocked {
        Color32::from_gray(55)
    } else if stars > 0 {
        accent
    } else {
        Color32::from_rgb(70, 76, 82)
    };
    p.circle_filled(c, if hovered { 25.0 } else { 23.0 }, fill);
    p.circle_stroke(c, 23.0, Stroke::new(2.0, if unlocked { accent } else { Color32::from_gray(80) }));
    if unlocked {
        p.text(c, Align2::CENTER_CENTER, n.to_string(), FontId::proportional(18.0), if stars > 0 { Color32::BLACK } else { Color32::WHITE });
    } else {
        // Candado
        let body = Rect::from_center_size(c + vec2(0.0, 3.0), vec2(14.0, 11.0));
        p.rect_filled(body, 2, Color32::from_gray(120));
        p.circle_stroke(c + vec2(0.0, -3.0), 5.0, Stroke::new(2.0, Color32::from_gray(120)));
    }
    for i in 0..3u8 {
        star(p, pos2(c.x - 16.0 + i as f32 * 16.0, r.min.y + 62.0), 6.5, if i < stars { GOLD } else { Color32::from_gray(65) });
    }
    let short: String = if title.chars().count() > 19 {
        title.chars().take(18).collect::<String>() + "…"
    } else {
        title.to_string()
    };
    p.text(pos2(c.x, r.min.y + 80.0), Align2::CENTER_CENTER, short, FontId::proportional(11.0), Color32::from_gray(if unlocked { 200 } else { 110 }));
    resp.on_hover_text(title)
}

fn draw_guides(p: &egui::Painter, guides: &[Guide], rect: Rect, scale: f32, brush_size: f32) {
    let to = |q: Pos2| rect.min + q.to_vec2() * scale;
    let guide_col = Color32::from_rgba_unmultiplied(70, 150, 230, 120);
    for g in guides {
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
fn draw_points(p: &egui::Painter, guides: &[Guide], rect: Rect, scale: f32) {
    let to = |q: Pos2| rect.min + q.to_vec2() * scale;
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
fn draw_scene(p: &egui::Painter, s: &Scene, rect: Rect, scale: f32, aids: bool, hints: bool) {
    let to = |q: Pos2| rect.min + q.to_vec2() * scale;
    if aids {
        if let Some(h) = s.horizon {
            let y = to(pos2(0.0, h)).y;
            if y >= rect.min.y && y <= rect.max.y {
                p.line_segment([pos2(rect.min.x, y), pos2(rect.max.x, y)], Stroke::new(1.5, Color32::from_rgb(120, 140, 170)));
                // A un cuarto del ancho: los puntos de fuga suelen estar en los extremos o en el centro.
                p.text(pos2(rect.min.x + rect.width() * 0.25, y - 3.0), Align2::CENTER_BOTTOM, "Horizonte", FontId::proportional(12.0), Color32::from_rgb(90, 110, 140));
            }
        }
        let inner = rect.shrink(16.0);
        for (i, vp) in s.vps.iter().enumerate() {
            let label = format!("PF{}", i + 1);
            let q = to(*vp);
            if rect.contains(q) {
                p.circle_stroke(q, 7.0, Stroke::new(2.0, VP_RED));
                p.line_segment([q - vec2(10.0, 0.0), q + vec2(10.0, 0.0)], Stroke::new(1.5, VP_RED));
                p.line_segment([q - vec2(0.0, 10.0), q + vec2(0.0, 10.0)], Stroke::new(1.5, VP_RED));
                p.text(q + vec2(10.0, 8.0), Align2::LEFT_TOP, label, FontId::proportional(13.0), VP_RED);
            } else {
                // Fuera del lienzo: flecha en el borde apuntando hacia él.
                let c = inner.clamp(q);
                let d = (q - c).normalized() * 14.0;
                p.arrow(c - d, d, Stroke::new(2.5, VP_RED));
                let anchor = if c.y < rect.center().y { Align2::CENTER_TOP } else { Align2::CENTER_BOTTOM };
                let off = if c.y < rect.center().y { vec2(0.0, 20.0) } else { vec2(0.0, -20.0) };
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

/// Miniatura de la referencia para los ejercicios de copia.
fn draw_reference(ui: &mut Ui, guides: &[Guide]) {
    let w = ui.available_width();
    let (r, _) = ui.allocate_exact_size(vec2(w, w * H as f32 / W as f32), Sense::hover());
    let p = ui.painter_at(r);
    p.rect_filled(r, 6, Color32::from_rgb(248, 247, 242));
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
        self.shortcuts(&ctx);
        self.top_bar(ui);
        match self.screen {
            Screen::Map => self.map_screen(ui),
            Screen::Exercise(..) | Screen::Free => self.exercise_or_free_screen(ui),
            Screen::Pressure => self.pressure_screen(ui),
            Screen::Guide => self.guide_screen(ui),
        }
        let busy = self.drawing || self.current_exercise().is_some_and(|e| e.memory.is_some());
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
        let _ = (CornerRadius::ZERO, Vec2::ZERO);
    }

    fn on_exit(&mut self, _gl: Option<&eframe::glow::Context>) {
        self.save();
    }
}
