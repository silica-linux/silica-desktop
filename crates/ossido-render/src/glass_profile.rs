//! Glass profiles: the settings of the glass per kind of surface (silica-glass-v1 version 7).
//!
//! The material (`glass::MaterialParams`) is the measured color transfer and edge of a glass
//! family; the profile says how a kind of surface uses it: how much it blurs, how strong the
//! lens at the edge is and how far it reaches, how bright the edge light and how dark the edge
//! line, a tint and saturation over the material, and the shadow the compositor may draw
//! under it. The neutral values (strength 1, width 1, no tint) are the material as measured.
//!
//! Each kind also owns its material ([`GlassKind::material`]): the tone (what black, the
//! reference gray and white become), the edge and the lens. The numbers come from the
//! GLASS-PROFILES measurement (silica-reference #97); where a value is not measured it is tuned
//! by eye against the reference captures (`plan/reference/glass.md` and the notes it cites)
//! and marked *provisional*.

use crate::glass::{
    CLEAR_DARK, CLEAR_LIGHT, CONTROL_DARK, CONTROL_LIGHT, MaterialParams, NOTIFICATION_DARK,
    NOTIFICATION_LIGHT, POPOVER_DARK, POPOVER_LIGHT, REGULAR_DARK, REGULAR_LIGHT, SHEET_DARK,
    SHEET_LIGHT, THICK_DARK, THICK_LIGHT, TOOLTIP_DARK, TOOLTIP_LIGHT,
};

/// The kinds of surface (`silica_glass_v1.profile`, without `default`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum GlassKind {
    /// Menus and context menus.
    Menu,
    /// Popovers (a bubble with a tip).
    Popover,
    /// Large panels: Notification Center, search, the launcher, an expanded module.
    Panel,
    /// Control Center modules and HUDs.
    Module,
    /// Small controls on glass: toolbar buttons, pills, round buttons.
    Control,
    /// The Dock and the app switcher.
    Dock,
    /// Tooltips and labels.
    Tooltip,
    /// Sheets and alerts.
    Sheet,
    /// Notification banners.
    Banner,
}

/// The shadow under a glass, outside its shape (drawn by Ossido only when the client asks).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct GlassShadow {
    /// Black alpha right under the edge, before the falloff.
    pub opacity: f32,
    /// σ of the falloff, in points.
    pub sigma: f32,
    /// How far the shadow sits below the shape, in points.
    pub offset_y: f32,
}

impl GlassShadow {
    pub const NONE: Self = Self {
        opacity: 0.0,
        sigma: 0.0,
        offset_y: 0.0,
    };

    /// How far it reaches beyond the shape, in points (3σ plus the offset).
    pub fn reach(&self) -> f32 {
        if self.opacity <= 0.0 {
            return 0.0;
        }
        3.0 * self.sigma.max(0.0) + self.offset_y.abs()
    }
}

/// The settings of a kind of surface.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct GlassProfile {
    /// σ of the blur in points; `None`: the material's measured σ.
    pub sigma: Option<f64>,
    /// Strength of the edge lens: 1 the measured displacement, 0 none.
    pub refraction: f32,
    /// Width of the lens band: 1 the measured band, 0.5 half as far from the edge (the curve
    /// keeps its shape). On a thin shape the band also narrows so that it never reaches past
    /// the far edge ([`lens_width`]).
    pub falloff: f32,
    /// Edge light (the light rows and the inner glow): 1 as measured.
    pub rim: f32,
    /// Edge dark (the dark line outside and the darker inner bands): 1 as measured.
    pub line: f32,
    /// Saturation over the material's (around the Rec. 709 luminance): 1 unchanged.
    pub saturation: f32,
    /// A color mixed over the fill (sRGB 0–1, amount in the fourth slot): 0 none.
    pub tint: [f32; 4],
    /// The shadow under the glass.
    pub shadow: GlassShadow,
}

impl GlassProfile {
    /// The material as measured, without a shadow.
    pub const NEUTRAL: Self = Self {
        sigma: None,
        refraction: 1.0,
        falloff: 1.0,
        rim: 1.0,
        line: 1.0,
        saturation: 1.0,
        tint: [0.0; 4],
        shadow: GlassShadow::NONE,
    };

    /// Without the shadow (the client draws its own).
    pub fn without_shadow(self) -> Self {
        Self {
            shadow: GlassShadow::NONE,
            ..self
        }
    }
}

/// The menus' shadow: blur 16 pt (σ ≈ 9.7), 4 pt lower, black 20 % in light and 26 % in
/// dark: the values `silica_ui`'s menu panel draws, measured on the menus (menubar.md).
const MENU_SHADOW_LIGHT: GlassShadow = GlassShadow {
    opacity: 0.20,
    sigma: 9.7,
    offset_y: 4.0,
};
const MENU_SHADOW_DARK: GlassShadow = GlassShadow {
    opacity: 0.26,
    ..MENU_SHADOW_LIGHT
};

impl GlassKind {
    pub const ALL: [Self; 9] = [
        Self::Menu,
        Self::Popover,
        Self::Panel,
        Self::Module,
        Self::Control,
        Self::Dock,
        Self::Tooltip,
        Self::Sheet,
        Self::Banner,
    ];

    /// The name used by the protocol, the `silica/glass` channel and `glass.list`.
    pub fn name(self) -> &'static str {
        match self {
            Self::Menu => "menu",
            Self::Popover => "popover",
            Self::Panel => "panel",
            Self::Module => "module",
            Self::Control => "control",
            Self::Dock => "dock",
            Self::Tooltip => "tooltip",
            Self::Sheet => "sheet",
            Self::Banner => "banner",
        }
    }

    /// The kind with this name.
    pub fn from_name(name: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|kind| kind.name() == name)
    }

    /// The material of this kind (tone, edge, lens) in the light or dark appearance; `None`
    /// for the panels, which take the material the client declares (Spotlight and the
    /// launcher are thick glass, an expanded module is regular).
    pub fn material(self, dark: bool) -> Option<MaterialParams> {
        let pick =
            |light: MaterialParams, darker: MaterialParams| if dark { darker } else { light };
        match self {
            Self::Dock => Some(pick(CLEAR_LIGHT, CLEAR_DARK)),
            Self::Module => Some(pick(REGULAR_LIGHT, REGULAR_DARK)),
            Self::Control => Some(pick(CONTROL_LIGHT, CONTROL_DARK)),
            Self::Menu => Some(pick(THICK_LIGHT, THICK_DARK)),
            Self::Popover => Some(pick(POPOVER_LIGHT, POPOVER_DARK)),
            Self::Tooltip => Some(pick(TOOLTIP_LIGHT, TOOLTIP_DARK)),
            Self::Sheet => Some(pick(SHEET_LIGHT, SHEET_DARK)),
            Self::Banner => Some(pick(NOTIFICATION_LIGHT, NOTIFICATION_DARK)),
            Self::Panel => None,
        }
    }

    /// The settings of this kind in the light or dark appearance.
    pub fn profile(self, dark: bool) -> GlassProfile {
        let neutral = GlassProfile::NEUTRAL;
        let menu_shadow = if dark {
            MENU_SHADOW_DARK
        } else {
            MENU_SHADOW_LIGHT
        };
        match self {
            // Measured (GLASS-PROFILES): blur σ 2.8 pt; the edge light is +64 (light) / +41
            // (dark) over the grey, against the material's +61 / +38; the lens is the
            // material's (2-3 pt, gone within 30 pt). No shadow under the Dock in the
            // captures (dock.md).
            Self::Dock => GlassProfile {
                sigma: Some(2.8),
                rim: if dark { 41.0 / 38.0 } else { 64.0 / 61.0 },
                ..neutral
            },
            // Measured: blur σ 8 pt in dark, 13 pt in light; thick glass, no lens. The shadow
            // is the one the menus draw.
            Self::Menu => GlassProfile {
                sigma: Some(if dark { 8.0 } else { 13.0 }),
                refraction: 0.0,
                shadow: menu_shadow,
                ..neutral
            },
            // Measured: blur σ 18 pt in dark, 13 pt in light; no lens; the menus' shadow.
            Self::Popover => GlassProfile {
                sigma: Some(if dark { 18.0 } else { 13.0 }),
                refraction: 0.0,
                shadow: menu_shadow,
                ..neutral
            },
            // Measured: blur σ 4 pt; the edge light is +96 over the grey, against the
            // material's +94; the lens reaches ~3.5 pt around 15 pt from the edge.
            Self::Module => GlassProfile {
                sigma: Some(4.0),
                rim: 96.0 / 94.0,
                ..neutral
            },
            // Measured: the notification material (no lens). The shadow is the banners' own:
            // σ 6 pt, 4 pt lower, black 6 % (notifications.md).
            Self::Banner => GlassProfile {
                refraction: 0.0,
                shadow: GlassShadow {
                    opacity: 0.063,
                    sigma: 6.0,
                    offset_y: 4.0,
                },
                ..neutral
            },
            // Provisional: a large panel shows a gentler lens over a wider band (a module's fold
            // over 24 pt reads as a dent on a 600 pt panel); search and launcher blur 8–14 pt
            // (spotlight.md, launcher.md), within the thick material's 9 ± 1 at the middle. The
            // shadow is the search panel's: blur 42 pt (σ ≈ 24.7), 20 pt lower, black 25 %.
            Self::Panel => GlassProfile {
                sigma: Some(10.0),
                refraction: 0.6,
                falloff: 1.5,
                shadow: GlassShadow {
                    opacity: 0.25,
                    sigma: 24.7,
                    offset_y: 20.0,
                },
                ..neutral
            },
            // Measured: blur σ ≈ 6 pt (the tone is the material's). Provisional: the edge light
            // and the lens; the modules' lens reaches 30 pt inside, on a 24–36 pt control that
            // reads as a distortion of everything around it, so half the strength over 0.6 of the
            // band, a slightly softer edge light, no shadow.
            Self::Control => GlassProfile {
                sigma: Some(6.0),
                refraction: 0.5,
                falloff: 0.6,
                rim: 0.85,
                ..neutral
            },
            // Tone measured (nearly opaque), no lens (thick glass). Provisional: the edge light
            // is fainter than a menu's. The shadow is the Dock label's (the menus').
            Self::Tooltip => GlassProfile {
                refraction: 0.0,
                rim: 0.7,
                shadow: menu_shadow,
                ..neutral
            },
            // Measured: opaque (the material is a flat fill), so the blur does not show; no
            // lens. The shadow is the alerts': blur 40 pt (σ ≈ 23.6), 10 pt lower, black 30 %.
            // The parent window's dimming (~40 %) is not part of the glass: the compositor draws
            // it behind the sheet.
            Self::Sheet => GlassProfile {
                sigma: Some(13.0),
                refraction: 0.0,
                shadow: GlassShadow {
                    opacity: 0.30,
                    sigma: 23.6,
                    offset_y: 10.0,
                },
                ..neutral
            },
        }
    }
}

/// How much the compositor dims the window a sheet is attached to (GLASS-PROFILES: ~40 %).
/// Ossido does not draw it yet: nothing in the render path darkens a parent window behind a
/// surface.
pub const SHEET_PARENT_DIM: f32 = 0.4;

/// The width of the lens band on a shape whose smaller side is `thickness` (pixels), for a
/// lens that samples at most `reach` pixels inside the edge and a profile `falloff`: the
/// profile's width, narrowed so the lens never samples past the shape's far edge. The
/// modules' fold samples the background up to 50 pt inside: on the 64 pt modules it fits as
/// measured, on a 24 pt pill it read the background beyond the pill (the "far too strong"
/// refraction of #168) and now gets 0.48 of the band.
pub fn lens_width(thickness: f32, reach: f32, falloff: f32) -> f32 {
    let falloff = falloff.max(0.0);
    if reach <= 0.0 || thickness <= 0.0 {
        return falloff;
    }
    falloff.min(thickness / reach).max(0.05)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::glass::REFERENCE_GRAY;

    #[test]
    fn every_kind_has_sane_settings_in_both_appearances() {
        for kind in GlassKind::ALL {
            assert_eq!(GlassKind::from_name(kind.name()), Some(kind));
            for dark in [false, true] {
                let p = kind.profile(dark);
                let label = format!("{kind:?} dark {dark}");
                if let Some(sigma) = p.sigma {
                    assert!((1.0..=20.0).contains(&sigma), "{label}: σ {sigma}");
                }
                assert!((0.0..=1.0).contains(&p.refraction), "{label}");
                assert!((0.1..=2.0).contains(&p.falloff), "{label}");
                assert!((0.0..=1.5).contains(&p.rim), "{label}");
                assert!((0.0..=1.5).contains(&p.line), "{label}");
                assert!((0.5..=1.5).contains(&p.saturation), "{label}");
                assert!((0.0..=1.0).contains(&p.tint[3]), "{label}");
                assert!((0.0..=0.5).contains(&p.shadow.opacity), "{label}");
                assert!(p.shadow.reach() <= 120.0, "{label}");
            }
        }
        assert_eq!(GlassKind::from_name("default"), None);
    }

    /// The output level of a gray input: the mean of the channels after the material and the
    /// profile's saturation and tint, in 0-255.
    fn level(kind: GlassKind, dark: bool, input: [f32; 3]) -> f32 {
        let material = kind.material(dark).expect("a material of its own");
        let [r, g, b] = crate::glass::profile_color(&kind.profile(dark), material.transfer(input));
        (r + g + b) / 3.0 * 255.0
    }

    /// The three points of GLASS-PROFILES: black, the reference gray (`#7a7b80`), white.
    fn tone(kind: GlassKind, dark: bool) -> [f32; 3] {
        [
            level(kind, dark, [0.0; 3]),
            level(kind, dark, REFERENCE_GRAY),
            level(kind, dark, [1.0; 3]),
        ]
    }

    #[test]
    fn each_profile_hits_its_three_measured_points() {
        use GlassKind::*;
        // (kind, dark, black, mid-grey, white); the module is the same in both appearances.
        let measured = [
            (Dock, true, [12.0, 105.0, 176.0]),
            (Dock, false, [38.0, 130.0, 229.0]),
            (Module, true, [32.0, 107.0, 186.0]),
            (Module, false, [32.0, 107.0, 186.0]),
            (Control, true, [34.0, 130.0, 185.0]),
            (Control, false, [133.0, 193.0, 252.0]),
            (Menu, true, [32.0, 114.0, 121.0]),
            (Menu, false, [136.0, 201.0, 254.0]),
            (Popover, true, [25.0, 112.0, 205.0]),
            (Popover, false, [136.0, 201.0, 254.0]),
            (Tooltip, true, [40.0, 60.0, 85.0]),
            (Tooltip, false, [204.0, 223.0, 229.0]),
        ];
        for (kind, dark, want) in measured {
            let got = tone(kind, dark);
            for (g, w) in got.iter().zip(want) {
                assert!(
                    (g - w).abs() <= 3.0,
                    "{kind:?} dark {dark}: {got:?} want {want:?}"
                );
            }
        }
    }

    #[test]
    fn a_sheet_is_opaque() {
        for dark in [false, true] {
            let [black, mid, white] = tone(GlassKind::Sheet, dark);
            assert!(
                (black - mid).abs() < 0.5 && (mid - white).abs() < 0.5,
                "{dark}"
            );
            // The same fill whatever is behind, even a saturated color.
            let material = GlassKind::Sheet.material(dark).unwrap();
            assert_eq!(
                material.transfer([1.0, 0.0, 0.0]),
                material.transfer([0.0, 0.0, 1.0])
            );
        }
        assert!((SHEET_PARENT_DIM - 0.4).abs() < 1e-6);
    }

    #[test]
    fn each_profile_blurs_by_its_measured_sigma() {
        use GlassKind::*;
        let sigma = |kind: GlassKind, dark: bool| {
            kind.profile(dark)
                .sigma
                .unwrap_or_else(|| kind.material(dark).unwrap().sigma)
        };
        assert_eq!(sigma(Dock, false), 2.8);
        assert_eq!(sigma(Dock, true), 2.8);
        assert_eq!(sigma(Module, false), 4.0);
        assert_eq!(sigma(Control, false), 6.0);
        assert_eq!(sigma(Menu, true), 8.0);
        assert_eq!(sigma(Menu, false), 13.0);
        assert_eq!(sigma(Popover, true), 18.0);
        assert_eq!(sigma(Popover, false), 13.0);
        // The material is close to its profile, so a region without the profile blurs alike.
        for kind in [Dock, Module, Control, Menu, Popover] {
            for dark in [false, true] {
                let material = kind.material(dark).unwrap();
                assert!(
                    (material.sigma - sigma(kind, dark)).abs() < 0.8,
                    "{kind:?} {dark}"
                );
            }
        }
    }

    #[test]
    fn the_edge_light_is_the_measured_one_and_strongest_on_modules_and_dock() {
        use GlassKind::*;
        // The top row over the grey: material offset times the profile's strength.
        let top = |kind: GlassKind, dark: bool| {
            kind.material(dark).unwrap().edge.top.inner[0] * kind.profile(dark).rim
        };
        assert!((top(Module, false) - 96.0).abs() < 0.5);
        assert!((top(Module, true) - 96.0).abs() < 0.5);
        assert!((top(Dock, true) - 41.0).abs() < 0.5, "{}", top(Dock, true));
        assert!(
            (top(Dock, false) - 64.0).abs() < 0.5,
            "{}",
            top(Dock, false)
        );
        for dark in [false, true] {
            for weaker in [Dock, Control, Menu, Popover, Tooltip, Sheet, Banner] {
                assert!(top(weaker, dark) < top(Module, dark), "{weaker:?} {dark}");
            }
        }
    }

    #[test]
    fn menus_popovers_tooltips_and_sheets_have_a_dark_outline_outside() {
        use GlassKind::*;
        for kind in [Menu, Popover, Tooltip, Sheet] {
            for dark in [false, true] {
                let edge = kind.material(dark).unwrap().edge;
                for side in [edge.top, edge.side, edge.bottom] {
                    assert!(side.outer[0] >= 0.2, "{kind:?} {dark}: {:?}", side.outer);
                }
                assert!(kind.profile(dark).line > 0.0);
            }
        }
        // The dark outline is darker than the light one.
        let outline = |dark| Menu.material(dark).unwrap().edge.top.outer[0];
        assert!(outline(true) > outline(false));
    }

    #[test]
    fn only_thin_glass_refracts() {
        use GlassKind::*;
        let lens = |kind: GlassKind, dark: bool| {
            let (d, at) = kind.material(dark).unwrap().lens.max_displacement();
            (d * kind.profile(dark).refraction, at)
        };
        // The Control Center: up to ~3.5 pt, around 15 pt from the edge.
        let (d, at) = lens(Module, false);
        assert!(
            (d - 3.5).abs() < 0.3 && (at - 15.0).abs() <= 1.0,
            "{d} {at}"
        );
        // The Dock: 2-3 pt.
        for dark in [false, true] {
            let (d, _) = lens(Dock, dark);
            assert!((2.0..=3.0).contains(&d), "{d}");
        }
        // Gone within ~30 pt on both.
        for material in [REGULAR_LIGHT, CLEAR_LIGHT, CLEAR_DARK] {
            assert!(material.lens.inward_reach() <= 30.0);
            for curve in [material.lens.side, material.lens.top, material.lens.bottom] {
                let (t, s) = *curve.last().unwrap();
                assert!(t <= 30.0 && (s - t).abs() < 1e-6);
            }
        }
        // Thin controls refract, less than the modules.
        let (d, _) = lens(Control, true);
        assert!(d > 0.0 && d < 3.5);
        // Thick glass does not.
        for kind in [Menu, Popover, Tooltip, Sheet, Banner] {
            for dark in [false, true] {
                assert_eq!(kind.profile(dark).refraction, 0.0, "{kind:?}");
            }
        }
        assert_eq!(Menu.material(false).unwrap().lens.max_displacement().0, 0.0);
    }

    #[test]
    fn small_controls_refract_less_and_closer_to_the_edge_than_modules() {
        let control = GlassKind::Control.profile(false);
        let module = GlassKind::Module.profile(false);
        assert!(control.refraction < module.refraction);
        assert!(control.falloff < module.falloff);
        // Labels and tooltips: no lens at all.
        assert_eq!(GlassKind::Tooltip.profile(true).refraction, 0.0);
        // Panels: gentler, over a wider band.
        let panel = GlassKind::Panel.profile(false);
        assert!(panel.refraction < 1.0 && panel.falloff > 1.0);
    }

    #[test]
    fn the_lens_band_fits_the_shape() {
        // The modules (64 pt, sampling up to 50 pt inside) keep the measured band.
        assert_eq!(lens_width(64.0, 50.0, 1.0), 1.0);
        // A 24 pt pill: the lens samples at most 24 pt inside, its far edge.
        let pill = lens_width(24.0, 50.0, 1.0);
        assert!((pill - 0.48).abs() < 1e-6, "{pill}");
        assert!(pill * 50.0 <= 24.0 + 1e-4);
        // The profile's own width when it fits.
        assert_eq!(lens_width(200.0, 50.0, 0.6), 0.6);
        // A wide band on a large panel stays wide.
        assert_eq!(lens_width(600.0, 50.0, 1.5), 1.5);
        // Degenerate inputs never give zero or a negative width.
        assert!(lens_width(0.0, 50.0, 1.0) > 0.0);
        assert!(lens_width(1.0, 50.0, 1.0) >= 0.05);
        assert_eq!(lens_width(30.0, 0.0, 0.8), 0.8);
    }

    #[test]
    fn a_shadow_reaches_three_sigmas_and_its_offset() {
        let menu = GlassKind::Menu.profile(false).shadow;
        assert!((menu.reach() - (3.0 * 9.7 + 4.0)).abs() < 1e-4);
        assert_eq!(GlassShadow::NONE.reach(), 0.0);
        assert!(GlassKind::Menu.profile(true).shadow.opacity > menu.opacity);
    }
}
