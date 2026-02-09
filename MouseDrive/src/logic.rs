use std::sync::atomic::Ordering;
use std::time::Instant;

use crate::config::Config;
use crate::input::{MOUSE_DELTA_X, LEFT_BUTTON, RIGHT_BUTTON};

pub const STEERING_RANGE: f64 = 16383.0;

#[derive(Clone, Copy, PartialEq)]
pub enum BrakeState { Idle, Press, PostHold, ReleaseHold, Release }

pub struct MouseDriveState {
    pub steering: f64,
    pub steering_filtered: f64,
    pub throttle: f64,
    pub throttle_target: f64,
    pub brake: f64,
    pub brake_state: BrakeState,
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
            // Instant Default yok, elle set etmek sart
            brake_press_start: now,
            brake_post_hold_start: now,
            brake_post_hold_start_value: 1.0,
            brake_release_hold_start: now,
            last_update: now,
            capture_enabled: true,
            // geri kalani sifir/false
            steering: 0.0,
            steering_filtered: 0.0,
            throttle: 0.0,
            throttle_target: 0.0,
            brake: 0.0,
            brake_state: BrakeState::Idle,
            capture_key_prev: false,
            w_key_pressed: false,
            s_key_pressed: false,
        }
    }

    pub fn update_steering(&mut self, config: &Config, delta_ms: f64) {
        let dx = MOUSE_DELTA_X.swap(0, Ordering::SeqCst) as f64;
        self.steering += dx * config.mouse_sens;
        self.steering = self.steering.clamp(-STEERING_RANGE, STEERING_RANGE);

        // yay modu: merkeze dogru exponential decay (frame-rate bagimsiz)
        if config.steering_mode == 3 {
            let k = config.steering_spring_strength.clamp(0.0, 5.0);
            let factor = (1.0 - (-k * (delta_ms / 1000.0)).exp()).clamp(0.0, 1.0);
            self.steering -= self.steering * factor;
        }

        let sat_ratio = config.steering_saturation.clamp(0.5, 1.0);
        let sat_range = STEERING_RANGE * sat_ratio;
        self.steering = self.steering.clamp(-sat_range, sat_range);

        // deadzone disindaki degeri saturation araligina yeniden olcekle --- muhtemelen kaldirilacak, kullanissiz
        let dz = config.steering_deadzone.clamp(0.0, 0.5) * STEERING_RANGE;
        let abs_steer = self.steering.abs();
        let sign = self.steering.signum();

        let after_dz = if abs_steer <= dz {
            0.0
        } else {
            (abs_steer - dz) * (sat_range / (sat_range - dz).max(1.0))
        };

        let shaped = match config.steering_mode {
            1 => { // expo egrisi
                let norm = (after_dz / sat_range).clamp(0.0, 1.0);
                sign * norm.powf(config.steering_expo) * sat_range
            }
            2 => { // EMA filtre --- experimental, tuning gerekebilir
                let alpha = config.steering_filter_alpha.clamp(0.0, 1.0);
                let target = sign * after_dz;
                self.steering_filtered + (target - self.steering_filtered) * alpha
            }
            _ => sign * after_dz, // lineer veya yay
        };

        self.steering_filtered = shaped.clamp(-sat_range, sat_range);
    }

    pub fn update_throttle(&mut self, config: &Config, time_scale: f64) {
        let start_cut = config.throttle_cut_start.clamp(0.0, 0.99);
        let max_cut = config.throttle_cut_max.clamp(start_cut + 0.01, 1.0);

        if LEFT_BUTTON.load(Ordering::SeqCst) {
            // direksiyon acisina gore gaz kesme
            let effective_ratio = self.steering_filtered.abs() / (STEERING_RANGE * 0.30);

            let modf = if effective_ratio <= start_cut {
                1.0
            } else if effective_ratio >= max_cut {
                config.throttle_min_cut_at_full
            } else {
                let normalized = ((effective_ratio - start_cut) / (max_cut - start_cut)).clamp(0.0, 1.0);
                let shaped = normalized.powf(config.throttle_curve_exp);
                1.0 - shaped * (1.0 - config.throttle_min_cut_at_full)
            };
            self.throttle_target = modf;
        } else {
            self.throttle_target = 0.0;
        }

        // yumusak gecis (ramp up / ramp down)
        let inc = rate_from_time(1.0, config.throttle_ramp_ms, config.thread_interval_ms) * time_scale;
        let dec = rate_from_time(1.0, config.throttle_drop_ms, config.thread_interval_ms) * time_scale;
        let diff = self.throttle_target - self.throttle;
        let step = if diff > 0.0 { diff.min(inc) } else { diff.max(-dec) };
        self.throttle = (self.throttle + step).clamp(0.0, 1.0);
    }

    pub fn update_brake(&mut self, config: &Config, now: Instant, time_scale: f64) {
        // dinamik minimum: direksiyon acisina gore fren tabani
        let dyn_min = if config.brake_trail_enabled {
            let shaped = (self.steering_filtered.abs() / STEERING_RANGE)
                .clamp(0.0, 1.0)
                .powf(config.brake_curve_exp);
            config.brake_min_ratio_base
                + (config.brake_min_ratio_max - config.brake_min_ratio_base) * shaped
        } else {
            config.brake_min_ratio_base
        };

        let fast_apply = rate_from_time(1.0, config.brake_fast_apply_ms, config.thread_interval_ms);
        let right_pressed = RIGHT_BUTTON.load(Ordering::SeqCst);

        if right_pressed {
            // yeni basis: state machine'i baslat
            if matches!(self.brake_state, BrakeState::Idle | BrakeState::Release | BrakeState::ReleaseHold) {
                self.brake_state = BrakeState::Press;
                self.brake_press_start = now;
            }

            let elapsed_ms = now.duration_since(self.brake_press_start).as_millis() as i32;

            if elapsed_ms < config.brake_hold_ms {
                // hold suresi icinde: hizli uygula
                self.brake = (self.brake + fast_apply * time_scale).clamp(0.0, 1.0);
            } else {
                // hold asildi: ustel egriyle minimum'a dogru birak
                if self.brake_state == BrakeState::Press {
                    self.brake_state = BrakeState::PostHold;
                    self.brake_post_hold_start = now;
                    self.brake_post_hold_start_value = self.brake;
                }

                let t_rel = now.duration_since(self.brake_post_hold_start).as_millis() as f64;
                let progress = (t_rel / config.brake_release_total_ms as f64).clamp(0.0, 1.0);
                let shaped = progress.powf(config.brake_release_accel_exp);
                let target = self.brake_post_hold_start_value
                    - shaped * (self.brake_post_hold_start_value - dyn_min);
                self.brake = target.max(dyn_min);
            }
        } else {
            // tus birakildi
            if matches!(self.brake_state, BrakeState::Press | BrakeState::PostHold) {
                let press_elapsed = now.duration_since(self.brake_press_start).as_millis() as i32;
                if press_elapsed < config.brake_tap_ms {
                    // kisa basis (tap): direkt birakma
                    self.brake_state = BrakeState::Release;
                } else {
                    // uzun basis: birakma sonrasi kisa tutma
                    self.brake_state = BrakeState::ReleaseHold;
                    self.brake_release_hold_start = now;
                    self.brake = config.brake_after_release_hold_ratio;
                }
            }

            if self.brake_state == BrakeState::ReleaseHold {
                let rel_elapsed = now.duration_since(self.brake_release_hold_start).as_millis() as i32;
                if rel_elapsed >= config.brake_after_release_hold_ms {
                    self.brake_state = BrakeState::Release;
                } else {
                    self.brake = config.brake_after_release_hold_ratio;
                }
            }

            if self.brake_state == BrakeState::Release {
                let fast_rel = rate_from_time(1.0, config.brake_fast_release_ms, config.thread_interval_ms);
                self.brake = (self.brake - fast_rel * time_scale).max(0.0);
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

/// Tam olcegi `ms` milisaniyede dolduracak adim miktarini hesaplar
pub fn rate_from_time(full_scale: f64, ms: i32, interval_ms: i32) -> f64 {
    full_scale / (ms.max(1) as f64 / interval_ms.max(1) as f64)
}
