//! `silica-glass-v1` lato server: i client dichiarano regioni di vetro sulle loro
//! superfici. Lo stato è doppio (si applica al `wl_surface.commit`) e le richieste sono
//! input ostile: dimensioni, raggio e materiale si controllano, e un client che sbaglia
//! riceve un errore di protocollo (disconnesso), mai un crash di Ossido.

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

/// Le due famiglie di vetro misurate (`plan/reference/glass.md`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Material {
    Clear,
    Thick,
    Regular,
    Notification,
}

/// La forma di una regione.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Shape {
    /// Rettangolo con angoli ad arco di cerchio.
    Rounded,
    /// Punta rivolta in basso (`add_tip`, versione 4): base sul lato superiore, vertice al
    /// centro del lato inferiore.
    Tip,
    /// Punta rivolta in alto (`add_upward_tip`, versione 5).
    UpwardTip,
}

/// Una regione di vetro, in pixel logici relativi alla superficie.
#[derive(Debug, Clone, Copy, PartialEq, serde::Serialize)]
pub struct GlassRegion {
    pub x: i32,
    pub y: i32,
    pub width: i32,
    pub height: i32,
    pub radius: f64,
    pub material: Material,
    /// Opacità con cui si compone il vetro (dalla versione 3; 1 con `add_region`).
    pub opacity: f64,
    pub shape: Shape,
    /// Le regioni si fondono solo nello stesso gruppo; un gruppo dopo si disegna sopra
    /// (`separate`, versione 4).
    pub group: u32,
}

/// Lo stato doppio del vetro di una superficie.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct GlassState {
    pub regions: Vec<GlassRegion>,
    /// Il gruppo delle prossime regioni.
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

/// La superficie ha già un oggetto vetro (uno solo per superficie).
#[derive(Default)]
struct HasGlass(AtomicBool);

/// Le regioni correnti di una superficie.
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
        dh.create_global::<State, SilicaGlassManagerV1, _>(5, ());
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
                        "la superficie ha già un oggetto vetro",
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
        // Superficie già distrutta: le richieste non hanno più effetto.
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
            | silica_glass_v1::Request::AddUpwardTip { .. } => {
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
                    _ => return,
                };
                let material = match material {
                    WEnum::Value(silica_glass_v1::Material::Clear) => Some(Material::Clear),
                    WEnum::Value(silica_glass_v1::Material::Thick) => Some(Material::Thick),
                    WEnum::Value(silica_glass_v1::Material::Regular) => Some(Material::Regular),
                    // Dalla versione 2 del protocollo.
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
                            "regione non valida: {width}x{height}, raggio {radius}, opacità {opacity}"
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
                        format!("oltre {MAX_REGIONS} regioni"),
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

/// Una superficie con vetro in `glass.list`.
#[derive(Debug, Clone, serde::Serialize)]
pub struct GlassSurface {
    /// Come in `perf.frames`: `window <id>` o `layer <namespace>`.
    pub surface: String,
    /// Origine della superficie nell'output, in pixel logici.
    pub x: i32,
    pub y: i32,
    pub regions: Vec<GlassRegion>,
}

impl State {
    /// Finestre e layer con regioni di vetro correnti.
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
