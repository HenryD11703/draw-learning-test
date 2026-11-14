//! Estudios con referencia: se dibuja a partir de una imagen real y, en vez de una nota, se compara
//! con herramientas y se responde una autocrítica guiada. Cada estudio se guarda en un diario con
//! su referencia al lado, para ver el progreso con el tiempo.

use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

/// Tema del estudio: decide qué preguntas de autocrítica se hacen.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Topic {
    Gesture,
    Proportion,
    Figure,
    Value,
    Head,
    Hands,
    Objects,
    Landscape,
    Master,
    Color,
}

impl Topic {
    pub const ALL: [Topic; 10] = [
        Topic::Gesture,
        Topic::Proportion,
        Topic::Figure,
        Topic::Value,
        Topic::Head,
        Topic::Hands,
        Topic::Objects,
        Topic::Landscape,
        Topic::Master,
        Topic::Color,
    ];

    pub fn label(self) -> &'static str {
        match self {
            Topic::Gesture => "Gesto",
            Topic::Proportion => "Forma y proporción",
            Topic::Figure => "Figura y anatomía",
            Topic::Value => "Valor",
            Topic::Head => "Cabeza y retrato",
            Topic::Hands => "Manos",
            Topic::Objects => "Objetos y bodegón",
            Topic::Landscape => "Paisaje y entorno",
            Topic::Master => "Copia de maestro",
            Topic::Color => "Color",
        }
    }

    /// De qué método salen las preguntas.
    pub fn source(self) -> &'static str {
        match self {
            Topic::Gesture => "Kimon Nicolaïdes, «The Natural Way to Draw»",
            Topic::Proportion => "Betty Edwards, «Dibujar con el lado derecho del cerebro»",
            Topic::Figure => "Andrew Loomis, Michael Hampton y George Bridgman",
            Topic::Value => "James Gurney, «Color and Light», y Bert Dodson, «Keys to Drawing»",
            Topic::Head => "Andrew Loomis, «Drawing the Head and Hands»",
            Topic::Hands => "Andrew Loomis, «Drawing the Head and Hands»",
            Topic::Objects => "Drawabox (construcción con cajas, cilindros y elipses)",
            Topic::Landscape => "James Gurney (perspectiva atmosférica) y la planificación en valores",
            Topic::Master => "La copia de maestros, práctica clásica de los talleres",
            Topic::Color => "James Gurney, «Color and Light»",
        }
    }

    /// Preguntas de autocrítica. Se responden comparando con la referencia, no de memoria.
    pub fn questions(self) -> &'static [&'static str] {
        match self {
            Topic::Gesture => &[
                "¿Se lee la línea de acción de la pose de un vistazo?",
                "¿Se nota hacia dónde cae el peso del cuerpo?",
                "¿Dibujaste el movimiento antes que el contorno y los detalles?",
                "¿El largo del torso frente al de las piernas se parece al de la referencia?",
            ],
            Topic::Proportion => &[
                "¿La relación entre el ancho total y el alto coincide con la referencia?",
                "¿Los ángulos principales (bordes, ejes) tienen la misma inclinación?",
                "¿Los espacios negativos (los huecos entre formas) tienen la misma forma?",
                "¿Cada parte está donde toca, medida con la unidad que elegiste?",
            ],
            Topic::Figure => &[
                "¿El tórax y la pelvis tienen la misma inclinación y el mismo giro que en la referencia?",
                "¿Los largos de brazos y piernas, medidos en cabezas, coinciden?",
                "¿Los puntos óseos (clavículas, codos, rodillas, cresta de la cadera) están en su sitio?",
                "¿Las formas envuelven el volumen en vez de ser contornos planos?",
            ],
            Topic::Value => &[
                "Vistos en grises, ¿las grandes masas de luz y sombra coinciden?",
                "¿El punto más oscuro y el más claro están donde en la referencia?",
                "¿Separaste la familia de luz de la familia de sombra?",
                "¿Agrupaste los valores parecidos en vez de copiar cada matiz?",
            ],
            Topic::Head => &[
                "¿La línea central y la de las cejas siguen el giro de la cabeza?",
                "¿Los tres tercios del rostro (pelo-cejas, cejas-nariz, nariz-barbilla) coinciden?",
                "¿La oreja está entre la línea de las cejas y la de la nariz?",
                "¿El ancho de la cabeza frente a su alto se parece al de la referencia?",
            ],
            Topic::Hands => &[
                "¿La palma tiene el tamaño correcto frente al largo de los dedos?",
                "¿Las articulaciones de los dedos siguen arcos, no líneas rectas?",
                "¿El pulgar sale del sitio correcto de la palma?",
                "¿Se entiende la pose de la mano pensada en bloques?",
            ],
            Topic::Objects => &[
                "¿Las elipses tienen el grado correcto (más abiertas cuanto más lejos del horizonte)?",
                "¿La perspectiva es coherente: las líneas paralelas convergen hacia el mismo lado?",
                "¿Los objetos se apoyan de verdad en la superficie, sin flotar?",
                "¿Las proporciones entre unos objetos y otros coinciden con la referencia?",
            ],
            Topic::Landscape => &[
                "¿El horizonte está a la misma altura que en la referencia?",
                "¿Se distinguen primer plano, plano medio y fondo?",
                "¿Lo lejano es más claro y con menos contraste que lo cercano?",
                "¿Lo importante de la escena está en el mismo sitio de la composición?",
            ],
            Topic::Master => &[
                "¿Encajaste las proporciones generales antes de entrar en detalle?",
                "¿Respetaste el tipo de línea del original (grosor, presión, velocidad)?",
                "Vistos en grises, ¿los valores coinciden con el original?",
                "¿Entendiste por qué el artista resolvió así las zonas difíciles?",
            ],
            Topic::Color => &[
                "Vistos en grises, ¿el valor de cada zona coincide?",
                "¿Cada color tira hacia el mismo lado del círculo cromático que en la referencia?",
                "¿La saturación está bien: lo apagado sigue apagado y lo intenso, intenso?",
                "¿La temperatura de luces y sombras (cálida o fría) coincide?",
            ],
        }
    }
}

/// Respuesta a una pregunta de autocrítica.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Answer {
    Yes,
    Partly,
    No,
}

impl Answer {
    pub fn label(self) -> &'static str {
        match self {
            Answer::Yes => "Sí",
            Answer::Partly => "A medias",
            Answer::No => "No",
        }
    }
}

/// Un estudio del diario, tal como se guarda en `estudio.json`.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Entry {
    /// Segundos desde 1970, para ordenar y mostrar la fecha.
    pub when: u64,
    pub topic: Topic,
    pub minutes: f32,
    pub answers: Vec<Option<Answer>>,
    #[serde(default)]
    pub best: String,
    #[serde(default)]
    pub improve: String,
}

impl Entry {
    /// Cuántas preguntas respondió «Sí», de las respondidas.
    pub fn score(&self) -> (usize, usize) {
        let answered = self.answers.iter().flatten().count();
        let yes = self.answers.iter().flatten().filter(|a| **a == Answer::Yes).count();
        (yes, answered)
    }
}

impl Topic {
    /// Nombre de la carpeta del tema en la biblioteca de referencias.
    pub fn folder(self) -> &'static str {
        match self {
            Topic::Gesture => "gesto",
            Topic::Proportion => "proporcion",
            Topic::Figure => "figura",
            Topic::Value => "valor",
            Topic::Head => "cabeza",
            Topic::Hands => "manos",
            Topic::Objects => "objetos",
            Topic::Landscape => "paisaje",
            Topic::Master => "maestros",
            Topic::Color => "color",
        }
    }
}

/// Biblioteca de referencias: una carpeta por tema (Imágenes › TrazoAcademia › referencias › tema).
pub fn library_dir() -> PathBuf {
    if let Some(d) = std::env::var_os("TRAZO_REFERENCIAS") {
        return PathBuf::from(d);
    }
    let base = std::env::var_os("USERPROFILE").map(PathBuf::from).unwrap_or_default();
    base.join("Pictures").join("TrazoAcademia").join("referencias")
}

/// Crea las carpetas de la biblioteca (si faltan) y devuelve la del tema.
pub fn library_folder(topic: Topic) -> PathBuf {
    for t in Topic::ALL {
        let _ = std::fs::create_dir_all(library_dir().join(t.folder()));
    }
    library_dir().join(topic.folder())
}

/// Imágenes (PNG o JPG) de la carpeta de un tema, ordenadas por nombre.
pub fn library_images(topic: Topic) -> Vec<PathBuf> {
    images_in(&library_dir().join(topic.folder()))
}

pub fn images_in(dir: &Path) -> Vec<PathBuf> {
    let mut v: Vec<PathBuf> = std::fs::read_dir(dir)
        .map(|rd| {
            rd.filter_map(|e| e.ok())
                .map(|e| e.path())
                .filter(|p| p.extension().and_then(|x| x.to_str()).is_some_and(|x| ["png", "jpg", "jpeg"].contains(&x.to_ascii_lowercase().as_str())))
                .collect()
        })
        .unwrap_or_default();
    v.sort();
    v
}

/// Elige una imagen al azar, evitando repetir la última si hay otras.
pub fn pick(images: &[PathBuf], last: Option<&Path>, seed: u64) -> Option<PathBuf> {
    let pool: Vec<&PathBuf> = images.iter().filter(|p| Some(p.as_path()) != last).collect();
    let pool = if pool.is_empty() { images.iter().collect() } else { pool };
    if pool.is_empty() {
        return None;
    }
    let mut z = seed.wrapping_add(0x9E37_79B9_7F4A_7C15);
    z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    z ^= z >> 27;
    Some(pool[(z % pool.len() as u64) as usize].clone())
}

/// Carpeta del diario de estudios (Imágenes › TrazoAcademia › estudios).
pub fn diary_dir() -> PathBuf {
    // TRAZO_DIARIO: otra carpeta, para las capturas de demostración.
    if let Some(d) = std::env::var_os("TRAZO_DIARIO") {
        return PathBuf::from(d);
    }
    let base = std::env::var_os("USERPROFILE").map(PathBuf::from).unwrap_or_default();
    base.join("Pictures").join("TrazoAcademia").join("estudios")
}

/// Guarda un estudio: su ficha, el dibujo y la referencia. Devuelve la carpeta creada.
pub fn save(entry: &Entry, drawing_rgba: &[u8], size: [u32; 2], reference: &image::RgbaImage) -> std::io::Result<PathBuf> {
    let dir = diary_dir().join(format!("estudio-{}", entry.when));
    std::fs::create_dir_all(&dir)?;
    image::save_buffer(dir.join("dibujo.png"), drawing_rgba, size[0], size[1], image::ColorType::Rgba8).map_err(std::io::Error::other)?;
    // La referencia se guarda reducida: basta para comparar en el diario.
    let small = image::DynamicImage::ImageRgba8(reference.clone()).thumbnail(1600, 1600).to_rgba8();
    small.save(dir.join("referencia.png")).map_err(std::io::Error::other)?;
    let json = serde_json::to_string_pretty(entry).map_err(std::io::Error::other)?;
    std::fs::write(dir.join("estudio.json"), json)?;
    Ok(dir)
}

/// Lee todos los estudios del diario, del más reciente al más antiguo.
pub fn load_all() -> Vec<(PathBuf, Entry)> {
    load_from(&diary_dir())
}

pub fn load_from(dir: &Path) -> Vec<(PathBuf, Entry)> {
    let mut out: Vec<(PathBuf, Entry)> = std::fs::read_dir(dir)
        .map(|rd| {
            rd.filter_map(|e| e.ok())
                .map(|e| e.path())
                .filter(|p| p.is_dir())
                .filter_map(|p| {
                    let text = std::fs::read_to_string(p.join("estudio.json")).ok()?;
                    let entry: Entry = serde_json::from_str(&text).ok()?;
                    Some((p, entry))
                })
                .collect()
        })
        .unwrap_or_default();
    out.sort_by(|a, b| b.1.when.cmp(&a.1.when));
    out
}

/// Fecha legible en la hora local (día/mes/año hora:minuto).
pub fn date_label(when: u64) -> String {
    match chrono::DateTime::from_timestamp(when as i64, 0) {
        Some(t) => t.with_timezone(&chrono::Local).format("%d/%m/%Y %H:%M").to_string(),
        None => date_label_utc(when),
    }
}

/// Fecha legible en UTC, sin depender de la zona horaria.
pub fn date_label_utc(when: u64) -> String {
    // Días desde 1970 a fecha civil (algoritmo de Howard Hinnant).
    let days = (when / 86_400) as i64;
    let secs = when % 86_400;
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = yoe + era * 400 + i64::from(m <= 2);
    format!("{d:02}/{m:02}/{y} {:02}:{:02}", secs / 3600, (secs % 3600) / 60)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_topic_has_questions_and_a_source() {
        for t in Topic::ALL {
            assert!(t.questions().len() >= 3, "{}", t.label());
            assert!(!t.source().is_empty());
        }
    }

    #[test]
    fn diary_round_trip() {
        let dir = std::env::temp_dir().join(format!("trazo-diario-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let entry = Entry {
            when: 1_763_000_000,
            topic: Topic::Value,
            minutes: 12.5,
            answers: vec![Some(Answer::Yes), Some(Answer::No), None, Some(Answer::Partly)],
            best: "Las masas de sombra".into(),
            improve: "Agrupar más".into(),
        };
        let sub = dir.join("estudio-1");
        std::fs::create_dir_all(&sub).unwrap();
        std::fs::write(sub.join("estudio.json"), serde_json::to_string(&entry).unwrap()).unwrap();
        let all = load_from(&dir);
        assert_eq!(all.len(), 1);
        assert_eq!(all[0].1.topic, Topic::Value);
        assert_eq!(all[0].1.score(), (1, 3));
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn dates_are_readable() {
        assert_eq!(date_label_utc(0), "01/01/1970 00:00");
        assert_eq!(date_label_utc(1_700_000_000), "14/11/2023 22:13");
        assert_eq!(date_label(1_700_000_000).len(), 16);
    }

    #[test]
    fn library_picks_images_without_repeating() {
        let dir = std::env::temp_dir().join(format!("trazo-refs-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        for n in ["a.png", "b.JPG", "c.txt"] {
            std::fs::write(dir.join(n), b"x").unwrap();
        }
        let imgs = images_in(&dir);
        assert_eq!(imgs.len(), 2);
        for seed in 0..20 {
            assert_eq!(pick(&imgs, Some(&imgs[0]), seed).as_ref(), Some(&imgs[1]));
        }
        assert!(pick(&[], None, 1).is_none());
        let _ = std::fs::remove_dir_all(&dir);
    }
}
