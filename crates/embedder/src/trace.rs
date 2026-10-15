//! Startup timestamps, off unless `SILICA_STARTUP_TRACE` is set: one line per phase on
//! stderr with the wall clock (microseconds since the epoch, comparable with Ossido's and
//! the shell's logs) and the milliseconds since the first mark of the process.

use std::sync::OnceLock;
use std::time::{Instant, SystemTime, UNIX_EPOCH};

static START: OnceLock<Instant> = OnceLock::new();
static ENABLED: OnceLock<bool> = OnceLock::new();

pub fn enabled() -> bool {
    *ENABLED.get_or_init(|| std::env::var_os("SILICA_STARTUP_TRACE").is_some())
}

/// Logs `phase` with its time, when the trace is on.
pub fn mark(phase: &str) {
    if !enabled() {
        return;
    }
    let start = START.get_or_init(Instant::now);
    let epoch = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| d.as_micros());
    eprintln!(
        "startup-trace epoch_us={epoch} +{:.1}ms {phase}",
        start.elapsed().as_secs_f64() * 1000.0
    );
}

/// Logs `phase` only the first time it is called (`once` is the caller's flag).
pub fn mark_once(once: &std::sync::atomic::AtomicBool, phase: &str) {
    if enabled() && !once.swap(true, std::sync::atomic::Ordering::Relaxed) {
        mark(phase);
    }
}
