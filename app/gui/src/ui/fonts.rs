//! Font setup (UX-DR2). Cormorant Garamond and Nunito Sans are bundled as
//! unmodified variable TTFs (`assets/fonts/`, provenance in `app/README.md`);
//! Hack, Ubuntu-Light and `egui-icons` come from egui's `default_fonts`. No
//! OS font is loaded (eframe's `system_fonts` feature is off).

use std::sync::Arc;

use egui::epaint::text::{FontTweak, VariationCoords};
use egui::{FontData, FontDefinitions, FontFamily, TextStyle};

use super::theme::{self, DISPLAY_FAMILY_NAME};

/// egui font name of the bundled Nunito Sans.
pub const NUNITO_SANS: &str = "NunitoSans";
/// egui font name of the bundled Cormorant Garamond.
pub const CORMORANT_GARAMOND: &str = "CormorantGaramond";

/// The bundled Nunito Sans variable font (`wght` 200–1000).
pub const NUNITO_SANS_TTF: &[u8] = include_bytes!("../../assets/fonts/NunitoSans-VF.ttf");
/// The bundled Cormorant Garamond variable font (`wght` 300–700).
pub const CORMORANT_GARAMOND_TTF: &[u8] =
    include_bytes!("../../assets/fonts/CormorantGaramond-VF.ttf");

/// Default `wght` of text that names no weight: both files default to their
/// axis minimum (200 and 300), so without these tweaks all text would be
/// ExtraLight / Light. A text's own `variation("wght", w)` overrides them
/// (epaint chains the font's tweak coords before the text's coords).
pub const NUNITO_DEFAULT_WGHT: f32 = 400.0;
/// Display text is bold unless it says otherwise (every display token is 700).
pub const CORMORANT_DEFAULT_WGHT: f32 = 700.0;

const UBUNTU_LIGHT: &str = "Ubuntu-Light";
const EGUI_ICONS: &str = "egui-icons";
const HACK: &str = "Hack";

fn variable(bytes: &'static [u8], wght: f32) -> Arc<FontData> {
    Arc::new(FontData::from_static(bytes).tweak(FontTweak {
        coords: VariationCoords::new([("wght", wght)]),
        ..Default::default()
    }))
}

/// The app's font definitions: egui's defaults plus the two bundled faces.
#[must_use]
pub fn definitions() -> FontDefinitions {
    let mut defs = FontDefinitions::default();
    defs.font_data.insert(
        NUNITO_SANS.to_owned(),
        variable(NUNITO_SANS_TTF, NUNITO_DEFAULT_WGHT),
    );
    defs.font_data.insert(
        CORMORANT_GARAMOND.to_owned(),
        variable(CORMORANT_GARAMOND_TTF, CORMORANT_DEFAULT_WGHT),
    );

    let names = |list: &[&str]| list.iter().map(|s| (*s).to_owned()).collect::<Vec<_>>();
    defs.families.insert(
        FontFamily::Proportional,
        names(&[NUNITO_SANS, UBUNTU_LIGHT, EGUI_ICONS]),
    );
    defs.families.insert(
        FontFamily::Name(DISPLAY_FAMILY_NAME.into()),
        names(&[CORMORANT_GARAMOND, NUNITO_SANS, UBUNTU_LIGHT, EGUI_ICONS]),
    );
    // Hack first, as egui's default.
    defs.families.insert(
        FontFamily::Monospace,
        names(&[HACK, UBUNTU_LIGHT, EGUI_ICONS]),
    );
    defs
}

/// Installs the fonts and maps egui's built-in text styles to the tokens.
pub fn install(ctx: &egui::Context) {
    ctx.set_fonts(definitions());
    ctx.all_styles_mut(|style| {
        style.text_styles = [
            (TextStyle::Body, theme::BODY.font_id()),
            (TextStyle::Small, theme::SMALL.font_id()),
            (TextStyle::Button, theme::BODY.font_id()),
            (TextStyle::Heading, theme::DIALOG_TITLE.font_id()),
            (TextStyle::Monospace, theme::CODE.font_id()),
        ]
        .into();
    });
}
