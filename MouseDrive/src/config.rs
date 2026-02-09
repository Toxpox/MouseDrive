use serde::{Serialize, Deserialize};
use windows::Win32::UI::Shell::{SHGetFolderPathW, CSIDL_APPDATA, SHGFP_TYPE_CURRENT};

#[derive(Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct Config {
    // genel
    pub thread_interval_ms: i32,
    pub input_sink_enabled: bool,
    pub exit_on_close: bool,
    pub capture_toggle_key: i32,
    pub language: i32,

    // direksiyon
    pub mouse_sens: f64,
    pub mouse_dpi_scale: f64,
    pub mouse_delta_cap: i32,
    pub steering_mode: i32,
    pub steering_expo: f64,
    pub steering_filter_alpha: f64,
    pub steering_deadzone: f64,
    pub steering_saturation: f64,
    pub steering_spring_strength: f64,

    // gaz
    pub throttle_curve_exp: f64,
    pub throttle_min_cut_at_full: f64,
    pub throttle_cut_start: f64,
    pub throttle_cut_max: f64,
    pub throttle_ramp_ms: i32,
    pub throttle_drop_ms: i32,

    // fren
    pub brake_fast_apply_ms: i32,
    pub brake_hold_ms: i32,
    pub brake_release_total_ms: i32,
    pub brake_release_accel_exp: f64,
    pub brake_fast_release_ms: i32,
    pub brake_tap_ms: i32,
    pub brake_min_ratio_base: f64,
    pub brake_min_ratio_max: f64,
    pub brake_curve_exp: f64,
    pub brake_trail_enabled: bool,
    pub brake_after_release_hold_ratio: f64,
    pub brake_after_release_hold_ms: i32,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            thread_interval_ms: 4,
            input_sink_enabled: true,
            exit_on_close: false,
            capture_toggle_key: 0x77, // F8
            language: 1, // 0=TR, 1=EN

            mouse_sens: 3.0,
            mouse_dpi_scale: 1.0,
            mouse_delta_cap: 180,
            steering_mode: 0,
            steering_expo: 1.5,
            steering_filter_alpha: 0.25,
            steering_deadzone: 0.02,
            steering_saturation: 1.0,
            steering_spring_strength: 0.15,

            throttle_curve_exp: 2.0,
            throttle_min_cut_at_full: 0.70,
            throttle_cut_start: 0.19,
            throttle_cut_max: 0.8,
            throttle_ramp_ms: 75,
            throttle_drop_ms: 25,

            brake_fast_apply_ms: 10,
            brake_hold_ms: 1750,
            brake_release_total_ms: 2500,
            brake_release_accel_exp: 1.7,
            brake_fast_release_ms: 65,
            brake_tap_ms: 120,
            brake_min_ratio_base: 0.40,
            brake_min_ratio_max: 0.55,
            brake_curve_exp: 2.0,
            brake_trail_enabled: false,
            brake_after_release_hold_ratio: 0.06,
            brake_after_release_hold_ms: 500,
        }
    }
}

impl Config {
    pub fn load_from_file(path: &str) -> Option<Self> {
        let content = std::fs::read_to_string(path).ok()?;
        toml::from_str(&content).ok()
    }

    pub fn save_to_file(&self, path: &str) -> std::io::Result<()> {
        let content = toml::to_string_pretty(self)
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e))?;
        std::fs::write(path, content)
    }
}

pub fn get_config_path() -> Option<String> {
    // once exe dizini (portable mod)
    if let Ok(exe_path) = std::env::current_exe() {
        if let Some(exe_dir) = exe_path.parent() {
            let portable = exe_dir.join("config.toml");
            if portable.exists() {
                return portable.to_str().map(|s| s.to_string());
            }
        }
    }

    // yoksa AppData
    unsafe {
        let mut buf = [0u16; 260];
        if SHGetFolderPathW(
            None,
            CSIDL_APPDATA as i32,
            None,
            SHGFP_TYPE_CURRENT.0 as u32,
            &mut buf,
        ).is_ok() {
            let len = buf.iter().position(|&c| c == 0).unwrap_or(buf.len());
            let path = String::from_utf16_lossy(&buf[..len]);
            let config_dir = format!("{}\\MouseDrive", path);
            let _ = std::fs::create_dir_all(&config_dir);
            return Some(format!("{}\\config.toml", config_dir));
        }
    }
    None
}
