//! Driver integration: finds `.inf` files in folders the user picks, reads
//! what each one is for, and turns the usable ones into [`Op::AddDriver`]
//! operations for the install image and (storage drivers) Windows Setup.
//!
//! DISM's API adds one INF at a time (`DismAddDriver`), so folder recursion
//! happens here. Unsigned packages (no `CatalogFile`) and packages built for
//! another architecture are skipped with a reason instead of failing the
//! build; unsigned drivers are never forced in.

use crate::plan::{CompiledPlan, Op, PlannedOp};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

/// Most INF files read from one folder (driver packs can be huge).
pub const MAX_INFS: usize = 2000;
/// Deepest folder level searched below the chosen folder.
const MAX_DEPTH: usize = 8;
/// INFs larger than this are not parsed (real ones are well under 1 MB).
const MAX_INF_BYTES: u64 = 4 * 1024 * 1024;
/// Plan origin for driver operations.
pub const ORIGIN: &str = "drivers";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct InfInfo {
    pub path: PathBuf,
    /// `Class` from `[Version]`, e.g. `Net`, `SCSIAdapter`, `Display`.
    pub class: String,
    pub provider: String,
    /// `DriverVer` as written: `date,version`.
    pub version: String,
    /// Architectures from the `[Manufacturer]` decorations, as image
    /// architecture names (`x86`, `x64`, `ARM64`).
    pub archs: Vec<String>,
    /// Has a `CatalogFile` entry, i.e. is a signed package.
    pub signed: bool,
}

impl InfInfo {
    /// Storage controllers: the drivers Windows Setup needs to see disks.
    pub fn is_storage(&self) -> bool {
        ["SCSIAdapter", "HDC"].iter().any(|c| self.class.eq_ignore_ascii_case(c))
    }

    pub fn file_name(&self) -> String {
        self.path.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default()
    }

    /// Why this INF can't go into an image of `arch`, or `None` if it can.
    pub fn unusable(&self, arch: &str) -> Option<String> {
        if !self.signed {
            return Some("not signed (no CatalogFile), so Windows would refuse it".into());
        }
        if !self.archs.iter().any(|a| a.eq_ignore_ascii_case(arch)) {
            let built = if self.archs.is_empty() { "no architecture".into() } else { self.archs.join(", ") };
            return Some(format!("built for {built}, not {arch}"));
        }
        None
    }
}

/// Decodes an INF: UTF-16 LE/BE (with BOM, or detected by NUL bytes),
/// UTF-8 with or without BOM, else Windows-1252 read as Latin-1.
pub fn decode(bytes: &[u8]) -> String {
    let utf16 = |b: &[u8], le: bool| {
        let units: Vec<u16> = b
            .chunks_exact(2)
            .map(|c| if le { u16::from_le_bytes([c[0], c[1]]) } else { u16::from_be_bytes([c[0], c[1]]) })
            .collect();
        String::from_utf16_lossy(&units)
    };
    match bytes {
        [0xFF, 0xFE, rest @ ..] => utf16(rest, true),
        [0xFE, 0xFF, rest @ ..] => utf16(rest, false),
        [0xEF, 0xBB, 0xBF, rest @ ..] => String::from_utf8_lossy(rest).into_owned(),
        [a, 0, ..] if *a != 0 => utf16(bytes, true),
        _ => match std::str::from_utf8(bytes) {
            Ok(s) => s.to_string(),
            Err(_) => bytes.iter().map(|&b| b as char).collect(),
        },
    }
}

fn strip_comment(line: &str) -> &str {
    // `;` starts a comment unless inside double quotes.
    let mut quoted = false;
    for (i, c) in line.char_indices() {
        match c {
            '"' => quoted = !quoted,
            ';' if !quoted => return &line[..i],
            _ => {}
        }
    }
    line
}

fn unquote(s: &str) -> String {
    s.trim().trim_matches('"').trim().to_string()
}

/// Maps an INF platform decoration (`NTamd64.10.0...1`) to an image arch.
fn decoration_arch(d: &str) -> Option<&'static str> {
    let d = d.trim().to_ascii_lowercase();
    let platform = d.strip_prefix("nt")?.split('.').next().unwrap_or("");
    match platform {
        "" | "x86" => Some("x86"),
        "amd64" => Some("x64"),
        "arm64" => Some("ARM64"),
        "arm" => Some("ARM"),
        _ => None,
    }
}

/// Reads `[Version]` and the `[Manufacturer]` decorations. `%string%`
/// tokens are resolved from `[Strings]`.
pub fn parse(path: PathBuf, text: &str) -> InfInfo {
    let mut section = String::new();
    let mut version: Vec<(String, String)> = Vec::new();
    let mut strings: Vec<(String, String)> = Vec::new();
    let mut archs: Vec<String> = Vec::new();
    let mut sections: Vec<String> = Vec::new();
    let mut models: Vec<String> = Vec::new();
    for raw in text.lines() {
        let line = strip_comment(raw).trim();
        if line.is_empty() {
            continue;
        }
        if let Some(name) = line.strip_prefix('[').and_then(|l| l.split(']').next()) {
            section = name.trim().to_ascii_lowercase();
            sections.push(section.clone());
            continue;
        }
        let (key, value) = match line.split_once('=') {
            Some((k, v)) => (k.trim().to_string(), v.trim().to_string()),
            None => (line.to_string(), String::new()),
        };
        match section.as_str() {
            "version" => version.push((key.to_ascii_lowercase(), value)),
            "strings" => strings.push((key.to_ascii_lowercase(), unquote(&value))),
            "manufacturer" => {
                // Manufacturer = ModelsSection[, Decoration1[, Decoration2…]]
                let mut parts = value.split(',');
                if let Some(base) = parts.next() {
                    models.push(base.trim().to_ascii_lowercase());
                }
                for d in parts {
                    if let Some(a) = decoration_arch(d) {
                        if !archs.iter().any(|x| x == a) {
                            archs.push(a.to_string());
                        }
                    }
                }
            }
            _ => {}
        }
    }
    // An undecorated models section is used on x86 only.
    if models.iter().any(|m| sections.contains(m)) && !archs.iter().any(|a| a == "x86") {
        archs.push("x86".into());
    }
    let resolve = |v: &str| -> String {
        let v = unquote(v);
        match v.strip_prefix('%').and_then(|s| s.strip_suffix('%')) {
            Some(token) => {
                strings.iter().find(|(k, _)| k.eq_ignore_ascii_case(token)).map(|(_, s)| s.clone()).unwrap_or(v.clone())
            }
            None => v,
        }
    };
    let get = |k: &str| version.iter().find(|(key, _)| key == k).map(|(_, v)| resolve(v)).unwrap_or_default();
    let signed = ["catalogfile", "catalogfile.nt", "catalogfile.ntamd64", "catalogfile.ntarm64", "catalogfile.ntx86"]
        .iter()
        .any(|k| version.iter().any(|(key, v)| key == k && !v.trim().is_empty()));
    InfInfo { path, class: get("class"), provider: get("provider"), version: get("driverver"), archs, signed }
}

/// Every `.inf` below `folder`, sorted, at most [`MAX_INFS`]. Unreadable
/// sub-folders are skipped. Returns `(infs, truncated)`.
pub fn scan(folder: &Path) -> std::io::Result<(Vec<InfInfo>, bool)> {
    let mut paths = Vec::new();
    let mut truncated = false;
    collect(folder, 0, &mut paths, &mut truncated)?;
    paths.sort();
    let infs = paths
        .into_iter()
        .filter_map(|p| {
            let len = std::fs::metadata(&p).ok()?.len();
            if len > MAX_INF_BYTES {
                return None;
            }
            let bytes = std::fs::read(&p).ok()?;
            Some(parse(p, &decode(&bytes)))
        })
        .collect();
    Ok((infs, truncated))
}

fn collect(dir: &Path, depth: usize, out: &mut Vec<PathBuf>, truncated: &mut bool) -> std::io::Result<()> {
    let entries = match std::fs::read_dir(dir) {
        Ok(e) => e,
        Err(e) if depth == 0 => return Err(e),
        Err(_) => return Ok(()),
    };
    let mut dirs = Vec::new();
    for e in entries.flatten() {
        let Ok(ft) = e.file_type() else { continue };
        let p = e.path();
        if ft.is_dir() {
            dirs.push(p);
        } else if ft.is_file() && p.extension().is_some_and(|x| x.eq_ignore_ascii_case("inf")) {
            if out.len() >= MAX_INFS {
                *truncated = true;
                return Ok(());
            }
            out.push(p);
        }
    }
    if depth < MAX_DEPTH {
        dirs.sort();
        for d in dirs {
            collect(&d, depth + 1, out, truncated)?;
        }
    }
    Ok(())
}

/// The user's driver choices.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct DriverChoice {
    pub folders: Vec<DriverFolder>,
    /// Also add storage-controller drivers to Windows Setup (boot.wim #2),
    /// so Setup can see disks behind e.g. Intel RST/VMD.
    pub setup_storage: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DriverFolder {
    pub path: PathBuf,
    pub infs: Vec<InfInfo>,
    pub truncated: bool,
}

impl DriverChoice {
    pub fn new() -> DriverChoice {
        DriverChoice { folders: Vec::new(), setup_storage: true }
    }

    pub fn is_empty(&self) -> bool {
        self.folders.iter().all(|f| f.infs.is_empty())
    }

    /// `(usable, skipped)` counts for an image architecture.
    pub fn counts(&self, arch: &str) -> (usize, usize) {
        let all = self.folders.iter().flat_map(|f| &f.infs);
        let usable = all.clone().filter(|i| i.unusable(arch).is_none()).count();
        (usable, all.count() - usable)
    }

    /// Adds driver operations to a compiled plan for an image of `arch`.
    /// Install-image drivers go before registry work and component cleanup;
    /// Setup drivers go first in the boot.wim list. Duplicate INF paths are
    /// added once; unusable INFs land in `plan.skipped`.
    pub fn extend_plan(&self, plan: &mut CompiledPlan, arch: &str) {
        let mut install = Vec::new();
        let mut boot = Vec::new();
        let mut seen = std::collections::BTreeSet::new();
        for inf in self.folders.iter().flat_map(|f| &f.infs) {
            if !seen.insert(inf.path.clone()) {
                continue;
            }
            if let Some(why) = inf.unusable(arch) {
                plan.skipped.push((format!("{ORIGIN}: {}", inf.path.display()), why));
                continue;
            }
            let op = PlannedOp { origin: ORIGIN.into(), op: Op::AddDriver { inf: inf.path.display().to_string() } };
            if self.setup_storage && inf.is_storage() {
                boot.push(op.clone());
            }
            install.push(op);
        }
        let at = plan
            .install
            .iter()
            .position(|o| matches!(o.op, Op::LoadHive { .. } | Op::ComponentCleanup { .. }))
            .unwrap_or(plan.install.len());
        plan.install.splice(at..at, install);
        plan.boot.splice(0..0, boot);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const NET: &str = r#"
; Sample network driver
[Version]
Signature   = "$WINDOWS NT$"
Class       = Net
ClassGUID   = {4d36e972-e325-11ce-bfc1-08002be10318}
Provider    = %Vendor%
CatalogFile = e1d.cat
DriverVer   = 06/14/2024,12.19.2.60

[Manufacturer]
%Vendor% = Models, NTamd64.10.0...19041, NTarm64

[Strings]
Vendor = "Example Corp ; not a comment"
"#;

    #[test]
    fn parses_version_and_decorations() {
        let i = parse(PathBuf::from("e1d.inf"), NET);
        assert_eq!(i.class, "Net");
        assert_eq!(i.provider, "Example Corp ; not a comment");
        assert_eq!(i.version, "06/14/2024,12.19.2.60");
        assert_eq!(i.archs, vec!["x64", "ARM64"]);
        assert!(i.signed);
        assert!(!i.is_storage());
        assert_eq!(i.unusable("x64"), None);
        assert!(i.unusable("x86").unwrap().contains("not x86"));
    }

    #[test]
    fn undecorated_is_x86_and_missing_catalog_is_unsigned() {
        let i = parse(PathBuf::from("old.inf"), "[Version]\nClass=SCSIAdapter\n[Manufacturer]\n%M%=Models\n[Models]\n");
        assert_eq!(i.archs, vec!["x86"]);
        assert!(!i.signed);
        assert!(i.is_storage());
        assert!(i.unusable("x86").unwrap().contains("not signed"));
    }

    #[test]
    fn decodes_utf16_with_and_without_bom() {
        let text = "[Version]\r\nClass=HDC\r\n";
        let le: Vec<u8> = text.encode_utf16().flat_map(|u| u.to_le_bytes()).collect();
        let mut bom = vec![0xFF, 0xFE];
        bom.extend(&le);
        assert_eq!(decode(&bom), text);
        assert_eq!(decode(&le), text);
        assert_eq!(decode(&[b'a', 0xE9]), "aé");
    }

    fn inf(path: &str, class: &str, signed: bool, arch: &str) -> InfInfo {
        InfInfo {
            path: PathBuf::from(path),
            class: class.into(),
            provider: String::new(),
            version: String::new(),
            archs: vec![arch.into()],
            signed,
        }
    }

    #[test]
    fn extends_plan_in_order_and_skips_unusable() {
        let mut plan = CompiledPlan::default();
        plan.install.push(PlannedOp { origin: "a".into(), op: Op::DeleteFile { path: "x".into() } });
        plan.install.push(PlannedOp { origin: String::new(), op: Op::LoadHive { hive: crate::Hive::Software } });
        plan.boot.push(PlannedOp { origin: String::new(), op: Op::LoadHive { hive: crate::Hive::BootSystem } });
        let choice = DriverChoice {
            setup_storage: true,
            folders: vec![DriverFolder {
                path: PathBuf::from("D"),
                truncated: false,
                infs: vec![
                    inf("D/net.inf", "Net", true, "x64"),
                    inf("D/vmd.inf", "SCSIAdapter", true, "x64"),
                    inf("D/unsigned.inf", "Net", false, "x64"),
                    inf("D/arm.inf", "Net", true, "ARM64"),
                    inf("D/net.inf", "Net", true, "x64"),
                ],
            }],
        };
        assert_eq!(choice.counts("x64"), (3, 2));
        choice.extend_plan(&mut plan, "x64");
        let names: Vec<_> = plan.install.iter().map(|o| o.op.describe()).collect();
        assert_eq!(names[0], "Deleting x");
        assert!(names[1].contains("net.inf") && names[2].contains("vmd.inf"));
        assert!(matches!(plan.install[3].op, Op::LoadHive { .. }));
        assert_eq!(plan.boot.len(), 2);
        assert!(plan.boot[0].op.describe().contains("vmd.inf"));
        assert_eq!(plan.skipped.len(), 2);

        let mut no_setup = CompiledPlan::default();
        DriverChoice { setup_storage: false, ..choice }.extend_plan(&mut no_setup, "x64");
        assert!(no_setup.boot.is_empty());
        assert_eq!(no_setup.install.len(), 2);
    }

    #[test]
    fn scans_folders_recursively() {
        let root = std::env::temp_dir().join(format!("wip-drivers-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(root.join("a/b")).unwrap();
        std::fs::write(root.join("a/b/x.INF"), NET).unwrap();
        std::fs::write(root.join("readme.txt"), "no").unwrap();
        std::fs::write(root.join("y.inf"), "[Version]\nClass=HDC\nCatalogFile=y.cat\n").unwrap();
        let (infs, truncated) = scan(&root).unwrap();
        assert!(!truncated);
        assert_eq!(infs.len(), 2);
        assert!(infs.iter().any(|i| i.class == "Net"));
        assert!(scan(&root.join("missing")).is_err());
        std::fs::remove_dir_all(&root).unwrap();
    }
}
