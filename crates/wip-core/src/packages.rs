//! Offline packages: Windows updates (`.msu`, `.cab`), language packs and
//! Features on Demand added to the install image with DISM `/Add-Package`.
//!
//! Order follows Microsoft's "Update Windows installation media with
//! Dynamic Update" guidance for the install image: servicing stack first,
//! then language packs and Features on Demand, then the cumulative update,
//! then (after component cleanup) .NET updates. On Windows 11 24H2 and later
//! DISM finds required "checkpoint" cumulative updates in the same folder as
//! the target update, so they should be kept together.
//!
//! The kind is guessed from Microsoft's file naming and can be overridden.

use crate::plan::{CompiledPlan, Op, PlannedOp};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

pub const ORIGIN: &str = "packages";

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PackageKind {
    /// Servicing stack update (separate SSUs exist on older builds).
    ServicingStack,
    LanguagePack,
    FeatureOnDemand,
    /// Cumulative or other Windows update.
    Update,
    /// .NET Framework cumulative update; added after component cleanup.
    DotNet,
    /// "Safe OS Dynamic Update": goes into the recovery environment (WinRE).
    SafeOs,
    /// "Setup Dynamic Update": files expanded onto the media's sources folder.
    SetupUpdate,
}

impl PackageKind {
    pub const ALL: [PackageKind; 7] = [
        PackageKind::ServicingStack,
        PackageKind::LanguagePack,
        PackageKind::FeatureOnDemand,
        PackageKind::Update,
        PackageKind::DotNet,
        PackageKind::SafeOs,
        PackageKind::SetupUpdate,
    ];

    pub fn label(self) -> &'static str {
        match self {
            PackageKind::ServicingStack => "Servicing stack",
            PackageKind::LanguagePack => "Language pack",
            PackageKind::FeatureOnDemand => "Feature on Demand",
            PackageKind::Update => "Windows update",
            PackageKind::DotNet => ".NET update",
            PackageKind::SafeOs => "Safe OS update (WinRE)",
            PackageKind::SetupUpdate => "Setup update",
        }
    }

    /// Where Microsoft's Dynamic Update guidance applies this kind.
    pub fn targets(self) -> &'static str {
        match self {
            PackageKind::ServicingStack => "Windows, recovery, Setup",
            PackageKind::Update => "Windows, Setup",
            PackageKind::SafeOs => "recovery",
            PackageKind::SetupUpdate => "media",
            _ => "Windows",
        }
    }

    /// Guess from Microsoft's file names.
    pub fn guess(file_name: &str) -> PackageKind {
        let n = file_name.to_ascii_lowercase();
        if n.contains("safeos") {
            PackageKind::SafeOs
        } else if n.contains("setupdu") || n.contains("setup-du") {
            PackageKind::SetupUpdate
        } else if n.contains("ssu-") || n.contains("servicingstack") {
            PackageKind::ServicingStack
        } else if n.contains("language-pack") || n.starts_with("lp.") || n.contains("languagepack") {
            PackageKind::LanguagePack
        } else if n.contains("~~") || n.contains("-fod-") || n.contains("languagefeatures") {
            PackageKind::FeatureOnDemand
        } else if n.contains("ndp") || n.contains("dotnet") || n.contains("netfx") {
            PackageKind::DotNet
        } else {
            PackageKind::Update
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Package {
    pub path: PathBuf,
    pub kind: PackageKind,
}

impl Package {
    pub fn new(path: PathBuf) -> Package {
        let kind = PackageKind::guess(&file_name(&path));
        Package { path, kind }
    }

    pub fn file_name(&self) -> String {
        file_name(&self.path)
    }

    /// Why this package can't go into an image of `arch`, if its name says
    /// it's for another architecture.
    pub fn arch_mismatch(&self, arch: &str) -> Option<String> {
        let n = self.file_name().to_ascii_lowercase();
        let named = [("arm64", "ARM64"), ("x64", "x64"), ("amd64", "x64"), ("x86", "x86")]
            .iter()
            .find(|(tag, _)| {
                n.contains(&format!("-{tag}")) || n.contains(&format!("_{tag}")) || n.contains(&format!("~{tag}~"))
            })
            .map(|(_, a)| *a)?;
        (!named.eq_ignore_ascii_case(arch)).then(|| format!("made for {named}, not {arch}"))
    }
}

/// File name of a path, also for Windows-style paths on other hosts.
pub fn file_name(p: &Path) -> String {
    let full = p.display().to_string();
    full.rsplit(['\\', '/']).next().unwrap_or(&full).to_string()
}

pub fn is_package_file(p: &Path) -> bool {
    p.extension().is_some_and(|e| e.eq_ignore_ascii_case("msu") || e.eq_ignore_ascii_case("cab"))
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct PackageChoice {
    pub packages: Vec<Package>,
}

impl PackageChoice {
    pub fn is_empty(&self) -> bool {
        self.packages.is_empty()
    }

    /// Adds files, ignoring duplicates and non-package files. Returns how
    /// many were added.
    pub fn add(&mut self, files: impl IntoIterator<Item = PathBuf>) -> usize {
        let mut n = 0;
        for f in files {
            if is_package_file(&f) && !self.packages.iter().any(|p| p.path == f) {
                self.packages.push(Package::new(f));
                n += 1;
            }
        }
        n
    }

    /// Packages in install order (kind order, then file name).
    pub fn ordered(&self) -> Vec<&Package> {
        let mut v: Vec<&Package> = self.packages.iter().collect();
        v.sort_by(|a, b| a.kind.cmp(&b.kind).then_with(|| a.file_name().cmp(&b.file_name())));
        v
    }

    /// Adds the packages to the plan following Microsoft's Dynamic Update
    /// order. Install image: SSU, language packs, FoD and updates before all
    /// other work; .NET last (after component cleanup). Windows Setup
    /// (boot.wim #2): SSU and cumulative updates. Recovery (WinRE): SSU and
    /// Safe OS updates. Media: Setup updates.
    pub fn extend_plan(&self, plan: &mut CompiledPlan, arch: &str) {
        let mut first = Vec::new();
        let mut last = Vec::new();
        let mut boot = Vec::new();
        for p in self.ordered() {
            if let Some(why) = p.arch_mismatch(arch) {
                plan.skipped.push((format!("{ORIGIN}: {}", p.file_name()), why));
                continue;
            }
            let op = PlannedOp {
                origin: ORIGIN.into(),
                op: Op::AddPackage { path: p.path.display().to_string(), kind: p.kind },
            };
            match p.kind {
                PackageKind::DotNet => last.push(op),
                PackageKind::SafeOs => plan.winre.push(op),
                PackageKind::SetupUpdate => plan.media.push(op),
                PackageKind::ServicingStack => {
                    plan.winre.push(op.clone());
                    boot.push(op.clone());
                    first.push(op);
                }
                PackageKind::Update => {
                    boot.push(op.clone());
                    first.push(op);
                }
                _ => first.push(op),
            }
        }
        plan.install.splice(0..0, first);
        plan.install.extend(last);
        plan.boot.splice(0..0, boot);
        // A recovery update only makes sense with something besides the SSU.
        if !plan.winre.iter().any(|o| matches!(o.op, Op::AddPackage { kind: PackageKind::SafeOs, .. })) {
            plan.winre.clear();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn guesses_kinds_from_microsoft_names() {
        let g = PackageKind::guess;
        assert_eq!(g("windows11.0-kb5043080-x64_953449672073f8fb99badb4cc6d5d7849b9c83e8.msu"), PackageKind::Update);
        assert_eq!(g("ssu-19041.1704-x64.cab"), PackageKind::ServicingStack);
        assert_eq!(g("windows11.0-kb5045934-ndp481-x64.msu"), PackageKind::DotNet);
        assert_eq!(g("Microsoft-Windows-Client-Language-Pack_x64_de-de.cab"), PackageKind::LanguagePack);
        assert_eq!(
            g("Microsoft-Windows-LanguageFeatures-Basic-de-de-Package~31bf3856ad364e35~amd64~~.cab"),
            PackageKind::FeatureOnDemand
        );
    }

    #[test]
    fn detects_architecture_from_names() {
        let p = |n: &str| Package::new(PathBuf::from(n));
        assert_eq!(p("windows11.0-kb5043080-x64_abc.msu").arch_mismatch("x64"), None);
        assert!(p("windows11.0-kb5043080-arm64_abc.msu").arch_mismatch("x64").is_some());
        assert!(p("Microsoft-Windows-LanguageFeatures-Basic-de-de-Package~31bf3856ad364e35~amd64~~.cab")
            .arch_mismatch("ARM64")
            .is_some());
        assert_eq!(p("update.cab").arch_mismatch("x64"), None);
    }

    #[test]
    fn plan_order_matches_microsoft_guidance() {
        let mut c = PackageChoice::default();
        let n = c.add(
            [
                "D:/u/windows11.0-kb2-ndp481-x64.msu",
                "D:/u/windows11.0-kb1-x64.msu",
                "D:/u/Microsoft-Windows-Client-Language-Pack_x64_de-de.cab",
                "D:/u/ssu-1-x64.cab",
                "D:/u/readme.txt",
                "D:/u/windows11.0-kb1-x64.msu",
                "D:/u/windows11.0-kb9-arm64.msu",
            ]
            .map(PathBuf::from),
        );
        assert_eq!(n, 5);
        let mut plan = CompiledPlan::default();
        plan.install.push(PlannedOp { origin: "a".into(), op: Op::DeleteFile { path: "x".into() } });
        plan.install.push(PlannedOp { origin: "c".into(), op: Op::ComponentCleanup { reset_base: false } });
        c.extend_plan(&mut plan, "x64");
        let kinds: Vec<_> = plan
            .install
            .iter()
            .map(|o| match &o.op {
                Op::AddPackage { kind, .. } => kind.label(),
                Op::DeleteFile { .. } => "delete",
                Op::ComponentCleanup { .. } => "cleanup",
                _ => "?",
            })
            .collect();
        assert_eq!(kinds, ["Servicing stack", "Language pack", "Windows update", "delete", "cleanup", ".NET update"]);
        assert_eq!(plan.skipped.len(), 1);
    }
}
