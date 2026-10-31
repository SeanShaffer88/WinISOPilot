//! Offline registry editing with reg.exe on hives inside a mounted image.
//! reg.exe handles the backup/restore privileges hive loading needs.

use super::host::system32;
use super::proc::{expect_ok, run};
use crate::{EngineError, Job, Result};
use std::path::{Path, PathBuf};
use wip_core::{Hive, RegData};

fn reg_exe() -> PathBuf {
    system32().map(|s| s.join("reg.exe")).unwrap_or_else(|| PathBuf::from("reg.exe"))
}

fn root(hive: Hive) -> String {
    format!(r"HKLM\{}", hive.mount_key())
}

fn is_loaded(hive: Hive, job: &Job) -> bool {
    run(&reg_exe(), &["query".into(), root(hive)], job).map(|o| o.code == 0).unwrap_or(false)
}

pub fn load(hive: Hive, mount_dir: &Path, job: &Job) -> Result<()> {
    if is_loaded(hive, job) {
        // Our own key name from an interrupted earlier run.
        job.log(format!("{} is still loaded from an earlier run; unloading it first.", root(hive)));
        unload(hive, job)?;
    }
    let file = mount_dir.join(hive.file());
    if !file.is_file() {
        return Err(EngineError::new(
            "WIP-E030",
            format!("Registry hive {} wasn't found in the image.", file.display()),
        ));
    }
    let out = run(&reg_exe(), &["load".into(), root(hive), file.display().to_string()], job)?;
    expect_ok(out, "Loading a registry hive", "WIP-E031").map(|_| ())
}

/// Unloads with a few retries: Windows can briefly hold the hive open.
pub fn unload(hive: Hive, job: &Job) -> Result<()> {
    let mut last = String::new();
    for attempt in 1..=5 {
        let out = run(&reg_exe(), &["unload".into(), root(hive)], job)?;
        if out.code == 0 {
            return Ok(());
        }
        last = out.text;
        if attempt < 5 {
            job.log(format!("Hive is busy; retrying in 1 s ({attempt}/4)"));
            std::thread::sleep(std::time::Duration::from_secs(1));
        }
    }
    Err(EngineError::new(
        "WIP-E032",
        format!("Couldn't close registry hive {}. Close Registry Editor if it's open. {}", root(hive), last.trim()),
    ))
}

pub fn set(hive: Hive, key: &str, value: &str, data: &RegData, job: &Job) -> Result<()> {
    let mut args = vec!["add".to_string(), format!(r"{}\{key}", root(hive))];
    if value.is_empty() {
        args.push("/ve".into());
    } else {
        args.push("/v".into());
        args.push(value.into());
    }
    match data {
        RegData::Dword(v) => {
            args.extend(["/t".into(), "REG_DWORD".into(), "/d".into(), v.to_string()]);
        }
        RegData::String(s) => {
            args.extend(["/t".into(), "REG_SZ".into(), "/d".into(), s.clone()]);
        }
    }
    args.push("/f".into());
    expect_ok(run(&reg_exe(), &args, job)?, "Writing a registry value", "WIP-E033").map(|_| ())
}

/// The value currently stored (offline hive must be loaded). Errors are
/// treated as "unknown" by the caller.
pub fn query(hive: Hive, key: &str, value: &str, job: &Job) -> Result<wip_core::revert::Prior> {
    let full = format!(r"{}\{key}", root(hive));
    let mut args = vec!["query".to_string(), full];
    if value.is_empty() {
        args.push("/ve".into());
    } else {
        args.extend(["/v".into(), value.into()]);
    }
    let out = run(&reg_exe(), &args, job)?;
    if out.code != 0 {
        return Ok(wip_core::revert::Prior::Absent);
    }
    Ok(wip_core::revert::parse_reg_query(&out.text, value).unwrap_or(wip_core::revert::Prior::Absent))
}

pub fn delete(hive: Hive, key: &str, value: &str, job: &Job) -> Result<()> {
    let full = format!(r"{}\{key}", root(hive));
    let q = run(&reg_exe(), &["query".into(), full.clone(), "/v".into(), value.into()], job)?;
    if q.code != 0 {
        job.log(format!("{value} isn't present; nothing to delete."));
        return Ok(());
    }
    let args = vec!["delete".into(), full, "/v".into(), value.into(), "/f".into()];
    expect_ok(run(&reg_exe(), &args, job)?, "Deleting a registry value", "WIP-E034").map(|_| ())
}

/// Lists service key names in an offline SYSTEM hive.
pub fn services(mount_dir: &Path, job: &Job) -> Result<Vec<String>> {
    load(Hive::System, mount_dir, job)?;
    let key = format!(r"{}\ControlSet001\Services", root(Hive::System));
    let result = run(&reg_exe(), &["query".into(), key.clone()], job);
    let unload_result = unload(Hive::System, job);
    let out = expect_ok(result?, "Listing services", "WIP-E035")?;
    unload_result?;
    const PREFIX: &str = r"HKEY_LOCAL_MACHINE\WIP_SYSTEM\CONTROLSET001\SERVICES\";
    Ok(out
        .lines()
        .map(str::trim)
        .filter(|l| l.len() > PREFIX.len() && l.to_ascii_uppercase().starts_with(PREFIX))
        .map(|l| l[PREFIX.len()..].to_string())
        .filter(|name| !name.contains('\\'))
        .collect())
}
