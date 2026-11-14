//! Programa de un año: 12 meses de 4 semanas. Cada semana tiene una lección corta basada en un libro,
//! los módulos del calentamiento y el tema del estudio con referencia. Cada día toca calentar con
//! ejercicios calificados y hacer un estudio; con cuatro días completos se revisa la semana y se pasa
//! a la siguiente. El avance va por días practicados, no por calendario: saltarse días no te retrasa.

use crate::study::Topic;
use chrono::{Local, NaiveDate};
use serde::{Deserialize, Serialize};

/// Ejercicios calificados que hacen falta para dar por hecho el calentamiento del día.
pub const WARMUP: usize = 2;
/// Días completos (calentamiento y estudio) para poder cerrar la semana.
pub const DAYS_PER_WEEK: usize = 4;

#[derive(Clone, Debug, Deserialize)]
pub struct Week {
    pub title: String,
    pub source: String,
    pub lesson: String,
    /// Módulos de ejercicios para el calentamiento (por su id).
    pub warmup: Vec<String>,
    pub topic: Topic,
    /// Qué dibujar en los estudios de la semana.
    pub study: String,
    /// Tiempo sugerido para cada estudio, en minutos.
    pub minutes: u32,
}

#[derive(Clone, Debug, Deserialize)]
pub struct Month {
    pub title: String,
    pub goal: String,
    pub weeks: Vec<Week>,
}

#[derive(Clone, Debug, Deserialize)]
pub struct Program {
    pub months: Vec<Month>,
}

impl Program {
    pub fn load() -> Self {
        serde_json::from_str(include_str!("../content/programa.json")).expect("content/programa.json no es válido")
    }

    pub fn total_weeks(&self) -> usize {
        self.months.iter().map(|m| m.weeks.len()).sum()
    }

    /// Mes (índice) y semana del índice global `i`.
    pub fn week(&self, i: usize) -> Option<(usize, &Week)> {
        let mut k = i;
        for (m, month) in self.months.iter().enumerate() {
            if k < month.weeks.len() {
                return Some((m, &month.weeks[k]));
            }
            k -= month.weeks.len();
        }
        None
    }

    /// Índice global de la primera semana del mes `m`.
    pub fn first_week(&self, m: usize) -> usize {
        self.months[..m].iter().map(|x| x.weeks.len()).sum()
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
}

impl Day {
    pub fn warmup_done(&self) -> bool {
        self.exercises.len() >= WARMUP
    }

    pub fn complete(&self) -> bool {
        self.warmup_done() && self.study
    }

    pub fn practiced(&self) -> bool {
        !self.exercises.is_empty() || self.study
    }
}

/// Revisión al cerrar una semana.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct Review {
    pub week: usize,
    pub date: String,
    pub improved: String,
    pub repeated: String,
    pub focus: String,
}

/// Dónde va el alumno en el programa. Se guarda junto al progreso.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct ProgramState {
    /// Semana actual (índice global, desde 0).
    pub week: usize,
    /// Fecha en que empezó la semana actual (vacía hasta el primer día de práctica).
    #[serde(default)]
    pub week_started: String,
    #[serde(default)]
    pub days: Vec<Day>,
    #[serde(default)]
    pub reviews: Vec<Review>,
}

/// Fecha local de hoy, AAAA-MM-DD.
pub fn today() -> String {
    Local::now().date_naive().format("%Y-%m-%d").to_string()
}

fn parse(d: &str) -> Option<NaiveDate> {
    NaiveDate::parse_from_str(d, "%Y-%m-%d").ok()
}

impl ProgramState {
    pub fn day(&self, date: &str) -> Option<&Day> {
        self.days.iter().find(|d| d.date == date)
    }

    fn day_mut(&mut self, date: &str) -> &mut Day {
        if self.week_started.is_empty() {
            self.week_started = date.to_string();
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

    /// Días completos desde que empezó la semana actual.
    pub fn week_days(&self) -> usize {
        let Some(start) = parse(&self.week_started) else { return 0 };
        self.days.iter().filter(|d| d.complete() && parse(&d.date).is_some_and(|x| x >= start)).count()
    }

    pub fn week_ready(&self) -> bool {
        self.week_days() >= DAYS_PER_WEEK
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

    /// Cierra la semana con su revisión y empieza la siguiente.
    pub fn advance(&mut self, mut review: Review, date: &str) {
        review.week = self.week;
        review.date = date.to_string();
        self.reviews.push(review);
        self.week += 1;
        self.week_started = date.to_string();
    }

    /// El foco que el alumno se puso en la última revisión.
    pub fn focus(&self) -> Option<&str> {
        self.reviews.last().map(|r| r.focus.as_str()).filter(|s| !s.trim().is_empty())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::content;

    #[test]
    fn program_is_a_year_and_uses_real_modules() {
        let p = Program::load();
        assert_eq!(p.months.len(), 12);
        assert_eq!(p.total_weeks(), 48);
        let ids: Vec<String> = content::module_ids();
        for m in &p.months {
            assert!(!m.goal.is_empty());
            for w in &m.weeks {
                assert!(!w.warmup.is_empty(), "{}", w.title);
                for id in &w.warmup {
                    assert!(ids.contains(id), "semana «{}»: no existe el módulo {id}", w.title);
                }
                assert!(w.lesson.len() > 200 && !w.source.is_empty() && !w.study.is_empty(), "{}", w.title);
                assert!(w.minutes > 0);
            }
        }
        assert_eq!(p.week(4).map(|(m, w)| (m, w.title.as_str())), Some((1, "Elipses y su eje menor")));
        assert_eq!(p.first_week(2), 8);
        assert!(p.week(48).is_none());
    }

    #[test]
    fn days_weeks_and_streaks() {
        let mut s = ProgramState::default();
        assert_eq!(s.streak("2026-11-10"), 0);
        for (i, date) in ["2026-11-10", "2026-11-11", "2026-11-12", "2026-11-14"].iter().enumerate() {
            s.record_exercise("lin01", date);
            s.record_exercise("lin01", date);
            assert!(!s.day(date).unwrap().warmup_done(), "el mismo ejercicio no cuenta dos veces");
            s.record_exercise("eli01", date);
            assert_eq!(s.week_days(), i);
            s.record_study(date);
            assert_eq!(s.week_days(), i + 1);
        }
        assert_eq!(s.week_started, "2026-11-10");
        assert!(s.week_ready());
        // Hoy (15) sin practicar: la racha cuenta hasta ayer; el 13 la cortó.
        assert_eq!(s.streak("2026-11-15"), 1);
        assert_eq!(s.streak("2026-11-12"), 3);
        assert_eq!(s.streak("2026-11-16"), 0);
        s.advance(Review { focus: "Medir antes".into(), ..Default::default() }, "2026-11-15");
        assert_eq!(s.week, 1);
        assert_eq!(s.week_days(), 0);
        assert_eq!(s.focus(), Some("Medir antes"));
        assert_eq!(s.reviews[0].week, 0);
        assert_eq!(s.total_days(), 4);
    }
}
