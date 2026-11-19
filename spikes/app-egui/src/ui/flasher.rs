use crate::app_core::uf2::{self, BootVolume, Progress};
use crossbeam_channel::{Receiver, unbounded};
use std::path::PathBuf;
use std::time::Duration;

enum Msg {
    Volumes(Vec<BootVolume>),
    Progress(Progress),
    Finished(Result<(), String>),
}

pub struct FlasherTab {
    ctx: egui::Context,
    sim_path: String,
    uf2_path: String,
    volumes: Vec<BootVolume>,
    rx: Option<Receiver<Msg>>,
    log: Vec<String>,
    progress: f32,
    busy: bool,
}

impl FlasherTab {
    pub fn new(ctx: egui::Context) -> Self {
        let root = std::env::temp_dir().join("ocarina-spike-sim");
        FlasherTab {
            ctx,
            sim_path: root.join(uf2::VOLUME_LABEL).display().to_string(),
            uf2_path: root.join("dummy-bridge.uf2").display().to_string(),
            volumes: vec![],
            rx: None,
            log: vec![],
            progress: 0.0,
            busy: false,
        }
    }

    fn sim_dir(&self) -> Option<PathBuf> {
        let s = self.sim_path.trim();
        (!s.is_empty()).then(|| PathBuf::from(s))
    }

    /// Run `job` on a worker thread; it reports through a channel and wakes the UI.
    fn spawn(&mut self, job: impl FnOnce(&dyn Fn(Msg)) + Send + 'static) {
        let (tx, rx) = unbounded();
        let ctx = self.ctx.clone();
        std::thread::spawn(move || {
            let send = |m: Msg| {
                let _ = tx.send(m);
                ctx.request_repaint();
            };
            job(&send);
        });
        self.rx = Some(rx);
    }

    pub fn poll(&mut self) {
        let Some(rx) = &self.rx else { return };
        while let Ok(m) = rx.try_recv() {
            match m {
                Msg::Volumes(v) => {
                    self.log.push(format!("scan: {} boot volume(s)", v.len()));
                    self.volumes = v;
                }
                Msg::Progress(Progress::Copying { written, total }) => {
                    self.progress = written as f32 / total as f32;
                }
                Msg::Progress(p) => self.log.push(format!("{p:?}")),
                Msg::Finished(r) => {
                    self.busy = false;
                    self.log.push(match r {
                        Ok(()) => "flash OK (volume disappeared)".into(),
                        Err(e) => format!("flash FAILED: {e}"),
                    });
                }
            }
        }
    }

    pub fn ui(&mut self, ui: &mut egui::Ui) {
        ui.heading("UF2 flasher (simulation only)");
        ui.colored_label(
            egui::Color32::from_rgb(230, 120, 60),
            "Safety: this spike only flashes the simulated directory. Real RPI-RP2 volumes are listed but cannot be selected.",
        );
        ui.horizontal(|ui| {
            ui.label("Simulated drive at path:");
            ui.add(egui::TextEdit::singleline(&mut self.sim_path).desired_width(420.0));
        });
        ui.horizontal(|ui| {
            ui.label("UF2 file:");
            ui.add(egui::TextEdit::singleline(&mut self.uf2_path).desired_width(420.0));
        });
        ui.horizontal(|ui| {
            if ui.button("Create fake RPI-RP2 + dummy UF2").clicked() {
                let root = std::env::temp_dir().join("ocarina-spike-sim");
                match uf2::make_simulated_drive(&root) {
                    Ok((d, f)) => {
                        self.sim_path = d.display().to_string();
                        self.uf2_path = f.display().to_string();
                        self.log.push(format!("created {}", d.display()));
                    }
                    Err(e) => self.log.push(format!("create failed: {e}")),
                }
            }
            if ui
                .add_enabled(!self.busy, egui::Button::new("Scan volumes"))
                .clicked()
            {
                let sim = self.sim_dir();
                self.spawn(move |send| send(Msg::Volumes(uf2::find_boot_volumes(sim.as_deref()))));
            }
            let sim_ok = self
                .sim_dir()
                .is_some_and(|d| uf2::probe(&d, uf2::VOLUME_LABEL).is_some());
            if ui
                .add_enabled(
                    !self.busy && sim_ok,
                    egui::Button::new("Flash simulated drive"),
                )
                .clicked()
            {
                let drive = self.sim_dir().unwrap();
                let file = PathBuf::from(self.uf2_path.trim());
                self.busy = true;
                self.progress = 0.0;
                // the fake bootloader "reboots" 300 ms after the UF2 lands
                uf2::spawn_fake_bootloader(drive.clone(), Duration::from_millis(300));
                self.spawn(move |send| {
                    // Spike guard: refuse anything that is a real mount point.
                    let canon = drive.canonicalize().unwrap_or(drive.clone());
                    let real = uf2::mounted_volumes()
                        .iter()
                        .any(|(m, _)| m.parent().is_some() && (m == &canon || m == &drive))
                        || canon.starts_with("/Volumes")
                        || canon.starts_with("/media")
                        || canon.starts_with("/run/media");
                    if real {
                        send(Msg::Finished(Err(
                            "refused: path is a real mounted volume".into()
                        )));
                        return;
                    }
                    let r = uf2::flash(
                        &file,
                        &drive,
                        Duration::from_secs(10),
                        Duration::from_millis(100),
                        |p| send(Msg::Progress(p)),
                    );
                    send(Msg::Finished(r.map_err(|e| e.to_string())));
                });
            }
            if !sim_ok {
                ui.label("(simulated path has no INFO_UF2.TXT: create it first)");
            }
        });
        ui.add(egui::ProgressBar::new(self.progress).show_percentage());
        ui.separator();
        ui.label("Detected boot volumes:");
        for v in &self.volumes {
            ui.monospace(format!(
                "{}  label={:?} model={:?} board={:?}",
                v.mount_point.display(),
                v.label,
                v.model,
                v.board_id
            ));
        }
        ui.separator();
        egui::ScrollArea::vertical()
            .stick_to_bottom(true)
            .show(ui, |ui| {
                for l in &self.log {
                    ui.monospace(l);
                }
            });
    }
}
