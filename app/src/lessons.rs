//! Módulos y ejercicios precargados.

use crate::canvas::{H, PAPER, W};
use crate::smoothing::SmoothingType;
use egui::{Pos2, pos2, vec2};
use std::f32::consts::{PI, TAU};

#[derive(Clone)]
pub struct Guide {
    pub pts: Vec<Pos2>,
    /// Presión objetivo en cada punto (0..1) si el ejercicio la califica.
    pub pressure: Option<Vec<f32>>,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum PaintTarget {
    ValueSteps,
    Gradient,
    Sphere,
    Cylinder,
    Silhouette,
    Cleanup,
    ColorBlend,
}

#[derive(Clone)]
pub enum Kind {
    Trace { guides: Vec<Guide>, tol: f32 },
    Paint { target: PaintTarget },
}

#[derive(Clone)]
pub struct Exercise {
    pub id: String,
    pub title: &'static str,
    pub goal: &'static str,
    pub krita_tip: &'static str,
    pub brush: &'static str,
    pub smoothing: Option<SmoothingType>,
    pub color: Option<[f32; 3]>,
    pub kind: Kind,
    /// Segundos que se ve la guía antes de ocultarse (ejercicios de memoria).
    pub memory: Option<f32>,
}

pub struct Module {
    pub title: &'static str,
    pub subtitle: &'static str,
    pub lesson: &'static str,
    pub accent: egui::Color32,
    pub exercises: Vec<Exercise>,
}

// ------------------------------------------------------------------ geometría

const STEP: f32 = 3.0;

fn resample(pts: &[Pos2]) -> Vec<Pos2> {
    let mut out = Vec::new();
    if pts.is_empty() {
        return out;
    }
    out.push(pts[0]);
    let mut carry = 0.0;
    for w in pts.windows(2) {
        let (a, b) = (w[0], w[1]);
        let len = (b - a).length();
        let mut s = STEP - carry;
        while s <= len {
            out.push(a + (b - a) * (s / len));
            s += STEP;
        }
        carry = len - (s - STEP);
    }
    if out.last() != pts.last() {
        out.push(*pts.last().unwrap());
    }
    out
}

fn g(pts: Vec<Pos2>) -> Guide {
    Guide { pts: resample(&pts), pressure: None }
}

fn gp(pts: Vec<Pos2>, profile: impl Fn(f32) -> f32) -> Guide {
    let pts = resample(&pts);
    let n = pts.len().max(2) as f32 - 1.0;
    let pressure = (0..pts.len()).map(|i| profile(i as f32 / n)).collect();
    Guide { pts, pressure: Some(pressure) }
}

fn line(a: Pos2, b: Pos2) -> Vec<Pos2> {
    vec![a, b]
}

fn ellipse(c: Pos2, rx: f32, ry: f32, rot: f32, a0: f32, a1: f32) -> Vec<Pos2> {
    let n = 160;
    let (s, co) = rot.sin_cos();
    (0..=n)
        .map(|i| {
            let t = a0 + (a1 - a0) * i as f32 / n as f32;
            let x = rx * t.cos();
            let y = ry * t.sin();
            pos2(c.x + x * co - y * s, c.y + x * s + y * co)
        })
        .collect()
}

fn circle(c: Pos2, r: f32) -> Vec<Pos2> {
    ellipse(c, r, r, 0.0, -PI / 2.0, -PI / 2.0 + TAU)
}

fn bezier(p0: Pos2, p1: Pos2, p2: Pos2, p3: Pos2) -> Vec<Pos2> {
    (0..=80)
        .map(|i| {
            let t = i as f32 / 80.0;
            let u = 1.0 - t;
            (p0.to_vec2() * (u * u * u) + p1.to_vec2() * (3.0 * u * u * t) + p2.to_vec2() * (3.0 * u * t * t) + p3.to_vec2() * (t * t * t)).to_pos2()
        })
        .collect()
}

fn spiral(c: Pos2, r0: f32, r1: f32, turns: f32) -> Vec<Pos2> {
    let n = (turns * 120.0) as usize;
    (0..=n)
        .map(|i| {
            let t = i as f32 / n as f32;
            let a = t * turns * TAU;
            let r = r0 + (r1 - r0) * t;
            pos2(c.x + r * a.cos(), c.y + r * a.sin())
        })
        .collect()
}

fn wave(x0: f32, x1: f32, y: f32, amp: f32, periods: f32) -> Vec<Pos2> {
    (0..=200)
        .map(|i| {
            let t = i as f32 / 200.0;
            pos2(x0 + (x1 - x0) * t, y + amp * (t * periods * TAU).sin())
        })
        .collect()
}

fn poly(pts: &[(f32, f32)], closed: bool) -> Vec<Pos2> {
    let mut v: Vec<Pos2> = pts.iter().map(|&(x, y)| pos2(x, y)).collect();
    if closed {
        v.push(v[0]);
    }
    v
}

fn star(c: Pos2, r_out: f32, r_in: f32) -> Vec<Pos2> {
    let mut v = Vec::new();
    for i in 0..=10 {
        let a = -PI / 2.0 + i as f32 * PI / 5.0;
        let r = if i % 2 == 0 { r_out } else { r_in };
        v.push(pos2(c.x + r * a.cos(), c.y + r * a.sin()));
    }
    v
}

// ------------------------------------------------------------------ objetivos de pintura

pub const CELL: usize = 16;

/// Imagen objetivo y máscara (dónde se califica) de un ejercicio de pintura.
pub fn paint_target(t: PaintTarget) -> (Vec<[f32; 3]>, Vec<bool>) {
    let mut img = vec![PAPER; W * H];
    let mut mask = vec![false; W * H];
    let set = |img: &mut Vec<[f32; 3]>, mask: &mut Vec<bool>, x: usize, y: usize, c: [f32; 3]| {
        img[y * W + x] = c;
        mask[y * W + x] = true;
    };
    match t {
        PaintTarget::ValueSteps => {
            let vals = [0.92, 0.72, 0.5, 0.3, 0.1];
            for (k, v) in vals.iter().enumerate() {
                let x0 = 120 + k * 150;
                for y in 220..400 {
                    for x in x0..x0 + 120 {
                        set(&mut img, &mut mask, x, y, [*v; 3]);
                    }
                }
            }
        }
        PaintTarget::Gradient => {
            for y in 240..400 {
                for x in 120..840 {
                    let t = (x - 120) as f32 / 720.0;
                    let v = 0.12 + 0.82 * t;
                    set(&mut img, &mut mask, x, y, [v; 3]);
                }
            }
        }
        PaintTarget::Sphere => {
            let (cx, cy, r) = (480.0, 320.0, 190.0);
            let light = vec2(-0.55, -0.6);
            let lz = (1.0 - light.length_sq()).max(0.0).sqrt();
            for y in 0..H {
                for x in 0..W {
                    let dx = (x as f32 - cx) / r;
                    let dy = (y as f32 - cy) / r;
                    let d2 = dx * dx + dy * dy;
                    if d2 <= 1.0 {
                        let nz = (1.0 - d2).sqrt();
                        let lam = (dx * light.x + dy * light.y + nz * lz).max(0.0);
                        let v = 0.12 + 0.8 * lam;
                        set(&mut img, &mut mask, x, y, [v; 3]);
                    }
                }
            }
        }
        PaintTarget::Cylinder => {
            let (x0, x1, y0, y1) = (330.0, 630.0, 150.0, 500.0);
            for y in y0 as usize..y1 as usize {
                for x in x0 as usize..x1 as usize {
                    let u = (x as f32 - x0) / (x1 - x0) * 2.0 - 1.0;
                    let nz = (1.0 - u * u).max(0.0).sqrt();
                    let lam = (-0.6 * u + 0.8 * nz).max(0.0);
                    let v = 0.14 + 0.78 * lam;
                    set(&mut img, &mut mask, x, y, [v; 3]);
                }
            }
        }
        PaintTarget::Silhouette => {
            // Una hoja: intersección de dos círculos.
            let c1 = pos2(480.0, 470.0);
            let c2 = pos2(480.0, 170.0);
            let r = 260.0;
            for y in 0..H {
                for x in 0..W {
                    let p = pos2(x as f32 + 0.5, y as f32 + 0.5);
                    let rot = pos2(480.0 + (p.x - 480.0) * 0.9 - (p.y - 320.0) * 0.44, 320.0 + (p.x - 480.0) * 0.44 + (p.y - 320.0) * 0.9);
                    if (rot - c1).length() < r && (rot - c2).length() < r {
                        set(&mut img, &mut mask, x, y, [0.2, 0.45, 0.85]);
                    }
                }
            }
        }
        PaintTarget::Cleanup => {
            for y in 140..500 {
                for x in 200..760 {
                    set(&mut img, &mut mask, x, y, PAPER);
                }
            }
        }
        PaintTarget::ColorBlend => {
            let a = [0.85, 0.35, 0.2];
            let b = [0.2, 0.4, 0.8];
            for y in 220..420 {
                for x in 140..820 {
                    let t = ((x - 140) as f32 / 680.0).clamp(0.0, 1.0);
                    let s = t * t * (3.0 - 2.0 * t);
                    set(&mut img, &mut mask, x, y, [a[0] + (b[0] - a[0]) * s, a[1] + (b[1] - a[1]) * s, a[2] + (b[2] - a[2]) * s]);
                }
            }
        }
    }
    (img, mask)
}

/// Lienzo inicial de un ejercicio (por ejemplo, manchas para limpiar o colores para mezclar).
pub fn starting_canvas(t: PaintTarget) -> Option<Vec<[f32; 3]>> {
    match t {
        PaintTarget::Cleanup => {
            let mut img = vec![PAPER; W * H];
            let blobs = [(260.0, 200.0, 40.0), (420.0, 330.0, 55.0), (640.0, 230.0, 35.0), (560.0, 430.0, 45.0), (330.0, 440.0, 30.0), (700.0, 380.0, 28.0)];
            for y in 0..H {
                for x in 0..W {
                    for &(bx, by, br) in &blobs {
                        let d = ((x as f32 - bx).powi(2) + (y as f32 - by).powi(2)).sqrt();
                        if d < br {
                            img[y * W + x] = [0.25, 0.22, 0.3];
                        }
                    }
                }
            }
            Some(img)
        }
        PaintTarget::ColorBlend => {
            let mut img = vec![PAPER; W * H];
            for y in 220..420 {
                for x in 140..820 {
                    img[y * W + x] = if x < 480 { [0.85, 0.35, 0.2] } else { [0.2, 0.4, 0.8] };
                }
            }
            Some(img)
        }
        _ => None,
    }
}

// ------------------------------------------------------------------ contenido

fn ex(
    id: &str,
    title: &'static str,
    goal: &'static str,
    krita_tip: &'static str,
    brush: &'static str,
    smoothing: Option<SmoothingType>,
    kind: Kind,
) -> Exercise {
    Exercise { id: id.to_string(), title, goal, krita_tip, brush, smoothing, color: None, kind, memory: None }
}

fn trace(guides: Vec<Guide>, tol: f32) -> Kind {
    Kind::Trace { guides, tol }
}

pub fn modules() -> Vec<Module> {
    use SmoothingType::*;
    let taper_out = |t: f32| 1.0 - 0.85 * t;
    let taper_in = |t: f32| 0.15 + 0.85 * t;
    let spindle = |t: f32| 0.12 + 0.88 * (t * PI).sin();
    vec![
        Module {
            title: "1. Fundamentos del trazo",
            subtitle: "Líneas, curvas y círculos con pulso firme",
            lesson: "Dibujar en digital empieza por el brazo, no por la muñeca. Apoya el antebrazo, mira el punto final antes de empezar y traza con un solo movimiento decidido. Es mejor una línea rápida y limpia que una lenta y temblorosa.\n\nEn estos ejercicios usarás el pincel por defecto de Krita (b) Basic-5 Size) con el suavizado Básico, el mismo que trae Krita al instalarlo.",
            accent: egui::Color32::from_rgb(88, 166, 255),
            exercises: vec![
                ex("m1e1", "Líneas horizontales", "Repasa las 5 líneas de izquierda a derecha, cada una de un solo trazo.",
                    "En Krita el Pincel a mano alzada se elige con la tecla B. Cambia el tamaño con [ y ] o con Mayús + arrastrar.",
                    "basico5", Some(Basico),
                    trace((0..5).map(|i| g(line(pos2(160.0, 140.0 + i as f32 * 90.0), pos2(800.0, 140.0 + i as f32 * 90.0)))).collect(), 12.0)),
                ex("m1e2", "Líneas verticales", "Repasa las líneas de arriba abajo. Gira el lienzo si te resulta incómodo: en Krita también puedes.",
                    "Si te equivocas, Ctrl+Z deshace el último trazo, igual que aquí.",
                    "basico5", Some(Basico),
                    trace((0..6).map(|i| g(line(pos2(180.0 + i as f32 * 120.0, 110.0), pos2(180.0 + i as f32 * 120.0, 530.0)))).collect(), 12.0)),
                ex("m1e3", "Diagonales", "Traza las diagonales en ambas direcciones sin detenerte.",
                    "Krita tiene la herramienta de Asistentes (Regla, Paralelas…) para cuando necesites diagonales exactas; aquí entrenas la mano.",
                    "basico5", Some(Basico),
                    trace(vec![
                        g(line(pos2(150.0, 120.0), pos2(450.0, 520.0))),
                        g(line(pos2(250.0, 120.0), pos2(550.0, 520.0))),
                        g(line(pos2(810.0, 120.0), pos2(510.0, 520.0))),
                        g(line(pos2(710.0, 120.0), pos2(410.0, 520.0))),
                    ], 12.0)),
                ex("m1e4", "Curvas en C y en S", "Repasa las curvas con un movimiento continuo del brazo.",
                    "Las curvas largas salen mejor si mueves el lienzo para que el gesto sea natural (en Krita: rotar con Mayús + barra espaciadora y arrastrar).",
                    "basico5", Some(Basico),
                    trace(vec![
                        g(bezier(pos2(260.0, 120.0), pos2(80.0, 160.0), pos2(80.0, 480.0), pos2(260.0, 520.0))),
                        g(bezier(pos2(440.0, 120.0), pos2(300.0, 180.0), pos2(580.0, 460.0), pos2(440.0, 520.0))),
                        g(bezier(pos2(640.0, 120.0), pos2(860.0, 200.0), pos2(500.0, 300.0), pos2(720.0, 520.0))),
                    ], 13.0)),
                ex("m1e5", "Círculos", "Dibuja cada círculo de una vez. Puedes \"fantasmear\": mueve el lápiz en el aire antes de apoyar.",
                    "Krita tiene un asistente de Elipse que atrae el trazo; aquí lo sustituye la guía magnética, pero intenta no usarla.",
                    "basico5", Some(Basico),
                    trace(vec![g(circle(pos2(230.0, 320.0), 120.0)), g(circle(pos2(500.0, 320.0), 90.0)), g(circle(pos2(730.0, 320.0), 60.0))], 13.0)),
                ex("m1e6", "Elipses", "Repasa las elipses: son círculos vistos en perspectiva.",
                    "En Krita, la herramienta Elipse (no el pincel) dibuja elipses perfectas, pero saber hacerlas a mano es clave para bocetar.",
                    "basico5", Some(Basico),
                    trace(vec![
                        g(ellipse(pos2(240.0, 320.0), 140.0, 60.0, 0.0, 0.0, TAU)),
                        g(ellipse(pos2(520.0, 320.0), 120.0, 80.0, 0.6, 0.0, TAU)),
                        g(ellipse(pos2(760.0, 320.0), 50.0, 150.0, 0.0, 0.0, TAU)),
                    ], 13.0)),
            ],
        },
        Module {
            title: "2. Control de presión",
            subtitle: "Grosor y peso de la línea con la presión del lápiz",
            lesson: "Con una tableta, la presión es tu herramienta expresiva: un mismo trazo puede empezar fino, engordar y terminar en punta. Krita lee la presión, la pasa por una curva global (Ajustes de tableta) y luego por la curva de cada pincel.\n\nLa guía muestra el grosor esperado. Si usas ratón, la app simula la presión con la velocidad (lento = más presión); puedes cambiarlo en Presión.",
            accent: egui::Color32::from_rgb(255, 170, 70),
            exercises: vec![
                ex("m2e1", "Trazos que se afinan", "Empieza presionando y suelta poco a poco hasta terminar en punta.",
                    "En el Editor de pinceles de Krita (F5), la opción Tamaño usa el sensor Presión con su propia curva.",
                    "basico5", Some(Basico),
                    trace((0..4).map(|i| gp(line(pos2(180.0, 150.0 + i as f32 * 110.0), pos2(780.0, 150.0 + i as f32 * 110.0)), taper_out)).collect(), 14.0)),
                ex("m2e2", "Trazos que engordan", "Empieza muy suave y aumenta la presión hasta el final.",
                    "La curva global de presión está en Configuración › Configurar Krita › Ajustes de tableta. Aquí la ajustas en la pestaña Presión.",
                    "basico5", Some(Basico),
                    trace((0..4).map(|i| gp(line(pos2(180.0, 150.0 + i as f32 * 110.0), pos2(780.0, 150.0 + i as f32 * 110.0)), taper_in)).collect(), 14.0)),
                ex("m2e3", "Husos", "Fino › grueso › fino. Es el trazo base del entintado.",
                    "Con la plumilla (d) Ink-3 Gpen) la diferencia entre poca y mucha presión es enorme; practica aquí antes de entintar en Krita.",
                    "gpen", Some(Basico),
                    trace(vec![
                        gp(bezier(pos2(150.0, 180.0), pos2(350.0, 80.0), pos2(600.0, 280.0), pos2(810.0, 160.0)), spindle),
                        gp(bezier(pos2(150.0, 340.0), pos2(350.0, 240.0), pos2(600.0, 440.0), pos2(810.0, 320.0)), spindle),
                        gp(bezier(pos2(150.0, 500.0), pos2(350.0, 400.0), pos2(600.0, 600.0), pos2(810.0, 480.0)), spindle),
                    ], 15.0)),
                ex("m2e4", "Ola con presión", "Presiona en las crestas y aligera en los valles.",
                    "Si tu tableta llega a presión máxima demasiado pronto, suaviza la curva: es lo primero que se ajusta al instalar Krita.",
                    "basico5", Some(Basico),
                    trace(vec![gp(wave(140.0, 820.0, 320.0, 120.0, 2.0), |t| 0.2 + 0.75 * ((t * 2.0 * TAU).sin() * -0.5 + 0.5))], 16.0)),
                ex("m2e5", "Escalera de presión", "Haz cada tramo con un nivel distinto: suave, medio, firme y máximo.",
                    "La calibración de la pestaña Presión adapta la curva a tu mano: así tu \"suave\" y tu \"fuerte\" cubren todo el rango.",
                    "basico2", Some(Basico),
                    trace((0..4).map(|i| {
                        let p = 0.25 + i as f32 * 0.25;
                        gp(line(pos2(160.0 + i as f32 * 170.0, 320.0), pos2(300.0 + i as f32 * 170.0, 320.0)), move |_| p)
                    }).collect(), 14.0)),
            ],
        },
        Module {
            title: "3. Asistente de trazo",
            subtitle: "Básico, Ponderado y Estabilizador de Krita",
            lesson: "Krita puede suavizar tu trazo mientras dibujas. Está en el acoplable Opciones de herramienta del Pincel a mano alzada › Suavizado del pincel.\n\n• Básico: une los puntos con curvas.\n• Ponderado: promedia los últimos puntos; la línea va un poco detrás.\n• Estabilizador: el pincel persigue al cursor como atado con una cuerda; la Distancia controla cuánto.\n\nEn cada ejercicio prueba el modo propuesto y cambia la Distancia para notar la diferencia.",
            accent: egui::Color32::from_rgb(140, 220, 120),
            exercises: vec![
                ex("m3e1", "Espiral con Básico", "Repasa la espiral desde el centro hacia afuera.",
                    "Básico es el modo por defecto. Con Distancia 50 y velocidad lenta casi no notarás corrección.",
                    "fineliner", Some(Basico),
                    trace(vec![g(spiral(pos2(480.0, 320.0), 12.0, 250.0, 3.0))], 12.0)),
                ex("m3e2", "Espiral con Ponderado", "La misma espiral, ahora con el modo Ponderado. Nota cómo la línea va detrás del cursor.",
                    "En Ponderado, la Agresividad de cola recorta la punta cuando levantas el lápiz; la Distancia indica cuántos píxeles promedia.",
                    "fineliner", Some(Ponderado),
                    trace(vec![g(spiral(pos2(480.0, 320.0), 12.0, 250.0, 3.0))], 12.0)),
                ex("m3e3", "Lazos con Estabilizador", "Repasa los lazos con el Estabilizador: mueve el cursor y deja que el pincel lo siga.",
                    "Con Retardo activado, el pincel no se mueve hasta que te alejas la Distancia de retardo: así ignora temblores pequeños.",
                    "fineliner", Some(Estabilizador),
                    trace(vec![g({
                        let mut v = Vec::new();
                        for i in 0..=400 {
                            let t = i as f32 / 400.0;
                            let a = t * 4.0 * TAU;
                            v.push(pos2(170.0 + t * 620.0 + 55.0 * a.cos(), 320.0 + 90.0 * a.sin()));
                        }
                        v
                    })], 14.0)),
                ex("m3e4", "Contorno largo", "Entinta el contorno de la hoja con un trazo por lado.",
                    "Para entintar en Krita, crea una capa nueva encima del boceto y usa el Estabilizador con un fineliner.",
                    "fineliner", Some(Estabilizador),
                    trace(vec![
                        g(bezier(pos2(250.0, 470.0), pos2(300.0, 160.0), pos2(600.0, 120.0), pos2(730.0, 150.0))),
                        g(bezier(pos2(250.0, 470.0), pos2(500.0, 470.0), pos2(720.0, 380.0), pos2(730.0, 150.0))),
                        g(bezier(pos2(250.0, 470.0), pos2(400.0, 330.0), pos2(560.0, 250.0), pos2(700.0, 175.0))),
                    ], 13.0)),
                ex("m3e5", "Pixel art", "Repasa la escalera con el suavizado Pixel y el pincel Pixel art.",
                    "Krita tiene un modo de suavizado Pixel que evita los píxeles de esquina en las diagonales.",
                    "pixel", Some(Pixel),
                    trace(vec![g(poly(&[(200.0, 480.0), (200.0, 400.0), (300.0, 400.0), (300.0, 320.0), (400.0, 320.0), (400.0, 240.0), (500.0, 240.0), (500.0, 160.0), (600.0, 160.0)], false)),
                        g(line(pos2(620.0, 480.0), pos2(780.0, 160.0)))], 8.0)),
            ],
        },
        Module {
            title: "4. Formas y perspectiva",
            subtitle: "Construye objetos con formas simples",
            lesson: "Todo dibujo complejo se arma con cajas, cilindros y esferas. Aquí repasarás formas básicas y luego en perspectiva. Dibuja también las líneas \"ocultas\": te ayudan a entender el volumen.",
            accent: egui::Color32::from_rgb(200, 130, 255),
            exercises: vec![
                ex("m4e1", "Cuadrados", "Cada lado de un trazo. Intenta que las esquinas cierren.",
                    "En Krita, mantener Mayús mientras dibujas con la herramienta Rectángulo hace un cuadrado; aquí es a mano alzada.",
                    "basico5", Some(Basico),
                    trace(vec![
                        g(poly(&[(140.0, 180.0), (400.0, 180.0), (400.0, 440.0), (140.0, 440.0)], true)),
                        g(poly(&[(560.0, 200.0), (800.0, 200.0), (800.0, 440.0), (560.0, 440.0)], true)),
                    ], 12.0)),
                ex("m4e2", "Triángulo y estrella", "Repasa el triángulo y la estrella sin levantar el lápiz en cada lado.",
                    "Las formas cerradas se pueden rellenar en Krita con la herramienta Relleno (tecla F).",
                    "basico5", Some(Basico),
                    trace(vec![
                        g(poly(&[(250.0, 140.0), (410.0, 470.0), (90.0, 470.0)], true)),
                        g(star(pos2(660.0, 320.0), 200.0, 80.0)),
                    ], 12.0)),
                ex("m4e3", "Cubo en perspectiva", "Dibuja las aristas del cubo, incluidas las de atrás.",
                    "La herramienta de Asistentes de Krita tiene un asistente de Perspectiva que dirige tus líneas al punto de fuga.",
                    "fineliner", Some(Basico),
                    trace({
                        let f = [(300.0, 260.0), (520.0, 260.0), (520.0, 480.0), (300.0, 480.0)];
                        let b = [(420.0, 170.0), (640.0, 170.0), (640.0, 390.0), (420.0, 390.0)];
                        let mut v = vec![g(poly(&f, true)), g(poly(&b, true))];
                        for i in 0..4 {
                            v.push(g(line(pos2(f[i].0, f[i].1), pos2(b[i].0, b[i].1))));
                        }
                        v
                    }, 11.0)),
                ex("m4e4", "Cilindro", "Dos elipses y dos lados. Haz la elipse de abajo completa, aunque no se vea.",
                    "Para dibujos limpios en Krita: boceto en una capa con baja opacidad y línea final en otra capa.",
                    "fineliner", Some(Basico),
                    trace(vec![
                        g(ellipse(pos2(480.0, 170.0), 150.0, 50.0, 0.0, 0.0, TAU)),
                        g(ellipse(pos2(480.0, 470.0), 150.0, 50.0, 0.0, 0.0, TAU)),
                        g(line(pos2(330.0, 170.0), pos2(330.0, 470.0))),
                        g(line(pos2(630.0, 170.0), pos2(630.0, 470.0))),
                    ], 12.0)),
            ],
        },
        Module {
            title: "5. Los pinceles de Krita",
            subtitle: "Lápiz, tinta, rotulador, aerógrafo y borrador",
            lesson: "Krita trae decenas de pinceles agrupados por letra: a) borradores, b) básicos, c) lápices, d) tintas, e) rotuladores, i) húmedos… Cada uno responde a la presión de forma distinta. En este módulo usarás uno en cada ejercicio para aprender cuándo conviene cada cual.\n\nEn Krita, los pinceles están en el acoplable Preajustes de pincel (F6 abre su menú emergente) y se editan con F5.",
            accent: egui::Color32::from_rgb(255, 110, 140),
            exercises: vec![
                Exercise {
                    color: Some([0.18, 0.18, 0.2]),
                    ..ex("m5e1", "Lápiz: rayado", "Rellena cada franja con líneas paralelas. Presión suave arriba, fuerte abajo.",
                        "c) Pencil-2 tiene grano de papel; la opacidad sube con la presión. Es ideal para el boceto inicial.",
                        "lapiz", Some(Basico),
                        trace((0..14).map(|i| {
                            let x = 200.0 + i as f32 * 40.0;
                            g(line(pos2(x, 160.0), pos2(x + 120.0, 480.0)))
                        }).collect(), 10.0))
                },
                ex("m5e2", "Tinta: flor", "Entinta la flor con el fineliner. Trazos seguros, sin repasar.",
                    "d) Ink-2 Fineliner: tamaño casi constante (la presión solo baja hasta el 75 %) y adelgaza con la velocidad.",
                    "fineliner", Some(Estabilizador),
                    trace({
                        let c = pos2(480.0, 280.0);
                        let mut v = vec![g(circle(c, 40.0))];
                        for k in 0..6 {
                            let a = k as f32 * TAU / 6.0;
                            let d = vec2(a.cos(), a.sin());
                            let n = vec2(-d.y, d.x);
                            let p0 = c + d * 40.0;
                            let tip = c + d * 150.0;
                            v.push(g(bezier(p0, p0 + d * 50.0 + n * 60.0, tip + n * 30.0, tip)));
                        }
                        v.push(g(bezier(pos2(480.0, 430.0), pos2(470.0, 480.0), pos2(500.0, 540.0), pos2(480.0, 600.0))));
                        v
                    }, 12.0)),
                Exercise {
                    color: Some([0.2, 0.45, 0.85]),
                    ..ex("m5e3", "Rotulador: rellenar", "Rellena la hoja sin salirte. Verás que los cruces de trazos se oscurecen.",
                        "e) Marker: la opacidad tiene un tope por trazo. En Krita, activa Bloquear alfa en la capa para no salirte del dibujo.",
                        "marcador", Some(Basico), Kind::Paint { target: PaintTarget::Silhouette })
                },
                Exercise {
                    color: Some([0.1, 0.1, 0.12]),
                    ..ex("m5e4", "Aerógrafo: degradado", "Haz un degradado de oscuro (izquierda) a claro (derecha) dentro del rectángulo.",
                        "b) Airbrush Soft acumula color al pasar. En Krita también tienes la herramienta Degradado (tecla G) para fondos.",
                        "aero", Some(Ponderado), Kind::Paint { target: PaintTarget::Gradient })
                },
                ex("m5e5", "Borrador: limpieza", "Borra todas las manchas del rectángulo.",
                    "En Krita, la tecla E pone el pincel actual en modo borrador; púlsala otra vez para volver a pintar.",
                    "borrador", Some(Basico), Kind::Paint { target: PaintTarget::Cleanup }),
            ],
        },
        Module {
            title: "6. Valor y pintura",
            subtitle: "Sombreado, volumen y mezcla de color",
            lesson: "El valor (lo claro u oscuro) da el volumen. Antes de pintar con color, entrena a ver y reproducir valores. A la derecha verás la referencia: compárala con tu pintura.\n\nConsejo de Krita: pinta en escala de grises en una capa y luego colorea con una capa en modo Color o Multiplicar.",
            accent: egui::Color32::from_rgb(255, 210, 80),
            exercises: vec![
                Exercise {
                    color: Some([0.1, 0.1, 0.12]),
                    ..ex("m6e1", "Escala de valores", "Pinta los 5 recuadros igualando los grises de la referencia.",
                        "Con b) Basic-2 Opacity la presión decide lo oscuro del gris; prueba distintas presiones sobre el mismo recuadro.",
                        "basico2", Some(Basico), Kind::Paint { target: PaintTarget::ValueSteps })
                },
                Exercise {
                    color: Some([0.1, 0.1, 0.12]),
                    ..ex("m6e2", "Sombrear una esfera", "Pinta la esfera: luz arriba a la izquierda, sombra abajo a la derecha.",
                        "En Krita usa el selector de color avanzado para elegir grises exactos, o el atajo para tomar color del lienzo (Ctrl + clic).",
                        "aero", Some(Ponderado), Kind::Paint { target: PaintTarget::Sphere })
                },
                Exercise {
                    color: Some([0.1, 0.1, 0.12]),
                    ..ex("m6e3", "Cilindro sombreado", "Sombrea el cilindro: el borde derecho es el más oscuro.",
                        "Las luces suaves salen mejor con pinceles de borde difuso; los bordes nítidos, con uno duro.",
                        "aero", Some(Ponderado), Kind::Paint { target: PaintTarget::Cylinder })
                },
                Exercise {
                    color: Some([0.55, 0.38, 0.5]),
                    ..ex("m6e4", "Mezcla húmeda", "Funde el naranja con el azul para que pasen de uno a otro sin corte.",
                        "i) Wet Paint arrastra el color del lienzo como pintura fresca. En Krita, los pinceles de mezcla usan la opción Pintura húmeda/Difuminar.",
                        "humedo", Some(Basico), Kind::Paint { target: PaintTarget::ColorBlend })
                },
            ],
        },
        Module {
            title: "7. Memoria y soltura",
            subtitle: "La guía desaparece: dibuja de memoria",
            lesson: "La guía se ve unos segundos y luego se oculta. Obsérvala, imagina el trazo y dibújalo sin referencia. Es el paso entre copiar y dibujar con confianza.",
            accent: egui::Color32::from_rgb(120, 220, 220),
            exercises: vec![
                Exercise {
                    memory: Some(3.0),
                    ..ex("m7e1", "Círculo de memoria", "Memoriza el círculo y dibújalo cuando desaparezca.",
                        "En Krita puedes activar el modo espejo (M) para revisar tus proporciones con ojos frescos.",
                        "basico5", Some(Basico), trace(vec![g(circle(pos2(480.0, 320.0), 170.0))], 18.0))
                },
                Exercise {
                    memory: Some(3.0),
                    ..ex("m7e2", "Ola de memoria", "Memoriza la ola y repítela sin guía.",
                        "Usa el Estabilizador en Krita cuando necesites curvas largas muy limpias, pero entrena también sin él.",
                        "basico5", Some(Basico), trace(vec![g(wave(140.0, 820.0, 320.0, 110.0, 1.5))], 18.0))
                },
                Exercise {
                    memory: Some(4.0),
                    ..ex("m7e3", "Cabeza simple", "Círculo, eje vertical y línea de ojos: la base de un retrato.",
                        "En Krita, dibuja la construcción en una capa y bájale la opacidad antes de detallar.",
                        "lapiz", Some(Basico), trace(vec![
                            g(circle(pos2(480.0, 290.0), 150.0)),
                            g(line(pos2(480.0, 140.0), pos2(480.0, 560.0))),
                            g(ellipse(pos2(480.0, 300.0), 150.0, 40.0, 0.0, 0.0, PI)),
                            g(bezier(pos2(380.0, 400.0), pos2(400.0, 520.0), pos2(560.0, 520.0), pos2(580.0, 400.0))),
                        ], 16.0))
                },
            ],
        },
    ]
}

#[allow(dead_code)]
pub fn all_exercise_ids(mods: &[Module]) -> Vec<String> {
    mods.iter().flat_map(|m| m.exercises.iter().map(|e| e.id.clone())).collect()
}
