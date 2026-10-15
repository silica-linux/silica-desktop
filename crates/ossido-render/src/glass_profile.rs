//! Glass profiles: the settings of the glass per kind of surface (silica-glass-v1 version 7).
//!
//! The material (`glass::MaterialParams`) is the measured color transfer and edge of a glass
//! family; the profile says how a kind of surface uses it: how much it blurs, how strong the
//! lens at the edge is and how far it reaches, how bright the edge light and how dark the edge
//! line, a tint and saturation over the material, and the shadow the compositor may draw
//! under it. The neutral values (strength 1, width 1, no tint) are the material as measured.
//!
//! Where a value is not measured it is tuned by eye against the reference captures
//! (`plan/reference/glass.md` and the notes it cites) and marked *provisional*: the
//! measurement GLASS-PROFILES is requested (silica-reference).

/// The kinds of surface (`silica_glass_v1.profile`, without `default`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum GlassKind {
    /// Menus and context menus.
    Menu,
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
    pub const ALL: [Self; 8] = [
        Self::Menu,
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

    /// The settings of this kind in the light or dark appearance.
    pub fn profile(self, dark: bool) -> GlassProfile {
        let neutral = GlassProfile::NEUTRAL;
        match self {
            // Measured as a whole: the Dock's material (σ, lens, edge) is this profile. No
            // shadow under the Dock in the captures (dock.md).
            Self::Dock => neutral,
            // Measured: the thick material with its ≤ 1–2 pt side lens. The shadow is the one
            // the menus draw.
            Self::Menu => GlassProfile {
                shadow: if dark {
                    MENU_SHADOW_DARK
                } else {
                    MENU_SHADOW_LIGHT
                },
                ..neutral
            },
            // Measured: the regular material is the Control Center modules' glass, lens included.
            Self::Module => neutral,
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
            // Provisional: the modules' lens folds the background in from 42–50 pt inside; on a
            // 24–36 pt control that reads as a distortion of everything around it. Half the
            // strength over 0.6 of the band, a slightly softer edge light, no shadow.
            Self::Control => GlassProfile {
                refraction: 0.5,
                falloff: 0.6,
                rim: 0.85,
                ..neutral
            },
            // Provisional: a 20–24 pt label shows no lens; its edge light is fainter than a
            // menu's. The shadow is the Dock label's (the menus').
            Self::Tooltip => GlassProfile {
                refraction: 0.0,
                rim: 0.7,
                shadow: if dark {
                    MENU_SHADOW_DARK
                } else {
                    MENU_SHADOW_LIGHT
                },
                ..neutral
            },
            // Provisional: sheets and alerts blur a little more than menus and refract half as
            // much; the shadow is the alerts': blur 40 pt (σ ≈ 23.6), 10 pt lower, black 30 %.
            Self::Sheet => GlassProfile {
                sigma: Some(11.0),
                refraction: 0.5,
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

    #[test]
    fn the_measured_kinds_are_their_materials_as_measured() {
        // Dock and modules were measured as a whole: no change over their materials.
        assert_eq!(GlassKind::Dock.profile(false), GlassProfile::NEUTRAL);
        assert_eq!(GlassKind::Module.profile(true), GlassProfile::NEUTRAL);
        assert_eq!(
            GlassKind::Menu.profile(false).without_shadow(),
            GlassProfile::NEUTRAL
        );
        // Banners show no lens (notifications.md).
        assert_eq!(GlassKind::Banner.profile(false).refraction, 0.0);
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
