//! The gamepad panel (Story 4.8; UX-DR35, UX-DR36, UX-DR38, UX-DR64): the
//! panel tab, the caption, the original Pro-Controller-like pad with its 16
//! button controls and two sticks, the key chips, the legend and the inline
//! hint, drawn from the `DESIGN.md` tokens with the geometry of
//! `mockups/key-empty-state.html` (its 280 × 200 svg; its hexes are not
//! used). Senses the pointer and reports what happened as [`PadInput`]; the
//! pad model (`crate::gamepad`) and the app decide what it means.
//!
//! Controls sense `Sense::CLICK | Sense::DRAG` (sticks `Sense::DRAG`), never
//! `FOCUSABLE`: they stay out of the Tab ring (AC 13) and a press does not
//! take keyboard focus. Every control is one AccessKit node (`Button` or
//! `Slider`); the chips and symbols add none.

use egui::epaint::{Mesh, PathShape};
use egui::{
    Align, Align2, CornerRadius, Id, Pos2, Rect, Response, Role, Sense, StrokeKind, Ui, WidgetInfo,
    pos2, vec2,
};

use super::paint::{self, ButtonState};
use super::symbols::{self, Symbol};
use super::theme::{self, TypeStyle};
use super::widgets;
use crate::gamepad::{self, CAPTION, CHIP_DPAD, Control, LEGEND, MOUSE, Stick, TAB, Target};

/// The letters on the controls ([`theme::GAMEPAD_LETTER`]).
pub const CONTROL_LETTER: TypeStyle = theme::GAMEPAD_LETTER;

/// The knob's travel from the well centre at full deflection (17 − 11).
pub const KNOB_TRAVEL: f32 = 6.0;

/// The body outline of the mock in svg units: `M52,46 Q140,36 228,46
/// Q262,52 270,96 L278,162 Q281,192 256,194 Q236,196 222,174 L204,156
/// Q140,148 76,156 L58,174 Q44,196 24,194 Q-1,192 2,162 L10,96 Q18,52 52,46
/// Z`: each segment is `(control, end)` from the previous end; a line has no
/// control.
pub const BODY_START: (f32, f32) = (52.0, 46.0);
/// A point of the svg (x, y).
pub type SvgPoint = (f32, f32);
/// One outline segment: its control point (none for a line) and its end.
pub type BodySegment = (Option<SvgPoint>, SvgPoint);

/// See [`BODY_START`].
pub const BODY_SEGMENTS: [BodySegment; 12] = [
    (Some((140.0, 36.0)), (228.0, 46.0)),
    (Some((262.0, 52.0)), (270.0, 96.0)),
    (None, (278.0, 162.0)),
    (Some((281.0, 192.0)), (256.0, 194.0)),
    (Some((236.0, 196.0)), (222.0, 174.0)),
    (None, (204.0, 156.0)),
    (Some((140.0, 148.0)), (76.0, 156.0)),
    (None, (58.0, 174.0)),
    (Some((44.0, 196.0)), (24.0, 194.0)),
    (Some((-1.0, 192.0)), (2.0, 162.0)),
    (None, (10.0, 96.0)),
    (Some((18.0, 52.0)), (52.0, 46.0)),
];
/// Chords per quadratic segment of the outline.
pub const BODY_QUAD_SEGMENTS: usize = 12;
/// The point every body fill triangle starts from (svg units): the outline
/// is star-shaped from here, so the fan's triangles never overlap (G.22).
pub const BODY_FAN_CENTRE: (f32, f32) = (140.0, 50.0);

/// The stable id of a control's widget (G.22, G.23).
#[must_use]
pub fn control_id(c: Control) -> Id {
    Id::unique(("gamepad", c.log_name()))
}

/// The stable id of a stick's widget.
#[must_use]
pub fn stick_id(s: Stick) -> Id {
    Id::unique(("gamepad-stick", s.log_name()))
}

/// The galley heights [`layout`] needs, measured once by the caller.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct TextHeights {
    /// The tab label's size ("Gamepad", `typography.panel-tab`).
    pub tab: egui::Vec2,
    /// The caption's height (`typography.small`).
    pub caption: f32,
    /// A legend row's text height (the taller of its two galleys).
    pub row: f32,
}

/// The tab label's line height in points (`typography.panel-tab`, measured
/// at 1 pixel per point). A galley's height moves by up to half a pixel with
/// the pixel density; the layout uses these fixed heights so nothing moves
/// with it (G.1), and G.22 checks every galley fits them at ppp 1–2.
pub const TAB_TEXT_HEIGHT: f32 = 19.0;
/// The caption's line height (`typography.small`).
pub const CAPTION_HEIGHT: f32 = 17.0;
/// A legend row's text height (`typography.small` and `legend-name`).
pub const LEGEND_TEXT_HEIGHT: f32 = 17.0;

impl TextHeights {
    /// The heights the panel uses: the fixed line heights above and the tab
    /// label's measured width, rounded up to whole points.
    #[must_use]
    pub fn measure(ctx: &egui::Context) -> Self {
        let tab = paint::layout_line(ctx, &theme::PANEL_TAB, TAB, theme::INK)
            .rect
            .width();
        Self {
            tab: vec2(tab.ceil(), TAB_TEXT_HEIGHT),
            caption: CAPTION_HEIGHT,
            row: LEGEND_TEXT_HEIGHT,
        }
    }
}

/// Where everything of the panel goes (pure geometry, G.22).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PanelLayout {
    /// The panel tab (parchment, top corners rounded).
    pub tab: Rect,
    /// The 1 pt `rim` underline's centre y, across `underline_x`.
    pub underline_y: f32,
    /// The underline's left and right ends.
    pub underline_x: (f32, f32),
    /// The caption's box.
    pub caption: Rect,
    /// The pad (280 × 200, centred).
    pub pad: Rect,
    /// The six legend rows (padding and divider included).
    pub legend_rows: [Rect; 6],
    /// Where the inline hint may paint (8 pt under the legend, down to the
    /// region's padding).
    pub hint: Rect,
}

/// The panel layout in the gamepad region (AC 1): 10 pt padding; the tab
/// (label + the mock's `2px 14px 0` padding) over a 1 pt underline; the
/// caption 4 pt under it; the pad after the larger of the caption's 4 pt and
/// the pad's 6 pt margin (the mock's CSS margins collapse: 6 pt); the legend
/// 6 pt under the pad; the hint 8 pt under the legend.
#[must_use]
pub fn layout(region: Rect, text: TextHeights) -> PanelLayout {
    use theme::components::gamepad_legend::{DIVIDER_WIDTH, ROW_PADDING_Y};
    use theme::components::gamepad_panel::{
        CAPTION_GAP, HINT_GAP, PAD_HEIGHT, PAD_MARGIN, PAD_WIDTH, PADDING,
    };
    let inner = region.shrink(PADDING);
    let tab = Rect::from_min_size(
        inner.min,
        vec2(
            text.tab.x + 2.0 * TAB_PADDING_X,
            text.tab.y + TAB_PADDING_TOP,
        ),
    );
    let underline_y = tab.bottom() + 0.5;
    let header_bottom = tab.bottom() + 1.0;
    let caption = Rect::from_min_size(
        pos2(inner.left(), header_bottom + CAPTION_GAP),
        vec2(inner.width(), text.caption),
    );
    let pad_top = caption.bottom() + CAPTION_GAP.max(PAD_MARGIN);
    let pad = Rect::from_min_size(
        pos2(inner.center().x - PAD_WIDTH / 2.0, pad_top),
        vec2(PAD_WIDTH, PAD_HEIGHT),
    );
    let row_h = text.row + 2.0 * ROW_PADDING_Y + DIVIDER_WIDTH;
    let legend_top = pad.bottom() + PAD_MARGIN;
    let legend_rows = std::array::from_fn(|i| {
        let top = legend_top + row_h * i as f32;
        Rect::from_min_max(pos2(inner.left(), top), pos2(inner.right(), top + row_h))
    });
    let hint_top = legend_top + row_h * 6.0 + HINT_GAP;
    let hint = Rect::from_min_max(
        pos2(inner.left(), hint_top),
        pos2(inner.right(), inner.bottom().max(hint_top)),
    );
    PanelLayout {
        tab,
        underline_y,
        underline_x: (inner.left(), inner.right()),
        caption,
        pad,
        legend_rows,
        hint,
    }
}

/// The tab's horizontal padding (mock `.ph span.tab{padding:2px 14px 0}`).
pub const TAB_PADDING_X: f32 = 14.0;
/// The tab's top padding.
pub const TAB_PADDING_TOP: f32 = 2.0;

/// A circle.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Circle {
    /// Centre.
    pub center: Pos2,
    /// Radius.
    pub radius: f32,
}

impl Circle {
    fn contains(&self, p: Pos2) -> bool {
        self.center.distance(p) <= self.radius
    }

    /// The bounding square.
    #[must_use]
    pub fn rect(&self) -> Rect {
        Rect::from_center_size(self.center, vec2(2.0 * self.radius, 2.0 * self.radius))
    }
}

/// A control's shape.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum ControlShape {
    /// A rounded rect (ZL, L, R, ZR, Capture, the D-pad arms).
    Rect(Rect, f32),
    /// A circle (A, B, X, Y, −, +, Home).
    Circle(Circle),
}

impl ControlShape {
    /// The hit test (the circle for round controls, AC 4).
    #[must_use]
    pub fn contains(&self, p: Pos2) -> bool {
        match self {
            Self::Rect(r, _) => r.contains(p),
            Self::Circle(c) => c.contains(p),
        }
    }

    /// The bounding rect (the widget's interact rect and AccessKit bounds).
    #[must_use]
    pub fn rect(&self) -> Rect {
        match self {
            Self::Rect(r, _) => *r,
            Self::Circle(c) => c.rect(),
        }
    }
}

/// What a key chip shows.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ChipContent {
    /// Text in `typography.key-chip`.
    Text(&'static str),
    /// A painted symbol (⌫, ↵).
    Symbol(Symbol),
}

/// Every place on the pad, from the mock's svg plus the pad origin.
#[derive(Clone, Debug, PartialEq)]
pub struct PadGeometry {
    /// The pad's top-left corner.
    pub origin: Pos2,
    /// The 16 controls, in [`Control::ALL`] order.
    pub controls: Vec<(Control, ControlShape)>,
    /// The stick wells (r 17), left then right.
    pub wells: [Circle; 2],
    /// The knob radius.
    pub knob_radius: f32,
    /// The D-pad's well (r 22).
    pub dpad_well: Circle,
    /// The D-pad's centre square (no hit target).
    pub dpad_centre: Rect,
    /// The ABXY well (r 31).
    pub abxy_well: Circle,
    /// The key chips.
    pub chips: Vec<(Rect, ChipContent)>,
    /// Where each stick's "mouse" label is centred.
    pub mouse_labels: [Pos2; 2],
}

impl PadGeometry {
    /// The geometry of a pad whose top-left corner is `origin`.
    #[must_use]
    pub fn new(origin: Pos2) -> Self {
        let p = |x: f32, y: f32| origin + vec2(x, y);
        let r = |x: f32, y: f32, w: f32, h: f32| Rect::from_min_size(p(x, y), vec2(w, h));
        let c = |x: f32, y: f32, radius: f32| Circle {
            center: p(x, y),
            radius,
        };
        let shape = |ctl: Control| match ctl {
            Control::Zl => ControlShape::Rect(r(30.0, 8.0, 54.0, 15.0), 6.0),
            Control::L => ControlShape::Rect(r(42.0, 28.0, 54.0, 13.0), 6.0),
            Control::R => ControlShape::Rect(r(184.0, 28.0, 54.0, 13.0), 6.0),
            Control::Zr => ControlShape::Rect(r(196.0, 8.0, 54.0, 15.0), 6.0),
            Control::Minus => ControlShape::Circle(c(112.0, 64.0, 7.0)),
            Control::Plus => ControlShape::Circle(c(168.0, 64.0, 7.0)),
            Control::Capture => ControlShape::Rect(r(114.0, 84.0, 11.0, 11.0), 2.0),
            Control::Home => ControlShape::Circle(c(160.0, 89.5, 6.5)),
            Control::DUp => ControlShape::Rect(r(97.0, 103.0, 14.0, 14.0), theme::rounded::XS),
            Control::DDown => ControlShape::Rect(r(97.0, 127.0, 14.0, 14.0), theme::rounded::XS),
            Control::DLeft => ControlShape::Rect(r(85.0, 115.0, 14.0, 14.0), theme::rounded::XS),
            Control::DRight => ControlShape::Rect(r(109.0, 115.0, 14.0, 14.0), theme::rounded::XS),
            Control::X => ControlShape::Circle(c(212.0, 70.0, 10.0)),
            Control::Y => ControlShape::Circle(c(194.0, 88.0, 10.0)),
            Control::A => ControlShape::Circle(c(230.0, 88.0, 10.0)),
            Control::B => ControlShape::Circle(c(212.0, 106.0, 10.0)),
        };
        let chip = |x: f32, y: f32, w: f32| r(x, y, w, theme::components::gamepad_key_chip::SIZE);
        Self {
            origin,
            controls: Control::ALL.iter().map(|&ctl| (ctl, shape(ctl))).collect(),
            wells: [c(70.0, 88.0, 17.0), c(176.0, 122.0, 17.0)],
            knob_radius: 11.0,
            dpad_well: c(104.0, 122.0, 22.0),
            dpad_centre: r(97.0, 115.0, 14.0, 14.0),
            abxy_well: c(212.0, 88.0, 31.0),
            chips: vec![
                (chip(11.5, 9.0, 13.0), ChipContent::Text("Z")),
                (chip(23.5, 29.0, 13.0), ChipContent::Text("Q")),
                (chip(243.5, 29.0, 13.0), ChipContent::Text("E")),
                (chip(255.5, 9.0, 13.0), ChipContent::Text("C")),
                (chip(205.5, 45.0, 13.0), ChipContent::Text("K")),
                (chip(170.5, 83.0, 13.0), ChipContent::Text("H")),
                (chip(240.5, 83.0, 13.0), ChipContent::Text("L")),
                (chip(205.5, 117.0, 13.0), ChipContent::Text("J")),
                (
                    chip(90.5, 47.0, 15.0),
                    ChipContent::Symbol(Symbol::Backspace),
                ),
                (chip(175.5, 47.0, 15.0), ChipContent::Symbol(Symbol::Return)),
                (chip(66.0, 151.0, 76.0), ChipContent::Text(CHIP_DPAD)),
            ],
            mouse_labels: [p(70.0, 114.5), p(176.0, 148.5)],
        }
    }

    /// The shape of a control.
    #[must_use]
    pub fn control(&self, c: Control) -> ControlShape {
        self.controls
            .iter()
            .find(|(k, _)| *k == c)
            .map_or(ControlShape::Rect(Rect::NOTHING, 0.0), |(_, s)| *s)
    }

    /// A stick's well.
    #[must_use]
    pub fn well(&self, s: Stick) -> Circle {
        match s {
            Stick::Left => self.wells[0],
            Stick::Right => self.wells[1],
        }
    }

    /// What a press at `pos` presses: the control or stick well whose shape
    /// contains it (circles by distance, AC 4), if any.
    #[must_use]
    pub fn hit(&self, pos: Pos2) -> Option<Target> {
        if let Some((c, _)) = self.controls.iter().find(|(_, s)| s.contains(pos)) {
            return Some(Target::Control(*c));
        }
        Stick::ALL
            .into_iter()
            .find(|s| self.well(*s).contains(pos))
            .map(Target::Stick)
    }

    /// The body outline, flattened (closed; the first point is not
    /// repeated).
    #[must_use]
    pub fn body_outline(&self) -> Vec<Pos2> {
        let p = |(x, y): (f32, f32)| self.origin + vec2(x, y);
        let mut pts = vec![p(BODY_START)];
        let mut cur = p(BODY_START);
        for (ctrl, end) in BODY_SEGMENTS {
            let end = p(end);
            match ctrl {
                Some(c) => pts.extend(
                    paint::quad_points(cur, p(c), end, BODY_QUAD_SEGMENTS)
                        .into_iter()
                        .skip(1),
                ),
                None => pts.push(end),
            }
            cur = end;
        }
        pts.pop();
        pts
    }

    /// The body fill's fan centre.
    #[must_use]
    pub fn body_fan_centre(&self) -> Pos2 {
        self.origin + vec2(BODY_FAN_CENTRE.0, BODY_FAN_CENTRE.1)
    }
}

/// What the panel shows this frame.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PadView {
    /// The gate is open: hover and pressed tints react.
    pub enabled: bool,
    /// The inline hint under the legend (the gate's reason), if blocked.
    pub hint: Option<&'static str>,
    /// The control held (drawn pressed).
    pub held: Option<Control>,
    /// The stick being dragged (its knob drawn pressed).
    pub dragging: Option<Stick>,
    /// The stick values (the knobs' positions).
    pub sticks: [(u8, u8); 2],
}

impl Default for PadView {
    fn default() -> Self {
        Self {
            enabled: true,
            hint: None,
            held: None,
            dragging: None,
            sticks: [(128, 128); 2],
        }
    }
}

/// What the pointer did on the pad this frame (read in the frame's first
/// pass only; the app ignores it in other passes).
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct PadInput {
    /// The primary button was pressed on this target this frame.
    pub press: Option<Target>,
    /// For a press on a stick: the press point's offset from its well
    /// centre.
    pub press_offset: (f32, f32),
    /// The primary button came up this frame.
    pub released: bool,
    /// The primary button is down at the end of the frame's input.
    pub primary_down: bool,
    /// While a stick is dragged: the pointer's offset from its well centre.
    pub drag_offset: Option<(f32, f32)>,
    /// An AccessKit `Click` on a control (a click the pointer did not make).
    pub tap: Option<Control>,
}

/// Draws the panel in the gamepad `region` and senses the pointer (AC 1–5,
/// 11, 13–16).
pub fn show(ui: &mut Ui, region: Rect, view: &PadView) -> PadInput {
    let ctx = ui.ctx().clone();
    let ppp = ctx.pixels_per_point();
    let lay = layout(region, TextHeights::measure(&ctx));
    paint_header(ui, &lay);
    let geo = PadGeometry::new(lay.pad.min);
    paint_body(ui, &geo, ppp);

    // Interaction first (the tints need the hover target).
    let mut responses: Vec<(Target, Response)> = Vec::with_capacity(18);
    for (c, s) in &geo.controls {
        let resp = ui.interact(s.rect(), control_id(*c), Sense::CLICK | Sense::DRAG);
        let name = c.a11y_name();
        resp.widget_info(|| WidgetInfo::labeled(Role::Button, true, name));
        responses.push((Target::Control(*c), resp));
    }
    for s in Stick::ALL {
        let resp = ui.interact(geo.well(s).rect(), stick_id(s), Sense::DRAG);
        let name = s.a11y_name();
        resp.widget_info(|| WidgetInfo::labeled(Role::Slider, true, name));
        responses.push((Target::Stick(s), resp));
    }
    let (pressed, origin, released, down, hover, interact) = ui.input(|i| {
        (
            i.pointer.primary_pressed(),
            // The press event's own position: `press_origin` is cleared
            // when the release comes in the same frame (a quick click).
            i.events.iter().find_map(|e| match e {
                egui::Event::PointerButton {
                    pos,
                    button: egui::PointerButton::Primary,
                    pressed: true,
                    ..
                } => Some(*pos),
                _ => None,
            }),
            i.pointer.primary_released(),
            i.pointer.primary_down(),
            i.pointer.hover_pos(),
            i.pointer.interact_pos(),
        )
    });
    let on_pad = |f: fn(&Response) -> bool| responses.iter().any(|(_, r)| f(r));
    let mut input = PadInput {
        released,
        primary_down: down,
        ..PadInput::default()
    };
    // The press reached the pad (no layer above it took it): egui marks
    // the widget pressed, or (a press and release in one frame) clicked;
    // the pad's own hit test then picks the control (circles by distance).
    if pressed
        && (on_pad(Response::is_pointer_button_down_on)
            || on_pad(Response::clicked)
            || on_pad(Response::contains_pointer))
        && let Some(o) = origin
        && let Some(t) = geo.hit(o)
    {
        input.press = Some(t);
        if let Target::Stick(s) = t {
            let d = o - geo.well(s).center;
            input.press_offset = (d.x, d.y);
        }
    }
    if let Some(s) = view.dragging
        && let Some(p) = interact
    {
        let d = p - geo.well(s).center;
        input.drag_offset = Some((d.x, d.y));
    }
    input.tap = responses.iter().find_map(|(t, r)| match t {
        Target::Control(c) if r.clicked() && !released => Some(*c),
        _ => None,
    });
    let hovered = if view.enabled && on_pad(Response::hovered) {
        hover.and_then(|p| geo.hit(p))
    } else {
        None
    };
    let state = |t: Target| {
        let held = match t {
            Target::Control(c) => view.held == Some(c),
            Target::Stick(s) => view.dragging == Some(s),
        };
        if !view.enabled {
            ButtonState::Idle
        } else if held {
            ButtonState::Pressed
        } else if hovered == Some(t) && view.held.is_none() && view.dragging.is_none() {
            ButtonState::Hovered
        } else {
            ButtonState::Idle
        }
    };
    paint_controls(ui, &geo, view, &state, ppp);
    paint_legend(ui, &lay);
    if let Some(text) = view.hint {
        let _ = widgets::inline_hint(ui, lay.hint, text);
    }
    input
}

fn paint_header(ui: &mut Ui, lay: &PanelLayout) {
    let ctx = ui.ctx().clone();
    let ppp = ctx.pixels_per_point();
    let painter = ui.painter();
    let r = theme::rounded::SM.round().clamp(0.0, 255.0) as u8;
    painter.rect_filled(
        lay.tab,
        CornerRadius {
            nw: r,
            ne: r,
            sw: 0,
            se: 0,
        },
        theme::PARCHMENT,
    );
    painter.line_segment(
        [
            pos2(lay.underline_x.0, lay.underline_y),
            pos2(lay.underline_x.1, lay.underline_y),
        ],
        paint::stroke(1.0, theme::RIM, ppp),
    );
    let tab = paint::layout_line(&ctx, &theme::PANEL_TAB, TAB, theme::INK);
    let text_rect = Rect::from_min_max(
        pos2(lay.tab.left(), lay.tab.top() + TAB_PADDING_TOP),
        lay.tab.max,
    );
    let _ = widgets::text_node_colored(ui, text_rect, tab, Align2::CENTER_TOP, theme::INK);
    let caption = paint::layout_line(&ctx, &theme::SMALL, CAPTION, theme::TEXT_MUTED);
    let _ = widgets::text_node_colored(
        ui,
        lay.caption,
        caption,
        Align2::LEFT_TOP,
        theme::TEXT_MUTED,
    );
}

fn paint_body(ui: &Ui, geo: &PadGeometry, ppp: f32) {
    use theme::components::gamepad_body::{BORDER, BORDER_COLOR, FILL};
    let painter = ui.painter();
    let outline = geo.body_outline();
    // The fill: a fan of triangles (each convex) from a point the outline
    // is star-shaped from, as one mesh, so no anti-aliasing seam shows
    // between the pieces; the outline stroke covers the mesh's edge.
    let mut mesh = Mesh::default();
    mesh.colored_vertex(geo.body_fan_centre(), FILL);
    for p in &outline {
        mesh.colored_vertex(*p, FILL);
    }
    let n = u32::try_from(outline.len()).unwrap_or(0);
    for i in 0..n {
        mesh.add_triangle(0, 1 + i, 1 + (i + 1) % n);
    }
    painter.add(mesh);
    painter.add(PathShape::closed_line(
        outline,
        paint::stroke(BORDER, BORDER_COLOR, ppp),
    ));
}

fn paint_controls(
    ui: &Ui,
    geo: &PadGeometry,
    view: &PadView,
    state: &dyn Fn(Target) -> ButtonState,
    ppp: f32,
) {
    use theme::components::gamepad_control::{BORDER, BORDER_COLOR, FILL, FOREGROUND, WELL};
    let ctx = ui.ctx().clone();
    let painter = ui.painter();
    let border = |s: ButtonState| paint::stroke(BORDER, s.tint(BORDER_COLOR), ppp);
    // Wells under the controls.
    painter.circle_filled(geo.dpad_well.center, geo.dpad_well.radius, WELL);
    painter.circle_filled(geo.abxy_well.center, geo.abxy_well.radius, WELL);
    for (i, s) in Stick::ALL.into_iter().enumerate() {
        let well = geo.well(s);
        let st = state(Target::Stick(s));
        // The well's ring is the stick's border: tinted with its knob.
        painter.circle(well.center, well.radius, WELL, border(st));
        let (x, y) = view.sticks.get(i).copied().unwrap_or((128, 128));
        let knob = well.center + knob_offset(x, y);
        painter.circle(knob, geo.knob_radius, st.tint(FILL), border(st));
    }
    // The D-pad: the centre square, the four arms (each tinted on its own),
    // one outline around the cross, the arrows.
    painter.rect_filled(geo.dpad_centre, CornerRadius::ZERO, FILL);
    for (c, shape) in &geo.controls {
        let st = state(Target::Control(*c));
        match shape {
            ControlShape::Rect(r, radius) => {
                let cr = theme::corner(*radius);
                painter.rect_filled(*r, cr, st.tint(FILL));
                if !is_dpad(*c) {
                    painter.rect_stroke(*r, cr, border(st), StrokeKind::Middle);
                }
            }
            ControlShape::Circle(k) => {
                painter.circle(k.center, k.radius, st.tint(FILL), border(st));
            }
        }
    }
    // The cross outline is the D-pad arms' border: tinted with the arm
    // that is held (else hovered).
    let arms = [
        Control::DUp,
        Control::DDown,
        Control::DLeft,
        Control::DRight,
    ]
    .map(|c| state(Target::Control(c)));
    let cross = if arms.contains(&ButtonState::Pressed) {
        ButtonState::Pressed
    } else if arms.contains(&ButtonState::Hovered) {
        ButtonState::Hovered
    } else {
        ButtonState::Idle
    };
    painter.add(PathShape::closed_line(dpad_outline(geo), border(cross)));
    for (c, sym) in [
        (Control::DUp, Symbol::TriangleUp),
        (Control::DDown, Symbol::ChevronDown),
        (Control::DLeft, Symbol::TriangleLeft),
        (Control::DRight, Symbol::TriangleRight),
    ] {
        let r = Rect::from_center_size(geo.control(c).rect().center(), vec2(6.0, 6.0));
        symbols::paint_symbol(painter, r, sym, FOREGROUND, ppp);
    }
    // Letters.
    for c in [
        Control::Zl,
        Control::L,
        Control::R,
        Control::Zr,
        Control::X,
        Control::Y,
        Control::A,
        Control::B,
    ] {
        let g = paint::layout_line(&ctx, &CONTROL_LETTER, c.a11y_name(), FOREGROUND);
        let rect = geo.control(c).rect();
        let pos = Align2::CENTER_CENTER
            .align_size_within_rect(g.rect.size(), rect)
            .min
            - g.rect.min.to_vec2();
        painter.galley(pos, g, FOREGROUND);
    }
    // − and + as bars, Capture's dot, Home's house.
    let bar = |center: Pos2, w: f32, h: f32| Rect::from_center_size(center, vec2(w, h));
    let minus = geo.control(Control::Minus).rect().center();
    painter.rect_filled(bar(minus, 6.0, 1.5), CornerRadius::ZERO, FOREGROUND);
    let plus = geo.control(Control::Plus).rect().center();
    painter.rect_filled(bar(plus, 6.0, 1.5), CornerRadius::ZERO, FOREGROUND);
    painter.rect_filled(bar(plus, 1.5, 6.0), CornerRadius::ZERO, FOREGROUND);
    painter.circle_filled(
        geo.control(Control::Capture).rect().center(),
        2.6,
        FOREGROUND,
    );
    let o = geo.origin;
    let house = [
        (156.5, 90.5),
        (160.0, 87.0),
        (163.5, 90.5),
        (163.5, 93.0),
        (156.5, 93.0),
    ]
    .map(|(x, y)| o + vec2(x, y))
    .to_vec();
    let _ = paint::fill_convex(painter, house, FOREGROUND, egui::Stroke::NONE);
    paint_chips(ui, geo, ppp);
}

fn is_dpad(c: Control) -> bool {
    matches!(
        c,
        Control::DUp | Control::DDown | Control::DLeft | Control::DRight
    )
}

/// The cross outline of the D-pad (arm rects and the centre square).
#[must_use]
pub fn dpad_outline(geo: &PadGeometry) -> Vec<Pos2> {
    let o = geo.origin;
    [
        (97.0, 103.0),
        (111.0, 103.0),
        (111.0, 115.0),
        (123.0, 115.0),
        (123.0, 129.0),
        (111.0, 129.0),
        (111.0, 141.0),
        (97.0, 141.0),
        (97.0, 129.0),
        (85.0, 129.0),
        (85.0, 115.0),
        (97.0, 115.0),
    ]
    .map(|(x, y)| o + vec2(x, y))
    .to_vec()
}

/// The knob's offset from its well centre for a stick value: the deflection
/// scaled to [`KNOB_TRAVEL`].
#[must_use]
pub fn knob_offset(x: u8, y: u8) -> egui::Vec2 {
    let t = |v: u8| {
        let d = f32::from(v) - 128.0;
        if d >= 0.0 { d / 127.0 } else { d / 128.0 }
    };
    vec2(t(x), t(y)) * KNOB_TRAVEL
}

fn paint_chips(ui: &Ui, geo: &PadGeometry, ppp: f32) {
    use theme::components::gamepad_key_chip::{BORDER, BORDER_COLOR, FILL, FOREGROUND, RADIUS};
    let ctx = ui.ctx().clone();
    let painter = ui.painter();
    for (rect, content) in &geo.chips {
        let cr = theme::corner(RADIUS);
        painter.rect_filled(*rect, cr, FILL);
        painter.rect_stroke(
            *rect,
            cr,
            paint::stroke(BORDER, BORDER_COLOR, ppp),
            StrokeKind::Inside,
        );
        match content {
            ChipContent::Text(t) => {
                let g = paint::layout_line(&ctx, &theme::KEY_CHIP, t, FOREGROUND);
                let pos = Align2::CENTER_CENTER
                    .align_size_within_rect(g.rect.size(), *rect)
                    .min
                    - g.rect.min.to_vec2();
                painter.galley(pos, g, FOREGROUND);
            }
            ChipContent::Symbol(s) => {
                symbols::paint_symbol(painter, rect.shrink2(vec2(3.0, 2.5)), *s, FOREGROUND, ppp);
            }
        }
    }
    for anchor in geo.mouse_labels {
        let g = paint::layout_line(&ctx, &theme::KEY_CHIP, MOUSE, theme::TEXT_MUTED);
        let pos = anchor - g.rect.size() / 2.0 - g.rect.min.to_vec2();
        painter.galley(pos, g, theme::TEXT_MUTED);
    }
}

/// The text of a legend row's node: "name: keys".
#[must_use]
pub fn legend_row_label(name: &str, keys: &str) -> String {
    format!("{name}: {keys}")
}

fn paint_legend(ui: &mut Ui, lay: &PanelLayout) {
    use theme::components::gamepad_legend::{
        DIVIDER, DIVIDER_WIDTH, KEYS, NAME, ROW_PADDING_X, ROW_PADDING_Y,
    };
    let ctx = ui.ctx().clone();
    let ppp = ctx.pixels_per_point();
    for (row, (name, keys)) in lay.legend_rows.iter().zip(LEGEND) {
        let resp = ui.allocate_rect(*row, Sense::hover());
        let label = legend_row_label(name, keys);
        resp.widget_info(|| WidgetInfo::labeled(Role::Label, true, &label));
        let painter = ui.painter();
        let text = Rect::from_min_max(
            pos2(row.left() + ROW_PADDING_X, row.top() + ROW_PADDING_Y),
            pos2(
                row.right() - ROW_PADDING_X,
                row.bottom() - ROW_PADDING_Y - DIVIDER_WIDTH,
            ),
        );
        let n = paint::layout_line(&ctx, &theme::LEGEND_NAME, name, NAME);
        let pos = Align2::LEFT_CENTER
            .align_size_within_rect(n.rect.size(), text)
            .min
            - n.rect.min.to_vec2();
        painter.galley(pos, n, NAME);
        let k = paint::layout(&ctx, &theme::SMALL, keys, KEYS, f32::INFINITY, Align::Min);
        let pos = Align2::RIGHT_CENTER
            .align_size_within_rect(k.rect.size(), text)
            .min
            - k.rect.min.to_vec2();
        painter.galley(pos, k, KEYS);
        let y = row.bottom() - DIVIDER_WIDTH / 2.0;
        painter.line_segment(
            [pos2(row.left(), y), pos2(row.right(), y)],
            paint::stroke(DIVIDER_WIDTH, DIVIDER, ppp),
        );
    }
}

/// The pad model's view for this frame (the app passes it to [`show`]).
#[must_use]
pub fn view_of(pad: &gamepad::Pad, enabled: bool, hint: Option<&'static str>) -> PadView {
    PadView {
        enabled,
        hint,
        held: pad.held(),
        dragging: pad.dragging(),
        sticks: [pad.stick_value(Stick::Left), pad.stick_value(Stick::Right)],
    }
}
