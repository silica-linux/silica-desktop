//! `silica-window-v1` lato server: un'app chiede il contenuto a tutta finestra. Ossido non
//! aggiunge la barra del titolo sopra il contenuto e disegna solo i pulsanti della finestra
//! sopra l'app, centrati nella sua barra degli strumenti. Lo stato è doppio (si applica al
//! `wl_surface.commit`); un client che chiede due oggetti per la stessa superficie riceve un
//! errore di protocollo, mai un crash di Ossido.

use std::sync::atomic::{AtomicBool, Ordering};

use silica_protocols::window::server::silica_window_manager_v1::{self, SilicaWindowManagerV1};
use silica_protocols::window::server::silica_window_v1::{self, SilicaWindowV1};
use smithay::reexports::wayland_server::protocol::wl_surface::WlSurface;
use smithay::reexports::wayland_server::{
    Client, DataInit, Dispatch, DisplayHandle, GlobalDispatch, New, Resource, Weak,
};
use smithay::wayland::compositor::{Cacheable, with_states};

use crate::state::State;

/// La barra degli strumenti più alta che si accetta (pixel logici).
pub const MAX_TOOLBAR: u32 = 200;

/// Lo stato doppio: l'altezza della barra degli strumenti dell'app (0: la barra di Ossido).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct AppearanceState {
    pub toolbar_height: u32,
}

impl Cacheable for AppearanceState {
    fn commit(&mut self, _dh: &DisplayHandle) -> Self {
        *self
    }

    fn merge_into(self, into: &mut Self, _dh: &DisplayHandle) {
        *into = self;
    }
}

#[derive(Default)]
struct HasWindow(AtomicBool);

/// Il contenuto a tutta finestra della superficie: l'altezza della barra degli strumenti.
pub fn full_size_toolbar(surface: &WlSurface) -> Option<u32> {
    let height = with_states(surface, |states| {
        states
            .cached_state
            .get::<AppearanceState>()
            .current()
            .toolbar_height
    });
    (height > 0).then_some(height)
}

pub struct WindowManagerState;

impl WindowManagerState {
    pub fn new(dh: &DisplayHandle) -> Self {
        dh.create_global::<State, SilicaWindowManagerV1, _>(1, ());
        Self
    }
}

impl GlobalDispatch<SilicaWindowManagerV1, ()> for State {
    fn bind(
        _state: &mut Self,
        _dh: &DisplayHandle,
        _client: &Client,
        resource: New<SilicaWindowManagerV1>,
        _global_data: &(),
        data_init: &mut DataInit<'_, Self>,
    ) {
        data_init.init(resource, ());
    }
}

impl Dispatch<SilicaWindowManagerV1, ()> for State {
    fn request(
        _state: &mut Self,
        _client: &Client,
        manager: &SilicaWindowManagerV1,
        request: silica_window_manager_v1::Request,
        _data: &(),
        _dh: &DisplayHandle,
        data_init: &mut DataInit<'_, Self>,
    ) {
        match request {
            silica_window_manager_v1::Request::GetWindow { id, surface } => {
                let already = with_states(&surface, |states| {
                    states
                        .data_map
                        .insert_if_missing_threadsafe(HasWindow::default);
                    states
                        .data_map
                        .get::<HasWindow>()
                        .is_some_and(|has| has.0.swap(true, Ordering::SeqCst))
                });
                if already {
                    manager.post_error(
                        silica_window_manager_v1::Error::AlreadyHasWindow,
                        "la superficie ha già un oggetto finestra",
                    );
                    return;
                }
                data_init.init(id, surface.downgrade());
            }
            silica_window_manager_v1::Request::Destroy => {}
            _ => {}
        }
    }
}

impl Dispatch<SilicaWindowV1, Weak<WlSurface>> for State {
    fn request(
        _state: &mut Self,
        _client: &Client,
        _window: &SilicaWindowV1,
        request: silica_window_v1::Request,
        surface: &Weak<WlSurface>,
        _dh: &DisplayHandle,
        _data_init: &mut DataInit<'_, Self>,
    ) {
        let Ok(surface) = surface.upgrade() else {
            return;
        };
        let set = |height: u32| {
            with_states(&surface, |states| {
                states
                    .cached_state
                    .get::<AppearanceState>()
                    .pending()
                    .toolbar_height = height.min(MAX_TOOLBAR);
            });
        };
        match request {
            silica_window_v1::Request::SetFullSizeContent { toolbar_height } => {
                set(toolbar_height);
            }
            silica_window_v1::Request::Destroy => {
                set(0);
                with_states(&surface, |states| {
                    if let Some(has) = states.data_map.get::<HasWindow>() {
                        has.0.store(false, Ordering::SeqCst);
                    }
                });
            }
            _ => {}
        }
    }
}
