//! XML e binding dei protocolli silica-*.
//!
//! - `silica-glass-v1` (`xml/silica-glass-v1.xml`): regioni di vetro di una superficie.
//!
//! Il lato server (`server`) serve a Ossido, il lato client (`client`) all'embedder.

/// `silica-glass-v1`.
pub mod glass {
    #[cfg(feature = "server")]
    #[allow(
        missing_docs,
        clippy::all,
        clippy::pedantic,
        unused_imports,
        non_upper_case_globals
    )]
    pub mod server {
        use wayland_server;
        use wayland_server::protocol::*;

        pub mod __interfaces {
            use wayland_server::protocol::__interfaces::*;
            wayland_scanner::generate_interfaces!("xml/silica-glass-v1.xml");
        }
        use self::__interfaces::*;

        wayland_scanner::generate_server_code!("xml/silica-glass-v1.xml");
    }

    #[cfg(feature = "client")]
    #[allow(
        missing_docs,
        clippy::all,
        clippy::pedantic,
        unused_imports,
        non_upper_case_globals
    )]
    pub mod client {
        use wayland_client;
        use wayland_client::protocol::*;

        pub mod __interfaces {
            use wayland_client::protocol::__interfaces::*;
            wayland_scanner::generate_interfaces!("xml/silica-glass-v1.xml");
        }
        use self::__interfaces::*;

        wayland_scanner::generate_client_code!("xml/silica-glass-v1.xml");
    }

    /// Regioni in attesa oltre le quali la richiesta è un errore del client.
    pub const MAX_REGIONS: usize = 64;
}
