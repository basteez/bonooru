use crate::app_core::discovery::{self, Command, Event, Match, PortInfo};
use std::time::Duration;

pub struct DevicesTab {
    worker: discovery::WorkerHandle,
    ports: Vec<PortInfo>,
    filter: String,
    log: Vec<String>,
    open: Option<String>,
    rx_total: usize,
    rx_preview: String,
    events: usize,
}

impl DevicesTab {
    pub fn new(ctx: egui::Context) -> Self {
        let worker = discovery::spawn(Duration::from_secs(1), move || ctx.request_repaint());
        DevicesTab {
            worker,
            ports: vec![],
            filter: "Ocarina Bot Bridge".into(),
            log: vec![],
            open: None,
            rx_total: 0,
            rx_preview: String::new(),
            events: 0,
        }
    }

    pub fn poll(&mut self) {
        while let Ok(ev) = self.worker.rx.try_recv() {
            self.events += 1;
            match ev {
                Event::Ports(p) => {
                    self.log
                        .push(format!("port list changed: {} ports", p.len()));
                    self.ports = p;
                }
                Event::Opened(n) => {
                    self.log.push(format!("opened {n} read-only"));
                    self.open = Some(n);
                    self.rx_total = 0;
                }
                Event::Rx { total, preview } => {
                    self.rx_total = total;
                    self.rx_preview = preview;
                }
                Event::Closed(why) => {
                    self.log.push(format!("closed: {why}"));
                    self.open = None;
                }
                Event::Error(e) => self.log.push(format!("error: {e}")),
            }
        }
        if self.log.len() > 200 {
            self.log.drain(..100);
        }
    }

    pub fn ui(&mut self, ui: &mut egui::Ui) {
        ui.heading("Serial ports");
        ui.horizontal(|ui| {
            ui.label("Product filter:");
            ui.text_edit_singleline(&mut self.filter);
            if ui.button("Rescan now").clicked() {
                let _ = self.worker.tx.send(Command::Rescan);
            }
            ui.label(format!(
                "(worker auto-rescans every 1 s; {} events received)",
                self.events
            ));
        });
        ui.label(
            "Green = product string matches the filter. Yellow = RP2040 VID 0x2E8A. \
             'Open' is READ-ONLY at 115200 baud: this spike never writes and never does the 1200-baud touch.",
        );
        ui.separator();

        let mut to_open = None;
        egui::Grid::new("ports")
            .striped(true)
            .num_columns(7)
            .show(ui, |ui| {
                for h in [
                    "Port",
                    "VID:PID",
                    "Manufacturer",
                    "Product",
                    "Serial",
                    "Match",
                    "",
                ] {
                    ui.strong(h);
                }
                ui.end_row();
                for p in &self.ports {
                    let m = discovery::classify(p, &self.filter);
                    let color = match m {
                        Match::Product => egui::Color32::from_rgb(80, 200, 120),
                        Match::Rp2040 => egui::Color32::from_rgb(230, 190, 60),
                        Match::None => ui.visuals().text_color(),
                    };
                    ui.colored_label(color, &p.name);
                    match &p.usb {
                        Some(u) => {
                            ui.colored_label(color, format!("{:04x}:{:04x}", u.vid, u.pid));
                            ui.label(u.manufacturer.as_deref().unwrap_or("–"));
                            ui.label(u.product.as_deref().unwrap_or("–"));
                            ui.label(u.serial.as_deref().unwrap_or("–"));
                        }
                        None => {
                            ui.label("(not USB)");
                            ui.label("");
                            ui.label("");
                            ui.label("");
                        }
                    }
                    ui.label(match m {
                        Match::Product => "bridge",
                        Match::Rp2040 => "RP2040",
                        Match::None => "",
                    });
                    let is_open = self.open.as_deref() == Some(p.name.as_str());
                    if is_open {
                        if ui.button("Close").clicked() {
                            let _ = self.worker.tx.send(Command::Close);
                        }
                    } else if ui.button("Open (read-only)").clicked() {
                        to_open = Some(p.name.clone());
                    }
                    ui.end_row();
                }
            });
        if let Some(n) = to_open {
            let _ = self.worker.tx.send(Command::Open(n));
        }

        ui.separator();
        match &self.open {
            Some(n) => ui.label(format!(
                "Reading {n}: {} bytes so far, last chunk: {}",
                self.rx_total, self.rx_preview
            )),
            None => ui.label("No port open."),
        };
        ui.separator();
        ui.label("Worker log:");
        egui::ScrollArea::vertical()
            .max_height(180.0)
            .stick_to_bottom(true)
            .show(ui, |ui| {
                for l in &self.log {
                    ui.monospace(l);
                }
            });
    }
}
