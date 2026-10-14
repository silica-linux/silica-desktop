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

/// Smoothstep: 3x² − 2x³ su [0, 1].
pub fn smoothstep(x: f64) -> f64 {
    let x = x.clamp(0.0, 1.0);
    x * x * (3.0 - 2.0 * x)
}

/// Riduzione nel Dock ("genio"), misurata sui fotogrammi di
/// `video/windows/minimize-restore-light.mov` (75 Hz, schermo 2560 × 1440, finestra 900 × 600 a
/// (700, 300), cella nel Dock a x ~1644–1698, cima del Dock a y 1356). Tempi dal primo
/// fotogramma piegato (960 ms del file); la miniatura è ferma a 1440 ms: **480 ms** in tutto
/// (le note contano 390 ms dal primo movimento del bordo superiore, a 1053 ms).
pub const MINIMIZE: Duration = Duration::from_millis(480);

/// Ripristino: 4868 → 5308/5335 ms del file, ~440–470 ms.
pub const RESTORE: Duration = Duration::from_millis(470);

/// Lo stato del genio a un istante: quanto è piegato l'imbuto (0–1) e quanto sono scesi il
/// bordo superiore e quello inferiore verso la miniatura (0 = finestra, 1 = miniatura).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct GenieShape {
    pub bend: f64,
    pub top: f64,
    pub bottom: f64,
}

/// La forma del genio dopo `ms` millisecondi di riduzione.
///
/// - Piega: il profilo dei bordi, una volta piegato, è esattamente una smoothstep fra la cima
///   della finestra e la cima del Dock (bordo sinistro a 1213 ms: frazione 0,389 / 0,553 /
///   0,712 / 0,850 / 0,950 contro 0,399 / 0,557 / 0,710 / 0,844 / 0,939). La piega cresce come
///   smoothstep(t / 240): 0,04 a 13 ms, 0,22 a 67, 0,41 a 107, 0,62 a 133, 0,80 a 173, 0,97 a 227.
/// - Bordo superiore: smoothstep((t − 75) / 380): y 308 a 93 ms, 384 a 133, 532 a 187, 731 a
///   253, 947 a 293, 1145 a 347, 1291 a 400, 1349 a 440 (frazione misurata 0,22 / 0,41 / 0,61 /
///   0,80 / 0,94 contro 0,21 / 0,45 / 0,62 / 0,80 / 0,94).
/// - Bordo inferiore: entra nel Dock prima, smoothstep((t − 95) / 185): y ~1060 a 173 ms, 1131 a
///   187, 1260 a 227, 1330 a 253, nel Dock a 280.
pub fn genie_minimize(ms: f64) -> GenieShape {
    GenieShape {
        bend: smoothstep(ms / 240.0),
        top: smoothstep((ms - 75.0) / 380.0),
        bottom: smoothstep((ms - 95.0) / 185.0),
    }
}

/// La forma del genio dopo `ms` millisecondi di ripristino (470 in tutto).
///
/// - Bordo superiore (`windows.md`, ripristino): esce subito e veloce, poi rallenta: frazione
///   0,944 al primo fotogramma, 0,81 a ~40 ms, 0,124 a 267, 0,045 a 307, 0,019 a 320, 0 a 360:
///   (1 − t / 370)^1,7.
/// - Piega: il bordo sinistro torna a x 700 fra 267 e 320 ms (734, 711, 701, 697, 700): la
///   piega si scioglie fra 130 e 330 ms.
/// - Bordo inferiore: l'inverso della riduzione (è l'ultimo a uscire dal Dock), sulla durata
///   del ripristino.
pub fn genie_restore(ms: f64) -> GenieShape {
    let total = RESTORE.as_secs_f64() * 1000.0;
    let minimize = MINIMIZE.as_secs_f64() * 1000.0;
    let mirrored = genie_minimize(minimize - ms.clamp(0.0, total) * minimize / total);
    GenieShape {
        bend: 1.0 - smoothstep((ms - 130.0) / 200.0),
        top: (1.0 - ms / 370.0).clamp(0.0, 1.0).powf(1.7),
        bottom: mirrored.bottom,
    }
}

/// Schermo intero, entrata (`windows.md`, "Schermo intero", 1000 → 1520 ms del file): il
/// rettangolo della finestra va dalla cornice allo schermo. Frazione misurata sul bordo destro
/// (il sinistro coincide fino a ~200 ms, poi tocca il bordo dello schermo): 0,06 a 53 ms, 0,12 a
/// 67, 0,22 a 80, 0,417 a 107, 0,56 a 133, 0,66 a 160, 0,80 a 213, 0,91 a 293, 0,95 a 347, 0,98
/// a 413, 1 a 520: parte accelerando (potenza 1,5 da 40 a 107 ms), poi rallenta come
/// 1 − e^(−t/100).
pub const FULLSCREEN_ENTER: Duration = Duration::from_millis(520);

/// Schermo intero, uscita (4988 → 5441 ms del file): altezza 1300 → 600 pt, veloce all'inizio e
/// lenta alla fine; frazione 0,24 a 67 ms, 0,43 a 94, 0,56 a 120, 0,74 a 174, 0,84 a 227, 0,92
/// a 280, 0,96 a 334, 0,99 a 400: 1 − e^(−(t − 40)/90), normalizzata a 1 a 453 ms.
pub const FULLSCREEN_EXIT: Duration = Duration::from_millis(453);

/// Frazione del rettangolo dopo `ms` di entrata nello schermo intero.
pub fn fullscreen_enter(ms: f64) -> f64 {
    const KNEE: f64 = 107.0;
    const AT_KNEE: f64 = 0.417;
    if ms <= 40.0 {
        return 0.0;
    }
    if ms < KNEE {
        return AT_KNEE * ((ms - 40.0) / (KNEE - 40.0)).powf(1.5);
    }
    let end = FULLSCREEN_ENTER.as_secs_f64() * 1000.0;
    let tail = |t: f64| (-(t - KNEE) / 100.0).exp();
    let norm = 1.0 - tail(end);
    (AT_KNEE + (1.0 - AT_KNEE) * (1.0 - tail(ms)) / norm).min(1.0)
}

/// Frazione del rettangolo dopo `ms` di uscita dallo schermo intero.
pub fn fullscreen_exit(ms: f64) -> f64 {
    if ms <= 40.0 {
        return 0.0;
    }
    let end = FULLSCREEN_EXIT.as_secs_f64() * 1000.0;
    let curve = |t: f64| 1.0 - (-(t - 40.0) / 90.0).exp();
    (curve(ms) / curve(end)).min(1.0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn genie_follows_the_measured_frames() {
        // Bordo superiore: (ms, y) da 300 a 1356.
        for (ms, y) in [
            (133.0, 384.0),
            (187.0, 532.0),
            (293.0, 947.0),
            (347.0, 1145.0),
            (440.0, 1349.0),
        ] {
            let measured = (y - 300.0) / 1056.0;
            let model = genie_minimize(ms).top;
            assert!(
                (model - measured).abs() <= 0.06,
                "{ms} ms: {model} contro {measured}"
            );
        }
        // Piega.
        for (ms, measured) in [(67.0, 0.22), (133.0, 0.62), (173.0, 0.80), (227.0, 0.97)] {
            let model = genie_minimize(ms).bend;
            assert!(
                (model - measured).abs() <= 0.07,
                "{ms} ms: {model} contro {measured}"
            );
        }
        assert_eq!(genie_minimize(0.0).top, 0.0);
        let end = genie_minimize(480.0);
        assert!(end.top >= 0.999 && end.bottom >= 0.999 && end.bend >= 0.999);
    }

    #[test]
    fn restore_is_a_genie_coming_out() {
        let start = genie_restore(0.0);
        assert!(start.top > 0.99 && start.bottom > 0.99 && start.bend > 0.99);
        let end = genie_restore(470.0);
        assert!(end.top < 0.001 && end.bottom < 0.001 && end.bend < 0.001);
        // Bordo superiore: 0,124 a 267 ms, 0,045 a 307.
        assert!((genie_restore(267.0).top - 0.124).abs() < 0.03);
        assert!((genie_restore(307.0).top - 0.045).abs() < 0.03);
    }

    #[test]
    fn fullscreen_follows_the_measured_frames() {
        for (ms, measured) in [
            (67.0, 0.123),
            (80.0, 0.217),
            (107.0, 0.417),
            (133.0, 0.561),
            (160.0, 0.663),
            (213.0, 0.797),
            (293.0, 0.909),
            (413.0, 0.984),
        ] {
            let model = fullscreen_enter(ms);
            assert!(
                (model - measured).abs() <= 0.05,
                "entrata {ms} ms: {model} contro {measured}"
            );
        }
        assert_eq!(fullscreen_enter(520.0), 1.0);
        for (ms, measured) in [
            (67.0, 0.24),
            (120.0, 0.563),
            (200.0, 0.799),
            (307.0, 0.94),
            (400.0, 0.991),
        ] {
            let model = fullscreen_exit(ms);
            assert!(
                (model - measured).abs() <= 0.05,
                "uscita {ms} ms: {model} contro {measured}"
            );
        }
        assert_eq!(fullscreen_exit(453.0), 1.0);
    }

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
