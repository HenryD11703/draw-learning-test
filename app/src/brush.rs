//! Pinceles basados en los preajustes por defecto de Krita (MECHANICS.md §3).
//! Las curvas de los sensores, la dureza (hfade) y el espaciado vienen de los archivos .kpp
//! del paquete "Krita 4 Default Resources"; los tamaños están escalados al lienzo de la app.

use crate::curve::Curve;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BrushKind {
    Round,
    Pixel,
    Eraser,
}

#[derive(Clone, Debug)]
pub struct BrushPreset {
    pub id: &'static str,
    /// Nombre del preajuste en Krita (categoría con letra, como en sus preajustes).
    pub krita_name: &'static str,
    pub name: &'static str,
    pub category: &'static str,
    pub description: &'static str,
    pub kind: BrushKind,
    pub size: f32,
    pub opacity: f32,
    pub flow: f32,
    /// 1 = borde duro, 0 = borde muy difuminado (el "hfade" de Krita).
    pub hardness: f32,
    pub spacing: f32,
    pub size_pressure: Option<Curve>,
    pub size_speed: Option<Curve>,
    pub opacity_pressure: Option<Curve>,
    pub flow_pressure: Option<Curve>,
    /// Textura de grano tipo papel (0..1).
    pub grain: f32,
    /// Cuánto color toma del lienzo (pincel húmedo, 0..1).
    pub smudge: f32,
    pub smudge_pressure: Option<Curve>,
    pub default_color: Option<[f32; 3]>,
}

impl BrushPreset {
    pub fn size_factor(&self, pressure: f32, speed: f32) -> f32 {
        let mut f = 1.0;
        if let Some(c) = &self.size_pressure {
            f *= c.value(pressure);
        }
        if let Some(c) = &self.size_speed {
            f *= c.value(speed);
        }
        f
    }
    pub fn opacity_at(&self, pressure: f32) -> f32 {
        self.opacity * self.opacity_pressure.as_ref().map_or(1.0, |c| c.value(pressure))
    }
    pub fn flow_at(&self, pressure: f32) -> f32 {
        self.flow * self.flow_pressure.as_ref().map_or(1.0, |c| c.value(pressure))
    }
    pub fn smudge_at(&self, pressure: f32) -> f32 {
        self.smudge * self.smudge_pressure.as_ref().map_or(1.0, |c| c.value(pressure))
    }
}

fn base(id: &'static str, krita_name: &'static str, name: &'static str, category: &'static str, description: &'static str) -> BrushPreset {
    BrushPreset {
        id,
        krita_name,
        name,
        category,
        description,
        kind: BrushKind::Round,
        size: 12.0,
        opacity: 1.0,
        flow: 1.0,
        hardness: 0.5,
        spacing: 0.1,
        size_pressure: None,
        size_speed: None,
        opacity_pressure: None,
        flow_pressure: None,
        grain: 0.0,
        smudge: 0.0,
        smudge_pressure: None,
        default_color: None,
    }
}

fn c(s: &str) -> Option<Curve> {
    Some(Curve::parse(s))
}

pub fn presets() -> Vec<BrushPreset> {
    vec![
        BrushPreset {
            kind: BrushKind::Eraser,
            size: 40.0,
            hardness: 0.87,
            size_pressure: c("0,0;0.413655,0.333333;0.674699,0.772125;1,1;"),
            ..base("borrador", "a) Eraser Circle", "Borrador círculo", "a) Borradores",
                "Borra con un círculo de borde firme; la presión cambia su tamaño. En Krita la tecla E convierte cualquier pincel en borrador.")
        },
        BrushPreset {
            size: 20.0,
            size_pressure: c("0,0;0.35,0.1;1,1;"),
            ..base("basico5", "b) Basic-5 Size", "Básico 5 – Tamaño", "b) Básicos",
                "El pincel por defecto de Krita: la presión cambia el grosor (poco al principio, mucho al final). Perfecto para practicar el control del trazo.")
        },
        BrushPreset {
            size: 26.0,
            opacity_pressure: c("0,0;1,1;"),
            ..base("basico2", "b) Basic-2 Opacity", "Básico 2 – Opacidad", "b) Básicos",
                "Grosor fijo; la presión controla lo opaco que queda el trazo. Bueno para sombrear por capas.")
        },
        BrushPreset {
            size: 90.0,
            hardness: 0.0,
            flow: 0.12,
            opacity_pressure: c("0,0;0.675298,0.578529;0.859438,1;"),
            ..base("aero", "b) Airbrush Soft", "Aerógrafo suave", "b) Básicos",
                "Borde totalmente difuminado: el color se acumula al pasar varias veces. Para degradados y luces.")
        },
        BrushPreset {
            size: 5.0,
            grain: 0.55,
            size_pressure: c("0,0.485944;1,1;"),
            opacity_pressure: c("0,0;0.128414,0.100402;0.473896,0.710843;1,1;"),
            default_color: Some([0.18, 0.18, 0.2]),
            ..base("lapiz", "c) Pencil-2", "Lápiz 2", "c) Lápices",
                "Imita un lápiz de grafito con grano de papel. Presiona suave para bocetar y fuerte para remarcar.")
        },
        BrushPreset {
            size: 5.0,
            hardness: 0.5,
            spacing: 0.1,
            size_pressure: c("0,0.753769;1,1;"),
            size_speed: c("0,1;1,0;"),
            default_color: Some([0.05, 0.05, 0.07]),
            ..base("fineliner", "d) Ink-2 Fineliner", "Tinta 2 – Fineliner", "d) Tintas",
                "Rotulador de tinta fino: casi no varía con la presión y adelgaza si vas muy rápido. Para entintar limpio.")
        },
        BrushPreset {
            size: 14.0,
            flow: 0.6,
            size_pressure: c("0,0.0703518;0.317328,0.234952;0.650602,0.838428;1,1;"),
            flow_pressure: c("0.00518135,0.69869;0.605261,0.899563;1,1;"),
            default_color: Some([0.05, 0.05, 0.07]),
            ..base("gpen", "d) Ink-3 Gpen", "Tinta 3 – Plumilla", "d) Tintas",
                "Plumilla de cómic: del hilo fino al trazo grueso según la presión. Es el pincel de los trazos con intención.")
        },
        BrushPreset {
            size: 20.0,
            hardness: 0.66,
            spacing: 0.2,
            size_pressure: c("0,0.346206;0.35743,0.524017;0.759036,1;"),
            opacity_pressure: c("0,0;0.347458,0.401747;0.496346,0.74141;0.745614,1;"),
            flow_pressure: c("0,0;0.759036,1;"),
            default_color: Some([0.2, 0.45, 0.85]),
            ..base("marcador", "e) Marker Details", "Rotulador", "e) Rotuladores",
                "Rotulador de detalle: con poca presión deja color translúcido y fino; a tres cuartos de presión ya es opaco.")
        },
        BrushPreset {
            size: 34.0,
            spacing: 0.07,
            smudge: 0.8,
            size_pressure: c("0,0.122271;0.136546,0.290033;0.269076,0.803493;0.333333,0.921397;0.39759,0.956332;1,1;"),
            opacity_pressure: c("0,0.15261;0.060241,0.261044;0.165254,0.419214;0.228814,0.480349;0.300847,0.528384;0.88755,1;"),
            smudge_pressure: c("0,0.0803213;0.128515,0.28514;0.706827,1;1,1;"),
            default_color: Some([0.85, 0.35, 0.2]),
            ..base("humedo", "i) Wet Paint", "Pintura húmeda", "i) Húmedos",
                "Arrastra y mezcla el color que ya hay en el lienzo, como óleo fresco; cuanto más presionas, más arrastra.")
        },
        BrushPreset {
            size: 36.0,
            hardness: 0.0,
            spacing: 0.08,
            flow: 0.5,
            smudge: 1.0,
            opacity_pressure: c("0,0;1,1;"),
            ..base("difuminar", "k) Blender Basic", "Difuminar", "k) Mezcladores",
                "No pinta color propio: arrastra y funde el que ya hay en el lienzo. Para suavizar transiciones entre valores o colores.")
        },
        BrushPreset {
            kind: BrushKind::Pixel,
            size: 4.0,
            hardness: 1.0,
            spacing: 0.05,
            ..base("pixel", "u) Pixel Art", "Pixel art", "u) Pixel art",
                "Píxeles cuadrados sin suavizado de bordes. Combínalo con el suavizado Pixel.")
        },
    ]
}
