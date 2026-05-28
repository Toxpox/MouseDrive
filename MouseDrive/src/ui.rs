use std::sync::atomic::Ordering;
use std::time::Duration;

use eframe::egui;
use egui::{
    Align, Align2, Color32, CornerRadius, FontId, Frame, Margin, Pos2, Rect, Sense, Stroke,
    StrokeKind, Vec2, pos2, vec2,
};
use windows::Win32::Foundation::{LPARAM, WPARAM};
use windows::Win32::UI::WindowsAndMessaging::PostThreadMessageW;

use crate::MouseDriveApp;
use crate::config::{Config, get_config_path, sanitize_lifecycle_curve};
use crate::input::{LEFT_BUTTON, RAW_INPUT_THREAD_ID, RIGHT_BUTTON};
use crate::lang::{Lang, Strings, strings};
use crate::logic::{LIFECYCLE_SPLIT_X, STEERING_RANGE, eval_curve_7};
use crate::vjoy::VJoyStatus;

const TAB_STEERING: u8 = 0;
const TAB_THROTTLE: u8 = 1;
const TAB_BRAKE: u8 = 2;
const TAB_GENERAL: u8 = 3;

const WM_QUIT: u32 = 0x0012;

// --- renk paleti (modern karanlik tema) ---
const BG_DEEP: Color32 = Color32::from_rgb(15, 17, 23);
const BG_SOFT: Color32 = Color32::from_rgb(20, 23, 31);
const CARD_BG: Color32 = Color32::from_rgb(26, 30, 40);
const CARD_BORDER: Color32 = Color32::from_rgb(46, 52, 66);
const GRID_COL: Color32 = Color32::from_rgb(38, 44, 58);
const MUTED: Color32 = Color32::from_rgb(140, 148, 165);
const TEXT_DIM: Color32 = Color32::from_rgb(180, 188, 205);
const ACCENT: Color32 = Color32::from_rgb(96, 140, 255);

const STEER_COL: Color32 = Color32::from_rgb(255, 196, 60);
const THROTTLE_COL: Color32 = Color32::from_rgb(82, 217, 124);
const BRAKE_COL: Color32 = Color32::from_rgb(255, 90, 90);

impl eframe::App for MouseDriveApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        self.update_input();
        apply_visuals(ctx);

        ctx.request_repaint_after(Duration::from_millis(
            self.config.thread_interval_ms.max(1) as u64
        ));

        let s = strings(Lang::from_i32(self.config.language));

        egui::TopBottomPanel::top("header_bar")
            .frame(
                Frame::NONE
                    .fill(BG_DEEP)
                    .inner_margin(Margin::symmetric(14, 10))
                    .stroke(Stroke::new(1.0, CARD_BORDER)),
            )
            .show(ctx, |ui| {
                self.draw_header(ui, s);
            });

        egui::SidePanel::left("settings_panel")
            .resizable(true)
            .default_width(380.0)
            .min_width(320.0)
            .frame(
                Frame::NONE
                    .fill(BG_SOFT)
                    .inner_margin(Margin::same(10))
                    .stroke(Stroke::new(1.0, CARD_BORDER)),
            )
            .show_animated(ctx, self.settings_panel_open, |ui| {
                self.draw_settings_panel(ui, s);
            });

        egui::CentralPanel::default()
            .frame(
                Frame::NONE
                    .fill(BG_DEEP)
                    .inner_margin(Margin::same(14)),
            )
            .show(ctx, |ui| {
                self.draw_dashboard(ui, s);
            });
    }

    fn on_exit(&mut self, _gl: Option<&eframe::glow::Context>) {
        if let Some(ref vjoy) = self.vjoy {
            vjoy.reset(self.device_id);
            vjoy.relinquish(self.device_id);
        }

        let thread_id = RAW_INPUT_THREAD_ID.load(Ordering::SeqCst);
        if thread_id != 0 {
            unsafe {
                let _ = PostThreadMessageW(thread_id, WM_QUIT, WPARAM(0), LPARAM(0));
            }
        }
    }
}

fn apply_visuals(ctx: &egui::Context) {
    let mut v = egui::Visuals::dark();
    v.panel_fill = BG_DEEP;
    v.window_fill = CARD_BG;
    v.extreme_bg_color = Color32::from_rgb(10, 12, 18);
    v.faint_bg_color = BG_SOFT;
    v.widgets.noninteractive.bg_stroke = Stroke::new(1.0, CARD_BORDER);
    v.widgets.inactive.bg_stroke = Stroke::new(1.0, CARD_BORDER);
    v.widgets.active.bg_stroke = Stroke::new(1.5, ACCENT);
    v.widgets.hovered.bg_stroke = Stroke::new(1.0, ACCENT);
    v.selection.bg_fill = ACCENT.linear_multiply(0.45);
    ctx.set_visuals(v);
}

// --- Header & paneller ---

impl MouseDriveApp {
    fn draw_header(&mut self, ui: &mut egui::Ui, s: &Strings) {
        ui.horizontal(|ui| {
            let btn = egui::Button::new(egui::RichText::new("\u{2261}").size(20.0).color(ACCENT))
                .min_size(vec2(36.0, 30.0))
                .corner_radius(6.0)
                .fill(CARD_BG);
            if ui.add(btn).clicked() {
                self.settings_panel_open = !self.settings_panel_open;
            }

            ui.add_space(10.0);
            ui.label(
                egui::RichText::new(&self.title)
                    .size(18.0)
                    .strong()
                    .color(Color32::WHITE),
            );

            ui.with_layout(egui::Layout::right_to_left(Align::Center), |ui| {
                let btn = egui::Button::new(s.btn_reconnect_vjoy)
                    .min_size(vec2(0.0, 28.0))
                    .corner_radius(6.0);
                if ui.add(btn).clicked() {
                    self.try_reconnect_vjoy();
                }
                ui.add_space(8.0);
                draw_status_badge(ui, &self.vjoy_status, s);
                ui.add_space(8.0);
                draw_capture_badge(ui, self.state.capture_enabled, s);
            });
        });
    }

    fn draw_settings_panel(&mut self, ui: &mut egui::Ui, s: &Strings) {
        ui.vertical_centered(|ui| {
            ui.label(
                egui::RichText::new(s.settings)
                    .size(15.0)
                    .strong()
                    .color(Color32::WHITE),
            );
        });
        ui.add_space(6.0);

        ui.horizontal_wrapped(|ui| {
            ui.spacing_mut().button_padding = vec2(12.0, 6.0);
            tab_button(ui, &mut self.settings_tab, TAB_STEERING, s.tab_steering, STEER_COL);
            tab_button(ui, &mut self.settings_tab, TAB_THROTTLE, s.tab_throttle, THROTTLE_COL);
            tab_button(ui, &mut self.settings_tab, TAB_BRAKE, s.tab_brake, BRAKE_COL);
            tab_button(ui, &mut self.settings_tab, TAB_GENERAL, s.tab_general, ACCENT);
        });
        ui.add_space(8.0);

        egui::ScrollArea::vertical()
            .auto_shrink([false; 2])
            .show(ui, |ui| match self.settings_tab {
                TAB_STEERING => self.draw_steering_tab(ui, s),
                TAB_THROTTLE => self.draw_throttle_tab(ui, s),
                TAB_BRAKE => self.draw_brake_tab(ui, s),
                TAB_GENERAL => self.draw_general_tab(ui, s),
                _ => {}
            });

        ui.add_space(8.0);
        ui.separator();
        ui.add_space(4.0);

        ui.horizontal_wrapped(|ui| {
            if ui.button(s.btn_load).clicked()
                && let Some(path) = get_config_path()
                && let Some(mut cfg) = Config::load_from_file(&path)
            {
                cfg.validate();
                self.config = cfg;
                self.sync_globals_from_config();
            }
            if ui.button(s.btn_save).clicked()
                && let Some(path) = get_config_path()
            {
                let _ = self.config.save_to_file(&path);
            }
            if ui.button(s.btn_default).clicked() {
                self.config = Config::default();
                self.sync_globals_from_config();
            }
        });
    }

    fn draw_dashboard(&mut self, ui: &mut egui::Ui, s: &Strings) {
        let total_w = ui.available_width();
        let card_w = ((total_w - 24.0) / 3.0).max(180.0);
        let card_h = 160.0;

        ui.horizontal(|ui| {
            let v = ((self.state.steering_filtered / STEERING_RANGE) + 1.0) * 0.5;
            gauge_card(
                ui,
                vec2(card_w, card_h),
                s.gauge_steering,
                v as f32,
                self.state.steering_filtered,
                STEER_COL,
                GaugeKind::Bidirectional,
                None,
            );
            ui.add_space(12.0);

            gauge_card(
                ui,
                vec2(card_w, card_h),
                s.gauge_throttle,
                self.state.throttle as f32,
                self.state.throttle * 100.0,
                THROTTLE_COL,
                GaugeKind::Percentage,
                Some((self.state.throttle_t, self.state.throttle_press_active)),
            );
            ui.add_space(12.0);

            gauge_card(
                ui,
                vec2(card_w, card_h),
                s.gauge_brake,
                self.state.brake as f32,
                self.state.brake * 100.0,
                BRAKE_COL,
                GaugeKind::Percentage,
                Some((self.state.brake_t, self.state.brake_press_active)),
            );
        });

        ui.add_space(14.0);

        ui.horizontal(|ui| {
            let half = (ui.available_width() - 14.0) * 0.62;
            ui.allocate_ui(vec2(half, 100.0), |ui| {
                self.draw_input_card(ui, s);
            });
            ui.add_space(14.0);
            ui.allocate_ui(vec2(ui.available_width(), 100.0), |ui| {
                self.draw_quick_actions(ui, s);
            });
        });
    }

    fn draw_input_card(&self, ui: &mut egui::Ui, s: &Strings) {
        card(ui, |ui| {
            ui.label(egui::RichText::new(s.section_input).size(11.0).color(MUTED));
            ui.add_space(4.0);

            let lmb = LEFT_BUTTON.load(Ordering::Acquire);
            let rmb = RIGHT_BUTTON.load(Ordering::Acquire);
            let w_on = self.state.w_key_pressed;
            let s_on = self.state.s_key_pressed;

            ui.horizontal(|ui| {
                input_chip(ui, s.left_click, lmb, THROTTLE_COL);
                input_chip(ui, s.right_click, rmb, BRAKE_COL);
                input_chip(ui, "W", w_on, ACCENT);
                input_chip(ui, "S", s_on, ACCENT);
            });
            ui.add_space(6.0);
            ui.label(egui::RichText::new(s.capture_toggle_hint).size(10.0).color(MUTED));
        });
    }

    fn draw_quick_actions(&mut self, ui: &mut egui::Ui, s: &Strings) {
        card(ui, |ui| {
            ui.label(
                egui::RichText::new(s.section_quick_actions)
                    .size(11.0)
                    .color(MUTED),
            );
            ui.add_space(6.0);
            ui.horizontal_wrapped(|ui| {
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
}

// --- Sekme icerikleri ---

impl MouseDriveApp {
    fn draw_steering_tab(&mut self, ui: &mut egui::Ui, s: &Strings) {
        section(ui, s.section_input, |ui| {
            slider_f64(ui, s.sensitivity, &mut self.config.mouse_sens, 0.5..=10.0);
            if slider_f64(ui, s.dpi_scale, &mut self.config.mouse_dpi_scale, 0.5..=2.0) {
                self.sync_globals_from_config();
            }
            if slider_i32(ui, s.delta_cap, &mut self.config.mouse_delta_cap, 50..=800) {
                self.sync_globals_from_config();
            }
        });

        section(ui, s.section_response, |ui| {
            ui.horizontal(|ui| {
                ui.label(s.mode);
                egui::ComboBox::from_id_salt("steering_mode")
                    .selected_text(match self.config.steering_mode {
                        0 => s.mode_linear,
                        1 => s.mode_expo,
                        2 => s.mode_filtered,
                        3 => s.mode_self_center,
                        _ => s.mode_linear,
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
        });

        section(ui, s.section_shaping, |ui| {
            slider_f64(ui, s.expo_power, &mut self.config.steering_expo, 0.5..=3.0);
            slider_f64(ui, s.filter_alpha, &mut self.config.steering_filter_alpha, 0.0..=1.0);
            slider_f64(
                ui,
                s.self_center_strength,
                &mut self.config.steering_spring_strength,
                0.0..=1.0,
            );
        });
    }

    fn draw_throttle_tab(&mut self, ui: &mut egui::Ui, s: &Strings) {
        section(ui, s.activity_curve, |ui| {
            ui.label(egui::RichText::new(s.curve_hint).size(10.0).color(MUTED));
            ui.add_space(4.0);
            lifecycle_curve_editor(
                ui,
                &mut self.config.throttle_lifecycle_xs,
                &mut self.config.throttle_lifecycle_ys,
                THROTTLE_COL,
                self.state.throttle_t,
                self.state.throttle_press_active,
            );
            ui.add_space(6.0);
            ui.horizontal(|ui| {
                if ui.button(s.reset_curve).clicked() {
                    let d = Config::default();
                    self.config.throttle_lifecycle_xs = d.throttle_lifecycle_xs;
                    self.config.throttle_lifecycle_ys = d.throttle_lifecycle_ys;
                }
                if ui.button(s.reset_linear).clicked() {
                    self.config.throttle_lifecycle_xs =
                        [0.0, 0.17, 0.33, 0.5, 0.67, 0.83, 1.0];
                    self.config.throttle_lifecycle_ys =
                        [0.0, 0.33, 0.67, 1.0, 0.67, 0.33, 0.0];
                }
                slider_i32_inline(
                    ui,
                    s.lifecycle_ms,
                    &mut self.config.throttle_lifecycle_ms,
                    50..=3000,
                );
            });
        });

        section(ui, s.steer_rate_section, |ui| {
            slider_f64(
                ui,
                s.steer_rate_threshold,
                &mut self.config.throttle_steer_rate_threshold,
                0.05..=1.0,
            );
            slider_f64(
                ui,
                s.steer_rate_min,
                &mut self.config.throttle_steer_rate_min,
                0.0..=1.0,
            );
        });

        section(ui, s.steer_cap_section, |ui| {
            slider_f64(ui, s.cut_start, &mut self.config.throttle_cut_start, 0.0..=0.5);
            slider_f64(ui, s.cut_max, &mut self.config.throttle_cut_max, 0.3..=1.0);
            slider_f64(
                ui,
                s.min_at_full_lock,
                &mut self.config.throttle_min_cut_at_full,
                0.3..=0.95,
            );
            slider_f64(ui, s.curve_power, &mut self.config.throttle_curve_exp, 0.5..=4.0);
        });
    }

    fn draw_brake_tab(&mut self, ui: &mut egui::Ui, s: &Strings) {
        section(ui, s.activity_curve, |ui| {
            ui.label(egui::RichText::new(s.curve_hint).size(10.0).color(MUTED));
            ui.add_space(4.0);
            lifecycle_curve_editor(
                ui,
                &mut self.config.brake_lifecycle_xs,
                &mut self.config.brake_lifecycle_ys,
                BRAKE_COL,
                self.state.brake_t,
                self.state.brake_press_active,
            );
            ui.add_space(6.0);
            ui.horizontal(|ui| {
                if ui.button(s.reset_curve).clicked() {
                    let d = Config::default();
                    self.config.brake_lifecycle_xs = d.brake_lifecycle_xs;
                    self.config.brake_lifecycle_ys = d.brake_lifecycle_ys;
                }
                if ui.button(s.reset_linear).clicked() {
                    self.config.brake_lifecycle_xs =
                        [0.0, 0.17, 0.33, 0.5, 0.67, 0.83, 1.0];
                    self.config.brake_lifecycle_ys =
                        [0.0, 0.33, 0.67, 1.0, 0.67, 0.33, 0.0];
                }
                slider_i32_inline(
                    ui,
                    s.lifecycle_ms,
                    &mut self.config.brake_lifecycle_ms,
                    200..=8000,
                );
            });
        });

        section(ui, s.section_shaping, |ui| {
            ui.checkbox(&mut self.config.brake_trail_enabled, s.dynamic_minimum);
            slider_f64(ui, s.min_ratio_base, &mut self.config.brake_min_ratio_base, 0.0..=1.0);
            slider_f64(ui, s.min_ratio_max, &mut self.config.brake_min_ratio_max, 0.0..=1.0);
            slider_f64(
                ui,
                s.brake_curve_power,
                &mut self.config.brake_curve_exp,
                0.5..=4.0,
            );
        });
    }

    fn draw_general_tab(&mut self, ui: &mut egui::Ui, s: &Strings) {
        section(ui, s.section_timing, |ui| {
            slider_i32(
                ui,
                s.update_interval_ms,
                &mut self.config.thread_interval_ms,
                1..=20,
            );
        });

        section(ui, s.section_input, |ui| {
            if ui
                .checkbox(&mut self.config.input_sink_enabled, s.background_capture)
                .changed()
            {
                self.sync_globals_from_config();
            }
            ui.checkbox(&mut self.config.exit_on_close, s.exit_on_close);
        });

        section(ui, s.language, |ui| {
            ui.horizontal(|ui| {
                ui.label(s.language);
                egui::ComboBox::from_id_salt("language_select")
                    .selected_text(Lang::from_i32(self.config.language).label())
                    .show_ui(ui, |ui| {
                        ui.selectable_value(&mut self.config.language, 0, Lang::Tr.label());
                        ui.selectable_value(&mut self.config.language, 1, Lang::En.label());
                    });
            });
        });
    }
}

// --- Gauge kart cizimleri ---

enum GaugeKind {
    Bidirectional,
    Percentage,
}

fn gauge_card(
    ui: &mut egui::Ui,
    size: Vec2,
    label: &str,
    norm: f32,
    raw: f64,
    accent: Color32,
    kind: GaugeKind,
    lifecycle: Option<(f64, bool)>, // (t, press_active)
) {
    let (rect, _) = ui.allocate_exact_size(size, Sense::hover());
    let painter = ui.painter_at(rect);

    painter.rect_filled(rect, CornerRadius::same(10), CARD_BG);
    painter.rect_stroke(
        rect,
        CornerRadius::same(10),
        Stroke::new(1.0, CARD_BORDER),
        StrokeKind::Inside,
    );

    let inner = rect.shrink(14.0);

    // ust: label
    painter.text(
        pos2(inner.left(), inner.top()),
        Align2::LEFT_TOP,
        label,
        FontId::proportional(12.0),
        MUTED,
    );

    // mini durum etiketi (press/release/idle)
    if let Some((_, press)) = lifecycle {
        let (txt, col) = if press {
            ("PRESS", accent)
        } else {
            ("IDLE", MUTED)
        };
        painter.text(
            pos2(inner.right(), inner.top()),
            Align2::RIGHT_TOP,
            txt,
            FontId::proportional(10.0),
            col,
        );
    }

    // buyuk deger metni
    let value_text = match kind {
        GaugeKind::Bidirectional => {
            let pct = ((raw / STEERING_RANGE) * 100.0).clamp(-100.0, 100.0);
            format!("{:+.0}%", pct)
        }
        GaugeKind::Percentage => format!("{:.0}%", raw),
    };
    painter.text(
        pos2(inner.left(), inner.top() + 26.0),
        Align2::LEFT_TOP,
        value_text,
        FontId::proportional(36.0),
        accent,
    );

    // lifecycle mini timeline (sadece throttle/brake)
    if let Some((t, _)) = lifecycle {
        let timeline_h = 4.0;
        let tl_rect = Rect::from_min_max(
            pos2(inner.left(), inner.top() + 76.0),
            pos2(inner.right(), inner.top() + 76.0 + timeline_h),
        );
        painter.rect_filled(tl_rect, CornerRadius::same(2), GRID_COL);
        let split_x = tl_rect.left() + tl_rect.width() * 0.5;
        painter.line_segment(
            [pos2(split_x, tl_rect.top()), pos2(split_x, tl_rect.bottom())],
            Stroke::new(1.0, MUTED.linear_multiply(0.6)),
        );
        let px = tl_rect.left() + tl_rect.width() * (t as f32);
        painter.circle_filled(pos2(px, tl_rect.center().y), 4.0, accent);
        painter.text(
            pos2(inner.left(), tl_rect.bottom() + 2.0),
            Align2::LEFT_TOP,
            "press",
            FontId::proportional(9.0),
            MUTED,
        );
        painter.text(
            pos2(inner.right(), tl_rect.bottom() + 2.0),
            Align2::RIGHT_TOP,
            "release",
            FontId::proportional(9.0),
            MUTED,
        );
    }

    // bar bolgesi
    let bar_h = 14.0;
    let bar_rect = Rect::from_min_max(
        pos2(inner.left(), inner.bottom() - bar_h - 4.0),
        pos2(inner.right(), inner.bottom() - 4.0),
    );

    painter.rect_filled(bar_rect, CornerRadius::same(7), Color32::from_rgb(16, 19, 26));
    painter.rect_stroke(
        bar_rect,
        CornerRadius::same(7),
        Stroke::new(1.0, CARD_BORDER),
        StrokeKind::Inside,
    );

    match kind {
        GaugeKind::Bidirectional => {
            let c_x = bar_rect.center().x;
            let half_w = bar_rect.width() * 0.5;
            let offset = (norm - 0.5) * 2.0;
            let fill_w = (offset.abs() * half_w).min(half_w);
            let (l, r) = if offset >= 0.0 {
                (c_x, c_x + fill_w)
            } else {
                (c_x - fill_w, c_x)
            };
            let fill_rect = Rect::from_min_max(
                pos2(l, bar_rect.top()),
                pos2(r, bar_rect.bottom()),
            );
            painter.rect_filled(fill_rect, CornerRadius::same(7), accent);
            painter.line_segment(
                [pos2(c_x, bar_rect.top()), pos2(c_x, bar_rect.bottom())],
                Stroke::new(1.0, Color32::from_rgb(70, 78, 95)),
            );
        }
        GaugeKind::Percentage => {
            let w = (bar_rect.width() * norm.clamp(0.0, 1.0) as f32).max(0.0);
            let fill_rect = Rect::from_min_max(
                bar_rect.min,
                pos2(bar_rect.min.x + w, bar_rect.bottom()),
            );
            painter.rect_filled(fill_rect, CornerRadius::same(7), accent);
        }
    }
}

// --- Badge / chip / card yardimcilari ---

fn draw_status_badge(ui: &mut egui::Ui, status: &VJoyStatus, s: &Strings) {
    let (text, color) = match status {
        VJoyStatus::Connected => (s.vjoy_connected, THROTTLE_COL),
        VJoyStatus::DllNotFound => (s.vjoy_dll_not_found, BRAKE_COL),
        VJoyStatus::DriverDisabled => (s.vjoy_driver_disabled, BRAKE_COL),
        VJoyStatus::DeviceBusy => (s.vjoy_device_busy, STEER_COL),
        VJoyStatus::DeviceMissing => (s.vjoy_device_missing, STEER_COL),
        VJoyStatus::AcquireFailed => (s.vjoy_acquire_failed, BRAKE_COL),
        VJoyStatus::Unknown => (s.vjoy_unknown, MUTED),
    };
    Frame::NONE
        .fill(CARD_BG)
        .stroke(Stroke::new(1.0, color.linear_multiply(0.6)))
        .corner_radius(8.0)
        .inner_margin(Margin::symmetric(8, 4))
        .show(ui, |ui| {
            ui.label(egui::RichText::new("\u{25CF}").color(color).size(10.0));
            ui.label(egui::RichText::new(text).size(11.0).color(Color32::WHITE));
        });
}

fn draw_capture_badge(ui: &mut egui::Ui, active: bool, s: &Strings) {
    let (text, color) = if active {
        (s.capture_active, THROTTLE_COL)
    } else {
        (s.capture_paused, MUTED)
    };
    Frame::NONE
        .fill(CARD_BG)
        .stroke(Stroke::new(1.0, color.linear_multiply(0.5)))
        .corner_radius(8.0)
        .inner_margin(Margin::symmetric(8, 4))
        .show(ui, |ui| {
            ui.label(egui::RichText::new(text).size(11.0).color(color));
        });
}

fn input_chip(ui: &mut egui::Ui, label: &str, on: bool, color: Color32) {
    let (fill, border, text_col) = if on {
        (color.linear_multiply(0.25), color, Color32::WHITE)
    } else {
        (Color32::from_rgb(22, 26, 34), CARD_BORDER, MUTED)
    };
    Frame::NONE
        .fill(fill)
        .stroke(Stroke::new(1.0, border))
        .corner_radius(6.0)
        .inner_margin(Margin::symmetric(10, 4))
        .show(ui, |ui| {
            ui.label(egui::RichText::new(label).size(12.0).strong().color(text_col));
        });
}

fn card<R>(ui: &mut egui::Ui, content: impl FnOnce(&mut egui::Ui) -> R) -> R {
    Frame::NONE
        .fill(CARD_BG)
        .stroke(Stroke::new(1.0, CARD_BORDER))
        .corner_radius(10.0)
        .inner_margin(Margin::same(12))
        .show(ui, content)
        .inner
}

fn section<R>(
    ui: &mut egui::Ui,
    title: &str,
    content: impl FnOnce(&mut egui::Ui) -> R,
) -> R {
    ui.add_space(2.0);
    ui.label(
        egui::RichText::new(title)
            .size(11.0)
            .color(MUTED)
            .strong(),
    );
    let r = Frame::NONE
        .fill(CARD_BG)
        .stroke(Stroke::new(1.0, CARD_BORDER))
        .corner_radius(8.0)
        .inner_margin(Margin::same(10))
        .show(ui, content)
        .inner;
    ui.add_space(8.0);
    r
}

fn tab_button(ui: &mut egui::Ui, value: &mut u8, target: u8, label: &str, accent: Color32) {
    let selected = *value == target;
    let (fill, text_col, border) = if selected {
        (accent.linear_multiply(0.35), Color32::WHITE, accent)
    } else {
        (Color32::from_rgb(22, 26, 34), MUTED, CARD_BORDER)
    };
    let btn = egui::Button::new(egui::RichText::new(label).color(text_col).size(13.0))
        .fill(fill)
        .stroke(Stroke::new(1.0, border))
        .corner_radius(6.0);
    if ui.add(btn).clicked() {
        *value = target;
    }
}

// --- Slider yardimlari ---

fn slider_f64(
    ui: &mut egui::Ui,
    label: &str,
    val: &mut f64,
    range: std::ops::RangeInclusive<f64>,
) -> bool {
    ui.horizontal(|ui| {
        ui.label(egui::RichText::new(label).size(12.0).color(TEXT_DIM));
        ui.with_layout(egui::Layout::right_to_left(Align::Center), |ui| {
            ui.add(egui::Slider::new(val, range).show_value(true)).changed()
        }).inner
    })
    .inner
}

fn slider_i32(
    ui: &mut egui::Ui,
    label: &str,
    val: &mut i32,
    range: std::ops::RangeInclusive<i32>,
) -> bool {
    ui.horizontal(|ui| {
        ui.label(egui::RichText::new(label).size(12.0).color(TEXT_DIM));
        ui.with_layout(egui::Layout::right_to_left(Align::Center), |ui| {
            ui.add(egui::Slider::new(val, range).show_value(true)).changed()
        }).inner
    })
    .inner
}

fn slider_i32_inline(
    ui: &mut egui::Ui,
    label: &str,
    val: &mut i32,
    range: std::ops::RangeInclusive<i32>,
) -> bool {
    ui.label(egui::RichText::new(label).size(11.0).color(TEXT_DIM));
    ui.add(egui::Slider::new(val, range).show_value(true)).changed()
}

// --- Egri editor widget (7-nokta lifecycle) ---

/// 7-nokta lifecycle egri editoru.
///
/// X[0]=0, X[3]=0.5, X[6]=1 sabit. Diger noktalar serbest (zonu icinde).
/// Goz onunde:
///   - Sol yari (X < 0.5): press zonu, hafif accent arka plan
///   - Sag yari (X > 0.5): release zonu, hafif griye dogru arka plan
///   - X=0.5'te dik split cizgisi
///
/// Etkilesim:
///   - Sol-tik surukle: en yakin noktayi tasi (sinirlar dahilinde)
///   - Shift basili: snap-to-grid (0.05 X, 0.05 Y)
///   - Sag-tik: tum egriyi lineer'e (default'a degil) sifirla
///   - Hover'da nokta buyur + tooltip ile X,Y degerleri
///   - live_t isaretci: t cizgisi ve nokta
fn lifecycle_curve_editor(
    ui: &mut egui::Ui,
    xs: &mut [f64; 7],
    ys: &mut [f64; 7],
    accent: Color32,
    live_t: f64,
    press_active: bool,
) {
    let desired = vec2(ui.available_width(), 220.0);
    let (rect, response) = ui.allocate_exact_size(desired, Sense::click_and_drag());
    let painter = ui.painter_at(rect);

    let pad = 14.0;
    let inner = rect.shrink(pad);

    // arka plan
    painter.rect_filled(rect, CornerRadius::same(10), Color32::from_rgb(14, 16, 22));
    painter.rect_stroke(
        rect,
        CornerRadius::same(10),
        Stroke::new(1.0, CARD_BORDER),
        StrokeKind::Inside,
    );

    // press zone hafif accent renkli arka plan
    let split_screen_x = inner.left() + inner.width() * (LIFECYCLE_SPLIT_X as f32);
    let press_zone = Rect::from_min_max(inner.left_top(), pos2(split_screen_x, inner.bottom()));
    let release_zone = Rect::from_min_max(pos2(split_screen_x, inner.top()), inner.right_bottom());
    painter.rect_filled(press_zone, CornerRadius::ZERO, accent.linear_multiply(0.05));
    painter.rect_filled(
        release_zone,
        CornerRadius::ZERO,
        Color32::from_rgb(30, 34, 44).linear_multiply(0.5),
    );

    // grid (10 dikey, 5 yatay)
    for i in 1..10 {
        let f = i as f32 / 10.0;
        let x = inner.left() + inner.width() * f;
        painter.line_segment(
            [pos2(x, inner.top()), pos2(x, inner.bottom())],
            Stroke::new(1.0, GRID_COL),
        );
    }
    for i in 1..5 {
        let f = i as f32 / 5.0;
        let y = inner.top() + inner.height() * f;
        painter.line_segment(
            [pos2(inner.left(), y), pos2(inner.right(), y)],
            Stroke::new(1.0, GRID_COL),
        );
    }

    // split dikey cizgi
    painter.line_segment(
        [
            pos2(split_screen_x, inner.top()),
            pos2(split_screen_x, inner.bottom()),
        ],
        Stroke::new(1.5, MUTED.linear_multiply(0.7)),
    );
    painter.text(
        pos2(split_screen_x - 4.0, inner.top() + 4.0),
        Align2::RIGHT_TOP,
        "PRESS",
        FontId::proportional(9.0),
        MUTED,
    );
    painter.text(
        pos2(split_screen_x + 4.0, inner.top() + 4.0),
        Align2::LEFT_TOP,
        "RELEASE",
        FontId::proportional(9.0),
        MUTED,
    );

    // koordinat donusumleri
    let to_screen = |x: f64, y: f64| -> Pos2 {
        pos2(
            inner.left() + (x as f32) * inner.width(),
            inner.bottom() - (y as f32) * inner.height(),
        )
    };
    let from_screen = |p: Pos2| -> (f64, f64) {
        let x = ((p.x - inner.left()) / inner.width()).clamp(0.0, 1.0) as f64;
        let y = ((inner.bottom() - p.y) / inner.height()).clamp(0.0, 1.0) as f64;
        (x, y)
    };

    // sag tik -> lineer
    if response.secondary_clicked() {
        *xs = [0.0, 0.17, 0.33, 0.5, 0.67, 0.83, 1.0];
        *ys = [0.0, 0.33, 0.67, 1.0, 0.67, 0.33, 0.0];
    }

    // hover noktasi (sadece gorsel buyutme + tooltip)
    let pointer_pos = response.hover_pos().or_else(|| response.interact_pointer_pos());
    let mut hovered_idx: Option<usize> = None;
    if let Some(ptr) = pointer_pos {
        let mut best_d = 18.0;
        for i in 0..7 {
            let d = to_screen(xs[i], ys[i]).distance(ptr);
            if d < best_d {
                best_d = d;
                hovered_idx = Some(i);
            }
        }
    }

    // surukleme
    if response.dragged_by(egui::PointerButton::Primary)
        || response.drag_started_by(egui::PointerButton::Primary)
    {
        if let Some(ptr) = response.interact_pointer_pos() {
            // suruklenmekte olan nokta: drag start'ta yakalanan en yakin
            // basit: her frame en yakin noktayi yakala (5px tolerans drag mode'da, daha yumusak)
            let mut best_i = 0usize;
            let mut best_d = f32::MAX;
            for i in 0..7 {
                let d = to_screen(xs[i], ys[i]).distance(ptr);
                if d < best_d {
                    best_d = d;
                    best_i = i;
                }
            }
            if best_d < 36.0 {
                let (mut nx, mut ny) = from_screen(ptr);
                // snap-to-grid (Shift)
                let shift = ui.input(|i| i.modifiers.shift);
                if shift {
                    nx = (nx * 20.0).round() / 20.0;
                    ny = (ny * 20.0).round() / 20.0;
                }
                apply_point_drag(xs, ys, best_i, nx, ny);
                let _ = sanitize_lifecycle_curve(xs, ys);
            }
        }
    }

    // egri cizimi (yumusak doldurma + cizgi)
    let samples = 96usize;
    let curve_color = accent;
    let fill_color = accent.linear_multiply(0.18);

    // dolgu polygon (curve altinda)
    let mut fill_pts: Vec<Pos2> = Vec::with_capacity(samples + 2);
    fill_pts.push(pos2(inner.left(), inner.bottom()));
    for s in 0..=samples {
        let t = s as f64 / samples as f64;
        let y = eval_curve_7(xs, ys, t);
        fill_pts.push(to_screen(t, y));
    }
    fill_pts.push(pos2(inner.right(), inner.bottom()));
    painter.add(egui::Shape::convex_polygon(
        fill_pts,
        fill_color,
        Stroke::NONE,
    ));

    // ust cizgi
    let mut prev = to_screen(xs[0], ys[0]);
    for s in 1..=samples {
        let t = s as f64 / samples as f64;
        let y = eval_curve_7(xs, ys, t);
        let cur = to_screen(t, y);
        painter.line_segment([prev, cur], Stroke::new(2.0, curve_color));
        prev = cur;
    }

    // canli isaretci (sadece active veya t > 0 iken)
    if press_active || live_t > 0.0 {
        let t = live_t.clamp(0.0, 1.0);
        let y = eval_curve_7(xs, ys, t);
        let p = to_screen(t, y);
        // dik cizgi
        painter.line_segment(
            [pos2(p.x, inner.top()), pos2(p.x, inner.bottom())],
            Stroke::new(1.0, Color32::from_rgb(230, 230, 110).linear_multiply(0.6)),
        );
        // halka
        painter.circle_filled(p, 5.0, Color32::from_rgb(255, 255, 130));
        painter.circle_stroke(
            p,
            7.0,
            Stroke::new(2.0, Color32::from_rgb(255, 220, 80)),
        );
    }

    // kontrol noktalari (hover'da buyur)
    for i in 0..7 {
        let p = to_screen(xs[i], ys[i]);
        let is_hovered = hovered_idx == Some(i);
        let r_outer = if is_hovered { 9.0 } else { 6.5 };
        let r_inner = if is_hovered { 5.0 } else { 3.5 };
        painter.circle_filled(p, r_outer, accent.linear_multiply(0.85));
        painter.circle_filled(p, r_inner, Color32::WHITE);
        // sabit X noktalari biraz farkli (kucuk kilit isareti)
        if i == 0 || i == 3 || i == 6 {
            painter.circle_stroke(p, r_outer + 2.0, Stroke::new(1.0, MUTED));
        }
    }

    // tooltip
    if let Some(i) = hovered_idx {
        let p = to_screen(xs[i], ys[i]);
        let txt = format!("t={:.2}  v={:.2}", xs[i], ys[i]);
        let txt_pos = pos2(p.x + 10.0, p.y - 10.0);
        let galley = painter.layout_no_wrap(
            txt.clone(),
            FontId::proportional(11.0),
            Color32::WHITE,
        );
        let bg_rect = Rect::from_min_size(txt_pos, galley.size()).expand2(vec2(6.0, 3.0));
        painter.rect_filled(
            bg_rect,
            CornerRadius::same(4),
            Color32::from_rgb(20, 22, 30),
        );
        painter.rect_stroke(
            bg_rect,
            CornerRadius::same(4),
            Stroke::new(1.0, CARD_BORDER),
            StrokeKind::Inside,
        );
        painter.galley(txt_pos, galley, Color32::WHITE);
    }

    // eksen etiketleri
    painter.text(
        pos2(inner.left(), inner.bottom() + 2.0),
        Align2::LEFT_TOP,
        "0",
        FontId::proportional(9.0),
        MUTED,
    );
    painter.text(
        pos2(inner.right(), inner.bottom() + 2.0),
        Align2::RIGHT_TOP,
        "1",
        FontId::proportional(9.0),
        MUTED,
    );
    painter.text(
        pos2(inner.left() - 4.0, inner.top()),
        Align2::RIGHT_TOP,
        "1",
        FontId::proportional(9.0),
        MUTED,
    );

    // alt: snap ipucu
    if ui.input(|i| i.modifiers.shift) {
        painter.text(
            pos2(inner.right(), inner.bottom() + 12.0),
            Align2::RIGHT_TOP,
            "SNAP",
            FontId::proportional(10.0),
            accent,
        );
    }
}

/// Bir kontrol noktasini suruklerken pozisyon kisitlamalarini uygula.
fn apply_point_drag(xs: &mut [f64; 7], ys: &mut [f64; 7], idx: usize, nx: f64, ny: f64) {
    match idx {
        0 => {
            // X=0 sabit, sadece Y
            ys[0] = ny.clamp(0.0, 1.0);
        }
        3 => {
            // X=0.5 sabit, sadece Y
            ys[3] = ny.clamp(0.0, 1.0);
        }
        6 => {
            // X=1 sabit, sadece Y
            ys[6] = ny.clamp(0.0, 1.0);
        }
        1 => {
            // press zonu: 0 < X < min(X[2], 0.5)
            let hi = (xs[2] - 1e-3).min(0.5 - 1e-3);
            xs[1] = nx.clamp(1e-3, hi.max(1e-3));
            ys[1] = ny.clamp(0.0, 1.0);
        }
        2 => {
            // press zonu: X[1] < X < 0.5
            let lo = (xs[1] + 1e-3).min(0.5 - 1e-3);
            xs[2] = nx.clamp(lo, 0.5 - 1e-3);
            ys[2] = ny.clamp(0.0, 1.0);
        }
        4 => {
            // release zonu: 0.5 < X < X[5]
            let hi = (xs[5] - 1e-3).max(0.5 + 1e-3);
            xs[4] = nx.clamp(0.5 + 1e-3, hi);
            ys[4] = ny.clamp(0.0, 1.0);
        }
        5 => {
            // release zonu: X[4] < X < 1
            let lo = (xs[4] + 1e-3).max(0.5 + 1e-3);
            xs[5] = nx.clamp(lo, 1.0 - 1e-3);
            ys[5] = ny.clamp(0.0, 1.0);
        }
        _ => {}
    }
}
