//! Bonooru desktop app: Rust + egui feasibility spike (throwaway).
//!
//! Env vars for unattended measurement (the spike author could not see the window):
//! - `SPIKE_EXIT_AFTER=<secs>`: close the window after N seconds (0 = after first frame).
//! - `SPIKE_AUTOPLAY=1`: open the piano roll tab and scroll/zoom it continuously.
//! - `SPIKE_AUDIO=1`: with autoplay, also start the audio preview.

mod app_core;
mod ui;

use std::time::Instant;

fn main() -> eframe::Result {
    let t0 = Instant::now();
    if std::env::args().any(|a| a == "--list") {
        // Headless check: enumerate only (no port is opened, nothing is written).
        for p in app_core::discovery::list_ports().unwrap_or_default() {
            let m = app_core::discovery::classify(&p, "Bonooru Bridge");
            println!("{:?}\t{}\t{:?}", m, p.name, p.usb);
        }
        for v in app_core::uf2::find_boot_volumes(None) {
            println!("boot volume: {v:?}");
        }
        println!("mounted volumes: {:?}", app_core::uf2::mounted_volumes());
        return Ok(());
    }
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([1100.0, 720.0])
            .with_title("Bonooru – egui spike"),
        ..Default::default()
    };
    eframe::run_native(
        "ocarina-spike",
        options,
        Box::new(move |cc| Ok(Box::new(ui::SpikeApp::new(cc, t0)))),
    )
}
