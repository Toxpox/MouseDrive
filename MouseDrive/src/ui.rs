use std::sync::atomic::Ordering;
use std::time::Duration;

use eframe::egui;
use windows::Win32::UI::WindowsAndMessaging::PostQuitMessage;

use crate::config::{Config, get_config_path};
use crate::input::{LEFT_BUTTON, RIGHT_BUTTON, RAW_INPUT_HWND};
use crate::lang::{Lang, strings};
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

        let s = strings(Lang::from_i32(self.config.language));

        // --- Sol acilir-kapanir ayar paneli ---
        egui::SidePanel::left("settings_panel")
            .resizable(true)
            .default_width(340.0)
            .min_width(280.0)
            .show_animated(ctx, self.settings_panel_open, |ui| {
                ui.vertical_centered(|ui| {
                    ui.heading(s.settings);
                });
                ui.separator();

                // Sekme butonlari
                ui.horizontal_wrapped(|ui| {
                    ui.spacing_mut().button_padding = egui::vec2(8.0, 4.0);
                    ui.selectable_value(&mut self.settings_tab, TAB_STEERING, s.tab_steering);
                    ui.selectable_value(&mut self.settings_tab, TAB_THROTTLE, s.tab_throttle);
                    ui.selectable_value(&mut self.settings_tab, TAB_BRAKE, s.tab_brake);
                    ui.selectable_value(&mut self.settings_tab, TAB_GENERAL, s.tab_general);
                });
                ui.separator();

                egui::ScrollArea::vertical().show(ui, |ui| {
                    match self.settings_tab {
                        TAB_STEERING => self.draw_steering_tab(ui, s),
                        TAB_THROTTLE => self.draw_throttle_tab(ui, s),
                        TAB_BRAKE => self.draw_brake_tab(ui, s),
                        TAB_GENERAL => self.draw_general_tab(ui, s),
                        _ => {}
                    }
                });

                ui.separator();

                // Alt butonlar
                ui.horizontal_wrapped(|ui| {
                    if ui.button(s.btn_load).clicked() {
                        if let Some(path) = get_config_path() {
                            if let Some(cfg) = Config::load_from_file(&path) {
                                self.config = cfg;
                                self.sync_globals_from_config();
                            }
                        }
                    }
                    if ui.button(s.btn_save).clicked() {
                        if let Some(path) = get_config_path() {
                            let _ = self.config.save_to_file(&path);
                        }
                    }
                    if ui.button(s.btn_default).clicked() {
                        self.config = Config::default();
                        self.sync_globals_from_config();
                    }
                });
            });

        // --- Merkez panel (gostergeler + durum) ---
        egui::CentralPanel::default().show(ctx, |ui| {
            ui.spacing_mut().item_spacing = egui::vec2(8.0, 6.0);

            ui.horizontal(|ui| {
                if ui.button(s.settings).clicked() {
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
                            s.capture_active
                        } else {
                            s.capture_paused
                        });
                        ui.separator();
                        ui.label(s.capture_toggle_hint);
                    });
                });

            ui.add_space(4.0);

            // Gostergeler
            let bar_width = (ui.available_width() - 24.0) / 3.0;
            ui.horizontal(|ui| {
                ui.vertical(|ui| {
                    ui.set_width(bar_width);
                    ui.label(s.gauge_steering);
                    let v = ((self.state.steering_filtered / STEERING_RANGE) + 1.0) * 0.5;
                    ui.add(egui::ProgressBar::new(v as f32).show_percentage());
                });
                ui.separator();
                ui.vertical(|ui| {
                    ui.set_width(bar_width);
                    ui.label(s.gauge_throttle);
                    ui.add(egui::ProgressBar::new(self.state.throttle as f32).show_percentage());
                });
                ui.separator();
                ui.vertical(|ui| {
                    ui.set_width(bar_width);
                    ui.label(s.gauge_brake);
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
                            "{}: {} | {}: {} | W: {} | S: {}",
                            s.left_click,
                            if LEFT_BUTTON.load(Ordering::SeqCst) { "ON" } else { "--" },
                            s.right_click,
                            if RIGHT_BUTTON.load(Ordering::SeqCst) { "ON" } else { "--" },
                            if self.state.w_key_pressed { "ON" } else { "--" },
                            if self.state.s_key_pressed { "ON" } else { "--" },
                        ));
                    });
                });

            ui.add_space(4.0);

            // Hizli islem butonlari
            ui.horizontal(|ui| {
                if ui.button(s.btn_reset_steering).clicked() {
                    self.state.steering = 0.0;
                    self.state.steering_filtered = 0.0;
                }
                if ui.button(s.btn_reconnect_vjoy).clicked() {
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

use crate::lang::Strings;

impl MouseDriveApp {
    fn draw_steering_tab(&mut self, ui: &mut egui::Ui, s: &Strings) {
        slider_f64(ui, s.sensitivity, &mut self.config.mouse_sens, 0.5..=10.0);

        if slider_f64(ui, s.dpi_scale, &mut self.config.mouse_dpi_scale, 0.5..=2.0) {
            self.sync_globals_from_config();
        }
        if slider_i32(ui, s.delta_cap, &mut self.config.mouse_delta_cap, 50..=800) {
            self.sync_globals_from_config();
        }

        ui.horizontal(|ui| {
            ui.label(s.mode);
            egui::ComboBox::from_id_salt("steering_mode")
                .selected_text(match self.config.steering_mode {
                    0 => s.mode_linear, 1 => s.mode_expo, 2 => s.mode_filtered, 3 => s.mode_self_center, _ => s.mode_linear,
                })
                .show_ui(ui, |ui| {
                    ui.selectable_value(&mut self.config.steering_mode, 0, s.mode_linear);
                    ui.selectable_value(&mut self.config.steering_mode, 1, s.mode_expo);
                    ui.selectable_value(&mut self.config.steering_mode, 2, s.mode_filtered);
                    ui.selectable_value(&mut self.config.steering_mode, 3, s.mode_self_center);
                });
        });

        slider_f64(ui, s.deadzone, &mut self.config.steering_deadzone, 0.0..=0.5);
        slider_f64(ui, s.saturation, &mut self.config.steering_saturation, 0.5..=1.0);
        slider_f64(ui, s.expo_power, &mut self.config.steering_expo, 0.5..=3.0);
        slider_f64(ui, s.filter_alpha, &mut self.config.steering_filter_alpha, 0.0..=1.0);
        slider_f64(ui, s.self_center_strength, &mut self.config.steering_spring_strength, 0.0..=1.0);
    }

    fn draw_throttle_tab(&mut self, ui: &mut egui::Ui, s: &Strings) {
        slider_f64(ui, s.cut_start, &mut self.config.throttle_cut_start, 0.0..=0.5);
        slider_f64(ui, s.cut_max, &mut self.config.throttle_cut_max, 0.3..=1.0);
        slider_f64(ui, s.min_at_full_lock, &mut self.config.throttle_min_cut_at_full, 0.3..=0.95);
        slider_i32(ui, s.ramp_ms, &mut self.config.throttle_ramp_ms, 10..=1000);
        slider_i32(ui, s.drop_ms, &mut self.config.throttle_drop_ms, 5..=200);
        slider_f64(ui, s.curve_power, &mut self.config.throttle_curve_exp, 0.5..=4.0);
    }

    fn draw_brake_tab(&mut self, ui: &mut egui::Ui, s: &Strings) {
        slider_f64(ui, s.min_ratio_base, &mut self.config.brake_min_ratio_base, 0.0..=1.0);
        slider_f64(ui, s.min_ratio_max, &mut self.config.brake_min_ratio_max, 0.0..=1.0);
        slider_f64(ui, s.brake_curve_power, &mut self.config.brake_curve_exp, 0.5..=4.0);
        ui.checkbox(&mut self.config.brake_trail_enabled, s.dynamic_minimum);
        slider_i32(ui, s.hold_ms, &mut self.config.brake_hold_ms, 100..=3000);
        slider_i32(ui, s.release_total_ms, &mut self.config.brake_release_total_ms, 200..=5000);
        slider_f64(ui, s.release_accel_power, &mut self.config.brake_release_accel_exp, 0.5..=4.0);
        slider_i32(ui, s.fast_apply_ms, &mut self.config.brake_fast_apply_ms, 1..=200);
        slider_i32(ui, s.fast_release_ms, &mut self.config.brake_fast_release_ms, 10..=500);
        slider_f64(ui, s.post_hold_ratio, &mut self.config.brake_after_release_hold_ratio, 0.0..=0.5);
        slider_i32(ui, s.post_hold_ms, &mut self.config.brake_after_release_hold_ms, 0..=3000);
    }

    fn draw_general_tab(&mut self, ui: &mut egui::Ui, s: &Strings) {
        slider_i32(ui, s.update_interval_ms, &mut self.config.thread_interval_ms, 1..=20);
        if ui.checkbox(&mut self.config.input_sink_enabled, s.background_capture).changed() {
            self.sync_globals_from_config();
        }
        ui.checkbox(&mut self.config.exit_on_close, s.exit_on_close);

        ui.separator();
        ui.horizontal(|ui| {
            ui.label(s.language);
            egui::ComboBox::from_id_salt("language_select")
                .selected_text(Lang::from_i32(self.config.language).label())
                .show_ui(ui, |ui| {
                    ui.selectable_value(&mut self.config.language, 0, Lang::Tr.label());
                    ui.selectable_value(&mut self.config.language, 1, Lang::En.label());
                });
        });
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
