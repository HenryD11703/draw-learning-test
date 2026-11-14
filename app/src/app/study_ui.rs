//! Pantallas del estudio con referencia y del diario de estudios.
//!
//! El estudio va en tres fases: preparar (soltar la foto, elegir tema y tiempo), dibujar (con la
//! referencia al lado) y comparar (superponer, alternar, espejo, grises, rejilla y medidas) con una
//! autocrítica guiada que se guarda en el diario.

use super::*;
use crate::study::{self, Answer, Entry, Topic};
use egui::Vec2;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(super) enum Phase {
    Setup,
    Draw,
    Compare,
}

/// La referencia del estudio en su orientación actual, en color y en grises.
pub(super) struct RefImage {
    pub rgba: image::RgbaImage,
    pub tex: TextureHandle,
    pub gray: TextureHandle,
}

impl RefImage {
    pub fn new(ctx: &egui::Context, rgba: image::RgbaImage) -> Self {
        let tex = ctx.load_texture("estudio_ref", color_image(&rgba), TextureOptions::LINEAR);
        let mut g = rgba.clone();
        for p in g.pixels_mut() {
            let l = (0.2126 * p[0] as f32 + 0.7152 * p[1] as f32 + 0.0722 * p[2] as f32).round() as u8;
            p.0 = [l, l, l, p[3]];
        }
        let gray = ctx.load_texture("estudio_ref_gris", color_image(&g), TextureOptions::LINEAR);
        Self { rgba, tex, gray }
    }

    pub fn size(&self) -> Vec2 {
        vec2(self.rgba.width() as f32, self.rgba.height() as f32)
    }
}

pub(super) struct StudyState {
    pub reference: Option<RefImage>,
    pub topic: Topic,
    /// Tiempo elegido, en segundos.
    pub timer: Option<f32>,
    pub started: f64,
    pub minutes: f32,
    pub phase: Phase,
    /// Vista de la referencia: zoom y desplazamiento.
    pub zoom: f32,
    pub pan: Vec2,
    pub hide_ref: bool,
    /// Comparar: opacidad de la referencia encima del dibujo, parpadeo y alineación.
    pub overlay: f32,
    pub blink: bool,
    pub align: bool,
    pub ov_offset: Vec2,
    pub ov_scale: f32,
    pub grid: bool,
    /// Medir: segmentos en px de la referencia y del lienzo. El primero de cada lado es la unidad.
    pub measuring: bool,
    pub meas_ref: Vec<[Pos2; 2]>,
    pub meas_draw: Vec<[Pos2; 2]>,
    pub drag: Option<(bool, Pos2)>,
    pub answers: Vec<Option<Answer>>,
    pub best: String,
    pub improve: String,
    pub saved: bool,
}

impl Default for StudyState {
    fn default() -> Self {
        Self {
            reference: None,
            topic: Topic::Proportion,
            timer: None,
            started: 0.0,
            minutes: 0.0,
            phase: Phase::Setup,
            zoom: 1.0,
            pan: Vec2::ZERO,
            hide_ref: false,
            overlay: 0.0,
            blink: false,
            align: false,
            ov_offset: Vec2::ZERO,
            ov_scale: 1.0,
            grid: false,
            measuring: false,
            meas_ref: Vec::new(),
            meas_draw: Vec::new(),
            drag: None,
            answers: Vec::new(),
            best: String::new(),
            improve: String::new(),
            saved: false,
        }
    }
}

/// Un estudio del diario con sus miniaturas cuando ya se cargaron.
pub(super) struct DiaryItem {
    pub path: std::path::PathBuf,
    pub entry: Entry,
    pub ref_tex: Option<TextureHandle>,
    pub draw_tex: Option<TextureHandle>,
    pub failed: bool,
}

const MEASURE_COL: Color32 = Color32::from_rgb(240, 150, 40);

impl TrazoApp {
    pub(super) fn open_study(&mut self) {
        self.use_canvas(Slot::Study);
        self.from_program = false;
        self.screen = Screen::Study;
        self.grade = None;
        self.clear_result();
        self.paint_ref = None;
    }

    /// Demostración (TRAZO_PANTALLA=estudio): una escena de pintura como referencia y un «dibujo»
    /// hecho a partir de ella, algo más pequeño y desplazado. Con TRAZO_FASE=comparar abre la
    /// comparación con la referencia superpuesta, medidas y la autocrítica a medio responder.
    pub(super) fn demo_study(&mut self, ctx: &egui::Context) {
        self.open_study();
        let scene = self.modules.iter().flat_map(|m| &m.exercises).filter_map(|ex| match ex.build(0) {
            Kind::Paint(p) => Some(p),
            _ => None,
        });
        let scene = scene.last().map(|p| p.render().target);
        let Some(target) = scene else { return };
        let mut img = image::RgbaImage::new(W as u32, H as u32);
        for (i, c) in target.iter().enumerate() {
            img.put_pixel((i % W) as u32, (i / W) as u32, image::Rgba([to8(c[0]), to8(c[1]), to8(c[2]), 255]));
        }
        self.study.topic = Topic::Value;
        self.set_study_reference(ctx, image::DynamicImage::ImageRgba8(img));
        self.start_study();
        let c = pos2(W as f32 / 2.0, H as f32 / 2.0);
        let mut layer = vec![[0.0f32; 4]; W * H];
        for (i, px) in layer.iter_mut().enumerate() {
            let p = pos2((i % W) as f32, (i / W) as f32);
            let q = c + (p - c) / 0.9 - vec2(30.0, -12.0);
            if q.x < 0.0 || q.y < 0.0 || q.x >= W as f32 || q.y >= H as f32 {
                continue;
            }
            let t = target[q.y as usize * W + q.x as usize];
            let l = 0.2126 * t[0] + 0.7152 * t[1] + 0.0722 * t[2];
            // Tres valores, como un estudio rápido de masas.
            let v = if l < 0.3 { 0.85 } else if l < 0.6 { 0.45 } else { 0.0 };
            *px = [0.12, 0.12, 0.14, v];
        }
        self.canvas.add_image_layer("Estudio", &layer);
        if std::env::var("TRAZO_FASE").as_deref() == Ok("comparar") {
            self.study.started -= 14.0 * 60_000.0;
            self.finish_study();
            self.study.overlay = 0.45;
            self.study.meas_ref = vec![[pos2(300.0, 180.0), pos2(300.0, 330.0)], [pos2(260.0, 400.0), pos2(640.0, 400.0)]];
            self.study.meas_draw = vec![[pos2(285.0, 175.0), pos2(285.0, 310.0)], [pos2(250.0, 375.0), pos2(590.0, 375.0)]];
            self.study.answers = vec![Some(Answer::Yes), Some(Answer::Partly), None, None];
            self.study.best = "Las masas de sombra se leen bien.".into();
            if std::env::var_os("TRAZO_DIARIO").is_some() {
                self.study.improve = "Medir antes de oscurecer.".into();
                self.save_study();
            }
        }
    }

    /// Demostración (TRAZO_PANTALLA=hoy): semana TRAZO_SEMANA (o la 6), tres días completos y hoy
    /// con un ejercicio hecho.
    pub(super) fn demo_program(&mut self, ctx: &egui::Context) {
        self.screen = Screen::Program;
        let p = &mut self.progress.program;
        p.week = std::env::var("TRAZO_SEMANA").ok().and_then(|s| s.parse::<usize>().ok()).map_or(5, |n| n.saturating_sub(1));
        let today = chrono::Local::now().date_naive();
        for back in [3i64, 2, 1] {
            let d = (today - chrono::Duration::days(back)).format("%Y-%m-%d").to_string();
            p.record_exercise("eli01", &d);
            p.record_exercise("persp01", &d);
            p.record_study(&d);
        }
        p.week_started = (today - chrono::Duration::days(3)).format("%Y-%m-%d").to_string();
        let week = p.week;
        let first = self.program.week(week).and_then(|(_, w)| w.warmup.first().cloned());
        if let Some(id) = first.and_then(|id| self.modules.iter().find(|m| m.id == id).map(|m| m.exercises[0].id.clone())) {
            self.progress.program.record_exercise(&id, &crate::program::today());
        }
        self.progress.program.reviews.push(Review { focus: "Medir la unidad antes de dibujar el contorno.".into(), ..Default::default() });
        // TRAZO_FASE=revision: hoy también completo, para ver la revisión de la semana.
        // TRAZO_FASE=estudio: abre el estudio de la semana con una foto de la biblioteca.
        match std::env::var("TRAZO_FASE").as_deref() {
            Ok("revision") => {
                let today = crate::program::today();
                self.progress.program.record_exercise("persp02", &today);
                self.progress.program.record_study(&today);
            }
            Ok("estudio") => {
                if let Some((_, w)) = self.program.week(week).map(|(m, w)| (m, w.clone())) {
                    self.start_program_study(ctx, &w);
                }
            }
            _ => {}
        }
    }

    pub(super) fn set_study_reference(&mut self, ctx: &egui::Context, img: image::DynamicImage) {
        let rgba = img.thumbnail(2048, 2048).to_rgba8();
        let s = &mut self.study;
        s.reference = Some(RefImage::new(ctx, rgba));
        s.zoom = 1.0;
        s.pan = Vec2::ZERO;
        s.meas_ref.clear();
        s.ov_offset = Vec2::ZERO;
        s.ov_scale = 1.0;
    }

    fn start_study(&mut self) {
        self.canvas = Canvas::new_free();
        self.time_up = false;
        let n = self.study.topic.questions().len();
        let now = self.now_ms();
        let s = &mut self.study;
        s.phase = Phase::Draw;
        s.started = now;
        s.answers = vec![None; n];
        s.best.clear();
        s.improve.clear();
        s.saved = false;
        s.meas_ref.clear();
        s.meas_draw.clear();
        s.overlay = 0.0;
        s.blink = false;
        s.align = false;
        s.hide_ref = false;
    }

    fn finish_study(&mut self) {
        let now = self.now_ms();
        let s = &mut self.study;
        s.minutes = ((now - s.started) / 60_000.0) as f32;
        s.phase = Phase::Compare;
        s.measuring = false;
    }

    /// Cada fotograma: el tiempo del estudio, si se eligió uno.
    pub(super) fn study_step(&mut self) {
        if self.screen != Screen::Study || self.study.phase != Phase::Draw {
            return;
        }
        if let Some(t) = self.study.timer {
            if self.now_ms() - self.study.started >= t as f64 * 1000.0 && !self.drawing {
                self.time_up = true;
                self.finish_study();
                self.message = Some(("¡Tiempo! Ahora compara tu dibujo con la referencia.".into(), self.now_ms()));
            }
        }
    }

    /// La referencia colocada sobre el lienzo (en px del lienzo), para superponer y para la rejilla.
    fn ref_on_canvas(&self) -> Option<Rect> {
        let r = self.study.reference.as_ref()?;
        let s = r.size();
        let fit = (W as f32 / s.x).min(H as f32 / s.y);
        let size = s * fit * self.study.ov_scale;
        Some(Rect::from_center_size(pos2(W as f32 / 2.0, H as f32 / 2.0) + self.study.ov_offset, size))
    }

    /// Rectángulo de pantalla donde se dibuja la referencia dentro de su área.
    fn ref_screen_rect(&self, area: Rect) -> Option<Rect> {
        let r = self.study.reference.as_ref()?;
        let s = r.size();
        let fit = (area.width() / s.x).min(area.height() / s.y) * self.study.zoom;
        Some(Rect::from_center_size(area.center() + self.study.pan, s * fit))
    }

    pub(super) fn study_screen(&mut self, ui: &mut Ui) {
        let ctx = ui.ctx().clone();
        let full = ui.available_width();
        let (lw, rw) = if full < 1320.0 { (200.0, 320.0) } else { (228.0, 370.0) };
        self.tools_panel(ui, lw);
        self.study_panel(ui, rw);
        egui::CentralPanel::default().frame(egui::Frame::new().fill(theme::STAGE).inner_margin(12)).show(ui, |ui| {
            if self.study.phase != Phase::Setup {
                ui.horizontal(|ui| self.view_controls(ui));
                ui.add_space(4.0);
            }
            let area = ui.available_rect_before_wrap();
            if self.study.reference.is_none() {
                let p = ui.painter_at(area);
                p.rect(area.shrink(40.0), 16, Color32::from_rgb(26, 29, 33), Stroke::new(2.0, theme::BORDER), StrokeKind::Inside);
                theme::paint_icon(&p, Rect::from_center_size(area.center() - vec2(0.0, 40.0), vec2(48.0, 48.0)), Icon::Image, theme::TEXT_3);
                p.text(area.center() + vec2(0.0, 20.0), Align2::CENTER_CENTER, "Suelta aquí una foto (PNG o JPG)", FontId::new(theme::TITLE, theme::semibold()), theme::TEXT_2);
                p.text(area.center() + vec2(0.0, 52.0), Align2::CENTER_CENTER, "Una pose, una cabeza, tu mano, un objeto de casa, un paisaje o una obra de un museo", FontId::proportional(theme::BODY), theme::TEXT_3);
                return;
            }
            if self.study.phase == Phase::Setup {
                self.ref_viewer(ui, area);
                return;
            }
            let (ref_area, canvas_area) = if self.study.hide_ref {
                (None, area)
            } else {
                let mid = area.center().x;
                (Some(Rect::from_min_max(area.min, pos2(mid - 5.0, area.max.y))), Rect::from_min_max(pos2(mid + 5.0, area.min.y), area.max))
            };
            if let Some(ra) = ref_area {
                self.ref_viewer(ui, ra);
            }
            self.study_canvas(ui, &ctx, canvas_area);
        });
    }

    /// Lienzo del estudio: dibujar, o en la comparación, superponer, alinear y medir.
    fn study_canvas(&mut self, ui: &mut Ui, ctx: &egui::Context, area: Rect) {
        self.last_area = area;
        let resp = ui.interact(area, egui::Id::unique("lienzo_estudio"), Sense::drag());
        let navigating = self.navigate_view(ui, &resp, area);
        let xf = self.view.xf(area);
        let comparing = self.study.phase == Phase::Compare;
        if self.study.measuring && !navigating {
            self.measure_drag(ui, &resp, false, |p| xf.to_canvas(p));
        } else if comparing && self.study.align && !navigating {
            if resp.dragged() {
                if let Some(p) = resp.interact_pointer_pos() {
                    let d = xf.to_canvas(p) - xf.to_canvas(p - resp.drag_delta());
                    self.study.ov_offset += d;
                }
            }
        } else {
            let allow = !navigating && self.study.phase == Phase::Draw;
            self.handle_canvas_input(ui, &resp, xf, allow);
        }
        let tex = self.canvas.texture(ctx).id();
        let painter = ui.painter_at(area);
        painter.add(xf.image(tex));
        painter.add(xf.frame(Color32::TRANSPARENT, Stroke::new(1.0, Color32::from_gray(45))));
        if comparing {
            if let (Some(r), Some(rect)) = (&self.study.reference, self.ref_on_canvas()) {
                let a = if self.study.blink { if (self.now_ms() / 500.0) as i64 % 2 == 0 { 1.0 } else { 0.0 } } else { self.study.overlay };
                if a > 0.01 {
                    let tex = if self.canvas.gray { r.gray.id() } else { r.tex.id() };
                    let mut mesh = egui::Mesh::with_texture(tex);
                    let tint = Color32::from_white_alpha((a * 255.0) as u8);
                    let corners = [rect.left_top(), rect.right_top(), rect.right_bottom(), rect.left_bottom()];
                    let uv = [pos2(0.0, 0.0), pos2(1.0, 0.0), pos2(1.0, 1.0), pos2(0.0, 1.0)];
                    for (c, t) in corners.iter().zip(uv) {
                        mesh.vertices.push(egui::epaint::Vertex { pos: xf.to_screen(*c), uv: t, color: tint });
                    }
                    mesh.add_triangle(0, 1, 2);
                    mesh.add_triangle(0, 2, 3);
                    painter.add(Shape::mesh(mesh));
                }
            }
        }
        if self.study.grid {
            if let Some(rect) = self.ref_on_canvas() {
                draw_grid(&painter, rect, |q| xf.to_screen(q));
            }
        }
        draw_measures(&painter, &self.study.meas_draw, |q| xf.to_screen(q));
        self.measure_preview(ui, &painter, false);
        if !navigating && !self.study.measuring && !(comparing && self.study.align) {
            if let Some(hp) = resp.hover_pos() {
                let d = (self.brushes[self.brush].size * self.canvas.size_mult * xf.scale).max(4.0);
                painter.circle_stroke(hp, d * 0.5, Stroke::new(1.0, Color32::from_gray(60)));
            }
        }
        if resp.hovered() {
            let icon = if navigating || (comparing && self.study.align) { egui::CursorIcon::Grabbing } else { egui::CursorIcon::Crosshair };
            ctx.set_cursor_icon(icon);
        }
    }

    /// Visor de la referencia: rueda para acercar, arrastrar para mover (o medir).
    fn ref_viewer(&mut self, ui: &mut Ui, area: Rect) {
        let resp = ui.interact(area, egui::Id::unique("visor_referencia"), Sense::drag());
        let Some(rect) = self.ref_screen_rect(area) else { return };
        let mirror = self.view.mirror;
        let size = self.study.reference.as_ref().map(|r| r.size()).unwrap_or(Vec2::splat(1.0));
        // De pantalla a px de la imagen, teniendo en cuenta el espejo de la vista.
        let to_img = move |p: Pos2| {
            let u = (p - rect.min) / rect.size();
            let x = if mirror { 1.0 - u.x } else { u.x };
            pos2(x * size.x, u.y * size.y)
        };
        if resp.hovered() {
            let (wheel, ptr) = ui.input(|i| (i.smooth_scroll_delta().y, i.pointer.hover_pos()));
            if wheel != 0.0 {
                if let Some(at) = ptr {
                    let f = (wheel * 0.0015).exp();
                    let new_zoom = (self.study.zoom * f).clamp(0.2, 12.0);
                    let real = new_zoom / self.study.zoom;
                    let c = area.center() + self.study.pan;
                    self.study.pan += (at - c) * (1.0 - real);
                    self.study.zoom = new_zoom;
                }
            }
        }
        if self.study.measuring {
            self.measure_drag(ui, &resp, true, to_img);
        } else if resp.dragged() {
            self.study.pan += resp.drag_delta();
        }
        let Some(rect) = self.ref_screen_rect(area) else { return };
        let p = ui.painter_at(area);
        p.rect_filled(area, 8, Color32::from_rgb(26, 29, 33));
        if let Some(r) = &self.study.reference {
            let tex = if self.canvas.gray { r.gray.id() } else { r.tex.id() };
            let uv = if mirror { Rect::from_min_max(pos2(1.0, 0.0), pos2(0.0, 1.0)) } else { Rect::from_min_max(pos2(0.0, 0.0), pos2(1.0, 1.0)) };
            p.image(tex, rect, uv, Color32::WHITE);
        }
        let to_screen = move |q: Pos2| {
            let x = if mirror { 1.0 - q.x / size.x } else { q.x / size.x };
            rect.min + vec2(x * rect.width(), q.y / size.y * rect.height())
        };
        if self.study.grid {
            draw_grid(&p, Rect::from_min_size(Pos2::ZERO, size), to_screen);
        }
        draw_measures(&p, &self.study.meas_ref, to_screen);
        self.measure_preview(ui, &p, true);
        p.text(area.left_top() + vec2(10.0, 8.0), Align2::LEFT_TOP, "Referencia", FontId::new(theme::CAPTION, theme::semibold()), theme::TEXT_3);
        if resp.hovered() {
            ui.ctx().set_cursor_icon(if self.study.measuring { egui::CursorIcon::Crosshair } else { egui::CursorIcon::Grab });
        }
    }

    /// Arrastrar para medir: al soltar, guarda el segmento en px de la imagen o del lienzo.
    fn measure_drag(&mut self, ui: &Ui, resp: &egui::Response, on_ref: bool, to_img: impl Fn(Pos2) -> Pos2) {
        let ptr = ui.input(|i| i.pointer.latest_pos());
        if resp.drag_started() {
            if let Some(p) = resp.interact_pointer_pos().or(ptr) {
                self.study.drag = Some((on_ref, p));
            }
        }
        if resp.drag_stopped() {
            if let (Some((side, a)), Some(b)) = (self.study.drag.take(), ptr) {
                if side == on_ref && (b - a).length() > 6.0 {
                    let seg = [to_img(a), to_img(b)];
                    if on_ref { self.study.meas_ref.push(seg) } else { self.study.meas_draw.push(seg) }
                }
            }
        }
    }

    /// Línea provisional mientras se arrastra una medida.
    fn measure_preview(&self, ui: &Ui, p: &egui::Painter, on_ref: bool) {
        if let Some((side, a)) = self.study.drag {
            if side == on_ref {
                if let Some(b) = ui.input(|i| i.pointer.latest_pos()) {
                    p.line_segment([a, b], Stroke::new(2.0, MEASURE_COL));
                }
            }
        }
    }

    fn study_panel(&mut self, ui: &mut Ui, width: f32) {
        let ctx = ui.ctx().clone();
        egui::Panel::right("estudio")
            .resizable(false)
            .exact_size(width)
            .frame(egui::Frame::new().fill(theme::SURFACE).inner_margin(16))
            .show(ui, |ui| {
                egui::ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| match self.study.phase {
                    Phase::Setup => self.study_setup_ui(ui),
                    Phase::Draw => self.study_draw_ui(ui, &ctx),
                    Phase::Compare => self.study_compare_ui(ui),
                });
            });
    }

    fn study_setup_ui(&mut self, ui: &mut Ui) {
        let ctx = ui.ctx().clone();
        ui.label(theme::strong("Estudio con referencia", theme::TITLE));
        ui.label(theme::body("Dibuja a partir de una foto real y, al terminar, compara y critica tu dibujo tú mismo. Aquí no hay nota: el objetivo es entrenar el ojo."));
        theme::section(ui, "1. La referencia");
        if self.study.reference.is_some() {
            ui.label(RichText::new("Referencia cargada. Puedes soltar otra para cambiarla.").size(theme::BODY).color(theme::GOOD));
        } else {
            ui.label(theme::caption("Arrastra una imagen a la ventana. Sirven tus propias fotos, fotos de poses o las colecciones de dominio público de museos (el Met y el Rijksmuseum, por ejemplo) para copiar maestros."));
        }
        theme::section(ui, "2. El tema");
        ui.horizontal_wrapped(|ui| {
            for t in Topic::ALL {
                let kind = if self.study.topic == t { Btn::Selected } else { Btn::Outline };
                if theme::button(ui, None, t.label(), kind).clicked() {
                    self.study.topic = t;
                }
            }
        });
        ui.label(theme::caption(format!("Las preguntas de autocrítica salen de: {}.", self.study.topic.source())));
        let n = study::library_images(self.study.topic).len();
        ui.horizontal_wrapped(|ui| {
            ui.add_enabled_ui(n > 0, |ui| {
                if theme::button(ui, Some(Icon::Image), &format!("Foto al azar ({n})"), Btn::Outline)
                    .on_hover_text("Una foto de la carpeta de este tema en tu biblioteca de referencias")
                    .clicked()
                {
                    self.random_reference(&ctx);
                }
            });
            if theme::button(ui, None, "Abrir la carpeta", Btn::Ghost)
                .on_hover_text(format!("Imágenes › TrazoAcademia › referencias › {}", self.study.topic.folder()))
                .clicked()
            {
                let _ = std::process::Command::new("explorer").arg(study::library_folder(self.study.topic)).spawn();
            }
        });
        ui.label(theme::caption("Ve guardando fotos en la carpeta de cada tema (tuyas, de poses o de museos) y tendrás tu propia biblioteca."));
        theme::section(ui, "3. El tiempo");
        ui.horizontal_wrapped(|ui| {
            let mut options: Vec<(Option<f32>, String)> = [None, Some(2.0), Some(5.0), Some(10.0), Some(20.0), Some(30.0)]
                .into_iter()
                .map(|m: Option<f32>| (m.map(|m| m * 60.0), m.map_or("Sin límite".to_string(), |m| format!("{m:.0} min"))))
                .collect();
            // El tiempo que propone el programa, si no está entre los de siempre.
            if let Some(t) = self.study.timer {
                if !options.iter().any(|(o, _)| *o == Some(t)) {
                    options.push((Some(t), format!("{:.0} min", t / 60.0)));
                }
            }
            for (t, label) in options {
                let label = label.as_str();
                let kind = if self.study.timer == t { Btn::Selected } else { Btn::Outline };
                if theme::button(ui, None, label, kind).clicked() {
                    self.study.timer = t;
                }
            }
        });
        ui.label(theme::caption("Poco tiempo obliga a buscar lo esencial (gesto, grandes masas); mucho tiempo, a afinar proporciones y valores."));
        ui.add_space(12.0);
        let w = ui.available_width();
        ui.add_enabled_ui(self.study.reference.is_some(), |ui| {
            if theme::button_ex(ui, Some(Icon::Play), "Empezar a dibujar", Btn::Primary, w, false).clicked() {
                self.start_study();
            }
        });
    }

    fn study_draw_ui(&mut self, ui: &mut Ui, ctx: &egui::Context) {
        let topic = self.study.topic;
        theme::chip(ui, Some(Icon::Copy), topic.label(), ACCENT);
        ui.label(theme::strong("Dibujando", theme::TITLE));
        if let Some(t) = self.study.timer {
            let left = (t as f64 - (self.now_ms() - self.study.started) / 1000.0).max(0.0);
            ui.horizontal(|ui| {
                theme::icon_widget(ui, Icon::Play, 16.0, GOLD);
                ui.label(theme::strong(format!("Tiempo: {}:{:02}", left as u64 / 60, left as u64 % 60), theme::LEAD).color(GOLD));
            });
            let w = ui.available_width();
            theme::progress(ui, (left / t as f64) as f32, GOLD, w, 6.0);
        }
        ui.label(theme::body("Mira la referencia más que tu dibujo. Empieza por lo grande: el gesto, la forma general y las proporciones; el detalle, al final."));
        theme::section(ui, "Referencia");
        ui.horizontal_wrapped(|ui| {
            let label = if self.study.hide_ref { "Mostrar" } else { "Ocultar" };
            if theme::button(ui, None, label, Btn::Outline).on_hover_text("Ocúltala para dibujar de memoria y vuelve a mostrarla para comprobar").clicked() {
                self.study.hide_ref = !self.study.hide_ref;
            }
            if theme::button(ui, Some(Icon::Retry), "Girar 90°", Btn::Ghost).clicked() {
                self.transform_reference(ctx, |img| image::imageops::rotate90(img));
            }
            if theme::button(ui, None, "Voltear", Btn::Ghost).on_hover_text("Voltea la foto: útil para dibujar del revés o en otra orientación").clicked() {
                self.transform_reference(ctx, |img| image::imageops::flip_horizontal(img));
            }
            if theme::button(ui, None, "Encajar", Btn::Ghost).on_hover_text("Ver la referencia entera").clicked() {
                self.study.zoom = 1.0;
                self.study.pan = Vec2::ZERO;
            }
        });
        ui.label(theme::caption("Rueda sobre la foto: acercar. Arrastrar: moverla. «Espejo» y «Grises», arriba, cambian la foto y el dibujo a la vez."));
        ui.add_space(12.0);
        let w = ui.available_width();
        if theme::button_ex(ui, Some(Icon::Check), "Terminar y comparar", Btn::Primary, w, false).clicked() {
            self.finish_study();
        }
        if theme::button_ex(ui, None, "Cambiar de referencia o de tema", Btn::Ghost, w, false).clicked() {
            self.study.phase = Phase::Setup;
        }
    }

    fn transform_reference(&mut self, ctx: &egui::Context, f: impl Fn(&image::RgbaImage) -> image::RgbaImage) {
        if let Some(r) = &self.study.reference {
            let rgba = f(&r.rgba);
            self.study.reference = Some(RefImage::new(ctx, rgba));
            self.study.meas_ref.clear();
        }
    }

    fn study_compare_ui(&mut self, ui: &mut Ui) {
        let topic = self.study.topic;
        theme::chip(ui, Some(Icon::Copy), topic.label(), ACCENT);
        ui.label(theme::strong("Compara y critica", theme::TITLE));
        ui.label(theme::caption(format!("{:.0} min de dibujo. Antes de responder, usa las herramientas: los errores se ven mejor de lejos, en espejo y en grises.", self.study.minutes.max(1.0))));

        theme::section(ui, "Comparar");
        ui.label(theme::caption("Referencia encima de tu dibujo"));
        ui.spacing_mut().slider_width = (ui.available_width() - 66.0).max(60.0);
        ui.add(egui::Slider::new(&mut self.study.overlay, 0.0..=1.0).max_decimals(2));
        ui.horizontal_wrapped(|ui| {
            let b = if self.study.blink { Btn::Selected } else { Btn::Outline };
            if theme::button(ui, None, "Alternar", b).on_hover_text("Cambia rápido entre tu dibujo y la referencia: lo que «salta» es lo que no coincide").clicked() {
                self.study.blink = !self.study.blink;
            }
            let a = if self.study.align { Btn::Selected } else { Btn::Outline };
            if theme::button(ui, None, "Alinear", a).on_hover_text("Arrastra sobre el lienzo para mover la referencia hasta que encaje con tu dibujo").clicked() {
                self.study.align = !self.study.align;
                self.study.measuring = false;
                if self.study.align && self.study.overlay < 0.2 {
                    self.study.overlay = 0.5;
                }
            }
            let g = if self.study.grid { Btn::Selected } else { Btn::Outline };
            if theme::button(ui, None, "Rejilla", g).on_hover_text("La misma rejilla de 4 × 4 sobre la referencia y sobre su sitio en tu lienzo").clicked() {
                self.study.grid = !self.study.grid;
            }
        });
        if self.study.align {
            ui.label(theme::caption("Tamaño de la referencia"));
            ui.add(egui::Slider::new(&mut self.study.ov_scale, 0.2..=3.0).logarithmic(true).max_decimals(2));
        }
        ui.label(theme::caption("«Espejo» y «Grises», encima del lienzo, voltean y pasan a grises la foto y el dibujo a la vez."));

        theme::section(ui, "Medir proporciones");
        let m = if self.study.measuring { Btn::Selected } else { Btn::Outline };
        ui.horizontal_wrapped(|ui| {
            if theme::button(ui, None, "Medir", m).clicked() {
                self.study.measuring = !self.study.measuring;
                self.study.align = false;
            }
            if (!self.study.meas_ref.is_empty() || !self.study.meas_draw.is_empty()) && theme::button(ui, None, "Borrar medidas", Btn::Ghost).clicked() {
                self.study.meas_ref.clear();
                self.study.meas_draw.clear();
            }
        });
        ui.label(theme::caption("Como enseña Betty Edwards: arrastra la primera medida (por ejemplo, el alto de la cabeza) en la foto y la misma en tu dibujo; es la unidad. Después mide otras partes en el mismo orden."));
        measure_table(ui, &self.study.meas_ref, &self.study.meas_draw);

        theme::section(ui, "Autocrítica");
        ui.label(theme::caption(format!("Según {}.", topic.source())));
        for (i, q) in topic.questions().iter().enumerate() {
            ui.label(RichText::new(*q).size(theme::BODY).color(theme::TEXT));
            ui.horizontal(|ui| {
                for a in [Answer::Yes, Answer::Partly, Answer::No] {
                    let sel = self.study.answers.get(i).copied().flatten() == Some(a);
                    if theme::button(ui, None, a.label(), if sel { Btn::Selected } else { Btn::Outline }).clicked() {
                        if let Some(slot) = self.study.answers.get_mut(i) {
                            *slot = Some(a);
                        }
                    }
                }
            });
            ui.add_space(4.0);
        }
        ui.label(RichText::new("Lo mejor de este estudio").size(theme::BODY).color(theme::TEXT));
        ui.add(egui::TextEdit::multiline(&mut self.study.best).desired_rows(2).desired_width(f32::INFINITY));
        ui.label(RichText::new("Qué voy a mejorar en el siguiente").size(theme::BODY).color(theme::TEXT));
        ui.add(egui::TextEdit::multiline(&mut self.study.improve).desired_rows(2).desired_width(f32::INFINITY));
        ui.add_space(10.0);
        let w = ui.available_width();
        if self.study.saved {
            ui.label(RichText::new("Guardado en el diario.").size(theme::BODY).color(theme::GOOD));
            if self.from_program && theme::button_ex(ui, Some(Icon::Play), "Volver a Hoy", Btn::Primary, w, false).clicked() {
                self.screen = Screen::Program;
            }
        } else if theme::button_ex(ui, Some(Icon::Save), "Guardar en el diario", Btn::Primary, w, false).clicked() {
            self.save_study();
        }
        ui.horizontal_wrapped(|ui| {
            if theme::button(ui, None, "Seguir dibujando", Btn::Ghost).clicked() {
                self.study.phase = Phase::Draw;
                self.time_up = false;
                self.study.overlay = 0.0;
                self.study.blink = false;
            }
            if theme::button(ui, Some(Icon::Play), "Nuevo estudio", Btn::Outline).clicked() {
                self.study.phase = Phase::Setup;
            }
        });
        if let Some((msg, t)) = &self.message {
            if self.now_ms() - t < 6000.0 {
                ui.label(RichText::new(msg).size(theme::BODY).color(GOLD));
            }
        }
    }

    fn save_study(&mut self) {
        let Some(r) = &self.study.reference else { return };
        let when = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0, |d| d.as_secs());
        let entry = Entry {
            when,
            topic: self.study.topic,
            minutes: self.study.minutes,
            answers: self.study.answers.clone(),
            best: self.study.best.trim().to_string(),
            improve: self.study.improve.trim().to_string(),
        };
        let now = self.now_ms();
        match study::save(&entry, &self.canvas.to_rgba(), [W as u32, H as u32], &r.rgba) {
            Ok(_) => {
                self.study.saved = true;
                self.progress.program.record_study(&crate::program::today());
                self.save();
                self.diary.clear();
                self.message = Some(("Estudio guardado en el diario.".into(), now));
            }
            Err(e) => self.message = Some((format!("No se pudo guardar el estudio: {e}"), now)),
        }
    }

    pub(super) fn diary_screen(&mut self, ui: &mut Ui) {
        let ctx = ui.ctx().clone();
        if self.diary.is_empty() && !self.diary_loaded {
            self.diary = study::load_all()
                .into_iter()
                .map(|(path, entry)| DiaryItem { path, entry, ref_tex: None, draw_tex: None, failed: false })
                .collect();
            self.diary_loaded = true;
        }
        // Miniaturas poco a poco.
        for it in self.diary.iter_mut().filter(|d| d.draw_tex.is_none() && !d.failed).take(2) {
            let load = |name: &str| image::open(it.path.join(name)).ok().map(|i| i.thumbnail(300, 200).to_rgba8());
            match (load("referencia.png"), load("dibujo.png")) {
                (Some(r), Some(d)) => {
                    it.ref_tex = Some(ctx.load_texture(format!("{}r", it.path.display()), color_image(&r), TextureOptions::LINEAR));
                    it.draw_tex = Some(ctx.load_texture(format!("{}d", it.path.display()), color_image(&d), TextureOptions::LINEAR));
                }
                _ => it.failed = true,
            }
        }
        let mut go_study = false;
        egui::CentralPanel::default().frame(egui::Frame::new().fill(theme::BG).inner_margin(24)).show(ui, |ui| {
            egui::ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
                centered(ui, 1180.0, |ui| {
                    ui.label(theme::strong("Diario de estudios", theme::TITLE + 4.0));
                    ui.label(theme::body("Cada estudio con su referencia al lado, tu autocrítica y lo que querías mejorar. Repásalo cada semana: el progreso se ve comparando, no recordando."));
                    let total = self.diary.len();
                    let minutes = self.diary.iter().map(|d| d.entry.minutes.max(0.0).round() as u32).sum::<u32>();
                    ui.horizontal(|ui| {
                        ui.label(theme::caption(format!("{total} estudios · {} h {:02} min dibujando", minutes / 60, minutes % 60)));
                        if theme::button(ui, Some(Icon::Retry), "Actualizar", Btn::Ghost).clicked() {
                            self.diary.clear();
                            self.diary_loaded = false;
                        }
                        if theme::button(ui, None, "Abrir la carpeta", Btn::Ghost).clicked() {
                            let dir = study::diary_dir();
                            let _ = std::fs::create_dir_all(&dir);
                            let _ = std::process::Command::new("explorer").arg(dir).spawn();
                        }
                    });
                    ui.horizontal_wrapped(|ui| {
                        let all = if self.diary_filter.is_none() { Btn::Selected } else { Btn::Outline };
                        if theme::button(ui, None, "Todos", all).clicked() {
                            self.diary_filter = None;
                        }
                        for t in Topic::ALL {
                            let n = self.diary.iter().filter(|d| d.entry.topic == t).count();
                            if n == 0 {
                                continue;
                            }
                            let k = if self.diary_filter == Some(t) { Btn::Selected } else { Btn::Outline };
                            if theme::button(ui, None, &format!("{} ({n})", t.label()), k).clicked() {
                                self.diary_filter = Some(t);
                            }
                        }
                    });
                    ui.add_space(10.0);
                    if self.diary.is_empty() {
                        theme::card(theme::RAISED).show(ui, |ui| {
                            ui.set_width(ui.available_width());
                            ui.label(theme::strong("Aún no hay estudios", theme::LEAD));
                            ui.label(theme::body("Haz un estudio con referencia y guárdalo al terminar la autocrítica: aparecerá aquí."));
                            if theme::button(ui, Some(Icon::Play), "Hacer un estudio", Btn::Primary).clicked() {
                                go_study = true;
                            }
                        });
                        return;
                    }
                    for it in self.diary.iter().filter(|d| self.diary_filter.is_none_or(|t| d.entry.topic == t)) {
                        diary_card(ui, it);
                        ui.add_space(10.0);
                    }
                });
            });
        });
        if go_study {
            self.open_study();
        }
    }
}

fn diary_card(ui: &mut Ui, it: &DiaryItem) {
    let e = &it.entry;
    theme::card(theme::RAISED).inner_margin(14).show(ui, |ui| {
        ui.set_width(ui.available_width());
        ui.horizontal_top(|ui| {
            for tex in [&it.ref_tex, &it.draw_tex] {
                match tex {
                    Some(t) => {
                        let [w, h] = t.size();
                        let s = (300.0 / w as f32).min(200.0 / h as f32);
                        let (r, _) = ui.allocate_exact_size(vec2(300.0, 200.0), Sense::hover());
                        ui.painter().rect_filled(r, 6, Color32::from_rgb(26, 29, 33));
                        ui.painter().image(t.id(), Rect::from_center_size(r.center(), vec2(w as f32, h as f32) * s), Rect::from_min_max(pos2(0.0, 0.0), pos2(1.0, 1.0)), Color32::WHITE);
                    }
                    None => {
                        ui.allocate_exact_size(vec2(300.0, 200.0), Sense::hover());
                    }
                }
            }
            ui.vertical(|ui| {
                ui.horizontal(|ui| {
                    theme::chip(ui, None, e.topic.label(), ACCENT);
                    ui.label(theme::caption(format!("{} · {:.0} min", study::date_label(e.when), e.minutes.max(1.0))));
                });
                let (yes, answered) = e.score();
                ui.label(theme::strong(format!("{yes} «sí» de {answered} respondidas en la autocrítica"), theme::BODY));
                for (q, a) in e.topic.questions().iter().zip(&e.answers) {
                    let (txt, col) = match a {
                        Some(Answer::Yes) => ("Sí", theme::GOOD),
                        Some(Answer::Partly) => ("A medias", GOLD),
                        Some(Answer::No) => ("No", theme::BAD),
                        None => ("—", theme::TEXT_3),
                    };
                    ui.horizontal_wrapped(|ui| {
                        ui.label(RichText::new(txt).size(theme::CAPTION).color(col).family(theme::semibold()));
                        ui.label(RichText::new(*q).size(theme::CAPTION).color(theme::TEXT_2));
                    });
                }
                if !e.best.is_empty() {
                    ui.label(RichText::new(format!("Lo mejor: {}", e.best)).size(theme::BODY).color(theme::TEXT));
                }
                if !e.improve.is_empty() {
                    ui.label(RichText::new(format!("A mejorar: {}", e.improve)).size(theme::BODY).color(theme::TEXT));
                }
            });
        });
    });
}

/// Rejilla de 4 × 4 sobre un rectángulo (en las coordenadas que convierte `to`).
fn draw_grid(p: &egui::Painter, r: Rect, to: impl Fn(Pos2) -> Pos2) {
    let col = Stroke::new(1.2, Color32::from_rgba_unmultiplied(70, 150, 230, 150));
    for k in 0..=4 {
        let t = k as f32 / 4.0;
        let x = r.min.x + r.width() * t;
        let y = r.min.y + r.height() * t;
        p.line_segment([to(pos2(x, r.min.y)), to(pos2(x, r.max.y))], col);
        p.line_segment([to(pos2(r.min.x, y)), to(pos2(r.max.x, y))], col);
    }
}

/// Medidas dibujadas con su número; la primera es la unidad.
fn draw_measures(p: &egui::Painter, segs: &[[Pos2; 2]], to: impl Fn(Pos2) -> Pos2) {
    let unit = segs.first().map(|s| (s[1] - s[0]).length());
    for (i, s) in segs.iter().enumerate() {
        let (a, b) = (to(s[0]), to(s[1]));
        p.line_segment([a, b], Stroke::new(2.5, MEASURE_COL));
        for q in [a, b] {
            p.circle_filled(q, 3.5, MEASURE_COL);
        }
        let label = match (i, unit) {
            (0, _) => "1 = unidad".to_string(),
            (_, Some(u)) if u > 0.0 => format!("{}: {:.2}", i + 1, (s[1] - s[0]).length() / u),
            _ => String::new(),
        };
        let m = a + (b - a) * 0.5;
        // Fondo oscuro para que el número se lea sobre cualquier foto.
        let font = FontId::new(13.0, theme::semibold());
        let galley = p.layout_no_wrap(label, font, MEASURE_COL);
        let r = Align2::LEFT_BOTTOM.anchor_size(m + vec2(6.0, -6.0), galley.size()).expand(3.0);
        p.rect_filled(r, 4, Color32::from_black_alpha(190));
        p.galley(r.min + vec2(3.0, 3.0), galley, MEASURE_COL);
    }
}

/// Tabla de medidas: cada medida en unidades, en la foto y en el dibujo, y la diferencia.
fn measure_table(ui: &mut Ui, r: &[[Pos2; 2]], d: &[[Pos2; 2]]) {
    let len = |s: &[Pos2; 2]| (s[1] - s[0]).length();
    let (Some(ur), Some(ud)) = (r.first().map(len), d.first().map(len)) else {
        if !r.is_empty() || !d.is_empty() {
            ui.label(theme::caption("Falta la unidad en uno de los dos lados."));
        }
        return;
    };
    let n = r.len().max(d.len());
    egui::Grid::new("medidas").num_columns(4).spacing([12.0, 4.0]).show(ui, |ui| {
        for h in ["Medida", "Foto", "Dibujo", "Diferencia"] {
            ui.label(theme::caption(h));
        }
        ui.end_row();
        for i in 1..n {
            ui.label(RichText::new(format!("{}", i + 1)).size(theme::CAPTION).color(theme::TEXT));
            let fr = r.get(i).map(|s| len(s) / ur);
            let fd = d.get(i).map(|s| len(s) / ud);
            ui.label(RichText::new(fr.map_or("—".into(), |v| format!("{v:.2}"))).size(theme::CAPTION).color(theme::TEXT));
            ui.label(RichText::new(fd.map_or("—".into(), |v| format!("{v:.2}"))).size(theme::CAPTION).color(theme::TEXT));
            match (fr, fd) {
                (Some(a), Some(b)) if a > 0.0 => {
                    let diff = (b / a - 1.0) * 100.0;
                    let col = if diff.abs() <= 5.0 { theme::GOOD } else if diff.abs() <= 12.0 { GOLD } else { theme::BAD };
                    ui.label(RichText::new(format!("{diff:+.0} %")).size(theme::CAPTION).color(col).family(theme::semibold()));
                }
                _ => {
                    ui.label(theme::caption("—"));
                }
            }
            ui.end_row();
        }
    });
}
