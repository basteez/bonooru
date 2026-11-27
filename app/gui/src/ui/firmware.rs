//! The "Firmware update needed" dialog (Story 4.6, UX-DR13 in its Epic 4
//! form): OK only, no secondary button, no wizard link, 440 pt wide. The
//! app requests it once per mismatching connection (`gating::FirmwareWatch`)
//! through the shared `Modals` host. Epic 8 replaces OK with "Not now" +
//! "Update now" here. The copy lives in `crate::gating`.

use egui::Id;

use super::modal::{ModalAction, ModalBody, ModalSpec};
use super::theme;
use crate::gating::{MODAL_BODY, MODAL_OK, MODAL_TITLE};

/// The id source of the dialog (stable: `Modals` dedupes by id and the app
/// reads the outcome back by it).
const ID_SOURCE: &str = "firmware-update-needed";

/// The required-update dialog.
#[must_use]
pub fn required_update_dialog() -> ModalSpec {
    ModalSpec::new(
        ID_SOURCE,
        MODAL_TITLE,
        ModalBody::Text(MODAL_BODY.to_owned()),
        ModalAction::new(MODAL_OK),
    )
    .with_width(theme::components::modal_dialog::MIN_WIDTH)
}

/// The dialog's id, for `Modals::take_outcome`.
#[must_use]
pub fn dialog_id() -> Id {
    Id::unique(ID_SOURCE)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn spec_is_ok_only_and_440_wide() {
        let s = required_update_dialog();
        assert_eq!(s.title, "Firmware update needed");
        assert_eq!(s.body, ModalBody::Text(MODAL_BODY.to_owned()));
        assert_eq!(s.primary, ModalAction::new("OK"));
        assert_eq!(s.secondary, None);
        assert!((s.outer_width() - 440.0).abs() < f32::EPSILON);
        assert_eq!(s.id, dialog_id());
        assert_eq!(required_update_dialog().id, s.id, "stable across calls");
    }
}
