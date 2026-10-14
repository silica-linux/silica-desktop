//! XML e binding dei protocolli silica-*.
//!
//! - `silica-glass-v1` (`xml/silica-glass-v1.xml`): regioni di vetro di una superficie.
//! - `silica-shell-v1` (`xml/silica-shell-v1.xml`): comandi della shell verso Ossido.
//! - `silica-window-v1` (`xml/silica-window-v1.xml`): aspetto della finestra di un'app
//!   (contenuto a tutta finestra con i pulsanti di Ossido sopra).
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

/// `silica-shell-v1`.
pub mod shell {
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
            wayland_scanner::generate_interfaces!("xml/silica-shell-v1.xml");
        }
        use self::__interfaces::*;

        wayland_scanner::generate_server_code!("xml/silica-shell-v1.xml");
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
            wayland_scanner::generate_interfaces!("xml/silica-shell-v1.xml");
        }
        use self::__interfaces::*;

        wayland_scanner::generate_client_code!("xml/silica-shell-v1.xml");
    }

    /// Variabile d'ambiente che Ossido mette ai componenti della shell che lancia: solo i
    /// loro processi vedono `silica_shell_manager_v1` e l'elenco delle finestre.
    pub const SHELL_ENV: &str = "SILICA_SHELL";
}

/// `silica-window-v1`.
pub mod window {
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
            wayland_scanner::generate_interfaces!("xml/silica-window-v1.xml");
        }
        use self::__interfaces::*;

        wayland_scanner::generate_server_code!("xml/silica-window-v1.xml");
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
            wayland_scanner::generate_interfaces!("xml/silica-window-v1.xml");
        }
        use self::__interfaces::*;

        wayland_scanner::generate_client_code!("xml/silica-window-v1.xml");
    }
}
