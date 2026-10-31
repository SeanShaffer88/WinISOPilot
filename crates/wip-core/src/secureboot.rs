//! Secure Boot readiness of Windows boot media.
//!
//! Microsoft is moving Windows boot media from the boot manager signed by
//! "Microsoft Windows Production PCA 2011" to one signed by "Windows UEFI CA
//! 2023" (KB5025885, KB5053484). PCs that have applied Microsoft's boot
//! manager revocation no longer boot 2011-signed media with Secure Boot on;
//! PCs whose firmware doesn't yet trust the 2023 CA can't boot 2023-signed
//! media with Secure Boot on.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum BootCa {
    Pca2011,
    Uefi2023,
    Other,
}

impl BootCa {
    pub fn from_issuer(cn: &str) -> BootCa {
        let c = cn.to_ascii_lowercase();
        if c.contains("windows uefi ca 2023") {
            BootCa::Uefi2023
        } else if c.contains("production pca 2011") {
            BootCa::Pca2011
        } else {
            BootCa::Other
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            BootCa::Pca2011 => "2011 certificate",
            BootCa::Uefi2023 => "2023 certificate",
            BootCa::Other => "unrecognized certificate",
        }
    }

    /// What this means for booting with Secure Boot on.
    pub fn advice(self) -> &'static str {
        match self {
            BootCa::Pca2011 => "Boots on most PCs today. PCs that have applied Microsoft's boot manager revocation (KB5025885) won't start it with Secure Boot on; build with the 2023-signed boot manager for those.",
            BootCa::Uefi2023 => "Boots on PCs whose firmware trusts the Windows UEFI CA 2023. Older PCs without that certificate need Secure Boot off, or media with the 2011-signed boot manager.",
            BootCa::Other => "Not signed by a Microsoft Windows certificate this app knows. It may not boot with Secure Boot on.",
        }
    }
}

/// Extracts the CN from an X.500 issuer string like
/// `CN=Windows UEFI CA 2023, O=Microsoft Corporation, C=US`.
pub fn issuer_cn(issuer: &str) -> Option<String> {
    issuer
        .split(',')
        .map(str::trim)
        .find_map(|part| part.strip_prefix("CN="))
        .map(|cn| cn.trim_matches('"').to_string())
        .filter(|cn| !cn.is_empty())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_issuers() {
        let cn = issuer_cn("CN=Windows UEFI CA 2023, O=Microsoft Corporation, C=US").unwrap();
        assert_eq!(BootCa::from_issuer(&cn), BootCa::Uefi2023);
        let cn = issuer_cn(
            "CN=Microsoft Windows Production PCA 2011, O=Microsoft Corporation, L=Redmond, S=Washington, C=US",
        )
        .unwrap();
        assert_eq!(BootCa::from_issuer(&cn), BootCa::Pca2011);
        assert_eq!(issuer_cn(""), None);
        assert_eq!(BootCa::from_issuer("Someone"), BootCa::Other);
    }
}
