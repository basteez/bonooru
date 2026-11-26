//! The theme: every design token of the `DESIGN.md` frontmatter sections
//! `colors`, `rounded`, `spacing` and `typography` (UX-DR1). This module is
//! the single source: no other module writes a colour hex, a token size or a
//! font size. `tests/design_tokens.rs` (G.3) compares the tables below with
//! the `DESIGN.md` frontmatter both ways.

use egui::{Color32, CornerRadius, FontFamily, FontId, RichText, Stroke, Visuals};

/// A colour token: its `DESIGN.md` name, its unmultiplied `0xRRGGBBAA`
/// value as written there, and the `Color32` built from it.
///
/// The test compares `rgba` (exact) rather than the premultiplied `Color32`,
/// which can map two different low-alpha hexes to the same value.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ColorToken {
    pub name: &'static str,
    pub rgba: u32,
    pub color: Color32,
}

/// `0xRRGGBBAA` (unmultiplied, as in `DESIGN.md`) to `Color32`, the same as
/// `Color32::from_rgba_unmultiplied`.
#[must_use]
pub const fn rgba(hex: u32) -> Color32 {
    let [r, g, b, a] = hex.to_be_bytes();
    Color32::from_rgba_unmultiplied_const(r, g, b, a)
}

/// One literal per token: the constant and the table entry come from it.
macro_rules! color_tokens {
    ($($(#[$doc:meta])* $konst:ident = $name:literal $hex:literal;)*) => {
        $( $(#[$doc])* pub const $konst: Color32 = rgba($hex); )*

        const COLOR_TOKENS: &[ColorToken] = &[
            $( ColorToken { name: $name, rgba: $hex, color: $konst }, )*
        ];
    };
}

color_tokens! {
    // Frame and panels (dark, warm).
    FRAME = "frame" 0x16110cff;
    PANEL = "panel" 0x1c150fff;
    PANEL_RAISED = "panel-raised" 0x221a13ff;
    WELL = "well" 0x2a1e13ff;
    CONTROL = "control" 0x56402aff;
    RULE = "rule" 0x3a2a1bff;
    RULE_STRONG = "rule-strong" 0x4a3826ff;
    RIM = "rim" 0xa37a3eff;
    RIM_SOFT = "rim-soft" 0xa37a3e66;
    TEXT = "text" 0xf3e3bdff;
    TEXT_MUTED = "text-muted" 0xc4a676ff;
    // Parchment card.
    PARCHMENT = "parchment" 0xecdcb2ff;
    PARCHMENT_LANE_ALT = "parchment-lane-alt" 0xe4d2a5ff;
    PARCHMENT_HEADER = "parchment-header" 0xe2cf9fff;
    PARCHMENT_HEADER_ALT = "parchment-header-alt" 0xd9c491ff;
    PARCHMENT_RULER = "parchment-ruler" 0xdcc794ff;
    PARCHMENT_RULE = "parchment-rule" 0xd8c595ff;
    PARCHMENT_GRID = "parchment-grid" 0xb59a63ff;
    INK = "ink" 0x2b2015ff;
    INK_SOFT = "ink-soft" 0x6e5132ff;
    INK_RULER = "ink-ruler" 0x4a3520ff;
    BRACKET = "bracket" 0x7a5a33ff;
    NOTE = "note" 0x5b3d22ff;
    // Accent: amber glow.
    AMBER = "amber" 0xf0b43cff;
    AMBER_GLOW = "amber-glow" 0xf0b43c66;
    AMBER_WASH = "amber-wash" 0xf0b43c22;
    /// `[ASSUMPTION]` until UX Open Question 7 is answered (Story 4.1 AC 11).
    AMBER_INK = "amber-ink" 0x8a5a12ff;
    // Ember.
    EMBER = "ember" 0xb8432fff;
    EMBER_WASH = "ember-wash" 0xb8432f2e;
    EMBER_TEXT = "ember-text" 0xf2a58fff;
    ON_EMBER = "on-ember" 0xfff4e0ff;
    STOP_IDLE_FILL = "stop-idle-fill" 0x3a1712ff;
    STOP_IDLE_TEXT = "stop-idle-text" 0xe3a597ff;
    // Out-of-range zone.
    OUT_OF_RANGE_ZONE = "out-of-range-zone" 0xcfc6b4ff;
    OUT_OF_RANGE_INK = "out-of-range-ink" 0x55504aff;
    OUT_OF_RANGE_NOTE = "out-of-range-note" 0xb9b0a0ff;
    // Status.
    STATUS_OK = "status-ok" 0x8fd16fff;
    /// `[ASSUMPTION]` until UX Open Question 7 is answered.
    STATUS_WARNING = "status-warning" 0xf08a4bff;
    /// `[ASSUMPTION]` until UX Open Question 7 is answered.
    STATUS_ERROR = "status-error" 0xef6a55ff;
    /// `[ASSUMPTION]` until UX Open Question 7 is answered.
    STATUS_UNKNOWN = "status-unknown" 0x9a8a72ff;
    // Overlays.
    BANNER_FILL = "banner-fill" 0x1c1610e0;
    BANNER_BORDER = "banner-border" 0xe9dcbc66;
    VEIL = "veil" 0x16110ccc;
    /// `[ASSUMPTION]` until UX Open Question 7 is answered.
    SCRIM = "scrim" 0x0b0806b3;
}

/// Every colour token in `DESIGN.md` order.
#[must_use]
pub fn color_tokens() -> &'static [ColorToken] {
    COLOR_TOKENS
}

/// `rounded` tokens, in points.
pub mod rounded {
    pub const XS: f32 = 2.0;
    pub const SM: f32 = 4.0;
    pub const MD: f32 = 6.0;
    pub const LG: f32 = 8.0;
    pub const XL: f32 = 12.0;
    /// Kept as the token value; [`super::corner`] clamps it to 255 because
    /// egui's `CornerRadius` stores `u8` (any radius ≥ half the shorter side
    /// draws a pill anyway).
    pub const FULL: f32 = 9999.0;
}

/// Every `rounded` token by its `DESIGN.md` name.
#[must_use]
pub fn rounded_tokens() -> &'static [(&'static str, f32)] {
    use rounded::*;
    &[
        ("xs", XS),
        ("sm", SM),
        ("md", MD),
        ("lg", LG),
        ("xl", XL),
        ("full", FULL),
    ]
}

/// `spacing` tokens, in points: the scale `S1`..`S6` and the named sizes.
pub mod spacing {
    pub const S1: f32 = 4.0;
    pub const S2: f32 = 8.0;
    pub const S3: f32 = 12.0;
    pub const S4: f32 = 16.0;
    pub const S5: f32 = 24.0;
    pub const S6: f32 = 32.0;
    pub const STATUS_STRIP_HEIGHT: f32 = 44.0;
    pub const BOTTOM_BAR_HEIGHT: f32 = 88.0;
    pub const SONG_PANEL_WIDTH: f32 = 260.0;
    pub const GAMEPAD_PANEL_WIDTH: f32 = 300.0;
    pub const GAMEPAD_COMPACT_WIDTH: f32 = 200.0;
    pub const SONG_RAIL_WIDTH: f32 = 48.0;
    pub const CARD_INSET: f32 = 6.0;
    pub const NOTE_VIEW_HEADER_HEIGHT: f32 = 40.0;
    pub const RULER_HEIGHT: f32 = 22.0;
    pub const LANE_HEIGHT: f32 = 21.0;
    pub const LANE_HEIGHT_MIN: f32 = 18.0;
    pub const LANE_HEADER_WIDTH: f32 = 96.0;
    pub const OUT_OF_RANGE_ZONE_HEIGHT: f32 = 30.0;
    pub const OUT_OF_RANGE_ZONE_COLLAPSED: f32 = 6.0;
    pub const SPLIT_DIVIDER_HEIGHT: f32 = 6.0;
    pub const NOTE_BAR_HEIGHT: f32 = 15.0;
    pub const BIG_BUTTON_HEIGHT: f32 = 60.0;
    pub const BIG_BUTTON_MIN_WIDTH: f32 = 190.0;
    pub const CORNER_BRACKET_SIZE: f32 = 22.0;
    pub const WIZARD_LIST_WIDTH: f32 = 280.0;
    pub const ORB_ICON_SIZE: f32 = 22.0;
    pub const FOCUS_RING_WIDTH: f32 = 2.0;
    pub const FOCUS_RING_OFFSET: f32 = 2.0;
}

/// Every `spacing` token by its `DESIGN.md` name.
#[must_use]
pub fn spacing_tokens() -> &'static [(&'static str, f32)] {
    use spacing::*;
    &[
        ("1", S1),
        ("2", S2),
        ("3", S3),
        ("4", S4),
        ("5", S5),
        ("6", S6),
        ("status-strip-height", STATUS_STRIP_HEIGHT),
        ("bottom-bar-height", BOTTOM_BAR_HEIGHT),
        ("song-panel-width", SONG_PANEL_WIDTH),
        ("gamepad-panel-width", GAMEPAD_PANEL_WIDTH),
        ("gamepad-compact-width", GAMEPAD_COMPACT_WIDTH),
        ("song-rail-width", SONG_RAIL_WIDTH),
        ("card-inset", CARD_INSET),
        ("note-view-header-height", NOTE_VIEW_HEADER_HEIGHT),
        ("ruler-height", RULER_HEIGHT),
        ("lane-height", LANE_HEIGHT),
        ("lane-height-min", LANE_HEIGHT_MIN),
        ("lane-header-width", LANE_HEADER_WIDTH),
        ("out-of-range-zone-height", OUT_OF_RANGE_ZONE_HEIGHT),
        ("out-of-range-zone-collapsed", OUT_OF_RANGE_ZONE_COLLAPSED),
        ("split-divider-height", SPLIT_DIVIDER_HEIGHT),
        ("note-bar-height", NOTE_BAR_HEIGHT),
        ("big-button-height", BIG_BUTTON_HEIGHT),
        ("big-button-min-width", BIG_BUTTON_MIN_WIDTH),
        ("corner-bracket-size", CORNER_BRACKET_SIZE),
        ("wizard-list-width", WIZARD_LIST_WIDTH),
        ("orb-icon-size", ORB_ICON_SIZE),
        ("focus-ring-width", FOCUS_RING_WIDTH),
        ("focus-ring-offset", FOCUS_RING_OFFSET),
    ]
}

/// The three bundled type families (UX-DR2).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Family {
    /// Cormorant Garamond, egui family `Name("display")`.
    Display,
    /// Nunito Sans, egui `Proportional`.
    Body,
    /// Hack (egui `default_fonts`), egui `Monospace`.
    Code,
}

/// The egui family name of the display face (Cormorant Garamond first).
pub const DISPLAY_FAMILY_NAME: &str = "display";

impl Family {
    /// The `fontFamily` value of `DESIGN.md`.
    #[must_use]
    pub const fn design_name(self) -> &'static str {
        match self {
            Self::Display => "Cormorant Garamond",
            Self::Body => "Nunito Sans",
            Self::Code => "Hack",
        }
    }

    /// The egui font family the fonts module installs for it.
    #[must_use]
    pub fn egui_family(self) -> FontFamily {
        match self {
            Self::Display => FontFamily::Name(DISPLAY_FAMILY_NAME.into()),
            Self::Body => FontFamily::Proportional,
            Self::Code => FontFamily::Monospace,
        }
    }

    /// Whether the face has a `wght` axis (Hack is a static font).
    #[must_use]
    pub const fn is_variable(self) -> bool {
        !matches!(self, Self::Code)
    }
}

/// A typography token.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct TypeStyle {
    pub family: Family,
    /// Points (`DESIGN.md` px = egui points).
    pub size: f32,
    /// The `wght` axis value.
    pub weight: f32,
    /// Line height as a factor of `size` (`DESIGN.md` `lineHeight`), if set.
    pub line_height: Option<f32>,
    /// Extra letter spacing in em (`DESIGN.md` `letterSpacing`), 0 if unset.
    pub letter_spacing_em: f32,
}

impl TypeStyle {
    const fn new(family: Family, size: f32, weight: f32) -> Self {
        Self {
            family,
            size,
            weight,
            line_height: None,
            letter_spacing_em: 0.0,
        }
    }

    const fn lh(self, factor: f32) -> Self {
        Self {
            line_height: Some(factor),
            ..self
        }
    }

    const fn ls(self, em: f32) -> Self {
        Self {
            letter_spacing_em: em,
            ..self
        }
    }

    /// The egui font id (size + family).
    #[must_use]
    pub fn font_id(&self) -> FontId {
        FontId::new(self.size, self.family.egui_family())
    }

    /// Extra letter spacing in points (`em × size`).
    #[must_use]
    pub fn letter_spacing_pt(&self) -> f32 {
        self.letter_spacing_em * self.size
    }
}

use Family::{Body, Code, Display};

pub const BRAND: TypeStyle = TypeStyle::new(Display, 23.0, 700.0);
pub const DIALOG_TITLE: TypeStyle = TypeStyle::new(Display, 21.0, 700.0).lh(1.2);
pub const PANEL_TAB: TypeStyle = TypeStyle::new(Display, 16.0, 700.0);
pub const CARD_TITLE: TypeStyle = TypeStyle::new(Display, 24.0, 700.0).lh(1.2);
pub const COUNTDOWN: TypeStyle = TypeStyle::new(Display, 128.0, 700.0).lh(1.0);
pub const BODY: TypeStyle = TypeStyle::new(Body, 13.5, 400.0).lh(1.45);
pub const BODY_STRONG: TypeStyle = TypeStyle::new(Body, 13.5, 600.0);
pub const BANNER_TEXT: TypeStyle = TypeStyle::new(Body, 15.0, 400.0).lh(1.4);
pub const LABEL: TypeStyle = TypeStyle::new(Body, 11.5, 700.0).ls(0.06);
pub const SMALL: TypeStyle = TypeStyle::new(Body, 12.5, 400.0);
pub const FILENAME: TypeStyle = TypeStyle::new(Body, 15.0, 600.0);
pub const VALUE_LG: TypeStyle = TypeStyle::new(Body, 24.0, 700.0);
pub const VALUE_MD: TypeStyle = TypeStyle::new(Body, 17.0, 700.0);
pub const BUTTON_LG: TypeStyle = TypeStyle::new(Body, 17.0, 700.0).lh(1.1);
pub const BUTTON_LG_SUB: TypeStyle = TypeStyle::new(Body, 12.5, 400.0);
pub const LANE_NAME: TypeStyle = TypeStyle::new(Body, 12.0, 700.0);
pub const LANE_FINGERING: TypeStyle = TypeStyle::new(Body, 10.5, 400.0);
pub const RULER: TypeStyle = TypeStyle::new(Body, 10.5, 400.0);
pub const NOTE_LABEL: TypeStyle = TypeStyle::new(Body, 11.5, 700.0);
pub const GLYPH: TypeStyle = TypeStyle::new(Body, 10.5, 700.0);
pub const KEY_CHIP: TypeStyle = TypeStyle::new(Body, 10.0, 600.0);
/// `typography.code` is a note in `DESIGN.md` ("Hack, size 13px",
/// `[ASSUMPTION]` until UX Open Question 7 is answered). Hack is static: the
/// weight is its only weight, Regular.
pub const CODE: TypeStyle = TypeStyle::new(Code, 13.0, 400.0);

/// Every typography token by its `DESIGN.md` name.
#[must_use]
pub fn typography_tokens() -> &'static [(&'static str, TypeStyle)] {
    &[
        ("brand", BRAND),
        ("dialog-title", DIALOG_TITLE),
        ("panel-tab", PANEL_TAB),
        ("card-title", CARD_TITLE),
        ("countdown", COUNTDOWN),
        ("body", BODY),
        ("body-strong", BODY_STRONG),
        ("banner-text", BANNER_TEXT),
        ("label", LABEL),
        ("small", SMALL),
        ("filename", FILENAME),
        ("value-lg", VALUE_LG),
        ("value-md", VALUE_MD),
        ("button-lg", BUTTON_LG),
        ("button-lg-sub", BUTTON_LG_SUB),
        ("lane-name", LANE_NAME),
        ("lane-fingering", LANE_FINGERING),
        ("ruler", RULER),
        ("note-label", NOTE_LABEL),
        ("glyph", GLYPH),
        ("key-chip", KEY_CHIP),
        ("code", CODE),
    ]
}

/// A `CornerRadius` from a `rounded` token, clamped to egui's `u8` range.
#[must_use]
pub fn corner(radius: f32) -> CornerRadius {
    // Float-to-int `as` saturates anyway; the clamp states the intent.
    let r = radius.round().clamp(0.0, 255.0) as u8;
    CornerRadius::same(r)
}

/// Text in a typography token: family, size, `wght`, letter spacing and line
/// height. The weight always comes from the token (never `strong()`).
#[must_use]
pub fn rich(style: &TypeStyle, text: impl Into<String>) -> RichText {
    let mut t = RichText::new(text)
        .font(style.font_id())
        .extra_letter_spacing(style.letter_spacing_pt())
        .line_height(style.line_height.map(|f| f * style.size));
    if style.family.is_variable() {
        t = t.variation("wght", style.weight);
    }
    t
}

/// A label: uppercase, in the `label` token (0.06 em tracking).
#[must_use]
pub fn label(text: &str) -> RichText {
    rich(&LABEL, text.to_uppercase())
}

/// The dark visuals from the tokens. Widget restyling (focus ring, buttons)
/// is Story 4.3.
#[must_use]
pub fn visuals() -> Visuals {
    let mut v = Visuals::dark();
    v.panel_fill = FRAME;
    v.window_fill = PANEL_RAISED;
    v.window_stroke = Stroke::new(1.0, RIM);
    v.window_corner_radius = corner(rounded::LG);
    v.extreme_bg_color = WELL;
    v.faint_bg_color = PANEL;
    v.widgets.noninteractive.fg_stroke.color = TEXT;
    v.selection.bg_fill = AMBER_WASH;
    v.selection.stroke = Stroke::new(1.0, AMBER);
    v.hyperlink_color = AMBER;
    v
}
