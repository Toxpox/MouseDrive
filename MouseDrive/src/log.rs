#![deny(unsafe_code)]

//! Cok hafif, bagimliliksiz olay gunlugu. Yalniz durum gecislerinde cagrilir
//! (connect/reconnect, eksik DLL/sembol, guncelleme hatasi) — sicak yola
//! (kontrol dongusu/send_to_vjoy) ASLA konmaz.

use std::io::Write;
use std::time::{SystemTime, UNIX_EPOCH};

/// Gunluk dosyasinin yolu: config.toml ile ayni dizinde mousedrive.log.
fn log_path() -> Option<String> {
    let cfg = crate::config::get_config_path()?;
    // ".../config.toml" -> ".../mousedrive.log"
    let dir = std::path::Path::new(&cfg).parent()?;
    dir.join("mousedrive.log").to_str().map(|s| s.to_string())
}

/// Tek satir olay ekler (unix zaman damgali). Hata olursa sessizce yutar —
/// loglama hicbir zaman uygulamayi etkilememeli.
pub fn line(msg: &str) {
    let Some(path) = log_path() else { return };
    let ts = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    if let Ok(mut f) = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(path)
    {
        let _ = writeln!(f, "[{ts}] {msg}");
    }
}
