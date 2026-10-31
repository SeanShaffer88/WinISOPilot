//! WinISOPilot core: everything that does not touch Windows APIs.
//!
//! The catalog describes options; an [`inventory::Inventory`] describes what
//! a mounted image actually contains; [`plan`] checks a selection against
//! both and compiles it into concrete [`plan::Op`]s the engine executes.

pub mod boottest;
pub mod catalog;
pub mod diagnostics;
pub mod download;
pub mod drivers;
pub mod footprint;
pub mod fsutil;
pub mod history;
pub mod inventory;
pub mod journal;
pub mod packages;
pub mod plan;
pub mod port;
pub mod raw;
pub mod recipe;
pub mod secureboot;
pub mod settings;
pub mod unattend;
pub mod usb;

pub use catalog::{Action, Catalog, CatalogOption, Category, Hive, Level, RegData, Risk};

use serde::{Deserialize, Serialize};

pub const APP_NAME: &str = "WinISOPilot";
pub const APP_VERSION: &str = env!("CARGO_PKG_VERSION");

/// First Windows 11 build number.
pub const WIN11_FIRST_BUILD: u32 = 22000;
/// First Windows 10 build number.
pub const WIN10_FIRST_BUILD: u32 = 10240;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum WindowsFamily {
    Win10,
    Win11,
}

impl WindowsFamily {
    /// Classifies an image by major version and build. Only client Windows
    /// 10 and 11 are supported; anything else returns `None`.
    pub fn from_version(major: u32, build: u32) -> Option<WindowsFamily> {
        if major != 10 {
            return None;
        }
        if build >= WIN11_FIRST_BUILD {
            Some(WindowsFamily::Win11)
        } else if build >= WIN10_FIRST_BUILD {
            Some(WindowsFamily::Win10)
        } else {
            None
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            WindowsFamily::Win10 => "Windows 10",
            WindowsFamily::Win11 => "Windows 11",
        }
    }
}

/// One edition (image index) inside install.wim / install.esd.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ImageSummary {
    pub index: u32,
    pub name: String,
    pub description: String,
    pub edition_id: String,
    pub installation_type: String,
    pub architecture: String,
    pub major: u32,
    pub minor: u32,
    pub build: u32,
    pub sp_build: u32,
    pub size_bytes: u64,
    pub languages: Vec<String>,
}

impl ImageSummary {
    pub fn family(&self) -> Option<WindowsFamily> {
        WindowsFamily::from_version(self.major, self.build)
    }

    pub fn version_string(&self) -> String {
        format!("{}.{}.{}.{}", self.major, self.minor, self.build, self.sp_build)
    }

    /// Server and WinPE images are not supported targets.
    pub fn is_client(&self) -> bool {
        self.installation_type.eq_ignore_ascii_case("Client")
    }

    pub fn support(&self) -> Result<WindowsFamily, String> {
        if !self.is_client() {
            return Err(format!(
                "{} is a '{}' image; only Windows 10/11 client editions are supported.",
                self.name, self.installation_type
            ));
        }
        self.family().ok_or_else(|| {
            format!("{} is version {}; only Windows 10 and 11 are supported.", self.name, self.version_string())
        })
    }
}

pub fn human_bytes(n: u64) -> String {
    const UNITS: [&str; 5] = ["B", "KB", "MB", "GB", "TB"];
    let mut v = n as f64;
    let mut u = 0;
    while v >= 1024.0 && u < UNITS.len() - 1 {
        v /= 1024.0;
        u += 1;
    }
    if u == 0 {
        format!("{n} B")
    } else {
        format!("{v:.1} {}", UNITS[u])
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn family_classification() {
        assert_eq!(WindowsFamily::from_version(10, 19045), Some(WindowsFamily::Win10));
        assert_eq!(WindowsFamily::from_version(10, 22000), Some(WindowsFamily::Win11));
        assert_eq!(WindowsFamily::from_version(10, 26100), Some(WindowsFamily::Win11));
        assert_eq!(WindowsFamily::from_version(6, 7601), None);
        assert_eq!(WindowsFamily::from_version(10, 9600), None);
    }

    #[test]
    fn bytes() {
        assert_eq!(human_bytes(512), "512 B");
        assert_eq!(human_bytes(1536), "1.5 KB");
        assert_eq!(human_bytes(5 * 1024 * 1024 * 1024), "5.0 GB");
    }
}
