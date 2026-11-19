use crate::app_core::gamepad::{Button, Pad};
use egui::{Color32, Key, Sense, vec2};

const KEYMAP: [(Key, Button); 10] = [
    (Key::J, Button::A),
    (Key::K, Button::B),
    (Key::I, Button::X),
    (Key::U, Button::Y),
    (Key::Q, Button::L),
    (Key::E, Button::R),
    (Key::ArrowUp, Button::Up),
    (Key::ArrowDown, Button::Down),
    (Key::ArrowLeft, Button::Left),
    (Key::ArrowRight, Button::Right),
];

#[derive(Default)]
pub struct PadTab {
    pad: Pad,
    log: Vec<String>,
}

impl PadTab {
    /// Key events are read every frame so a key-up is never missed. Key-downs only count while
    /// the gamepad tab is active. Auto-repeat events are counted and ignored.
    pub fn handle_keys(&mut self, ctx: &egui::Context, active: bool) {
        let (events, focused) = ctx.input(|i| (i.events.clone(), i.focused));
        if !focused {
            self.pad.release_all();
        }
        if !active {
            for b in crate::app_core::gamepad::ALL {
                self.pad.pointer(b, false);
            }
        }
        for ev in events {
            if let egui::Event::Key {
                key,
                pressed,
                repeat,
                ..
            } = ev
                && let Some((_, b)) = KEYMAP.iter().find(|(k, _)| *k == key)
                && (active || !pressed)
                && self.pad.key(*b, pressed, repeat)
            {
                self.log.push(format!(
                    "{b:?} {} -> {:02x?}",
                    if pressed { "down" } else { "up" },
                    self.pad.state()
                ));
            }
        }
        if self.log.len() > 200 {
            self.log.drain(..100);
        }
    }

    fn button(&mut self, ui: &mut egui::Ui, b: Button, label: &str) {
        let (rect, resp) = ui.allocate_exact_size(vec2(56.0, 56.0), Sense::click_and_drag());
        let down = resp.is_pointer_button_down_on();
        if self.pad.pointer(b, down) {
            self.log.push(format!(
                "{b:?} {} (pointer) -> {:02x?}",
                if down { "down" } else { "up" },
                self.pad.state()
            ));
        }
        let fill = if self.pad.held(b) {
            Color32::from_rgb(255, 170, 60)
        } else {
            ui.visuals().widgets.inactive.bg_fill
        };
        ui.painter().circle_filled(rect.center(), 26.0, fill);
        ui.painter().text(
            rect.center(),
            egui::Align2::CENTER_CENTER,
            label,
            egui::FontId::proportional(16.0),
            ui.visuals().strong_text_color(),
        );
    }

    pub fn ui(&mut self, ui: &mut egui::Ui) {
        ui.heading("On-screen gamepad (stub)");
        ui.label("Keys: Q=L  E=R  U=Y  I=X  J=A  K=B  arrows=D-pad. Hold the mouse on a button to press it.");
        ui.horizontal(|ui| {
            ui.vertical(|ui| {
                ui.horizontal(|ui| {
                    ui.add_space(60.0);
                    self.button(ui, Button::Up, "▲");
                });
                ui.horizontal(|ui| {
                    self.button(ui, Button::Left, "◀");
                    ui.add_space(60.0);
                    self.button(ui, Button::Right, "▶");
                });
                ui.horizontal(|ui| {
                    ui.add_space(60.0);
                    self.button(ui, Button::Down, "▼");
                });
            });
            ui.add_space(40.0);
            ui.vertical(|ui| {
                ui.horizontal(|ui| {
                    self.button(ui, Button::L, "L");
                    self.button(ui, Button::R, "R");
                });
                ui.horizontal(|ui| {
                    self.button(ui, Button::Y, "Y");
                    self.button(ui, Button::X, "X");
                });
                ui.horizontal(|ui| {
                    self.button(ui, Button::B, "B");
                    self.button(ui, Button::A, "A");
                });
            });
        });
        ui.separator();
        ui.monospace(format!(
            "state {:02x?}  presses={} releases={} repeats ignored={}",
            self.pad.state(),
            self.pad.presses,
            self.pad.releases,
            self.pad.repeats_ignored
        ));
        egui::ScrollArea::vertical()
            .stick_to_bottom(true)
            .show(ui, |ui| {
                for l in &self.log {
                    ui.monospace(l);
                }
            });
    }
}
