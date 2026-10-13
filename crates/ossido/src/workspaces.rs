//! Gli spazi di lavoro (M5.1): più scrivanie, ogni finestra nella sua.
//!
//! Le finestre dello spazio attivo stanno nello `Space` di Smithay; quelle degli altri spazi
//! sono tenute qui con la loro posizione, fuori dalla scena. Nel cambio di spazio
//! (plan/reference/spaces.md) le due scrivanie scorrono in orizzontale insieme, rigidamente:
//! finestre, sfondo e barra dei menu dello spazio; il Dock e i pannelli della shell restano
//! fermi. Fra le due scrivanie una fascia nera di 64 pt; la corsa è la larghezza dello schermo
//! più la fascia. Il movimento segue una molla smorzata criticamente (ω = 0,0132 rad/ms,
//! nessun superamento). Oltre il primo o l'ultimo spazio il comando si ignora, senza rimbalzo.
//!
//! Durante la transizione le finestre dei due spazi restano mappate insieme e si spostano a
//! ogni frame: ombre, vetro e animazioni funzionano come sempre. Alla fine quelle dello spazio
//! lasciato escono dalla scena.

use std::time::Instant;

use smithay::desktop::Window;
use smithay::utils::{IsAlive, Logical, Point};

use crate::state::State;

/// La fascia nera fra due spazi durante lo scorrimento, in pt.
pub const SPACE_GAP: i32 = 64;

/// Pulsazione della molla del cambio di spazio, in rad/ms (adattamento delle misure).
pub const SPRING_OMEGA: f64 = 0.0132;

/// Spazi al più: oltre, `space.add` rifiuta.
pub const MAX_SPACES: usize = 16;

/// Avanzamento del cambio di spazio `ms` dopo l'inizio: `1 − (1 + ωt) e^(−ωt)`.
pub fn spring_progress(ms: f64) -> f64 {
    if ms <= 0.0 {
        return 0.0;
    }
    let x = SPRING_OMEGA * ms;
    (1.0 - (1.0 + x) * (-x).exp()).clamp(0.0, 1.0)
}

/// Finestre con la posizione nello spazio, dal basso verso l'alto.
pub type Placed = Vec<(Window, Point<i32, Logical>)>;

/// Uno scorrimento in corso fra due spazi.
pub struct Transition {
    pub from: usize,
    pub to: usize,
    pub start: Instant,
    /// +1 verso destra (lo spazio nuovo entra da destra), −1 verso sinistra.
    pub direction: i32,
    /// La corsa: larghezza dell'output più la fascia.
    pub travel: i32,
    pub outgoing: Placed,
    pub incoming: Placed,
    /// Avanzamento all'ultimo frame (0–1).
    pub progress: f64,
}

/// Lo scostamento delle due scrivanie in questo momento, in pt.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SpaceShift {
    /// Di quanto è spostata la scrivania che esce.
    pub outgoing: f64,
    /// Di quanto è spostata la scrivania che entra.
    pub incoming: f64,
    /// Dove comincia la fascia nera.
    pub gap: f64,
}

pub struct Workspaces {
    /// Le finestre degli spazi non attivi; la voce dello spazio attivo resta vuota.
    pub stored: Vec<Placed>,
    pub active: usize,
    pub transition: Option<Transition>,
}

impl Default for Workspaces {
    fn default() -> Self {
        Self {
            stored: vec![Vec::new()],
            active: 0,
            transition: None,
        }
    }
}

impl Workspaces {
    pub fn count(&self) -> usize {
        self.stored.len()
    }

    /// Lo scostamento per il frame, se uno scorrimento è in corso.
    pub fn shift(&self, output_width: f64) -> Option<SpaceShift> {
        let t = self.transition.as_ref()?;
        let travel = f64::from(t.travel);
        let moved = t.progress * travel * f64::from(t.direction);
        let outgoing = -moved;
        let incoming = f64::from(t.direction) * travel - moved;
        let gap = if t.direction > 0 {
            outgoing + output_width
        } else {
            incoming + output_width
        };
        Some(SpaceShift {
            outgoing,
            incoming,
            gap,
        })
    }
}

impl State {
    /// In quale spazio sta una finestra.
    pub fn window_space(&self, window: &Window) -> Option<usize> {
        if self.space.elements().any(|w| w == window) {
            let in_transition = self
                .workspaces
                .transition
                .as_ref()
                .is_some_and(|t| t.outgoing.iter().any(|(w, _)| w == window));
            return Some(if in_transition {
                self.workspaces.transition.as_ref().map_or(0, |t| t.from)
            } else {
                self.workspaces.active
            });
        }
        self.workspaces
            .stored
            .iter()
            .position(|placed| placed.iter().any(|(w, _)| w == window))
    }

    /// Passa allo spazio `to` con lo scorrimento. Falso se è già quello attivo o non esiste
    /// (oltre il bordo non si rimbalza: come nel riferimento, il comando si ignora).
    pub fn switch_space(&mut self, to: usize, now: Instant) -> bool {
        self.settle_transition();
        let from = self.workspaces.active;
        if to == from || to >= self.workspaces.count() {
            return false;
        }
        let output_width = self
            .space
            .output_geometry(&self.output)
            .map_or(0, |geo| geo.size.w);
        let travel = output_width.saturating_add(SPACE_GAP);
        let direction: i32 = if to > from { 1 } else { -1 };
        let outgoing: Placed = self
            .space
            .elements()
            .filter_map(|w| Some((w.clone(), self.space.element_location(w)?)))
            .collect();
        let incoming = self
            .workspaces
            .stored
            .get_mut(to)
            .map(std::mem::take)
            .unwrap_or_default();
        for (window, location) in &incoming {
            let start = *location + Point::from((direction.saturating_mul(travel), 0));
            self.space.map_element(window.clone(), start, false);
        }
        self.workspaces.active = to;
        self.workspaces.transition = Some(Transition {
            from,
            to,
            start: now,
            direction,
            travel,
            outgoing,
            incoming,
            progress: 0.0,
        });
        let top = self
            .workspaces
            .transition
            .as_ref()
            .and_then(|t| t.incoming.last().map(|(w, _)| w.clone()));
        self.focus_window(top);
        if self.animation_speed <= 0.0 {
            self.settle_transition();
        }
        true
    }

    /// Fa avanzare lo scorrimento in corso (a ogni frame).
    pub fn advance_spaces(&mut self, now: Instant) {
        let speed = self.animation_speed;
        let Some(t) = self.workspaces.transition.as_mut() else {
            return;
        };
        let ms = now.saturating_duration_since(t.start).as_secs_f64() * 1000.0;
        let progress = if speed <= 0.0 {
            1.0
        } else {
            spring_progress(ms / speed)
        };
        t.progress = progress;
        let travel = f64::from(t.travel);
        // Mezzo punto dall'arrivo: fermo.
        if (1.0 - progress) * travel < 0.5 {
            self.settle_transition();
            return;
        }
        let moved = (progress * travel).round() as i32 * t.direction;
        let ahead = t.direction.saturating_mul(t.travel);
        let mut placements: Vec<(Window, Point<i32, Logical>)> = Vec::new();
        for (window, base) in &t.outgoing {
            placements.push((window.clone(), *base - Point::from((moved, 0))));
        }
        for (window, base) in &t.incoming {
            placements.push((window.clone(), *base + Point::from((ahead - moved, 0))));
        }
        for (window, location) in placements {
            if self.space.element_location(&window).is_some() {
                self.space.map_element(window, location, false);
            }
        }
    }

    /// Chiude lo scorrimento: le finestre dello spazio lasciato escono dalla scena.
    pub fn settle_transition(&mut self) {
        let Some(t) = self.workspaces.transition.take() else {
            return;
        };
        let mut left = Placed::new();
        for (window, base) in t.outgoing {
            // Una finestra chiusa durante lo scorrimento è già uscita dallo Space.
            if self.space.element_location(&window).is_some() {
                self.space.unmap_elem(&window);
                left.push((window, base));
            }
        }
        if let Some(slot) = self.workspaces.stored.get_mut(t.from) {
            slot.extend(left);
        }
        for (window, base) in t.incoming {
            if self.space.element_location(&window).is_some() {
                self.space.map_element(window, base, false);
            }
        }
    }

    /// Aggiunge uno spazio in fondo e ne restituisce l'indice.
    pub fn add_space(&mut self) -> Result<usize, String> {
        if self.workspaces.count() >= MAX_SPACES {
            return Err(format!("al più {MAX_SPACES} spazi"));
        }
        self.workspaces.stored.push(Vec::new());
        Ok(self.workspaces.count() - 1)
    }

    /// Toglie uno spazio: le sue finestre passano a quello prima (o al primo). Togliere lo
    /// spazio attivo porta subito in quello che riceve le finestre.
    pub fn remove_space(&mut self, index: usize) -> Result<(), String> {
        self.settle_transition();
        let count = self.workspaces.count();
        if count <= 1 {
            return Err("c'è un solo spazio".to_owned());
        }
        if index >= count {
            return Err(format!("spazio {index} assente (ce ne sono {count})"));
        }
        let active = self.workspaces.active;
        // Le finestre dello spazio tolto (fuori dalla scena se era l'attivo).
        let moving: Placed = if index == active {
            let placed: Placed = self
                .space
                .elements()
                .filter_map(|w| Some((w.clone(), self.space.element_location(w)?)))
                .collect();
            for (window, _) in &placed {
                self.space.unmap_elem(window);
            }
            placed
        } else {
            self.workspaces
                .stored
                .get_mut(index)
                .map(std::mem::take)
                .unwrap_or_default()
        };
        self.workspaces.stored.remove(index);
        // Chi riceve le finestre, con gli indici già scalati dopo la rimozione.
        let receiver = index.saturating_sub(1);
        let new_active = match active.cmp(&index) {
            std::cmp::Ordering::Equal => receiver,
            std::cmp::Ordering::Greater => active - 1,
            std::cmp::Ordering::Less => active,
        };
        if receiver == new_active {
            // Chi riceve è (o diventa) l'attivo: tutto in scena.
            let mut placed = if index == active {
                self.workspaces
                    .stored
                    .get_mut(receiver)
                    .map(std::mem::take)
                    .unwrap_or_default()
            } else {
                Placed::new()
            };
            placed.extend(moving);
            for (window, location) in placed {
                self.space.map_element(window, location, false);
            }
        } else if let Some(slot) = self.workspaces.stored.get_mut(receiver) {
            slot.extend(moving);
        }
        self.workspaces.active = new_active;
        let top = self.space.elements().last().cloned();
        self.focus_window(top);
        Ok(())
    }

    /// Sposta una finestra in un altro spazio, alla stessa posizione.
    pub fn move_window_to_space(&mut self, window: &Window, to: usize) -> Result<(), String> {
        self.settle_transition();
        if to >= self.workspaces.count() {
            return Err(format!("spazio {to} assente"));
        }
        let from = self
            .window_space(window)
            .ok_or("finestra non in uno spazio (ridotta?)")?;
        if from == to {
            return Ok(());
        }
        let active = self.workspaces.active;
        let location = if from == active {
            let location = self
                .space
                .element_location(window)
                .ok_or("finestra non sullo schermo")?;
            self.space.unmap_elem(window);
            location
        } else {
            let slot = self
                .workspaces
                .stored
                .get_mut(from)
                .ok_or("spazio assente")?;
            let index = slot
                .iter()
                .position(|(w, _)| w == window)
                .ok_or("finestra assente dallo spazio")?;
            slot.remove(index).1
        };
        if to == active {
            self.space.map_element(window.clone(), location, true);
            self.focus_window(Some(window.clone()));
        } else {
            if let Some(slot) = self.workspaces.stored.get_mut(to) {
                slot.push((window.clone(), location));
            }
            if from == active {
                let top = self.space.elements().last().cloned();
                self.focus_window(top);
            }
        }
        Ok(())
    }

    /// Lo scostamento delle scrivanie per il frame, in pt.
    pub fn space_shift(&self) -> Option<SpaceShift> {
        let width = self
            .space
            .output_geometry(&self.output)
            .map_or(0.0, |geo| f64::from(geo.size.w));
        self.workspaces.shift(width)
    }

    /// Le finestre chiuse escono anche dagli spazi non attivi.
    pub fn forget_dead_windows(&mut self) {
        for slot in &mut self.workspaces.stored {
            slot.retain(|(window, _)| window.alive());
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_spring_follows_the_measured_curve() {
        // Misure (spaces.md): 50 % a ~115–127 ms, 90 % a ~284–296 ms, 99 % a ~503–508 ms.
        let at = |p: f64| {
            let mut ms = 0.0;
            while spring_progress(ms) < p {
                ms += 1.0;
            }
            ms
        };
        assert!((115.0..=130.0).contains(&at(0.5)), "{}", at(0.5));
        assert!((280.0..=300.0).contains(&at(0.9)), "{}", at(0.9));
        assert!((495.0..=515.0).contains(&at(0.99)), "{}", at(0.99));
        // Nessun superamento.
        assert!((0..3000).all(|ms| spring_progress(f64::from(ms)) <= 1.0));
    }

    #[test]
    fn the_gap_sits_between_the_two_desktops() {
        let mut spaces = Workspaces::default();
        assert_eq!(spaces.shift(2560.0), None);
        spaces.transition = Some(Transition {
            from: 0,
            to: 1,
            start: Instant::now(),
            direction: 1,
            travel: 2624,
            outgoing: Vec::new(),
            incoming: Vec::new(),
            progress: 0.25,
        });
        let shift = spaces.shift(2560.0).unwrap();
        assert_eq!(shift.outgoing, -656.0);
        assert_eq!(shift.incoming, 2624.0 - 656.0);
        // La fascia comincia dove finisce la scrivania che esce, larga 64.
        assert_eq!(shift.gap, -656.0 + 2560.0);
        assert_eq!(shift.incoming - shift.gap, 64.0);
    }
}
