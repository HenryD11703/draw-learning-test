//! Programa de unos doce bloques de cuatro lecciones (48 en total). Cada lección tiene un texto corto
//! basado en un libro, los módulos de su calentamiento y el tema de sus estudios.
//!
//! El ritmo no va por calendario: te quedas en una lección mientras la necesites (se recomiendan
//! cuatro días completos) y la cierras con una revisión. Lo aprendido no se abandona al pasar de tema:
//! cada día hay gesto, el calentamiento repasa un módulo de lecciones anteriores, cada semana toca una
//! copia de maestro y, cada cuatro semanas o al acabar un bloque, unos dibujos de control con las
//! mismas referencias de siempre para ver el avance. La práctica repartida y mezclada retiene mejor que
//! la práctica en bloques (ver MECHANICS.md, sección 6).

use crate::study::Topic;
use chrono::{Local, NaiveDate};
use serde::{Deserialize, Serialize};

/// Ejercicios calificados que hacen falta para dar por hecho el calentamiento del día.
pub const WARMUP: usize = 2;
/// Días completos recomendados antes de cerrar una lección.
pub const DAYS_PER_LESSON: usize = 4;
/// Cada cuántos días toca una ronda de dibujos de control (además de al cerrar cada bloque).
pub const CONTROL_EVERY: i64 = 28;
/// Cada cuántos días conviene pedir una opinión de fuera.
pub const FEEDBACK_EVERY: i64 = 35;

#[derive(Clone, Debug, Deserialize)]
pub struct Lesson {
    pub title: String,
    pub source: String,
    pub lesson: String,
    /// Módulos de ejercicios para el calentamiento (por su id).
    pub warmup: Vec<String>,
    pub topic: Topic,
    /// Qué dibujar en los estudios de la lección.
    pub study: String,
    /// Tiempo sugerido para cada estudio, en minutos.
    pub minutes: u32,
}

#[derive(Clone, Debug, Deserialize)]
pub struct Block {
    pub title: String,
    pub goal: String,
    pub lessons: Vec<Lesson>,
}

#[derive(Clone, Debug, Deserialize)]
pub struct Program {
    pub blocks: Vec<Block>,
}

impl Program {
    pub fn load() -> Self {
        serde_json::from_str(include_str!("../content/programa.json")).expect("content/programa.json no es válido")
    }

    pub fn total(&self) -> usize {
        self.blocks.iter().map(|b| b.lessons.len()).sum()
    }

    /// Bloque (índice) y lección del índice global `i`.
    pub fn lesson(&self, i: usize) -> Option<(usize, &Lesson)> {
        let mut k = i;
        for (b, block) in self.blocks.iter().enumerate() {
            if k < block.lessons.len() {
                return Some((b, &block.lessons[k]));
            }
            k -= block.lessons.len();
        }
        None
    }

    /// Índice global de la primera lección del bloque `b`.
    pub fn first_lesson(&self, b: usize) -> usize {
        self.blocks[..b].iter().map(|x| x.lessons.len()).sum()
    }

    /// ¿Es `i` la última lección de su bloque?
    pub fn closes_block(&self, i: usize) -> bool {
        self.lesson(i).is_some_and(|(b, _)| i + 1 == self.first_lesson(b) + self.blocks[b].lessons.len())
    }

    /// Módulos de las lecciones anteriores a `i` que no están en la lección `i`, sin repetir y en el
    /// orden en que aparecieron.
    pub fn review_modules(&self, i: usize) -> Vec<String> {
        let current: Vec<String> = self.lesson(i).map(|(_, l)| l.warmup.clone()).unwrap_or_default();
        let mut out: Vec<String> = Vec::new();
        for k in 0..i.min(self.total()) {
            if let Some((_, l)) = self.lesson(k) {
                for m in &l.warmup {
                    if !current.contains(m) && !out.contains(m) {
                        out.push(m.clone());
                    }
                }
            }
        }
        out
    }
}

/// Lo que se hizo un día.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct Day {
    /// Fecha local, AAAA-MM-DD.
    pub date: String,
    /// Ejercicios calificados ese día (sin repetir).
    #[serde(default)]
    pub exercises: Vec<String>,
    #[serde(default)]
    pub study: bool,
    /// Sesión de gesto del día.
    #[serde(default)]
    pub gesture: bool,
}

impl Day {
    pub fn warmup_done(&self) -> bool {
        self.exercises.len() >= WARMUP
    }

    /// Día completo: calentamiento, gesto y estudio.
    pub fn complete(&self) -> bool {
        self.warmup_done() && self.study && self.gesture
    }

    pub fn practiced(&self) -> bool {
        !self.exercises.is_empty() || self.study || self.gesture
    }
}

/// Revisión al cerrar una lección.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct Review {
    #[serde(alias = "week")]
    pub lesson: usize,
    pub date: String,
    pub improved: String,
    pub repeated: String,
    pub focus: String,
    /// Lo que dijo alguien de fuera (vacío si no se pidió opinión).
    #[serde(default)]
    pub external: String,
}

/// Un dibujo de control: qué referencia y cuándo.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct ControlRecord {
    pub date: String,
    pub reference: String,
}

/// Dónde va el alumno en el programa. Se guarda junto al progreso.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct ProgramState {
    /// Lección actual (índice global, desde 0).
    #[serde(alias = "week")]
    pub lesson: usize,
    /// Fecha en que empezó la lección actual (vacía hasta el primer día de práctica).
    #[serde(default, alias = "week_started")]
    pub lesson_started: String,
    /// Fecha en que empezó el bloque actual.
    #[serde(default)]
    pub block_started: String,
    #[serde(default)]
    pub days: Vec<Day>,
    #[serde(default)]
    pub reviews: Vec<Review>,
    #[serde(default)]
    pub controls: Vec<ControlRecord>,
    /// Ronda de dibujos de control abierta (fecha en que se abrió).
    #[serde(default)]
    pub control_round: Option<String>,
    /// Fechas en que se completó cada ronda de dibujos de control.
    #[serde(default)]
    pub control_rounds: Vec<String>,
    /// Fechas de los estudios de copia de maestro.
    #[serde(default)]
    pub master_copies: Vec<String>,
}

/// Fecha local de hoy, AAAA-MM-DD.
pub fn today() -> String {
    Local::now().date_naive().format("%Y-%m-%d").to_string()
}

fn parse(d: &str) -> Option<NaiveDate> {
    NaiveDate::parse_from_str(d, "%Y-%m-%d").ok()
}

/// Días entre dos fechas (b − a); `None` si alguna no es válida.
pub fn days_between(a: &str, b: &str) -> Option<i64> {
    Some((parse(b)? - parse(a)?).num_days())
}

impl ProgramState {
    pub fn day(&self, date: &str) -> Option<&Day> {
        self.days.iter().find(|d| d.date == date)
    }

    fn day_mut(&mut self, date: &str) -> &mut Day {
        if self.lesson_started.is_empty() {
            self.lesson_started = date.to_string();
        }
        if self.block_started.is_empty() {
            self.block_started = date.to_string();
        }
        match self.days.iter().position(|d| d.date == date) {
            Some(i) => &mut self.days[i],
            None => {
                self.days.push(Day { date: date.to_string(), ..Default::default() });
                self.days.last_mut().unwrap()
            }
        }
    }

    /// Un ejercicio calificado cuenta para el calentamiento del día.
    pub fn record_exercise(&mut self, id: &str, date: &str) {
        let d = self.day_mut(date);
        if !d.exercises.iter().any(|e| e == id) {
            d.exercises.push(id.to_string());
        }
    }

    /// Un estudio guardado en el diario cuenta como el estudio del día.
    pub fn record_study(&mut self, date: &str) {
        self.day_mut(date).study = true;
    }

    pub fn record_gesture(&mut self, date: &str) {
        self.day_mut(date).gesture = true;
    }

    pub fn record_master_copy(&mut self, date: &str) {
        self.master_copies.push(date.to_string());
    }

    /// Hay una copia de maestro en los últimos siete días.
    pub fn master_copy_this_week(&self, date: &str) -> bool {
        self.master_copies.iter().any(|d| days_between(d, date).is_some_and(|n| (0..7).contains(&n)))
    }

    /// Último día antes de `before` en que se calificó un ejercicio de alguno de estos ids. (Se ignora
    /// el día de hoy para que la sugerencia del calentamiento no cambie mientras se hace.)
    pub fn last_practiced(&self, ids: &[String], before: &str) -> Option<String> {
        self.days
            .iter()
            .filter(|d| d.date.as_str() < before && d.exercises.iter().any(|e| ids.contains(e)))
            .map(|d| d.date.clone())
            .max()
    }

    /// Días completos desde que empezó la lección actual.
    pub fn lesson_days(&self) -> usize {
        let Some(start) = parse(&self.lesson_started) else { return 0 };
        self.days.iter().filter(|d| d.complete() && parse(&d.date).is_some_and(|x| x >= start)).count()
    }

    pub fn lesson_ready(&self) -> bool {
        self.lesson_days() >= DAYS_PER_LESSON
    }

    /// Días seguidos con algo de práctica, hasta hoy (o hasta ayer, si hoy aún no se practicó).
    pub fn streak(&self, date: &str) -> u32 {
        let Some(mut d) = parse(date) else { return 0 };
        let practiced = |x: NaiveDate| self.days.iter().any(|day| day.practiced() && parse(&day.date) == Some(x));
        if !practiced(d) {
            d = d.pred_opt().unwrap_or(d);
        }
        let mut n = 0;
        while practiced(d) {
            n += 1;
            match d.pred_opt() {
                Some(p) => d = p,
                None => break,
            }
        }
        n
    }

    pub fn total_days(&self) -> usize {
        self.days.iter().filter(|d| d.practiced()).count()
    }

    /// Cierra la lección con su revisión y empieza la siguiente. `new_block`: la siguiente lección
    /// empieza un bloque.
    pub fn advance(&mut self, mut review: Review, date: &str, new_block: bool) {
        review.lesson = self.lesson;
        review.date = date.to_string();
        self.reviews.push(review);
        self.lesson += 1;
        self.lesson_started = date.to_string();
        if new_block {
            self.block_started = date.to_string();
        }
    }

    /// El foco que el alumno se puso en la última revisión.
    pub fn focus(&self) -> Option<&str> {
        self.reviews.last().map(|r| r.focus.as_str()).filter(|s| !s.trim().is_empty())
    }

    /// Toca pedir una opinión de fuera: hace más de `FEEDBACK_EVERY` días de la última (o de que se
    /// empezó, si nunca se pidió).
    pub fn feedback_due(&self, date: &str) -> bool {
        let last = self.reviews.iter().filter(|r| !r.external.trim().is_empty()).map(|r| r.date.clone()).max();
        let since = last.or_else(|| self.days.iter().map(|d| d.date.clone()).min());
        since.and_then(|s| days_between(&s, date)).is_some_and(|n| n >= FEEDBACK_EVERY)
    }

    /// Referencias de control que faltan en la ronda abierta.
    pub fn control_pending(&self, refs: &[String]) -> Vec<String> {
        let Some(start) = &self.control_round else { return Vec::new() };
        refs.iter()
            .filter(|r| !self.controls.iter().any(|c| &c.reference == *r && c.date >= *start))
            .cloned()
            .collect()
    }

    /// Abre una ronda de control si toca: la primera vez (para tener el punto de partida), cada
    /// `CONTROL_EVERY` días, o en la última lección de un bloque si no hubo ninguna en el bloque.
    /// Cierra la ronda abierta cuando ya no le falta ninguna referencia.
    pub fn update_control(&mut self, refs: &[String], date: &str, last_lesson_of_block: bool) {
        if refs.is_empty() {
            return;
        }
        if self.control_round.is_some() {
            if self.control_pending(refs).is_empty() {
                self.control_round = None;
                self.control_rounds.push(date.to_string());
            }
            return;
        }
        let last = self.control_rounds.iter().max().cloned();
        let due = match &last {
            None => true,
            Some(l) => {
                days_between(l, date).is_some_and(|n| n >= CONTROL_EVERY)
                    || (last_lesson_of_block && !self.block_started.is_empty() && *l < self.block_started)
            }
        };
        if due {
            self.control_round = Some(date.to_string());
        }
    }

    pub fn record_control(&mut self, reference: &str, date: &str) {
        self.controls.push(ControlRecord { date: date.to_string(), reference: reference.to_string() });
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::content;

    #[test]
    fn program_has_48_lessons_and_uses_real_modules() {
        let p = Program::load();
        assert_eq!(p.blocks.len(), 12);
        assert_eq!(p.total(), 48);
        let ids: Vec<String> = content::module_ids();
        for b in &p.blocks {
            assert!(!b.goal.is_empty());
            for l in &b.lessons {
                assert!(!l.warmup.is_empty(), "{}", l.title);
                for id in &l.warmup {
                    assert!(ids.contains(id), "lección «{}»: no existe el módulo {id}", l.title);
                }
                assert!(l.lesson.len() > 200 && !l.source.is_empty() && !l.study.is_empty(), "{}", l.title);
                assert!(l.minutes > 0);
            }
        }
        assert_eq!(p.lesson(4).map(|(b, l)| (b, l.title.as_str())), Some((1, "Elipses y su eje menor")));
        assert_eq!(p.first_lesson(2), 8);
        assert!(p.closes_block(3) && !p.closes_block(4) && p.closes_block(47));
        assert!(p.lesson(48).is_none());
        // Repaso: en la lección 5 (elipses) se repasan los módulos de antes que no son de esa lección.
        let r = p.review_modules(4);
        assert!(r.contains(&"lineas".to_string()) && r.contains(&"formas".to_string()) && !r.contains(&"elipses".to_string()));
        assert!(p.review_modules(0).is_empty());
    }

    fn full_day(s: &mut ProgramState, date: &str) {
        s.record_exercise("lin01", date);
        s.record_exercise("eli01", date);
        s.record_gesture(date);
        s.record_study(date);
    }

    #[test]
    fn days_lessons_and_streaks() {
        let mut s = ProgramState::default();
        assert_eq!(s.streak("2026-11-10"), 0);
        for (i, date) in ["2026-11-10", "2026-11-11", "2026-11-12", "2026-11-14"].iter().enumerate() {
            s.record_exercise("lin01", date);
            s.record_exercise("lin01", date);
            assert!(!s.day(date).unwrap().warmup_done(), "el mismo ejercicio no cuenta dos veces");
            s.record_exercise("eli01", date);
            s.record_study(date);
            assert_eq!(s.lesson_days(), i, "sin gesto el día no está completo");
            s.record_gesture(date);
            assert_eq!(s.lesson_days(), i + 1);
        }
        assert_eq!(s.lesson_started, "2026-11-10");
        assert!(s.lesson_ready());
        assert_eq!(s.streak("2026-11-15"), 1);
        assert_eq!(s.streak("2026-11-12"), 3);
        assert_eq!(s.streak("2026-11-16"), 0);
        assert_eq!(s.last_practiced(&["eli01".into()], "2026-11-15").as_deref(), Some("2026-11-14"));
        assert_eq!(s.last_practiced(&["eli01".into()], "2026-11-14").as_deref(), Some("2026-11-12"));
        s.advance(Review { focus: "Medir antes".into(), ..Default::default() }, "2026-11-15", false);
        assert_eq!(s.lesson, 1);
        assert_eq!(s.lesson_days(), 0);
        assert_eq!(s.focus(), Some("Medir antes"));
        assert_eq!(s.reviews[0].lesson, 0);
        assert_eq!(s.total_days(), 4);
    }

    #[test]
    fn control_rounds_open_and_close() {
        let refs = vec!["objeto.png".to_string(), "figura.png".to_string()];
        let mut s = ProgramState::default();
        full_day(&mut s, "2026-01-01");
        // La primera vez siempre toca: es el punto de partida.
        s.update_control(&refs, "2026-01-01", false);
        assert_eq!(s.control_round.as_deref(), Some("2026-01-01"));
        s.record_control("objeto.png", "2026-01-01");
        s.update_control(&refs, "2026-01-01", false);
        assert_eq!(s.control_pending(&refs), vec!["figura.png".to_string()]);
        s.record_control("figura.png", "2026-01-02");
        s.update_control(&refs, "2026-01-02", false);
        assert!(s.control_round.is_none());
        assert_eq!(s.control_rounds, vec!["2026-01-02".to_string()]);
        // No vuelve a tocar hasta 28 días después...
        s.update_control(&refs, "2026-01-20", false);
        assert!(s.control_round.is_none());
        s.update_control(&refs, "2026-01-30", false);
        assert!(s.control_round.is_some());
        // ...o al final de un bloque empezado después de la última ronda.
        let mut t = ProgramState { control_rounds: vec!["2026-01-02".into()], block_started: "2026-01-10".into(), ..Default::default() };
        t.update_control(&refs, "2026-01-15", false);
        assert!(t.control_round.is_none());
        t.update_control(&refs, "2026-01-15", true);
        assert!(t.control_round.is_some());
    }

    #[test]
    fn feedback_and_master_copies() {
        let mut s = ProgramState::default();
        full_day(&mut s, "2026-01-01");
        assert!(!s.feedback_due("2026-01-20"));
        assert!(s.feedback_due("2026-02-10"));
        s.advance(Review { external: "Las elipses tiemblan".into(), ..Default::default() }, "2026-02-10", false);
        assert!(!s.feedback_due("2026-02-20"));
        assert!(!s.master_copy_this_week("2026-02-20"));
        s.record_master_copy("2026-02-15");
        assert!(s.master_copy_this_week("2026-02-20"));
        assert!(!s.master_copy_this_week("2026-02-23"));
    }

    #[test]
    fn old_saved_state_still_loads() {
        let old = r#"{"week":3,"week_started":"2026-11-01","days":[{"date":"2026-11-01","exercises":["a"],"study":true}],"reviews":[{"week":2,"date":"2026-10-30","improved":"","repeated":"","focus":"x"}]}"#;
        let s: ProgramState = serde_json::from_str(old).unwrap();
        assert_eq!(s.lesson, 3);
        assert_eq!(s.lesson_started, "2026-11-01");
        assert_eq!(s.reviews[0].lesson, 2);
    }
}
