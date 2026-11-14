//! Pantalla «Hoy»: el recorrido de 48 lecciones. Muestra lo que toca hoy (calentamiento con repaso,
//! gesto y estudio o dibujos de control), los hilos semanales (copia de maestro, opinión de fuera,
//! ojo crítico), la lección actual con su revisión y el mapa de bloques.

use super::*;
use crate::program::{self, DAYS_PER_LESSON, Lesson};
use crate::study;
use std::path::PathBuf;

/// Lo que se pulsó en la pantalla; se aplica al final, cuando ya no se está dibujando la interfaz.
enum Act {
    Exercise(usize, usize),
    Study,
    Gesture,
    Control(PathBuf),
    MasterCopy,
    OpenFolder(PathBuf),
    Diary,
    Advance,
    View(Option<usize>),
    ToggleReview,
}

/// Un ejercicio sugerido para el calentamiento.
pub(super) struct Pick {
    pub m: usize,
    pub e: usize,
    pub done: bool,
    /// Viene de un módulo de lecciones anteriores.
    pub review: bool,
}

impl TrazoApp {
    /// Elige el ejercicio de un módulo: el que ya se hizo hoy, o el primero sin tres estrellas, o el
    /// menos repetido.
    fn pick_in_module(&self, m: usize, done_today: &[String]) -> (usize, bool) {
        let exs = &self.modules[m].exercises;
        if let Some(e) = exs.iter().position(|x| done_today.contains(&x.id)) {
            return (e, true);
        }
        if let Some(e) = exs.iter().position(|x| self.progress.stars(&x.id) < 3) {
            return (e, false);
        }
        let e = (0..exs.len()).min_by_key(|&e| self.progress.results.get(&exs[e].id).map_or(0, |r| r.attempts)).unwrap_or(0);
        (e, false)
    }

    /// De estos módulos, el que lleva más tiempo sin practicarse (sin contar hoy).
    fn stalest_module(&self, ids: &[String], date: &str) -> Option<usize> {
        ids.iter()
            .filter_map(|id| self.modules.iter().position(|m| m.id == id))
            .min_by_key(|&m| {
                let ex_ids: Vec<String> = self.modules[m].exercises.iter().map(|x| x.id.clone()).collect();
                self.progress.program.last_practiced(&ex_ids, date).unwrap_or_default()
            })
    }

    /// Calentamiento de hoy: un ejercicio de la lección y otro de repaso de un módulo anterior (el que
    /// más tiempo lleva sin practicarse), porque la práctica repartida y mezclada retiene mejor.
    pub(super) fn warmup_picks(&self, lesson: usize, date: &str) -> Vec<Pick> {
        let Some((_, l)) = self.program.lesson(lesson) else { return Vec::new() };
        let done: Vec<String> = self.progress.program.day(date).map(|d| d.exercises.clone()).unwrap_or_default();
        let mut out = Vec::new();
        if let Some(m) = self.stalest_module(&l.warmup, date) {
            let (e, d) = self.pick_in_module(m, &done);
            out.push(Pick { m, e, done: d, review: false });
        }
        let review = self.program.review_modules(lesson);
        match self.stalest_module(&review, date) {
            Some(m) => {
                let (e, d) = self.pick_in_module(m, &done);
                out.push(Pick { m, e, done: d, review: true });
            }
            None => {
                // Primera lección: otro ejercicio del mismo módulo.
                if let Some(first) = out.first() {
                    let m = first.m;
                    let exs = &self.modules[m].exercises;
                    let skip = first.e;
                    let other = exs.iter().enumerate().find(|(i, x)| *i != skip && done.contains(&x.id)).map(|(i, _)| (i, true)).or_else(|| {
                        exs.iter().enumerate().find(|(i, x)| *i != skip && self.progress.stars(&x.id) < 3).map(|(i, _)| (i, false))
                    });
                    if let Some((e, d)) = other {
                        out.push(Pick { m, e, done: d, review: false });
                    }
                }
            }
        }
        out
    }

    /// Abre el estudio de la lección: su tema, su tiempo y una foto al azar de la biblioteca.
    pub(super) fn start_program_study(&mut self, ctx: &egui::Context, lesson: &Lesson) {
        self.open_study();
        self.from_program = true;
        self.study.phase = study_ui::Phase::Setup;
        self.study.topic = lesson.topic;
        self.study.timer = Some(lesson.minutes as f32 * 60.0);
        self.random_reference(ctx);
    }

    /// Carga una foto al azar de la carpeta del tema elegido. Devuelve si encontró alguna.
    pub(super) fn random_reference(&mut self, ctx: &egui::Context) -> bool {
        let imgs = study::library_images(self.study.topic);
        let seed = self.next_seed();
        let Some(path) = study::pick(&imgs, self.last_ref.as_deref(), seed) else { return false };
        self.load_reference_file(ctx, &path)
    }

    pub(super) fn load_reference_file(&mut self, ctx: &egui::Context, path: &std::path::Path) -> bool {
        match image::open(path) {
            Ok(img) => {
                self.set_study_reference(ctx, img);
                self.last_ref = Some(path.to_path_buf());
                true
            }
            Err(_) => {
                self.message = Some((format!("No pude abrir {}", path.display()), self.now_ms()));
                false
            }
        }
    }

    /// Carga el diario si aún no se leyó (lo usan las cifras del ojo crítico).
    pub(super) fn ensure_diary(&mut self) {
        if self.diary.is_empty() && !self.diary_loaded {
            self.diary = study::load_all()
                .into_iter()
                .map(|(path, entry)| study_ui::DiaryItem { path, entry, ref_tex: None, draw_tex: None, failed: false })
                .collect();
            self.diary_loaded = true;
        }
    }

    /// Ojo crítico: aciertos de la primera impresión en los diez últimos estudios con autocrítica
    /// doble y, si hay más de diez, en los diez primeros.
    pub(super) fn eye_trend(&self) -> Option<(f32, Option<f32>)> {
        // El diario va del más nuevo al más antiguo.
        let with: Vec<&study::Entry> = self.diary.iter().map(|d| &d.entry).filter(|e| e.eye().is_some()).collect();
        let recent = study::eye_rate(with.iter().take(10).copied())?;
        let early = (with.len() > 10).then(|| study::eye_rate(with.iter().rev().take(10).copied())).flatten();
        Some((recent, early))
    }

    pub(super) fn program_screen(&mut self, ui: &mut Ui) {
        let ctx = ui.ctx().clone();
        let today = program::today();
        let total = self.program.total();
        self.ensure_diary();
        // Rondas de dibujos de control: se abren y se cierran solas.
        let refs: Vec<String> = study::control_refs().iter().map(|p| study::file_name(p)).collect();
        let before = (self.progress.program.control_round.clone(), self.progress.program.control_rounds.len());
        let current = self.progress.program.lesson;
        self.progress.program.update_control(&refs, &today, self.program.closes_block(current));
        if before != (self.progress.program.control_round.clone(), self.progress.program.control_rounds.len()) {
            self.save();
        }
        let mut act: Option<Act> = None;
        egui::CentralPanel::default().frame(egui::Frame::new().fill(theme::BG).inner_margin(28)).show(ui, |ui| {
            let mut scroll = egui::ScrollArea::vertical().auto_shrink([false, false]);
            // Capturas de demostración: TRAZO_DESPLAZAR=<px> baja la pantalla.
            if let Some(y) = std::env::var("TRAZO_DESPLAZAR").ok().and_then(|v| v.parse::<f32>().ok()) {
                scroll = scroll.vertical_scroll_offset(y);
            }
            scroll.show(ui, |ui| {
                centered(ui, 1180.0, |ui| {
                    let streak = self.progress.program.streak(&today);
                    let days = self.progress.program.total_days();
                    ui.label(theme::strong("Tu recorrido de dibujo", theme::DISPLAY));
                    match self.program.lesson(current) {
                        Some((b, _)) => ui.label(theme::body(format!(
                            "Lección {} de {total} · Bloque {}: {}. Sin fechas: avanzas cuando cierras cada lección.",
                            current + 1,
                            b + 1,
                            self.program.blocks[b].title
                        ))),
                        None => ui.label(theme::body("Recorrido terminado")),
                    };
                    ui.horizontal(|ui| {
                        small_stat(ui, &streak.to_string(), if streak == 1 { "día seguido" } else { "días seguidos" }, GOLD);
                        ui.add_space(18.0);
                        small_stat(ui, &days.to_string(), "días practicados", ACCENT);
                        if let Some((recent, early)) = self.eye_trend() {
                            ui.add_space(18.0);
                            let label = match early {
                                Some(e) => format!("ojo crítico (al principio: {:.0} %)", e * 100.0),
                                None => "ojo crítico".to_string(),
                            };
                            small_stat(ui, &format!("{:.0} %", recent * 100.0), &label, theme::GOOD)
                                .on_hover_text("De tus diez últimos estudios: en qué parte de las preguntas tu primera impresión ya era la respuesta que diste tras superponer y medir.");
                        }
                    });
                    ui.add_space(14.0);
                    if self.program.lesson(current).is_none() {
                        theme::card(theme::RAISED).inner_margin(22).show(ui, |ui| {
                            ui.set_width(ui.available_width());
                            ui.label(theme::strong("¡Terminaste las 48 lecciones!", theme::TITLE));
                            ui.label(theme::body("Sigue con estudios del natural, copias de maestros, gesto y tus dibujos de control. Repasa el diario: ahí está todo lo que has avanzado."));
                        });
                    } else {
                        // Disposición sin justificar dentro de cada columna: el texto justificado de egui
                        // deja huecos raros entre palabras.
                        let top = egui::Layout::top_down(egui::Align::Min);
                        ui.columns(2, |cols| {
                            let (left, right) = cols.split_at_mut(1);
                            left[0].with_layout(top, |ui| {
                                if let Some(a) = self.today_card(ui, current, &today, &refs) {
                                    act = Some(a);
                                }
                                ui.add_space(12.0);
                                if let Some(a) = self.lesson_close_card(ui, current) {
                                    act = Some(a);
                                }
                            });
                            right[0].with_layout(top, |ui| {
                                if let Some(a) = self.lesson_card(ui, current) {
                                    act = Some(a);
                                }
                                ui.add_space(12.0);
                                if let Some(a) = self.threads_card(ui, &today) {
                                    act = Some(a);
                                }
                            });
                        });
                    }
                    ui.add_space(20.0);
                    if let Some(a) = self.blocks_map(ui, current) {
                        act = Some(a);
                    }
                    ui.add_space(12.0);
                });
            });
        });
        let lesson = self.program.lesson(current).map(|(_, l)| l.clone());
        match act {
            Some(Act::Exercise(m, e)) => {
                self.open_exercise(&ctx, m, e);
                self.from_program = true;
            }
            Some(Act::Study) => {
                if let Some(l) = lesson {
                    self.start_program_study(&ctx, &l);
                }
            }
            Some(Act::Gesture) => {
                if study::library_images(Topic::Gesture).is_empty() {
                    self.start_session(&ctx, 5, 60.0);
                } else {
                    self.start_gesture_photos(&ctx, 5, 60.0);
                }
            }
            Some(Act::Control(path)) => self.start_control(&ctx, &path),
            Some(Act::MasterCopy) => {
                self.open_study();
                self.from_program = true;
                self.study.topic = Topic::Master;
                self.study.timer = Some(30.0 * 60.0);
                self.random_reference(&ctx);
            }
            Some(Act::OpenFolder(dir)) => {
                let _ = std::process::Command::new("explorer").arg(dir).spawn();
            }
            Some(Act::Diary) => {
                self.diary.clear();
                self.diary_loaded = false;
                self.diary_filter = None;
                self.screen = Screen::Diary;
            }
            Some(Act::Advance) => {
                let review = std::mem::take(&mut self.review);
                let new_block = self.program.closes_block(current);
                self.progress.program.advance(review, &today, new_block);
                self.program_view = None;
                self.review_open = false;
                self.save();
                let msg = if new_block { "Bloque cerrado. ¡A por el siguiente!" } else { "Lección cerrada. ¡A por la siguiente!" };
                self.message = Some((msg.into(), self.now_ms()));
            }
            Some(Act::View(v)) => self.program_view = v,
            Some(Act::ToggleReview) => self.review_open = !self.review_open,
            None => {}
        }
    }

    fn today_card(&self, ui: &mut Ui, current: usize, today: &str, refs: &[String]) -> Option<Act> {
        let (_, lesson) = self.program.lesson(current)?;
        let mut act = None;
        let day = self.progress.program.day(today).cloned().unwrap_or_default();
        theme::card(theme::RAISED).stroke(Stroke::new(1.0, ACCENT.gamma_multiply(0.5))).inner_margin(20).show(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.label(RichText::new("HOY TOCA · UNOS 45 MINUTOS").size(theme::CAPTION).color(ACCENT).family(theme::semibold()));
            if let Some(f) = self.progress.program.focus() {
                ui.label(RichText::new(format!("Tu foco: {f}")).size(theme::BODY).color(GOLD));
            }
            if refs.is_empty() {
                ui.add_space(6.0);
                theme::card(theme::SURFACE).inner_margin(12).show(ui, |ui| {
                    ui.set_width(ui.available_width());
                    ui.label(theme::strong("Prepara tus referencias de control", theme::BODY));
                    ui.label(theme::caption(
                        "Guarda 2 o 3 fotos tuyas en la carpeta «control»: un objeto de casa, una figura y una cabeza. Las dibujarás ahora (tu punto de partida) y cada cuatro semanas, siempre igual, para ver el avance.",
                    ));
                    if theme::button(ui, None, "Abrir la carpeta «control»", Btn::Outline).clicked() {
                        act = Some(Act::OpenFolder(study::control_folder()));
                    }
                });
            }
            ui.add_space(6.0);
            step_header(ui, 1, day.warmup_done(), "Calentamiento · 10 min", "Uno de la lección y otro de repaso de lo anterior. Hazlos con atención: la nota es solo orientativa.");
            for p in self.warmup_picks(current, today) {
                let ex = &self.modules[p.m].exercises[p.e];
                let label = format!("{} · {}", self.modules[p.m].title, ex.title);
                ui.horizontal(|ui| {
                    ui.add_space(34.0);
                    let tag = if p.review { "Repaso" } else { "Lección" };
                    let (r, _) = ui.allocate_exact_size(vec2(64.0, 26.0), Sense::hover());
                    let col = if p.review { GOLD } else { ACCENT };
                    ui.painter().rect_filled(r.shrink2(vec2(0.0, 2.0)), 6, col.gamma_multiply(0.18));
                    ui.painter().text(r.center(), Align2::CENTER_CENTER, tag, FontId::new(theme::CAPTION, theme::semibold()), col);
                    let w = ui.available_width();
                    let kind = if p.done { Btn::Ghost } else { Btn::Outline };
                    let icon = if p.done { Icon::Check } else { kind_info(&ex.kind).0 };
                    if theme::button_ex(ui, Some(icon), &label, kind, w, true).on_hover_text(ex.goal).clicked() {
                        act = Some(Act::Exercise(p.m, p.e));
                    }
                });
            }
            ui.add_space(10.0);
            let poses = study::library_images(Topic::Gesture).len();
            step_header(ui, 2, day.gesture, "Gesto · 5 min", "Cinco poses de un minuto, todos los días: el gesto no se aprende en un mes y se olvida.");
            ui.horizontal(|ui| {
                ui.add_space(34.0);
                ui.vertical(|ui| {
                    let hint = if poses > 0 {
                        format!("Con fotos al azar de tu carpeta «gesto» ({poses}).")
                    } else {
                        "Aún no hay fotos en tu carpeta «gesto»: se usarán las poses esquemáticas de la app. Guarda fotos de poses reales para practicar con personas de verdad.".to_string()
                    };
                    ui.label(theme::caption(hint));
                    ui.horizontal_wrapped(|ui| {
                        if theme::button(ui, Some(Icon::Play), "Sesión de gesto", if day.gesture { Btn::Ghost } else { Btn::Outline }).clicked() {
                            act = Some(Act::Gesture);
                        }
                        if theme::button(ui, None, "Carpeta «gesto»", Btn::Ghost).clicked() {
                            act = Some(Act::OpenFolder(study::library_folder(Topic::Gesture)));
                        }
                    });
                });
            });
            ui.add_space(10.0);
            let pending = self.progress.program.control_pending(refs);
            if !pending.is_empty() {
                let first_time = self.progress.program.control_rounds.is_empty();
                let why = if first_time {
                    "Tu punto de partida: dibuja tus referencias de control, 15 minutos cada una. Cuentan como el estudio del día."
                } else {
                    "Toca ronda de control: las mismas fotos y el mismo tiempo (15 min) que siempre. Cuentan como el estudio del día."
                };
                step_header(ui, 3, day.study, "Dibujos de control", why);
                ui.horizontal(|ui| {
                    ui.add_space(34.0);
                    ui.horizontal_wrapped(|ui| {
                        for name in &pending {
                            if theme::button(ui, Some(Icon::Play), &format!("Dibujar «{name}»"), Btn::Primary).clicked() {
                                act = Some(Act::Control(study::control_dir().join(name)));
                            }
                        }
                    });
                });
            } else {
                step_header(
                    ui,
                    3,
                    day.study,
                    &format!("Estudio · {} · {} min", lesson.topic.label(), lesson.minutes),
                    "Con referencia, primera impresión, comprobación y autocrítica.",
                );
                ui.horizontal(|ui| {
                    ui.add_space(34.0);
                    ui.vertical(|ui| {
                        ui.label(theme::body(&lesson.study));
                        let n = study::library_images(lesson.topic).len();
                        let hint = if n > 0 {
                            format!("{n} fotos de «{}» en tu biblioteca: el estudio empieza con una al azar.", lesson.topic.label())
                        } else {
                            format!("Guarda fotos en tu carpeta «{}» y el estudio elegirá una al azar. También puedes soltar una en la ventana.", lesson.topic.folder())
                        };
                        ui.label(theme::caption(hint));
                        ui.horizontal_wrapped(|ui| {
                            if theme::button(ui, Some(Icon::Play), "Empezar el estudio", Btn::Primary).clicked() {
                                act = Some(Act::Study);
                            }
                            if theme::button(ui, None, "Carpeta de referencias", Btn::Ghost).clicked() {
                                act = Some(Act::OpenFolder(study::library_folder(lesson.topic)));
                            }
                        });
                    });
                });
            }
            if day.complete() {
                ui.add_space(8.0);
                ui.label(theme::strong("Día completado. ¡Hasta mañana!", theme::LEAD).color(theme::GOOD));
            }
        });
        act
    }

    /// Hilos que no se cortan: copia de maestro semanal, opinión de fuera y el ojo crítico.
    fn threads_card(&self, ui: &mut Ui, today: &str) -> Option<Act> {
        let mut act = None;
        theme::card(theme::RAISED).inner_margin(20).show(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.label(RichText::new("CADA SEMANA, TODO EL AÑO").size(theme::CAPTION).color(theme::TEXT_3).family(theme::semibold()));
            let copy = self.progress.program.master_copy_this_week(today);
            step_header(ui, 1, copy, "Una copia de maestro", "Una obra de museo, 30 minutos, con la misma autocrítica. Se ve qué simplificó el artista y por qué.");
            if !copy {
                ui.horizontal(|ui| {
                    ui.add_space(34.0);
                    if theme::button(ui, Some(Icon::Play), "Hacer la copia", Btn::Outline).clicked() {
                        act = Some(Act::MasterCopy);
                    }
                    if theme::button(ui, None, "Carpeta «maestros»", Btn::Ghost).clicked() {
                        act = Some(Act::OpenFolder(study::library_folder(Topic::Master)));
                    }
                });
            }
            ui.add_space(8.0);
            let due = self.progress.program.feedback_due(today);
            step_header(
                ui,
                2,
                !due,
                "Una opinión de fuera cada 5 semanas",
                "Tu ojo aún no ve todos tus errores. Enseña dos o tres estudios recientes (por ejemplo, en la comunidad de Drawabox o en r/ArtFundamentals) y apunta lo que te digan al cerrar la lección.",
            );
            ui.add_space(8.0);
            let eye = self.eye_trend();
            step_header(
                ui,
                3,
                false,
                "Tu ojo crítico",
                "Al comparar, primero respondes de memoria y después compruebas con superposición y medidas. Cuanto más coinciden, mejor ve tu ojo.",
            );
            ui.horizontal(|ui| {
                ui.add_space(34.0);
                let txt = match eye {
                    Some((r, Some(e))) => format!("Últimos estudios: {:.0} % de aciertos de primeras · al principio: {:.0} %", r * 100.0, e * 100.0),
                    Some((r, None)) => format!("Hasta ahora: {:.0} % de aciertos de primeras", r * 100.0),
                    None => "Aún sin datos: aparecerá al guardar tu primer estudio.".to_string(),
                };
                ui.label(RichText::new(txt).size(theme::BODY).color(theme::TEXT_2));
            });
        });
        act
    }

    fn lesson_close_card(&mut self, ui: &mut Ui, current: usize) -> Option<Act> {
        let mut act = None;
        let k = self.progress.program.lesson_days();
        let ready = self.progress.program.lesson_ready();
        let closes_block = self.program.closes_block(current);
        let refs_exist = !study::control_refs().is_empty();
        let control_open = self.progress.program.control_round.is_some();
        theme::card(theme::RAISED).inner_margin(20).show(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.label(RichText::new("ESTA LECCIÓN").size(theme::CAPTION).color(theme::TEXT_3).family(theme::semibold()));
            ui.horizontal(|ui| {
                ui.label(theme::strong(format!("Días completos: {} de {DAYS_PER_LESSON} recomendados", k.min(DAYS_PER_LESSON)), theme::LEAD));
                ui.add_space(8.0);
                for i in 0..DAYS_PER_LESSON {
                    let (r, _) = ui.allocate_exact_size(vec2(18.0, 18.0), Sense::hover());
                    if i < k {
                        ui.painter().circle_filled(r.center(), 7.0, theme::GOOD);
                    } else {
                        ui.painter().circle_stroke(r.center(), 6.5, Stroke::new(1.5, theme::BORDER));
                    }
                }
            });
            ui.label(theme::caption(
                "Un día completo lleva calentamiento, gesto y estudio. Quédate en la lección mientras la necesites: no hay calendario, y los días que te saltes no te retrasan.",
            ));
            if ready {
                ui.label(RichText::new("Puedes cerrar la lección cuando quieras: revisa tus estudios antes.").size(theme::BODY).color(theme::GOOD));
            }
            if closes_block && refs_exist && control_open {
                ui.label(RichText::new("Es la última lección del bloque: haz antes tus dibujos de control.").size(theme::BODY).color(GOLD));
            }
            if ready || self.review_open {
                ui.add_space(6.0);
                if theme::button(ui, Some(Icon::Check), "Ver mis estudios en el diario", Btn::Ghost).clicked() {
                    act = Some(Act::Diary);
                }
                let fields: [(&str, &mut String); 4] = [
                    ("¿Qué mejoró respecto a la lección anterior?", &mut self.review.improved),
                    ("¿Qué error se repite en varios estudios?", &mut self.review.repeated),
                    ("Lo que te dijo alguien de fuera (si pediste opinión)", &mut self.review.external),
                    ("Mi foco para la próxima lección", &mut self.review.focus),
                ];
                for (label, text) in fields {
                    ui.label(RichText::new(label).size(theme::BODY).color(theme::TEXT));
                    ui.add(egui::TextEdit::multiline(text).desired_rows(2).desired_width(f32::INFINITY));
                }
                ui.add_space(6.0);
                let w = ui.available_width();
                let label = if closes_block {
                    format!("Cerrar la lección {} y el bloque", current + 1)
                } else {
                    format!("Cerrar la lección {} y pasar a la {}", current + 1, current + 2)
                };
                if theme::button_ex(ui, Some(Icon::Next), &label, if ready { Btn::Primary } else { Btn::Outline }, w, false).clicked() {
                    act = Some(Act::Advance);
                }
            }
            if !ready {
                let label = if self.review_open { "Seguir con esta lección" } else { "Cerrar la lección antes" };
                if theme::button(ui, None, label, Btn::Ghost).on_hover_text("Si ya dominas el tema, puedes pasar a la siguiente lección sin los cuatro días").clicked() {
                    act = Some(Act::ToggleReview);
                }
            }
        });
        act
    }

    fn lesson_card(&self, ui: &mut Ui, current: usize) -> Option<Act> {
        let i = self.program_view.unwrap_or(current);
        let (b, lesson) = self.program.lesson(i)?;
        let total = self.program.total();
        let mut act = None;
        theme::card(theme::SURFACE).inner_margin(20).show(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.horizontal_wrapped(|ui| {
                theme::chip(ui, Some(Icon::Book), &format!("Bloque {} · {}", b + 1, self.program.blocks[b].title), ACCENT);
                let state = if i < current {
                    "hecha"
                } else if i == current {
                    "lección actual"
                } else {
                    "más adelante"
                };
                ui.label(theme::caption(format!("Lección {} de {total} · {state}", i + 1)));
            });
            ui.add_space(4.0);
            ui.label(theme::strong(&lesson.title, theme::TITLE));
            ui.label(RichText::new(&lesson.source).size(theme::CAPTION).color(ACCENT));
            ui.add_space(6.0);
            for para in lesson.lesson.split("\n\n") {
                ui.label(RichText::new(para).size(theme::BODY + 1.0).color(theme::TEXT));
                ui.add_space(6.0);
            }
            theme::section(ui, "En la práctica");
            let mods: Vec<&str> = lesson.warmup.iter().filter_map(|id| self.modules.iter().find(|m| m.id == id).map(|m| m.title)).collect();
            ui.label(theme::caption(format!("Calentamiento: {}, más un repaso de lo anterior.", mods.join(" y "))));
            ui.label(theme::caption(format!("Estudios: {} · {} min.", lesson.topic.label(), lesson.minutes)));
            ui.label(theme::caption(format!("Objetivo del bloque: {}", self.program.blocks[b].goal)));
            if i != current {
                ui.add_space(6.0);
                if theme::button(ui, Some(Icon::Prev), "Volver a la lección actual", Btn::Outline).clicked() {
                    act = Some(Act::View(None));
                }
            }
        });
        act
    }

    fn blocks_map(&self, ui: &mut Ui, current: usize) -> Option<Act> {
        let mut act = None;
        theme::section(ui, "El recorrido");
        ui.label(theme::caption(
            "Doce bloques sin fecha: cada uno dura lo que necesites (suelen ser entre dos y ocho semanas). Pulsa una lección para leerla. Las de color ya están hechas; la del borde dorado es la actual.",
        ));
        ui.add_space(6.0);
        let viewing = self.program_view.unwrap_or(current);
        for (b, block) in self.program.blocks.iter().enumerate() {
            let first = self.program.first_lesson(b);
            let active = (first..first + block.lessons.len()).contains(&current);
            let fill = if active { theme::RAISED } else { theme::SURFACE };
            theme::card(fill).inner_margin(egui::Margin::symmetric(14, 10)).show(ui, |ui| {
                ui.set_width(ui.available_width());
                ui.horizontal(|ui| {
                    let (r, _) = ui.allocate_exact_size(vec2(30.0, 30.0), Sense::hover());
                    let done = current >= first + block.lessons.len();
                    let col = if done { theme::GOOD } else if active { ACCENT } else { theme::TEXT_3 };
                    ui.painter().circle_filled(r.center(), 14.0, col.gamma_multiply(0.25));
                    ui.painter().text(r.center(), Align2::CENTER_CENTER, (b + 1).to_string(), FontId::new(14.0, theme::semibold()), col);
                    ui.vertical(|ui| {
                        ui.label(theme::strong(&block.title, theme::BODY + 1.0));
                        ui.label(theme::caption(&block.goal));
                    });
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        for k in (0..block.lessons.len()).rev() {
                            let i = first + k;
                            let (r, resp) = ui.allocate_exact_size(vec2(30.0, 26.0), Sense::click());
                            let p = ui.painter();
                            let bg = if i < current { theme::GOOD.gamma_multiply(0.35) } else { theme::BG };
                            p.rect_filled(r, 6, bg);
                            let stroke = if i == current {
                                Stroke::new(2.0, GOLD)
                            } else if i == viewing {
                                Stroke::new(1.5, ACCENT)
                            } else {
                                Stroke::new(1.0, theme::BORDER)
                            };
                            p.rect_stroke(r, 6, stroke, StrokeKind::Inside);
                            p.text(r.center(), Align2::CENTER_CENTER, (i + 1).to_string(), FontId::proportional(12.0), theme::TEXT_2);
                            let resp = resp.on_hover_text(&block.lessons[k].title).on_hover_cursor(egui::CursorIcon::PointingHand);
                            if resp.clicked() {
                                act = Some(Act::View(if i == current { None } else { Some(i) }));
                            }
                        }
                    });
                });
            });
            ui.add_space(6.0);
        }
        act
    }

    /// Tarjeta del inicio con lo que toca hoy. Devuelve si se pulsó.
    pub(super) fn home_today_card(&self, ui: &mut Ui) -> bool {
        let today = program::today();
        let current = self.progress.program.lesson;
        let Some((b, lesson)) = self.program.lesson(current) else { return false };
        let day = self.progress.program.day(&today).cloned().unwrap_or_default();
        let streak = self.progress.program.streak(&today);
        let mut go = false;
        theme::card(theme::RAISED).stroke(Stroke::new(1.0, GOLD.gamma_multiply(0.5))).inner_margin(22).show(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.horizontal(|ui| {
                ui.vertical(|ui| {
                    ui.label(RichText::new("HOY TOCA").size(theme::CAPTION).color(GOLD).family(theme::semibold()));
                    ui.label(theme::strong(format!("Lección {}: {}", current + 1, lesson.title), theme::TITLE));
                    ui.label(theme::caption(format!("Bloque {} · {}", b + 1, self.program.blocks[b].title)));
                    ui.add_space(4.0);
                    ui.horizontal(|ui| {
                        for (t, done) in [("Calentamiento", day.warmup_done()), ("Gesto", day.gesture), ("Estudio", day.study)] {
                            let col = if done { theme::GOOD } else { theme::TEXT_2 };
                            theme::icon_widget(ui, if done { Icon::Check } else { Icon::Play }, 14.0, col);
                            ui.label(RichText::new(t).size(theme::BODY).color(col));
                            ui.add_space(10.0);
                        }
                    });
                    if streak > 0 {
                        ui.label(RichText::new(format!("Racha: {streak} {}", if streak == 1 { "día" } else { "días seguidos" })).size(theme::BODY).color(GOLD));
                    }
                });
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    let label = if day.complete() { "Ver el recorrido" } else { "Empezar" };
                    if theme::button_ex(ui, Some(Icon::Play), label, Btn::Primary, 220.0, false).clicked() {
                        go = true;
                    }
                });
            });
        });
        go
    }
}

/// Encabezado de un paso: círculo con el número (o marca si está hecho), título y explicación.
fn step_header(ui: &mut Ui, n: usize, done: bool, title: &str, text: &str) {
    ui.horizontal(|ui| {
        let (r, _) = ui.allocate_exact_size(vec2(26.0, 26.0), Sense::hover());
        let p = ui.painter();
        if done {
            p.circle_filled(r.center(), 12.0, theme::GOOD);
            theme::paint_icon(p, r.shrink(5.0), Icon::Check, theme::BG);
        } else {
            p.circle_stroke(r.center(), 11.0, Stroke::new(1.5, ACCENT));
            p.text(r.center(), Align2::CENTER_CENTER, n.to_string(), FontId::new(13.0, theme::semibold()), ACCENT);
        }
        ui.add_space(4.0);
        ui.vertical(|ui| {
            ui.label(theme::strong(title, theme::LEAD));
            ui.label(theme::caption(text));
        });
    });
}

fn small_stat(ui: &mut Ui, value: &str, label: &str, color: Color32) -> egui::Response {
    ui.label(theme::strong(value, theme::TITLE).color(color));
    ui.label(theme::caption(label))
}
