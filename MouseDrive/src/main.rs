#![windows_subsystem = "windows"]

mod config;
mod control;
mod curve;
mod curve_editor;
mod input;
mod lang;
mod log;
mod logic;
mod ui;
#[cfg(feature = "updater")]
mod update;
mod vjoy;

use std::ffi::c_void;
use std::sync::Arc;
use std::sync::atomic::Ordering;

use eframe::egui;
use windows::Win32::Foundation::HWND;
use windows::Win32::Media::{timeBeginPeriod, timeEndPeriod};
use windows::Win32::System::Threading::{
    GetCurrentProcess, PROCESS_CREATION_FLAGS, SetPriorityClass,
};

use crate::config::{Config, get_config_path};
use crate::control::{Shared, Snapshot};
use crate::input::*;
use crate::vjoy::VJoyStatus;

// --- MouseDrive uygulamasi (GUI yarisi) ---
//
// Kontrol matematigi + vJoy artik ayri bir thread'de (control.rs). Bu struct
// yalniz GUI'yi yonetir: config'i duzenler/yayinlar, anlik goruntuyu okur,
// komut gonderir.

pub(crate) struct MouseDriveApp {
    pub(crate) config: Config,
    pub(crate) shared: Arc<Shared>,
    pub(crate) snapshot: Snapshot,
    pub(crate) vjoy_status: VJoyStatus,
    pub(crate) settings_panel_open: bool,
    pub(crate) settings_tab: u8,
    pub(crate) title: String,
    #[cfg(feature = "updater")]
    pub(crate) update_checker: update::UpdateChecker,
    #[cfg(feature = "updater")]
    pub(crate) restart_initiated: bool,
    /// Config yuklenirken duzeltilen alan sayisi varsa kullaniciya bildirilir.
    pub(crate) config_notice: Option<u32>,
    _raw_input_handle: Option<std::thread::JoinHandle<()>>,
    control_handle: Option<std::thread::JoinHandle<()>>,
}

impl MouseDriveApp {
    pub(crate) fn new(cc: &eframe::CreationContext<'_>) -> Self {
        // Koyu tema + tek accent (mavi): secili sekme ve slider dolgusu ACCENT.
        // ONEMLI: stroke.color = secili selectable_value'nun YAZI rengi. ACCENT
        // yapilirsa mavi-zemin-uzerine-mavi-yazi okunmaz olur; beyaz birakiyoruz
        // ki secili sekme (Direksiyon/Gaz/Fren/Genel) ve combobox secimleri okunsun.
        let mut visuals = egui::Visuals::dark();
        visuals.selection.bg_fill = crate::ui::ACCENT;
        visuals.selection.stroke.color = egui::Color32::WHITE;
        cc.egui_ctx.set_visuals(visuals);

        crate::log::line(concat!("MouseDrive v", env!("CARGO_PKG_VERSION"), " baslatildi"));

        let mut config = get_config_path()
            .and_then(|p| Config::load_from_file(&p))
            .unwrap_or_default();

        // Config dogrulama: sinir disi degerleri clamp et
        let corrected = config.validate();
        let config_notice = (corrected > 0).then_some(corrected);

        // global state'i config ile senkronize et (input thread bunlari okur)
        INPUT_SINK_ENABLED.store(config.input_sink_enabled, Ordering::Release);
        MOUSE_DELTA_CAP.store(config.mouse_delta_cap, Ordering::Release);
        store_dpi_scale(config.mouse_dpi_scale);

        let raw_input_handle = start_raw_input_thread();

        // kontrol thread'i: vJoy handle'ina sahip, 250Hz bagimsiz dongu
        let shared = Shared::new(config.clone());
        let control_handle = control::spawn(Arc::clone(&shared));

        // gunde 1 otomatik guncelleme denetimi (ayri thread, acilisi geciktirmez)
        #[cfg(feature = "updater")]
        let update_checker = {
            let checker = update::UpdateChecker::new();
            if config.auto_check_updates {
                let now_ts = update::unix_now();
                if now_ts - config.last_update_check >= 86_400 {
                    config.last_update_check = now_ts;
                    if let Some(path) = get_config_path() {
                        let _ = config.save_to_file(&path);
                    }
                    // zaman damgasi degisti — kontrol thread'ine de yansit
                    shared.publish_config(&config);
                    checker.spawn_check();
                }
            }
            checker
        };

        Self {
            config,
            shared,
            snapshot: Snapshot::default(),
            vjoy_status: VJoyStatus::Unknown,
            settings_panel_open: true,
            settings_tab: 0,
            title: format!("MouseDrive v{}", env!("CARGO_PKG_VERSION")),
            #[cfg(feature = "updater")]
            update_checker,
            #[cfg(feature = "updater")]
            restart_initiated: false,
            config_notice,
            _raw_input_handle: Some(raw_input_handle),
            control_handle: Some(control_handle),
        }
    }

    /// Kontrol thread'ini durdurup vJoy'un birakilmasini bekler.
    pub(crate) fn stop_control_thread(&mut self) {
        self.shared.stop();
        if let Some(handle) = self.control_handle.take() {
            let _ = handle.join();
        }
    }

    /// GUI config'i degistirdiginde kontrol thread'ine yayinlar.
    pub(crate) fn publish_config(&self) {
        self.shared.publish_config(&self.config);
    }

    pub(crate) fn sync_globals_from_config(&self) {
        INPUT_SINK_ENABLED.store(self.config.input_sink_enabled, Ordering::Release);
        MOUSE_DELTA_CAP.store(self.config.mouse_delta_cap, Ordering::Release);
        store_dpi_scale(self.config.mouse_dpi_scale);

        let hwnd = RAW_INPUT_HWND.load(Ordering::SeqCst);
        if hwnd != 0 {
            register_raw_input(HWND(hwnd as *mut c_void), self.config.input_sink_enabled);
        }
    }
}

fn main() -> eframe::Result<()> {
    // Yuksek cozunurluklu zamanlayici: Windows 15.6ms -> 1ms
    // thread::sleep(4ms) gercekten 4ms bekler, 15ms degil
    unsafe {
        timeBeginPeriod(1);
    }

    // Proses onceligi: HIGH_PRIORITY_CLASS (0x80)
    unsafe {
        let _ = SetPriorityClass(GetCurrentProcess(), PROCESS_CREATION_FLAGS(0x80));
    }

    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([1000.0, 620.0])
            .with_min_inner_size([750.0, 480.0])
            .with_title(format!("MouseDrive v{}", env!("CARGO_PKG_VERSION"))),
        ..Default::default()
    };

    let result = eframe::run_native(
        "MouseDrive",
        options,
        Box::new(|cc| Ok(Box::new(MouseDriveApp::new(cc)))),
    );

    // HPET geri al
    unsafe {
        timeEndPeriod(1);
    }

    result
}
