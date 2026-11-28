//! Shared helpers of the UI tests (Story 4.3): flattened frame shapes and
//! node queries. Not every test file uses every helper.

#![allow(dead_code, clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use egui::epaint::{ClippedShape, Shape};
use egui::{Color32, Pos2, Rect, pos2, vec2};
use ocb_app::ui::theme;

/// One painted shape: its index in paint order, the clip rect of its
/// `ClippedShape`, and the shape (nested `Shape::Vec`s walked).
#[derive(Clone, Debug)]
pub struct Flat {
    pub index: usize,
    pub clip: Rect,
    pub shape: Shape,
}

/// Every shape of the frame in paint order, `Shape::Vec` flattened.
pub fn shapes_flat(shapes: &[ClippedShape]) -> Vec<Flat> {
    fn walk(shape: &Shape, clip: Rect, out: &mut Vec<Flat>) {
        match shape {
            Shape::Vec(v) => v.iter().for_each(|s| walk(s, clip, out)),
            other => out.push(Flat {
                index: out.len(),
                clip,
                shape: other.clone(),
            }),
        }
    }
    let mut out = Vec::new();
    for c in shapes {
        walk(&c.shape, c.clip_rect, &mut out);
    }
    out
}

/// The stroke colour of a shape that has one.
pub fn stroke_color(shape: &Shape) -> Option<Color32> {
    match shape {
        Shape::LineSegment { stroke, .. } => Some(stroke.color),
        Shape::Path(p) => match &p.stroke.color {
            egui::epaint::ColorMode::Solid(c) => Some(*c),
            egui::epaint::ColorMode::UV(_) => None,
        },
        Shape::QuadraticBezier(q) => match &q.stroke.color {
            egui::epaint::ColorMode::Solid(c) => Some(*c),
            egui::epaint::ColorMode::UV(_) => None,
        },
        Shape::Rect(r) => Some(r.stroke.color),
        Shape::Circle(c) => Some(c.stroke.color),
        _ => None,
    }
}

/// The stroke width of a shape that has one (0 for none).
pub fn stroke_width(shape: &Shape) -> Option<f32> {
    match shape {
        Shape::LineSegment { stroke, .. } => Some(stroke.width),
        Shape::Path(p) => Some(p.stroke.width),
        Shape::QuadraticBezier(q) => Some(q.stroke.width),
        Shape::CubicBezier(q) => Some(q.stroke.width),
        Shape::Rect(r) => Some(r.stroke.width),
        Shape::Circle(c) => Some(c.stroke.width),
        Shape::Ellipse(e) => Some(e.stroke.width),
        _ => None,
    }
}

/// Whether a shape uses `color` as fill or stroke.
pub fn uses(shape: &Shape, color: Color32) -> bool {
    let fill = match shape {
        Shape::Path(p) => Some(p.fill),
        Shape::QuadraticBezier(q) => Some(q.fill),
        Shape::Rect(r) => Some(r.fill),
        Shape::Circle(c) => Some(c.fill),
        _ => None,
    };
    fill == Some(color) || stroke_color(shape) == Some(color)
}

/// Every text shape: (text, ink rect on screen, shape index).
pub fn texts(flat: &[Flat]) -> Vec<(String, Rect, usize)> {
    flat.iter()
        .filter_map(|f| match &f.shape {
            Shape::Text(t) => {
                let ink = t.galley.rect.union(t.galley.mesh_bounds);
                Some((
                    t.galley.text().to_owned(),
                    ink.translate(t.pos.to_vec2()),
                    f.index,
                ))
            }
            _ => None,
        })
        .collect()
}

/// `a` ⊆ `b` within `eps`.
pub fn inside(a: Rect, b: Rect, eps: f32) -> bool {
    a.min.x >= b.min.x - eps
        && a.min.y >= b.min.y - eps
        && a.max.x <= b.max.x + eps
        && a.max.y <= b.max.y + eps
}

/// Two rects equal within `eps`.
pub fn rect_eq(a: Rect, b: Rect, eps: f32) -> bool {
    (a.min - b.min).length() <= eps && (a.max - b.max).length() <= eps
}

/// All `.rs` files under `dir`.
pub fn rs_files(dir: &std::path::Path, out: &mut Vec<std::path::PathBuf>) {
    for entry in std::fs::read_dir(dir).unwrap() {
        let path = entry.unwrap().path();
        if path.is_dir() {
            rs_files(&path, out);
        } else if path.extension().is_some_and(|e| e == "rs") {
            out.push(path);
        }
    }
}

/// Rust source reduced to code: `//` and (nested) `/* */` comments removed,
/// string, raw-string and char literal contents blanked (the quotes stay).
/// So a `//` inside a string does not cut the line, and a pattern written in
/// a comment or a string (a scan's own self-test snippets) is not code.
pub fn code_only(source: &str) -> String {
    let c: Vec<char> = source.chars().collect();
    let mut out = String::with_capacity(source.len());
    let mut i = 0;
    let at = |k: usize| c.get(k).copied().unwrap_or('\0');
    while i < c.len() {
        let ch = c[i];
        if ch == '/' && at(i + 1) == '/' {
            while i < c.len() && c[i] != '\n' {
                i += 1;
            }
        } else if ch == '/' && at(i + 1) == '*' {
            let mut depth = 0;
            while i < c.len() {
                if c[i] == '/' && at(i + 1) == '*' {
                    depth += 1;
                    i += 2;
                } else if c[i] == '*' && at(i + 1) == '/' {
                    depth -= 1;
                    i += 2;
                    if depth == 0 {
                        break;
                    }
                } else {
                    i += 1;
                }
            }
            out.push(' ');
        } else if ch == 'r'
            && (at(i + 1) == '"' || (at(i + 1) == '#' && (at(i + 2) == '"' || at(i + 2) == '#')))
            && !at(i.wrapping_sub(1)).is_alphanumeric()
            && at(i.wrapping_sub(1)) != '_'
        {
            // Raw string r"…" / r#"…"#.
            let mut k = i + 1;
            let mut hashes = 0;
            while at(k) == '#' {
                hashes += 1;
                k += 1;
            }
            out.push_str("r\"");
            k += 1;
            loop {
                if k >= c.len() {
                    break;
                }
                if c[k] == '"' && (0..hashes).all(|h| at(k + 1 + h) == '#') {
                    k += 1 + hashes;
                    break;
                }
                k += 1;
            }
            out.push('"');
            i = k;
        } else if ch == '"' {
            out.push('"');
            i += 1;
            while i < c.len() && c[i] != '"' {
                if c[i] == '\\' {
                    i += 1;
                }
                i += 1;
            }
            out.push('"');
            i += 1;
        } else if ch == '\'' && (at(i + 2) == '\'' || (at(i + 1) == '\\' && at(i + 1) != '\0')) {
            // A char literal ('x' or '\n', '\u{…}'); a lifetime has no
            // closing quote and falls through.
            let mut k = i + 1;
            if at(k) == '\\' {
                k += 2;
                while k < c.len() && c[k] != '\'' {
                    k += 1;
                }
            } else {
                k += 1;
            }
            out.push_str("' '");
            i = k + 1;
        } else {
            out.push(ch);
            i += 1;
        }
    }
    out
}

/// The four bracket boxes of `rect` (TL, TR, BL, BR) and their mirror signs.
pub fn bracket_boxes(rect: Rect) -> [(Rect, f32, f32); 4] {
    let s = theme::spacing::CORNER_BRACKET_SIZE;
    let i = theme::components::CORNER_BRACKET_INSET;
    [
        (
            Rect::from_min_size(rect.min + vec2(i, i), vec2(s, s)),
            1.0,
            1.0,
        ),
        (
            Rect::from_min_size(pos2(rect.right() - i - s, rect.top() + i), vec2(s, s)),
            -1.0,
            1.0,
        ),
        (
            Rect::from_min_size(pos2(rect.left() + i, rect.bottom() - i - s), vec2(s, s)),
            1.0,
            -1.0,
        ),
        (
            Rect::from_min_size(
                pos2(rect.right() - i - s, rect.bottom() - i - s),
                vec2(s, s),
            ),
            -1.0,
            -1.0,
        ),
    ]
}

/// Four corner sets, each 1 outer path (2 pt), 1 quadratic (1.2 pt) and 1
/// dot (r 1.6) inside its box; the sets are mirror images of the top-left.
pub fn check_brackets(flat: &[Flat], rect: Rect, ctx: &str) {
    let boxes = bracket_boxes(rect);
    // Per corner: the outer path's points, the quadratic's three points and
    // the dot centre, in that order (the mirror check covers all three).
    let mut sets: Vec<Vec<Pos2>> = vec![Vec::new(); 4];
    for (k, (bx, _, _)) in boxes.iter().enumerate() {
        let mine: Vec<&Flat> = flat
            .iter()
            .filter(|f| inside(f.shape.visual_bounding_rect(), *bx, 0.01))
            .collect();
        let paths: Vec<_> = mine
            .iter()
            .filter_map(|f| match &f.shape {
                Shape::Path(p) if (p.stroke.width - 2.0).abs() < 1e-4 => Some(p.points.clone()),
                _ => None,
            })
            .collect();
        let quads: Vec<_> = mine
            .iter()
            .filter_map(|f| match &f.shape {
                Shape::QuadraticBezier(q) if (q.stroke.width - 1.2).abs() < 1e-4 => {
                    Some(q.points.to_vec())
                }
                _ => None,
            })
            .collect();
        let dots: Vec<_> = mine
            .iter()
            .filter_map(|f| match &f.shape {
                Shape::Circle(c) if (c.radius - 1.6).abs() < 1e-4 => Some(c.center),
                _ => None,
            })
            .collect();
        assert_eq!(
            (paths.len(), quads.len(), dots.len()),
            (1, 1, 1),
            "{ctx}: corner {k} in {bx:?}"
        );
        sets[k] = paths[0].clone();
        sets[k].extend(quads[0].iter().copied());
        sets[k].push(dots[0]);
    }
    assert_eq!(
        flat.len(),
        12,
        "{ctx}: 4 × 3 shapes, nothing outside the boxes"
    );
    // Mirror images: corner k's points, mapped back to the top-left box.
    let tl = boxes[0].0;
    for (k, (bx, sx, sy)) in boxes.iter().enumerate() {
        assert_eq!(
            sets[k].len(),
            sets[0].len(),
            "{ctx}: corner {k} point count"
        );
        for (i, (p, q)) in sets[k].iter().zip(&sets[0]).enumerate() {
            let local_x = if *sx > 0.0 {
                p.x - bx.left()
            } else {
                bx.right() - p.x
            };
            let local_y = if *sy > 0.0 {
                p.y - bx.top()
            } else {
                bx.bottom() - p.y
            };
            assert!(
                (local_x - (q.x - tl.left())).abs() < 1e-3
                    && (local_y - (q.y - tl.top())).abs() < 1e-3,
                "{ctx}: corner {k} point {i} is not a mirror image"
            );
        }
    }
}

/// A serial worker on a fake backend with no ports and no log (Story 4.4:
/// tests never start the real backend or write the real log). Its thread
/// ends when the app (and so the handle) is dropped.
pub fn fake_link(ctx: &egui::Context) -> ocb_services::worker::WorkerHandle {
    use ocb_services::log::LogHandle;
    use ocb_services::sim::FakeBackend;
    use ocb_services::worker::{FakeClock, RealClock, host_rules, spawn_with};
    let ctx = ctx.clone();
    spawn_with(
        FakeBackend::new(FakeClock::at(0)),
        RealClock::new(),
        host_rules(),
        LogHandle::disabled(),
        Box::new(move || ctx.request_repaint()),
    )
    .expect("worker thread")
}

/// A UI clock moved by hand (Story 4.5): clones share the time.
#[derive(Clone, Debug, Default)]
pub struct TestClock(std::sync::Arc<std::sync::atomic::AtomicU64>);

impl TestClock {
    /// The time, µs.
    pub fn now_us(&self) -> u64 {
        self.0.load(std::sync::atomic::Ordering::SeqCst)
    }

    /// Moves the time forward.
    pub fn advance_us(&self, d: u64) {
        self.0.fetch_add(d, std::sync::atomic::Ordering::SeqCst);
    }

    /// The clock for `App::ui_clock`.
    pub fn ui_clock(&self) -> ocb_app::app::UiClock {
        let c = self.clone();
        std::sync::Arc::new(move || std::time::Duration::from_micros(c.now_us()))
    }
}

/// How a test worker's waker reaches the UI.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Wake {
    /// Count the publishes only: no repaint request from the worker thread,
    /// so a test reading egui's repaint delay sees the app's own requests
    /// only (a wake landing before a frame makes egui report 0 for it).
    CountOnly,
    /// Count, then the app's own waker (`app::repaint_waker`).
    App,
}

/// A worker thread on `backend` and its fake `clock` (macOS rules, no log),
/// whose waker counts the snapshots it published (and, with [`Wake::App`],
/// asks `ctx` for a repaint the way the app does). Its fake ports never
/// spin the thread: reads with nothing to return sleep 1 ms of real time.
pub fn fake_worker(
    ctx: &egui::Context,
    backend: ocb_services::sim::FakeBackend,
    clock: ocb_services::worker::FakeClock,
    published: std::sync::Arc<std::sync::atomic::AtomicU64>,
    wake: Wake,
) -> ocb_services::worker::WorkerHandle {
    fake_worker_logged(
        ctx,
        backend,
        clock,
        published,
        wake,
        ocb_services::log::LogHandle::disabled(),
    )
}

/// [`fake_worker`] writing its own lines to `log` (Story 4.7: the worker's
/// `stop` lines and the app's land in one sink, in FIFO order).
pub fn fake_worker_logged(
    ctx: &egui::Context,
    backend: ocb_services::sim::FakeBackend,
    clock: ocb_services::worker::FakeClock,
    published: std::sync::Arc<std::sync::atomic::AtomicU64>,
    wake: Wake,
    log: ocb_services::log::LogHandle,
) -> ocb_services::worker::WorkerHandle {
    use ocb_services::worker::spawn_with;
    backend.state().read_sleep = Some(std::time::Duration::from_millis(1));
    let app_waker = (wake == Wake::App).then(|| ocb_app::app::repaint_waker(ctx));
    spawn_with(
        backend,
        clock,
        ocb_core::device::Rules::MACOS,
        log,
        Box::new(move || {
            published.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
            if let Some(w) = &app_waker {
                w();
            }
        }),
    )
    .expect("worker thread")
}

/// eframe's repaint scheduling, emulated on real time: egui calls the
/// request-repaint callback with a delay whenever anything (a frame, the
/// worker's waker) asks for a repaint; the next frame is due at the
/// earliest such time. A test that steps only when a frame is due catches
/// a missing repaint request (hand-stepping would hide it).
#[derive(Clone, Debug, Default)]
pub struct Scheduler(std::sync::Arc<std::sync::Mutex<Option<std::time::Instant>>>);

impl Scheduler {
    /// Installs the callback on `ctx`.
    pub fn install(ctx: &egui::Context) -> Self {
        let s = Self::default();
        let slot = std::sync::Arc::clone(&s.0);
        ctx.set_request_repaint_callback(move |info| {
            let at = std::time::Instant::now() + info.delay;
            let mut g = slot.lock().unwrap();
            if g.is_none_or(|t| at < t) {
                *g = Some(at);
            }
        });
        s
    }

    /// A frame is due now.
    pub fn due(&self) -> bool {
        self.0
            .lock()
            .unwrap()
            .is_some_and(|t| std::time::Instant::now() >= t)
    }

    /// Forgets every pending request.
    pub fn clear(&self) {
        *self.0.lock().unwrap() = None;
    }

    /// Runs frames only when due, until `cond` holds after a frame; panics
    /// after `limit` of real time.
    pub fn drive_until<S>(
        &self,
        h: &mut egui_kittest::Harness<'_, S>,
        limit: std::time::Duration,
        cond: impl Fn(&egui_kittest::Harness<'_, S>) -> bool,
    ) {
        let start = std::time::Instant::now();
        loop {
            if self.due() {
                self.clear();
                h.step();
                if cond(h) {
                    return;
                }
            } else {
                std::thread::sleep(std::time::Duration::from_millis(1));
            }
            assert!(start.elapsed() < limit, "no frame reached the condition");
        }
    }
}

/// One field pattern of a log line matcher (Story 4.6).
#[derive(Clone, Copy, Debug)]
pub enum Pat {
    /// One of these words.
    OneOf(&'static [&'static str]),
    /// `[a-z_]+`.
    Word,
    /// `\d+`.
    Digits,
    /// `\d+` or `-`.
    DigitsOrDash,
    /// `\d+\.\d+\.\d+`.
    Triple,
}

fn digits(v: &str) -> bool {
    !v.is_empty() && v.bytes().all(|b| b.is_ascii_digit())
}

impl Pat {
    fn matches(self, v: &str) -> bool {
        match self {
            Self::OneOf(words) => words.contains(&v),
            Self::Word => !v.is_empty() && v.bytes().all(|b| b.is_ascii_lowercase() || b == b'_'),
            Self::Digits => digits(v),
            Self::DigitsOrDash => v == "-" || digits(v),
            Self::Triple => {
                let parts: Vec<&str> = v.split('.').collect();
                parts.len() == 3 && parts.iter().all(|p| digits(p))
            }
        }
    }
}

/// Whether `line` is exactly `head` followed by ` key=value` fields in this
/// order, each value matching its pattern (a small stand-in for an anchored
/// regex; no regex crate in the workspace).
pub fn line_matches(line: &str, head: &str, fields: &[(&str, Pat)]) -> bool {
    let mut words = line.split(' ');
    if !head.is_empty() && words.next() != Some(head) {
        return false;
    }
    for (key, pat) in fields {
        let Some((k, v)) = words.next().and_then(|w| w.split_once('=')) else {
            return false;
        };
        if k != *key || !pat.matches(v) {
            return false;
        }
    }
    words.next().is_none()
}

const ON_OFF: Pat = Pat::OneOf(&["on", "off"]);
/// The six gate reasons (Story 4.8 AC 9) or `none`.
const BLOCK_OR_NONE: Pat = Pat::OneOf(&[
    "none",
    "worker",
    "no_bridge",
    "bridge_not_ready",
    "firmware_update",
    "controller_link",
    "switch_not_ready",
]);

/// Story 4.8 `gate` line (4.6 finding 14): `^gamepad=(on|off)
/// gamepad_reason=<r|none> play=(on|off) play_reason=<r|none>
/// controller_proto=(\d+|-) app_proto=(\d+|-)$`.
pub fn is_gate_line(line: &str) -> bool {
    line_matches(
        line,
        "",
        &[
            ("gamepad", ON_OFF),
            ("gamepad_reason", BLOCK_OR_NONE),
            ("play", ON_OFF),
            ("play_reason", BLOCK_OR_NONE),
            ("controller_proto", Pat::DigitsOrDash),
            ("app_proto", Pat::DigitsOrDash),
        ],
    )
}

/// Story 4.6 `dialog shown` line: `^shown id=firmware_update_needed
/// connected_us=\d+ controller_proto=\d+ app_proto=\d+ fw=\d+\.\d+\.\d+
/// bridge=\d+\.\d+\.\d+$`.
pub fn is_dialog_shown_line(line: &str) -> bool {
    line_matches(
        line,
        "shown",
        &[
            ("id", Pat::OneOf(&["firmware_update_needed"])),
            ("connected_us", Pat::Digits),
            ("controller_proto", Pat::Digits),
            ("app_proto", Pat::Digits),
            ("fw", Pat::Triple),
            ("bridge", Pat::Triple),
        ],
    )
}

/// Story 4.6 `dialog closed` line: `^closed id=firmware_update_needed
/// outcome=(primary|dismissed)$`.
pub fn is_dialog_closed_line(line: &str) -> bool {
    line_matches(
        line,
        "closed",
        &[
            ("id", Pat::OneOf(&["firmware_update_needed"])),
            ("outcome", Pat::OneOf(&["primary", "dismissed"])),
        ],
    )
}

/// The texts of the log lines of `kind` (the part after ` <kind> `).
pub fn kind_texts(lines: &[String], kind: &str) -> Vec<String> {
    let tag = format!(" {kind} ");
    lines
        .iter()
        .filter_map(|l| l.split_once(tag.as_str()).map(|(_, t)| t.to_owned()))
        .collect()
}

/// The one dialog frame of a frame's shapes (Story 4.3 G.12, reused by
/// G.18): filled `panel-raised`, 1 pt `rim` stroke, `rounded.lg`.
pub fn dialog_frame_rect(flat: &[Flat]) -> Rect {
    let frames: Vec<_> = flat
        .iter()
        .filter_map(|f| match &f.shape {
            Shape::Rect(r)
                if r.fill == theme::PANEL_RAISED
                    && r.stroke.color == theme::RIM
                    && r.stroke.width == 1.0 =>
            {
                Some(r.clone())
            }
            _ => None,
        })
        .collect();
    assert_eq!(frames.len(), 1, "one dialog frame: {frames:?}");
    assert_eq!(frames[0].corner_radius, theme::corner(theme::rounded::LG));
    frames[0].rect
}
