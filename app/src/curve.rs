//! Curvas de respuesta (presión, velocidad), al estilo de la curva cúbica de Krita.
//! Puntos en [0,1]x[0,1], interpolados con una spline cúbica natural y muestreados
//! en una tabla de 1025 valores que se lee con interpolación lineal (ver MECHANICS.md §2).

use serde::{Deserialize, Serialize};

pub const LUT_SIZE: usize = 1025;

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct Curve {
    pub points: Vec<[f32; 2]>,
    #[serde(skip)]
    lut: Vec<f32>,
}

impl Curve {
    pub fn new(mut points: Vec<[f32; 2]>) -> Self {
        points.sort_by(|a, b| a[0].total_cmp(&b[0]));
        let mut c = Self { points, lut: Vec::new() };
        c.rebuild();
        c
    }

    pub fn linear() -> Self {
        Self::new(vec![[0.0, 0.0], [1.0, 1.0]])
    }

    /// Lee una curva en el formato de texto de Krita: "0,0;1,1;".
    pub fn parse(text: &str) -> Self {
        let pts = text
            .split(';')
            .filter_map(|p| {
                let mut it = p.split(',');
                let x = it.next()?.trim().parse::<f32>().ok()?;
                let y = it.next()?.trim().parse::<f32>().ok()?;
                Some([x, y])
            })
            .collect::<Vec<_>>();
        if pts.len() < 2 { Self::linear() } else { Self::new(pts) }
    }

    pub fn to_krita_string(&self) -> String {
        self.points.iter().map(|p| format!("{},{};", p[0], p[1])).collect()
    }

    pub fn rebuild(&mut self) {
        self.points.sort_by(|a, b| a[0].total_cmp(&b[0]));
        for p in &mut self.points {
            p[0] = p[0].clamp(0.0, 1.0);
            p[1] = p[1].clamp(0.0, 1.0);
        }
        self.lut = (0..LUT_SIZE)
            .map(|i| self.spline_value(i as f32 / (LUT_SIZE - 1) as f32))
            .collect();
    }

    fn ensure(&self) -> bool {
        self.lut.len() == LUT_SIZE
    }

    /// Valor de la curva usando la tabla de 1025 muestras con interpolación lineal.
    pub fn value(&self, x: f32) -> f32 {
        if !self.ensure() {
            return self.spline_value(x);
        }
        let x = x.clamp(0.0, 1.0) * (LUT_SIZE - 1) as f32;
        let i = (x.floor() as usize).min(LUT_SIZE - 2);
        let t = x - i as f32;
        self.lut[i] * (1.0 - t) + self.lut[i + 1] * t
    }

    /// Spline cúbica natural a través de los puntos.
    fn spline_value(&self, x: f32) -> f32 {
        let p = &self.points;
        let n = p.len();
        if n == 0 {
            return x;
        }
        if n == 1 {
            return p[0][1];
        }
        if x <= p[0][0] {
            return p[0][1].clamp(0.0, 1.0);
        }
        if x >= p[n - 1][0] {
            return p[n - 1][1].clamp(0.0, 1.0);
        }
        if n == 2 {
            let t = (x - p[0][0]) / (p[1][0] - p[0][0]).max(1e-6);
            return (p[0][1] + (p[1][1] - p[0][1]) * t).clamp(0.0, 1.0);
        }
        // Segundas derivadas de la spline natural (algoritmo de Thomas).
        let mut h = vec![0f32; n - 1];
        for i in 0..n - 1 {
            h[i] = (p[i + 1][0] - p[i][0]).max(1e-6);
        }
        let mut alpha = vec![0f32; n];
        for i in 1..n - 1 {
            alpha[i] = 3.0 / h[i] * (p[i + 1][1] - p[i][1]) - 3.0 / h[i - 1] * (p[i][1] - p[i - 1][1]);
        }
        let mut l = vec![1f32; n];
        let mut mu = vec![0f32; n];
        let mut z = vec![0f32; n];
        for i in 1..n - 1 {
            l[i] = 2.0 * (p[i + 1][0] - p[i - 1][0]) - h[i - 1] * mu[i - 1];
            mu[i] = h[i] / l[i];
            z[i] = (alpha[i] - h[i - 1] * z[i - 1]) / l[i];
        }
        let mut c = vec![0f32; n];
        let mut b = vec![0f32; n - 1];
        let mut d = vec![0f32; n - 1];
        for j in (0..n - 1).rev() {
            c[j] = z[j] - mu[j] * c[j + 1];
            b[j] = (p[j + 1][1] - p[j][1]) / h[j] - h[j] * (c[j + 1] + 2.0 * c[j]) / 3.0;
            d[j] = (c[j + 1] - c[j]) / (3.0 * h[j]);
        }
        let mut seg = 0;
        while seg < n - 2 && x > p[seg + 1][0] {
            seg += 1;
        }
        let dx = x - p[seg][0];
        (p[seg][1] + b[seg] * dx + c[seg] * dx * dx + d[seg] * dx * dx * dx).clamp(0.0, 1.0)
    }
}

impl Default for Curve {
    fn default() -> Self {
        Self::linear()
    }
}
