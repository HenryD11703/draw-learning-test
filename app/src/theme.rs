//! Sistema visual: colores, tamaños de texto, fuentes, estilo de egui y piezas reutilizables
//! (botones, chips, barras de progreso, estrellas e iconos dibujados).

use egui::text::LayoutJob;
use egui::{
    Color32, FontData, FontDefinitions, FontFamily, FontId, Painter, Pos2, Rect, Response, RichText, Sense, Shape, Stroke, StrokeKind,
    TextStyle, Ui, pos2, vec2,
};
use std::sync::Arc;

// ------------------------------------------------------------------ colores

/// Fondo de la app.
pub const BG: Color32 = Color32::from_rgb(23, 25, 28);
/// Paneles laterales.
pub const SURFACE: Color32 = Color32::from_rgb(31, 34, 38);
/// Tarjetas.
pub const RAISED: Color32 = Color32::from_rgb(40, 44, 49);
pub const HOVER: Color32 = Color32::from_rgb(52, 57, 63);
pub const BORDER: Color32 = Color32::from_rgb(58, 63, 70);
pub const TEXT: Color32 = Color32::from_rgb(234, 236, 239);
pub const TEXT_2: Color32 = Color32::from_rgb(182, 188, 196);
pub const TEXT_3: Color32 = Color32::from_rgb(132, 139, 148);
pub const ACCENT: Color32 = Color32::from_rgb(61, 174, 233);
pub const ON_ACCENT: Color32 = Color32::from_rgb(8, 24, 34);
pub const GOLD: Color32 = Color32::from_rgb(255, 200, 60);
pub const GOOD: Color32 = Color32::from_rgb(110, 210, 120);
pub const BAD: Color32 = Color32::from_rgb(235, 100, 90);
/// Fondo detrás del lienzo.
pub const STAGE: Color32 = Color32::from_rgb(18, 20, 23);
pub const PAPER: Color32 = Color32::from_rgb(248, 247, 242);

// ------------------------------------------------------------------ texto

pub const CAPTION: f32 = 13.0;
pub const BODY: f32 = 15.0;
pub const LEAD: f32 = 17.0;
pub const TITLE: f32 = 22.0;
pub const DISPLAY: f32 = 30.0;

pub fn semibold() -> FontFamily {
    FontFamily::Name("semibold".into())
}

/// Texto en seminegrita, en el color principal.
pub fn strong(text: impl Into<String>, size: f32) -> RichText {
    RichText::new(text).family(semibold()).size(size).color(TEXT)
}

/// Texto de lectura.
pub fn body(text: impl Into<String>) -> RichText {
    RichText::new(text).size(BODY).color(TEXT_2)
}

/// Texto secundario pequeño.
pub fn caption(text: impl Into<String>) -> RichText {
    RichText::new(text).size(CAPTION).color(TEXT_3)
}

/// Rótulo de sección en mayúsculas.
pub fn section(ui: &mut Ui, title: &str) {
    ui.add_space(6.0);
    ui.label(RichText::new(title.to_uppercase()).family(semibold()).size(12.0).color(TEXT_3));
    ui.add_space(2.0);
}

/// Tarjeta estándar.
pub fn card(fill: Color32) -> egui::Frame {
    egui::Frame::new().fill(fill).stroke(Stroke::new(1.0, BORDER)).corner_radius(12).inner_margin(16)
}

// ------------------------------------------------------------------ instalación

pub fn install(ctx: &egui::Context) {
    install_fonts(ctx);
    let mut v = egui::Visuals::dark();
    v.panel_fill = SURFACE;
    v.window_fill = RAISED;
    v.window_stroke = Stroke::new(1.0, BORDER);
    v.extreme_bg_color = BG;
    v.faint_bg_color = RAISED;
    v.hyperlink_color = ACCENT;
    v.selection.bg_fill = ACCENT.gamma_multiply(0.45);
    v.selection.stroke = Stroke::new(1.0, TEXT);
    v.widgets.noninteractive.fg_stroke = Stroke::new(1.0, TEXT_2);
    v.widgets.noninteractive.bg_stroke = Stroke::new(1.0, BORDER);
    for w in [&mut v.widgets.inactive, &mut v.widgets.hovered, &mut v.widgets.active, &mut v.widgets.open] {
        w.corner_radius = 8.into();
    }
    v.widgets.inactive.weak_bg_fill = RAISED;
    v.widgets.inactive.bg_fill = HOVER;
    v.widgets.inactive.fg_stroke = Stroke::new(1.0, TEXT);
    v.widgets.hovered.weak_bg_fill = HOVER;
    v.widgets.hovered.bg_fill = HOVER;
    v.widgets.hovered.fg_stroke = Stroke::new(1.5, TEXT);
    v.widgets.active.weak_bg_fill = ACCENT.gamma_multiply(0.7);
    v.widgets.active.bg_fill = ACCENT;
    ctx.set_visuals(v);
    ctx.global_style_mut(|s| {
        s.text_styles = [
            (TextStyle::Small, FontId::proportional(CAPTION)),
            (TextStyle::Body, FontId::proportional(BODY)),
            (TextStyle::Button, FontId::proportional(BODY)),
            (TextStyle::Heading, FontId::new(TITLE, semibold())),
            (TextStyle::Monospace, FontId::monospace(14.0)),
        ]
        .into();
        s.spacing.item_spacing = vec2(8.0, 7.0);
        s.spacing.button_padding = vec2(12.0, 6.0);
        s.spacing.interact_size.y = 28.0;
        s.spacing.slider_width = 120.0;
    });
}

/// Usa Segoe UI de Windows (tiene todos los acentos, comillas y flechas). Si no está, se quedan las
/// fuentes de egui y la seminegrita usa la misma.
fn install_fonts(ctx: &egui::Context) {
    let dir = std::env::var_os("WINDIR").map_or_else(|| std::path::PathBuf::from(r"C:\Windows"), std::path::PathBuf::from).join("Fonts");
    let read = |f: &str| std::fs::read(dir.join(f)).ok();
    let mut fonts = FontDefinitions::default();
    let mut fallback = fonts.families.get(&FontFamily::Proportional).cloned().unwrap_or_default();
    match (read("segoeui.ttf"), read("seguisb.ttf")) {
        (Some(regular), Some(semi)) => {
            fonts.font_data.insert("segoe".into(), Arc::new(FontData::from_owned(regular)));
            fonts.font_data.insert("segoe_sb".into(), Arc::new(FontData::from_owned(semi)));
            if let Some(sym) = read("seguisym.ttf") {
                fonts.font_data.insert("segoe_sym".into(), Arc::new(FontData::from_owned(sym)));
                fallback.insert(0, "segoe_sym".into());
            }
            let mut prop = vec!["segoe".to_string()];
            prop.extend(fallback.iter().cloned());
            let mut sb = vec!["segoe_sb".to_string(), "segoe".to_string()];
            sb.extend(fallback);
            fonts.families.insert(FontFamily::Proportional, prop);
            fonts.families.insert(semibold(), sb);
        }
        _ => {
            fonts.families.insert(semibold(), fallback);
        }
    }
    ctx.set_fonts(fonts);
}

// ------------------------------------------------------------------ botones

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Btn {
    /// Acción principal: relleno de acento.
    Primary,
    /// Acción secundaria con borde.
    Outline,
    /// Sin fondo hasta pasar el ratón.
    Ghost,
    /// Pestaña o elemento elegido.
    Selected,
}

/// Botón con icono opcional. `width` 0 = ajustado al texto; con `left` el contenido va a la izquierda.
pub fn button_ex(ui: &mut Ui, icon: Option<Icon>, text: &str, kind: Btn, width: f32, left: bool) -> Response {
    let primary = kind == Btn::Primary;
    let font = if primary { FontId::new(LEAD, semibold()) } else { FontId::proportional(BODY) };
    let galley = ui.painter().layout_no_wrap(text.to_owned(), font, Color32::PLACEHOLDER);
    let (pad_x, h) = if primary { (16.0, 42.0) } else { (12.0, 32.0) };
    let icon_w = if icon.is_some() { 18.0 } else { 0.0 };
    let gap = if icon.is_some() && !text.is_empty() { 8.0 } else { 0.0 };
    let content = icon_w + gap + galley.size().x;
    let w = if width > 0.0 { width } else { content + pad_x * 2.0 };
    let (rect, resp) = ui.allocate_exact_size(vec2(w, h), Sense::click());
    let enabled = ui.is_enabled();
    let hovered = resp.hovered() && enabled;
    let (fill, fg, stroke) = match kind {
        Btn::Primary => (if hovered { Color32::from_rgb(98, 192, 241) } else { ACCENT }, ON_ACCENT, Stroke::NONE),
        Btn::Outline => (if hovered { HOVER } else { RAISED }, TEXT, Stroke::new(1.0, BORDER)),
        Btn::Ghost => (if hovered { HOVER } else { Color32::TRANSPARENT }, if hovered { TEXT } else { TEXT_2 }, Stroke::NONE),
        Btn::Selected => (ACCENT.gamma_multiply(0.2), ACCENT, Stroke::NONE),
    };
    let fg = if enabled { fg } else { fg.gamma_multiply(0.4) };
    let p = ui.painter();
    p.rect(rect, 8, if enabled { fill } else { fill.gamma_multiply(0.5) }, stroke, StrokeKind::Inside);
    let x0 = if left { rect.min.x + pad_x } else { rect.center().x - content / 2.0 };
    if let Some(i) = icon {
        paint_icon(p, Rect::from_center_size(pos2(x0 + icon_w / 2.0, rect.center().y), vec2(icon_w, icon_w)), i, fg);
    }
    p.galley(pos2(x0 + icon_w + gap, rect.center().y - galley.size().y / 2.0), galley, fg);
    if hovered {
        ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
    }
    resp
}

pub fn button(ui: &mut Ui, icon: Option<Icon>, text: &str, kind: Btn) -> Response {
    button_ex(ui, icon, text, kind, 0.0, false)
}

/// Etiqueta pequeña con color.
pub fn chip(ui: &mut Ui, icon: Option<Icon>, text: &str, color: Color32) -> Response {
    let galley = ui.painter().layout_no_wrap(text.to_owned(), FontId::new(CAPTION, semibold()), color);
    let icon_w = if icon.is_some() { 14.0 } else { 0.0 };
    let gap = if icon.is_some() { 6.0 } else { 0.0 };
    let size = vec2(icon_w + gap + galley.size().x + 20.0, 24.0);
    let (rect, resp) = ui.allocate_exact_size(size, Sense::hover());
    let p = ui.painter();
    p.rect_filled(rect, 12, color.gamma_multiply(0.16));
    if let Some(i) = icon {
        paint_icon(p, Rect::from_center_size(pos2(rect.min.x + 10.0 + icon_w / 2.0, rect.center().y), vec2(icon_w, icon_w)), i, color);
    }
    p.galley(pos2(rect.min.x + 10.0 + icon_w + gap, rect.center().y - galley.size().y / 2.0), galley, color);
    resp
}

/// Barra de progreso.
pub fn progress(ui: &mut Ui, frac: f32, color: Color32, width: f32, height: f32) {
    let (r, _) = ui.allocate_exact_size(vec2(width, height), Sense::hover());
    paint_progress(ui.painter(), r, frac, color);
}

pub fn paint_progress(p: &Painter, r: Rect, frac: f32, color: Color32) {
    let radius = (r.height() / 2.0) as u8;
    p.rect_filled(r, radius, Color32::from_rgb(20, 22, 25));
    if frac > 0.0 {
        let mut f = r;
        f.set_width((r.width() * frac.clamp(0.0, 1.0)).max(r.height()));
        p.rect_filled(f, radius, color);
    }
}

/// Texto envuelto en `width` px, con como mucho `rows` líneas (las que sobran terminan en «…»).
pub fn layout_wrapped(p: &Painter, text: &str, font: FontId, color: Color32, width: f32, rows: usize) -> Arc<egui::Galley> {
    let mut job = LayoutJob::simple(text.to_owned(), font, color, width);
    job.wrap.max_rows = rows;
    p.layout_job(job)
}

// ------------------------------------------------------------------ estrellas

pub fn star(p: &Painter, c: Pos2, r: f32, color: Color32) {
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

/// Tres estrellas centradas en `c`, las `n` primeras encendidas.
pub fn stars(p: &Painter, c: Pos2, r: f32, n: u8) {
    let gap = r * 2.5;
    for i in 0..3u8 {
        star(p, pos2(c.x + (i as f32 - 1.0) * gap, c.y), r, if i < n { GOLD } else { Color32::from_gray(70) });
    }
}

// ------------------------------------------------------------------ iconos

/// Iconos dibujados con trazos, en una caja cuadrada.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Icon {
    Home,
    Map,
    Brush,
    Pressure,
    Book,
    Trace,
    Points,
    Copy,
    Perspective,
    Hatch,
    Image,
    Paint,
    Undo,
    Trash,
    Save,
    Lock,
    Play,
    Retry,
    Prev,
    Next,
    Check,
    Star,
}

pub fn icon_widget(ui: &mut Ui, icon: Icon, size: f32, color: Color32) -> Response {
    let (r, resp) = ui.allocate_exact_size(vec2(size, size), Sense::hover());
    paint_icon(ui.painter(), r, icon, color);
    resp
}

pub fn paint_icon(p: &Painter, r: Rect, icon: Icon, color: Color32) {
    let k = r.width() / 18.0;
    let s = Stroke::new(1.7 * k, color);
    // Coordenadas relativas a la caja (0..1).
    let at = |x: f32, y: f32| pos2(r.min.x + x * r.width(), r.min.y + y * r.height());
    let line = |pts: &[(f32, f32)]| {
        p.add(Shape::line(pts.iter().map(|&(x, y)| at(x, y)).collect(), s));
    };
    match icon {
        Icon::Home => {
            line(&[(0.12, 0.5), (0.5, 0.14), (0.88, 0.5)]);
            line(&[(0.24, 0.42), (0.24, 0.86), (0.76, 0.86), (0.76, 0.42)]);
            line(&[(0.42, 0.86), (0.42, 0.62), (0.58, 0.62), (0.58, 0.86)]);
        }
        Icon::Map => {
            line(&[(0.2, 0.78), (0.5, 0.42), (0.8, 0.66)]);
            for (x, y) in [(0.2, 0.78), (0.5, 0.42), (0.8, 0.66)] {
                p.circle_filled(at(x, y), 2.6 * k, color);
            }
            line(&[(0.5, 0.42), (0.5, 0.14)]);
            line(&[(0.5, 0.14), (0.78, 0.22), (0.5, 0.3)]);
        }
        Icon::Brush => {
            line(&[(0.22, 0.86), (0.36, 0.64), (0.82, 0.18), (0.86, 0.22), (0.4, 0.68), (0.22, 0.86)]);
            line(&[(0.66, 0.34), (0.7, 0.38)]);
        }
        Icon::Pressure => {
            for (i, h) in [0.3f32, 0.5, 0.72].iter().enumerate() {
                let x = 0.24 + i as f32 * 0.26;
                p.line_segment([at(x, 0.84), at(x, 0.84 - h)], Stroke::new(3.2 * k, color));
            }
        }
        Icon::Book => {
            line(&[(0.5, 0.26), (0.5, 0.84)]);
            line(&[(0.5, 0.26), (0.3, 0.18), (0.1, 0.22), (0.1, 0.78), (0.3, 0.74), (0.5, 0.84), (0.7, 0.74), (0.9, 0.78), (0.9, 0.22), (0.7, 0.18), (0.5, 0.26)]);
        }
        Icon::Trace => {
            let pts: Vec<Pos2> = (0..=24).map(|i| {
                let t = i as f32 / 24.0;
                at(0.1 + t * 0.8, 0.5 - 0.22 * (t * std::f32::consts::TAU).sin())
            }).collect();
            p.add(Shape::dashed_line(&pts, s, 3.0 * k, 2.2 * k));
        }
        Icon::Points => {
            p.add(Shape::dashed_line(&[at(0.22, 0.78), at(0.78, 0.22)], s, 2.5 * k, 2.2 * k));
            p.circle_filled(at(0.22, 0.78), 3.0 * k, color);
            p.circle_filled(at(0.78, 0.22), 3.0 * k, color);
        }
        Icon::Copy => {
            line(&[(0.14, 0.3), (0.62, 0.3), (0.62, 0.86), (0.14, 0.86), (0.14, 0.3)]);
            line(&[(0.38, 0.3), (0.38, 0.14), (0.86, 0.14), (0.86, 0.7), (0.62, 0.7)]);
        }
        Icon::Perspective => {
            line(&[(0.06, 0.3), (0.94, 0.3)]);
            line(&[(0.5, 0.3), (0.12, 0.88)]);
            line(&[(0.5, 0.3), (0.88, 0.88)]);
            line(&[(0.5, 0.3), (0.5, 0.88)]);
        }
        Icon::Hatch => {
            line(&[(0.14, 0.86), (0.86, 0.14), (0.86, 0.86), (0.14, 0.86), (0.14, 0.14), (0.86, 0.14)]);
            line(&[(0.14, 0.5), (0.5, 0.14)]);
            line(&[(0.5, 0.86), (0.86, 0.5)]);
        }
        Icon::Image => {
            line(&[(0.1, 0.2), (0.9, 0.2), (0.9, 0.8), (0.1, 0.8), (0.1, 0.2)]);
            line(&[(0.16, 0.74), (0.4, 0.46), (0.56, 0.62), (0.68, 0.5), (0.84, 0.74)]);
            p.circle_filled(at(0.68, 0.34), 2.2 * k, color);
        }
        Icon::Paint => {
            line(&[(0.5, 0.12), (0.76, 0.52), (0.72, 0.74), (0.5, 0.86), (0.28, 0.74), (0.24, 0.52), (0.5, 0.12)]);
            p.add(Shape::convex_polygon(vec![at(0.27, 0.6), at(0.73, 0.6), at(0.71, 0.74), at(0.5, 0.84), at(0.29, 0.74)], color, Stroke::NONE));
        }
        Icon::Undo | Icon::Retry => {
            let flip = icon == Icon::Retry;
            let pts: Vec<Pos2> = (0..=16)
                .map(|i| {
                    let a = std::f32::consts::PI * (1.1 + 1.3 * i as f32 / 16.0);
                    let x = 0.52 + 0.32 * a.cos();
                    at(if flip { 1.0 - x } else { x }, 0.56 - 0.32 * a.sin())
                })
                .collect();
            let start = pts[0];
            p.add(Shape::line(pts, s));
            let d = if flip { 1.0 } else { -1.0 };
            p.line_segment([start, start + vec2(0.0, -5.5 * k)], s);
            p.line_segment([start, start + vec2(-5.5 * k * d, 0.0)], s);
        }
        Icon::Trash => {
            line(&[(0.16, 0.26), (0.84, 0.26)]);
            line(&[(0.38, 0.26), (0.4, 0.14), (0.6, 0.14), (0.62, 0.26)]);
            line(&[(0.24, 0.26), (0.3, 0.88), (0.7, 0.88), (0.76, 0.26)]);
            line(&[(0.44, 0.42), (0.44, 0.74)]);
            line(&[(0.56, 0.42), (0.56, 0.74)]);
        }
        Icon::Save => {
            line(&[(0.5, 0.12), (0.5, 0.6)]);
            line(&[(0.3, 0.42), (0.5, 0.62), (0.7, 0.42)]);
            line(&[(0.14, 0.62), (0.14, 0.86), (0.86, 0.86), (0.86, 0.62)]);
        }
        Icon::Lock => {
            p.rect_filled(Rect::from_min_max(at(0.22, 0.46), at(0.78, 0.88)), (2.0 * k) as u8, color);
            line(&[(0.34, 0.46), (0.34, 0.32)]);
            line(&[(0.66, 0.46), (0.66, 0.32)]);
            p.add(Shape::line(
                (0..=12).map(|i| {
                    let a = std::f32::consts::PI * i as f32 / 12.0;
                    at(0.5 + 0.16 * a.cos(), 0.32 - 0.16 * a.sin())
                }).collect(),
                s,
            ));
        }
        Icon::Play => {
            p.add(Shape::convex_polygon(vec![at(0.28, 0.16), at(0.84, 0.5), at(0.28, 0.84)], color, Stroke::NONE));
        }
        Icon::Prev => line(&[(0.64, 0.16), (0.3, 0.5), (0.64, 0.84)]),
        Icon::Next => line(&[(0.36, 0.16), (0.7, 0.5), (0.36, 0.84)]),
        Icon::Check => line(&[(0.14, 0.52), (0.4, 0.78), (0.86, 0.24)]),
        Icon::Star => star(p, r.center(), r.width() * 0.48, color),
    }
}
