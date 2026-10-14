//! Server-side `silica-glass-v1`: clients declare glass regions on their surfaces. The state
//! is double-buffered (it applies on `wl_surface.commit`) and the requests are hostile
//! input: sizes, radius and material are checked, and a client that gets it wrong receives a
//! protocol error (disconnected), never an Ossido crash.

use std::sync::atomic::{AtomicBool, Ordering};

use silica_protocols::glass::MAX_REGIONS;
use silica_protocols::glass::server::silica_glass_manager_v1::{self, SilicaGlassManagerV1};
use silica_protocols::glass::server::silica_glass_v1::{self, SilicaGlassV1};
use smithay::reexports::wayland_server::protocol::wl_surface::WlSurface;
use smithay::reexports::wayland_server::{
    Client, DataInit, Dispatch, DisplayHandle, GlobalDispatch, New, Resource, WEnum, Weak,
};
use smithay::wayland::compositor::{Cacheable, with_states};

use crate::state::State;

/// The two measured glass families (`plan/reference/glass.md`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Material {
    Clear,
    Thick,
    Regular,
    Notification,
}

/// The shape of a region.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Shape {
    /// Rectangle with circular-arc corners.
    Rounded,
    /// Downward tip (`add_tip`, version 4): base on the top side, apex at the middle of the
    /// bottom side.
    Tip,
    /// Upward tip (`add_upward_tip`, version 5).
    UpwardTip,
    /// Tip pointing left (`add_side_tip`, version 6): base on the right side, apex at the
    /// middle of the left side.
    LeftTip,
    /// Tip pointing right (`add_side_tip`, version 6).
    RightTip,
}

/// A glass region, in logical pixels relative to the surface.
#[derive(Debug, Clone, Copy, PartialEq, serde::Serialize)]
pub struct GlassRegion {
    pub x: i32,
    pub y: i32,
    pub width: i32,
    pub height: i32,
    pub radius: f64,
    pub material: Material,
    /// Opacity the glass is composed with (since version 3; 1 with `add_region`).
    pub opacity: f64,
    pub shape: Shape,
    /// Regions merge only within the same group; a later group is drawn on top
    /// (`separate`, version 4).
    pub group: u32,
}

/// The double-buffered glass state of a surface.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct GlassState {
    pub regions: Vec<GlassRegion>,
    /// The group of the next regions.
    pub group: u32,
}

impl Cacheable for GlassState {
    fn commit(&mut self, _dh: &DisplayHandle) -> Self {
        self.clone()
    }

    fn merge_into(self, into: &mut Self, _dh: &DisplayHandle) {
        *into = self;
    }
}

/// The surface already has a glass object (only one per surface).
#[derive(Default)]
struct HasGlass(AtomicBool);

/// The current regions of a surface.
pub fn current_regions(surface: &WlSurface) -> Vec<GlassRegion> {
    with_states(surface, |states| {
        states
            .cached_state
            .get::<GlassState>()
            .current()
            .regions
            .clone()
    })
}

pub struct GlassManagerState;

impl GlassManagerState {
    pub fn new(dh: &DisplayHandle) -> Self {
        dh.create_global::<State, SilicaGlassManagerV1, _>(6, ());
        Self
    }
}

impl GlobalDispatch<SilicaGlassManagerV1, ()> for State {
    fn bind(
        _state: &mut Self,
        _dh: &DisplayHandle,
        _client: &Client,
        resource: New<SilicaGlassManagerV1>,
        _global_data: &(),
        data_init: &mut DataInit<'_, Self>,
    ) {
        data_init.init(resource, ());
    }
}

impl Dispatch<SilicaGlassManagerV1, ()> for State {
    fn request(
        _state: &mut Self,
        _client: &Client,
        manager: &SilicaGlassManagerV1,
        request: silica_glass_manager_v1::Request,
        _data: &(),
        _dh: &DisplayHandle,
        data_init: &mut DataInit<'_, Self>,
    ) {
        match request {
            silica_glass_manager_v1::Request::GetGlass { id, surface } => {
                let already = with_states(&surface, |states| {
                    states
                        .data_map
                        .insert_if_missing_threadsafe(HasGlass::default);
                    states
                        .data_map
                        .get::<HasGlass>()
                        .is_some_and(|has| has.0.swap(true, Ordering::SeqCst))
                });
                if already {
                    manager.post_error(
                        silica_glass_manager_v1::Error::AlreadyHasGlass,
                        "the surface already has a glass object",
                    );
                    return;
                }
                data_init.init(id, surface.downgrade());
            }
            silica_glass_manager_v1::Request::Destroy => {}
            _ => {}
        }
    }
}

impl Dispatch<SilicaGlassV1, Weak<WlSurface>> for State {
    fn request(
        _state: &mut Self,
        _client: &Client,
        glass: &SilicaGlassV1,
        request: silica_glass_v1::Request,
        surface: &Weak<WlSurface>,
        _dh: &DisplayHandle,
        _data_init: &mut DataInit<'_, Self>,
    ) {
        // Surface already destroyed: the requests have no effect anymore.
        let Ok(surface) = surface.upgrade() else {
            return;
        };
        let pending = |change: &mut dyn FnMut(&mut GlassState)| {
            with_states(&surface, |states| {
                change(states.cached_state.get::<GlassState>().pending());
            });
        };
        match request {
            silica_glass_v1::Request::Clear => pending(&mut |state| {
                state.regions.clear();
                state.group = 0;
            }),
            silica_glass_v1::Request::Separate => pending(&mut |state| {
                state.group = state.group.saturating_add(1);
            }),
            silica_glass_v1::Request::AddRegion { .. }
            | silica_glass_v1::Request::AddTranslucentRegion { .. }
            | silica_glass_v1::Request::AddTip { .. }
            | silica_glass_v1::Request::AddUpwardTip { .. }
            | silica_glass_v1::Request::AddSideTip { .. } => {
                let (x, y, width, height, radius, material, opacity, shape) = match request {
                    silica_glass_v1::Request::AddRegion {
                        x,
                        y,
                        width,
                        height,
                        radius,
                        material,
                    } => (x, y, width, height, radius, material, 1.0, Shape::Rounded),
                    silica_glass_v1::Request::AddTranslucentRegion {
                        x,
                        y,
                        width,
                        height,
                        radius,
                        material,
                        opacity,
                    } => (
                        x,
                        y,
                        width,
                        height,
                        radius,
                        material,
                        opacity,
                        Shape::Rounded,
                    ),
                    silica_glass_v1::Request::AddTip {
                        x,
                        y,
                        width,
                        height,
                        material,
                        opacity,
                    } => (x, y, width, height, 0.0, material, opacity, Shape::Tip),
                    silica_glass_v1::Request::AddUpwardTip {
                        x,
                        y,
                        width,
                        height,
                        material,
                        opacity,
                    } => (
                        x,
                        y,
                        width,
                        height,
                        0.0,
                        material,
                        opacity,
                        Shape::UpwardTip,
                    ),
                    silica_glass_v1::Request::AddSideTip {
                        x,
                        y,
                        width,
                        height,
                        side,
                        material,
                        opacity,
                    } => {
                        let shape = match side {
                            WEnum::Value(silica_glass_v1::Side::Left) => Shape::LeftTip,
                            WEnum::Value(silica_glass_v1::Side::Right) => Shape::RightTip,
                            _ => {
                                glass.post_error(
                                    silica_glass_v1::Error::InvalidRegion,
                                    "unknown tip side",
                                );
                                return;
                            }
                        };
                        (x, y, width, height, 0.0, material, opacity, shape)
                    }
                    _ => return,
                };
                let material = match material {
                    WEnum::Value(silica_glass_v1::Material::Clear) => Some(Material::Clear),
                    WEnum::Value(silica_glass_v1::Material::Thick) => Some(Material::Thick),
                    WEnum::Value(silica_glass_v1::Material::Regular) => Some(Material::Regular),
                    // Since version 2 of the protocol.
                    WEnum::Value(silica_glass_v1::Material::Notification)
                        if glass.version() >= 2 =>
                    {
                        Some(Material::Notification)
                    }
                    _ => None,
                };
                let valid = width > 0
                    && height > 0
                    && radius.is_finite()
                    && radius >= 0.0
                    && (0.0..=1.0).contains(&opacity);
                let Some(material) = material.filter(|_| valid) else {
                    glass.post_error(
                        silica_glass_v1::Error::InvalidRegion,
                        format!(
                            "invalid region: {width}x{height}, radius {radius}, opacity {opacity}"
                        ),
                    );
                    return;
                };
                let mut too_many = false;
                pending(&mut |state| {
                    if state.regions.len() >= MAX_REGIONS {
                        too_many = true;
                    } else {
                        state.regions.push(GlassRegion {
                            x,
                            y,
                            width,
                            height,
                            radius,
                            material,
                            opacity,
                            shape,
                            group: state.group,
                        });
                    }
                });
                if too_many {
                    glass.post_error(
                        silica_glass_v1::Error::TooManyRegions,
                        format!("over {MAX_REGIONS} regions"),
                    );
                }
            }
            silica_glass_v1::Request::Destroy => {
                pending(&mut |state| state.regions.clear());
                with_states(&surface, |states| {
                    if let Some(has) = states.data_map.get::<HasGlass>() {
                        has.0.store(false, Ordering::SeqCst);
                    }
                });
            }
            _ => {}
        }
    }
}

/// A surface with glass in `glass.list`.
#[derive(Debug, Clone, serde::Serialize)]
pub struct GlassSurface {
    /// As in `perf.frames`: `window <id>` or `layer <namespace>`.
    pub surface: String,
    /// Origin of the surface in the output, in logical pixels.
    pub x: i32,
    pub y: i32,
    pub regions: Vec<GlassRegion>,
}

impl State {
    /// Windows and layers with current glass regions.
    pub fn glass_list(&self) -> Vec<GlassSurface> {
        let mut list = Vec::new();
        for window in self.space.elements().rev() {
            let (Some(toplevel), Some(location)) =
                (window.toplevel(), self.space.element_location(window))
            else {
                continue;
            };
            let regions = current_regions(toplevel.wl_surface());
            if regions.is_empty() {
                continue;
            }
            let origin = location - window.geometry().loc;
            let id = Self::window_id(window).unwrap_or_default();
            list.push(GlassSurface {
                surface: format!("window {id}"),
                x: origin.x,
                y: origin.y,
                regions,
            });
        }
        let map = smithay::desktop::layer_map_for_output(&self.output);
        for layer in map.layers() {
            let regions = current_regions(layer.wl_surface());
            let Some(geometry) = map.layer_geometry(layer) else {
                continue;
            };
            if regions.is_empty() {
                continue;
            }
            list.push(GlassSurface {
                surface: format!("layer {}", layer.namespace()),
                x: geometry.loc.x,
                y: geometry.loc.y,
                regions,
            });
        }
        list
    }
}
