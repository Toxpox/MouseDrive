#![deny(unsafe_code)]

use serde::{Deserialize, Serialize};

/// Uclar dahil maksimum kontrol noktasi sayisi
pub const MAX_POINTS: usize = 8;
/// Noktalar arasi minimum yatay (t) aralik
pub const MIN_T_SPACING: f64 = 0.02;
/// Kesin monotonluk icin minimum dikey (v) aralik
pub const MIN_V_SPACING: f64 = 0.005;

/// Interpolasyon modu. i32 kodlu (steering_mode/language gibi):
/// bozuk TOML degeri tum config'i dusurmek yerine Linear'a duser.
/// 0 = Linear, 1 = Smooth (monoton kubik / Fritsch-Carlson)
#[derive(Clone, Copy, PartialEq, Debug)]
pub enum CurveMode {
    Linear,
    Smooth,
}

impl CurveMode {
    pub fn from_i32(v: i32) -> Self {
        match v {
            1 => Self::Smooth,
            _ => Self::Linear,
        }
    }
}

#[derive(Clone, Copy, PartialEq)]
pub enum CurvePreset {
    Linear,
    SCurve,
    Aggressive,
    Progressive,
}

/// Normalize zaman zarfi egrisi: 0..1 faz -> 0..1 deger.
/// Noktalar t'ye gore sirali, uclar (0,0) ve (1,1) sabit,
/// v kesin artan (validate() bunu garanti eder).
#[derive(Clone, Serialize, Deserialize, PartialEq, Debug)]
#[serde(default)]
pub struct Curve {
    pub mode: i32,
    /// [t, v] ciftleri. TOML: points = [[0.0, 0.0], [1.0, 1.0]]
    pub points: Vec<[f64; 2]>,
}

impl Default for Curve {
    fn default() -> Self {
        // identity: eski lineer rampa davranisinin birebir karsiligi
        Self {
            mode: 0,
            points: vec![[0.0, 0.0], [1.0, 1.0]],
        }
    }
}

impl Curve {
    /// Egriyi t fazinda degerlendirir (t 0..1'e kelepcelenir).
    /// Dogrulanmis egride asla NaN donmez (cikti dogrudan vJoy'a gider).
    pub fn eval(&self, t: f64) -> f64 {
        // tangents() sabit MAX_POINTS dizisi dondurur; dogrulanmamis (>8 noktali)
        // egride OOB panigini onlemek icin segment aramasi da ayni siniri kullanir
        let n = self.points.len().min(MAX_POINTS);
        if n < 2 {
            return t.clamp(0.0, 1.0);
        }
        let t = t.clamp(0.0, 1.0);

        let mut i = 0;
        while i + 2 < n && t > self.points[i + 1][0] {
            i += 1;
        }
        let [x0, y0] = self.points[i];
        let [x1, y1] = self.points[i + 1];
        let h = (x1 - x0).max(1e-9);
        let u = ((t - x0) / h).clamp(0.0, 1.0);

        match CurveMode::from_i32(self.mode) {
            CurveMode::Linear => y0 + (y1 - y0) * u,
            CurveMode::Smooth => {
                let m = self.tangents();
                let (m0, m1) = (m[i], m[i + 1]);
                let u2 = u * u;
                let u3 = u2 * u;
                let h00 = 2.0 * u3 - 3.0 * u2 + 1.0;
                let h10 = u3 - 2.0 * u2 + u;
                let h01 = -2.0 * u3 + 3.0 * u2;
                let h11 = u3 - u2;
                (h00 * y0 + h10 * h * m0 + h01 * y1 + h11 * h * m1).clamp(0.0, 1.0)
            }
        }
    }

    /// Ters fonksiyon: eval(t) == v olan t'yi bulur (v 0..1'e kelepcelenir).
    /// Kesin v-monotonluk sayesinde tekil tanimlidir. Yon degisimlerinde
    /// fazin yeniden tohumlanmasi icin kullanilir.
    pub fn inverse_eval(&self, v: f64) -> f64 {
        let n = self.points.len().min(MAX_POINTS);
        if n < 2 {
            return v.clamp(0.0, 1.0);
        }
        let v = v.clamp(0.0, 1.0);

        let mut i = 0;
        while i + 2 < n && v > self.points[i + 1][1] {
            i += 1;
        }
        let [x0, y0] = self.points[i];
        let [x1, y1] = self.points[i + 1];

        match CurveMode::from_i32(self.mode) {
            CurveMode::Linear => {
                let dy = (y1 - y0).max(1e-9);
                x0 + (v - y0).clamp(0.0, dy) / dy * (x1 - x0)
            }
            CurveMode::Smooth => {
                // segment icinde bisection (PCHIP segment ici monoton).
                // 1e-9 t-toleransi: izin verilen en dik segmentte (~50 egim)
                // bile v hatasi 1e-6'nin cok altinda kalir
                let (mut lo, mut hi) = (x0, x1);
                for _ in 0..40 {
                    let mid = (lo + hi) * 0.5;
                    if self.eval(mid) < v {
                        lo = mid;
                    } else {
                        hi = mid;
                    }
                    if hi - lo < 1e-9 {
                        break;
                    }
                }
                (lo + hi) * 0.5
            }
        }
    }

    /// Varsayilan 2 noktali identity egrisi mi? (UI: Sifirla butonu pasiflestirme)
    pub fn is_identity(&self) -> bool {
        self.points.len() == 2
            && self.points[0] == [0.0, 0.0]
            && self.points[1] == [1.0, 1.0]
    }

    /// Fritsch-Carlson tanjantlari: monoton noktalarda monoton interpolasyon garantisi
    fn tangents(&self) -> [f64; MAX_POINTS] {
        let n = self.points.len().min(MAX_POINTS);
        let mut d = [0.0; MAX_POINTS]; // sekantlar
        for (dk, w) in d.iter_mut().zip(self.points.windows(2)).take(n - 1) {
            let dx = (w[1][0] - w[0][0]).max(1e-9);
            *dk = (w[1][1] - w[0][1]) / dx;
        }
        let mut m = [0.0; MAX_POINTS];
        m[0] = d[0];
        m[n - 1] = d[n - 2];
        for k in 1..n - 1 {
            m[k] = if d[k - 1] * d[k] <= 0.0 {
                0.0
            } else {
                (d[k - 1] + d[k]) * 0.5
            };
        }
        for k in 0..n - 1 {
            if d[k].abs() < 1e-12 {
                m[k] = 0.0;
                m[k + 1] = 0.0;
            } else {
                let a = m[k] / d[k];
                let b = m[k + 1] / d[k];
                let s = a * a + b * b;
                if s > 9.0 {
                    let tau = 3.0 / s.sqrt();
                    m[k] = tau * a * d[k];
                    m[k + 1] = tau * b * d[k];
                }
            }
        }
        m
    }

    /// Egri invariantlarini yerinde onarir; duzeltme sayisini dondurur
    /// (Config::validate ile ayni sozlesme).
    pub fn validate(&mut self) -> u32 {
        let mut corrected = 0u32;

        if !(0..=1).contains(&self.mode) {
            self.mode = self.mode.clamp(0, 1);
            corrected += 1;
        }

        // NaN/sonsuz veya yetersiz nokta: identity'ye tam sifirlama
        let broken = self.points.len() < 2
            || self
                .points
                .iter()
                .any(|p| !p[0].is_finite() || !p[1].is_finite());
        if broken {
            self.points = Curve::default().points;
            return corrected + 1;
        }

        // fazla noktalari ortadan (sondan geriye) dusur
        while self.points.len() > MAX_POINTS {
            let idx = self.points.len() - 2;
            self.points.remove(idx);
            corrected += 1;
        }

        // koordinatlari kelepcele
        for p in &mut self.points {
            let c = [p[0].clamp(0.0, 1.0), p[1].clamp(0.0, 1.0)];
            if c != *p {
                *p = c;
                corrected += 1;
            }
        }

        // t'ye gore sirala
        let sorted = self.points.windows(2).all(|w| w[0][0] <= w[1][0]);
        if !sorted {
            self.points
                .sort_by(|a, b| a[0].partial_cmp(&b[0]).unwrap_or(std::cmp::Ordering::Equal));
            corrected += 1;
        }

        // uclari sabitle
        let last = self.points.len() - 1;
        if self.points[0] != [0.0, 0.0] {
            self.points[0] = [0.0, 0.0];
            corrected += 1;
        }
        if self.points[last] != [1.0, 1.0] {
            self.points[last] = [1.0, 1.0];
            corrected += 1;
        }

        // minimum t araligi: ihlal eden orta noktayi dusur
        // (itmek yerine dusurmek kararli; sabit uc noktayi tasiramaz)
        let mut i = 1;
        while i < self.points.len() - 1 {
            let remaining_right = self.points.len() - 1 - i;
            let too_left = self.points[i][0] < self.points[i - 1][0] + MIN_T_SPACING;
            let too_right = self.points[i][0] > 1.0 - MIN_T_SPACING * remaining_right as f64;
            if too_left || too_right {
                self.points.remove(i);
                corrected += 1;
            } else {
                i += 1;
            }
        }

        // kesin v-monotonluk onarimi; sigmayan nokta dusurulur
        let mut i = 1;
        while i < self.points.len() - 1 {
            let last = self.points.len() - 1;
            let lo = self.points[i - 1][1] + MIN_V_SPACING;
            let hi = 1.0 - MIN_V_SPACING * (last - i) as f64;
            if lo > hi {
                self.points.remove(i);
                corrected += 1;
                continue;
            }
            let v = self.points[i][1].clamp(lo, hi);
            if v != self.points[i][1] {
                self.points[i][1] = v;
                corrected += 1;
            }
            i += 1;
        }

        corrected
    }

    pub fn preset(p: CurvePreset) -> Self {
        match p {
            CurvePreset::Linear => Self::default(),
            CurvePreset::SCurve => Self {
                mode: 1,
                points: vec![[0.0, 0.0], [0.25, 0.1], [0.75, 0.9], [1.0, 1.0]],
            },
            CurvePreset::Aggressive => Self {
                mode: 1,
                points: vec![[0.0, 0.0], [0.2, 0.55], [0.5, 0.85], [1.0, 1.0]],
            },
            CurvePreset::Progressive => Self {
                mode: 1,
                points: vec![[0.0, 0.0], [0.5, 0.15], [0.8, 0.5], [1.0, 1.0]],
            },
        }
    }
}

// ---- Unit Tests ----

#[cfg(test)]
mod tests {
    use super::*;

    fn all_presets() -> Vec<Curve> {
        vec![
            Curve::preset(CurvePreset::Linear),
            Curve::preset(CurvePreset::SCurve),
            Curve::preset(CurvePreset::Aggressive),
            Curve::preset(CurvePreset::Progressive),
        ]
    }

    #[test]
    fn identity_eval_and_inverse() {
        let c = Curve::default();
        for i in 0..=100 {
            let t = i as f64 / 100.0;
            assert!((c.eval(t) - t).abs() < 1e-12);
            assert!((c.inverse_eval(t) - t).abs() < 1e-12);
        }
    }

    #[test]
    fn endpoints_exact_both_modes() {
        for mut c in all_presets() {
            for mode in 0..=1 {
                c.mode = mode;
                assert_eq!(c.eval(0.0), 0.0);
                assert_eq!(c.eval(1.0), 1.0);
                assert_eq!(c.eval(-5.0), 0.0); // kelepce
                assert_eq!(c.eval(5.0), 1.0);
            }
        }
    }

    #[test]
    fn linear_midpoint() {
        let c = Curve {
            mode: 0,
            points: vec![[0.0, 0.0], [0.5, 0.2], [1.0, 1.0]],
        };
        assert!((c.eval(0.25) - 0.1).abs() < 1e-12);
        assert!((c.eval(0.75) - 0.6).abs() < 1e-12);
    }

    #[test]
    fn smooth_is_monotone() {
        for mut c in all_presets() {
            c.mode = 1;
            let mut prev = c.eval(0.0);
            for i in 1..=1000 {
                let t = i as f64 / 1000.0;
                let v = c.eval(t);
                assert!(
                    v >= prev - 1e-12,
                    "monotonluk ihlali: t={t} v={v} prev={prev}"
                );
                prev = v;
            }
        }
    }

    #[test]
    fn inverse_roundtrip_both_modes() {
        for mut c in all_presets() {
            for mode in 0..=1 {
                c.mode = mode;
                for i in 0..=100 {
                    let v = i as f64 / 100.0;
                    let t = c.inverse_eval(v);
                    assert!(
                        (c.eval(t) - v).abs() < 1e-6,
                        "roundtrip ihlali: mode={mode} v={v} t={t} eval={}",
                        c.eval(t)
                    );
                }
            }
        }
    }

    #[test]
    fn validate_nan_resets_to_identity() {
        let mut c = Curve {
            mode: 0,
            points: vec![[0.0, 0.0], [f64::NAN, 0.5], [1.0, 1.0]],
        };
        let n = c.validate();
        assert!(n >= 1);
        assert!(c.is_identity());
    }

    #[test]
    fn validate_too_few_points_resets() {
        let mut c = Curve {
            mode: 0,
            points: vec![[0.5, 0.5]],
        };
        c.validate();
        assert!(c.is_identity());
    }

    #[test]
    fn validate_caps_point_count() {
        let mut pts = vec![[0.0, 0.0]];
        for i in 1..15 {
            pts.push([i as f64 / 15.0, i as f64 / 15.0]);
        }
        pts.push([1.0, 1.0]);
        let mut c = Curve { mode: 0, points: pts };
        c.validate();
        assert!(c.points.len() <= MAX_POINTS);
    }

    #[test]
    fn validate_sorts_and_pins_endpoints() {
        let mut c = Curve {
            mode: 0,
            points: vec![[0.1, 0.2], [0.9, 0.8], [0.5, 0.5]],
        };
        let n = c.validate();
        assert!(n >= 1);
        assert_eq!(c.points[0], [0.0, 0.0]);
        assert_eq!(*c.points.last().unwrap(), [1.0, 1.0]);
        assert!(c.points.windows(2).all(|w| w[0][0] < w[1][0]));
    }

    #[test]
    fn validate_repairs_decreasing_v() {
        let mut c = Curve {
            mode: 0,
            points: vec![[0.0, 0.0], [0.3, 0.8], [0.6, 0.2], [1.0, 1.0]],
        };
        let n = c.validate();
        assert!(n >= 1);
        assert!(
            c.points.windows(2).all(|w| w[0][1] < w[1][1]),
            "v kesin artan olmali: {:?}",
            c.points
        );
    }

    #[test]
    fn validate_valid_curves_no_changes() {
        for mut c in all_presets() {
            assert_eq!(c.validate(), 0, "preset gecersiz: {:?}", c.points);
        }
    }

    #[test]
    fn validate_mode_clamped() {
        let mut c = Curve {
            mode: 99,
            points: vec![[0.0, 0.0], [1.0, 1.0]],
        };
        assert_eq!(c.validate(), 1);
        assert_eq!(c.mode, 1);
    }

    #[test]
    fn unvalidated_oversized_curve_does_not_panic() {
        // dogrulanmamis >MAX_POINTS egri: eval/inverse panik yapmamali
        let mut pts = vec![[0.0, 0.0]];
        for i in 1..=10 {
            pts.push([i as f64 / 11.0, i as f64 / 11.0]);
        }
        pts.push([1.0, 1.0]);
        let c = Curve { mode: 1, points: pts };
        for i in 0..=100 {
            let t = i as f64 / 100.0;
            let v = c.eval(t);
            assert!(v.is_finite());
            assert!(c.inverse_eval(v).is_finite());
        }
    }

    #[test]
    fn inverse_roundtrip_steep_curve() {
        // izin verilen en dik segment (~50 egim): gidis-donus yine 1e-6 altinda
        let mut c = Curve {
            mode: 1,
            points: vec![[0.0, 0.0], [0.02, 0.99], [0.5, 0.995], [1.0, 1.0]],
        };
        assert_eq!(c.validate(), 0, "test egrisi gecerli olmali");
        for mode in 0..=1 {
            c.mode = mode;
            for i in 0..=200 {
                let v = i as f64 / 200.0;
                let t = c.inverse_eval(v);
                assert!(
                    (c.eval(t) - v).abs() < 1e-6,
                    "dik egri roundtrip ihlali: mode={mode} v={v}"
                );
            }
        }
    }

    #[test]
    fn toml_roundtrip() {
        let c = Curve::preset(CurvePreset::SCurve);
        let s = toml::to_string(&c).unwrap();
        let back: Curve = toml::from_str(&s).unwrap();
        assert_eq!(back, c);
    }
}
