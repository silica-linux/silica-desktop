//! The `silica/glass` channel: the glass regions declared by `silica_ui`'s `Glass` widgets,
//! passed to Ossido with `silica-glass-v1` (`docs/protocols.md`).
//!
//! The message comes from the app's Dart code: it is checked here (positive sizes, finite
//! radius, at most 64 regions) so that a wrong value does not become a protocol error that
//! would disconnect the app.

use std::collections::VecDeque;
use std::sync::{Arc, Mutex};

use serde_json::{Value, json};
use silica_protocols::glass::MAX_REGIONS;
use silica_protocols::glass::client::silica_glass_manager_v1::SilicaGlassManagerV1;
use silica_protocols::glass::client::silica_glass_v1::{
    Material, Profile, Shadow, Side, SilicaGlassV1,
};
use wayland_client::protocol::wl_surface::WlSurface;
use wayland_client::{Connection, Dispatch, Proxy, QueueHandle};

pub const CHANNEL: &str = "silica/glass";

/// A region in logical pixels of the surface.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Region {
    pub x: i32,
    pub y: i32,
    pub width: i32,
    pub height: i32,
    pub radius: f64,
    pub material: Material,
    /// 0–1: the glass is composed with this opacity (silica-glass-v1 version 3).
    pub opacity: f64,
    /// A tip instead of a rectangle: down (version 4, `add_tip`), up (version 5,
    /// `add_upward_tip`), left or right (version 6, `add_side_tip`).
    pub tip: Option<Tip>,
    /// Regions merge only within the same group; a higher group is drawn on top
    /// (version 4, `separate`). Signed: a shell puts what must stay under its menus (group 0
    /// and up, as `silica_ui` numbers them) in negative groups.
    pub group: i32,
    /// The kind of surface (`profile`: `menu`, `panel`, `module`, `control`, `dock`,
    /// `tooltip`, `sheet`, `banner`, `popover`; version 7, `set_profile`). Default: from the material.
    pub profile: Profile,
    /// Ossido draws the profile's shadow (`shadow`: true; only with a profile).
    pub shadow: bool,
}

/// The profile named in the channel's JSON (unknown names: from the material).
fn profile_named(name: Option<&str>) -> Profile {
    match name {
        Some("menu") => Profile::Menu,
        Some("panel") => Profile::Panel,
        Some("module") => Profile::Module,
        Some("control") => Profile::Control,
        Some("dock") => Profile::Dock,
        Some("tooltip") => Profile::Tooltip,
        Some("sheet") => Profile::Sheet,
        Some("banner") => Profile::Banner,
        Some("popover") => Profile::Popover,
        _ => Profile::Default,
    }
}

/// Where a tip points (`shape`: `tip`, `tip-up`, `tip-left`, `tip-right`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Tip {
    Down,
    Up,
    Left,
    Right,
}

impl Tip {
    /// The protocol version that draws it.
    fn since(self) -> u32 {
        match self {
            Self::Down => 4,
            Self::Up => 5,
            Self::Left | Self::Right => 6,
        }
    }
}

impl Region {
    /// A region from the JSON of `setRegions`, if valid.
    fn from_json(value: &Value) -> Option<Self> {
        let int = |name: &str| {
            value
                .get(name)
                .and_then(Value::as_f64)
                .filter(|v| v.is_finite())
                .map(|v| v.round().clamp(f64::from(i32::MIN), f64::from(i32::MAX)) as i32)
        };
        let radius = value
            .get("radius")
            .and_then(Value::as_f64)
            .filter(|r| r.is_finite() && *r >= 0.0)
            .unwrap_or(0.0);
        let material = match value.get("material").and_then(Value::as_str) {
            Some("thick") => Material::Thick,
            Some("regular") => Material::Regular,
            Some("notification") => Material::Notification,
            _ => Material::Clear,
        };
        // Opacity from fading ancestors; without it, full.
        let opacity = value
            .get("opacity")
            .and_then(Value::as_f64)
            .filter(|o| o.is_finite())
            .map_or(1.0, |o| o.clamp(0.0, 1.0));
        let region = Self {
            x: int("x")?,
            y: int("y")?,
            width: int("width")?,
            height: int("height")?,
            radius,
            material,
            opacity,
            tip: match value.get("shape").and_then(Value::as_str) {
                Some("tip") => Some(Tip::Down),
                Some("tip-up") => Some(Tip::Up),
                Some("tip-left") => Some(Tip::Left),
                Some("tip-right") => Some(Tip::Right),
                _ => None,
            },
            group: value.get("group").and_then(Value::as_i64).map_or(0, |g| {
                i32::try_from(g).unwrap_or(if g < 0 { i32::MIN } else { i32::MAX })
            }),
            profile: profile_named(value.get("profile").and_then(Value::as_str)),
            shadow: false,
        };
        let region = Self {
            shadow: region.profile != Profile::Default
                && value.get("shadow").and_then(Value::as_bool) == Some(true),
            ..region
        };
        (region.width > 0 && region.height > 0).then_some(region)
    }
}

/// The valid regions of a `setRegions`, at most `MAX_REGIONS`.
pub fn parse_regions(args: &Value) -> Vec<Region> {
    args.as_array()
        .map(|list| {
            let mut regions: Vec<Region> = list
                .iter()
                .filter_map(Region::from_json)
                .take(MAX_REGIONS)
                .collect();
            // By group (stable): `separate` splits the regions in order.
            regions.sort_by_key(|r| r.group);
            regions
        })
        .unwrap_or_default()
}

/// The glass of the app surface.
pub struct Glass {
    manager: Option<SilicaGlassManagerV1>,
    /// The regions last asked by the app.
    current: Vec<Region>,
    sync: Arc<GlassSync>,
}

impl Glass {
    pub fn new(manager: Option<SilicaGlassManagerV1>, sync: Arc<GlassSync>) -> Self {
        Self {
            manager,
            current: Vec::new(),
            sync,
        }
    }

    /// Takes in a channel call, as soon as the platform task delivering it runs (one main
    /// loop turn sooner than the reply). `raster_seq` is the raster task that presents the
    /// frame the regions were measured on (`PlatformMessage::raster_seq`). Returns whether the
    /// surface must be committed now: the regions go with their frame, and only when that
    /// frame is already on screen they need a commit of their own.
    pub fn receive<D>(
        &mut self,
        message: &[u8],
        raster_seq: u64,
        surface: &WlSurface,
        qh: &QueueHandle<D>,
    ) -> bool
    where
        D: Dispatch<SilicaGlassV1, ()> + 'static,
    {
        let Ok(call) = serde_json::from_slice::<Value>(message) else {
            return false;
        };
        match call.get("method").and_then(Value::as_str) {
            Some("setRegions") => {
                let regions = parse_regions(call.get("args").unwrap_or(&Value::Null));
                self.set_regions(regions, raster_seq, surface, qh)
            }
            _ => false,
        }
    }

    /// The reply to a channel call (JSON method codec), once [`Glass::receive`] took it in.
    pub fn reply(&self, message: &[u8]) -> Vec<u8> {
        let Ok(call) = serde_json::from_slice::<Value>(message) else {
            return Vec::new();
        };
        match call.get("method").and_then(Value::as_str) {
            // `isSupported`: does Ossido offer glass? Without it, the app draws an opaque
            // background.
            Some("setRegions" | "isSupported") => reply(json!(self.manager.is_some())),
            _ => Vec::new(),
        }
    }

    fn set_regions<D>(
        &mut self,
        regions: Vec<Region>,
        raster_seq: u64,
        surface: &WlSurface,
        qh: &QueueHandle<D>,
    ) -> bool
    where
        D: Dispatch<SilicaGlassV1, ()> + 'static,
    {
        if regions == self.current {
            return false;
        }
        let Some(manager) = &self.manager else {
            return false;
        };
        self.sync
            .ensure_object(|| manager.get_glass(surface, qh, ()));
        self.current = regions.clone();
        self.sync.submit(raster_seq, regions)
    }
}

/// The glass regions on their way to the compositor, in step with the frames.
///
/// Dart sends the regions after the frame they were measured on has gone to the raster thread
/// (`GlassRegistry` runs after the paint). Sent at once, the next commit would apply them: the
/// commit asking for the next vsync, a frame ahead of their content (a double edge while a
/// panel opens). Sent with a commit of their own after a delay, they would stay on screen after
/// a closing panel's last frame (the outline of the Control Center's tiles left for a moment).
/// Here they wait for the present of the raster task that draws their frame and are sent just
/// before it, so the frame's own commit applies them; if that frame is already on screen, or
/// the task draws nothing, they are committed right away.
pub struct GlassSync {
    conn: Connection,
    surface: WlSurface,
    state: Mutex<SyncState>,
}

#[derive(Default)]
struct SyncState {
    object: Option<SilicaGlassV1>,
    /// Regions waiting for the frame of their raster task, in order.
    pending: VecDeque<(u64, Vec<Region>)>,
    /// The last raster task whose frame is committed (or that has run without a frame).
    done: u64,
    /// A software frame drawn by this raster task, not yet committed by the main thread.
    software_frame: Option<u64>,
}

impl GlassSync {
    pub fn new(conn: Connection, surface: WlSurface) -> Self {
        Self {
            conn,
            surface,
            state: Mutex::new(SyncState::default()),
        }
    }

    fn ensure_object(&self, create: impl FnOnce() -> SilicaGlassV1) {
        if let Ok(mut state) = self.state.lock()
            && state.object.is_none()
        {
            state.object = Some(create());
        }
    }

    /// New regions measured on the frame of raster task `seq`. True if the caller must commit
    /// the surface now (their frame is already on screen).
    fn submit(&self, seq: u64, regions: Vec<Region>) -> bool {
        let Ok(mut state) = self.state.lock() else {
            return false;
        };
        if seq <= state.done {
            state.pending.clear();
            if let Some(object) = &state.object {
                send_regions(object, &regions);
            }
            return true;
        }
        state.pending.push_back((seq, regions));
        false
    }

    /// The frame of raster task `seq` is about to be committed: its regions go with it. On
    /// the raster thread (GL, Vulkan) or on the main thread (software frames).
    pub fn before_present(&self, seq: u64) {
        let Ok(mut state) = self.state.lock() else {
            return;
        };
        state.send_due(seq);
        state.done = state.done.max(seq);
        if state.software_frame.is_some_and(|frame| frame <= seq) {
            state.software_frame = None;
        }
    }

    /// The software renderer drew a frame in raster task `seq`: the main thread commits it.
    pub fn software_frame(&self, seq: u64) {
        if let Ok(mut state) = self.state.lock() {
            state.software_frame = Some(seq);
        }
    }

    /// Raster task `seq` has run (on the raster thread). If regions waited for it and it
    /// committed no frame (a task that draws nothing, a frame dropped while resizing), they
    /// are committed now on their own.
    pub fn after_task(&self, seq: u64) {
        let Ok(mut state) = self.state.lock() else {
            return;
        };
        if state.software_frame.is_some() {
            // The main thread commits that frame, with what goes with it.
            return;
        }
        state.done = state.done.max(seq);
        if state.send_due(seq) {
            self.surface.commit();
            if let Err(err) = self.conn.flush() {
                tracing::warn!("glass commit: {err}");
            }
        }
    }
}

impl SyncState {
    /// Sends the latest regions waiting for a raster task up to `seq`; true if any.
    fn send_due(&mut self, seq: u64) -> bool {
        let mut due = None;
        while self.pending.front().is_some_and(|(at, _)| *at <= seq) {
            due = self.pending.pop_front().map(|(_, regions)| regions);
        }
        match (due, &self.object) {
            (Some(regions), Some(object)) => {
                send_regions(object, &regions);
                true
            }
            _ => false,
        }
    }
}

/// The requests that replace the surface's regions (applied at the next commit).
fn send_regions(object: &SilicaGlassV1, regions: &[Region]) {
    object.clear();
    // An Ossido with version 1 of the protocol does not know the notification glass:
    // the closest one is the thick glass.
    let notification = object.version() >= 2;
    let translucent = object.version() >= 3;
    let groups = object.version() >= 4;
    let profiles = object.version() >= 7;
    let mut group = regions.first().map_or(0, |r| r.group);
    // After `clear` the regions take the default profile, the client's shadow.
    let mut profile = (Profile::Default, false);
    for region in regions {
        if region.tip.is_some_and(|tip| object.version() < tip.since()) {
            // An earlier Ossido cannot draw this tip.
            continue;
        }
        if groups && region.group != group {
            object.separate();
            group = region.group;
        }
        if profiles && (region.profile, region.shadow) != profile {
            profile = (region.profile, region.shadow);
            object.set_profile(
                region.profile,
                if region.shadow {
                    Shadow::Compositor
                } else {
                    Shadow::Client
                },
            );
        }
        let material = match region.material {
            Material::Notification if !notification => Material::Thick,
            material => material,
        };
        let (x, y, w, h, opacity) = (
            region.x,
            region.y,
            region.width,
            region.height,
            region.opacity,
        );
        if let Some(tip) = region.tip {
            match tip {
                Tip::Down => object.add_tip(x, y, w, h, material, opacity),
                Tip::Up => object.add_upward_tip(x, y, w, h, material, opacity),
                Tip::Left => object.add_side_tip(x, y, w, h, Side::Left, material, opacity),
                Tip::Right => object.add_side_tip(x, y, w, h, Side::Right, material, opacity),
            }
        } else if translucent && region.opacity < 1.0 {
            object.add_translucent_region(
                region.x,
                region.y,
                region.width,
                region.height,
                region.radius,
                material,
                region.opacity,
            );
        } else if region.opacity >= 0.5 || translucent {
            // An Ossido without version 3 composes the glass at full opacity: better no
            // glass for an almost invisible region.
            object.add_region(
                region.x,
                region.y,
                region.width,
                region.height,
                region.radius,
                material,
            );
        }
    }
}

use crate::text_input::success as reply;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn regions_are_validated_before_reaching_ossido() {
        let args = json!([
            {"x": 10, "y": 20.4, "width": 100, "height": 50, "radius": 22, "material": "clear"},
            {"x": 0, "y": 0, "width": 30, "height": 30, "material": "thick"},
            {"x": 0, "y": 0, "width": -1, "height": 30},
            {"x": 0, "y": 0, "width": 10, "height": 10, "radius": -3},
            {"y": 0, "width": 10, "height": 10},
            "not a region"
        ]);
        let regions = parse_regions(&args);
        assert_eq!(regions.len(), 3);
        assert_eq!((regions[0].y, regions[0].radius), (20, 22.0));
        assert_eq!(regions[1].material, Material::Thick);
        // Negative radius: 0 is used, the region stays.
        assert_eq!(regions[2].radius, 0.0);
        let many = Value::Array(vec![json!({"x": 0, "y": 0, "width": 1, "height": 1}); 100]);
        assert_eq!(parse_regions(&many).len(), MAX_REGIONS);
        assert!(parse_regions(&json!(null)).is_empty());
    }

    #[test]
    fn opacity_defaults_to_full_and_is_clamped() {
        let regions = parse_regions(&json!([
            {"x": 0, "y": 0, "width": 10, "height": 10},
            {"x": 0, "y": 0, "width": 10, "height": 10, "opacity": 0.25},
            {"x": 0, "y": 0, "width": 10, "height": 10, "opacity": 7},
            {"x": 0, "y": 0, "width": 10, "height": 10, "opacity": -1},
            {"x": 0, "y": 0, "width": 10, "height": 10, "opacity": "half"}
        ]));
        let opacities: Vec<f64> = regions.iter().map(|r| r.opacity).collect();
        assert_eq!(opacities, [1.0, 0.25, 1.0, 0.0, 1.0]);
    }

    #[test]
    fn profiles_are_parsed_and_the_shadow_needs_one() {
        let regions = parse_regions(&json!([
            {"x": 0, "y": 0, "width": 10, "height": 10},
            {"x": 0, "y": 0, "width": 10, "height": 10, "profile": "menu"},
            {"x": 0, "y": 0, "width": 10, "height": 10, "profile": "tooltip", "shadow": true},
            {"x": 0, "y": 0, "width": 10, "height": 10, "shadow": true},
            {"x": 0, "y": 0, "width": 10, "height": 10, "profile": "bubble", "shadow": true},
            {"x": 0, "y": 0, "width": 10, "height": 10, "profile": 3, "shadow": "yes"}
        ]));
        let summary: Vec<(Profile, bool)> = regions.iter().map(|r| (r.profile, r.shadow)).collect();
        assert_eq!(
            summary,
            [
                (Profile::Default, false),
                (Profile::Menu, false),
                (Profile::Tooltip, true),
                // No profile: no shadow to draw; unknown names fall back to the material.
                (Profile::Default, false),
                (Profile::Default, false),
                (Profile::Default, false)
            ]
        );
    }

    #[test]
    fn tips_and_groups_are_parsed_and_groups_sorted() {
        let regions = parse_regions(&json!([
            {"x": 0, "y": 0, "width": 10, "height": 10, "group": 1},
            {"x": 0, "y": 0, "width": 22, "height": 8, "shape": "tip"},
            {"x": 0, "y": 0, "width": 10, "height": 10, "group": "one"},
            {"x": 0, "y": 0, "width": 22, "height": 8, "shape": "tip-up"},
            {"x": 0, "y": 0, "width": 8, "height": 22, "shape": "tip-left"},
            {"x": 0, "y": 0, "width": 8, "height": 22, "shape": "tip-right"},
            {"x": 0, "y": 0, "width": 8, "height": 22, "shape": "tip-sideways"}
        ]));
        let summary: Vec<(i32, Option<Tip>)> = regions.iter().map(|r| (r.group, r.tip)).collect();
        assert_eq!(
            summary,
            [
                (0, Some(Tip::Down)),
                (0, None),
                (0, Some(Tip::Up)),
                (0, Some(Tip::Left)),
                (0, Some(Tip::Right)),
                (0, None),
                (1, None)
            ]
        );
    }

    #[test]
    fn negative_groups_sort_below_zero_and_out_of_range_ones_saturate() {
        let regions = parse_regions(&json!([
            {"x": 0, "y": 0, "width": 10, "height": 10},
            {"x": 0, "y": 0, "width": 10, "height": 10, "group": 1},
            {"x": 0, "y": 0, "width": 10, "height": 10, "group": -900},
            {"x": 0, "y": 0, "width": 10, "height": 10, "group": -1000},
            {"x": 0, "y": 0, "width": 10, "height": 10, "group": -99999999999i64},
            {"x": 0, "y": 0, "width": 10, "height": 10, "group": 99999999999i64}
        ]));
        let groups: Vec<i32> = regions.iter().map(|r| r.group).collect();
        assert_eq!(groups, [i32::MIN, -1000, -900, 0, 1, i32::MAX]);
    }
}
