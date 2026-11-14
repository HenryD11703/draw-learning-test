//! Calificación de los ejercicios (diseño propio, MECHANICS.md §4).

use crate::canvas::{Canvas, H, StrokePoint, W, luminance};
use crate::lessons::{CELL, Guide, PaintTarget, Scene};
use egui::Pos2;
use std::collections::BTreeMap;

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

/// Remuestreo a distancia de arco constante (interpola sobre la polilínea de manchas).
fn resample_uniform(s: &[StrokePoint], step: f32) -> Vec<Pos2> {
    let mut out: Vec<Pos2> = Vec::new();
    let Some(first) = s.first() else { return out };
    out.push(first.pos);
    let mut prev = first.pos;
    let mut left = step;
    for p in &s[1..] {
        let mut seg = (p.pos - prev).length();
        while seg >= left && seg > 1e-6 {
            prev = prev + (p.pos - prev) * (left / seg);
            out.push(prev);
            seg -= left;
            left = step;
        }
        left -= seg;
        prev = p.pos;
    }
    out
}

/// Lado de la celda con la que se agrupan las manchas para precisión y presión.
const INK_CELL: f32 = 4.0;
/// Cobertura a partir de la cual precisión, pulso y presión cuentan completos.
const FULL_CREDIT_COVERAGE: f32 = 0.5;

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
    // Precisión y presión: cada punto pintado frente a la guía más cercana, promediado por
    // celda de tinta. Así cada zona pintada cuenta una vez: repasar un tramo fácil o pintar
    // fino (más manchas por píxel) no le da más peso.
    // Por celda: [suma de distancias, manchas, suma de error de presión, manchas con objetivo].
    let mut cells: BTreeMap<(i32, i32), [f32; 4]> = BTreeMap::new();
    for p in &pts {
        let (d, target) = nearest(p.pos, guides);
        let key = ((p.pos.x / INK_CELL).floor() as i32, (p.pos.y / INK_CELL).floor() as i32);
        let c = cells.entry(key).or_insert([0.0; 4]);
        c[0] += d.min(tol * 4.0);
        c[1] += 1.0;
        if let Some(t) = target {
            c[2] += (p.pressure - t).abs();
            c[3] += 1.0;
        }
    }
    let mut sum_d = 0.0;
    let mut sum_perr = 0.0;
    let mut n_p = 0.0;
    for c in cells.values() {
        sum_d += c[0] / c[1];
        if c[3] > 0.0 {
            sum_perr += c[2] / c[3];
            n_p += 1.0;
        }
    }
    let mean_d = sum_d / cells.len() as f32;
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

    let steadiness = steadiness(strokes.iter().map(|s| s.as_slice()));

    let mut metrics = vec![
        Metric { name: "Precisión", value: precision, help: "Qué tan cerca de la guía pasan tus trazos." },
        Metric { name: "Cobertura", value: coverage, help: "Qué parte de la guía completaste." },
        Metric { name: "Pulso", value: steadiness, help: "Lo firme y continuo que es el trazo (sin temblores)." },
    ];
    // Precisión, pulso y presión describen la calidad de lo dibujado; solo cuentan completos
    // si se cubrió al menos la mitad de la guía, para que un tramo corto y perfecto no apruebe.
    let done = (coverage / FULL_CREDIT_COVERAGE).min(1.0);
    let mut total_score = if has_pressure && n_p > 0.0 {
        let pressure = (1.0 - (sum_perr / n_p) / 0.35).clamp(0.0, 1.0);
        metrics.push(Metric { name: "Presión", value: pressure, help: "Si el grosor sigue el perfil de la guía." });
        0.25 * coverage + done * (0.30 * precision + 0.15 * steadiness + 0.30 * pressure)
    } else {
        0.30 * coverage + done * (0.40 * precision + 0.30 * steadiness)
    };
    let penalty = 0.25 * magnet.clamp(0.0, 1.0);
    total_score *= 1.0 - penalty;
    let total = (total_score * 100.0).round();
    let mut advice = advice_for(&metrics, penalty);
    if done < 1.0 {
        advice.push_str(" (Cubriste menos de la mitad de la guía: las demás medidas cuentan en proporción.)");
    }
    Grade { total, stars: stars_for(total), metrics, advice, assist_penalty: penalty }
}

/// Pulso: cuánto cambia la flecha (desvío con signo del punto central respecto a la cuerda de
/// sus vecinos) de una muestra a la siguiente, con muestras cada 6 px de arco. Una curva limpia
/// tiene flecha casi constante y no penaliza; el temblor la hace oscilar.
/// 0.85 conserva la escala del desvío anterior para un temblor aleatorio sobre una recta;
/// el tope de 2 px limita lo que cuesta una esquina intencionada.
fn steadiness<'a>(strokes: impl IntoIterator<Item = &'a [StrokePoint]>) -> f32 {
    let mut resid = 0.0;
    let mut n_r = 0.0;
    for s in strokes {
        let r = resample_uniform(s, 6.0);
        let sag: Vec<f32> = r
            .windows(3)
            .map(|w| {
                let chord = w[2] - w[0];
                let l = chord.length();
                if l < 1e-3 {
                    0.0
                } else {
                    let d = w[1] - w[0];
                    (chord.x * d.y - chord.y * d.x) / l
                }
            })
            .collect();
        for w in sag.windows(2) {
            resid += (w[1] - w[0]).abs().min(2.0) * 0.85;
            n_r += 1.0;
        }
    }
    if n_r == 0.0 { 0.5 } else { (1.0 - ((resid / n_r) - 0.15).max(0.0) / 1.5).clamp(0.0, 1.0) }
}

fn no_strokes(advice: &str) -> Grade {
    Grade { total: 0.0, metrics: vec![], stars: 0, advice: advice.into(), assist_penalty: 0.0 }
}

fn finish(metrics: Vec<Metric>, total_score: f32) -> Grade {
    let total = (total_score.clamp(0.0, 1.0) * 100.0).round();
    let advice = advice_for(&metrics, 0.0);
    Grade { total, stars: stars_for(total), metrics, advice, assist_penalty: 0.0 }
}

fn stroke_len(s: &[StrokePoint]) -> f32 {
    s.windows(2).map(|w| (w[1].pos - w[0].pos).length()).sum()
}

fn dist_to_polyline(p: Pos2, pts: &[Pos2]) -> f32 {
    if pts.len() == 1 {
        return (p - pts[0]).length();
    }
    pts.windows(2).map(|w| dist_to_segment(p, w[0], w[1]).0).fold(f32::MAX, f32::min)
}

/// Trazos con longitud suficiente para contar (descarta toques sueltos).
fn real_strokes(strokes: &[Vec<StrokePoint>], min_len: f32) -> Vec<&[StrokePoint]> {
    strokes.iter().filter(|s| s.len() >= 2 && stroke_len(s) >= min_len).map(|s| s.as_slice()).collect()
}

/// Empareja elementos por coste ascendente: cada uno se usa una sola vez.
fn greedy_match(mut pairs: Vec<(f32, usize, usize)>, na: usize, nb: usize) -> Vec<(f32, usize, usize)> {
    pairs.sort_by(|x, y| x.0.total_cmp(&y.0));
    let (mut used_a, mut used_b) = (vec![false; na], vec![false; nb]);
    let mut out = Vec::new();
    for (c, a, b) in pairs {
        if !used_a[a] && !used_b[b] {
            used_a[a] = true;
            used_b[b] = true;
            out.push((c, a, b));
        }
    }
    out
}

/// Punto a punto: solo se ven los extremos de cada guía y hay que unirlos de un trazo.
pub fn grade_points(strokes: &[Vec<StrokePoint>], guides: &[Guide], tol: f32) -> Grade {
    let real = real_strokes(strokes, 8.0);
    if real.is_empty() || guides.is_empty() {
        return no_strokes("Aún no hay trazos que calificar. Une cada par de puntos con una línea y vuelve a pulsar Calificar.");
    }
    let mut pairs = Vec::new();
    for (gi, g) in guides.iter().enumerate() {
        let (a, b) = (g.pts[0], g.pts[g.pts.len() - 1]);
        for (si, s) in real.iter().enumerate() {
            let (s0, s1) = (s[0].pos, s[s.len() - 1].pos);
            let c = ((s0 - a).length() + (s1 - b).length()).min((s0 - b).length() + (s1 - a).length()) * 0.5;
            if c <= 4.0 * tol {
                pairs.push((c, gi, si));
            }
        }
    }
    let matched = greedy_match(pairs, guides.len(), real.len());
    if matched.is_empty() {
        return no_strokes("Ningún trazo une un par de puntos. Empieza cada línea sobre un punto y termínala en el de su mismo número.");
    }
    let n = matched.len() as f32;
    let aim_err = matched.iter().map(|m| m.0).sum::<f32>() / n;
    let (mut dsum, mut dn) = (0.0f32, 0.0f32);
    for &(_, gi, si) in &matched {
        for q in resample_uniform(real[si], 3.0) {
            dsum += dist_to_polyline(q, &guides[gi].pts).min(4.0 * tol);
            dn += 1.0;
        }
    }
    let aim = (1.0 - aim_err / (2.5 * tol)).clamp(0.0, 1.0);
    let precision = (1.0 - (dsum / dn.max(1.0)) / (2.0 * tol)).clamp(0.0, 1.0);
    let steady = steadiness(matched.iter().map(|m| real[m.2]));
    let done = n / guides.len() as f32;
    let metrics = vec![
        Metric { name: "Puntería", value: aim, help: "Si tus líneas empiezan y terminan en los puntos." },
        Metric { name: "Precisión", value: precision, help: "Qué tan recta y bien dirigida va cada línea entre sus puntos." },
        Metric { name: "Pulso", value: steady, help: "Lo firme y continuo que es el trazo (sin temblores)." },
        Metric { name: "Completadas", value: done, help: "Cuántos pares de puntos uniste." },
    ];
    finish(metrics, completion_factor(done) * (0.40 * aim + 0.35 * precision + 0.25 * steady))
}

/// Cuánto pesa haber hecho solo parte del ejercicio: con 2 de 3 líneas no se aprueba,
/// con 7 de 8 bien hechas aún se pueden sacar dos estrellas.
fn completion_factor(done: f32) -> f32 {
    done.clamp(0.0, 1.0).powf(1.5)
}

fn mean(v: &[Pos2]) -> egui::Vec2 {
    v.iter().fold(egui::Vec2::ZERO, |a, p| a + p.to_vec2()) / v.len().max(1) as f32
}

fn nearest_idx(p: Pos2, set: &[Pos2]) -> usize {
    let mut best = (f32::MAX, 0);
    for (i, q) in set.iter().enumerate() {
        let d = (p - *q).length_sq();
        if d < best.0 {
            best = (d, i);
        }
    }
    best.1
}

/// Escala y traslación que mejor llevan los puntos `s` sobre `r` (ICP simétrico, sin giro).
fn align(s: &[Pos2], r: &[Pos2]) -> (f32, egui::Vec2) {
    let rms = |v: &[Pos2], m: egui::Vec2| (v.iter().map(|p| (p.to_vec2() - m).length_sq()).sum::<f32>() / v.len() as f32).sqrt();
    let (ms, mr) = (mean(s), mean(r));
    let mut k = rms(r, mr) / rms(s, ms).max(1e-3);
    let mut t = mr - ms * k;
    for _ in 0..25 {
        let q: Vec<Pos2> = s.iter().map(|p| (p.to_vec2() * k + t).to_pos2()).collect();
        let mut src = Vec::with_capacity(s.len() + r.len());
        let mut dst = Vec::with_capacity(s.len() + r.len());
        for (i, qi) in q.iter().enumerate() {
            src.push(s[i]);
            dst.push(r[nearest_idx(*qi, r)]);
        }
        for rj in r {
            src.push(s[nearest_idx(*rj, &q)]);
            dst.push(*rj);
        }
        let (ps, pd) = (mean(&src), mean(&dst));
        let num: f32 = src.iter().zip(&dst).map(|(a, b)| (a.to_vec2() - ps).dot(b.to_vec2() - pd)).sum();
        let den: f32 = src.iter().map(|a| (a.to_vec2() - ps).length_sq()).sum();
        if den < 1e-6 {
            break;
        }
        k = (num / den).max(1e-3);
        t = pd - ps * k;
    }
    (k, t)
}

fn aspect(v: &[Pos2]) -> f32 {
    let (mut lo, mut hi) = (Pos2::new(f32::MAX, f32::MAX), Pos2::new(f32::MIN, f32::MIN));
    for p in v {
        lo = lo.min(*p);
        hi = hi.max(*p);
    }
    (hi.x - lo.x).max(1.0) / (hi.y - lo.y).max(1.0)
}

/// Copia: se dibuja en cualquier sitio y tamaño; se alinea con la referencia antes de comparar.
pub fn grade_copy(strokes: &[Vec<StrokePoint>], guides: &[Guide], tol: f32) -> Grade {
    let r: Vec<Pos2> = guides.iter().flat_map(|g| g.pts.iter().copied()).collect();
    let s: Vec<Pos2> = real_strokes(strokes, 8.0).into_iter().flat_map(|st| resample_uniform(st, 3.0)).collect();
    if s.len() < 10 || r.is_empty() {
        return no_strokes("Aún no hay un dibujo que calificar. Copia la referencia en el lienzo y vuelve a pulsar Calificar.");
    }
    let (k, t) = align(&s, &r);
    let q: Vec<Pos2> = s.iter().map(|p| (p.to_vec2() * k + t).to_pos2()).collect();
    // Forma: distancia media en los dos sentidos (lo dibujado frente a la referencia y al revés),
    // para que tanto lo que sobra como lo que falta aleje de la referencia.
    let d_sr = q.iter().map(|p| (*p - r[nearest_idx(*p, &r)]).length().min(4.0 * tol)).sum::<f32>() / q.len() as f32;
    let d_r: Vec<f32> = r.iter().map(|p| (*p - q[nearest_idx(*p, &q)]).length()).collect();
    let d_rs = d_r.iter().map(|d| d.min(4.0 * tol)).sum::<f32>() / r.len() as f32;
    let cover = d_r.iter().filter(|d| **d <= 1.5 * tol).count() as f32 / r.len() as f32;
    let shape = (1.0 - (d_sr + d_rs) / 2.0 / (2.0 * tol)).clamp(0.0, 1.0);
    let proportion = (1.0 - (aspect(&s) / aspect(&r)).ln().abs() / 0.4).clamp(0.0, 1.0);
    let steady = steadiness(real_strokes(strokes, 8.0));
    let done = (cover / FULL_CREDIT_COVERAGE).min(1.0);
    let metrics = vec![
        Metric { name: "Forma", value: shape, help: "Cuánto se parece tu dibujo a la referencia, una vez igualados posición y tamaño." },
        Metric { name: "Proporción", value: proportion, help: "Si la relación entre ancho y alto es la de la referencia." },
        Metric { name: "Cobertura", value: cover, help: "Qué parte de la referencia dibujaste." },
        Metric { name: "Pulso", value: steady, help: "Lo firme y continuo que es el trazo (sin temblores)." },
    ];
    finish(metrics, 0.30 * cover + done * (0.50 * shape + 0.10 * proportion + 0.10 * steady))
}

/// Recta que mejor se ajusta a un trazo: extremos, centro, dirección unitaria y longitud.
struct Fit {
    a: Pos2,
    b: Pos2,
    c: Pos2,
    u: egui::Vec2,
    len: f32,
}

fn fit_line(s: &[StrokePoint]) -> Option<Fit> {
    if s.len() < 2 {
        return None;
    }
    let pts: Vec<Pos2> = s.iter().map(|p| p.pos).collect();
    let m = mean(&pts);
    let (mut sxx, mut syy, mut sxy) = (0.0, 0.0, 0.0);
    for p in &pts {
        let d = p.to_vec2() - m;
        sxx += d.x * d.x;
        syy += d.y * d.y;
        sxy += d.x * d.y;
    }
    let ang = 0.5 * (2.0 * sxy).atan2(sxx - syy);
    let u = egui::vec2(ang.cos(), ang.sin());
    let (mut lo, mut hi) = (f32::MAX, f32::MIN);
    for p in &pts {
        let t = (p.to_vec2() - m).dot(u);
        lo = lo.min(t);
        hi = hi.max(t);
    }
    let c = m.to_pos2();
    Some(Fit { a: c + u * lo, b: c + u * hi, c, u, len: hi - lo })
}

/// Ángulo en grados entre dos rectas (sin sentido).
fn line_angle(u: egui::Vec2, v: egui::Vec2) -> f32 {
    u.dot(v).abs().clamp(0.0, 1.0).acos().to_degrees()
}

/// Perspectiva: cada línea debe apuntar a su punto de fuga (o ser vertical u horizontal) y
/// salir de su punto verde. No se exige calcar la longitud exacta.
pub fn grade_perspective(strokes: &[Vec<StrokePoint>], scene: &Scene) -> Grade {
    let tol = scene.tol;
    let fits: Vec<(usize, Fit)> = strokes
        .iter()
        .enumerate()
        .filter_map(|(i, s)| fit_line(s).filter(|f| f.len >= 20.0).map(|f| (i, f)))
        .collect();
    if fits.is_empty() || scene.tasks.is_empty() {
        return no_strokes("Aún no hay líneas que calificar. Traza desde los puntos verdes y vuelve a pulsar Calificar.");
    }
    // Convergencia de todas las líneas, ponderada por su longitud.
    let dirs = scene.dirs();
    let (mut err, mut wsum) = (0.0, 0.0);
    for (_, f) in &fits {
        let e = dirs.iter().map(|d| line_angle(f.u, scene.dir_at(f.c, *d))).fold(f32::MAX, f32::min);
        err += e.min(20.0) * f.len;
        wsum += f.len;
    }
    let convergence = (1.0 - (err / wsum) / 8.0).clamp(0.0, 1.0);

    // Cada tarea con la línea que sale de su punto en su dirección.
    let mut pairs = Vec::new();
    for (ti, t) in scene.tasks.iter().enumerate() {
        let dir = scene.dir_at(t.from, t.dir);
        for (fi, (_, f)) in fits.iter().enumerate() {
            let d = (f.a - t.from).length().min((f.b - t.from).length());
            let ang = line_angle(f.u, dir);
            if d <= 2.5 * tol && ang <= 15.0 {
                pairs.push((d / tol + ang / 5.0, ti, fi));
            }
        }
    }
    let matched = greedy_match(pairs, scene.tasks.len(), fits.len());
    let mut done = 0.0;
    let mut aim_err = 0.0;
    for &(_, ti, fi) in &matched {
        let t = &scene.tasks[ti];
        let f = &fits[fi].1;
        let dir = scene.dir_at(t.from, t.dir);
        aim_err += (f.a - t.from).length().min((f.b - t.from).length());
        let (ta, tb) = ((f.a - t.from).dot(dir), (f.b - t.from).dot(dir));
        let overlap = (ta.max(tb).min(t.len) - ta.min(tb).max(0.0)).max(0.0);
        done += (overlap / (0.8 * t.len)).min(1.0);
    }
    let done = done / scene.tasks.len() as f32;
    let aim = if matched.is_empty() { 0.0 } else { (1.0 - (aim_err / matched.len() as f32) / (2.0 * tol)).clamp(0.0, 1.0) };
    let steady = steadiness(fits.iter().map(|(i, _)| strokes[*i].as_slice()));
    let metrics = vec![
        Metric { name: "Convergencia", value: convergence, help: "Si tus líneas apuntan a su punto de fuga (o son verticales u horizontales cuando toca)." },
        Metric { name: "Puntería", value: aim, help: "Si cada línea sale de su punto verde." },
        Metric { name: "Pulso", value: steady, help: "Lo firme y continuo que es el trazo (sin temblores)." },
        Metric { name: "Completadas", value: done, help: "Cuántas de las líneas pedidas trazaste, y con largo suficiente." },
    ];
    finish(metrics, completion_factor(done) * (0.45 * convergence + 0.30 * aim + 0.25 * steady))
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
    // Transición: los saltos entre celdas vecinas (a la derecha y abajo) deben parecerse a los
    // de la referencia.
    let mut tr = 0.0;
    let mut n_tr = 0.0;
    for cy in 0..rows {
        for cx in 0..cols {
            let i = cy * cols + cx;
            let right = (cx + 1 < cols).then(|| i + 1);
            let below = (cy + 1 < rows).then(|| i + cols);
            for j in [right, below].into_iter().flatten() {
                if let (Some(a), Some(b), Some(ta), Some(tb)) = (mine[i], mine[j], theirs[i], theirs[j]) {
                    tr += ((a - b).abs() - (ta - tb).abs()).abs();
                    n_tr += 1.0;
                }
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
        Some(("Puntería", _)) => "Tus líneas no salen o no llegan a los puntos. Mira el punto de llegada mientras trazas, no la punta del lápiz.".into(),
        Some(("Completadas", _)) => "Te faltan líneas por trazar o se quedan cortas. Revisa que hayas salido de todos los puntos.".into(),
        Some(("Forma", _)) => "La forma no coincide con la referencia. Compara ángulos y distancias entre las partes antes de trazar.".into(),
        Some(("Proporción", _)) => "Tu dibujo es más ancho o más alto que la referencia. Mide cuántas veces cabe el ancho en el alto.".into(),
        Some(("Convergencia", _)) => "Tus líneas no apuntan al punto de fuga. Antes de trazar, imagina la recta que une el punto verde con su PF.".into(),
        _ => "Sigue practicando.".into(),
    };
    if penalty > 0.0 {
        s.push_str(&format!(" (La guía magnética restó un {:.0} %.)", penalty * 100.0));
    }
    s
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::lessons::{Dir, Task};
    use egui::pos2;

    fn metric(g: &Grade, name: &str) -> f32 {
        g.metrics.iter().find(|m| m.name == name).map(|m| m.value).expect("métrica")
    }

    /// Manchas cada ~1 px a lo largo de una polilínea paramétrica.
    fn stroke(n: usize, f: impl Fn(f32) -> Pos2) -> Vec<StrokePoint> {
        (0..=n).map(|i| StrokePoint { pos: f(i as f32 / n as f32), pressure: 0.5 }).collect()
    }

    fn line_guide() -> Vec<Guide> {
        let pts = (0..=100).map(|i| pos2(100.0 + i as f32 * 3.0, 200.0)).collect();
        vec![Guide { pts, pressure: None }]
    }

    fn circle(r: f32) -> impl Fn(f32) -> Pos2 {
        move |t| pos2(300.0 + r * (t * std::f32::consts::TAU).cos(), 300.0 + r * (t * std::f32::consts::TAU).sin())
    }

    #[test]
    fn clean_small_circle_keeps_full_steadiness() {
        for r in [20.0, 40.0] {
            let n = (std::f32::consts::TAU * r) as usize;
            let guides = vec![Guide { pts: stroke(n / 3, circle(r)).iter().map(|p| p.pos).collect(), pressure: None }];
            let g = grade_trace(&[stroke(n, circle(r))], &guides, 6.0, 0.0);
            assert!(metric(&g, "Pulso") >= 0.95, "radio {r}: pulso {}", metric(&g, "Pulso"));
            assert_eq!(g.stars, 3, "radio {r}: total {}", g.total);
        }
    }

    #[test]
    fn tremor_still_lowers_steadiness() {
        let clean = stroke(300, |t| pos2(100.0 + t * 300.0, 200.0));
        let shaky = stroke(300, |t| pos2(100.0 + t * 300.0, 200.0 + 1.5 * (t * 300.0 / 14.0 * std::f32::consts::TAU).sin()));
        let c = grade_trace(&[clean], &line_guide(), 6.0, 0.0);
        let s = grade_trace(&[shaky], &line_guide(), 6.0, 0.0);
        assert!(metric(&c, "Pulso") >= 0.99);
        assert!(metric(&s, "Pulso") < 0.6, "pulso con temblor {}", metric(&s, "Pulso"));
    }

    #[test]
    fn short_perfect_segment_does_not_pass() {
        let full = stroke(300, |t| pos2(100.0 + t * 300.0, 200.0));
        let short = stroke(60, |t| pos2(100.0 + t * 60.0, 200.0));
        let f = grade_trace(&[full], &line_guide(), 6.0, 0.0);
        let s = grade_trace(&[short], &line_guide(), 6.0, 0.0);
        assert_eq!(f.stars, 3, "total completo {}", f.total);
        assert_eq!(s.stars, 0, "total del tramo corto {}", s.total);
        assert!(metric(&s, "Precisión") >= 0.99);
    }

    #[test]
    fn retracing_the_good_part_does_not_raise_precision() {
        // Mitad izquierda sobre la guía, mitad derecha 9 px por debajo.
        let once = stroke(300, |t| pos2(100.0 + t * 300.0, if t < 0.5 { 200.0 } else { 209.0 }));
        let good_half = stroke(150, |t| pos2(100.0 + t * 150.0, 200.0));
        let mut many = vec![once.clone()];
        many.extend(std::iter::repeat_n(good_half, 20));
        let a = metric(&grade_trace(&[once], &line_guide(), 6.0, 0.0), "Precisión");
        let b = metric(&grade_trace(&many, &line_guide(), 6.0, 0.0), "Precisión");
        assert!((a - b).abs() < 0.02, "una pasada {a}, con repasos {b}");
        assert!(a < 0.7);
    }

    /// Manchas cada ~1 px de `a` a `b`.
    fn seg(a: Pos2, b: Pos2) -> Vec<StrokePoint> {
        let n = ((b - a).length().ceil() as usize).max(2);
        stroke(n, |t| a + (b - a) * t)
    }

    fn g2(a: Pos2, b: Pos2) -> Guide {
        Guide { pts: vec![a, b], pressure: None }
    }

    #[test]
    fn points_scores_aim_and_completion() {
        let guides = vec![g2(pos2(100.0, 100.0), pos2(400.0, 160.0)), g2(pos2(500.0, 300.0), pos2(300.0, 500.0)), g2(pos2(600.0, 100.0), pos2(800.0, 400.0))];
        let perfect: Vec<_> = guides.iter().map(|g| seg(g.pts[1], g.pts[0])).collect();
        let p = grade_points(&perfect, &guides, 12.0);
        assert_eq!(p.stars, 3, "total {}", p.total);
        // Dos de tres, una con el final 20 px corto.
        let partial = vec![seg(pos2(100.0, 100.0), pos2(400.0, 160.0)), seg(pos2(500.0, 300.0), pos2(314.0, 486.0))];
        let q = grade_points(&partial, &guides, 12.0);
        assert!((metric(&q, "Completadas") - 2.0 / 3.0).abs() < 1e-3);
        assert!(metric(&q, "Puntería") < metric(&p, "Puntería"));
        assert_eq!(q.stars, 0, "total {}", q.total);
    }

    fn house() -> Vec<Guide> {
        let poly = |v: &[(f32, f32)]| Guide { pts: crate::lessons::resample(&v.iter().map(|&(x, y)| pos2(x, y)).collect::<Vec<_>>()), pressure: None };
        vec![
            poly(&[(330.0, 290.0), (630.0, 290.0), (630.0, 520.0), (330.0, 520.0), (330.0, 290.0)]),
            poly(&[(300.0, 300.0), (480.0, 140.0), (660.0, 300.0)]),
            poly(&[(400.0, 520.0), (400.0, 410.0), (470.0, 410.0), (470.0, 520.0)]),
        ]
    }

    fn drawn(guides: &[Guide], f: impl Fn(Pos2) -> Pos2) -> Vec<Vec<StrokePoint>> {
        guides.iter().map(|g| g.pts.iter().map(|p| StrokePoint { pos: f(*p), pressure: 0.5 }).collect()).collect()
    }

    #[test]
    fn copy_ignores_position_and_size_but_not_shape() {
        let guides = house();
        let small = grade_copy(&drawn(&guides, |p| pos2(p.x * 0.6 + 200.0, p.y * 0.6 + 120.0)), &guides, 14.0);
        assert_eq!(small.stars, 3, "copia pequeña y desplazada: {}", small.total);
        let stretched = grade_copy(&drawn(&guides, |p| pos2(p.x * 1.5 - 200.0, p.y)), &guides, 14.0);
        assert!(metric(&stretched, "Proporción") < 0.1);
        assert!(stretched.total < small.total - 15.0, "estirada {} vs {}", stretched.total, small.total);
        let circle = vec![stroke(900, |t| pos2(480.0 + 150.0 * (t * std::f32::consts::TAU).cos(), 330.0 + 150.0 * (t * std::f32::consts::TAU).sin()))];
        let wrong = grade_copy(&circle, &guides, 14.0);
        assert!(wrong.total < 60.0, "un círculo no es una casa: {}", wrong.total);
    }

    fn scene() -> Scene {
        let vp = pos2(480.0, 200.0);
        let tasks = [pos2(150.0, 550.0), pos2(480.0, 600.0), pos2(820.0, 520.0), pos2(120.0, 330.0)]
            .iter()
            .map(|&from| Task { from, dir: Dir::Vp(0), len: (vp - from).length() * 0.7 })
            .collect();
        Scene { horizon: Some(200.0), vps: vec![vp], given: vec![], tasks, tol: 12.0, hints: false }
    }

    #[test]
    fn perspective_rewards_lines_that_converge() {
        let s = scene();
        let good: Vec<_> = s.tasks.iter().map(|t| { let (a, b) = s.target(t); seg(a, b) }).collect();
        let g = grade_perspective(&good, &s);
        assert_eq!(g.stars, 3, "total {}", g.total);
        // Mismas salidas, pero apuntando 150 px a la derecha del punto de fuga.
        let wrong: Vec<_> = s.tasks.iter().map(|t| {
            let to = pos2(630.0, 200.0);
            seg(t.from, t.from + (to - t.from) * 0.7)
        }).collect();
        let w = grade_perspective(&wrong, &s);
        assert!(metric(&w, "Convergencia") < 0.5, "convergencia {}", metric(&w, "Convergencia"));
        assert!(w.total < 60.0, "total {}", w.total);
        // Solo la mitad de las líneas.
        let half = grade_perspective(&good[..2], &s);
        assert!((metric(&half, "Completadas") - 0.5).abs() < 0.05);
    }

    #[test]
    fn transition_sees_vertical_gradients() {
        // Referencia: degradado vertical en las filas de celdas 10..20; saltos de 0.09 por celda.
        let rows = 10..20;
        let tone = |y: usize| 0.05 + 0.09 * (y / CELL - 10) as f32;
        let mut target = vec![crate::canvas::PAPER; W * H];
        let mut mask = vec![false; W * H];
        for y in rows.start * CELL..rows.end * CELL {
            for x in 0..W {
                target[y * W + x] = [tone(y); 3];
                mask[y * W + x] = true;
            }
        }
        let mut flat = Canvas::new();
        let mut exact = Canvas::new();
        for y in rows.start * CELL..rows.end * CELL {
            for x in 0..W {
                flat.base[y * W + x] = [0.45; 3];
                exact.base[y * W + x] = [tone(y); 3];
            }
        }
        let f = grade_paint(&flat, &target, &mask, PaintTarget::Gradient);
        let e = grade_paint(&exact, &target, &mask, PaintTarget::Gradient);
        assert!(metric(&e, "Transición") >= 0.99);
        assert!(metric(&f, "Transición") < 0.6, "transición del tono plano {}", metric(&f, "Transición"));
    }
}
