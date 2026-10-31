//! Undo script for an installed Windows built by WinISOPilot.
//!
//! Before each offline registry write the engine records the value that was
//! there (or that there was none). After the build, this module turns those
//! records plus the feature/capability/app operations into a PowerShell
//! script that puts things back on a PC installed from the media.

use crate::boottest::ps_lit;
use crate::plan::{CompiledPlan, Op};
use crate::Hive;
use serde::{Deserialize, Serialize};

/// A registry value as it was in the image before WinISOPilot changed it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum Prior {
    Absent,
    Dword(u32),
    String(String),
    /// Another type; shown in the script but not restored automatically.
    Other {
        kind: String,
        data: String,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PriorValue {
    pub hive: Hive,
    pub key: String,
    pub value: String,
    pub prior: Prior,
}

/// Parses `reg query <key> /v <value>` output. `None` if the value line
/// isn't there (value absent).
pub fn parse_reg_query(text: &str, value: &str) -> Option<Prior> {
    let want = if value.is_empty() { "(Default)" } else { value };
    for line in text.lines() {
        let t = line.trim();
        let mut parts = t.splitn(3, "    ");
        let (Some(name), Some(kind)) = (parts.next(), parts.next()) else { continue };
        if !name.trim().eq_ignore_ascii_case(want) || !kind.trim().starts_with("REG_") {
            continue;
        }
        let data = parts.next().unwrap_or("").trim().to_string();
        return Some(match kind.trim() {
            "REG_DWORD" => {
                let hex = data.trim_start_matches("0x");
                match u32::from_str_radix(hex, 16) {
                    Ok(v) => Prior::Dword(v),
                    Err(_) => Prior::Other { kind: "REG_DWORD".into(), data },
                }
            }
            "REG_SZ" => Prior::String(data),
            k => Prior::Other { kind: k.into(), data },
        });
    }
    None
}

fn live_path(hive: Hive, key: &str) -> Option<String> {
    match hive {
        Hive::Software => Some(format!(r"HKLM:\SOFTWARE\{key}")),
        Hive::System => {
            let k = key.strip_prefix(r"ControlSet001\").map(|r| format!(r"CurrentControlSet\{r}"));
            Some(format!(r"HKLM:\SYSTEM\{}", k.as_deref().unwrap_or(key)))
        }
        Hive::DefaultUser => Some(format!(r"HKCU:\{key}")),
        Hive::BootSystem => None,
    }
}

/// The undo script. `priors` come from the engine; the plan supplies the
/// non-registry operations.
pub fn script(plan: &CompiledPlan, priors: &[PriorValue], edition: &str) -> String {
    let mut s = String::new();
    s += "# WinISOPilot undo script\n";
    s += &format!("# For a PC installed from media built from: {edition}\n");
    s += "# Run in an elevated PowerShell on that PC. Review it first; it only touches what the build changed.\n";
    s += "# Per-user (HKCU) lines apply to the account that runs the script.\n\n";
    s += "$ErrorActionPreference = 'Continue'\n\n";

    if !priors.is_empty() {
        s += "# --- Registry and services: restore the values the image had before ---\n";
        for p in priors {
            let Some(path) = live_path(p.hive, &p.key) else {
                s += &format!("# Windows Setup setting {}\\{} (not on the installed PC)\n", p.key, p.value);
                continue;
            };
            let name = ps_lit(&p.value);
            let lp = ps_lit(&path);
            match &p.prior {
                Prior::Absent => {
                    s += &format!("Remove-ItemProperty -LiteralPath {lp} -Name {name} -ErrorAction SilentlyContinue\n");
                }
                Prior::Dword(v) => {
                    s += &format!("New-Item -Path {lp} -Force | Out-Null; Set-ItemProperty -LiteralPath {lp} -Name {name} -Type DWord -Value {v}\n");
                }
                Prior::String(v) => {
                    s += &format!(
                        "New-Item -Path {lp} -Force | Out-Null; Set-ItemProperty -LiteralPath {lp} -Name {name} -Type String -Value {}\n",
                        ps_lit(v)
                    );
                }
                Prior::Other { kind, data } => {
                    s += &format!("# Not restored automatically: {path}\\{} was {kind} {data}\n", p.value);
                }
            }
        }
        s += "\n";
    }

    let mut features = String::new();
    let mut caps = String::new();
    let mut apps = String::new();
    let mut other = String::new();
    for o in &plan.install {
        match &o.op {
            Op::DisableFeature { name, .. } => {
                features += &format!("Enable-WindowsOptionalFeature -Online -FeatureName {} -NoRestart\n", ps_lit(name))
            }
            Op::EnableFeature { name, .. } => {
                features += &format!("Disable-WindowsOptionalFeature -Online -FeatureName {} -NoRestart\n", ps_lit(name))
            }
            Op::RemoveCapability { name } => {
                caps += &format!("Add-WindowsCapability -Online -Name {}  # needs internet\n", ps_lit(name))
            }
            Op::RemoveAppx { display_name, .. } => apps += &format!("#   {display_name}\n"),
            Op::DeleteFile { path } => other += &format!("# Deleted {path}: reinstall or repair Windows to get it back.\n"),
            Op::ComponentCleanup { reset_base: true } => {
                other += "# ResetBase: updates already in the image can't be uninstalled; this can't be undone.\n"
            }
            Op::AddDriver { inf } => {
                other += &format!("# Added driver {inf}: remove it in Device Manager, or with pnputil /delete-driver <oemNN.inf>.\n")
            }
            Op::AddPackage { path, .. } => {
                other += &format!("# Added package {path}: uninstall it in Settings > Windows Update > Update history, if Windows allows.\n")
            }
            _ => {}
        }
    }
    if !features.is_empty() {
        s += "# --- Optional features ---\n";
        s += &features;
        s += "\n";
    }
    if !caps.is_empty() {
        s += "# --- Capabilities ---\n";
        s += &caps;
        s += "\n";
    }
    if !apps.is_empty() {
        s += "# --- Apps removed from new accounts: reinstall from the Microsoft Store ---\n";
        s += &apps;
        s += "\n";
    }
    if !other.is_empty() {
        s += "# --- Other changes ---\n";
        s += &other;
        s += "\n";
    }
    s += "Write-Host 'Done. Restart Windows to finish.'\n";
    s
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::plan::PlannedOp;

    #[test]
    fn parses_reg_query_output() {
        let out = "\r\nHKEY_LOCAL_MACHINE\\WIP_SOFTWARE\\Policies\\Microsoft\\Dsh\r\n    AllowNewsAndInterests    REG_DWORD    0x1\r\n\r\n";
        assert_eq!(parse_reg_query(out, "AllowNewsAndInterests"), Some(Prior::Dword(1)));
        let s = "HKEY_LOCAL_MACHINE\\X\n    Name    REG_SZ    hello world\n";
        assert_eq!(parse_reg_query(s, "name"), Some(Prior::String("hello world".into())));
        let d = "HKEY_LOCAL_MACHINE\\X\n    (Default)    REG_SZ    v\n";
        assert_eq!(parse_reg_query(d, ""), Some(Prior::String("v".into())));
        let b = "HKEY_LOCAL_MACHINE\\X\n    Bin    REG_BINARY    0102\n";
        assert!(matches!(parse_reg_query(b, "Bin"), Some(Prior::Other { .. })));
        assert_eq!(parse_reg_query("ERROR: not found", "x"), None);
    }

    #[test]
    fn script_restores_and_lists() {
        let priors = vec![
            PriorValue {
                hive: Hive::Software,
                key: r"Policies\Microsoft\Dsh".into(),
                value: "AllowNewsAndInterests".into(),
                prior: Prior::Absent,
            },
            PriorValue {
                hive: Hive::System,
                key: r"ControlSet001\Services\DiagTrack".into(),
                value: "Start".into(),
                prior: Prior::Dword(2),
            },
            PriorValue {
                hive: Hive::BootSystem,
                key: "Setup\\LabConfig".into(),
                value: "BypassTPMCheck".into(),
                prior: Prior::Absent,
            },
        ];
        let mut plan = CompiledPlan::default();
        plan.install.push(PlannedOp {
            origin: "x".into(),
            op: Op::DisableFeature { name: "Recall".into(), remove_payload: false },
        });
        plan.install.push(PlannedOp {
            origin: "x".into(),
            op: Op::RemoveAppx { package_name: "p".into(), display_name: "Clipchamp.Clipchamp".into() },
        });
        let s = script(&plan, &priors, "Windows 11 Pro");
        assert!(s.contains(
            r"Remove-ItemProperty -LiteralPath 'HKLM:\SOFTWARE\Policies\Microsoft\Dsh' -Name 'AllowNewsAndInterests'"
        ));
        assert!(s.contains(r"'HKLM:\SYSTEM\CurrentControlSet\Services\DiagTrack' -Name 'Start' -Type DWord -Value 2"));
        assert!(s.contains("Enable-WindowsOptionalFeature -Online -FeatureName 'Recall'"));
        assert!(s.contains("#   Clipchamp.Clipchamp"));
        assert!(s.contains("# Windows Setup setting"));
    }
}
