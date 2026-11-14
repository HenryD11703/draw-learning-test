//! Pantalla «Hoy»: el programa del año. Muestra lo que toca hoy (calentamiento y estudio), la lección
//! de la semana, cuántos días completos lleva la semana con su revisión, y el mapa del año.

use super::*;
use crate::program::{self, DAYS_PER_WEEK, WARMUP, Week};
use crate::study;

/// Lo que se pulsó en la pantalla; se aplica al final, cuando ya no se está dibujando la interfaz.
enum Act {
    Exercise(usize, usize),
    Study,
    Folder,
    Diary,
    Advance,
    View(Option<usize>),
    ToggleReview,
}

impl TrazoApp {
    /// Ejercicios del calentamiento de hoy: primero los ya hechos hoy (con su marca) y luego, por cada
    /// módulo de la semana, el primero que aún no tiene tres estrellas.
    pub(super) fn warmup_picks(&self, week: &Week, date: &str) -> Vec<(usize, usize, bool)> {
        let done: Vec<String> = self.progress.program.day(date).map(|d| d.exercises.clone()).unwrap_or_default();
        let mods: Vec<usize> = week.warmup.iter().filter_map(|id| self.modules.iter().position(|m| m.id == id)).collect();
        if mods.is_empty() {
            return Vec::new();
        }
        let per = WARMUP.div_ceil(mods.len()).max(1);
        let mut out = Vec::new();
        for &m in &mods {
            let exs = &self.modules[m].exercises;
            let mut mine: Vec<(usize, usize, bool)> =
                exs.iter().enumerate().filter(|(_, x)| done.contains(&x.id)).map(|(e, _)| (m, e, true)).take(per).collect();
            let fresh = exs.iter().enumerate().filter(|(_, x)| !done.contains(&x.id));
            let mut todo: Vec<usize> = fresh.clone().filter(|(_, x)| self.progress.stars(&x.id) < 3).map(|(e, _)| e).collect();
            if todo.is_empty() {
                // Todo con tres estrellas: los menos repetidos.
                let mut all: Vec<(u32, usize)> =
                    fresh.map(|(e, x)| (self.progress.results.get(&x.id).map_or(0, |r| r.attempts), e)).collect();
                all.sort();
                todo = all.into_iter().map(|(_, e)| e).collect();
            }
            for e in todo {
                if mine.len() >= per {
                    break;
                }
                mine.push((m, e, false));
            }
            out.extend(mine);
        }
        out.truncate(WARMUP.max(mods.len()));
        out
    }

    /// Abre el estudio de la semana: su tema, su tiempo y una foto al azar de la biblioteca.
    pub(super) fn start_program_study(&mut self, ctx: &egui::Context, week: &Week) {
        self.open_study();
        self.from_program = true;
        self.study.phase = study_ui::Phase::Setup;
        self.study.topic = week.topic;
        self.study.timer = Some(week.minutes as f32 * 60.0);
        self.random_reference(ctx);
    }

    /// Carga una foto al azar de la carpeta del tema elegido. Devuelve si encontró alguna.
    pub(super) fn random_reference(&mut self, ctx: &egui::Context) -> bool {
        let imgs = study::library_images(self.study.topic);
        let seed = self.next_seed();
        let Some(path) = study::pick(&imgs, self.last_ref.as_deref(), seed) else { return false };
        match image::open(&path) {
            Ok(img) => {
                self.set_study_reference(ctx, img);
                self.last_ref = Some(path);
                true
            }
            Err(_) => {
                self.message = Some((format!("No pude abrir {}", path.display()), self.now_ms()));
                false
            }
        }
    }

    pub(super) fn program_screen(&mut self, ui: &mut Ui) {
        let ctx = ui.ctx().clone();
        let today = program::today();
        let total = self.program.total_weeks();
        let mut act: Option<Act> = None;
        egui::CentralPanel::default().frame(egui::Frame::new().fill(theme::BG).inner_margin(28)).show(ui, |ui| {
            let mut scroll = egui::ScrollArea::vertical().auto_shrink([false, false]);
            // Capturas de demostración: TRAZO_DESPLAZAR=<px> baja la pantalla.
            if let Some(y) = std::env::var("TRAZO_DESPLAZAR").ok().and_then(|v| v.parse::<f32>().ok()) {
                scroll = scroll.vertical_scroll_offset(y);
            }
            scroll.show(ui, |ui| {
                centered(ui, 1180.0, |ui| {
                    let current = self.progress.program.week;
                    let streak = self.progress.program.streak(&today);
                    let days = self.progress.program.total_days();
                    ui.horizontal(|ui| {
                        ui.vertical(|ui| {
                            ui.label(theme::strong("Tu año de dibujo", theme::DISPLAY));
                            match self.program.week(current) {
                                Some((m, _)) => ui.label(theme::body(format!(
                                    "Semana {} de {total} · Mes {}: {}",
                                    current + 1,
                                    m + 1,
                                    self.program.months[m].title
                                ))),
                                None => ui.label(theme::body("Programa terminado")),
                            };
                        });
                    });
                    ui.horizontal(|ui| {
                        small_stat(ui, &streak.to_string(), if streak == 1 { "día seguido" } else { "días seguidos" }, GOLD);
                        ui.add_space(18.0);
                        small_stat(ui, &days.to_string(), "días practicados en total", ACCENT);
                    });
                    ui.add_space(14.0);
                    if self.program.week(current).is_none() {
                        theme::card(theme::RAISED).inner_margin(22).show(ui, |ui| {
                            ui.set_width(ui.available_width());
                            ui.label(theme::strong("¡Terminaste el programa del año!", theme::TITLE));
                            ui.label(theme::body("Sigue con estudios del natural, copias de maestros y proyectos propios. Repasa el diario: ahí está todo lo que has avanzado."));
                        });
                    } else {
                        // Disposición normal (sin justificar) dentro de cada columna: el texto justificado
                        // de egui deja huecos raros entre palabras.
                        let top = egui::Layout::top_down(egui::Align::Min);
                        ui.columns(2, |cols| {
                            let (left, right) = cols.split_at_mut(1);
                            left[0].with_layout(top, |ui| {
                                if let Some(a) = self.today_card(ui, current, &today) {
                                    act = Some(a);
                                }
                                ui.add_space(12.0);
                                if let Some(a) = self.week_card(ui, current) {
                                    act = Some(a);
                                }
                            });
                            right[0].with_layout(top, |ui| {
                                if let Some(a) = self.lesson_card(ui, current) {
                                    act = Some(a);
                                }
                            });
                        });
                    }
                    ui.add_space(20.0);
                    if let Some(a) = self.year_map(ui, current) {
                        act = Some(a);
                    }
                    ui.add_space(12.0);
                });
            });
        });
        let current = self.progress.program.week;
        let week = self.program.week(current).map(|(_, w)| w.clone());
        match act {
            Some(Act::Exercise(m, e)) => {
                self.open_exercise(&ctx, m, e);
                self.from_program = true;
            }
            Some(Act::Study) => {
                if let Some(w) = week {
                    self.start_program_study(&ctx, &w);
                }
            }
            Some(Act::Folder) => {
                if let Some(w) = week {
                    let _ = std::process::Command::new("explorer").arg(study::library_folder(w.topic)).spawn();
                }
            }
            Some(Act::Diary) => {
                self.diary.clear();
                self.diary_loaded = false;
                self.diary_filter = None;
                self.screen = Screen::Diary;
            }
            Some(Act::Advance) => {
                let review = std::mem::take(&mut self.review);
                self.progress.program.advance(review, &today);
                self.program_view = None;
                self.review_open = false;
                self.save();
                self.message = Some(("Semana cerrada. ¡A por la siguiente!".into(), self.now_ms()));
            }
            Some(Act::View(v)) => self.program_view = v,
            Some(Act::ToggleReview) => self.review_open = !self.review_open,
            None => {}
        }
    }

    fn today_card(&self, ui: &mut Ui, current: usize, today: &str) -> Option<Act> {
        let (_, week) = self.program.week(current)?;
        let mut act = None;
        let day = self.progress.program.day(today).cloned().unwrap_or_default();
        theme::card(theme::RAISED).stroke(Stroke::new(1.0, ACCENT.gamma_multiply(0.5))).inner_margin(20).show(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.label(RichText::new("HOY TOCA").size(theme::CAPTION).color(ACCENT).family(theme::semibold()));
            if let Some(f) = self.progress.program.focus() {
                ui.label(RichText::new(format!("Tu foco esta semana: {f}")).size(theme::BODY).color(GOLD));
            }
            ui.add_space(6.0);
            step_header(ui, 1, day.warmup_done(), "Calentamiento", "Unos 10 minutos: dos ejercicios calificados de la semana.");
            for (m, e, done) in self.warmup_picks(week, today) {
                let ex = &self.modules[m].exercises[e];
                let stars = self.progress.stars(&ex.id);
                let label = format!("{} · {}", self.modules[m].title, ex.title);
                ui.horizontal(|ui| {
                    ui.add_space(34.0);
                    let w = ui.available_width() - 84.0;
                    let kind = if done { Btn::Ghost } else { Btn::Outline };
                    let icon = if done { Icon::Check } else { kind_info(&ex.kind).0 };
                    if theme::button_ex(ui, Some(icon), &label, kind, w, true).on_hover_text(ex.goal).clicked() {
                        act = Some(Act::Exercise(m, e));
                    }
                    let (r, _) = ui.allocate_exact_size(vec2(76.0, 24.0), Sense::hover());
                    theme::stars(ui.painter(), r.center(), 9.0, stars);
                });
            }
            ui.add_space(10.0);
            step_header(
                ui,
                2,
                day.study,
                &format!("Estudio · {}", week.topic.label()),
                &format!("Unos {} minutos con referencia, comparación y autocrítica.", week.minutes),
            );
            ui.horizontal(|ui| {
                ui.add_space(34.0);
                ui.vertical(|ui| {
                    ui.label(theme::body(&week.study));
                    let n = study::library_images(week.topic).len();
                    let hint = if n > 0 {
                        format!("Tienes {n} fotos de «{}» en tu biblioteca: el estudio empieza con una al azar.", week.topic.label())
                    } else {
                        format!("Guarda fotos en tu carpeta «{}» y el estudio elegirá una al azar. También puedes soltar una en la ventana.", week.topic.folder())
                    };
                    ui.label(theme::caption(hint));
                    ui.horizontal_wrapped(|ui| {
                        if theme::button(ui, Some(Icon::Play), "Empezar el estudio", Btn::Primary).clicked() {
                            act = Some(Act::Study);
                        }
                        if theme::button(ui, None, "Carpeta de referencias", Btn::Ghost).on_hover_text("Abre la carpeta de este tema en tu biblioteca").clicked() {
                            act = Some(Act::Folder);
                        }
                    });
                });
            });
            if day.complete() {
                ui.add_space(8.0);
                ui.label(theme::strong("Día completado. ¡Hasta mañana!", theme::LEAD).color(theme::GOOD));
            }
        });
        act
    }

    fn week_card(&mut self, ui: &mut Ui, current: usize) -> Option<Act> {
        let mut act = None;
        let k = self.progress.program.week_days();
        let ready = self.progress.program.week_ready();
        theme::card(theme::RAISED).inner_margin(20).show(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.label(RichText::new("ESTA SEMANA").size(theme::CAPTION).color(theme::TEXT_3).family(theme::semibold()));
            ui.horizontal(|ui| {
                ui.label(theme::strong(format!("Días completos: {} de {DAYS_PER_WEEK}", k.min(DAYS_PER_WEEK)), theme::LEAD));
                ui.add_space(8.0);
                for i in 0..DAYS_PER_WEEK {
                    let (r, _) = ui.allocate_exact_size(vec2(18.0, 18.0), Sense::hover());
                    if i < k {
                        ui.painter().circle_filled(r.center(), 7.0, theme::GOOD);
                    } else {
                        ui.painter().circle_stroke(r.center(), 6.5, Stroke::new(1.5, theme::BORDER));
                    }
                }
            });
            ui.label(theme::caption(
                "Un día cuenta como completo con el calentamiento y el estudio hechos. Los días que te saltes no te retrasan: el programa avanza a tu ritmo.",
            ));
            if ready {
                ui.label(RichText::new("¡Semana lista! Revisa tus estudios y ciérrala.").size(theme::BODY).color(theme::GOOD));
            }
            if ready || self.review_open {
                ui.add_space(6.0);
                if theme::button(ui, Some(Icon::Check), "Ver mis estudios en el diario", Btn::Ghost).clicked() {
                    act = Some(Act::Diary);
                }
                let fields: [(&str, &mut String); 3] = [
                    ("¿Qué mejoró respecto a la semana pasada?", &mut self.review.improved),
                    ("¿Qué error se repite en varios estudios?", &mut self.review.repeated),
                    ("Mi foco para la semana que viene", &mut self.review.focus),
                ];
                for (label, text) in fields {
                    ui.label(RichText::new(label).size(theme::BODY).color(theme::TEXT));
                    ui.add(egui::TextEdit::multiline(text).desired_rows(2).desired_width(f32::INFINITY));
                }
                ui.add_space(6.0);
                let w = ui.available_width();
                let label = format!("Cerrar la semana {} y empezar la {}", current + 1, current + 2);
                if theme::button_ex(ui, Some(Icon::Next), &label, if ready { Btn::Primary } else { Btn::Outline }, w, false).clicked() {
                    act = Some(Act::Advance);
                }
            }
            if !ready {
                let label = if self.review_open { "Seguir con esta semana" } else { "Cerrar la semana antes" };
                if theme::button(ui, None, label, Btn::Ghost).on_hover_text("Si ya dominas el tema de la semana, puedes pasar a la siguiente sin los cuatro días").clicked() {
                    act = Some(Act::ToggleReview);
                }
            }
        });
        act
    }

    fn lesson_card(&self, ui: &mut Ui, current: usize) -> Option<Act> {
        let i = self.program_view.unwrap_or(current);
        let (m, week) = self.program.week(i)?;
        let total = self.program.total_weeks();
        let mut act = None;
        theme::card(theme::SURFACE).inner_margin(20).show(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.horizontal_wrapped(|ui| {
                theme::chip(ui, Some(Icon::Book), &format!("Mes {} · {}", m + 1, self.program.months[m].title), ACCENT);
                let state = if i < current {
                    "hecha"
                } else if i == current {
                    "semana actual"
                } else {
                    "más adelante"
                };
                ui.label(theme::caption(format!("Semana {} de {total} · {state}", i + 1)));
            });
            ui.add_space(4.0);
            ui.label(theme::strong(&week.title, theme::TITLE));
            ui.label(RichText::new(&week.source).size(theme::CAPTION).color(ACCENT));
            ui.add_space(6.0);
            for para in week.lesson.split("\n\n") {
                ui.label(RichText::new(para).size(theme::BODY + 1.0).color(theme::TEXT));
                ui.add_space(6.0);
            }
            theme::section(ui, "En la práctica");
            let mods: Vec<&str> = week.warmup.iter().filter_map(|id| self.modules.iter().find(|m| m.id == id).map(|m| m.title)).collect();
            ui.label(theme::caption(format!("Calentamiento: {}.", mods.join(" y "))));
            ui.label(theme::caption(format!("Estudios: {} · {} min.", week.topic.label(), week.minutes)));
            ui.label(theme::caption(format!("Objetivo del mes: {}", self.program.months[m].goal)));
            if i != current {
                ui.add_space(6.0);
                if theme::button(ui, Some(Icon::Prev), "Volver a la semana actual", Btn::Outline).clicked() {
                    act = Some(Act::View(None));
                }
            }
        });
        act
    }

    fn year_map(&self, ui: &mut Ui, current: usize) -> Option<Act> {
        let mut act = None;
        theme::section(ui, "El año");
        ui.label(theme::caption("Pulsa una semana para leer su lección. Las de color ya están hechas; la del borde dorado es la actual."));
        ui.add_space(6.0);
        let viewing = self.program_view.unwrap_or(current);
        for (m, month) in self.program.months.iter().enumerate() {
            let first = self.program.first_week(m);
            let active = (first..first + month.weeks.len()).contains(&current);
            let fill = if active { theme::RAISED } else { theme::SURFACE };
            theme::card(fill).inner_margin(egui::Margin::symmetric(14, 10)).show(ui, |ui| {
                ui.set_width(ui.available_width());
                ui.horizontal(|ui| {
                    let (r, _) = ui.allocate_exact_size(vec2(30.0, 30.0), Sense::hover());
                    let done = current >= first + month.weeks.len();
                    let col = if done { theme::GOOD } else if active { ACCENT } else { theme::TEXT_3 };
                    ui.painter().circle_filled(r.center(), 14.0, col.gamma_multiply(0.25));
                    ui.painter().text(r.center(), Align2::CENTER_CENTER, (m + 1).to_string(), FontId::new(14.0, theme::semibold()), col);
                    ui.vertical(|ui| {
                        ui.label(theme::strong(&month.title, theme::BODY + 1.0));
                        ui.label(theme::caption(&month.goal));
                    });
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        for k in (0..month.weeks.len()).rev() {
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
                            let resp = resp.on_hover_text(&month.weeks[k].title).on_hover_cursor(egui::CursorIcon::PointingHand);
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
        let current = self.progress.program.week;
        let Some((m, week)) = self.program.week(current) else { return false };
        let day = self.progress.program.day(&today).cloned().unwrap_or_default();
        let streak = self.progress.program.streak(&today);
        let mut go = false;
        theme::card(theme::RAISED).stroke(Stroke::new(1.0, GOLD.gamma_multiply(0.5))).inner_margin(22).show(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.horizontal(|ui| {
                ui.vertical(|ui| {
                    ui.label(RichText::new("HOY TOCA").size(theme::CAPTION).color(GOLD).family(theme::semibold()));
                    ui.label(theme::strong(format!("Semana {}: {}", current + 1, week.title), theme::TITLE));
                    ui.label(theme::caption(format!("Mes {} · {}", m + 1, self.program.months[m].title)));
                    ui.add_space(4.0);
                    let mark = |done: bool| if done { "hecho" } else { "pendiente" };
                    let warm = format!("Calentamiento: {}", mark(day.warmup_done()));
                    let st = format!("Estudio de {}: {}", week.topic.label().to_lowercase(), mark(day.study));
                    ui.horizontal(|ui| {
                        for (t, done) in [(warm, day.warmup_done()), (st, day.study)] {
                            let col = if done { theme::GOOD } else { theme::TEXT_2 };
                            if done {
                                theme::icon_widget(ui, Icon::Check, 16.0, col);
                            }
                            ui.label(RichText::new(t).size(theme::BODY).color(col));
                            ui.add_space(10.0);
                        }
                    });
                    if streak > 0 {
                        ui.label(RichText::new(format!("Racha: {streak} {}", if streak == 1 { "día" } else { "días seguidos" })).size(theme::BODY).color(GOLD));
                    }
                });
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    let label = if day.complete() { "Ver el programa" } else { "Empezar" };
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

fn small_stat(ui: &mut Ui, value: &str, label: &str, color: Color32) {
    ui.label(theme::strong(value, theme::TITLE).color(color));
    ui.label(theme::caption(label));
}
