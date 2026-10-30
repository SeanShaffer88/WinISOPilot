//! Host checks: elevation, DISM/ADK tools, free space.

use super::wide;
use crate::HostInfo;
use std::path::{Path, PathBuf};
use std::ptr;
use windows_sys::Win32::Foundation::{CloseHandle, HANDLE};
use windows_sys::Win32::Security::{GetTokenInformation, TokenElevation, TOKEN_ELEVATION, TOKEN_QUERY};
use windows_sys::Win32::Storage::FileSystem::{
    GetDiskFreeSpaceExW, GetFileVersionInfoSizeW, GetFileVersionInfoW, VerQueryValueW, VS_FIXEDFILEINFO,
};
use windows_sys::Win32::System::Registry::{RegGetValueW, HKEY_LOCAL_MACHINE, RRF_RT_REG_SZ};
use windows_sys::Win32::System::SystemInformation::GetSystemDirectoryW;
use windows_sys::Win32::System::Threading::{GetCurrentProcess, OpenProcessToken};

pub fn is_admin() -> bool {
    let mut token: HANDLE = ptr::null_mut();
    // SAFETY: pseudo-handle for the current process; out-pointer.
    if unsafe { OpenProcessToken(GetCurrentProcess(), TOKEN_QUERY, &mut token) } == 0 {
        return false;
    }
    let mut elev = TOKEN_ELEVATION { TokenIsElevated: 0 };
    let mut len = 0u32;
    // SAFETY: buffer is a TOKEN_ELEVATION of the stated size.
    let ok = unsafe {
        GetTokenInformation(
            token,
            TokenElevation,
            (&mut elev as *mut TOKEN_ELEVATION).cast(),
            std::mem::size_of::<TOKEN_ELEVATION>() as u32,
            &mut len,
        )
    };
    // SAFETY: token from OpenProcessToken.
    unsafe { CloseHandle(token) };
    ok != 0 && elev.TokenIsElevated != 0
}

pub fn system32() -> Option<PathBuf> {
    let mut buf = [0u16; 260];
    // SAFETY: buffer length passed.
    let n = unsafe { GetSystemDirectoryW(buf.as_mut_ptr(), buf.len() as u32) } as usize;
    (n > 0 && n < buf.len()).then(|| PathBuf::from(String::from_utf16_lossy(&buf[..n])))
}

/// Default Windows ADK Deployment Tools folder for this architecture.
pub fn adk_root() -> PathBuf {
    let pf = std::env::var_os("ProgramFiles(x86)")
        .or_else(|| std::env::var_os("ProgramFiles"))
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from(r"C:\Program Files (x86)"));
    let arch = if cfg!(target_arch = "aarch64") { "arm64" } else { "amd64" };
    pf.join(r"Windows Kits\10\Assessment and Deployment Kit\Deployment Tools").join(arch)
}

fn os_build() -> u32 {
    let key = wide(r"SOFTWARE\Microsoft\Windows NT\CurrentVersion");
    let name = wide("CurrentBuildNumber");
    let mut buf = [0u16; 32];
    let mut size = (buf.len() * 2) as u32;
    // SAFETY: buffer and size describe the same memory.
    let rc = unsafe {
        RegGetValueW(
            HKEY_LOCAL_MACHINE,
            key.as_ptr(),
            name.as_ptr(),
            RRF_RT_REG_SZ,
            ptr::null_mut(),
            buf.as_mut_ptr().cast(),
            &mut size,
        )
    };
    if rc != 0 {
        return 0;
    }
    let s = String::from_utf16_lossy(&buf);
    s.trim_end_matches('\0').trim().parse().unwrap_or(0)
}

/// File version "a.b.c.d" of an executable or DLL.
pub fn file_version(path: &Path) -> Option<(u32, u32, u32, u32)> {
    let w = wide(path);
    let mut handle = 0u32;
    // SAFETY: valid path.
    let size = unsafe { GetFileVersionInfoSizeW(w.as_ptr(), &mut handle) };
    if size == 0 {
        return None;
    }
    let mut data = vec![0u8; size as usize];
    // SAFETY: buffer of `size` bytes.
    if unsafe { GetFileVersionInfoW(w.as_ptr(), 0, size, data.as_mut_ptr().cast()) } == 0 {
        return None;
    }
    let root = wide("\\");
    let mut info: *mut VS_FIXEDFILEINFO = ptr::null_mut();
    let mut len = 0u32;
    // SAFETY: `data` holds the version resource; VerQueryValue returns a
    // pointer into it.
    let ok = unsafe {
        VerQueryValueW(data.as_ptr().cast(), root.as_ptr(), (&mut info as *mut *mut VS_FIXEDFILEINFO).cast(), &mut len)
    };
    if ok == 0 || info.is_null() {
        return None;
    }
    // SAFETY: pointer into `data`, which is still alive.
    let fi = unsafe { &*info };
    Some((fi.dwFileVersionMS >> 16, fi.dwFileVersionMS & 0xFFFF, fi.dwFileVersionLS >> 16, fi.dwFileVersionLS & 0xFFFF))
}

pub fn free_space(path: &Path) -> Option<u64> {
    // Walk up to an existing folder.
    let mut p = path.to_path_buf();
    while !p.exists() {
        p = p.parent()?.to_path_buf();
    }
    let w = wide(&p);
    let mut free = 0u64;
    // SAFETY: out-pointer for caller-available bytes.
    let ok = unsafe { GetDiskFreeSpaceExW(w.as_ptr(), &mut free, ptr::null_mut(), ptr::null_mut()) };
    (ok != 0).then_some(free)
}

pub fn find_oscdimg(dism_folder: Option<&Path>) -> Option<PathBuf> {
    let mut c = Vec::new();
    if let Some(f) = dism_folder {
        c.push(f.join("oscdimg.exe"));
        if let Some(parent) = f.parent() {
            c.push(parent.join(r"Oscdimg\oscdimg.exe"));
        }
    }
    c.push(adk_root().join(r"Oscdimg\oscdimg.exe"));
    c.into_iter().find(|p| p.is_file())
}

pub fn probe(dism_folder: Option<&Path>) -> HostInfo {
    let mut h = HostInfo { is_admin: is_admin(), os_build: os_build(), ..Default::default() };
    h.dism_api = super::dism::find_dll(dism_folder);
    if let Some(dll) = &h.dism_api {
        let dir = dll.parent().map(Path::to_path_buf).unwrap_or_default();
        let exe = dir.join("dism.exe");
        h.dism_exe =
            if exe.is_file() { Some(exe) } else { system32().map(|s| s.join("dism.exe")).filter(|p| p.is_file()) };
        if let Some((a, b, c, d)) = file_version(dll) {
            h.dism_version = format!("{a}.{b}.{c}.{d}");
            h.dism_build = c;
        }
    } else {
        h.notes.push(
            "DISM wasn't found. Install the Windows ADK Deployment Tools or choose its folder in Settings.".into(),
        );
    }
    h.oscdimg = find_oscdimg(dism_folder);
    if h.oscdimg.is_none() {
        h.notes.push(
            "oscdimg.exe (Windows ADK Deployment Tools) wasn't found, so the result will be a media folder instead of an ISO."
                .into(),
        );
    }
    if !h.is_admin {
        h.notes.push("WinISOPilot isn't running as administrator. Mounting images needs administrator rights.".into());
    }
    h
}
