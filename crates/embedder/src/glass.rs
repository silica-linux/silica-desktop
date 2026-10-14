//! The `silica/glass` channel: the glass regions declared by `silica_ui`'s `Glass` widgets,
//! passed to Ossido with `silica-glass-v1` (`docs/protocols.md`).
//!
//! The message comes from the app's Dart code: it is checked here (positive sizes, finite
//! radius, at most 64 regions) so that a wrong value does not become a protocol error that
//! would disconnect the app.

use serde_json::{Value, json};
use silica_protocols::glass::MAX_REGIONS;
use silica_protocols::glass::client::silica_glass_manager_v1::SilicaGlassManagerV1;
use silica_protocols::glass::client::silica_glass_v1::{Material, Side, SilicaGlassV1};
use wayland_client::protocol::wl_surface::WlSurface;
use wayland_client::{Dispatch, Proxy, QueueHandle};

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
    /// (version 4, `separate`).
    pub group: u32,
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
            group: value
                .get("group")
                .and_then(Value::as_u64)
                .map_or(0, |g| u32::try_from(g).unwrap_or(u32::MAX)),
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
    object: Option<SilicaGlassV1>,
    current: Vec<Region>,
}

impl Glass {
    pub fn new(manager: Option<SilicaGlassManagerV1>) -> Self {
        Self {
            manager,
            object: None,
            current: Vec::new(),
        }
    }

    /// Handles a channel call. Returns the reply (JSON method codec) and whether the regions
    /// changed: in that case the surface needs a commit.
    pub fn handle_call<D>(
        &mut self,
        message: &[u8],
        surface: &WlSurface,
        qh: &QueueHandle<D>,
    ) -> (Vec<u8>, bool)
    where
        D: Dispatch<SilicaGlassV1, ()> + 'static,
    {
        let Ok(call) = serde_json::from_slice::<Value>(message) else {
            return (Vec::new(), false);
        };
        match call.get("method").and_then(Value::as_str) {
            Some("setRegions") => {
                let regions = parse_regions(call.get("args").unwrap_or(&Value::Null));
                let changed = self.set_regions(regions, surface, qh);
                (reply(json!(self.manager.is_some())), changed)
            }
            // Does Ossido offer glass? Without it, the app draws an opaque background.
            Some("isSupported") => (reply(json!(self.manager.is_some())), false),
            _ => (Vec::new(), false),
        }
    }

    fn set_regions<D>(
        &mut self,
        regions: Vec<Region>,
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
        let object = self
            .object
            .get_or_insert_with(|| manager.get_glass(surface, qh, ()));
        object.clear();
        // An Ossido with version 1 of the protocol does not know the notification glass:
        // the closest one is the thick glass.
        let notification = object.version() >= 2;
        let translucent = object.version() >= 3;
        let groups = object.version() >= 4;
        let mut group = regions.first().map_or(0, |r| r.group);
        for region in &regions {
            if region.tip.is_some_and(|tip| object.version() < tip.since()) {
                // An earlier Ossido cannot draw this tip.
                continue;
            }
            if groups && region.group != group {
                object.separate();
                group = region.group;
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
        self.current = regions;
        true
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
        let summary: Vec<(u32, Option<Tip>)> = regions.iter().map(|r| (r.group, r.tip)).collect();
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
}
