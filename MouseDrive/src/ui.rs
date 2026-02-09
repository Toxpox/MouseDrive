use std::sync::atomic::Ordering;
use std::time::Duration;

use eframe::egui;
use windows::Win32::UI::WindowsAndMessaging::PostQuitMessage;

use crate::config::{Config, get_config_path};
use crate::input::{LEFT_BUTTON, RIGHT_BUTTON, RAW_INPUT_HWND};
use crate::logic::STEERING_RANGE;
use crate::MouseDriveApp;

const TAB_STEERING: u8 = 0;
const TAB_THROTTLE: u8 = 1;
const TAB_BRAKE: u8 = 2;
const TAB_GENERAL: u8 = 3;

impl eframe::App for MouseDriveApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        self.update_input();
        ctx.request_repaint_after(Duration::from_millis(self.config.thread_interval_ms.max(1) as u64));

        // --- Sol acilir-kapanir ayar paneli ---
        egui::SidePanel::left("settings_panel")
            .resizable(true)
            .default_width(340.0)
            .min_width(280.0)
            .show_animated(ctx, self.settings_panel_open, |ui| {
                ui.vertical_centered(|ui| {
                    ui.heading("Ayarlar");
                });
                ui.separator();

                // Sekme butonlari
                ui.horizontal_wrapped(|ui| {
                    ui.spacing_mut().button_padding = egui::vec2(8.0, 4.0);
                    ui.selectable_value(&mut self.settings_tab, TAB_STEERING, "Direksiyon");
                    ui.selectable_value(&mut self.settings_tab, TAB_THROTTLE, "Gaz");
                    ui.selectable_value(&mut self.settings_tab, TAB_BRAKE, "Fren");
                    ui.selectable_value(&mut self.settings_tab, TAB_GENERAL, "Genel");
                });
                ui.separator();

                egui::ScrollArea::vertical().show(ui, |ui| {
                    match self.settings_tab {
                        TAB_STEERING => self.draw_steering_tab(ui),
                        TAB_THROTTLE => self.draw_throttle_tab(ui),
                        TAB_BRAKE => self.draw_brake_tab(ui),
                        TAB_GENERAL => self.draw_general_tab(ui),
                        _ => {}
                    }
                });

                ui.separator();

                // Alt butonlar
                ui.horizontal_wrapped(|ui| {
                    if ui.button("Yukle").clicked() {
                        if let Some(path) = get_config_path() {
                            if let Some(cfg) = Config::load_from_file(&path) {
                                self.config = cfg;
                                self.sync_globals_from_config();
                            }
                        }
                    }
                    if ui.button("Kaydet").clicked() {
                        if let Some(path) = get_config_path() {
                            let _ = self.config.save_to_file(&path);
                        }
                    }
                    if ui.button("Varsayilan").clicked() {
                        self.config = Config::default();
                        self.sync_globals_from_config();
                    }
                });
            });

        // --- Merkez panel (gostergeler + durum) ---
        egui::CentralPanel::default().show(ctx, |ui| {
            ui.spacing_mut().item_spacing = egui::vec2(8.0, 6.0);

            ui.horizontal(|ui| {
                // Panel ac/kapa butonu
                let panel_label = if self.settings_panel_open { "Ayarlar" } else { "Ayarlar" };
                if ui.button(panel_label).clicked() {
                    self.settings_panel_open = !self.settings_panel_open;
                }
                ui.heading(format!("MouseDrive v{}", env!("CARGO_PKG_VERSION")));
            });
            ui.separator();

            // Durum cubugu
            egui::Frame::NONE
                .inner_margin(egui::Margin::symmetric(6, 4))
                .corner_radius(4.0)
                .fill(ui.visuals().faint_bg_color)
                .show(ui, |ui| {
                    ui.horizontal(|ui| {
                        ui.label(&self.vjoy_status);
                        ui.separator();
                        ui.label(if self.state.capture_enabled {
                            "Yakalama: AKTIF"
                        } else {
                            "Yakalama: DURDURULDU"
                        });
                        ui.separator();
                        ui.label("F8 ile ac/kapat");
                    });
                });

            ui.add_space(4.0);

            // Gostergeler — yatay, esit genislik
            let bar_width = (ui.available_width() - 24.0) / 3.0;
            ui.horizontal(|ui| {
                ui.vertical(|ui| {
                    ui.set_width(bar_width);
                    ui.label("Direksiyon:");
                    let v = ((self.state.steering_filtered / STEERING_RANGE) + 1.0) * 0.5;
                    ui.add(egui::ProgressBar::new(v as f32).show_percentage());
                });
                ui.separator();
                ui.vertical(|ui| {
                    ui.set_width(bar_width);
                    ui.label("Gaz:");
                    ui.add(egui::ProgressBar::new(self.state.throttle as f32).show_percentage());
                });
                ui.separator();
                ui.vertical(|ui| {
                    ui.set_width(bar_width);
                    ui.label("Fren:");
                    ui.add(egui::ProgressBar::new(self.state.brake as f32).show_percentage());
                });
            });

            ui.add_space(2.0);
            ui.separator();

            // Girdi durumu
            egui::Frame::NONE
                .inner_margin(egui::Margin::symmetric(6, 4))
                .corner_radius(4.0)
                .fill(ui.visuals().faint_bg_color)
                .show(ui, |ui| {
                    ui.horizontal(|ui| {
                        ui.label(format!(
                            "Sol Tik: {} | Sag Tik: {} | W: {} | S: {}",
                            if LEFT_BUTTON.load(Ordering::SeqCst) { "ON" } else { "--" },
                            if RIGHT_BUTTON.load(Ordering::SeqCst) { "ON" } else { "--" },
                            if self.state.w_key_pressed { "ON" } else { "--" },
                            if self.state.s_key_pressed { "ON" } else { "--" },
                        ));
                    });
                });

            ui.add_space(4.0);

            // Hizli islem butonlari
            ui.horizontal(|ui| {
                if ui.button("Direksiyonu Sifirla").clicked() {
                    self.state.steering = 0.0;
                    self.state.steering_filtered = 0.0;
                }
                if ui.button("vJoy Yeniden Baglan").clicked() {
                    self.try_reconnect_vjoy();
                }
            });
        });
    }

    fn on_exit(&mut self, _gl: Option<&eframe::glow::Context>) {
        if let Some(ref vjoy) = self.vjoy {
            vjoy.reset(self.device_id);
            vjoy.relinquish(self.device_id);
        }
        let hwnd = RAW_INPUT_HWND.load(Ordering::SeqCst);
        if hwnd != 0 {
            unsafe { PostQuitMessage(0); }
        }
    }
}

// --- Sekme icerik fonksiyonlari ---

impl MouseDriveApp {
    fn draw_steering_tab(&mut self, ui: &mut egui::Ui) {
        slider_f64(ui, "Hassasiyet:", &mut self.config.mouse_sens, 0.5..=10.0);

        if slider_f64(ui, "DPI Olcek:", &mut self.config.mouse_dpi_scale, 0.5..=2.0) {
            self.sync_globals_from_config();
        }
        if slider_i32(ui, "Delta Siniri:", &mut self.config.mouse_delta_cap, 50..=800) {
            self.sync_globals_from_config();
        }

        ui.horizontal(|ui| {
            ui.label("Mod:");
            egui::ComboBox::from_id_salt("steering_mode")
                .selected_text(match self.config.steering_mode {
                    0 => "Lineer", 1 => "Expo", 2 => "Filtreli", 3 => "Self-centering", _ => "Lineer",
                })
                .show_ui(ui, |ui| {
                    ui.selectable_value(&mut self.config.steering_mode, 0, "Lineer");
                    ui.selectable_value(&mut self.config.steering_mode, 1, "Expo");
                    ui.selectable_value(&mut self.config.steering_mode, 2, "Filtreli");
                    ui.selectable_value(&mut self.config.steering_mode, 3, "Self-centering");
                });
        });

        slider_f64(ui, "Deadzone: (Onerilmez)", &mut self.config.steering_deadzone, 0.0..=0.5);
        slider_f64(ui, "Saturation:", &mut self.config.steering_saturation, 0.5..=1.0);
        slider_f64(ui, "Expo Ussu:", &mut self.config.steering_expo, 0.5..=3.0);
        slider_f64(ui, "Filtre Alpha:", &mut self.config.steering_filter_alpha, 0.0..=1.0);
        slider_f64(ui, "Self-center Gucu:", &mut self.config.steering_spring_strength, 0.0..=1.0);
    }

    fn draw_throttle_tab(&mut self, ui: &mut egui::Ui) {
        slider_f64(ui, "Kesme Baslangici:", &mut self.config.throttle_cut_start, 0.0..=0.5);
        slider_f64(ui, "Kesme Maksimum:", &mut self.config.throttle_cut_max, 0.3..=1.0);
        slider_f64(ui, "Min (tam kirma):", &mut self.config.throttle_min_cut_at_full, 0.3..=0.95);
        slider_i32(ui, "Yukselme (ms):", &mut self.config.throttle_ramp_ms, 10..=1000);
        slider_i32(ui, "Dusme (ms):", &mut self.config.throttle_drop_ms, 5..=200);
        slider_f64(ui, "Egri Ussu:", &mut self.config.throttle_curve_exp, 0.5..=4.0);
    }

    fn draw_brake_tab(&mut self, ui: &mut egui::Ui) {
        slider_f64(ui, "Min Oran (taban):", &mut self.config.brake_min_ratio_base, 0.0..=1.0);
        slider_f64(ui, "Min Oran (maks):", &mut self.config.brake_min_ratio_max, 0.0..=1.0);
        slider_f64(ui, "Egri Ussu:", &mut self.config.brake_curve_exp, 0.5..=4.0);
        ui.checkbox(&mut self.config.brake_trail_enabled, "Dinamik Minimum");
        slider_i32(ui, "Tutma (ms):", &mut self.config.brake_hold_ms, 100..=3000);
        slider_i32(ui, "Birakma Toplam (ms):", &mut self.config.brake_release_total_ms, 200..=5000);
        slider_f64(ui, "Birakma Ivme Ussu:", &mut self.config.brake_release_accel_exp, 0.5..=4.0);
        slider_i32(ui, "Hizli Dolum (ms):", &mut self.config.brake_fast_apply_ms, 1..=200);
        slider_i32(ui, "Hizli Birakma (ms):", &mut self.config.brake_fast_release_ms, 10..=500);
        slider_f64(ui, "Sonrasi Tutma Orani:", &mut self.config.brake_after_release_hold_ratio, 0.0..=0.5);
        slider_i32(ui, "Sonrasi Tutma (ms):", &mut self.config.brake_after_release_hold_ms, 0..=3000);
    }

    fn draw_general_tab(&mut self, ui: &mut egui::Ui) {
        slider_i32(ui, "Guncelleme (ms):", &mut self.config.thread_interval_ms, 1..=20);
        if ui.checkbox(&mut self.config.input_sink_enabled, "Odak Disi Yakalama").changed() {
            self.sync_globals_from_config();
        }
        ui.checkbox(&mut self.config.exit_on_close, "Kapatinca Cik");
    }
}

// --- Slider yardimlari ---

fn slider_f64(ui: &mut egui::Ui, label: &str, val: &mut f64, range: std::ops::RangeInclusive<f64>) -> bool {
    ui.horizontal(|ui| {
        ui.label(label);
        ui.add(egui::Slider::new(val, range)).changed()
    }).inner
}

fn slider_i32(ui: &mut egui::Ui, label: &str, val: &mut i32, range: std::ops::RangeInclusive<i32>) -> bool {
    ui.horizontal(|ui| {
        ui.label(label);
        ui.add(egui::Slider::new(val, range)).changed()
    }).inner
}
