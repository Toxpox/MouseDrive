// MouseDrive v0.0.1 alpha /w Rust
// Toxpox -- GitHub.com/Toxpox

use std::thread;
use std::time::Duration;
use std::sync::atomic::{AtomicI32, AtomicBool, Ordering};

// Genel değişkenler
static MOUSE_X: AtomicI32 = AtomicI32::new(0);
static SOL_TIK: AtomicBool = AtomicBool::new(false);
static SAG_TIK: AtomicBool = AtomicBool::new(false);
static CALIS: AtomicBool = AtomicBool::new(true);

// vJoy fonksiyonlari
type VJoyEnabled = unsafe extern "C" fn() -> bool;
type AcquireVJD = unsafe extern "C" fn(u32) -> bool;
type RelinquishVJD = unsafe extern "C" fn(u32);
type SetAxis = unsafe extern "C" fn(i32, u32, u32) -> bool;
type ResetVJD = unsafe extern "C" fn(u32) -> bool;

// Axis kodlari
const AXIS_X: u32 = 0x30;  // direksiyon
const AXIS_Y: u32 = 0x31;  // gaz
const AXIS_RZ: u32 = 0x35; // fren

fn main() {
    println!("MouseDrive v0.0.1 alpha basliyor...");
    println!("Cikmak icin Ctrl+C basin");
    
    // vJoy bagla
    let vjoy = unsafe { libloading::Library::new("vJoyInterface.dll") };
    
    if vjoy.is_err() {
        println!("HATA: vJoyInterface.dll bulunamadi!");
        println!("vJoy Driver yüklü mü kontrol edin, yüklü ise vJoyInterface.dll dosyasini bu dosya ile ayni klasore tasiyin.");
        return;
    }
    
    let vjoy = vjoy.unwrap();
    
    // vJoy fonksiyonlarini al
    let vjoy_enabled: VJoyEnabled;
    let acquire: AcquireVJD;
    let relinquish: RelinquishVJD;
    let set_axis: SetAxis;
    let reset: ResetVJD;
    
    unsafe {
        vjoy_enabled = *vjoy.get(b"vJoyEnabled").unwrap();
        acquire = *vjoy.get(b"AcquireVJD").unwrap();
        relinquish = *vjoy.get(b"RelinquishVJD").unwrap();
        set_axis = *vjoy.get(b"SetAxis").unwrap();
        reset = *vjoy.get(b"ResetVJD").unwrap();
    }
    
    // vJoy aktif kontrolu
    unsafe {
        if !vjoy_enabled() {
            println!("HATA: vJoy aktif degil!");
            return;
        }
    }
    
    println!("vJoy bulundu!");
    
    // Device 1'i al
    let device = 1;
    unsafe {
        if !acquire(device) {
            println!("HATA: vJoy device alinamadi!");
            return;
        }
        reset(device);
    }
    
    println!("vJoy device {} baglandi!", device);
    
    // Mouse input thread'i baslat
    let input_thread = thread::spawn(|| {
        mouse_input_dongusu();
    });
    
    // Direksiyon pozisyonu (ortada basla)
    let mut direksiyon: i32 = 16383; // orta nokta
    let hassasiyet = 50; // mouse hassasiyeti
    
    println!("W! Sol tik=gaz, Sag tik=fren, Mouse=direksiyon");
    
    // Ana dongu
    while CALIS.load(Ordering::Relaxed) {
        // Mouse delta'yi al ve sifirla
        let dx = MOUSE_X.swap(0, Ordering::Relaxed);
        
        // Direksiyonu guncelle
        direksiyon = direksiyon + (dx * hassasiyet);
        
        // Sinirla (0 - 32767)
        if direksiyon < 0 {
            direksiyon = 0;
        }
        if direksiyon > 32767 {
            direksiyon = 32767;
        }
        
        // Gaz (sol tik basiliysa tam gaz)
        let gaz: i32;
        if SOL_TIK.load(Ordering::Relaxed) {
            gaz = 32767;
        } else {
            gaz = 0;
        }
        
        // Fren (sag tik basiliysa tam fren)
        let fren: i32;
        if SAG_TIK.load(Ordering::Relaxed) {
            fren = 32767;
        } else {
            fren = 0;
        }
        
        // vJoy'a gonder
        unsafe {
            set_axis(direksiyon, device, AXIS_X);
            set_axis(gaz, device, AXIS_Y);
            set_axis(fren, device, AXIS_RZ);
        }
        
        thread::sleep(Duration::from_millis(16));
    }
    
    // Kapanirken vJoy'u serbest birak
    println!("Kapatiliyor...");
    unsafe {
        reset(device);
        relinquish(device);
    }
    
    let _ = input_thread.join();
}

fn mouse_input_dongusu() {
    use windows::Win32::UI::WindowsAndMessaging::*;
    use windows::Win32::System::LibraryLoader::GetModuleHandleW;
    use windows::Win32::UI::Input::*;
    use windows::core::PCWSTR;
    
    unsafe {
        // Pencere sinifi olustur
        let sinif_adi: Vec<u16> = "MouseDriveInput\0".encode_utf16().collect();
        
        let wc = WNDCLASSW {
            lpfnWndProc: Some(pencere_proc),
            hInstance: GetModuleHandleW(None).unwrap().into(),
            lpszClassName: PCWSTR(sinif_adi.as_ptr()),
            ..Default::default()
        };
        
        let _ = RegisterClassW(&wc);
        
        // Gizli pencere olustur
        let hwnd = CreateWindowExW(
            WINDOW_EX_STYLE(0),
            PCWSTR(sinif_adi.as_ptr()),
            PCWSTR::null(),
            WS_POPUP,
            0, 0, 0, 0,
            None,
            None,
            wc.hInstance,
            None,
        ).unwrap();
        
        // Raw input kaydet
        let rid = RAWINPUTDEVICE {
            usUsagePage: 0x01,
            usUsage: 0x02,
            dwFlags: RIDEV_INPUTSINK,
            hwndTarget: hwnd,
        };
        
        RegisterRawInputDevices(
            &[rid],
            std::mem::size_of::<RAWINPUTDEVICE>() as u32
        ).unwrap();
        
        // Mesaj dongusu
        let mut msg = MSG::default();
        while GetMessageW(&mut msg, None, 0, 0).as_bool() {
            let _ = TranslateMessage(&msg);
            let _ = DispatchMessageW(&msg);
        }
    }
}

unsafe extern "system" fn pencere_proc(
    hwnd: windows::Win32::Foundation::HWND,
    msg: u32,
    wparam: windows::Win32::Foundation::WPARAM,
    lparam: windows::Win32::Foundation::LPARAM,
) -> windows::Win32::Foundation::LRESULT {
    use windows::Win32::UI::WindowsAndMessaging::*;
    use windows::Win32::UI::Input::*;
    use std::ffi::c_void;
    
    const WM_INPUT: u32 = 0x00FF;
    
    if msg == WM_INPUT {
        // Raw input verisini al
        let mut boyut: u32 = 0;
        GetRawInputData(
            HRAWINPUT(lparam.0 as *mut c_void),
            RID_INPUT,
            None,
            &mut boyut,
            std::mem::size_of::<RAWINPUTHEADER>() as u32,
        );
        
        if boyut > 0 {
            let mut buffer = vec![0u8; boyut as usize];
            let sonuc = GetRawInputData(
                HRAWINPUT(lparam.0 as *mut c_void),
                RID_INPUT,
                Some(buffer.as_mut_ptr() as *mut c_void),
                &mut boyut,
                std::mem::size_of::<RAWINPUTHEADER>() as u32,
            );
            
            if sonuc == boyut {
                let raw = &*(buffer.as_ptr() as *const RAWINPUT);
                
                // Mouse kontrolu
                if raw.header.dwType == RIM_TYPEMOUSE.0 {
                    let mouse = &raw.data.mouse;
                    
                    // Mouse hareketi
                    let dx = mouse.lLastX;
                    MOUSE_X.fetch_add(dx, Ordering::Relaxed);
                    
                    // Buton durumu
                    let butonlar = mouse.Anonymous.Anonymous.usButtonFlags;
                    
                    // Sol tik
                    if butonlar & 0x0001 != 0 { // basma
                        SOL_TIK.store(true, Ordering::Relaxed);
                    }
                    if butonlar & 0x0002 != 0 { // birakma
                        SOL_TIK.store(false, Ordering::Relaxed);
                    }
                    
                    // Sag tik
                    if butonlar & 0x0004 != 0 { // basma
                        SAG_TIK.store(true, Ordering::Relaxed);
                    }
                    if butonlar & 0x0008 != 0 { // birakma
                        SAG_TIK.store(false, Ordering::Relaxed);
                    }
                }
            }
        }
        
        return windows::Win32::Foundation::LRESULT(0);
    }
    
    DefWindowProcW(hwnd, msg, wparam, lparam)
}
