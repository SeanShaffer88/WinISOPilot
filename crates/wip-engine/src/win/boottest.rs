//! Boot test: unattended install of the built ISO in a throwaway Hyper-V
//! VM, then read the in-guest check results from the VM's disk.

use super::proc::{expect_ok, quoted, run};
use super::workflow::ps;
use crate::{BootOutcome, BootTestReport, BootTestRequest, EngineError, HostInfo, Job, Result};
use std::fs;
use std::path::Path;
use std::time::{Duration, Instant};
use wip_core::boottest::{score, unattend_xml, verify_script, RESULT_PATH};
use wip_core::recipe::sanitize_label;

fn q(s: &str) -> String {
    wip_core::boottest::ps_lit(s)
}

fn q_path(p: &Path) -> String {
    q(&p.display().to_string())
}

/// Checks that Hyper-V's PowerShell module and host service are usable.
fn check_hyperv(job: &Job) -> Result<()> {
    let script = "if (-not (Get-Command New-VM -ErrorAction SilentlyContinue)) { 'nomodule' } \
                  elseif (-not (Get-VMHost -ErrorAction SilentlyContinue)) { 'nohost' } else { 'ok' }";
    let out = ps(script, job)?;
    match out.trim() {
        "ok" => Ok(()),
        "nomodule" => Err(EngineError::new(
            "WIP-E100",
            "Hyper-V isn't installed. Turn on Hyper-V (Windows Pro, Enterprise or Education) and restart, then try again.",
        )),
        _ => Err(EngineError::new(
            "WIP-E101",
            "Hyper-V is installed but not running. Restart the PC, and check that virtualization is enabled in the firmware.",
        )),
    }
}

fn vm_state(name: &str, job: &Job) -> Result<String> {
    Ok(ps(&format!("(Get-VM -Name {}).State", q(name)), job)?.trim().to_string())
}

fn remove_vm(name: &str, folder: &Path, job: &Job) {
    let script = format!(
        "Stop-VM -Name {n} -TurnOff -Force -ErrorAction SilentlyContinue; Remove-VM -Name {n} -Force -ErrorAction SilentlyContinue; \
         Remove-Item -LiteralPath {f} -Recurse -Force -ErrorAction SilentlyContinue",
        n = q(name),
        f = q_path(folder)
    );
    if let Err(e) = ps(&script, job) {
        job.log(format!("Cleanup: {e}"));
    }
}

fn read_results(vhd: &Path, job: &Job) -> Result<Option<String>> {
    let script = format!(
        "$ErrorActionPreference='Stop'; $v={v}; $d = Mount-VHD -Path $v -Passthru | Get-Disk; try {{ \
         $p = Get-Partition -DiskNumber $d.Number | Sort-Object Size -Descending | Select-Object -First 1; \
         if (-not $p.DriveLetter) {{ $p | Add-PartitionAccessPath -AssignDriveLetter; $p = Get-Partition -DiskNumber $d.Number -PartitionNumber $p.PartitionNumber }}; \
         $f = \"$($p.DriveLetter):\\{r}\"; if (Test-Path -LiteralPath $f) {{ Get-Content -Raw -LiteralPath $f }} else {{ 'WIP-NO-RESULTS' }} \
         }} finally {{ Dismount-VHD -Path $v }}",
        v = q_path(vhd),
        r = RESULT_PATH
    );
    let out = ps(&script, job)?;
    let t = out.trim();
    Ok(if t.is_empty() || t.contains("WIP-NO-RESULTS") { None } else { Some(t.to_string()) })
}

pub fn run_test(req: &BootTestRequest, host: &HostInfo, job: &Job) -> Result<BootTestReport> {
    if !host.is_admin {
        return Err(EngineError::new("WIP-E002", "Administrator rights are needed to run a boot test."));
    }
    let oscdimg = host.oscdimg.clone().ok_or_else(|| {
        EngineError::new("WIP-E102", "The boot test needs oscdimg.exe from the Windows ADK to make its answer disc.")
    })?;
    if !req.iso.is_file() {
        return Err(EngineError::new("WIP-E103", format!("{} wasn't found.", req.iso.display())));
    }
    const STAGES: u32 = 4;
    job.stage(1, STAGES, "Checking Hyper-V and preparing the answer disc");
    check_hyperv(job)?;
    let stamp = wip_core::journal::now_secs();
    let name = format!("WinISOPilot-BootTest-{stamp}");
    let folder = req.work_parent.join(&name);
    let answer_dir = folder.join("answer");
    fs::create_dir_all(&answer_dir)
        .map_err(|e| EngineError::new("WIP-E104", format!("Can't create {}: {e}", answer_dir.display())))?;
    let write = |p: &Path, s: &str| {
        wip_core::fsutil::atomic_write(p, s.as_bytes())
            .map_err(|e| EngineError::new("WIP-E104", format!("Can't write {}: {e}", p.display())))
    };
    write(&answer_dir.join("autounattend.xml"), &unattend_xml(&req.target))?;
    write(&answer_dir.join("wip-verify.ps1"), &verify_script(&req.checks))?;
    let answer_iso = folder.join("answer.iso");
    let args = vec![
        "-m".into(),
        "-n".into(),
        format!("-l{}", sanitize_label("WIPANSWER")),
        quoted(&answer_dir),
        quoted(&answer_iso),
    ];
    expect_ok(run(&oscdimg, &args, job)?, "Making the answer disc", "WIP-E105")?;
    job.check()?;

    job.stage(2, STAGES, "Creating the test virtual machine");
    let vhd = folder.join("disk.vhdx");
    let create = format!(
        "$ErrorActionPreference='Stop'; $n={n}; \
         New-VM -Name $n -Generation 2 -MemoryStartupBytes 4GB -NewVHDPath {vhd} -NewVHDSizeBytes 64GB -Path {dir} | Out-Null; \
         Set-VMProcessor -VMName $n -Count 2; Set-VMMemory -VMName $n -DynamicMemoryEnabled $false; \
         Set-VM -Name $n -AutomaticCheckpointsEnabled $false; \
         Set-VMFirmware -VMName $n -EnableSecureBoot On -SecureBootTemplate 'MicrosoftWindows'; \
         Set-VMKeyProtector -VMName $n -NewLocalKeyProtector; Enable-VMTPM -VMName $n; \
         Add-VMDvdDrive -VMName $n -Path {iso}; Add-VMDvdDrive -VMName $n -Path {ans}; \
         $dvd = Get-VMDvdDrive -VMName $n | Where-Object {{ $_.Path -eq {iso} }} | Select-Object -First 1; \
         Set-VMFirmware -VMName $n -FirstBootDevice $dvd; Start-VM -Name $n",
        n = q(&name),
        vhd = q_path(&vhd),
        dir = q_path(&folder),
        iso = q_path(&req.iso),
        ans = q_path(&answer_iso),
    );
    if let Err(e) = ps(&create, job) {
        remove_vm(&name, &folder, job);
        return Err(EngineError::new("WIP-E106", format!("Couldn't create the test VM. {}", e.message)));
    }
    // UEFI DVD boot waits for a key press; send Enter for a few seconds.
    let keys = format!(
        "$vm = Get-CimInstance -Namespace root\\virtualization\\v2 -ClassName Msvm_ComputerSystem -Filter \"ElementName='{n}'\"; \
         $kb = Get-CimAssociatedInstance -InputObject $vm -ResultClassName Msvm_Keyboard; \
         for ($i = 0; $i -lt 24; $i++) {{ Invoke-CimMethod -InputObject $kb -MethodName TypeKey -Arguments @{{keyCode=13}} | Out-Null; Start-Sleep -Milliseconds 500 }}",
        n = name.replace('\'', "")
    );
    if let Err(e) = ps(&keys, job) {
        job.log(format!("Couldn't send key presses to the VM: {e}"));
    }

    job.stage(3, STAGES, "Installing Windows in the test VM");
    let started = Instant::now();
    let timeout = Duration::from_secs(u64::from(req.timeout_minutes.max(20)) * 60);
    let mut last_log_min = u64::MAX;
    let timed_out = loop {
        // One-second steps so Cancel is responsive; state checked every 15 s.
        for _ in 0..15 {
            if job.cancelled() {
                remove_vm(&name, &folder, job);
                return Err(EngineError::cancelled());
            }
            std::thread::sleep(Duration::from_secs(1));
        }
        let elapsed = started.elapsed();
        job.progress(elapsed.as_secs_f32() / timeout.as_secs_f32());
        let min = elapsed.as_secs() / 60;
        if min != last_log_min && min % 2 == 0 {
            last_log_min = min;
            job.log(format!("Installing… {min} min elapsed, stops after {} min", timeout.as_secs() / 60));
        }
        match vm_state(&name, job) {
            Ok(s) if s.eq_ignore_ascii_case("Off") => break false,
            Ok(_) => {}
            Err(e) => job.log(format!("VM state: {e}")),
        }
        if elapsed >= timeout {
            let _ = ps(&format!("Stop-VM -Name {} -TurnOff -Force", q(&name)), job);
            break true;
        }
    };
    let minutes = (started.elapsed().as_secs() / 60) as u32;

    job.stage(4, STAGES, "Reading the results");
    let scored = if timed_out {
        None
    } else {
        match read_results(&vhd, job)? {
            Some(json) => Some(score(&json, &req.checks).map_err(|e| EngineError::new("WIP-E107", e))?),
            None => None,
        }
    };
    let outcome = match (timed_out, &scored) {
        (true, _) => BootOutcome::TimedOut,
        (false, None) => BootOutcome::SetupDidNotFinish,
        (false, Some(s)) if s.all_passed() => BootOutcome::Passed,
        _ => BootOutcome::ChecksFailed,
    };
    let keep = req.keep_vm || outcome != BootOutcome::Passed;
    if !keep {
        remove_vm(&name, &folder, job);
    }
    Ok(BootTestReport { vm_name: name, folder, minutes, outcome, scored, vm_kept: keep })
}
