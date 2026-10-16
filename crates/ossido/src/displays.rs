//! Displays: `output.list`, `output.configure` and `output.color_temperature`, and the choices
//! saved in `$XDG_CONFIG_HOME/silica/displays.json`, keyed by the display's identity (EDID make,
//! model and serial; the connector name when there is no EDID). They are applied again at start,
//! when the file changes and when a display is plugged in.
//!
//! The real session drives one output (the first connected connector, `drm.rs`); the others are
//! listed but not enabled. Nested and headless sessions have a single virtual output: its scale
//! and rotation can change, its mode cannot.

use std::collections::BTreeMap;

use serde_json::{Map, Value, json};
use smithay::output::{Mode, Scale};
use smithay::utils::Transform;
use tracing::{info, warn};

use crate::backend::Backend;
use crate::config::{read_json, text_of};
use crate::state::State;

pub const DISPLAYS_FILE: &str = "displays.json";

/// Neutral colour temperature: no night light.
pub const NEUTRAL_KELVIN: u32 = 6500;
pub const MIN_KELVIN: u32 = 1000;

/// A display mode as the apps see it.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ModeInfo {
    pub width: i32,
    pub height: i32,
    /// Hz, with decimals (59.94).
    pub refresh: f64,
    pub preferred: bool,
}

impl ModeInfo {
    pub fn json(&self) -> Value {
        json!({
            "width": self.width,
            "height": self.height,
            "refresh": (self.refresh * 100.0).round() / 100.0,
            "preferred": self.preferred,
        })
    }
}

/// The render scale of the scaled modes: clients and the shell draw at 2x.
pub const HIDPI_SCALE: i32 = 2;

/// A scaled mode, as macOS has them: the screen "looks like" `logical` (the size apps and the
/// shell lay out in), Ossido draws everything at 2x that into an offscreen framebuffer and
/// reduces it to the display's `native` mode (`ossido_render::scaler`). The output's own mode
/// is the 2x size and its scale 2, so everything that works in logical or physical pixels of
/// the output (layout, input, cursor, captures) follows without special cases; the backends
/// are the ones that know the native size.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct HiDpi {
    pub logical: (i32, i32),
    pub native: (i32, i32),
}

impl HiDpi {
    /// The size drawn: 2x the logical size.
    pub fn render_size(&self) -> (i32, i32) {
        (self.logical.0 * HIDPI_SCALE, self.logical.1 * HIDPI_SCALE)
    }

    /// Native pixels per logical pixel, per axis (what a capture at native size multiplies
    /// the logical coordinates by).
    pub fn native_per_logical(&self) -> (f64, f64) {
        (
            f64::from(self.native.0) / f64::from(self.logical.0.max(1)),
            f64::from(self.native.1) / f64::from(self.logical.1.max(1)),
        )
    }
}

/// The "looks like" sizes offered for a display of `native` pixels, native first (that one is
/// not scaled): 7/8, 4/5, 3/4 and 2/3 of it, and on a panel of 2560 or more across also 3/5
/// and 1/2 (the HiDPI size proper). On 1920x1200: 1920x1200, 1680x1050, 1536x960, 1440x900,
/// 1280x800.
pub fn looks_like(native: (i32, i32)) -> Vec<(i32, i32)> {
    let mut out = vec![native];
    let mut factors = vec![(7, 8), (4, 5), (3, 4), (2, 3)];
    if native.0 >= 2560 {
        factors.extend([(3, 5), (1, 2)]);
    }
    for (num, den) in factors {
        let size = (
            (f64::from(native.0) * f64::from(num) / f64::from(den)).round() as i32,
            (f64::from(native.1) * f64::from(num) / f64::from(den)).round() as i32,
        );
        if valid_logical(native, size) && !out.contains(&size) {
            out.push(size);
        }
    }
    out
}

/// Whether a display of `native` pixels can look like `logical`: not larger than itself
/// (below that the 2x image is reduced, never enlarged), at least half (the 2x image is not
/// smaller than the display), at least 640 across, and the same shape within 1%.
pub fn valid_logical(native: (i32, i32), logical: (i32, i32)) -> bool {
    let (nw, nh) = (i64::from(native.0), i64::from(native.1));
    let (lw, lh) = (i64::from(logical.0), i64::from(logical.1));
    lw >= 640
        && lh >= 400
        && lw <= nw
        && lh <= nh
        && lw * HIDPI_SCALE as i64 >= nw
        && lh * HIDPI_SCALE as i64 >= nh
        && (lw * nh - lh * nw).abs() * 100 <= lw * nh
}

/// A display, connected, driven or not.
#[derive(Debug, Clone, PartialEq)]
pub struct OutputInfo {
    pub connector: String,
    pub make: String,
    pub model: String,
    pub serial: String,
    pub modes: Vec<ModeInfo>,
    pub current: Option<ModeInfo>,
    /// The fractional scale chosen (1 in a scaled mode: the scale of the output is then the
    /// render scale of the mode, see [`HiDpi`]).
    pub scale: f64,
    pub transform: Transform,
    pub enabled: bool,
    pub primary: bool,
    /// Physical size in millimetres.
    pub size_mm: (i32, i32),
    /// The size the screen "looks like" to apps and the shell, in logical pixels.
    pub logical: (i32, i32),
    /// The display is in a scaled mode: drawn at 2x `logical` and reduced to `current`.
    pub hidpi: bool,
}

impl OutputInfo {
    /// The key in `displays.json`.
    pub fn identity(&self) -> String {
        identity(&self.make, &self.model, &self.serial, &self.connector)
    }

    pub fn json(&self) -> Value {
        json!({
            "connector": self.connector,
            "identity": self.identity(),
            "make": self.make,
            "model": self.model,
            "serial": self.serial,
            "modes": self.modes.iter().map(ModeInfo::json).collect::<Vec<_>>(),
            "current": self.current.as_ref().map(ModeInfo::json),
            "scale": self.scale,
            "transform": transform_name(self.transform),
            "enabled": self.enabled,
            "primary": self.primary,
            "widthMm": self.size_mm.0,
            "heightMm": self.size_mm.1,
            "logical": {"width": self.logical.0, "height": self.logical.1},
            "hidpi": self.hidpi,
            "looksLike": self.current.map(|m| looks_like((m.width, m.height)))
                .unwrap_or_default()
                .into_iter()
                .map(|(width, height)| json!({
                    "width": width,
                    "height": height,
                    "scaled": (width, height) != self.current.map_or((0, 0), |m| (m.width, m.height)),
                }))
                .collect::<Vec<_>>(),
        })
    }
}

/// `make model serial`, or the connector without EDID.
pub fn identity(make: &str, model: &str, serial: &str, connector: &str) -> String {
    let parts: Vec<&str> = [make, model, serial]
        .into_iter()
        .filter(|p| !p.is_empty())
        .collect();
    if parts.is_empty() {
        connector.to_owned()
    } else {
        parts.join(" ")
    }
}

pub fn transform_name(transform: Transform) -> &'static str {
    match transform {
        Transform::Normal => "normal",
        Transform::_90 => "90",
        Transform::_180 => "180",
        Transform::_270 => "270",
        Transform::Flipped => "flipped",
        Transform::Flipped90 => "flipped-90",
        Transform::Flipped180 => "flipped-180",
        Transform::Flipped270 => "flipped-270",
    }
}

pub fn parse_transform(name: &str) -> Option<Transform> {
    Some(match name {
        "normal" => Transform::Normal,
        "90" => Transform::_90,
        "180" => Transform::_180,
        "270" => Transform::_270,
        "flipped" => Transform::Flipped,
        "flipped-90" => Transform::Flipped90,
        "flipped-180" => Transform::Flipped180,
        "flipped-270" => Transform::Flipped270,
        _ => return None,
    })
}

/// The make, model and serial in an EDID block: the PNP id, the monitor name descriptor
/// (`0xFC`, else the product code) and the serial string (`0xFF`, else the serial number).
pub fn edid_identity(edid: &[u8]) -> Option<(String, String, String)> {
    const HEADER: [u8; 8] = [0x00, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0x00];
    if edid.get(..8)? != HEADER {
        return None;
    }
    let id = u16::from_be_bytes([*edid.get(8)?, *edid.get(9)?]);
    let letter = |shift: u16| char::from(b'@' + ((id >> shift) & 0x1f) as u8);
    let make: String = [letter(10), letter(5), letter(0)].iter().collect();
    let product = u16::from_le_bytes([*edid.get(10)?, *edid.get(11)?]);
    let number = u32::from_le_bytes([
        *edid.get(12)?,
        *edid.get(13)?,
        *edid.get(14)?,
        *edid.get(15)?,
    ]);
    let mut model = String::new();
    let mut serial = String::new();
    for start in [54usize, 72, 90, 108] {
        let Some(block) = edid.get(start..start + 18) else {
            continue;
        };
        if block.get(..3) != Some(&[0, 0, 0]) {
            continue;
        }
        let text = || {
            let bytes = block.get(5..18).unwrap_or_default();
            let end = bytes
                .iter()
                .position(|&b| b == b'\n')
                .unwrap_or(bytes.len());
            String::from_utf8_lossy(bytes.get(..end).unwrap_or_default())
                .trim()
                .chars()
                .filter(|c| !c.is_control())
                .collect::<String>()
        };
        match block.get(3) {
            Some(0xfc) => model = text(),
            Some(0xff) => serial = text(),
            _ => {}
        }
    }
    if model.is_empty() {
        model = format!("0x{product:04x}");
    }
    if serial.is_empty() && number != 0 {
        serial = number.to_string();
    }
    Some((make, model, serial))
}

/// One display's saved choice.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct OutputPrefs {
    /// Width, height and refresh in Hz (none: the closest).
    pub mode: Option<(i32, i32, Option<f64>)>,
    pub scale: Option<f64>,
    pub transform: Option<Transform>,
    /// A scaled mode: the size the screen looks like (see [`HiDpi`]); it replaces `scale`.
    pub logical: Option<(i32, i32)>,
}

impl OutputPrefs {
    pub fn parse(map: &Map<String, Value>) -> Self {
        let mode = map.get("mode").and_then(Value::as_object).and_then(|m| {
            let width = m
                .get("width")?
                .as_i64()
                .and_then(|w| i32::try_from(w).ok())?;
            let height = m
                .get("height")?
                .as_i64()
                .and_then(|h| i32::try_from(h).ok())?;
            let refresh = m
                .get("refresh")
                .and_then(Value::as_f64)
                .filter(|r| *r > 0.0);
            (width > 0 && height > 0).then_some((width, height, refresh))
        });
        let scale = map
            .get("scale")
            .and_then(Value::as_f64)
            .filter(|s| valid_scale(*s));
        let transform = text_of(map, "transform").and_then(|t| parse_transform(&t));
        let logical = map.get("logical").and_then(Value::as_object).and_then(|m| {
            let width = i32::try_from(m.get("width")?.as_i64()?).ok()?;
            let height = i32::try_from(m.get("height")?.as_i64()?).ok()?;
            (width > 0 && height > 0).then_some((width, height))
        });
        Self {
            mode,
            scale,
            transform,
            logical,
        }
    }

    pub fn json(&self) -> Value {
        let mut map = Map::new();
        if let Some((width, height, refresh)) = self.mode {
            map.insert(
                "mode".into(),
                json!({"width": width, "height": height, "refresh": refresh}),
            );
        }
        if let Some(scale) = self.scale {
            map.insert("scale".into(), json!(scale));
        }
        if let Some(transform) = self.transform {
            map.insert("transform".into(), json!(transform_name(transform)));
        }
        if let Some((width, height)) = self.logical {
            map.insert("logical".into(), json!({"width": width, "height": height}));
        }
        Value::Object(map)
    }
}

/// 1–4 in quarter steps.
pub fn valid_scale(scale: f64) -> bool {
    scale.is_finite() && (1.0..=4.0).contains(&scale) && (scale * 4.0).fract() == 0.0
}

/// The saved choices, by identity.
pub fn parse_displays(map: Option<&Map<String, Value>>) -> BTreeMap<String, OutputPrefs> {
    map.and_then(|m| m.get("outputs"))
        .and_then(Value::as_object)
        .map(|outputs| {
            outputs
                .iter()
                .filter_map(|(key, value)| {
                    Some((key.clone(), OutputPrefs::parse(value.as_object()?)))
                })
                .collect()
        })
        .unwrap_or_default()
}

/// The mode among `modes` that matches the wish: same size, the closest refresh (the highest
/// without one).
pub fn pick_mode(modes: &[ModeInfo], wish: (i32, i32, Option<f64>)) -> Option<usize> {
    let (width, height, refresh) = wish;
    modes
        .iter()
        .enumerate()
        .filter(|(_, m)| m.width == width && m.height == height)
        .min_by(|(_, a), (_, b)| {
            let distance = |m: &ModeInfo| match refresh {
                Some(r) => (m.refresh - r).abs(),
                None => -m.refresh,
            };
            distance(a).total_cmp(&distance(b))
        })
        .map(|(i, _)| i)
}

/// The red, green and blue factors of a colour temperature (an approximation of the blackbody
/// curve in sRGB, 1000–6500 K; 6500 is white).
pub fn kelvin_rgb(kelvin: u32) -> (f64, f64, f64) {
    if kelvin >= NEUTRAL_KELVIN {
        return (1.0, 1.0, 1.0);
    }
    let k = f64::from(kelvin.max(MIN_KELVIN)) / 100.0;
    let red = 1.0;
    let green = if k <= 66.0 {
        (99.470_802_586_1 * k.ln() - 161.119_568_166_1) / 255.0
    } else {
        1.0
    };
    let blue = if k >= 65.0 {
        1.0
    } else if k <= 19.0 {
        0.0
    } else {
        (138.517_731_223_1 * (k - 10.0).ln() - 305.044_792_730_7) / 255.0
    };
    (red, green.clamp(0.0, 1.0), blue.clamp(0.0, 1.0))
}

/// A gamma ramp of `size` entries for a colour temperature.
pub fn gamma_ramp(kelvin: u32, size: usize) -> (Vec<u16>, Vec<u16>, Vec<u16>) {
    let (r, g, b) = kelvin_rgb(kelvin);
    let ramp = |factor: f64| {
        (0..size)
            .map(|i| {
                let x = i as f64 / (size.max(2) - 1) as f64;
                (x * factor * 65535.0).round().clamp(0.0, 65535.0) as u16
            })
            .collect::<Vec<u16>>()
    };
    (ramp(r), ramp(g), ramp(b))
}

/// The parameters of `output.configure`.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Configure {
    pub connector: Option<String>,
    pub prefs: OutputPrefs,
    /// `"logical": null`: back to the plain mode, without a scaled one.
    pub clear_logical: bool,
}

impl Configure {
    pub fn parse(params: &Value) -> Result<Self, String> {
        let map = params
            .as_object()
            .ok_or("parameters: an object with connector, mode, scale, logical, transform")?;
        for key in map.keys() {
            if !["connector", "mode", "scale", "logical", "transform"].contains(&key.as_str()) {
                return Err(format!("parameters: unknown field {key}"));
            }
        }
        let prefs = OutputPrefs::parse(map);
        if map.contains_key("mode") && prefs.mode.is_none() {
            return Err("mode: {width, height, refresh?}".into());
        }
        if map.contains_key("scale") && prefs.scale.is_none() {
            return Err("scale: 1–4 in steps of 0.25".into());
        }
        let clear_logical = map.get("logical").is_some_and(Value::is_null);
        if map.contains_key("logical") && !clear_logical && prefs.logical.is_none() {
            return Err("logical: {width, height} (what the screen looks like), or null".into());
        }
        if prefs.logical.is_some() && prefs.scale.is_some() {
            return Err("logical and scale exclude each other".into());
        }
        if map.contains_key("transform") && prefs.transform.is_none() {
            return Err("transform: normal, 90, 180, 270, flipped, flipped-90…".into());
        }
        Ok(Self {
            connector: text_of(map, "connector"),
            prefs,
            clear_logical,
        })
    }
}

impl State {
    /// Every display known to the backend.
    pub fn outputs(&mut self) -> Vec<OutputInfo> {
        #[cfg(feature = "drm")]
        if let Some(list) = self.drm_outputs() {
            return list;
        }
        let mode = self.output.current_mode();
        let native = self.native_size();
        let current = mode.map(|m| ModeInfo {
            width: native.0,
            height: native.1,
            refresh: f64::from(m.refresh) / 1000.0,
            preferred: true,
        });
        let physical = self.output.physical_properties();
        vec![OutputInfo {
            connector: self.output.name(),
            make: String::new(),
            model: String::new(),
            serial: String::new(),
            modes: current.into_iter().collect(),
            current,
            scale: self.user_scale(),
            transform: self.output.current_transform(),
            enabled: true,
            primary: true,
            size_mm: (physical.size.w, physical.size.h),
            logical: self.logical_size(),
            hidpi: self.hidpi.is_some(),
        }]
    }

    pub fn output_list(&mut self) -> Value {
        Value::Array(self.outputs().iter().map(OutputInfo::json).collect())
    }

    /// `output.configure`: applies at once and saves it for that display.
    pub fn output_configure(&mut self, params: &Value) -> Result<Value, String> {
        let configure = Configure::parse(params)?;
        let outputs = self.outputs();
        let target = match &configure.connector {
            Some(name) => outputs.iter().find(|o| &o.connector == name),
            None => outputs.iter().find(|o| o.enabled),
        }
        .ok_or("no such display")?
        .clone();
        if !target.enabled {
            return Err("only the display in use can be configured for now".into());
        }
        if let Some(wish) = configure.prefs.mode
            && pick_mode(&target.modes, wish).is_none()
        {
            return Err(format!(
                "{}: no {}x{} mode",
                target.connector, wish.0, wish.1
            ));
        }
        // The mode the display will have: a scaled mode has to fit it.
        let native = configure
            .prefs
            .mode
            .and_then(|wish| pick_mode(&target.modes, wish))
            .and_then(|index| target.modes.get(index))
            .or(target.current.as_ref())
            .map(|m| (m.width, m.height))
            .unwrap_or_default();
        if let Some(logical) = configure.prefs.logical
            && logical != native
            && !valid_logical(native, logical)
        {
            return Err(format!(
                "{}: {}x{} cannot look like {}x{} (between half and all of it, same shape)",
                target.connector, native.0, native.1, logical.0, logical.1
            ));
        }
        // Merge with what is saved for the display, then apply the whole choice.
        let identity = target.identity();
        let mut saved = self.saved_displays();
        let prefs = saved.entry(identity.clone()).or_default();
        if configure.prefs.mode.is_some() {
            prefs.mode = configure.prefs.mode;
        }
        if configure.clear_logical {
            prefs.logical = None;
        }
        if let Some(logical) = configure.prefs.logical {
            // Looking like the native size is the plain mode.
            prefs.logical = (logical != native).then_some(logical);
            prefs.scale = None;
        }
        if configure.prefs.scale.is_some() {
            prefs.scale = configure.prefs.scale;
            prefs.logical = None;
        }
        if configure.prefs.transform.is_some() {
            prefs.transform = configure.prefs.transform;
        }
        let prefs = prefs.clone();
        self.apply_output_prefs(&target, &prefs)?;
        self.save_displays(&saved);
        self.report_outputs();
        Ok(self
            .outputs()
            .into_iter()
            .find(|o| o.connector == target.connector)
            .map(|o| o.json())
            .unwrap_or(Value::Null))
    }

    fn saved_displays(&self) -> BTreeMap<String, OutputPrefs> {
        match &self.config_dir {
            Some(dir) => parse_displays(read_json(dir, DISPLAYS_FILE).as_ref()),
            None => BTreeMap::new(),
        }
    }

    /// Writes `displays.json` keeping the keys other components put there.
    fn save_displays(&self, outputs: &BTreeMap<String, OutputPrefs>) {
        let Some(dir) = &self.config_dir else {
            return;
        };
        let mut map = read_json(dir, DISPLAYS_FILE).unwrap_or_default();
        map.insert(
            "outputs".into(),
            Value::Object(
                outputs
                    .iter()
                    .map(|(key, prefs)| (key.clone(), prefs.json()))
                    .collect(),
            ),
        );
        let text = serde_json::to_string_pretty(&Value::Object(map)).unwrap_or_default();
        let temporary = dir.join(format!("{DISPLAYS_FILE}.tmp"));
        let written = std::fs::write(&temporary, text)
            .and_then(|()| std::fs::rename(&temporary, dir.join(DISPLAYS_FILE)));
        if let Err(err) = written {
            warn!("{DISPLAYS_FILE}: {err}");
        }
    }

    /// Mode, scale and rotation of a display.
    fn apply_output_prefs(
        &mut self,
        target: &OutputInfo,
        prefs: &OutputPrefs,
    ) -> Result<(), String> {
        let mut native = target
            .current
            .as_ref()
            .map(|m| (m.width, m.height))
            .unwrap_or_default();
        if let Some(wish) = prefs.mode
            && let Some(index) = pick_mode(&target.modes, wish)
        {
            if let Some(mode) = target.modes.get(index) {
                native = (mode.width, mode.height);
            }
            if target.current.as_ref() != target.modes.get(index) {
                #[cfg(feature = "drm")]
                self.drm_set_mode(index)?;
                #[cfg(not(feature = "drm"))]
                let _ = index;
            }
        }
        // A scaled mode (it replaces the scale) if it still fits the display's mode.
        let logical = prefs
            .logical
            .filter(|l| valid_logical(native, *l) && *l != native);
        if let Some(wanted) = prefs.logical
            && logical.is_none()
            && wanted != native
        {
            warn!(
                "{DISPLAYS_FILE}: {}: {}x{} cannot look like {}x{}",
                target.connector, native.0, native.1, wanted.0, wanted.1
            );
        }
        if logical != self.hidpi.map(|h| h.logical) {
            self.set_hidpi(logical)?;
        }
        let scale = prefs.scale.filter(|_| logical.is_none()).map(|s| {
            if s.fract() == 0.0 {
                Scale::Integer(s as i32)
            } else {
                Scale::Fractional(s)
            }
        });
        if scale.is_some() || prefs.transform.is_some() {
            self.output
                .change_current_state(None, prefs.transform, scale, None);
            #[cfg(feature = "drm")]
            self.drm_output_changed();
            self.output_resized();
            info!(
                "display {}: scale {:?}, {:?}",
                target.connector, prefs.scale, prefs.transform
            );
        }
        Ok(())
    }

    /// The display's mode in pixels: what is scanned out, which is not the size of the output
    /// in a scaled mode (the output is then the 2x size that is drawn).
    pub fn native_size(&self) -> (i32, i32) {
        match self.hidpi {
            Some(hidpi) => hidpi.native,
            None => self
                .output
                .current_mode()
                .map_or((0, 0), |m| (m.size.w, m.size.h)),
        }
    }

    /// The scale the user chose: 1 in a scaled mode.
    pub fn user_scale(&self) -> f64 {
        if self.hidpi.is_some() {
            1.0
        } else {
            self.output.current_scale().fractional_scale()
        }
    }

    /// Pixels of a screen capture per logical pixel: the scale of the output, or in a scaled
    /// mode the display's pixels over the logical ones (the capture is at native size).
    pub fn capture_scale(&self) -> f64 {
        match self.hidpi {
            Some(hidpi) => hidpi.native_per_logical().0,
            None => self.output.current_scale().fractional_scale(),
        }
    }

    /// What the display looks like, in logical pixels.
    pub fn logical_size(&self) -> (i32, i32) {
        self.space
            .output_geometry(&self.output)
            .map_or((0, 0), |g| (g.size.w, g.size.h))
    }

    /// Enters (`Some(looks like)`) or leaves the scaled mode: the output takes the 2x size and
    /// scale 2 (or its native mode and scale 1 again), the backend learns of it, and the layers
    /// are arranged again.
    pub fn set_hidpi(&mut self, logical: Option<(i32, i32)>) -> Result<(), String> {
        let native = self.native_size();
        let refresh = self.output.current_mode().map_or(60_000, |m| m.refresh);
        match logical {
            Some(logical) => {
                if !valid_logical(native, logical) {
                    return Err(format!(
                        "{}x{} cannot look like {}x{}",
                        native.0, native.1, logical.0, logical.1
                    ));
                }
                if matches!(self.backend, Backend::Winit(_)) {
                    return Err("scaled modes need the real session or headless".into());
                }
                let hidpi = HiDpi { logical, native };
                let size = hidpi.render_size();
                self.hidpi = Some(hidpi);
                let mode = Mode {
                    size: size.into(),
                    refresh,
                };
                self.output.change_current_state(
                    Some(mode),
                    None,
                    Some(Scale::Integer(HIDPI_SCALE)),
                    None,
                );
                self.output.set_preferred(mode);
                info!(
                    "scaled mode: looks like {}x{}, drawn at {}x{}, shown at {}x{}",
                    logical.0, logical.1, size.0, size.1, native.0, native.1
                );
            }
            None => {
                if self.hidpi.take().is_none() {
                    return Ok(());
                }
                let mode = Mode {
                    size: native.into(),
                    refresh,
                };
                self.output
                    .change_current_state(Some(mode), None, Some(Scale::Integer(1)), None);
                self.output.set_preferred(mode);
            }
        }
        #[cfg(feature = "drm")]
        self.drm_output_changed();
        self.output_resized();
        self.wake();
        Ok(())
    }

    /// `displays.json` changed, Ossido started or a display was plugged in: the saved choice of
    /// the display in use.
    pub fn apply_displays_config(&mut self) {
        let saved = self.saved_displays();
        let outputs = self.outputs();
        for output in outputs.iter().filter(|o| o.enabled) {
            if let Some(prefs) = saved.get(&output.identity())
                && let Err(err) = self.apply_output_prefs(output, prefs)
            {
                warn!("{DISPLAYS_FILE}: {}: {err}", output.connector);
            }
        }
        self.report_outputs();
    }

    /// Tells silica-daemon the displays (`displays` in `System1`, for Settings).
    pub fn report_outputs(&mut self) {
        let list = self.output_list().to_string();
        self.system.report("displays", Value::String(list));
    }

    /// `output.color_temperature`: the night light (1000–6500 K; 6500 is neutral) on every
    /// display, through the CRTC gamma ramp. False where the backend has no gamma.
    pub fn set_color_temperature(&mut self, kelvin: u32) -> bool {
        self.color_temperature = kelvin.clamp(MIN_KELVIN, NEUTRAL_KELVIN);
        #[cfg(feature = "drm")]
        if let Some(applied) = self.drm_apply_gamma() {
            return applied;
        }
        false
    }
}

/// Used by the backends: a `ModeInfo` from Smithay's mode.
pub fn mode_info(mode: Mode, preferred: bool) -> ModeInfo {
    ModeInfo {
        width: mode.size.w,
        height: mode.size.h,
        refresh: f64::from(mode.refresh) / 1000.0,
        preferred,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// An EDID with PNP id "DEL", product 0x4321, serial number 7, a name and a serial string.
    fn edid() -> Vec<u8> {
        let mut e = vec![0u8; 128];
        e[..8].copy_from_slice(&[0, 255, 255, 255, 255, 255, 255, 0]);
        // D=4, E=5, L=12: 00100 00101 01100.
        let id: u16 = (4 << 10) | (5 << 5) | 12;
        e[8..10].copy_from_slice(&id.to_be_bytes());
        e[10..12].copy_from_slice(&0x4321u16.to_le_bytes());
        e[12..16].copy_from_slice(&7u32.to_le_bytes());
        let descriptor = |e: &mut Vec<u8>, at: usize, tag: u8, text: &str| {
            e[at..at + 5].copy_from_slice(&[0, 0, 0, tag, 0]);
            let mut bytes = text.as_bytes().to_vec();
            bytes.push(b'\n');
            bytes.resize(13, b' ');
            e[at + 5..at + 18].copy_from_slice(&bytes);
        };
        descriptor(&mut e, 72, 0xfc, "DELL U2720Q");
        descriptor(&mut e, 90, 0xff, "ABC123");
        e
    }

    #[test]
    fn edid_names_the_display() {
        assert_eq!(
            edid_identity(&edid()),
            Some(("DEL".into(), "DELL U2720Q".into(), "ABC123".into()))
        );
        let mut bare = edid();
        bare[72..108].fill(1);
        assert_eq!(
            edid_identity(&bare),
            Some(("DEL".into(), "0x4321".into(), "7".into()))
        );
        assert_eq!(edid_identity(&[0; 20]), None);
        assert_eq!(edid_identity(&edid()[..9]), None);
        assert_eq!(identity("", "", "", "Virtual-1"), "Virtual-1");
        assert_eq!(identity("DEL", "U", "", "DP-1"), "DEL U");
    }

    #[test]
    fn modes_match_size_then_refresh() {
        let mode = |width, height, refresh| ModeInfo {
            width,
            height,
            refresh,
            preferred: false,
        };
        let modes = [
            mode(1920, 1080, 60.0),
            mode(1920, 1080, 144.0),
            mode(1920, 1080, 59.94),
            mode(1280, 720, 60.0),
        ];
        assert_eq!(pick_mode(&modes, (1920, 1080, Some(59.9))), Some(2));
        assert_eq!(pick_mode(&modes, (1920, 1080, None)), Some(1));
        assert_eq!(pick_mode(&modes, (800, 600, None)), None);
    }

    #[test]
    fn configure_parameters_are_checked() {
        let c = Configure::parse(
            &json!({"connector": "DP-1", "scale": 1.5, "transform": "90",
            "mode": {"width": 1920, "height": 1080, "refresh": 60}}),
        )
        .unwrap();
        assert_eq!(c.connector.as_deref(), Some("DP-1"));
        assert_eq!(c.prefs.scale, Some(1.5));
        assert_eq!(c.prefs.transform, Some(Transform::_90));
        assert_eq!(c.prefs.mode, Some((1920, 1080, Some(60.0))));
        for bad in [
            json!({"scale": 1.3}),
            json!({"scale": 9}),
            json!({"transform": "sideways"}),
            json!({"mode": {"width": 0, "height": 1}}),
            json!({"position": 1}),
            json!([1]),
        ] {
            assert!(Configure::parse(&bad).is_err(), "{bad}");
        }
        // Saved choices survive a round trip.
        let saved = parse_displays(Some(
            json!({"outputs": {"DEL U": c.prefs.json(), "bad": 3}})
                .as_object()
                .unwrap(),
        ));
        assert_eq!(saved.len(), 1);
        assert_eq!(saved["DEL U"], c.prefs);
    }

    #[test]
    fn scaled_modes_look_like_fractions_of_the_display() {
        assert_eq!(
            looks_like((1920, 1200)),
            [
                (1920, 1200),
                (1680, 1050),
                (1536, 960),
                (1440, 900),
                (1280, 800)
            ]
        );
        // A 4K panel also offers its 2x size, and sizes that are not whole are not offered.
        assert_eq!(
            looks_like((3840, 2160)),
            [
                (3840, 2160),
                (3360, 1890),
                (3072, 1728),
                (2880, 1620),
                (2560, 1440),
                (2304, 1296),
                (1920, 1080)
            ]
        );
        assert!(looks_like((1366, 768)).contains(&(1366, 768)));
        for native in [(1920, 1200), (1366, 768), (2560, 1600), (800, 600)] {
            assert!(looks_like(native).iter().all(|l| valid_logical(native, *l)));
        }
        // Not larger, not smaller than half, not another shape, not tiny.
        assert!(valid_logical((1920, 1200), (1536, 960)));
        assert!(!valid_logical((1920, 1200), (2560, 1600)));
        assert!(!valid_logical((1920, 1200), (900, 562)));
        assert!(!valid_logical((1920, 1200), (1536, 1000)));
        assert!(!valid_logical((1024, 768), (512, 384)));
    }

    #[test]
    fn a_scaled_mode_is_drawn_at_twice_its_size() {
        let hidpi = HiDpi {
            logical: (1536, 960),
            native: (1920, 1200),
        };
        assert_eq!(hidpi.render_size(), (3072, 1920));
        assert_eq!(hidpi.native_per_logical(), (1.25, 1.25));
    }

    #[test]
    fn logical_sizes_are_configured_saved_and_exclusive_with_scale() {
        let c = Configure::parse(&json!({"logical": {"width": 1536, "height": 960}})).unwrap();
        assert_eq!(c.prefs.logical, Some((1536, 960)));
        assert!(!c.clear_logical);
        let cleared = Configure::parse(&json!({"logical": null})).unwrap();
        assert!(cleared.clear_logical && cleared.prefs.logical.is_none());
        for bad in [
            json!({"logical": {"width": 0, "height": 5}}),
            json!({"logical": {"width": 1536}}),
            json!({"logical": 3}),
            json!({"logical": {"width": 1536, "height": 960}, "scale": 2}),
        ] {
            assert!(Configure::parse(&bad).is_err(), "{bad}");
        }
        // The saved choice round-trips with its logical size.
        let prefs = OutputPrefs {
            logical: Some((1536, 960)),
            ..OutputPrefs::default()
        };
        let saved = parse_displays(Some(
            json!({"outputs": {"DEL U": prefs.json()}})
                .as_object()
                .unwrap(),
        ));
        assert_eq!(saved["DEL U"], prefs);
    }

    #[test]
    fn night_light_warms_the_ramp() {
        assert_eq!(kelvin_rgb(6500), (1.0, 1.0, 1.0));
        let (r, g, b) = kelvin_rgb(3400);
        assert_eq!(r, 1.0);
        assert!(g < 1.0 && b < g, "{g} {b}");
        assert_eq!(kelvin_rgb(10), kelvin_rgb(1000));
        let (red, green, blue) = gamma_ramp(3400, 256);
        assert_eq!((red[0], red[255]), (0, 65535));
        assert!(green[255] < 65535 && blue[255] < green[255]);
        assert_eq!(gamma_ramp(6500, 4).2, [0, 21845, 43690, 65535]);
    }
}
