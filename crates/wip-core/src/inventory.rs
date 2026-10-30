//! What a mounted image actually contains, as reported by DISM.

use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum PackageState {
    NotPresent,
    UninstallPending,
    Staged,
    Removed,
    Installed,
    InstallPending,
    Superseded,
    PartiallyInstalled,
    Unknown,
}

impl PackageState {
    /// Maps the DismPackageFeatureState enum value.
    pub fn from_dism(v: i32) -> PackageState {
        match v {
            0 => PackageState::NotPresent,
            1 => PackageState::UninstallPending,
            2 => PackageState::Staged,
            3 => PackageState::Removed,
            4 => PackageState::Installed,
            5 => PackageState::InstallPending,
            6 => PackageState::Superseded,
            7 => PackageState::PartiallyInstalled,
            _ => PackageState::Unknown,
        }
    }

    pub fn is_on(self) -> bool {
        matches!(self, PackageState::Installed | PackageState::InstallPending | PackageState::PartiallyInstalled)
    }

    /// Off but the payload is still present or obtainable.
    pub fn is_off(self) -> bool {
        matches!(
            self,
            PackageState::Staged | PackageState::Removed | PackageState::UninstallPending | PackageState::NotPresent
        )
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct NamedState {
    pub name: String,
    pub state: PackageState,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AppxPackage {
    pub display_name: String,
    pub package_name: String,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Inventory {
    pub appx: Vec<AppxPackage>,
    pub capabilities: Vec<NamedState>,
    pub features: Vec<NamedState>,
    /// Service key names present under ControlSet001\Services.
    pub services: BTreeSet<String>,
    /// Whether the media contains sources\sxs (needed for .NET 3.5).
    pub has_sxs: bool,
}

pub fn starts_with_ci(haystack: &str, prefix: &str) -> bool {
    haystack.len() >= prefix.len() && haystack.as_bytes()[..prefix.len()].eq_ignore_ascii_case(prefix.as_bytes())
}

impl Inventory {
    pub fn appx_matching<'a>(&'a self, prefix: &'a str) -> impl Iterator<Item = &'a AppxPackage> {
        self.appx.iter().filter(move |p| starts_with_ci(&p.display_name, prefix))
    }

    pub fn capabilities_matching<'a>(&'a self, prefix: &'a str) -> impl Iterator<Item = &'a NamedState> {
        self.capabilities.iter().filter(move |c| starts_with_ci(&c.name, prefix))
    }

    pub fn feature(&self, name: &str) -> Option<&NamedState> {
        self.features.iter().find(|f| f.name.eq_ignore_ascii_case(name))
    }

    pub fn has_service(&self, name: &str) -> bool {
        self.services.iter().any(|s| s.eq_ignore_ascii_case(name))
    }
}
