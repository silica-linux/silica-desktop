//! Servizi di sistema di Silica (D-Bus).
//!
//! Oggi le notifiche: il server `org.freedesktop.Notifications` per le app e l'interfaccia
//! `dev.frascella.Silica.Notifications1` per la shell (`docs/daemon.md`).

pub mod notifications;
pub mod server;

pub use server::Daemon;
