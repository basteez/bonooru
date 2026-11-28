//! G.27 (Story 5.1 AC 10): the app and its mapping profile. Test
//! constructors use the bundled default with no file access and log
//! nothing; `.profile(resolved)` writes the `profile` lines to the app's
//! log; a refused user copy leaves the app running on the bundled default;
//! `App::new` loads through `profiles::load()` (pinned by a source check:
//! a test never touches the real config dir). No worker, no clock.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing
)]

use std::path::Path;

use egui::vec2;
use egui_kittest::Harness;
use ocb_app::app::{App, Mode};
use ocb_core::profile::{self, Source, UserFile};
use ocb_services::log::{Logger, MemorySink};
use ocb_services::profiles::{self, BUNDLED};

/// The text after ` profile ` of each `profile` line.
fn profile_lines(sink: &MemorySink) -> Vec<String> {
    sink.lines()
        .iter()
        .filter_map(|l| l.split_once(" profile ").map(|(_, t)| t.to_owned()))
        .collect()
}

#[test]
fn g27_test_constructor_uses_the_bundled_default() {
    let sink = MemorySink::new();
    let logger = Logger::spawn(Box::new(sink.clone()), false, None);
    let log = logger.handle();
    let mut h = Harness::builder()
        .with_size(vec2(1280.0, 800.0))
        .build_eframe(move |cc| App::without_link(cc, Mode::Main).led_log(log.clone()));
    h.step();
    assert_eq!(h.state().mapping_profile().name, "OoT Switch 2");
    assert_eq!(h.state().profile_source(), &Source::Bundled);
    drop(h);
    assert!(logger.stop());
    assert_eq!(profile_lines(&sink), Vec::<String>::new());
}

#[test]
fn g27_profile_builder_logs_and_keeps_running() {
    let sink = MemorySink::new();
    let logger = Logger::spawn(Box::new(sink.clone()), false, None);
    let log = logger.handle();
    let bad = BUNDLED.replacen("schema = 1", "schema = 7", 1);
    let resolved = profile::resolve(
        BUNDLED,
        &[UserFile {
            name: "mine.toml".to_owned(),
            text: Ok(bad),
        }],
    )
    .unwrap();
    let mut h = Harness::builder()
        .with_size(vec2(1280.0, 800.0))
        .build_eframe(move |cc| {
            App::without_link(cc, Mode::Main)
                .led_log(log.clone())
                .profile(resolved.clone())
        });
    for _ in 0..5 {
        h.step();
    }
    assert_eq!(h.state().mapping_profile().timing.min_press_frames, 3);
    assert_eq!(h.state().profile_source(), &Source::Bundled);
    drop(h);
    assert!(logger.stop());
    assert_eq!(
        profile_lines(&sink),
        vec![
            "refused file=mine.toml reason=schema detail=\"schema 7, this version reads 1\""
                .to_owned(),
            "loaded name=\"OoT Switch 2\" game_version=1.1.1 schema=1 source=bundled file=-"
                .to_owned(),
        ]
    );
    // The same lines as the adapter's formatter, once each.
    assert_eq!(profile_lines(&sink).len(), 2);
}

#[test]
fn g27_new_loads_through_the_adapter() {
    let app = Path::new(env!("CARGO_MANIFEST_DIR")).join("src/app.rs");
    let code: String = std::fs::read_to_string(app)
        .unwrap()
        .chars()
        .filter(|c| !c.is_whitespace())
        .collect();
    assert_eq!(
        code.matches("profiles::load()").count(),
        1,
        "App::new must call profiles::load()"
    );
    assert_eq!(
        profiles::log_lines(&profiles::bundled_only()),
        vec![
            "loaded name=\"OoT Switch 2\" game_version=1.1.1 schema=1 source=bundled file=-"
                .to_owned()
        ]
    );
}
