//! Curve delle animazioni delle finestre, da `plan/reference/windows.md` ("Animazioni"):
//! riprese a 75 Hz, ±13 ms per istante.

use std::time::Duration;

/// Apertura: la finestra compare già opaca e cresce da 0,98 a 1 attorno al centro in
/// ~93–100 ms, rallentando (0,980 → 0,985 → 0,990 → 0,9925 → 0,995 → 1,000).
pub const OPEN: Duration = Duration::from_millis(100);
pub const OPEN_FROM_SCALE: f64 = 0.98;

/// Chiusura: dissolvenza in ~120 ms (opacità 0,76 → 0,51 → 0,22 → 0,14 → 0,05 → 0,02 → 0)
/// con una riduzione di scala ≲ 0,5 %.
pub const CLOSE: Duration = Duration::from_millis(125);
pub const CLOSE_TO_SCALE: f64 = 0.995;

/// Avanzamento 0–1 dopo `elapsed`, con le durate moltiplicate per `speed` (0: niente
/// animazioni, "Riduci movimento").
pub fn progress(elapsed: Duration, duration: Duration, speed: f64) -> f64 {
    if speed <= 0.0 || !speed.is_finite() {
        return 1.0;
    }
    let total = duration.as_secs_f64() * speed;
    if total <= 0.0 {
        return 1.0;
    }
    (elapsed.as_secs_f64() / total).clamp(0.0, 1.0)
}

/// Scala della finestra che si apre: ease-out quadratico.
pub fn open_scale(progress: f64) -> f64 {
    let eased = 1.0 - (1.0 - progress).powi(2);
    OPEN_FROM_SCALE + (1.0 - OPEN_FROM_SCALE) * eased
}

/// Opacità della finestra che si chiude: (1 − p)³.
pub fn close_alpha(progress: f64) -> f32 {
    (1.0 - progress).powi(3) as f32
}

/// Scala della finestra che si chiude.
pub fn close_scale(progress: f64) -> f64 {
    1.0 + (CLOSE_TO_SCALE - 1.0) * progress
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn open_follows_the_measured_frames() {
        // (ms dal primo fotogramma, scala misurata): ±0,0025 di incertezza, ±13 ms.
        for (ms, measured) in [
            (0, 0.980),
            (13, 0.985),
            (40, 0.990),
            (53, 0.9925),
            (93, 1.0),
        ] {
            let p = progress(Duration::from_millis(ms), OPEN, 1.0);
            assert!(
                (open_scale(p) - measured).abs() <= 0.004,
                "{ms} ms: {}",
                open_scale(p)
            );
        }
    }

    #[test]
    fn close_follows_the_measured_frames() {
        for (ms, measured) in [
            (13, 0.76),
            (26, 0.51),
            (53, 0.22),
            (66, 0.14),
            (93, 0.05),
            (125, 0.0),
        ] {
            let p = progress(Duration::from_millis(ms), CLOSE, 1.0);
            assert!(
                (f64::from(close_alpha(p)) - measured).abs() <= 0.08,
                "{ms} ms: {}",
                close_alpha(p)
            );
        }
    }

    #[test]
    fn speed_zero_disables_animations() {
        assert_eq!(progress(Duration::ZERO, OPEN, 0.0), 1.0);
        assert_eq!(progress(Duration::from_millis(50), OPEN, 2.0), 0.25);
    }
}
