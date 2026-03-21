#![deny(unsafe_code)]

/// UI localization.
/// Language: 0 = Turkce, 1 = English

#[derive(Clone, Copy, PartialEq)]
pub enum Lang {
    Tr = 0,
    En = 1,
}

impl Lang {
    pub fn from_i32(v: i32) -> Self {
        match v {
            1 => Lang::En,
            _ => Lang::Tr,
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Lang::Tr => "Turkce",
            Lang::En => "English",
        }
    }
}

pub struct Strings {
    // panel & tabs
    pub settings: &'static str,
    pub tab_steering: &'static str,
    pub tab_throttle: &'static str,
    pub tab_brake: &'static str,
    pub tab_general: &'static str,

    // buttons
    pub btn_load: &'static str,
    pub btn_save: &'static str,
    pub btn_default: &'static str,
    pub btn_reset_steering: &'static str,
    pub btn_reconnect_vjoy: &'static str,

    // status
    pub capture_active: &'static str,
    pub capture_paused: &'static str,
    pub capture_toggle_hint: &'static str,

    // gauges
    pub gauge_steering: &'static str,
    pub gauge_throttle: &'static str,
    pub gauge_brake: &'static str,
    pub left_click: &'static str,
    pub right_click: &'static str,

    // steering settings
    pub sensitivity: &'static str,
    pub dpi_scale: &'static str,
    pub delta_cap: &'static str,
    pub mode: &'static str,
    pub mode_linear: &'static str,
    pub mode_expo: &'static str,
    pub mode_filtered: &'static str,
    pub mode_self_center: &'static str,
    pub deadzone: &'static str,
    pub saturation: &'static str,
    pub expo_power: &'static str,
    pub filter_alpha: &'static str,
    pub self_center_strength: &'static str,

    // throttle settings
    pub cut_start: &'static str,
    pub cut_max: &'static str,
    pub min_at_full_lock: &'static str,
    pub ramp_ms: &'static str,
    pub drop_ms: &'static str,
    pub curve_power: &'static str,

    // brake settings
    pub min_ratio_base: &'static str,
    pub min_ratio_max: &'static str,
    pub brake_curve_power: &'static str,
    pub dynamic_minimum: &'static str,
    pub hold_ms: &'static str,
    pub release_total_ms: &'static str,
    pub release_accel_power: &'static str,
    pub fast_apply_ms: &'static str,
    pub fast_release_ms: &'static str,
    pub post_hold_ratio: &'static str,
    pub post_hold_ms: &'static str,

    // general settings
    pub update_interval_ms: &'static str,
    pub background_capture: &'static str,
    pub exit_on_close: &'static str,
    pub language: &'static str,

    // vjoy status
    pub vjoy_connected: &'static str,
    pub vjoy_dll_not_found: &'static str,
    pub vjoy_driver_disabled: &'static str,
    pub vjoy_device_busy: &'static str,
    pub vjoy_device_missing: &'static str,
    pub vjoy_acquire_failed: &'static str,
    pub vjoy_unknown: &'static str,

    // config validation (v0.5+ UI'da kullanilacak)
    #[allow(dead_code)]
    pub config_corrected: &'static str,
}

static TR: Strings = Strings {
    settings: "Ayarlar",
    tab_steering: "Direksiyon",
    tab_throttle: "Gaz",
    tab_brake: "Fren",
    tab_general: "Genel",

    btn_load: "Yukle",
    btn_save: "Kaydet",
    btn_default: "Varsayilan",
    btn_reset_steering: "Direksiyonu Sifirla",
    btn_reconnect_vjoy: "vJoy Yeniden Baglan",

    capture_active: "Yakalama: AKTIF",
    capture_paused: "Yakalama: DURDURULDU",
    capture_toggle_hint: "F8 ile ac/kapat",

    gauge_steering: "Direksiyon:",
    gauge_throttle: "Gaz:",
    gauge_brake: "Fren:",
    left_click: "Sol Tik",
    right_click: "Sag Tik",

    sensitivity: "Hassasiyet:",
    dpi_scale: "DPI Olcek:",
    delta_cap: "Delta Siniri:",
    mode: "Mod:",
    mode_linear: "Lineer",
    mode_expo: "Expo",
    mode_filtered: "Filtreli",
    mode_self_center: "Self-centering",
    deadzone: "Deadzone:",
    saturation: "Saturation:",
    expo_power: "Expo Ussu:",
    filter_alpha: "Filtre Alpha:",
    self_center_strength: "Self-center Gucu:",

    cut_start: "Kesme Baslangici:",
    cut_max: "Kesme Maksimum:",
    min_at_full_lock: "Min (tam kirma):",
    ramp_ms: "Yukselme (ms):",
    drop_ms: "Dusme (ms):",
    curve_power: "Egri Ussu:",

    min_ratio_base: "Min Oran (taban):",
    min_ratio_max: "Min Oran (maks):",
    brake_curve_power: "Egri Ussu:",
    dynamic_minimum: "Dinamik Minimum",
    hold_ms: "Tutma (ms):",
    release_total_ms: "Birakma Toplam (ms):",
    release_accel_power: "Birakma Ivme Ussu:",
    fast_apply_ms: "Hizli Dolum (ms):",
    fast_release_ms: "Hizli Birakma (ms):",
    post_hold_ratio: "Sonrasi Tutma Orani:",
    post_hold_ms: "Sonrasi Tutma (ms):",

    update_interval_ms: "Guncelleme (ms):",
    background_capture: "Odak Disi Yakalama",
    exit_on_close: "Kapatinca Cik",
    language: "Dil:",

    vjoy_connected: "vJoy baglandi \u{2713}",
    vjoy_dll_not_found: "vJoyInterface.dll bulunamadi! vJoy yuklu degil.",
    vjoy_driver_disabled: "vJoy surucusu etkin degil!",
    vjoy_device_busy: "vJoy cihazi baska uygulama tarafindan kullaniliyor.",
    vjoy_device_missing: "vJoy cihazi bulunamadi. vJoy Configure'dan etkinlestirin.",
    vjoy_acquire_failed: "vJoy cihazi alinamadi!",
    vjoy_unknown: "vJoy bilinmeyen hata.",

    config_corrected: "Config: {} parametre duzeltildi.",
};

static EN: Strings = Strings {
    settings: "Settings",
    tab_steering: "Steering",
    tab_throttle: "Throttle",
    tab_brake: "Brake",
    tab_general: "General",

    btn_load: "Load",
    btn_save: "Save",
    btn_default: "Default",
    btn_reset_steering: "Reset Steering",
    btn_reconnect_vjoy: "Reconnect vJoy",

    capture_active: "Capture: ACTIVE",
    capture_paused: "Capture: PAUSED",
    capture_toggle_hint: "F8 to toggle",

    gauge_steering: "Steering:",
    gauge_throttle: "Throttle:",
    gauge_brake: "Brake:",
    left_click: "LMB",
    right_click: "RMB",

    sensitivity: "Sensitivity:",
    dpi_scale: "DPI Scale:",
    delta_cap: "Delta Cap:",
    mode: "Mode:",
    mode_linear: "Linear",
    mode_expo: "Expo",
    mode_filtered: "Filtered",
    mode_self_center: "Self-centering",
    deadzone: "Deadzone:",
    saturation: "Saturation:",
    expo_power: "Expo Power:",
    filter_alpha: "Filter Alpha:",
    self_center_strength: "Self-center Strength:",

    cut_start: "Cut Start:",
    cut_max: "Cut Maximum:",
    min_at_full_lock: "Min (full lock):",
    ramp_ms: "Ramp (ms):",
    drop_ms: "Drop (ms):",
    curve_power: "Curve Power:",

    min_ratio_base: "Min Ratio (base):",
    min_ratio_max: "Min Ratio (max):",
    brake_curve_power: "Curve Power:",
    dynamic_minimum: "Dynamic Minimum",
    hold_ms: "Hold (ms):",
    release_total_ms: "Release Total (ms):",
    release_accel_power: "Release Accel Power:",
    fast_apply_ms: "Fast Apply (ms):",
    fast_release_ms: "Fast Release (ms):",
    post_hold_ratio: "Post-hold Ratio:",
    post_hold_ms: "Post-hold (ms):",

    update_interval_ms: "Update (ms):",
    background_capture: "Background Capture",
    exit_on_close: "Exit on Close",
    language: "Language:",

    vjoy_connected: "vJoy connected \u{2713}",
    vjoy_dll_not_found: "vJoyInterface.dll not found! vJoy is not installed.",
    vjoy_driver_disabled: "vJoy driver is not enabled!",
    vjoy_device_busy: "vJoy device is in use by another application.",
    vjoy_device_missing: "vJoy device not found. Enable it in vJoy Configure.",
    vjoy_acquire_failed: "Failed to acquire vJoy device!",
    vjoy_unknown: "vJoy unknown error.",

    config_corrected: "Config: {} parameter(s) corrected.",
};

pub fn strings(lang: Lang) -> &'static Strings {
    match lang {
        Lang::Tr => &TR,
        Lang::En => &EN,
    }
}
