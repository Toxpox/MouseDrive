#![deny(unsafe_code)]

use std::sync::atomic::Ordering;
use std::time::Instant;

use crate::config::Config;
use crate::input::{LEFT_BUTTON, MOUSE_DELTA_X, RIGHT_BUTTON};

pub const STEERING_RANGE: f64 = 16383.0;

/// Tam-yasam-dongusu egrisinde press/release sinirinin oldugu sabit X degeri.
pub const LIFECYCLE_SPLIT_X: f64 = 0.5;

#[derive(Clone, Copy, PartialEq, Debug)]
pub enum SteeringMode {
    Linear,
    Expo,
    Filtered,
    SelfCenter,
}

impl SteeringMode {
    pub fn from_i32(v: i32) -> Self {
        match v {
            1 => Self::Expo,
            2 => Self::Filtered,
            3 => Self::SelfCenter,
            _ => Self::Linear,
        }
    }
}

pub struct MouseDriveState {
    pub steering: f64,
    pub steering_filtered: f64,

    pub throttle: f64,
    pub throttle_target: f64,
    pub throttle_t: f64, // 0..1 lifecycle ilerlemesi
    pub throttle_press_active: bool,

    pub brake: f64,
    pub brake_t: f64,
    pub brake_press_active: bool,

    pub last_update: Instant,
    pub capture_enabled: bool,
    pub capture_key_prev: bool,
    pub w_key_pressed: bool,
    pub s_key_pressed: bool,
}

impl MouseDriveState {
    pub fn new() -> Self {
        let now = Instant::now();
        Self {
            last_update: now,
            capture_enabled: true,
            steering: 0.0,
            steering_filtered: 0.0,
            throttle: 0.0,
            throttle_target: 0.0,
            throttle_t: 0.0,
            throttle_press_active: false,
            brake: 0.0,
            brake_t: 0.0,
            brake_press_active: false,
            capture_key_prev: false,
            w_key_pressed: false,
            s_key_pressed: false,
        }
    }

    pub fn update_steering(&mut self, config: &Config, delta_ms: f64) {
        let dx = MOUSE_DELTA_X.swap(0, Ordering::Relaxed) as f64;
        self.steering += dx * config.mouse_sens;
        self.steering = self.steering.clamp(-STEERING_RANGE, STEERING_RANGE);

        let mode = SteeringMode::from_i32(config.steering_mode);

        // yay modu: merkeze dogru exponential decay (frame-rate bagimsiz)
        if mode == SteeringMode::SelfCenter {
            let k = config.steering_spring_strength.clamp(0.0, 5.0);
            let factor = (1.0 - (-k * (delta_ms / 1000.0)).exp()).clamp(0.0, 1.0);
            self.steering -= self.steering * factor;
        }

        let sat_ratio = config.steering_saturation.clamp(0.5, 1.0);
        let sat_range = STEERING_RANGE * sat_ratio;
        self.steering = self.steering.clamp(-sat_range, sat_range);

        let dz = config.steering_deadzone.clamp(0.0, 0.5) * STEERING_RANGE;
        let abs_steer = self.steering.abs();
        let sign = self.steering.signum();

        let after_dz = if abs_steer <= dz {
            0.0
        } else {
            (abs_steer - dz) * (sat_range / (sat_range - dz).max(1.0))
        };

        let shaped = match mode {
            SteeringMode::Expo => {
                let norm = (after_dz / sat_range).clamp(0.0, 1.0);
                sign * norm.powf(config.steering_expo) * sat_range
            }
            SteeringMode::Filtered => {
                let alpha = config.steering_filter_alpha.clamp(0.0, 1.0);
                let target = sign * after_dz;
                self.steering_filtered + (target - self.steering_filtered) * alpha
            }
            _ => sign * after_dz, // Linear veya SelfCenter
        };

        self.steering_filtered = shaped.clamp(-sat_range, sat_range);
    }

    /// Gaz mantigi:
    /// - LMB basili: t = 0'dan SPLIT'e (0.5) dogru ilerler, direksiyon uzakta iken yavaslar
    /// - LMB birakildi: t = max(t, SPLIT)'ten 1.0'a dogru ilerler (release zonu)
    /// - Egri direkt hedef degeri verir; direksiyon ust kesimi target'i bastirir
    pub fn update_throttle(&mut self, config: &Config, delta_ms: f64) {
        // direksiyon-tabanli ust limit
        let cap = compute_steer_cap(self.steering_filtered, config);

        // direksiyon-tabanli ilerleme carpani (yalniz press zonunda etkilidir)
        let steer_norm = (self.steering_filtered.abs() / STEERING_RANGE).clamp(0.0, 1.0);
        let threshold = config.throttle_steer_rate_threshold.clamp(0.05, 1.0);
        let min_rate = config.throttle_steer_rate_min.clamp(0.0, 1.0);
        let rate_factor = if steer_norm <= threshold {
            1.0 - (steer_norm / threshold) * (1.0 - min_rate)
        } else {
            min_rate
        };

        let life_ms = config.throttle_lifecycle_ms.max(1) as f64;
        let raw_advance = delta_ms / life_ms;

        if LEFT_BUTTON.load(Ordering::Acquire) {
            if !self.throttle_press_active {
                self.throttle_press_active = true;
                self.throttle_t = 0.0;
            }
            // press zonu: SPLIT'e kadar ilerle, rate_factor uygulanir
            self.throttle_t = (self.throttle_t + raw_advance * rate_factor).min(LIFECYCLE_SPLIT_X);
        } else {
            if self.throttle_press_active {
                self.throttle_press_active = false;
                // release zonuna gec
                self.throttle_t = self.throttle_t.max(LIFECYCLE_SPLIT_X);
            }
            self.throttle_t = (self.throttle_t + raw_advance).min(1.0);
        }

        // egri okuma + kesim
        let curve_v = eval_curve_7(
            &config.throttle_lifecycle_xs,
            &config.throttle_lifecycle_ys,
            self.throttle_t,
        );
        let target = curve_v.min(cap);
        self.throttle_target = target;
        self.throttle = target.clamp(0.0, 1.0);
    }

    /// Fren mantigi (tek-egri full lifecycle):
    /// - RMB basili: t = 0'dan SPLIT'e (0.5) dogru ilerler (rise + peak)
    /// - RMB birakildi: t = max(t, SPLIT)'ten 1.0'a dogru ilerler (down)
    /// - trail-braking: peak degeri direksiyon uzakligina gore bastirilir
    pub fn update_brake(&mut self, config: &Config, delta_ms: f64) {
        let life_ms = config.brake_lifecycle_ms.max(1) as f64;
        let advance = delta_ms / life_ms;

        if RIGHT_BUTTON.load(Ordering::Acquire) {
            if !self.brake_press_active {
                self.brake_press_active = true;
                self.brake_t = 0.0;
            }
            self.brake_t = (self.brake_t + advance).min(LIFECYCLE_SPLIT_X);
        } else {
            if self.brake_press_active {
                self.brake_press_active = false;
                self.brake_t = self.brake_t.max(LIFECYCLE_SPLIT_X);
            }
            self.brake_t = (self.brake_t + advance).min(1.0);
        }

        let curve_v = eval_curve_7(
            &config.brake_lifecycle_xs,
            &config.brake_lifecycle_ys,
            self.brake_t,
        );

        // trail-braking: direksiyon uzaginken peak'i bastir
        let scale = if config.brake_trail_enabled {
            let s = (self.steering_filtered.abs() / STEERING_RANGE)
                .clamp(0.0, 1.0)
                .powf(config.brake_curve_exp);
            // s=0 -> 1.0 (peak korunur), s=1 -> brake_min_ratio_max
            let lo = config.brake_min_ratio_max.clamp(0.0, 1.0);
            1.0 - s * (1.0 - lo).max(0.0)
        } else {
            1.0
        };

        // alt taban (brake_min_ratio_base): RMB basili iken bile minimum bastirilamayacagi sinir degil,
        // bunun yerine en az bu kadar fren uygulansin (yumusak basamak) — sadece press zonunda etkili
        let base_floor = if self.brake_press_active {
            config.brake_min_ratio_base.clamp(0.0, 1.0)
        } else {
            0.0
        };

        let v = (curve_v * scale).max(base_floor * curve_v.signum().max(0.0));
        self.brake = v.clamp(0.0, 1.0);
    }
}

// --- Yardimcilar ---

/// Direksiyon merkezden ne kadar uzaksa gazin ust sinirini o kadar dusur.
/// Mevcut "cut_start/cut_max/min_at_full" semantigi korunur.
fn compute_steer_cap(steering_filtered: f64, config: &Config) -> f64 {
    let start_cut = config.throttle_cut_start.clamp(0.0, 0.99);
    let max_cut = config.throttle_cut_max.clamp(start_cut + 0.01, 1.0);
    let effective_ratio = steering_filtered.abs() / (STEERING_RANGE * 0.30);
    if effective_ratio <= start_cut {
        1.0
    } else if effective_ratio >= max_cut {
        config.throttle_min_cut_at_full
    } else {
        let normalized = ((effective_ratio - start_cut) / (max_cut - start_cut)).clamp(0.0, 1.0);
        let shaped = normalized.powf(config.throttle_curve_exp);
        1.0 - shaped * (1.0 - config.throttle_min_cut_at_full)
    }
}

/// 7-nokta parcali-lineer egri uzerinde t (0..1) icin Y degeri.
pub fn eval_curve_7(xs: &[f64; 7], ys: &[f64; 7], t: f64) -> f64 {
    let t = t.clamp(0.0, 1.0);
    if t <= xs[0] {
        return ys[0];
    }
    if t >= xs[6] {
        return ys[6];
    }
    for i in 0..6 {
        if t <= xs[i + 1] {
            let dx = (xs[i + 1] - xs[i]).max(1e-9);
            let f = (t - xs[i]) / dx;
            return ys[i] + (ys[i + 1] - ys[i]) * f;
        }
    }
    ys[6]
}

// ---- Unit Tests ----

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn steering_mode_conversion() {
        assert_eq!(SteeringMode::from_i32(0), SteeringMode::Linear);
        assert_eq!(SteeringMode::from_i32(1), SteeringMode::Expo);
        assert_eq!(SteeringMode::from_i32(2), SteeringMode::Filtered);
        assert_eq!(SteeringMode::from_i32(3), SteeringMode::SelfCenter);
        assert_eq!(SteeringMode::from_i32(99), SteeringMode::Linear);
        assert_eq!(SteeringMode::from_i32(-1), SteeringMode::Linear);
    }

    #[test]
    fn initial_state_capture_enabled() {
        let state = MouseDriveState::new();
        assert!(state.capture_enabled);
        assert!(!state.capture_key_prev);
        assert_eq!(state.throttle_t, 0.0);
        assert_eq!(state.brake_t, 0.0);
        assert!(!state.throttle_press_active);
        assert!(!state.brake_press_active);
    }

    #[test]
    fn eval_curve_7_endpoints() {
        let xs = [0.0, 0.15, 0.35, 0.5, 0.65, 0.85, 1.0];
        let ys = [0.0, 0.5, 0.85, 1.0, 0.95, 0.4, 0.0];
        assert!((eval_curve_7(&xs, &ys, 0.0) - 0.0).abs() < 1e-9);
        assert!((eval_curve_7(&xs, &ys, 1.0) - 0.0).abs() < 1e-9);
        // SPLIT'te peak
        assert!((eval_curve_7(&xs, &ys, 0.5) - 1.0).abs() < 1e-9);
    }

    #[test]
    fn eval_curve_7_segment_interp() {
        let xs = [0.0, 0.25, 0.5, 0.5, 0.5, 0.75, 1.0];
        let ys = [0.0, 0.5, 1.0, 1.0, 1.0, 0.5, 0.0];
        // 0..0.25 segmentinde t=0.125 -> y=0.25
        let v = eval_curve_7(&xs, &ys, 0.125);
        assert!((v - 0.25).abs() < 1e-6, "got {}", v);
    }

    #[test]
    fn eval_curve_7_clamps_out_of_range() {
        let xs = [0.0, 0.15, 0.35, 0.5, 0.65, 0.85, 1.0];
        let ys = [0.1, 0.5, 0.85, 1.0, 0.95, 0.4, 0.05];
        assert_eq!(eval_curve_7(&xs, &ys, -0.5), 0.1);
        assert_eq!(eval_curve_7(&xs, &ys, 2.0), 0.05);
    }

    #[test]
    fn split_constant_is_half() {
        assert_eq!(LIFECYCLE_SPLIT_X, 0.5);
    }
}
