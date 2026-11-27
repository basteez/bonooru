//! The modal dialog component and its host (Story 4.3, UX-DR7, UX-DR62).
//!
//! [`Modals`] (one per `App`, shared by every screen) shows at most one
//! dialog: a request while one is open waits in a FIFO queue (a request with
//! the id of the open or a queued dialog is ignored) and opens after the
//! current one closes, never in the same frame. Esc closes the open dialog
//! (`Dismissed`); a click on the backdrop does not (D4). The primary button
//! has keyboard focus when the dialog opens; Enter activates the focused
//! button (egui: a focused clickable widget is clicked by Enter). Tab and
//! Shift+Tab stay inside the dialog: egui gives focus only to widgets on the
//! top modal layer or above (`Memory::allows_interaction`).

use std::collections::VecDeque;
use std::hash::Hash;

use egui::epaint::Shadow;
use egui::{
    Align, Align2, Context, Frame, Id, Key, Layout, Margin, Modal, Modifiers, Rect, Sense, Stroke,
    Ui, pos2, vec2,
};

use super::paint;
use super::theme::{self, components::modal_dialog as md};
use super::widgets::{self, ButtonKind};

/// A dialog button.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ModalAction {
    pub label: String,
    pub enabled: bool,
}

impl ModalAction {
    /// An enabled action.
    #[must_use]
    pub fn new(label: impl Into<String>) -> Self {
        Self {
            label: label.into(),
            enabled: true,
        }
    }

    /// The same action, disabled (drawn at `button-disabled` opacity).
    #[must_use]
    pub fn disabled(self) -> Self {
        Self {
            enabled: false,
            ..self
        }
    }
}

/// What the dialog says under its title.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ModalBody {
    /// One centred text in `dialog-body`.
    Text(String),
    /// Checklist rows in `dialog-body`, each with an amber diamond bullet.
    Checklist(Vec<String>),
}

/// One dialog.
#[derive(Clone, Debug, PartialEq)]
pub struct ModalSpec {
    /// Identifies the dialog (deduplication, egui area id).
    pub id: Id,
    pub title: String,
    /// Requested outer width; clamped to `min-width`..=`width` of
    /// `components.modal-dialog` (D6).
    pub width: f32,
    pub body: ModalBody,
    /// The rightmost button (amber, glow, initial focus).
    pub primary: ModalAction,
    /// The ghost button to its left, if any.
    pub secondary: Option<ModalAction>,
}

impl ModalSpec {
    /// A dialog at the default width with only a primary button.
    #[must_use]
    pub fn new(
        id: impl Hash + std::fmt::Debug,
        title: impl Into<String>,
        body: ModalBody,
        primary: ModalAction,
    ) -> Self {
        Self {
            id: Id::unique(id),
            title: title.into(),
            width: md::WIDTH,
            body,
            primary,
            secondary: None,
        }
    }

    /// Adds the ghost secondary button.
    #[must_use]
    pub fn with_secondary(self, secondary: ModalAction) -> Self {
        Self {
            secondary: Some(secondary),
            ..self
        }
    }

    /// Asks for an outer width (clamped when shown).
    #[must_use]
    pub fn with_width(self, width: f32) -> Self {
        Self { width, ..self }
    }

    /// The outer width the dialog is drawn at.
    #[must_use]
    pub fn outer_width(&self) -> f32 {
        if self.width.is_nan() {
            md::WIDTH
        } else {
            self.width.clamp(md::MIN_WIDTH, md::WIDTH)
        }
    }
}

/// How a dialog closed.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ModalOutcome {
    Primary,
    Secondary,
    /// Esc.
    Dismissed,
}

#[derive(Debug)]
struct Open {
    spec: ModalSpec,
    /// The primary button has been seen focused in a visible frame.
    focused: bool,
}

/// The app's dialog host: at most one dialog open, the others queued.
#[derive(Debug, Default)]
pub struct Modals {
    open: Option<Open>,
    queue: VecDeque<ModalSpec>,
}

impl Modals {
    /// Asks for a dialog. Shown now if none is open, else queued (FIFO);
    /// ignored if a dialog with the same id is open or queued.
    pub fn request(&mut self, spec: ModalSpec) {
        let open_id = self.open.as_ref().map(|o| o.spec.id);
        if open_id == Some(spec.id) || self.queue.iter().any(|q| q.id == spec.id) {
            return;
        }
        if self.open.is_none() {
            self.open = Some(Open {
                spec,
                focused: false,
            });
        } else {
            self.queue.push_back(spec);
        }
    }

    /// Whether a dialog is open.
    #[must_use]
    pub fn is_open(&self) -> bool {
        self.open.is_some()
    }

    /// The id of the open dialog.
    #[must_use]
    pub fn open_id(&self) -> Option<Id> {
        self.open.as_ref().map(|o| o.spec.id)
    }

    /// How many requests wait behind the open dialog.
    #[must_use]
    pub fn queued(&self) -> usize {
        self.queue.len()
    }

    /// Shows the open dialog, if any, over everything drawn before it in
    /// this frame. Returns the dialog's id and outcome in the frame it
    /// closes; the next queued dialog then opens from the next frame.
    pub fn show(&mut self, ctx: &Context) -> Option<(Id, ModalOutcome)> {
        if self.open.is_none() {
            let spec = self.queue.pop_front()?;
            self.open = Some(Open {
                spec,
                focused: false,
            });
        }
        let open = self.open.as_mut()?;
        let outcome = show_dialog(ctx, open);
        let id = open.spec.id;
        let outcome = outcome?;
        self.open = self.queue.pop_front().map(|spec| Open {
            spec,
            focused: false,
        });
        ctx.request_repaint();
        Some((id, outcome))
    }
}

/// The dialog frame: `panel-raised`, 1 pt `rim`, `rounded.lg`, padding
/// 20 / 26, no shadow (no drop shadows as hierarchy).
fn frame() -> Frame {
    // The paddings are whole points well inside i8.
    Frame {
        inner_margin: Margin::symmetric(md::PADDING_X as i8, md::PADDING_Y as i8),
        outer_margin: Margin::ZERO,
        fill: theme::PANEL_RAISED,
        stroke: Stroke::new(1.0, theme::RIM),
        corner_radius: theme::corner(theme::rounded::LG),
        shadow: Shadow::NONE,
    }
}

fn show_dialog(ctx: &Context, open: &mut Open) -> Option<ModalOutcome> {
    let spec = &open.spec;
    let frame = frame();
    let border = frame.stroke.width;
    let content_w = spec.outer_width() - 2.0 * (md::PADDING_X + border);
    let area = Modal::default_area(spec.id).accessible_name(spec.title.clone());
    let response = Modal::new(spec.id)
        .area(area)
        .backdrop_color(theme::SCRIM)
        .frame(frame)
        .show(ctx, |ui| contents(ui, spec, content_w, open.focused));
    let (clicked, primary_focused_visible) = response.inner;
    if primary_focused_visible {
        open.focused = true;
    }
    if clicked.is_some() {
        return clicked;
    }
    // Esc closes only the topmost dialog and only when no popup is open;
    // consumed here so nothing later reads it (Story 4.7 reads Esc for STOP
    // at the top of the frame, before any screen or dialog).
    if response.is_top_modal
        && !response.any_popup_open
        && ctx.input_mut(|i| i.consume_key(Modifiers::NONE, Key::Escape))
    {
        return Some(ModalOutcome::Dismissed);
    }
    None
}

/// The dialog content. Returns the clicked button (if any) and whether the
/// primary button is focused in a visible frame.
fn contents(
    ui: &mut Ui,
    spec: &ModalSpec,
    content_w: f32,
    focused_once: bool,
) -> (Option<ModalOutcome>, bool) {
    ui.set_width(content_w);
    ui.spacing_mut().item_spacing = vec2(0.0, 0.0);
    let ctx = ui.ctx().clone();
    let ppp = ctx.pixels_per_point();

    // Title, centred.
    let title = paint::layout(
        &ctx,
        &theme::DIALOG_TITLE,
        &spec.title,
        theme::PARCHMENT,
        content_w,
        Align::Center,
    );
    let title_rect = Rect::from_min_size(ui.cursor().min, vec2(content_w, title.rect.height()));
    widgets::text_node(ui, title_rect, title, Align2::CENTER_TOP);

    // Flourish rule.
    ui.add_space(md::FLOURISH_MARGIN_TOP);
    let (flourish, _) =
        ui.allocate_exact_size(vec2(content_w, md::FLOURISH_HEIGHT), Sense::hover());
    paint::paint_flourish(
        ui.painter(),
        pos2(flourish.center().x, flourish.top()),
        theme::RIM,
        ppp,
    );
    ui.add_space(md::FLOURISH_MARGIN_BOTTOM);

    match &spec.body {
        ModalBody::Text(text) => {
            let galley = paint::layout(
                &ctx,
                &theme::DIALOG_BODY,
                text,
                theme::TEXT,
                content_w,
                Align::Center,
            );
            let rect = Rect::from_min_size(ui.cursor().min, vec2(content_w, galley.rect.height()));
            widgets::text_node(ui, rect, galley, Align2::CENTER_TOP);
            ui.add_space(md::TEXT_MARGIN_BOTTOM);
        }
        ModalBody::Checklist(rows) => {
            checklist(ui, rows, content_w, ppp);
            ui.add_space(md::CHECKLIST_MARGIN_BOTTOM);
        }
    }

    // Actions, right-aligned: primary first in code so it is rightmost.
    let mut clicked = None;
    let mut primary_focused_visible = false;
    ui.allocate_ui_with_layout(
        vec2(content_w, md::ACTION_HEIGHT),
        Layout::right_to_left(Align::Center),
        |ui| {
            ui.spacing_mut().item_spacing = vec2(md::ACTION_GAP, 0.0);
            let primary = widgets::button(
                ui,
                ButtonKind::Primary,
                &spec.primary.label,
                spec.primary.enabled,
                None,
            );
            if spec.primary.enabled && !focused_once {
                if primary.has_focus() && ui.is_visible() {
                    primary_focused_visible = true;
                } else {
                    primary.request_focus();
                }
            }
            if primary.clicked() {
                clicked = Some(ModalOutcome::Primary);
            }
            if let Some(secondary) = &spec.secondary
                && widgets::button(
                    ui,
                    ButtonKind::Ghost,
                    &secondary.label,
                    secondary.enabled,
                    None,
                )
                .clicked()
            {
                clicked = Some(ModalOutcome::Secondary);
            }
        },
    );
    (clicked, primary_focused_visible)
}

/// Checklist rows: `row-padding-y` above and below the text, a
/// `bullet-size` amber diamond (a square turned 45°) centred on the text,
/// `row-gap` between the diamond's right corner and the text, and a 1 pt
/// `rule` line after every row but the last.
fn checklist(ui: &mut Ui, rows: &[String], content_w: f32, ppp: f32) {
    let ctx = ui.ctx().clone();
    let diagonal = md::BULLET_SIZE * std::f32::consts::SQRT_2;
    let text_x = diagonal + md::ROW_GAP;
    let count = rows.len();
    for (i, row) in rows.iter().enumerate() {
        ui.add_space(md::ROW_PADDING_Y);
        let galley = paint::layout(
            &ctx,
            &theme::DIALOG_BODY,
            row,
            theme::TEXT,
            content_w - text_x,
            Align::Min,
        );
        let left = ui.cursor().min;
        let text_rect = Rect::from_min_size(pos2(left.x + text_x, left.y), galley.rect.size());
        widgets::text_node(ui, text_rect, galley, Align2::LEFT_TOP);
        let center = pos2(left.x + diagonal / 2.0, text_rect.center().y);
        // A diamond is convex by construction; a refusal would only drop it.
        let _ = paint::fill_convex(
            ui.painter(),
            paint::diamond_points(center, md::BULLET_SIZE),
            theme::AMBER,
            Stroke::NONE,
        );
        ui.add_space(md::ROW_PADDING_Y);
        if i + 1 < count {
            let width = paint::stroke(1.0, theme::RULE, ppp);
            let y = ui.cursor().min.y + width.width / 2.0;
            ui.painter()
                .line_segment([pos2(left.x, y), pos2(left.x + content_w, y)], width);
            ui.add_space(width.width);
        }
    }
}
