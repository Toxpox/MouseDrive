#![windows_subsystem = "windows"]

mod vjoy;
mod config;
mod input;
mod logic;
mod ui;

use std::ffi::c_void;
use std::sync::atomic::Ordering;
use std::time::Instant;

use eframe::egui;
use windows::Win32::Foundation::HWND;

use crate::vjoy::{VJoyApi, VjdStat, AXIS_CENTER, AXIS_MAX, AXIS_MIN, HID_USAGE_X, HID_USAGE_Y, HID_USAGE_RZ};
use crate::config::{Config, get_config_path};
use crate::input::*;
use crate::logic::*;

pub(crate) struct MouseDriveApp {
    pub(crate) config: Config,
    pub(crate) state: MouseDriveState,
    pub(crate) vjoy: Option<VJoyApi>,
    pub(crate) device_id: u32,
    pub(crate) vjoy_status: String,
    pub(crate) settings_panel_open: bool,
    pub(crate) settings_tab: u8,
    _raw_input_handle: Option<std::thread::JoinHandle<()>>,
}

impl MouseDriveApp {
    pub(crate) fn new(cc: &eframe::CreationContext<'_>) -> Self {
        cc.egui_ctx.set_visuals(egui::Visuals::dark());

        let config = get_config_path()
            .and_then(|p| Config::load_from_file(&p))
            .unwrap_or_default();

        // global state'i config ile senkronize et
        INPUT_SINK_ENABLED.store(config.input_sink_enabled, Ordering::SeqCst);
        MOUSE_DELTA_CAP.store(config.mouse_delta_cap, Ordering::SeqCst);
        store_dpi_scale(config.mouse_dpi_scale);

        let raw_input_handle = start_raw_input_thread();
        let (vjoy, vjoy_status) = Self::connect_vjoy(1);

        Self {
            config,
            state: MouseDriveState::new(),
            vjoy,
            device_id: 1,
            vjoy_status,
            settings_panel_open: true,
            settings_tab: 0,
            _raw_input_handle: Some(raw_input_handle),
        }
    }

    /// vJoy baglantisini kur veya yeniden kur
    fn connect_vjoy(device_id: u32) -> (Option<VJoyApi>, String) {
        match VJoyApi::load() {
            Some(api) => {
                if !api.is_enabled() {
                    return (None, "vJoy driver etkin degil!".into());
                }
                let status = api.get_status(device_id);
                if status == VjdStat::Free || status == VjdStat::Own {
                    if api.acquire(device_id) {
                        api.reset(device_id);
                        (Some(api), "vJoy baglandi \u{2713}".into())
                    } else {
                        (None, "vJoy device alinamadi!".into())
                    }
                } else {
                    (None, format!("vJoy kullanilamaz: {:?}", status))
                }
            }
            None => (None, "vJoyInterface.dll bulunamadi!".into()),
        }
    }

    pub(crate) fn try_reconnect_vjoy(&mut self) {
        if let Some(ref vjoy) = self.vjoy {
            vjoy.relinquish(self.device_id);
        }
        let (vjoy, status) = Self::connect_vjoy(self.device_id);
        self.vjoy = vjoy;
        self.vjoy_status = status;
    }

    pub(crate) fn update_input(&mut self) {
        let now = Instant::now();
        let delta_ms = now.duration_since(self.state.last_update).as_secs_f64() * 1000.0;
        self.state.last_update = now;

        // F8 toggle
        let key_pressed = is_key_down(self.config.capture_toggle_key);
        if key_pressed && !self.state.capture_key_prev {
            self.state.capture_enabled = !self.state.capture_enabled;
            self.reset_state();
        }
        self.state.capture_key_prev = key_pressed;

        // orta tik -> direksiyon sifirla
        if MIDDLE_BUTTON_CLICKED.swap(false, Ordering::SeqCst) {
            self.state.steering = 0.0;
            self.state.steering_filtered = 0.0;
        }

        self.state.w_key_pressed = is_key_down(0x57); // W
        self.state.s_key_pressed = is_key_down(0x53); // S

        let safe_interval = self.config.thread_interval_ms.max(1) as f64;
        let time_scale = (delta_ms / safe_interval).clamp(0.5, 2.0);

        if self.state.capture_enabled {
            self.state.update_steering(&self.config, delta_ms);
            self.state.update_throttle(&self.config, time_scale);
            self.state.update_brake(&self.config, now, time_scale);
        } else {
            self.state.steering_filtered = 0.0;
            self.state.throttle = 0.0;
            self.state.brake = 0.0;
        }

        self.send_to_vjoy();
    }

    fn reset_state(&mut self) {
        LEFT_BUTTON.store(false, Ordering::SeqCst);
        RIGHT_BUTTON.store(false, Ordering::SeqCst);
        MOUSE_DELTA_X.store(0, Ordering::SeqCst);
        self.state.steering = 0.0;
        self.state.steering_filtered = 0.0;
        self.state.throttle = 0.0;
        self.state.throttle_target = 0.0;
        self.state.brake = 0.0;
        self.state.brake_state = BrakeState::Idle;
        if let Some(ref vjoy) = self.vjoy {
            vjoy.reset(self.device_id);
        }
    }

    fn send_to_vjoy(&self) {
        let Some(ref vjoy) = self.vjoy else { return };

        let safe_steering = self.state.steering_filtered.clamp(-STEERING_RANGE, STEERING_RANGE);
        let steer_axis = (AXIS_CENTER + safe_steering.round() as i32).clamp(AXIS_MIN, AXIS_MAX);
        let throttle_axis = (self.state.throttle * AXIS_MAX as f64).round() as i32;
        let brake_axis = (self.state.brake * AXIS_MAX as f64).round() as i32;

        vjoy.set_axis(steer_axis, self.device_id, HID_USAGE_X);
        vjoy.set_axis(throttle_axis, self.device_id, HID_USAGE_Y);
        vjoy.set_axis(brake_axis, self.device_id, HID_USAGE_RZ);
        vjoy.set_btn(self.state.w_key_pressed, self.device_id, 1);
        vjoy.set_btn(self.state.s_key_pressed, self.device_id, 2);
    }

    pub(crate) fn sync_globals_from_config(&self) {
        INPUT_SINK_ENABLED.store(self.config.input_sink_enabled, Ordering::SeqCst);
        MOUSE_DELTA_CAP.store(self.config.mouse_delta_cap, Ordering::SeqCst);
        store_dpi_scale(self.config.mouse_dpi_scale);

        let hwnd = RAW_INPUT_HWND.load(Ordering::SeqCst);
        if hwnd != 0 {
            register_raw_input(HWND(hwnd as *mut c_void), self.config.input_sink_enabled);
        }
    }
}

fn main() -> eframe::Result<()> {
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([1000.0, 620.0])
            .with_min_inner_size([750.0, 480.0])
            .with_title(format!("MouseDrive v{}", env!("CARGO_PKG_VERSION"))),
        ..Default::default()
    };

    eframe::run_native(
        "MouseDrive",
        options,
        Box::new(|cc| Ok(Box::new(MouseDriveApp::new(cc)))),
    )
}
