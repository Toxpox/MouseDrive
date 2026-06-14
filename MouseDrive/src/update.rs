#![deny(unsafe_code)]

use std::sync::{Arc, Mutex};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

const RELEASES_API: &str = "https://api.github.com/repos/Toxpox/MouseDrive/releases/latest";
const HTTP_TIMEOUT_SECS: u64 = 5;
const DOWNLOAD_TIMEOUT_SECS: u64 = 120;
const MAX_ZIP_BYTES: u64 = 100 * 1024 * 1024;
const MAX_SUMS_BYTES: u64 = 64 * 1024;
const USER_AGENT: &str = concat!("MouseDrive/", env!("CARGO_PKG_VERSION"));

/// Son surumun indirilebilir varliklari
#[derive(Clone, PartialEq, Debug)]
pub struct ReleaseInfo {
    pub version: String,  // tag (orn. "v0.5.0")
    pub html_url: String, // surum sayfasi (fallback)
    pub zip_url: Option<String>,
    pub sums_url: Option<String>,
    pub zip_name: String, // checksum satiri eslestirme icin
}

impl ReleaseInfo {
    /// Otomatik kurulum icin gerekli varliklar (zip + checksum) mevcut mu?
    pub fn auto_installable(&self) -> bool {
        self.zip_url.is_some() && self.sums_url.is_some()
    }
}

#[derive(Clone, PartialEq, Debug)]
pub enum UpdateStatus {
    Idle,
    Checking,
    UpToDate,
    Failed, // denetim basarisiz (ag yok vb.) — sessiz
    Available(ReleaseInfo),
    Updating, // indirme + dogrulama + exe degisimi suruyor
    ReadyToRestart,
    UpdateFailed(ReleaseInfo), // otomatik kurulum basarisiz -> linke geri dus
}

/// Arka plan thread'lerinde surum denetimi ve otomatik kurulum yapar.
/// UI her frame status() ile durumu okur; ana dongu asla bloklanmaz.
pub struct UpdateChecker {
    status: Arc<Mutex<UpdateStatus>>,
}

impl Default for UpdateChecker {
    fn default() -> Self {
        Self::new()
    }
}

impl UpdateChecker {
    pub fn new() -> Self {
        Self {
            status: Arc::new(Mutex::new(UpdateStatus::Idle)),
        }
    }

    pub fn status(&self) -> UpdateStatus {
        self.status
            .lock()
            .map(|s| s.clone())
            .unwrap_or(UpdateStatus::Failed)
    }

    /// Denetimi ayri thread'de baslatir; mesgulse yenisini acmaz
    pub fn spawn_check(&self) {
        if let Ok(mut s) = self.status.lock() {
            if matches!(*s, UpdateStatus::Checking | UpdateStatus::Updating) {
                return;
            }
            *s = UpdateStatus::Checking;
        } else {
            return;
        }

        let status = Arc::clone(&self.status);
        std::thread::spawn(move || {
            let result = check_latest().unwrap_or(UpdateStatus::Failed);
            if let Ok(mut s) = status.lock() {
                *s = result;
            }
        });
    }

    /// Otomatik kurulumu ayri thread'de baslatir:
    /// zip indir -> SHA-256 dogrula -> mousedrive.exe'yi cikar -> calisan
    /// exe'yi degistir. Basarida ReadyToRestart; UI yeniden baslatir.
    pub fn spawn_update(&self, info: ReleaseInfo) {
        if let Ok(mut s) = self.status.lock() {
            if matches!(*s, UpdateStatus::Updating) {
                return;
            }
            *s = UpdateStatus::Updating;
        } else {
            return;
        }

        let status = Arc::clone(&self.status);
        std::thread::spawn(move || {
            let result = match run_update(&info) {
                Ok(()) => UpdateStatus::ReadyToRestart,
                Err(()) => UpdateStatus::UpdateFailed(info),
            };
            if let Ok(mut s) = status.lock() {
                *s = result;
            }
        });
    }
}

fn check_latest() -> Option<UpdateStatus> {
    let info = fetch_latest()?;
    let remote = parse_version(&info.version)?;
    let local = parse_version(env!("CARGO_PKG_VERSION"))?;
    if remote > local {
        Some(UpdateStatus::Available(info))
    } else {
        Some(UpdateStatus::UpToDate)
    }
}

fn fetch_latest() -> Option<ReleaseInfo> {
    let resp = ureq::get(RELEASES_API)
        // GitHub API User-Agent'siz istekleri 403 ile reddeder
        .set("User-Agent", USER_AGENT)
        .set("Accept", "application/vnd.github+json")
        .timeout(Duration::from_secs(HTTP_TIMEOUT_SECS))
        .call()
        .ok()?;
    let body = resp.into_string().ok()?;
    parse_release_json(&body)
}

/// GitHub /releases/latest yanitindan surum + varlik adreslerini cikarir
fn parse_release_json(body: &str) -> Option<ReleaseInfo> {
    let v: serde_json::Value = serde_json::from_str(body).ok()?;
    let version = v.get("tag_name")?.as_str()?.to_string();
    let html_url = v.get("html_url")?.as_str()?.to_string();

    let mut zip_url = None;
    let mut sums_url = None;
    let mut zip_name = String::new();
    if let Some(assets) = v.get("assets").and_then(|a| a.as_array()) {
        for asset in assets {
            let name = asset.get("name").and_then(|x| x.as_str());
            let url = asset.get("browser_download_url").and_then(|x| x.as_str());
            let (Some(name), Some(url)) = (name, url) else {
                continue;
            };
            if name.starts_with("MouseDrive-") && name.ends_with("-windows-x64.zip") {
                zip_url = Some(url.to_string());
                zip_name = name.to_string();
            } else if name == "SHA256SUMS.txt" {
                sums_url = Some(url.to_string());
            }
        }
    }

    Some(ReleaseInfo {
        version,
        html_url,
        zip_url,
        sums_url,
        zip_name,
    })
}

// ---- Otomatik kurulum adimlari ----

fn run_update(info: &ReleaseInfo) -> Result<(), ()> {
    let zip_url = info.zip_url.as_ref().ok_or(())?;
    let sums_url = info.sums_url.as_ref().ok_or(())?;

    let zip_bytes = download(zip_url, MAX_ZIP_BYTES)?;
    let sums_text = String::from_utf8(download(sums_url, MAX_SUMS_BYTES)?).map_err(|_| ())?;

    // checksum dogrulamasi: uyusmazlikta kurulum reddedilir
    let expected = parse_sha256sums(&sums_text, &info.zip_name).ok_or(())?;
    if sha256_hex(&zip_bytes) != expected {
        return Err(());
    }

    let exe_bytes = extract_exe(&zip_bytes)?;

    // calisan exe dogrudan degistirilemez; self-replace rename hilesini uygular
    let tmp = std::env::temp_dir().join("mousedrive-update.exe");
    std::fs::write(&tmp, exe_bytes).map_err(|_| ())?;
    let replaced = self_replace::self_replace(&tmp);
    let _ = std::fs::remove_file(&tmp);
    replaced.map_err(|_| ())
}

fn download(url: &str, limit: u64) -> Result<Vec<u8>, ()> {
    use std::io::Read;

    let resp = ureq::get(url)
        .set("User-Agent", USER_AGENT)
        .timeout(Duration::from_secs(DOWNLOAD_TIMEOUT_SECS))
        .call()
        .map_err(|_| ())?;
    let mut buf = Vec::new();
    resp.into_reader()
        .take(limit + 1)
        .read_to_end(&mut buf)
        .map_err(|_| ())?;
    if buf.len() as u64 > limit {
        return Err(());
    }
    Ok(buf)
}

fn sha256_hex(bytes: &[u8]) -> String {
    use sha2::{Digest, Sha256};
    Sha256::digest(bytes)
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect()
}

/// "hash  dosyaadi" satirlarindan istenen dosyanin hash'ini bulur
fn parse_sha256sums(text: &str, file_name: &str) -> Option<String> {
    for line in text.lines() {
        let mut parts = line.split_whitespace();
        let (Some(hash), Some(name)) = (parts.next(), parts.next()) else {
            continue;
        };
        // sha256sum binary modunda dosya adi '*' onekiyle yazilir
        if name.trim_start_matches('*') == file_name {
            return Some(hash.to_ascii_lowercase());
        }
    }
    None
}

/// Zip icinden mousedrive.exe baytlarini cikarir (path traversal korumali)
fn extract_exe(zip_bytes: &[u8]) -> Result<Vec<u8>, ()> {
    use std::io::Read;

    let mut archive =
        zip::ZipArchive::new(std::io::Cursor::new(zip_bytes)).map_err(|_| ())?;
    for i in 0..archive.len() {
        let mut file = archive.by_index(i).map_err(|_| ())?;
        let name = file.name().to_string();
        if name.contains("..") {
            continue;
        }
        let base = name.rsplit(['/', '\\']).next().unwrap_or("");
        if base.eq_ignore_ascii_case("mousedrive.exe") {
            let mut out = Vec::new();
            file.read_to_end(&mut out).map_err(|_| ())?;
            if out.is_empty() {
                return Err(());
            }
            return Ok(out);
        }
    }
    Err(())
}

/// "v0.5.0" veya "0.5.0" -> (0, 5, 0). Bozuk girdide None.
pub fn parse_version(s: &str) -> Option<(u32, u32, u32)> {
    let s = s.trim().trim_start_matches('v');
    let mut parts = s.split('.');
    let major = parts.next()?.parse().ok()?;
    let minor = parts.next()?.parse().ok()?;
    let patch = parts.next()?.parse().ok()?;
    if parts.next().is_some() {
        return None;
    }
    Some((major, minor, patch))
}

/// Gunluk denetim kilidi icin unix zaman damgasi (saniye)
pub fn unix_now() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}

// ---- Unit Tests ----

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_version_valid() {
        assert_eq!(parse_version("v0.5.0"), Some((0, 5, 0)));
        assert_eq!(parse_version("0.4.0"), Some((0, 4, 0)));
        assert_eq!(parse_version("  v1.2.3 "), Some((1, 2, 3)));
        assert_eq!(parse_version("10.20.30"), Some((10, 20, 30)));
    }

    #[test]
    fn parse_version_invalid() {
        assert_eq!(parse_version("abc"), None);
        assert_eq!(parse_version("1.2"), None);
        assert_eq!(parse_version("1.2.3.4"), None);
        assert_eq!(parse_version(""), None);
        assert_eq!(parse_version("v1.x.0"), None);
    }

    #[test]
    fn version_ordering() {
        // tuple karsilastirmasi: cift haneli bilesenler dogru siralanir
        assert!(parse_version("0.10.0") > parse_version("0.9.9"));
        assert!(parse_version("v0.5.0") > parse_version("0.4.9"));
        assert!(parse_version("1.0.0") > parse_version("0.99.99"));
        assert_eq!(parse_version("v0.4.0"), parse_version("0.4.0"));
    }

    #[test]
    fn release_json_parsed_with_assets() {
        let body = r#"{
            "tag_name": "v0.5.0",
            "html_url": "https://github.com/Toxpox/MouseDrive/releases/tag/v0.5.0",
            "assets": [
                {"name": "MouseDrive-v0.5.0-windows-x64.zip",
                 "browser_download_url": "https://github.com/dl/MouseDrive-v0.5.0-windows-x64.zip"},
                {"name": "SHA256SUMS.txt",
                 "browser_download_url": "https://github.com/dl/SHA256SUMS.txt"}
            ]
        }"#;
        let info = parse_release_json(body).unwrap();
        assert_eq!(info.version, "v0.5.0");
        assert!(info.html_url.ends_with("/v0.5.0"));
        assert_eq!(info.zip_name, "MouseDrive-v0.5.0-windows-x64.zip");
        assert!(info.auto_installable());
    }

    #[test]
    fn release_json_without_assets_falls_back() {
        // elle yuklenmis eski release: varlik adlandirmasi standart disi
        let body = r#"{
            "tag_name": "v0.4.0",
            "html_url": "https://github.com/Toxpox/MouseDrive/releases/tag/v0.4.0",
            "assets": [{"name": "release.zip", "browser_download_url": "https://x/release.zip"}]
        }"#;
        let info = parse_release_json(body).unwrap();
        assert!(!info.auto_installable());
    }

    #[test]
    fn release_json_malformed() {
        assert_eq!(parse_release_json("not json"), None);
        assert_eq!(parse_release_json("{}"), None);
        assert_eq!(parse_release_json(r#"{"tag_name": 5}"#), None);
    }

    #[test]
    fn sha256_known_vector() {
        // SHA-256("") standart test vektoru
        assert_eq!(
            sha256_hex(b""),
            "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"
        );
        assert_eq!(
            sha256_hex(b"abc"),
            "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
        );
    }

    #[test]
    fn sha256sums_parsing() {
        let text = "abc123  MouseDrive-v0.5.0-windows-x64.zip\r\ndef456  *other.zip\n";
        assert_eq!(
            parse_sha256sums(text, "MouseDrive-v0.5.0-windows-x64.zip"),
            Some("abc123".to_string())
        );
        // '*' onekli (binary mod) ad da eslesir
        assert_eq!(parse_sha256sums(text, "other.zip"), Some("def456".to_string()));
        assert_eq!(parse_sha256sums(text, "yok.zip"), None);
        assert_eq!(parse_sha256sums("", "x.zip"), None);
    }

    #[test]
    fn extract_exe_from_zip() {
        use std::io::Write;
        use zip::write::SimpleFileOptions;

        let mut buf = std::io::Cursor::new(Vec::new());
        {
            let mut zw = zip::ZipWriter::new(&mut buf);
            let opts = SimpleFileOptions::default();
            zw.start_file("README.md", opts).unwrap();
            zw.write_all(b"readme").unwrap();
            zw.start_file("mousedrive.exe", opts).unwrap();
            zw.write_all(b"FAKE_EXE_BYTES").unwrap();
            zw.finish().unwrap();
        }
        let bytes = buf.into_inner();
        assert_eq!(extract_exe(&bytes).unwrap(), b"FAKE_EXE_BYTES");
    }

    #[test]
    fn extract_exe_missing_or_traversal() {
        use std::io::Write;
        use zip::write::SimpleFileOptions;

        let mut buf = std::io::Cursor::new(Vec::new());
        {
            let mut zw = zip::ZipWriter::new(&mut buf);
            let opts = SimpleFileOptions::default();
            // path traversal girisimi: yok sayilmali
            zw.start_file("../mousedrive.exe", opts).unwrap();
            zw.write_all(b"EVIL").unwrap();
            zw.finish().unwrap();
        }
        let bytes = buf.into_inner();
        assert!(extract_exe(&bytes).is_err());
        assert!(extract_exe(b"not a zip").is_err());
    }
}
