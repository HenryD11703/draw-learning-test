//! Ejercicios como datos. Cada módulo es un archivo JSON de `content/` (incrustado en el
//! ejecutable) y cada ejercicio se construye con una semilla: la semilla 0 da la versión de
//! referencia y cualquier otra, una variación (posición, tamaño, giro, puntos de fuga…).
//! El formato está explicado en `content/README.md`.

use crate::canvas::{H, W};
use crate::lessons::{self, Dir, Exercise, Guide, Kind, Module, PaintTarget, Region, Role, Scene, Task};
use crate::smoothing::SmoothingType;
use egui::{Color32, Pos2, Vec2, pos2, vec2};
use serde::Deserialize;
use crate::painting::{Paint, PaintScene};
use std::collections::HashSet;
use std::sync::Arc;
use std::f32::consts::{PI, TAU};

/// Módulos en el orden en que aparecen en el mapa.
const FILES: &[(&str, &str)] = &[
    ("01_lineas.json", include_str!("../content/01_lineas.json")),
    ("02_elipses.json", include_str!("../content/02_elipses.json")),
    ("03_peso.json", include_str!("../content/03_peso.json")),
    ("04_formas.json", include_str!("../content/04_formas.json")),
    ("05_perspectiva.json", include_str!("../content/05_perspectiva.json")),
    ("06_volumen.json", include_str!("../content/06_volumen.json")),
    ("07_sombreado.json", include_str!("../content/07_sombreado.json")),
    ("08_figura.json", include_str!("../content/08_figura.json")),
    ("09_cabeza.json", include_str!("../content/09_cabeza.json")),
    ("10_manos.json", include_str!("../content/10_manos.json")),
    ("11_entintado.json", include_str!("../content/11_entintado.json")),
    ("12_observacion.json", include_str!("../content/12_observacion.json")),
    ("13_valor.json", include_str!("../content/13_valor.json")),
    ("14_luz_forma.json", include_str!("../content/14_luz_forma.json")),
    ("15_bordes.json", include_str!("../content/15_bordes.json")),
    ("16_color.json", include_str!("../content/16_color.json")),
    ("17_color_luz.json", include_str!("../content/17_color_luz.json")),
];

/// Margen mínimo entre el contenido de un ejercicio y el borde del lienzo.
const MARGIN: f32 = 12.0;

type P = [f32; 2];

fn p(a: P) -> Pos2 {
    pos2(a[0], a[1])
}

// ------------------------------------------------------------------ formato

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ModuleSpec {
    id: String,
    title: String,
    subtitle: String,
    lesson: String,
    accent: [u8; 3],
    exercises: Vec<ExerciseSpec>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ExerciseSpec {
    id: String,
    title: String,
    goal: String,
    krita_tip: String,
    brush: String,
    #[serde(default)]
    smoothing: Option<SmoothingType>,
    #[serde(default)]
    color: Option<[f32; 3]>,
    #[serde(default)]
    memory: Option<f32>,
    #[serde(default)]
    numbered: bool,
    #[serde(default)]
    time_limit: Option<f32>,
    #[serde(default)]
    vary: Vary,
    kind: KindSpec,
}

/// Variación aleatoria de todo el ejercicio: desplazamiento (px), escala y giro (grados).
/// `turn` es un giro fijo que se aplica siempre (por ejemplo 180 para copiar del revés).
#[derive(Clone, Debug, Default, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Vary {
    #[serde(default)]
    pub shift: f32,
    #[serde(default)]
    pub scale: Option<[f32; 2]>,
    #[serde(default)]
    pub rotate: f32,
    #[serde(default)]
    pub turn: f32,
}

/// Zona de un ejercicio de rayado.
#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RegionSpec {
    pts: Vec<P>,
    #[serde(default)]
    angle: Option<f32>,
    spacing: f32,
    #[serde(default)]
    cross: bool,
    #[serde(default)]
    label: Option<String>,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case", deny_unknown_fields)]
pub enum KindSpec {
    Trace {
        tol: f32,
        guides: Vec<GuideSpec>,
    },
    Points {
        tol: f32,
        guides: Vec<GuideSpec>,
    },
    Copy {
        tol: f32,
        #[serde(default)]
        guides: Vec<GuideSpec>,
        #[serde(default)]
        scene: Option<SceneSpec>,
    },
    Perspective {
        tol: f32,
        #[serde(default)]
        hints: bool,
        scene: SceneSpec,
    },
    Hatch {
        tol: f32,
        regions: Vec<RegionSpec>,
    },
    Paint {
        target: PaintTarget,
    },
    /// Pintura a partir de una escena de figuras (ver `painting.rs`).
    Painting(PaintScene),
}

#[derive(Clone, Debug, Deserialize)]
pub struct GuideSpec {
    #[serde(flatten)]
    shape: Shape,
    #[serde(default)]
    pressure: Option<Profile>,
    /// `context`: ya dibujada, no se califica. `hidden`: no se ve, pero se califica.
    #[serde(default)]
    role: Role,
}

fn yes() -> bool {
    true
}

/// Figuras de las guías. Los ángulos van en grados.
#[derive(Clone, Debug, Deserialize)]
#[serde(tag = "shape", rename_all = "snake_case")]
enum Shape {
    Line { a: P, b: P },
    /// `count` líneas iguales, cada una desplazada `step` respecto a la anterior.
    Lines { a: P, b: P, step: P, count: usize },
    /// `count` radios alrededor de `c`, de `r0` a `r1`, empezando en el ángulo `start`.
    Rays {
        c: P,
        r0: f32,
        r1: f32,
        count: usize,
        #[serde(default)]
        start: f32,
    },
    Bezier { p: [P; 4] },
    Circle { c: P, r: f32 },
    Ellipse {
        c: P,
        rx: f32,
        ry: f32,
        #[serde(default)]
        rot: f32,
        #[serde(default)]
        a0: f32,
        #[serde(default)]
        a1: Option<f32>,
    },
    Poly {
        pts: Vec<P>,
        #[serde(default)]
        closed: bool,
    },
    Spiral { c: P, r0: f32, r1: f32, turns: f32 },
    Wave { x0: f32, x1: f32, y: f32, amp: f32, cycles: f32 },
    Star { c: P, r_out: f32, r_in: f32 },
    /// Lazos encadenados de izquierda a derecha.
    Loops { x0: f32, x1: f32, y: f32, rx: f32, ry: f32, turns: f32 },
    /// Pétalos curvos alrededor de `c`.
    Petals { c: P, inner: f32, outer: f32, count: usize },
    /// Embudo: elipses a lo largo de un eje, con el eje menor sobre él (Drawabox, lección 1).
    /// `r0`/`r1` son los radios mayores en cada extremo y `d0`/`d1` el grado (alto/ancho, 0..1).
    /// `frame` es el eje y los lados; `ellipses`, las elipses (para dar uno y pedir el otro).
    Funnel {
        a: P,
        b: P,
        count: usize,
        r0: f32,
        r1: f32,
        d0: f32,
        d1: f32,
        #[serde(default = "yes")]
        frame: bool,
        #[serde(default = "yes")]
        ellipses: bool,
    },
    /// Plano en perspectiva: cuadrilátero con, opcionalmente, su círculo inscrito (que se ve como
    /// elipse), sus diagonales y sus líneas medias.
    Plane {
        quad: [P; 4],
        #[serde(default = "yes")]
        outline: bool,
        #[serde(default)]
        circle: bool,
        #[serde(default)]
        diagonals: bool,
        #[serde(default)]
        cross: bool,
    },
    /// Cilindro vertical con anillos de contorno (la mitad de delante de cada elipse intermedia).
    Cylinder {
        c: P,
        rx: f32,
        ry: f32,
        h: f32,
        #[serde(default)]
        rings: usize,
        #[serde(default = "yes")]
        outline: bool,
    },
    /// Esfera con paralelos de contorno; `tilt` es el grado de las elipses (0..1).
    Sphere {
        c: P,
        r: f32,
        tilt: f32,
        #[serde(default)]
        rings: usize,
        #[serde(default = "yes")]
        outline: bool,
    },
    /// Cono: base elíptica y dos lados hasta el vértice.
    Cone { c: P, rx: f32, ry: f32, apex: P },
    /// Figura humana esquemática (maniquí de Loomis): cabeza, columna, hombros, caderas,
    /// brazos y piernas. Con `volume`, añade caja torácica, pelvis y articulaciones.
    /// Los pares van como [izquierda, derecha] del dibujo.
    Figure {
        head: P,
        r: f32,
        neck: P,
        #[serde(default)]
        chest: Option<P>,
        hip: P,
        shoulders: [P; 2],
        hips: [P; 2],
        elbows: [P; 2],
        hands: [P; 2],
        knees: [P; 2],
        feet: [P; 2],
        #[serde(default)]
        volume: bool,
    },
    /// «Salchicha» (forma orgánica simple): dos círculos unidos y anillos de contorno.
    Sausage {
        a: P,
        b: P,
        r: f32,
        #[serde(default)]
        rings: usize,
        #[serde(default = "yes")]
        outline: bool,
    },
}

/// Presión objetivo a lo largo de la guía.
#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "snake_case")]
enum Profile {
    TaperOut,
    TaperIn,
    Spindle,
    Const(f32),
    /// Ola de presión con este número de ciclos.
    Wave(f32),
}

impl Profile {
    fn value(&self, t: f32) -> f32 {
        match *self {
            Profile::TaperOut => 1.0 - 0.85 * t,
            Profile::TaperIn => 0.15 + 0.85 * t,
            Profile::Spindle => 0.12 + 0.88 * (t * PI).sin(),
            Profile::Const(v) => v,
            Profile::Wave(c) => 0.2 + 0.75 * ((t * c * TAU).sin() * -0.5 + 0.5),
        }
    }
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Box1 {
    rect: [f32; 4],
    #[serde(default = "default_depth")]
    depth: f32,
}

fn default_depth() -> f32 {
    0.35
}

fn default_reach() -> f32 {
    0.7
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Box2 {
    x: f32,
    top: f32,
    bottom: f32,
    left: f32,
    right: f32,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Box3 {
    x: f32,
    y: f32,
    height: f32,
    left: f32,
    right: f32,
}

/// Generadores de escenas en perspectiva.
#[derive(Clone, Debug, Deserialize)]
#[serde(tag = "gen", rename_all = "snake_case", deny_unknown_fields)]
pub enum SceneSpec {
    /// Rayos desde cada punto de `from` hacia el punto de fuga.
    Converge {
        vp: P,
        from: Vec<P>,
        #[serde(default = "default_reach")]
        reach: f32,
    },
    /// Suelo en cuadrícula a un punto.
    Grid1 { vp: P, baseline: f32, x0: f32, x1: f32, cols: usize, rows: usize },
    /// Cajas a un punto: la cara frontal está dada.
    Box1 {
        vp: P,
        boxes: Vec<Box1>,
        #[serde(default)]
        back: bool,
    },
    /// Cajas a dos puntos: la arista frontal vertical está dada.
    Box2 {
        horizon: f32,
        vps: [f32; 2],
        boxes: Vec<Box2>,
        #[serde(default)]
        far: bool,
        #[serde(default)]
        xray: bool,
    },
    /// Cajas a tres puntos: la arista frontal apunta al tercer punto de fuga.
    Box3 {
        horizon: f32,
        vps: [f32; 2],
        vp3: P,
        boxes: Vec<Box3>,
        #[serde(default)]
        far: bool,
    },
}

// ------------------------------------------------------------------ carga

fn leak(s: String) -> &'static str {
    Box::leak(s.into_boxed_str())
}

pub fn modules() -> Vec<Module> {
    let brushes: HashSet<&str> = crate::brush::presets().iter().map(|b| b.id).collect();
    let mut module_ids = HashSet::new();
    let mut exercise_ids = HashSet::new();
    FILES
        .iter()
        .map(|(name, text)| {
            let m: ModuleSpec = serde_json::from_str(text).unwrap_or_else(|e| panic!("content/{name}: {e}"));
            assert!(module_ids.insert(m.id.clone()), "content/{name}: módulo repetido «{}»", m.id);
            let exercises = m
                .exercises
                .into_iter()
                .map(|e| {
                    assert!(exercise_ids.insert(e.id.clone()), "content/{name}: ejercicio repetido «{}»", e.id);
                    assert!(brushes.contains(e.brush.as_str()), "content/{name}: pincel desconocido «{}» en {}", e.brush, e.id);
                    let kind = e.kind.build(&e.vary, 0);
                    Exercise {
                        id: e.id,
                        title: leak(e.title),
                        goal: leak(e.goal),
                        krita_tip: leak(e.krita_tip),
                        brush: leak(e.brush),
                        smoothing: e.smoothing,
                        color: e.color,
                        kind,
                        memory: e.memory,
                        numbered: e.numbered,
                        time_limit: e.time_limit,
                        spec: e.kind,
                        vary: e.vary,
                    }
                })
                .collect();
            Module {
                title: leak(m.title),
                subtitle: leak(m.subtitle),
                lesson: leak(m.lesson),
                accent: Color32::from_rgb(m.accent[0], m.accent[1], m.accent[2]),
                exercises,
            }
        })
        .collect()
}

// ------------------------------------------------------------------ azar

/// Generador pseudoaleatorio pequeño (SplitMix64), para no depender de otra biblioteca.
struct Rng(u64);

impl Rng {
    fn next_u64(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^ (z >> 31)
    }

    fn range(&mut self, a: f32, b: f32) -> f32 {
        let u = (self.next_u64() >> 40) as f32 / (1u64 << 24) as f32;
        a + (b - a) * u
    }
}

/// Variación: sin generador (semilla 0) siempre devuelve el valor de referencia.
struct Jit(Option<Rng>);

impl Jit {
    fn new(seed: u64, attempt: u64) -> Self {
        if seed == 0 { Jit(None) } else { Jit(Some(Rng(seed ^ attempt.wrapping_mul(0xA24B_AED4_963E_E407)))) }
    }

    /// Desplazamiento en [-a, a]; 0 sin variación.
    fn off(&mut self, a: f32) -> f32 {
        self.0.as_mut().map_or(0.0, |r| r.range(-a, a))
    }

    /// Factor en [lo, hi]; 1 sin variación.
    fn fac(&mut self, lo: f32, hi: f32) -> f32 {
        self.0.as_mut().map_or(1.0, |r| r.range(lo, hi))
    }
}

// ------------------------------------------------------------------ construcción

impl KindSpec {
    pub fn build(&self, vary: &Vary, seed: u64) -> Kind {
        // Las variaciones que se salen del lienzo se descartan y se prueba otra.
        for attempt in 0..16 {
            let mut j = Jit::new(seed, attempt);
            let kind = self.build_with(vary, &mut j);
            if seed == 0 || in_bounds(&kind) {
                return kind;
            }
        }
        self.build_with(vary, &mut Jit(None))
    }

    fn build_with(&self, vary: &Vary, j: &mut Jit) -> Kind {
        match self {
            KindSpec::Trace { tol, guides } => Kind::Trace { guides: transform(build_guides(guides), vary, j), tol: *tol },
            KindSpec::Points { tol, guides } => Kind::Points { guides: transform(build_guides(guides), vary, j), tol: *tol },
            KindSpec::Copy { tol, guides, scene } => {
                let mut all = build_guides(guides);
                if let Some(sc) = scene {
                    let s = build_scene(sc, j, *tol, false);
                    all.extend(s.given.iter().cloned());
                    all.extend(s.tasks.iter().map(|t| {
                        let (a, b) = s.target(t);
                        guide(&[a, b])
                    }));
                }
                Kind::Copy { guides: transform(all, vary, j), tol: *tol }
            }
            KindSpec::Perspective { tol, hints, scene } => Kind::Perspective(build_scene(scene, j, *tol, *hints)),
            KindSpec::Hatch { tol, regions } => {
                // Solo desplazamiento y escala: girar cambiaría la dirección pedida.
                let s = vary.scale.map_or(1.0, |[lo, hi]| j.fac(lo, hi));
                let t = vec2(j.off(vary.shift), j.off(vary.shift));
                let c = pos2(W as f32 / 2.0, H as f32 / 2.0);
                let regions = regions
                    .iter()
                    .map(|r| Region {
                        pts: r.pts.iter().map(|q| c + (p(*q) - c) * s + t).collect(),
                        angle: r.angle,
                        spacing: r.spacing,
                        cross: r.cross,
                        label: r.label.clone(),
                    })
                    .collect();
                Kind::Hatch { regions, tol: *tol }
            }
            KindSpec::Paint { target } => Kind::Paint(Paint::Legacy(*target)),
            KindSpec::Painting(scene) => Kind::Paint(Paint::Scene(Arc::new(scene.clone()))),
        }
    }
}

fn in_bounds(kind: &Kind) -> bool {
    let ok = |q: Pos2| q.x >= MARGIN && q.y >= MARGIN && q.x <= W as f32 - MARGIN && q.y <= H as f32 - MARGIN;
    match kind {
        Kind::Trace { guides, .. } | Kind::Points { guides, .. } | Kind::Copy { guides, .. } => {
            guides.iter().all(|g| g.pts.iter().all(|q| ok(*q)))
        }
        Kind::Perspective(s) => {
            s.given.iter().all(|g| g.pts.iter().all(|q| ok(*q)))
                && s.tasks.iter().all(|t| {
                    let (a, b) = s.target(t);
                    ok(a) && ok(b)
                })
        }
        Kind::Hatch { regions, .. } => regions.iter().all(|r| r.pts.iter().all(|q| ok(*q))),
        Kind::Paint(_) => true,
    }
}

fn guide(pts: &[Pos2]) -> Guide {
    Guide { pts: lessons::resample(pts), pressure: None, role: Role::Trace }
}

fn build_guides(specs: &[GuideSpec]) -> Vec<Guide> {
    let mut out = Vec::new();
    for s in specs {
        for line in s.shape.polylines() {
            let pts = lessons::resample(&line);
            let pressure = s.pressure.as_ref().map(|pr| {
                let n = pts.len().max(2) as f32 - 1.0;
                (0..pts.len()).map(|i| pr.value(i as f32 / n)).collect()
            });
            out.push(Guide { pts, pressure, role: s.role });
        }
    }
    out
}

/// Desplaza, escala y gira todas las guías alrededor del centro del lienzo.
fn transform(guides: Vec<Guide>, vary: &Vary, j: &mut Jit) -> Vec<Guide> {
    let s = vary.scale.map_or(1.0, |[lo, hi]| j.fac(lo, hi));
    let rot = (vary.turn + j.off(vary.rotate)).to_radians();
    let t = vec2(j.off(vary.shift), j.off(vary.shift));
    if s == 1.0 && rot == 0.0 && t == Vec2::ZERO {
        return guides;
    }
    let c = pos2(W as f32 / 2.0, H as f32 / 2.0);
    let (sn, cs) = rot.sin_cos();
    let f = |q: Pos2| {
        let d = (q - c) * s;
        c + vec2(d.x * cs - d.y * sn, d.x * sn + d.y * cs) + t
    };
    guides.into_iter().map(|g| Guide { pts: g.pts.into_iter().map(f).collect(), pressure: g.pressure, role: g.role }).collect()
}

impl Shape {
    fn polylines(&self) -> Vec<Vec<Pos2>> {
        match self {
            Shape::Line { a, b } => vec![vec![p(*a), p(*b)]],
            Shape::Lines { a, b, step, count } => (0..*count)
                .map(|i| {
                    let d = vec2(step[0], step[1]) * i as f32;
                    vec![p(*a) + d, p(*b) + d]
                })
                .collect(),
            Shape::Rays { c, r0, r1, count, start } => (0..*count)
                .map(|k| {
                    let a = start.to_radians() + k as f32 * TAU / *count as f32;
                    let d = vec2(a.cos(), a.sin());
                    vec![p(*c) + d * *r0, p(*c) + d * *r1]
                })
                .collect(),
            Shape::Bezier { p: q } => vec![lessons::bezier(p(q[0]), p(q[1]), p(q[2]), p(q[3]))],
            Shape::Circle { c, r } => vec![lessons::circle(p(*c), *r)],
            Shape::Ellipse { c, rx, ry, rot, a0, a1 } => vec![lessons::ellipse(
                p(*c),
                *rx,
                *ry,
                rot.to_radians(),
                a0.to_radians(),
                a1.map_or(TAU, |a| a.to_radians()),
            )],
            Shape::Poly { pts, closed } => {
                let mut v: Vec<Pos2> = pts.iter().map(|a| p(*a)).collect();
                if *closed && !v.is_empty() {
                    v.push(v[0]);
                }
                vec![v]
            }
            Shape::Spiral { c, r0, r1, turns } => vec![lessons::spiral(p(*c), *r0, *r1, *turns)],
            Shape::Wave { x0, x1, y, amp, cycles } => vec![lessons::wave(*x0, *x1, *y, *amp, *cycles)],
            Shape::Star { c, r_out, r_in } => vec![lessons::star(p(*c), *r_out, *r_in)],
            Shape::Loops { x0, x1, y, rx, ry, turns } => {
                let n = (turns * 100.0).round().max(1.0) as usize;
                vec![
                    (0..=n)
                        .map(|i| {
                            let t = i as f32 / n as f32;
                            let a = t * turns * TAU;
                            pos2(x0 + t * (x1 - x0) + rx * a.cos(), y + ry * a.sin())
                        })
                        .collect(),
                ]
            }
            Shape::Petals { c, inner, outer, count } => (0..*count)
                .map(|k| {
                    let a = k as f32 * TAU / *count as f32;
                    let d = vec2(a.cos(), a.sin());
                    let n = vec2(-d.y, d.x);
                    let c = p(*c);
                    let p0 = c + d * *inner;
                    let tip = c + d * *outer;
                    lessons::bezier(p0, p0 + d * 50.0 + n * 60.0, tip + n * 30.0, tip)
                })
                .collect(),
            Shape::Funnel { a, b, count, r0, r1, d0, d1, frame, ellipses } => {
                let (a, b) = (p(*a), p(*b));
                let axis = (b - a).normalized();
                let perp = vec2(-axis.y, axis.x);
                // El eje mayor de cada elipse va perpendicular al eje del embudo.
                let rot = perp.y.atan2(perp.x);
                let n = (*count).max(2);
                let mut out = Vec::new();
                let mut ends = Vec::new();
                for i in 0..n {
                    let t = i as f32 / (n - 1) as f32;
                    let c = a + (b - a) * t;
                    let rx = r0 + (r1 - r0) * t;
                    if *ellipses {
                        out.push(lessons::ellipse(c, rx, rx * (d0 + (d1 - d0) * t), rot, 0.0, TAU));
                    }
                    ends.push((c - perp * rx, c + perp * rx));
                }
                if *frame {
                    out.push(vec![a, b]);
                    out.push(vec![ends[0].0, ends[n - 1].0]);
                    out.push(vec![ends[0].1, ends[n - 1].1]);
                }
                out
            }
            Shape::Plane { quad, outline, circle, diagonals, cross } => {
                let q: Vec<Pos2> = quad.iter().map(|x| p(*x)).collect();
                let h = homography(&q);
                let mut out = if *outline { vec![vec![q[0], q[1], q[2], q[3], q[0]]] } else { Vec::new() };
                if *diagonals {
                    out.push(vec![q[0], q[2]]);
                    out.push(vec![q[1], q[3]]);
                }
                if *cross {
                    out.push(vec![h(0.5, 0.0), h(0.5, 1.0)]);
                    out.push(vec![h(0.0, 0.5), h(1.0, 0.5)]);
                }
                if *circle {
                    out.push(
                        (0..=160)
                            .map(|i| {
                                let t = i as f32 / 160.0 * TAU;
                                h(0.5 + 0.5 * t.cos(), 0.5 + 0.5 * t.sin())
                            })
                            .collect(),
                    );
                }
                out
            }
            Shape::Cylinder { c, rx, ry, h, rings, outline } => {
                let c = p(*c);
                let bot = c + vec2(0.0, *h);
                let mut out = if *outline {
                    vec![
                        lessons::ellipse(c, *rx, *ry, 0.0, 0.0, TAU),
                        lessons::ellipse(bot, *rx, *ry, 0.0, 0.0, TAU),
                        vec![c - vec2(*rx, 0.0), bot - vec2(*rx, 0.0)],
                        vec![c + vec2(*rx, 0.0), bot + vec2(*rx, 0.0)],
                    ]
                } else {
                    Vec::new()
                };
                for k in 1..=*rings {
                    let t = k as f32 / (*rings + 1) as f32;
                    // Solo la mitad de delante: es la que se ve.
                    out.push(lessons::ellipse(c + vec2(0.0, h * t), *rx, *ry, 0.0, 0.0, PI));
                }
                out
            }
            Shape::Sphere { c, r, tilt, rings, outline } => {
                let c = p(*c);
                let mut out = if *outline { vec![lessons::circle(c, *r)] } else { Vec::new() };
                for k in 0..*rings {
                    let lat = (-1.0 + 2.0 * (k as f32 + 0.5) / *rings as f32) * PI / 3.0;
                    let rx = r * lat.cos();
                    out.push(lessons::ellipse(c + vec2(0.0, r * lat.sin()), rx, rx * tilt, 0.0, 0.0, PI));
                }
                out
            }
            Shape::Cone { c, rx, ry, apex } => {
                let (c, ap) = (p(*c), p(*apex));
                vec![lessons::ellipse(c, *rx, *ry, 0.0, 0.0, TAU), vec![c - vec2(*rx, 0.0), ap], vec![c + vec2(*rx, 0.0), ap]]
            }
            Shape::Figure { head, r, neck, chest, hip, shoulders, hips, elbows, hands, knees, feet, volume } => {
                let (neck, hip) = (p(*neck), p(*hip));
                let spine = match chest {
                    Some(c) => lessons::bezier(neck, p(*c), p(*c), hip),
                    None => vec![neck, hip],
                };
                let mut out = vec![lessons::circle(p(*head), *r), spine, vec![p(shoulders[0]), p(shoulders[1])], vec![p(hips[0]), p(hips[1])]];
                for i in 0..2 {
                    out.push(vec![p(shoulders[i]), p(elbows[i]), p(hands[i])]);
                    out.push(vec![p(hips[i]), p(knees[i]), p(feet[i])]);
                }
                if *volume {
                    // Caja torácica y pelvis como óvalos orientados con la columna.
                    let d = hip - neck;
                    let rot = d.y.atan2(d.x) - PI / 2.0;
                    let len = d.length();
                    let sw = (p(shoulders[1]) - p(shoulders[0])).length();
                    let hw = (p(hips[1]) - p(hips[0])).length();
                    out.push(lessons::ellipse(neck + d * 0.3, sw * 0.42, len * 0.3, rot, 0.0, TAU));
                    out.push(lessons::ellipse(hip - d * 0.02, hw * 0.75, len * 0.13, rot, 0.0, TAU));
                    for j in [elbows[0], elbows[1], knees[0], knees[1]] {
                        out.push(lessons::circle(p(j), r * 0.2));
                    }
                }
                out
            }
            Shape::Sausage { a, b, r, rings, outline } => {
                let (a, b) = (p(*a), p(*b));
                let axis = (b - a).normalized();
                let perp = vec2(-axis.y, axis.x);
                let rot = perp.y.atan2(perp.x);
                let mut out = if *outline {
                    vec![lessons::circle(a, *r), lessons::circle(b, *r), vec![a + perp * *r, b + perp * *r], vec![a - perp * *r, b - perp * *r]]
                } else {
                    Vec::new()
                };
                for k in 1..=*rings {
                    let t = k as f32 / (*rings + 1) as f32;
                    out.push(lessons::ellipse(a + (b - a) * t, *r, r * 0.35, rot, 0.0, PI));
                }
                out
            }
        }
    }
}

/// Proyección del cuadrado unidad sobre un cuadrilátero (Heckbert):
/// (0,0)→q0, (1,0)→q1, (1,1)→q2, (0,1)→q3. Con ella, un círculo inscrito se ve en perspectiva.
fn homography(q: &[Pos2]) -> impl Fn(f32, f32) -> Pos2 {
    let (x0, y0, x1, y1, x2, y2, x3, y3) = (q[0].x, q[0].y, q[1].x, q[1].y, q[2].x, q[2].y, q[3].x, q[3].y);
    let (dx1, dx2, dx3) = (x1 - x2, x3 - x2, x0 - x1 + x2 - x3);
    let (dy1, dy2, dy3) = (y1 - y2, y3 - y2, y0 - y1 + y2 - y3);
    let den = dx1 * dy2 - dx2 * dy1;
    let (g, h) = if den.abs() < 1e-6 { (0.0, 0.0) } else { ((dx3 * dy2 - dx2 * dy3) / den, (dx1 * dy3 - dx3 * dy1) / den) };
    let (a, b, c) = (x1 - x0 + g * x1, x3 - x0 + h * x3, x0);
    let (d, e, f) = (y1 - y0 + g * y1, y3 - y0 + h * y3, y0);
    move |u: f32, v: f32| {
        let w = g * u + h * v + 1.0;
        pos2((a * u + b * v + c) / w, (d * u + e * v + f) / w)
    }
}

/// Intersección de la recta a→a2 con la recta b→b2.
fn intersect(a: Pos2, a2: Pos2, b: Pos2, b2: Pos2) -> Pos2 {
    let r = a2 - a;
    let s = b2 - b;
    let den = r.x * s.y - r.y * s.x;
    if den.abs() < 1e-6 {
        return a2;
    }
    let t = ((b - a).x * s.y - (b - a).y * s.x) / den;
    a + r * t
}

fn build_scene(spec: &SceneSpec, j: &mut Jit, tol: f32, hints: bool) -> Scene {
    let mut s = Scene { horizon: None, vps: Vec::new(), given: Vec::new(), tasks: Vec::new(), tol, hints };
    let task = |from: Pos2, dir: Dir, to: Pos2| Task { from, dir, len: (to - from).length() };
    match spec {
        SceneSpec::Converge { vp, from, reach } => {
            let vp = p(*vp) + vec2(j.off(60.0), j.off(25.0));
            s.horizon = Some(vp.y);
            s.vps.push(vp);
            for f in from {
                let a = p(*f) + vec2(j.off(20.0), j.off(20.0));
                s.tasks.push(Task { from: a, dir: Dir::Vp(0), len: (vp - a).length() * reach });
            }
        }
        SceneSpec::Grid1 { vp, baseline, x0, x1, cols, rows } => {
            let vp = p(*vp) + vec2(j.off(80.0), j.off(20.0));
            let b = baseline + j.off(15.0);
            s.horizon = Some(vp.y);
            s.vps.push(vp);
            s.given.push(guide(&[pos2(*x0, b), pos2(*x1, b)]));
            // Fracción del camino hacia el PF a la que queda cada fila (las filas se juntan al alejarse).
            let depth = |k: usize| 1.0 - 1.0 / (1.0 + k as f32 * 0.35);
            let far = depth(*rows);
            for i in 0..=*cols {
                let a = pos2(x0 + (x1 - x0) * i as f32 / *cols as f32, b);
                s.tasks.push(Task { from: a, dir: Dir::Vp(0), len: (vp - a).length() * far });
            }
            for k in 1..=*rows {
                let f = depth(k);
                let left = pos2(*x0, b) + (vp - pos2(*x0, b)) * f;
                let right = pos2(*x1, b) + (vp - pos2(*x1, b)) * f;
                s.tasks.push(Task { from: left, dir: Dir::Horizontal, len: right.x - left.x });
            }
        }
        SceneSpec::Box1 { vp, boxes, back } => {
            let vp = p(*vp) + vec2(j.off(80.0), j.off(40.0));
            s.horizon = Some(vp.y);
            s.vps.push(vp);
            for bx in boxes {
                let [x0, y0, x1, y1] = bx.rect;
                let c = pos2((x0 + x1) / 2.0 + j.off(40.0), (y0 + y1) / 2.0 + j.off(40.0));
                let k = j.fac(0.85, 1.15);
                let (hw, hh) = ((x1 - x0) / 2.0 * k, (y1 - y0) / 2.0 * k);
                let depth = bx.depth * j.fac(0.85, 1.15);
                let corners = [c + vec2(-hw, -hh), c + vec2(hw, -hh), c + vec2(hw, hh), c + vec2(-hw, hh)];
                for i in 0..4 {
                    s.given.push(guide(&[corners[i], corners[(i + 1) % 4]]));
                }
                for q in corners {
                    s.tasks.push(Task { from: q, dir: Dir::Vp(0), len: (vp - q).length() * depth });
                }
                if *back {
                    let bc: Vec<Pos2> = corners.iter().map(|q| *q + (vp - *q) * depth).collect();
                    s.tasks.push(task(bc[0], Dir::Horizontal, bc[1]));
                    s.tasks.push(task(bc[3], Dir::Horizontal, bc[2]));
                    s.tasks.push(task(bc[0], Dir::Vertical, bc[3]));
                    s.tasks.push(task(bc[1], Dir::Vertical, bc[2]));
                }
            }
        }
        SceneSpec::Box2 { horizon, vps, boxes, far, xray } => {
            let h = horizon + j.off(25.0);
            let vl = pos2(vps[0] + j.off(50.0), h);
            let vr = pos2(vps[1] + j.off(50.0), h);
            s.horizon = Some(h);
            s.vps = vec![vl, vr];
            for bx in boxes {
                let x = bx.x + j.off(40.0);
                let mid = (bx.top + bx.bottom) / 2.0 + j.off(25.0);
                let half = (bx.bottom - bx.top) / 2.0 * j.fac(0.9, 1.1);
                let (ft, fb) = (pos2(x, mid - half), pos2(x, mid + half));
                let (l, r) = (bx.left * j.fac(0.85, 1.15), bx.right * j.fac(0.85, 1.15));
                let (lt, lb) = (ft + (vl - ft) * l, fb + (vl - fb) * l);
                let (rt, rb) = (ft + (vr - ft) * r, fb + (vr - fb) * r);
                s.given.push(guide(&[ft, fb]));
                s.tasks.push(task(ft, Dir::Vp(0), lt));
                s.tasks.push(task(fb, Dir::Vp(0), lb));
                s.tasks.push(task(ft, Dir::Vp(1), rt));
                s.tasks.push(task(fb, Dir::Vp(1), rb));
                s.tasks.push(task(lt, Dir::Vertical, lb));
                s.tasks.push(task(rt, Dir::Vertical, rb));
                let bt = intersect(lt, vr, rt, vl);
                let bb = intersect(lb, vr, rb, vl);
                // Se ve la cara de arriba si la caja está bajo el horizonte, y la de abajo si está encima.
                if (*far && ft.y > h) || *xray {
                    s.tasks.push(task(lt, Dir::Vp(1), bt));
                    s.tasks.push(task(rt, Dir::Vp(0), bt));
                }
                if (*far && fb.y < h) || *xray {
                    s.tasks.push(task(lb, Dir::Vp(1), bb));
                    s.tasks.push(task(rb, Dir::Vp(0), bb));
                }
                if *xray {
                    s.tasks.push(task(bt, Dir::Vertical, bb));
                }
            }
        }
        SceneSpec::Box3 { horizon, vps, vp3, boxes, far } => {
            let h = horizon + j.off(20.0);
            let vl = pos2(vps[0] + j.off(50.0), h);
            let vr = pos2(vps[1] + j.off(50.0), h);
            let v3 = p(*vp3) + vec2(j.off(40.0), 0.0);
            s.horizon = Some(h);
            s.vps = vec![vl, vr, v3];
            for bx in boxes {
                // `a` es el extremo de la arista frontal más cercano al horizonte; `o`, el otro.
                let a = pos2(bx.x + j.off(40.0), bx.y + j.off(20.0));
                let o = a + (v3 - a).normalized() * bx.height * j.fac(0.9, 1.1);
                let (l, r) = (bx.left * j.fac(0.85, 1.15), bx.right * j.fac(0.85, 1.15));
                let (la, ra) = (a + (vl - a) * l, a + (vr - a) * r);
                let lo = intersect(la, v3, o, vl);
                let ro = intersect(ra, v3, o, vr);
                s.given.push(guide(&[a, o]));
                s.tasks.push(task(a, Dir::Vp(0), la));
                s.tasks.push(task(a, Dir::Vp(1), ra));
                s.tasks.push(task(o, Dir::Vp(0), lo));
                s.tasks.push(task(o, Dir::Vp(1), ro));
                s.tasks.push(task(la, Dir::Vp(2), lo));
                s.tasks.push(task(ra, Dir::Vp(2), ro));
                if *far {
                    let fa = intersect(la, vr, ra, vl);
                    s.tasks.push(task(la, Dir::Vp(1), fa));
                    s.tasks.push(task(ra, Dir::Vp(0), fa));
                }
            }
        }
    }
    s
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_exercise_builds_inside_the_canvas() {
        for m in modules() {
            for e in &m.exercises {
                for seed in [0u64, 1, 2, 3, 17, 99, 12345, 987654321] {
                    let k = e.build(seed);
                    assert!(in_bounds(&k), "{} con semilla {seed} se sale del lienzo", e.id);
                    match &k {
                        Kind::Trace { guides, .. } | Kind::Points { guides, .. } | Kind::Copy { guides, .. } => {
                            assert!(!guides.is_empty(), "{} sin guías", e.id)
                        }
                        Kind::Perspective(s) => {
                            assert!(!s.tasks.is_empty(), "{} sin tareas", e.id);
                            assert!(s.tasks.iter().all(|t| t.len > 10.0), "{} tiene tareas demasiado cortas", e.id);
                        }
                        Kind::Hatch { regions, .. } => {
                            assert!(!regions.is_empty(), "{} sin zonas", e.id);
                            for r in regions {
                                assert!(r.pts.len() >= 3 && r.spacing >= 3.0, "{}: zona mal definida", e.id);
                                assert!(!r.ideal_lines().is_empty(), "{}: zona sin líneas posibles", e.id);
                            }
                        }
                        Kind::Paint(_) => {}
                    }
                }
            }
        }
    }

    #[test]
    fn variations_change_the_exercise_but_seed_zero_does_not() {
        let mods = modules();
        let e = mods.iter().flat_map(|m| &m.exercises).find(|e| e.id == "persp11").expect("persp11");
        let first = |k: &Kind| match k {
            Kind::Perspective(s) => s.tasks[0].from,
            _ => unreachable!(),
        };
        assert_eq!(first(&e.build(0)), first(&e.build(0)));
        assert_ne!(first(&e.build(0)), first(&e.build(7)));
    }

    #[test]
    fn homography_maps_the_unit_square_onto_the_quad() {
        let q = [pos2(100.0, 400.0), pos2(500.0, 380.0), pos2(420.0, 250.0), pos2(180.0, 260.0)];
        let h = homography(&q);
        for (uv, want) in [((0.0, 0.0), q[0]), ((1.0, 0.0), q[1]), ((1.0, 1.0), q[2]), ((0.0, 1.0), q[3])] {
            assert!((h(uv.0, uv.1) - want).length() < 1e-2, "{uv:?}");
        }
        // El centro del cuadrado cae en el cruce de las diagonales del cuadrilátero.
        let x = intersect(q[0], q[2], q[1], q[3]);
        assert!((h(0.5, 0.5) - x).length() < 0.5, "{:?} vs {x:?}", h(0.5, 0.5));
    }

    #[test]
    fn two_point_box_edges_meet_at_the_far_corner() {
        let spec = SceneSpec::Box2 {
            horizon: 200.0,
            vps: [60.0, 900.0],
            boxes: vec![Box2 { x: 480.0, top: 300.0, bottom: 520.0, left: 0.35, right: 0.35 }],
            far: true,
            xray: false,
        };
        let s = build_scene(&spec, &mut Jit(None), 12.0, false);
        // Las dos aristas lejanas de la cara de arriba terminan en el mismo punto.
        let (_, e1) = s.target(&s.tasks[6]);
        let (_, e2) = s.target(&s.tasks[7]);
        assert!((e1 - e2).length() < 0.5, "{e1:?} vs {e2:?}");
        // Y las verticales siguen verticales.
        assert_eq!(s.dir_at(s.tasks[4].from, s.tasks[4].dir), vec2(0.0, 1.0));
    }
}
