//! DISM API bindings, loaded at runtime from dismapi.dll.
//!
//! Struct layouts follow DismApi.h (`#pragma pack(push, 1)`), checked against
//! the mingw-w64 dismapi.h header. The DLL is loaded dynamically so the ADK
//! copy can be preferred over the system copy.

use super::{from_wide_ptr, wide};
use crate::{EngineError, Job, Result};
use std::ffi::c_void;
use std::path::{Path, PathBuf};
use std::ptr;
use std::sync::{Mutex, OnceLock};
use windows_sys::Win32::Foundation::{CloseHandle, FreeLibrary, HANDLE, HMODULE};
use windows_sys::Win32::System::LibraryLoader::{
    GetProcAddress, LoadLibraryExW, LOAD_LIBRARY_SEARCH_DEFAULT_DIRS, LOAD_LIBRARY_SEARCH_DLL_LOAD_DIR,
};
use windows_sys::Win32::System::Threading::{CreateEventW, SetEvent};
use wip_core::inventory::{NamedState, PackageState};
use wip_core::ImageSummary;

pub type DismSession = u32;
type HResult = i32;
type ProgressFn = unsafe extern "system" fn(u32, u32, *mut c_void);

const DISM_MOUNT_READWRITE: u32 = 0;
const DISM_COMMIT_IMAGE: u32 = 0;
const DISM_DISCARD_IMAGE: u32 = 1;
const DISM_LOG_ERRORS_WARNINGS: i32 = 1;
const DISM_IMAGE_INDEX: i32 = 0;
const DISM_PACKAGE_NONE: i32 = 0;
/// DISMAPI_E_DISMAPI_ALREADY_INITIALIZED
const ALREADY_INITIALIZED: HResult = 0xC0040001u32 as i32;

#[repr(C, packed)]
struct DismString {
    value: *const u16,
}

#[repr(C, packed)]
struct DismImageInfo {
    image_type: i32,
    image_index: u32,
    image_name: *const u16,
    image_description: *const u16,
    image_size: u64,
    architecture: u32,
    product_name: *const u16,
    edition_id: *const u16,
    installation_type: *const u16,
    hal: *const u16,
    product_type: *const u16,
    product_suite: *const u16,
    major_version: u32,
    minor_version: u32,
    build: u32,
    sp_build: u32,
    sp_level: u32,
    bootable: i32,
    system_root: *const u16,
    language: *const DismString,
    language_count: u32,
    default_language_index: u32,
    customized_info: *const c_void,
}

#[repr(C, packed)]
struct DismNameState {
    name: *const u16,
    state: i32,
}

#[allow(non_snake_case)]
struct Api {
    _module: Module,
    Initialize: unsafe extern "system" fn(i32, *const u16, *const u16) -> HResult,
    GetImageInfo: unsafe extern "system" fn(*const u16, *mut *mut DismImageInfo, *mut u32) -> HResult,
    MountImage: unsafe extern "system" fn(
        *const u16,
        *const u16,
        u32,
        *const u16,
        i32,
        u32,
        HANDLE,
        Option<ProgressFn>,
        *mut c_void,
    ) -> HResult,
    UnmountImage: unsafe extern "system" fn(*const u16, u32, HANDLE, Option<ProgressFn>, *mut c_void) -> HResult,
    OpenSession: unsafe extern "system" fn(*const u16, *const u16, *const u16, *mut DismSession) -> HResult,
    CloseSession: unsafe extern "system" fn(DismSession) -> HResult,
    GetLastErrorMessage: unsafe extern "system" fn(*mut *mut DismString) -> HResult,
    Delete: unsafe extern "system" fn(*mut c_void) -> HResult,
    GetFeatures: unsafe extern "system" fn(DismSession, *const u16, i32, *mut *mut DismNameState, *mut u32) -> HResult,
    GetCapabilities: unsafe extern "system" fn(DismSession, *mut *mut DismNameState, *mut u32) -> HResult,
    EnableFeature: unsafe extern "system" fn(
        DismSession,
        *const u16,
        *const u16,
        i32,
        i32,
        *const *const u16,
        u32,
        i32,
        HANDLE,
        Option<ProgressFn>,
        *mut c_void,
    ) -> HResult,
    DisableFeature: unsafe extern "system" fn(
        DismSession,
        *const u16,
        *const u16,
        i32,
        HANDLE,
        Option<ProgressFn>,
        *mut c_void,
    ) -> HResult,
    RemoveCapability:
        unsafe extern "system" fn(DismSession, *const u16, HANDLE, Option<ProgressFn>, *mut c_void) -> HResult,
    /// DismAddDriver(Session, DriverPath, ForceUnsigned)
    AddDriver: unsafe extern "system" fn(DismSession, *const u16, i32) -> HResult,
}

struct Module(HMODULE);
// SAFETY: a module handle is a process-wide value; FreeLibrary is only
// called on drop and the Api is never dropped while in use (global).
unsafe impl Send for Module {}
unsafe impl Sync for Module {}
impl Drop for Module {
    fn drop(&mut self) {
        // SAFETY: handle came from LoadLibraryExW.
        unsafe { FreeLibrary(self.0) };
    }
}

// SAFETY: function pointers into a loaded DLL are plain addresses.
unsafe impl Send for Api {}
unsafe impl Sync for Api {}

static API: OnceLock<std::result::Result<Api, String>> = OnceLock::new();
/// DISM API calls on one image must not overlap.
static LOCK: Mutex<()> = Mutex::new(());

macro_rules! sym {
    ($m:expr, $name:literal) => {{
        // SAFETY: name is a NUL-terminated ASCII literal; transmuting the
        // FARPROC to the documented signature is how dynamic loading works.
        let p = unsafe { GetProcAddress($m, concat!($name, "\0").as_ptr()) };
        match p {
            Some(f) => unsafe { std::mem::transmute::<unsafe extern "system" fn() -> isize, _>(f) },
            None => return Err(format!("dismapi.dll is missing {}", $name)),
        }
    }};
}

#[allow(clippy::missing_transmute_annotations)]
fn load(dll: &Path, log_file: &Path) -> std::result::Result<Api, String> {
    let w = wide(dll);
    // SAFETY: valid wide path; flags restrict dependency search to the DLL's
    // own folder and default safe directories.
    let m = unsafe {
        LoadLibraryExW(w.as_ptr(), ptr::null_mut(), LOAD_LIBRARY_SEARCH_DLL_LOAD_DIR | LOAD_LIBRARY_SEARCH_DEFAULT_DIRS)
    };
    if m.is_null() {
        return Err(format!("Couldn't load {}: {}", dll.display(), std::io::Error::last_os_error()));
    }
    let module = Module(m);
    let api = Api {
        Initialize: sym!(m, "DismInitialize"),
        GetImageInfo: sym!(m, "DismGetImageInfo"),
        MountImage: sym!(m, "DismMountImage"),
        UnmountImage: sym!(m, "DismUnmountImage"),
        OpenSession: sym!(m, "DismOpenSession"),
        CloseSession: sym!(m, "DismCloseSession"),
        GetLastErrorMessage: sym!(m, "DismGetLastErrorMessage"),
        Delete: sym!(m, "DismDelete"),
        GetFeatures: sym!(m, "DismGetFeatures"),
        GetCapabilities: sym!(m, "DismGetCapabilities"),
        EnableFeature: sym!(m, "DismEnableFeature"),
        DisableFeature: sym!(m, "DismDisableFeature"),
        RemoveCapability: sym!(m, "DismRemoveCapability"),
        AddDriver: sym!(m, "DismAddDriver"),
        _module: module,
    };
    if let Some(dir) = log_file.parent() {
        let _ = std::fs::create_dir_all(dir);
    }
    let lw = wide(log_file);
    // SAFETY: valid wide strings; scratch NULL = default temp folder.
    let hr = unsafe { (api.Initialize)(DISM_LOG_ERRORS_WARNINGS, lw.as_ptr(), ptr::null()) };
    if hr < 0 && hr != ALREADY_INITIALIZED {
        return Err(format!("DismInitialize failed (0x{:08X}). Run WinISOPilot as administrator.", hr as u32));
    }
    Ok(api)
}

fn api(dll: &Path) -> Result<&'static Api> {
    let log = crate::app_data_dir().join("logs").join("dism-api.log");
    API.get_or_init(|| load(dll, &log)).as_ref().map_err(|e| EngineError::new("WIP-E010", e.clone()))
}

fn last_error(a: &Api, hr: HResult, what: &str) -> EngineError {
    let mut msg: *mut DismString = ptr::null_mut();
    // SAFETY: out-pointer; DISM allocates and we free with DismDelete.
    let detail = unsafe {
        if (a.GetLastErrorMessage)(&mut msg) >= 0 && !msg.is_null() {
            let s = from_wide_ptr((*msg).value);
            (a.Delete)(msg.cast());
            s
        } else {
            String::new()
        }
    };
    let code = hr as u32;
    let hint = match code {
        0x8007_0005 => " Access denied: run WinISOPilot as administrator.",
        0x8007_0070 => " Not enough disk space.",
        0xC142_0127 => " The image is already mounted somewhere else.",
        0x8004_1010 => " This DISM version can't service this image; install the Windows ADK that matches it.",
        _ => "",
    };
    EngineError::new("WIP-E011", format!("{what} failed (0x{code:08X}). {}{hint}", detail.trim()))
}

/// Cancel event + progress forwarding for one DISM call.
struct Progress<'a> {
    job: &'a Job,
    event: HANDLE,
    signalled: bool,
}

impl<'a> Progress<'a> {
    fn new(job: &'a Job) -> Progress<'a> {
        // SAFETY: anonymous manual-reset event.
        let event = unsafe { CreateEventW(ptr::null(), 1, 0, ptr::null()) };
        Progress { job, event, signalled: false }
    }
}

impl Drop for Progress<'_> {
    fn drop(&mut self) {
        if !self.event.is_null() {
            // SAFETY: handle from CreateEventW.
            unsafe { CloseHandle(self.event) };
        }
    }
}

unsafe extern "system" fn on_progress(current: u32, total: u32, user: *mut c_void) {
    if user.is_null() {
        return;
    }
    // SAFETY: `user` is the &mut Progress passed by the caller below and
    // outlives the DISM call that invokes this callback.
    let p = unsafe { &mut *(user as *mut Progress) };
    if total > 0 {
        p.job.progress(current as f32 / total as f32);
    }
    if p.job.cancelled() && !p.signalled && !p.event.is_null() {
        p.signalled = true;
        // SAFETY: valid event handle.
        unsafe { SetEvent(p.event) };
    }
}

fn arch_name(a: u32) -> &'static str {
    match a {
        0 => "x86",
        5 => "ARM",
        9 => "x64",
        12 => "ARM64",
        _ => "unknown",
    }
}

pub struct Dism {
    api: &'static Api,
}

impl Dism {
    pub fn open(dll: &Path) -> Result<Dism> {
        Ok(Dism { api: api(dll)? })
    }

    pub fn image_info(&self, file: &Path) -> Result<Vec<ImageSummary>> {
        let _g = LOCK.lock().unwrap_or_else(|e| e.into_inner());
        let w = wide(file);
        let mut info: *mut DismImageInfo = ptr::null_mut();
        let mut count = 0u32;
        // SAFETY: valid path and out-pointers.
        let hr = unsafe { (self.api.GetImageInfo)(w.as_ptr(), &mut info, &mut count) };
        if hr < 0 {
            return Err(last_error(self.api, hr, "Reading the editions"));
        }
        let mut out = Vec::with_capacity(count as usize);
        for i in 0..count as usize {
            // SAFETY: DISM returned `count` packed structs at `info`.
            let r = unsafe { ptr::read_unaligned(info.add(i)) };
            let mut languages = Vec::new();
            for l in 0..r.language_count as usize {
                // SAFETY: `language_count` entries at `language`.
                let s = unsafe { ptr::read_unaligned(r.language.add(l)) };
                languages.push(unsafe { from_wide_ptr(s.value) });
            }
            // SAFETY: strings owned by the DISM structure until DismDelete.
            unsafe {
                out.push(ImageSummary {
                    index: r.image_index,
                    name: from_wide_ptr(r.image_name),
                    description: from_wide_ptr(r.image_description),
                    edition_id: from_wide_ptr(r.edition_id),
                    installation_type: from_wide_ptr(r.installation_type),
                    architecture: arch_name(r.architecture).into(),
                    major: r.major_version,
                    minor: r.minor_version,
                    build: r.build,
                    sp_build: r.sp_build,
                    size_bytes: r.image_size,
                    languages,
                });
            }
        }
        if !info.is_null() {
            // SAFETY: allocated by DISM.
            unsafe { (self.api.Delete)(info.cast()) };
        }
        Ok(out)
    }

    pub fn mount(&self, file: &Path, index: u32, dir: &Path, job: &Job) -> Result<()> {
        let _g = LOCK.lock().unwrap_or_else(|e| e.into_inner());
        let (f, d) = (wide(file), wide(dir));
        let mut p = Progress::new(job);
        // SAFETY: valid strings; progress struct outlives the call.
        let hr = unsafe {
            (self.api.MountImage)(
                f.as_ptr(),
                d.as_ptr(),
                index,
                ptr::null(),
                DISM_IMAGE_INDEX,
                DISM_MOUNT_READWRITE,
                p.event,
                Some(on_progress),
                (&mut p as *mut Progress).cast(),
            )
        };
        if hr < 0 {
            return Err(self.cancel_or(hr, job, "Mounting the image"));
        }
        Ok(())
    }

    pub fn unmount(&self, dir: &Path, commit: bool, job: &Job) -> Result<()> {
        let _g = LOCK.lock().unwrap_or_else(|e| e.into_inner());
        let d = wide(dir);
        // Unmount must not be cancelled half-way: use a job-independent progress.
        let mut p = Progress::new(job);
        p.signalled = true;
        let flags = if commit { DISM_COMMIT_IMAGE } else { DISM_DISCARD_IMAGE };
        // SAFETY: as above. The event is never signalled, so unmount runs to completion.
        let hr = unsafe {
            (self.api.UnmountImage)(
                d.as_ptr(),
                flags,
                ptr::null_mut(),
                Some(on_progress),
                (&mut p as *mut Progress).cast(),
            )
        };
        if hr < 0 {
            return Err(last_error(self.api, hr, if commit { "Saving the image" } else { "Unmounting the image" }));
        }
        Ok(())
    }

    fn cancel_or(&self, hr: HResult, job: &Job, what: &str) -> EngineError {
        // ERROR_CANCELLED as HRESULT
        if job.cancelled() || hr as u32 == 0x8007_04C7 {
            EngineError::cancelled()
        } else {
            last_error(self.api, hr, what)
        }
    }

    pub fn session<'a>(&'a self, mount_dir: &Path) -> Result<Session<'a>> {
        let d = wide(mount_dir);
        let mut s: DismSession = 0;
        let _g = LOCK.lock().unwrap_or_else(|e| e.into_inner());
        // SAFETY: valid path, out-pointer.
        let hr = unsafe { (self.api.OpenSession)(d.as_ptr(), ptr::null(), ptr::null(), &mut s) };
        if hr < 0 {
            return Err(last_error(self.api, hr, "Opening the mounted image"));
        }
        Ok(Session { dism: self, handle: s })
    }
}

pub struct Session<'a> {
    dism: &'a Dism,
    handle: DismSession,
}

impl Drop for Session<'_> {
    fn drop(&mut self) {
        // SAFETY: handle from DismOpenSession.
        unsafe { (self.dism.api.CloseSession)(self.handle) };
    }
}

impl Session<'_> {
    fn list(&self, features: bool) -> Result<Vec<NamedState>> {
        let a = self.dism.api;
        let mut items: *mut DismNameState = ptr::null_mut();
        let mut count = 0u32;
        // SAFETY: out-pointers; DismFeature and DismCapability share layout.
        let hr = unsafe {
            if features {
                (a.GetFeatures)(self.handle, ptr::null(), DISM_PACKAGE_NONE, &mut items, &mut count)
            } else {
                (a.GetCapabilities)(self.handle, &mut items, &mut count)
            }
        };
        if hr < 0 {
            return Err(last_error(a, hr, if features { "Listing features" } else { "Listing capabilities" }));
        }
        let mut out = Vec::with_capacity(count as usize);
        for i in 0..count as usize {
            // SAFETY: `count` packed entries.
            let r = unsafe { ptr::read_unaligned(items.add(i)) };
            out.push(NamedState { name: unsafe { from_wide_ptr(r.name) }, state: PackageState::from_dism(r.state) });
        }
        if !items.is_null() {
            // SAFETY: allocated by DISM.
            unsafe { (a.Delete)(items.cast()) };
        }
        Ok(out)
    }

    pub fn features(&self) -> Result<Vec<NamedState>> {
        self.list(true)
    }

    /// Capabilities exist on Windows 10 1809+; older images return an error,
    /// which is reported as an empty list.
    pub fn capabilities(&self) -> Vec<NamedState> {
        self.list(false).unwrap_or_default()
    }

    pub fn disable_feature(&self, name: &str, remove_payload: bool, job: &Job) -> Result<()> {
        let n = wide(name);
        let mut p = Progress::new(job);
        // SAFETY: valid strings, progress outlives call.
        let hr = unsafe {
            (self.dism.api.DisableFeature)(
                self.handle,
                n.as_ptr(),
                ptr::null(),
                remove_payload as i32,
                p.event,
                Some(on_progress),
                (&mut p as *mut Progress).cast(),
            )
        };
        if hr < 0 {
            return Err(self.dism.cancel_or(hr, job, &format!("Turning off {name}")));
        }
        Ok(())
    }

    pub fn enable_feature(&self, name: &str, source: Option<&Path>, job: &Job) -> Result<()> {
        let n = wide(name);
        let src: Option<Vec<u16>> = source.map(wide);
        let srcs: Vec<*const u16> = src.iter().map(|s| s.as_ptr()).collect();
        let mut p = Progress::new(job);
        // SAFETY: valid strings/array; LimitAccess when a local source is
        // given so DISM doesn't reach Windows Update.
        let hr = unsafe {
            (self.dism.api.EnableFeature)(
                self.handle,
                n.as_ptr(),
                ptr::null(),
                DISM_PACKAGE_NONE,
                source.is_some() as i32,
                if srcs.is_empty() { ptr::null() } else { srcs.as_ptr() },
                srcs.len() as u32,
                1,
                p.event,
                Some(on_progress),
                (&mut p as *mut Progress).cast(),
            )
        };
        if hr < 0 {
            return Err(self.dism.cancel_or(hr, job, &format!("Turning on {name}")));
        }
        Ok(())
    }

    pub fn remove_capability(&self, name: &str, job: &Job) -> Result<()> {
        let n = wide(name);
        let mut p = Progress::new(job);
        // SAFETY: as above.
        let hr = unsafe {
            (self.dism.api.RemoveCapability)(
                self.handle,
                n.as_ptr(),
                p.event,
                Some(on_progress),
                (&mut p as *mut Progress).cast(),
            )
        };
        if hr < 0 {
            return Err(self.dism.cancel_or(hr, job, &format!("Removing {name}")));
        }
        Ok(())
    }
}

impl Session<'_> {
    /// Adds one signed driver package. Unsigned drivers are never forced.
    /// DismAddDriver has no progress callback or cancel event.
    pub fn add_driver(&self, inf: &Path, job: &Job) -> Result<()> {
        let w = wide(inf);
        // SAFETY: valid NUL-terminated path; ForceUnsigned = FALSE.
        let hr = unsafe { (self.dism.api.AddDriver)(self.handle, w.as_ptr(), 0) };
        if hr < 0 {
            let name = inf.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
            return Err(self.dism.cancel_or(hr, job, &format!("Adding driver {name}")));
        }
        Ok(())
    }
}

/// Locates dismapi.dll: user-chosen folder, then the ADK, then System32.
pub fn find_dll(user_folder: Option<&Path>) -> Option<PathBuf> {
    let mut candidates = Vec::new();
    if let Some(f) = user_folder {
        candidates.push(f.join("dismapi.dll"));
    }
    candidates.push(super::host::adk_root().join(r"DISM\dismapi.dll"));
    if let Some(sys) = super::host::system32() {
        candidates.push(sys.join("dismapi.dll"));
    }
    candidates.into_iter().find(|p| p.is_file())
}
