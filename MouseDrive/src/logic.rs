#![deny(unsafe_code)]

use std::sync::atomic::Ordering;
use std::time::Instant;

use crate::config::Config;
use crate::input::{LEFT_BUTTON, MOUSE_DELTA_X, RIGHT_BUTTON};

pub const STEERING_RANGE: f64 = 16383.0;

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

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum BrakeState {
    Idle,
    Press,
    PostHold,
    ReleaseHold,
    Release,
}

/// Zarf egrisi faz yonu: hangi egrinin (rise/fall) aktif oldugunu belirler
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum RampDir {
    Hold,
    Rising,
    Falling,
}

pub struct MouseDriveState {
    pub steering: f64,
    pub steering_filtered: f64,
    pub throttle: f64,
    pub throttle_target: f64,
    pub throttle_phase: f64,
    pub throttle_dir: RampDir,
    pub brake: f64,
    pub brake_state: BrakeState,
    pub brake_apply_phase: f64,
    pub brake_posthold_phase: f64,
    pub brake_press_start: Instant,
    pub brake_post_hold_start: Instant,
    pub brake_post_hold_start_value: f64,
    pub brake_release_hold_start: Instant,
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
            brake_press_start: now,
            brake_post_hold_start: now,
            brake_post_hold_start_value: 1.0,
            brake_release_hold_start: now,
            last_update: now,
            capture_enabled: true,
            steering: 0.0,
            steering_filtered: 0.0,
            throttle: 0.0,
            throttle_target: 0.0,
            throttle_phase: 0.0,
            throttle_dir: RampDir::Hold,
            brake: 0.0,
            brake_state: BrakeState::Idle,
            brake_apply_phase: 0.0,
            brake_posthold_phase: 0.0,
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

    pub fn update_throttle(&mut self, config: &Config, time_scale: f64) {
        let start_cut = config.throttle_cut_start.clamp(0.0, 0.99);
        let max_cut = config.throttle_cut_max.clamp(start_cut + 0.01, 1.0);

        if LEFT_BUTTON.load(Ordering::Acquire) {
            let effective_ratio = self.steering_filtered.abs() / (STEERING_RANGE * 0.30);

            let modf = if effective_ratio <= start_cut {
                1.0
            } else if effective_ratio >= max_cut {
                config.throttle_min_cut_at_full
            } else {
                let normalized =
                    ((effective_ratio - start_cut) / (max_cut - start_cut)).clamp(0.0, 1.0);
                let shaped = normalized.powf(config.throttle_curve_exp);
                1.0 - shaped * (1.0 - config.throttle_min_cut_at_full)
            };
            self.throttle_target = modf;
        } else {
            self.throttle_target = 0.0;
        }

        let dt_ms = config.thread_interval_ms.max(1) as f64 * time_scale;
        let desired = self.throttle_target;
        self.ramp_toward(config, desired, dt_ms);
    }

    /// Mevcut degerden hedefe dogru, zarf egrisi uzerinde faz takibiyle ilerler.
    /// Hedef deger uzayinda hareketli tavan/tabandir (direksiyon-cut salinimi
    /// bugunku gibi hiz sinirli kalir). Yon degisiminde faz, yeni egrinin ters
    /// fonksiyonuyla yeniden tohumlanir -> cikti yapisal olarak surekli.
    /// Identity egrilerde eski lineer rampaya birebir indirgenir.
    fn ramp_toward(&mut self, config: &Config, desired: f64, dt_ms: f64) {
        const EPS: f64 = 1e-6;

        if desired > self.throttle + EPS {
            let rise = &config.throttle_rise_curve;
            if self.throttle_dir != RampDir::Rising {
                self.throttle_phase = rise.inverse_eval(self.throttle);
                self.throttle_dir = RampDir::Rising;
            }
            self.throttle_phase = (self.throttle_phase
                + dt_ms / config.throttle_ramp_ms.max(1) as f64)
                .min(1.0);
            let mut y = rise.eval(self.throttle_phase);
            if y >= desired {
                y = desired;
                self.throttle_phase = rise.inverse_eval(desired);
            }
            self.throttle = y;
        } else if desired < self.throttle - EPS {
            let fall = &config.throttle_fall_curve;
            if self.throttle_dir != RampDir::Falling {
                self.throttle_phase = fall.inverse_eval(self.throttle);
                self.throttle_dir = RampDir::Falling;
            }
            self.throttle_phase = (self.throttle_phase
                - dt_ms / config.throttle_drop_ms.max(1) as f64)
                .max(0.0);
            let mut y = fall.eval(self.throttle_phase);
            if y <= desired {
                y = desired;
                self.throttle_phase = fall.inverse_eval(desired);
            }
            self.throttle = y;
        } else {
            self.throttle = desired;
            self.throttle_dir = RampDir::Hold;
        }
        self.throttle = self.throttle.clamp(0.0, 1.0);
    }

    pub fn update_brake(&mut self, config: &Config, now: Instant, time_scale: f64) {
        let dyn_min = if config.brake_trail_enabled {
            let shaped = (self.steering_filtered.abs() / STEERING_RANGE)
                .clamp(0.0, 1.0)
                .powf(config.brake_curve_exp);
            config.brake_min_ratio_base
                + (config.brake_min_ratio_max - config.brake_min_ratio_base) * shaped
        } else {
            config.brake_min_ratio_base
        };

        let dt_ms = config.thread_interval_ms.max(1) as f64 * time_scale;
        let right_pressed = RIGHT_BUTTON.load(Ordering::Acquire);

        if right_pressed {
            if matches!(
                self.brake_state,
                BrakeState::Idle | BrakeState::Release | BrakeState::ReleaseHold
            ) {
                self.brake_state = BrakeState::Press;
                self.brake_press_start = now;
                // mevcut degerden (plato 0.06 veya Release ortasi) surekli devam
                self.brake_apply_phase = config.brake_apply_curve.inverse_eval(self.brake);
            }

            let elapsed_ms = now.duration_since(self.brake_press_start).as_millis() as i32;

            if elapsed_ms < config.brake_hold_ms {
                self.brake_apply_phase = (self.brake_apply_phase
                    + dt_ms / config.brake_fast_apply_ms.max(1) as f64)
                    .min(1.0);
                self.brake = config.brake_apply_curve.eval(self.brake_apply_phase);
            } else {
                if self.brake_state == BrakeState::Press {
                    self.brake_state = BrakeState::PostHold;
                    self.brake_post_hold_start = now;
                    self.brake_post_hold_start_value = self.brake;
                }

                let t_rel = now.duration_since(self.brake_post_hold_start).as_millis() as f64;
                let progress = (t_rel / config.brake_release_total_ms as f64).clamp(0.0, 1.0);
                // egri, exp on-sekillemesinin UZERINE uygulanir: identity egride
                // eski progress^exp davranisi birebir korunur, Ivme Ussu calismaya
                // devam eder. Saf grafiksel kontrol icin exp = 1.0 secilebilir.
                let pre = progress.powf(config.brake_release_accel_exp);
                self.brake_posthold_phase = pre;
                let shaped = config.brake_posthold_curve.eval(pre);
                let target = self.brake_post_hold_start_value
                    - shaped * (self.brake_post_hold_start_value - dyn_min);
                self.brake = target.max(dyn_min);
            }
        } else {
            if matches!(self.brake_state, BrakeState::Press | BrakeState::PostHold) {
                let press_elapsed = now.duration_since(self.brake_press_start).as_millis() as i32;
                if press_elapsed < config.brake_tap_ms {
                    self.brake_state = BrakeState::Release;
                } else {
                    self.brake_state = BrakeState::ReleaseHold;
                    self.brake_release_hold_start = now;
                    self.brake = config.brake_after_release_hold_ratio;
                }
            }

            if self.brake_state == BrakeState::ReleaseHold {
                let rel_elapsed = now
                    .duration_since(self.brake_release_hold_start)
                    .as_millis() as i32;
                if rel_elapsed >= config.brake_after_release_hold_ms {
                    self.brake_state = BrakeState::Release;
                } else {
                    self.brake = config.brake_after_release_hold_ratio;
                }
            }

            if self.brake_state == BrakeState::Release {
                // buton birakma: egrisiz, eski lineer hizli birakma
                self.brake = (self.brake - dt_ms / config.brake_fast_release_ms.max(1) as f64)
                    .max(0.0);
                if self.brake <= 0.0 {
                    self.brake = 0.0;
                    self.brake_state = BrakeState::Idle;
                }
            }

            if self.brake_state == BrakeState::Idle {
                self.brake = 0.0;
            }
        }

        self.brake = self.brake.clamp(0.0, 1.0);
    }
}

// ---- Unit Tests ----

#[cfg(test)]
mod tests {
    use super::*;
    use crate::curve::{Curve, CurvePreset};

    #[test]
    fn ramp_identity_matches_legacy_linear() {
        // identity egrilerle faz algoritmasi eski lineer rampaya birebir esdeger
        let config = Config::default(); // ramp 75ms, drop 25ms, interval 4ms
        let mut st = MouseDriveState::new();
        let dt = 4.0; // time_scale = 1.0

        let inc = 4.0 / 75.0;
        let dec = 4.0 / 25.0;
        let mut expected: f64 = 0.0;
        for _ in 0..40 {
            st.ramp_toward(&config, 1.0, dt);
            expected = (expected + inc).min(1.0);
            assert!((st.throttle - expected).abs() < 1e-9);
        }
        for _ in 0..40 {
            st.ramp_toward(&config, 0.0, dt);
            expected = (expected - dec).max(0.0);
            assert!((st.throttle - expected).abs() < 1e-9);
        }
    }

    #[test]
    fn ramp_reseed_is_continuous() {
        // dt=0 ile yon degisimi: deger sicramamali (ters tohumlama dogrulamasi)
        let mut config = Config::default();
        config.throttle_rise_curve = Curve::preset(CurvePreset::SCurve);
        config.throttle_fall_curve = Curve::preset(CurvePreset::Aggressive);
        let mut st = MouseDriveState::new();

        for _ in 0..10 {
            st.ramp_toward(&config, 1.0, 4.0);
        }
        let prev = st.throttle;
        assert!(prev > 0.0 && prev < 1.0);

        st.ramp_toward(&config, 0.0, 0.0); // yon degisti, faz ilerlemedi
        assert!(
            (st.throttle - prev).abs() < 1e-5,
            "tohumlama sicramasi: {} -> {}",
            prev,
            st.throttle
        );
    }

    #[test]
    fn ramp_moving_ceiling_clamps_to_target() {
        // salinan hedef (direksiyon-cut benzeri): cikti hedefi asla asmamali
        let mut config = Config::default();
        config.throttle_rise_curve = Curve::preset(CurvePreset::SCurve);
        let mut st = MouseDriveState::new();

        for i in 0..200 {
            let target = if i % 2 == 0 { 0.6 } else { 0.7 };
            st.ramp_toward(&config, target, 4.0);
            assert!(st.throttle <= target + 1e-9, "tavan ihlali: {}", st.throttle);
        }
    }

    #[test]
    fn brake_identity_press_release_matches_legacy() {
        use std::time::Duration;

        // not: RIGHT_BUTTON global atomigini yalnizca bu test kullanir
        let config = Config::default(); // fast_apply 10ms, fast_release 65ms, tap 120ms
        let mut st = MouseDriveState::new();
        let mut now = Instant::now();
        let mut expected: f64 = 0.0;

        RIGHT_BUTTON.store(true, Ordering::Release);
        for _ in 0..5 {
            now += Duration::from_millis(4);
            st.update_brake(&config, now, 1.0);
            expected = (expected + 4.0 / 10.0).min(1.0);
            assert!((st.brake - expected).abs() < 1e-9);
        }
        assert_eq!(st.brake_state, BrakeState::Press);

        // 20ms < tap_ms (120) -> dogrudan Release, eski lineer azalmayla esdeger
        RIGHT_BUTTON.store(false, Ordering::Release);
        for _ in 0..20 {
            now += Duration::from_millis(4);
            st.update_brake(&config, now, 1.0);
            expected = (expected - 4.0 / 65.0).max(0.0);
            assert!((st.brake - expected).abs() < 1e-9);
        }
        assert_eq!(st.brake_state, BrakeState::Idle);

        // faz 2: uzun tutma -> PostHold dususu identity egriyle eski
        // progress^exp formulune birebir esit olmali
        RIGHT_BUTTON.store(true, Ordering::Release);
        for _ in 0..438 {
            now += Duration::from_millis(4);
            st.update_brake(&config, now, 1.0);
        }
        // elapsed = 4*437 = 1748ms < hold (1750): hala Press, fren dolu
        assert_eq!(st.brake_state, BrakeState::Press);
        assert!((st.brake - 1.0).abs() < 1e-12);

        let post_hold_start = now + Duration::from_millis(4);
        for k in 0..=100 {
            now += Duration::from_millis(4);
            st.update_brake(&config, now, 1.0);
            if k == 0 {
                assert_eq!(st.brake_state, BrakeState::PostHold);
            }
            let t_rel = now.duration_since(post_hold_start).as_millis() as f64;
            let progress = (t_rel / 2500.0).clamp(0.0, 1.0);
            let pre = progress.powf(1.7); // brake_release_accel_exp
            let exp_val = (1.0 - pre * (1.0 - 0.40)).max(0.40); // dyn_min = 0.40
            assert!(
                (st.brake - exp_val).abs() < 1e-9,
                "PostHold sapmasi: k={k} brake={} beklenen={exp_val}",
                st.brake
            );
        }

        // uzun basis birakildi -> ReleaseHold platosu, sonra Release -> Idle
        RIGHT_BUTTON.store(false, Ordering::Release);
        now += Duration::from_millis(4);
        st.update_brake(&config, now, 1.0);
        assert_eq!(st.brake_state, BrakeState::ReleaseHold);
        assert!((st.brake - 0.06).abs() < 1e-12);
        for _ in 0..200 {
            now += Duration::from_millis(4);
            st.update_brake(&config, now, 1.0);
        }
        assert_eq!(st.brake_state, BrakeState::Idle);
        assert_eq!(st.brake, 0.0);
    }

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
    fn brake_state_initial_idle() {
        let state = MouseDriveState::new();
        assert_eq!(state.brake_state, BrakeState::Idle);
        assert_eq!(state.brake, 0.0);
        assert_eq!(state.throttle, 0.0);
        assert_eq!(state.steering, 0.0);
    }

    #[test]
    fn initial_state_capture_enabled() {
        let state = MouseDriveState::new();
        assert!(state.capture_enabled);
        assert!(!state.capture_key_prev);
    }
}
