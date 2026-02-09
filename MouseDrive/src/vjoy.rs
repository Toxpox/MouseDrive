use libloading::Library;

pub const HID_USAGE_X: u32 = 0x30;   // direksiyon
pub const HID_USAGE_Y: u32 = 0x31;   // gaz
pub const HID_USAGE_RZ: u32 = 0x35;  // fren

pub const AXIS_MAX: i32 = 32767;
pub const AXIS_MIN: i32 = 0;
pub const AXIS_CENTER: i32 = 16383;

#[repr(u32)]
#[derive(Debug, PartialEq)]
pub enum VjdStat { Own = 0, Free = 1, Busy = 2, Miss = 3, Unkn = 4 }

// vJoy SDK fonksiyonlari __cdecl kullaniyor (x64'te fark etmez ama dogrusu bu)
type FnVJoyEnabled   = unsafe extern "C" fn() -> bool;
type FnGetVJDStatus  = unsafe extern "C" fn(u32) -> u32;
type FnAcquireVJD    = unsafe extern "C" fn(u32) -> bool;
type FnRelinquishVJD = unsafe extern "C" fn(u32);
type FnSetAxis       = unsafe extern "C" fn(i32, u32, u32) -> bool;
type FnSetBtn        = unsafe extern "C" fn(bool, u32, u8) -> bool;
type FnResetVJD      = unsafe extern "C" fn(u32) -> bool;

pub struct VJoyApi {
    _lib: Library,
    vjoy_enabled: FnVJoyEnabled,
    get_vjd_status: FnGetVJDStatus,
    acquire_vjd: FnAcquireVJD,
    relinquish_vjd: FnRelinquishVJD,
    set_axis: FnSetAxis,
    set_btn: FnSetBtn,
    reset_vjd: FnResetVJD,
}

impl VJoyApi {
    pub fn load() -> Option<Self> {
        unsafe {
            let lib = Library::new("vJoyInterface.dll").ok()?;

            // lib'den symbol cek, transmute ile fn pointer'a cevir.
            // lib alive kaldigi surece pointer'lar gecerli.
            macro_rules! sym {
                ($name:literal, $ty:ty) => {
                    std::mem::transmute::<_, $ty>(
                        lib.get::<$ty>($name).ok()?.into_raw().into_raw()
                    )
                };
            }

            let api = Self {
                vjoy_enabled:   sym!(b"vJoyEnabled",   FnVJoyEnabled),
                get_vjd_status: sym!(b"GetVJDStatus",  FnGetVJDStatus),
                acquire_vjd:    sym!(b"AcquireVJD",    FnAcquireVJD),
                relinquish_vjd: sym!(b"RelinquishVJD", FnRelinquishVJD),
                set_axis:       sym!(b"SetAxis",       FnSetAxis),
                set_btn:        sym!(b"SetBtn",        FnSetBtn),
                reset_vjd:      sym!(b"ResetVJD",      FnResetVJD),
                _lib: lib,
            };
            Some(api)
        }
    }

    pub fn is_enabled(&self) -> bool         { unsafe { (self.vjoy_enabled)() } }
    pub fn acquire(&self, dev: u32) -> bool   { unsafe { (self.acquire_vjd)(dev) } }
    pub fn relinquish(&self, dev: u32)        { unsafe { (self.relinquish_vjd)(dev) } }
    pub fn reset(&self, dev: u32) -> bool     { unsafe { (self.reset_vjd)(dev) } }

    pub fn set_axis(&self, val: i32, dev: u32, usage: u32) -> bool {
        unsafe { (self.set_axis)(val, dev, usage) }
    }
    pub fn set_btn(&self, on: bool, dev: u32, btn: u8) -> bool {
        unsafe { (self.set_btn)(on, dev, btn) }
    }

    pub fn get_status(&self, dev: u32) -> VjdStat {
        match unsafe { (self.get_vjd_status)(dev) } {
            0 => VjdStat::Own,  1 => VjdStat::Free,
            2 => VjdStat::Busy, 3 => VjdStat::Miss,
            _ => VjdStat::Unkn,
        }
    }
}
