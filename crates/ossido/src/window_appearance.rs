//! Server-side `silica-window-v1`: an app asks for full-size content. Ossido does not add the
//! title bar above the content and only draws the window buttons over the app, centered in
//! its toolbar. The state is double-buffered (it applies on `wl_surface.commit`); a client
//! asking for two objects for the same surface receives a protocol error, never an Ossido
//! crash.

use std::sync::atomic::{AtomicBool, Ordering};

use silica_protocols::window::server::silica_window_manager_v1::{self, SilicaWindowManagerV1};
use silica_protocols::window::server::silica_window_v1::{self, SilicaWindowV1};
use smithay::reexports::wayland_server::protocol::wl_surface::WlSurface;
use smithay::reexports::wayland_server::{
    Client, DataInit, Dispatch, DisplayHandle, GlobalDispatch, New, Resource, Weak,
};
use smithay::wayland::compositor::{Cacheable, with_states};

use crate::state::State;

/// The tallest toolbar accepted (logical pixels).
pub const MAX_TOOLBAR: u32 = 200;

/// The double-buffered state: the height of the app's toolbar (0: Ossido's bar).
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

/// The full-size content of the surface: the height of the toolbar.
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
        dh.create_global::<State, SilicaWindowManagerV1, _>(2, ());
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
                        "the surface already has a window object",
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
        state: &mut Self,
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
            silica_window_v1::Request::SetHidden { hidden } => {
                if hidden != 0 {
                    state.hide_window(&surface);
                } else {
                    state.show_window(&surface);
                }
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
