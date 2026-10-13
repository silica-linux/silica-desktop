//! Il canale `silica/glass`: le regioni di vetro dichiarate dai widget `Glass` di
//! `silica_ui`, passate a Ossido con `silica-glass-v1` (`docs/protocols.md`).
//!
//! Il messaggio arriva dal codice Dart dell'app: si controlla qui (dimensioni positive,
//! raggio finito, al più 64 regioni) perché un valore sbagliato non diventi un errore di
//! protocollo che disconnetterebbe l'app.

use serde_json::{Value, json};
use silica_protocols::glass::MAX_REGIONS;
use silica_protocols::glass::client::silica_glass_manager_v1::SilicaGlassManagerV1;
use silica_protocols::glass::client::silica_glass_v1::{Material, SilicaGlassV1};
use wayland_client::protocol::wl_surface::WlSurface;
use wayland_client::{Dispatch, Proxy, QueueHandle};

pub const CHANNEL: &str = "silica/glass";

/// Una regione in pixel logici della superficie.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Region {
    pub x: i32,
    pub y: i32,
    pub width: i32,
    pub height: i32,
    pub radius: f64,
    pub material: Material,
}

impl Region {
    /// Una regione dal JSON di `setRegions`, se valida.
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
        let region = Self {
            x: int("x")?,
            y: int("y")?,
            width: int("width")?,
            height: int("height")?,
            radius,
            material,
        };
        (region.width > 0 && region.height > 0).then_some(region)
    }
}

/// Le regioni valide di un `setRegions`, al più `MAX_REGIONS`.
pub fn parse_regions(args: &Value) -> Vec<Region> {
    args.as_array()
        .map(|list| {
            list.iter()
                .filter_map(Region::from_json)
                .take(MAX_REGIONS)
                .collect()
        })
        .unwrap_or_default()
}

/// Il vetro della superficie dell'app.
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

    /// Gestisce una chiamata del canale. Restituisce la risposta (JSON method codec) e se
    /// le regioni sono cambiate: in quel caso serve un commit della superficie.
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
            // Ossido offre il vetro? Senza, l'app disegna un fondo opaco.
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
        // Un Ossido con la versione 1 del protocollo non conosce il vetro delle notifiche:
        // il più vicino è quello spesso.
        let notification = object.version() >= 2;
        for region in &regions {
            let material = match region.material {
                Material::Notification if !notification => Material::Thick,
                material => material,
            };
            object.add_region(
                region.x,
                region.y,
                region.width,
                region.height,
                region.radius,
                material,
            );
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
            "non è una regione"
        ]);
        let regions = parse_regions(&args);
        assert_eq!(regions.len(), 3);
        assert_eq!((regions[0].y, regions[0].radius), (20, 22.0));
        assert_eq!(regions[1].material, Material::Thick);
        // Raggio negativo: si usa 0, la regione resta.
        assert_eq!(regions[2].radius, 0.0);
        let many = Value::Array(vec![json!({"x": 0, "y": 0, "width": 1, "height": 1}); 100]);
        assert_eq!(parse_regions(&many).len(), MAX_REGIONS);
        assert!(parse_regions(&json!(null)).is_empty());
    }
}
