//! Vista del lienzo: acercar, desplazar, girar y voltear (como en Krita), y la transformación
//! entre coordenadas del lienzo y de la pantalla que usan el dibujo y la entrada del lápiz.

use crate::canvas::{H, W};
use egui::{Color32, Pos2, Rect, Shape, Stroke, TextureId, Vec2, pos2, vec2};

/// Estado de la vista que el usuario controla.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct View {
    /// Multiplicador sobre el tamaño que encaja el lienzo en la ventana.
    pub zoom: f32,
    /// Desplazamiento del centro del lienzo, en px de pantalla.
    pub pan: Vec2,
    /// Giro en radianes (positivo = horario en pantalla).
    pub rot: f32,
    /// Espejo horizontal (tecla M en Krita).
    pub mirror: bool,
}

impl Default for View {
    fn default() -> Self {
        Self { zoom: 1.0, pan: Vec2::ZERO, rot: 0.0, mirror: false }
    }
}

impl View {
    /// Transformación para un área de pantalla concreta.
    pub fn xf(&self, area: Rect) -> Xf {
        let fit = (area.width() / W as f32).min(area.height() / H as f32).max(0.05);
        Xf { center: area.center() + self.pan, scale: fit * self.zoom, rot: self.rot, mirror: self.mirror }
    }

    /// Acerca o aleja dejando fijo el punto de pantalla `at`.
    pub fn zoom_at(&mut self, area: Rect, at: Pos2, factor: f32) {
        let new_zoom = (self.zoom * factor).clamp(0.1, 16.0);
        let real = new_zoom / self.zoom;
        let c = area.center() + self.pan;
        self.pan += (at - c) * (1.0 - real);
        self.zoom = new_zoom;
    }

    /// Gira alrededor del punto de pantalla `at`.
    pub fn rotate_at(&mut self, area: Rect, at: Pos2, angle: f32) {
        let c = area.center() + self.pan;
        let d = rot(c - at, angle);
        self.pan = (at + d) - area.center();
        self.rot = (self.rot + angle).rem_euclid(std::f32::consts::TAU);
    }

    /// Giro en grados entre -180 y 180, para mostrarlo.
    pub fn degrees(&self) -> f32 {
        let d = self.rot.to_degrees();
        if d > 180.0 { d - 360.0 } else { d }
    }
}

fn rot(v: Vec2, a: f32) -> Vec2 {
    let (s, c) = a.sin_cos();
    vec2(v.x * c - v.y * s, v.x * s + v.y * c)
}

/// Transformación lienzo ↔ pantalla de un fotograma.
#[derive(Clone, Copy, Debug)]
pub struct Xf {
    pub center: Pos2,
    /// Px de pantalla por px de lienzo.
    pub scale: f32,
    pub rot: f32,
    pub mirror: bool,
}

impl Xf {
    pub fn to_screen(&self, q: Pos2) -> Pos2 {
        let mut d = (q - pos2(W as f32 / 2.0, H as f32 / 2.0)) * self.scale;
        if self.mirror {
            d.x = -d.x;
        }
        self.center + rot(d, self.rot)
    }

    pub fn to_canvas(&self, p: Pos2) -> Pos2 {
        let mut d = rot(p - self.center, -self.rot) / self.scale;
        if self.mirror {
            d.x = -d.x;
        }
        pos2(W as f32 / 2.0, H as f32 / 2.0) + d
    }

    /// Esquinas del lienzo en pantalla, en orden (0,0), (W,0), (W,H), (0,H).
    pub fn corners(&self) -> [Pos2; 4] {
        let (w, h) = (W as f32, H as f32);
        [pos2(0.0, 0.0), pos2(w, 0.0), pos2(w, h), pos2(0.0, h)].map(|q| self.to_screen(q))
    }

    /// Una textura del tamaño del lienzo, girada y volteada con la vista.
    pub fn image(&self, tex: TextureId) -> Shape {
        let mut mesh = egui::Mesh::with_texture(tex);
        let uv = [pos2(0.0, 0.0), pos2(1.0, 0.0), pos2(1.0, 1.0), pos2(0.0, 1.0)];
        for (p, t) in self.corners().iter().zip(uv) {
            mesh.vertices.push(egui::epaint::Vertex { pos: *p, uv: t, color: Color32::WHITE });
        }
        mesh.add_triangle(0, 1, 2);
        mesh.add_triangle(0, 2, 3);
        Shape::mesh(mesh)
    }

    /// Contorno del lienzo (para el marco y la sombra).
    pub fn frame(&self, fill: Color32, stroke: Stroke) -> Shape {
        Shape::convex_polygon(self.corners().to_vec(), fill, stroke)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn screen_and_canvas_round_trip_with_rotation_and_mirror() {
        let area = Rect::from_min_size(pos2(100.0, 50.0), vec2(1200.0, 800.0));
        let mut v = View { zoom: 1.7, pan: vec2(30.0, -20.0), rot: 0.6, mirror: true };
        let xf = v.xf(area);
        for q in [pos2(0.0, 0.0), pos2(480.0, 320.0), pos2(913.0, 17.0)] {
            assert!((xf.to_canvas(xf.to_screen(q)) - q).length() < 1e-3);
        }
        // Acercar con el ratón encima deja ese punto del lienzo en el mismo sitio.
        let at = pos2(700.0, 300.0);
        let before = v.xf(area).to_canvas(at);
        v.zoom_at(area, at, 1.8);
        assert!((v.xf(area).to_canvas(at) - before).length() < 1e-2);
        // Igual al girar.
        let before = v.xf(area).to_canvas(at);
        v.rotate_at(area, at, 0.4);
        assert!((v.xf(area).to_canvas(at) - before).length() < 1e-2);
    }
}
