//! Calificación de los ejercicios (diseño propio, MECHANICS.md §4).

use crate::canvas::{Canvas, H, StrokePoint, W, luminance};
use crate::lessons::{CELL, Guide, PaintTarget};
use egui::Pos2;

#[derive(Clone, Debug)]
pub struct Metric {
    pub name: &'static str,
    pub value: f32,
    pub help: &'static str,
}

#[derive(Clone, Debug)]
pub struct Grade {
    pub total: f32,
    pub metrics: Vec<Metric>,
    pub stars: u8,
    pub advice: String,
    #[allow(dead_code)]
    pub assist_penalty: f32,
}

pub fn stars_for(total: f32) -> u8 {
    if total >= 90.0 {
        3
    } else if total >= 75.0 {
        2
    } else if total >= 60.0 {
        1
    } else {
        0
    }
}

fn dist_to_segment(p: Pos2, a: Pos2, b: Pos2) -> (f32, f32) {
    let ab = b - a;
    let l2 = ab.length_sq();
    if l2 <= 1e-6 {
        return ((p - a).length(), 0.0);
    }
    let t = ((p - a).dot(ab) / l2).clamp(0.0, 1.0);
    ((p - (a + ab * t)).length(), t)
}

/// Distancia de un punto a la guía más cercana, y la presión objetivo en ese punto.
fn nearest(p: Pos2, guides: &[Guide]) -> (f32, Option<f32>) {
    let mut best = (f32::MAX, None);
    for g in guides {
        for (i, w) in g.pts.windows(2).enumerate() {
            let (d, t) = dist_to_segment(p, w[0], w[1]);
            if d < best.0 {
                let pr = g.pressure.as_ref().map(|pp| pp[i] + (pp[i + 1] - pp[i]) * t);
                best = (d, pr);
            }
        }
        if g.pts.len() == 1 {
            let d = (p - g.pts[0]).length();
            if d < best.0 {
                best = (d, g.pressure.as_ref().map(|pp| pp[0]));
            }
        }
    }
    best
}

fn resample_stroke(s: &[StrokePoint], step: f32) -> Vec<Pos2> {
    let mut out: Vec<Pos2> = Vec::new();
    for p in s {
        match out.last() {
            None => out.push(p.pos),
            Some(&l) if (p.pos - l).length() >= step => out.push(p.pos),
            _ => {}
        }
    }
    out
}

pub fn grade_trace(strokes: &[Vec<StrokePoint>], guides: &[Guide], tol: f32, magnet: f32) -> Grade {
    let pts: Vec<&StrokePoint> = strokes.iter().flatten().collect();
    let has_pressure = guides.iter().any(|g| g.pressure.is_some());
    if pts.len() < 3 {
        return Grade {
            total: 0.0,
            metrics: vec![],
            stars: 0,
            advice: "Aún no hay trazos que calificar. Dibuja sobre la guía y vuelve a pulsar Calificar.".into(),
            assist_penalty: 0.0,
        };
    }
    // Precisión y presión: cada punto pintado frente a la guía más cercana.
    let mut sum_d = 0.0;
    let mut sum_perr = 0.0;
    let mut n_p = 0.0;
    for p in &pts {
        let (d, target) = nearest(p.pos, guides);
        sum_d += d.min(tol * 4.0);
        if let Some(t) = target {
            sum_perr += (p.pressure - t).abs();
            n_p += 1.0;
        }
    }
    let mean_d = sum_d / pts.len() as f32;
    let precision = (1.0 - mean_d / (2.0 * tol)).clamp(0.0, 1.0);

    // Cobertura: cuánta guía quedó cubierta.
    let sparse: Vec<Pos2> = strokes.iter().flat_map(|s| resample_stroke(s, 2.0)).collect();
    let mut covered = 0usize;
    let mut total = 0usize;
    for g in guides {
        for gp in g.pts.iter().step_by(2) {
            total += 1;
            if sparse.iter().any(|p| (*p - *gp).length_sq() <= tol * tol) {
                covered += 1;
            }
        }
    }
    let coverage = if total == 0 { 0.0 } else { covered as f32 / total as f32 };

    // Pulso: desvío de cada punto respecto al promedio de sus vecinos (muestras cada 6 px).
    let mut resid = 0.0;
    let mut n_r = 0.0;
    for s in strokes {
        let r = resample_stroke(s, 6.0);
        for w in r.windows(3) {
            let mid = (w[0].to_vec2() + w[2].to_vec2()) * 0.5;
            resid += (w[1].to_vec2() - mid).length();
            n_r += 1.0;
        }
    }
    let steadiness = if n_r == 0.0 { 0.5 } else { (1.0 - ((resid / n_r) - 0.15).max(0.0) / 1.5).clamp(0.0, 1.0) };

    let mut metrics = vec![
        Metric { name: "Precisión", value: precision, help: "Qué tan cerca de la guía pasan tus trazos." },
        Metric { name: "Cobertura", value: coverage, help: "Qué parte de la guía completaste." },
        Metric { name: "Pulso", value: steadiness, help: "Lo firme y continuo que es el trazo (sin temblores)." },
    ];
    let mut total_score = if has_pressure && n_p > 0.0 {
        let pressure = (1.0 - (sum_perr / n_p) / 0.35).clamp(0.0, 1.0);
        metrics.push(Metric { name: "Presión", value: pressure, help: "Si el grosor sigue el perfil de la guía." });
        0.30 * precision + 0.25 * coverage + 0.15 * steadiness + 0.30 * pressure
    } else {
        0.40 * precision + 0.30 * coverage + 0.30 * steadiness
    };
    let penalty = 0.25 * magnet.clamp(0.0, 1.0);
    total_score *= 1.0 - penalty;
    let total = (total_score * 100.0).round();
    let advice = advice_for(&metrics, penalty);
    Grade { total, stars: stars_for(total), metrics, advice, assist_penalty: penalty }
}

fn cell_mean(f: impl Fn(usize, usize) -> [f32; 3], cx: usize, cy: usize) -> [f32; 3] {
    let mut acc = [0.0; 3];
    let mut n = 0.0;
    for y in (cy * CELL..((cy + 1) * CELL).min(H)).step_by(2) {
        for x in (cx * CELL..((cx + 1) * CELL).min(W)).step_by(2) {
            let c = f(x, y);
            acc[0] += c[0];
            acc[1] += c[1];
            acc[2] += c[2];
            n += 1.0;
        }
    }
    [acc[0] / n, acc[1] / n, acc[2] / n]
}

pub fn grade_paint(canvas: &Canvas, target: &[[f32; 3]], mask: &[bool], kind: PaintTarget) -> Grade {
    let cols = W / CELL;
    let rows = H / CELL;
    let use_color = kind == PaintTarget::ColorBlend || kind == PaintTarget::Silhouette;
    let paper = luminance(crate::canvas::PAPER);
    let mut err = 0.0;
    let mut n_in = 0.0;
    let mut outside = 0.0;
    let mut n_out = 0.0;
    let mut mine: Vec<Option<f32>> = vec![None; cols * rows];
    let mut theirs: Vec<Option<f32>> = vec![None; cols * rows];
    for cy in 0..rows {
        for cx in 0..cols {
            let cm = (cx * CELL + CELL / 2, cy * CELL + CELL / 2);
            let inside = mask[cm.1 * W + cm.0];
            let c = cell_mean(|x, y| canvas.pixel(x, y), cx, cy);
            if inside {
                let t = cell_mean(|x, y| target[y * W + x], cx, cy);
                let e = if kind == PaintTarget::Silhouette {
                    // Silueta: cuenta que esté cubierta, no el tono exacto del rotulador.
                    if (luminance(c) - paper).abs() > 0.08 { 0.0 } else { 0.3 }
                } else if use_color {
                    ((c[0] - t[0]).abs() + (c[1] - t[1]).abs() + (c[2] - t[2]).abs()) / 3.0
                } else {
                    (luminance(c) - luminance(t)).abs()
                };
                err += e;
                n_in += 1.0;
                mine[cy * cols + cx] = Some(luminance(c));
                theirs[cy * cols + cx] = Some(luminance(t));
            } else {
                n_out += 1.0;
                if (luminance(c) - paper).abs() > 0.06 {
                    outside += 1.0;
                }
            }
        }
    }
    let value = if n_in == 0.0 { 0.0 } else { (1.0 - (err / n_in) / 0.3).clamp(0.0, 1.0) };
    let _ = n_out;
    let containment = if n_in == 0.0 { 1.0 } else { (1.0 - 3.0 * outside / n_in).clamp(0.0, 1.0) };
    // Transición: los saltos entre celdas vecinas deben parecerse a los de la referencia.
    let mut tr = 0.0;
    let mut n_tr = 0.0;
    for cy in 0..rows {
        for cx in 0..cols.saturating_sub(1) {
            let i = cy * cols + cx;
            if let (Some(a), Some(b), Some(ta), Some(tb)) = (mine[i], mine[i + 1], theirs[i], theirs[i + 1]) {
                tr += ((a - b).abs() - (ta - tb).abs()).abs();
                n_tr += 1.0;
            }
        }
    }
    let blend = if n_tr == 0.0 { 0.0 } else { (1.0 - (tr / n_tr) / 0.08).clamp(0.0, 1.0) };
    let metrics = vec![
        Metric { name: "Valor", value, help: "Qué tanto se parecen tus claros y oscuros a la referencia." },
        Metric { name: "Contención", value: containment, help: "Si pintaste dentro de la zona sin salirte." },
        Metric { name: "Transición", value: blend, help: "Si los cambios de tono son suaves donde deben serlo." },
    ];
    let total = ((0.60 * value + 0.25 * containment + 0.15 * blend) * 100.0).round();
    let advice = advice_for(&metrics, 0.0);
    Grade { total, stars: stars_for(total), metrics, advice, assist_penalty: 0.0 }
}

fn advice_for(metrics: &[Metric], penalty: f32) -> String {
    let worst = metrics.iter().min_by(|a, b| a.value.total_cmp(&b.value));
    let mut s = match worst.map(|m| (m.name, m.value)) {
        Some((_, v)) if v >= 0.9 => "¡Excelente! Trazo limpio y controlado. Prueba el siguiente nivel o quita las ayudas.".to_string(),
        Some(("Precisión", _)) => "Tus trazos se apartan de la guía. Mira el punto final antes de empezar y mueve el brazo entero, no solo la muñeca.".into(),
        Some(("Cobertura", _)) => "Faltan partes de la guía. Completa todas las líneas, aunque sea con trazos más cortos.".into(),
        Some(("Pulso", _)) => "Se nota temblor. Traza más rápido y decidido, o prueba el modo Ponderado o el Estabilizador para ver la diferencia.".into(),
        Some(("Presión", _)) => "El grosor no sigue el perfil. Revisa la curva en la pestaña Presión o calíbrala para tu mano.".into(),
        Some(("Valor", _)) => "Los tonos no coinciden con la referencia. Entrecierra los ojos para ver las masas de luz y sombra.".into(),
        Some(("Contención", _)) => "Te saliste de la zona. Pinta primero el borde con cuidado y luego rellena el interior.".into(),
        Some(("Transición", _)) => "Hay saltos bruscos de tono. Usa menos presión y pasa varias veces para fundir.".into(),
        _ => "Sigue practicando.".into(),
    };
    if penalty > 0.0 {
        s.push_str(&format!(" (La guía magnética restó un {:.0} %.)", penalty * 100.0));
    }
    s
}
