//! Silica system services (D-Bus).
//!
//! Notifications (the `org.freedesktop.Notifications` server for apps and the
//! `dev.frascella.Silica.Notifications1` interface for the shell) and the system state for the
//! Control Center (`dev.frascella.Silica.System1`). See `docs/daemon.md`.

pub mod actions;
pub mod clipboard;
pub mod notifications;
pub mod server;
pub mod system;

pub use clipboard::ClipboardService;
pub use server::Daemon;
pub use system::{SystemConfig, SystemService};
