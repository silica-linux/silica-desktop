//! Servizi di sistema di Silica (D-Bus).
//!
//! Le notifiche (il server `org.freedesktop.Notifications` per le app e l'interfaccia
//! `dev.frascella.Silica.Notifications1` per la shell) e lo stato del sistema per il Centro
//! di controllo (`dev.frascella.Silica.System1`). `docs/daemon.md`.

pub mod notifications;
pub mod server;
pub mod system;

pub use server::Daemon;
pub use system::{SystemConfig, SystemService};
