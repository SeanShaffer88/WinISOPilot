//! The end-to-end jobs: inspect an ISO, prepare a workspace, build.

use super::dism::{Dism, Session};
use super::host::system32;
use super::proc::{expect_ok, opt, ps_quote, quoted, run, run_with_progress};
use super::{journal, registry};
use crate::{required_space, BuildReport, EngineError, HostInfo, Job, PrepareRequest, Result, SourceInfo, Workspace};
use std::fs;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use wip_core::boottest::TestTarget;
use wip_core::footprint::{Changes, Footprint, Measure};
use wip_core::inventory::{AppxPackage, Inventory};
use wip_core::plan::{CompiledPlan, Op, PlannedOp};
use wip_core::recipe::{sanitize_label, Compression, OutputSettings};
use wip_core::{human_bytes, Hive};

fn powershell() -> PathBuf {
    system32()
        .map(|s| s.join(r"WindowsPowerShell\v1.0\powershell.exe"))
        .unwrap_or_else(|| PathBuf::from("powershell.exe"))
}

pub(crate) fn ps(script: &str, job: &Job) -> Result<String> {
    let args = vec!["-NoProfile".into(), "-NonInteractive".into(), "-Command".into(), script.to_string()];
    expect_ok(run(&powershell(), &args, job)?, "PowerShell", "WIP-E040")
}

fn need_dism(host: &HostInfo) -> Result<(Dism, PathBuf)> {
    let dll = host.dism_api.as_ref().ok_or_else(|| {
        EngineError::new(
            "WIP-E012",
            "DISM wasn't found. Install the Windows ADK Deployment Tools or choose its folder in Settings.",
        )
    })?;
    let exe = host.dism_exe.clone().ok_or_else(|| EngineError::new("WIP-E013", "dism.exe wasn't found."))?;
    Ok((Dism::open(dll)?, exe))
}

fn need_admin(host: &HostInfo) -> Result<()> {
    if host.is_admin {
        Ok(())
    } else {
        Err(EngineError::new("WIP-E002", "Administrator rights are needed. Restart WinISOPilot as administrator."))
    }
}

/// An attached ISO. Detached on drop if we attached it.
pub(crate) struct IsoMount {
    iso: PathBuf,
    pub(crate) root: PathBuf,
    ours: bool,
    job: Job,
}

impl IsoMount {
    pub(crate) fn attach(iso: &Path, job: &Job) -> Result<IsoMount> {
        let script = format!(
            "$ErrorActionPreference='Stop'; $p={}; $i=Get-DiskImage -ImagePath $p; $m=$false; \
             if(-not $i.Attached){{ $i=Mount-DiskImage -ImagePath $p -StorageType ISO -Access ReadOnly -PassThru; $m=$true }}; \
             $d=$null; for($n=0; $n -lt 20 -and -not $d; $n++){{ $d=($i | Get-Volume).DriveLetter; if(-not $d){{ Start-Sleep -Milliseconds 250 }} }}; \
             Write-Output ('{{0}}|{{1}}' -f $d,$m)",
            ps_quote(&iso.display().to_string())
        );
        let out = ps(&script, job).map_err(|e| {
            EngineError::new(
                "WIP-E041",
                format!("Couldn't open the ISO. Check that it isn't damaged or in use. {}", e.message),
            )
        })?;
        let line = out.lines().rev().find(|l| l.contains('|')).unwrap_or("").trim().to_string();
        let (letter, ours) = line.split_once('|').unwrap_or(("", "False"));
        let letter = letter.trim();
        if letter.len() != 1 {
            return Err(EngineError::new("WIP-E042", "The ISO opened but Windows didn't give it a drive letter."));
        }
        Ok(IsoMount {
            iso: iso.to_path_buf(),
            root: PathBuf::from(format!("{letter}:\\")),
            ours: ours.eq_ignore_ascii_case("true"),
            job: job.clone(),
        })
    }
}

impl Drop for IsoMount {
    fn drop(&mut self) {
        if self.ours {
            let script =
                format!("Dismount-DiskImage -ImagePath {} | Out-Null", ps_quote(&self.iso.display().to_string()));
            if let Err(e) = ps(&script, &self.job) {
                self.job.log(format!("Couldn't detach the ISO: {e}"));
            }
        }
    }
}

fn find_install_image(root: &Path) -> Result<String> {
    for name in ["install.wim", "install.esd"] {
        if root.join("sources").join(name).is_file() {
            return Ok(format!(r"sources\{name}"));
        }
    }
    if root.join(r"sources\install.swm").is_file() {
        return Err(EngineError::new(
            "WIP-E043",
            "This media uses a split image (install.swm). Split images aren't supported yet; use an ISO from Microsoft.",
        ));
    }
    Err(EngineError::new(
        "WIP-E044",
        "This ISO doesn't contain a Windows installation image (sources\\install.wim or install.esd).",
    ))
}

pub fn inspect_iso(iso: &Path, host: &HostInfo, job: &Job) -> Result<SourceInfo> {
    need_admin(host)?;
    let (dism, _) = need_dism(host)?;
    let iso_bytes =
        fs::metadata(iso).map_err(|e| EngineError::new("WIP-E045", format!("Can't read the ISO: {e}")))?.len();
    job.stage(1, 2, "Opening the ISO");
    let m = IsoMount::attach(iso, job)?;
    job.stage(2, 2, "Reading Windows editions");
    let image_rel = find_install_image(&m.root)?;
    let images = dism.image_info(&m.root.join(&image_rel))?;
    if images.is_empty() {
        return Err(EngineError::new("WIP-E046", "No Windows editions were found in this ISO."));
    }
    Ok(SourceInfo { iso: iso.to_path_buf(), iso_bytes, image_rel, images })
}

/// Recursively lists files under `root` with their sizes.
pub(crate) fn walk(root: &Path, rel: &Path, out: &mut Vec<(PathBuf, u64)>) -> std::io::Result<()> {
    for entry in fs::read_dir(root.join(rel))? {
        let entry = entry?;
        let r = rel.join(entry.file_name());
        let ty = entry.file_type()?;
        if ty.is_dir() {
            walk(root, &r, out)?;
        } else if ty.is_file() {
            out.push((r, entry.metadata()?.len()));
        }
    }
    Ok(())
}

pub(crate) fn copy_tree(src: &Path, dst: &Path, skip: &[&str], job: &Job) -> Result<u64> {
    let io = |e: std::io::Error| EngineError::new("WIP-E050", format!("Copying files failed: {e}"));
    let mut files = Vec::new();
    walk(src, Path::new(""), &mut files).map_err(io)?;
    files.retain(|(p, _)| !skip.iter().any(|s| p.as_os_str().eq_ignore_ascii_case(s)));
    let total: u64 = files.iter().map(|f| f.1).sum::<u64>().max(1);
    let mut done = 0u64;
    let mut buf = vec![0u8; 4 << 20];
    for (rel, _) in &files {
        job.check()?;
        let to = dst.join(rel);
        if let Some(parent) = to.parent() {
            fs::create_dir_all(parent).map_err(io)?;
        }
        let mut r = fs::File::open(src.join(rel)).map_err(io)?;
        let mut w = fs::File::create(&to).map_err(io)?;
        loop {
            let n = r.read(&mut buf).map_err(io)?;
            if n == 0 {
                break;
            }
            w.write_all(&buf[..n]).map_err(io)?;
            done += n as u64;
            job.progress(done as f32 / total as f32);
        }
        // Files from an ISO are read-only; the copies must be editable.
        let mut perm = w.metadata().map_err(io)?.permissions();
        #[allow(clippy::permissions_set_readonly_false)]
        perm.set_readonly(false);
        let _ = fs::set_permissions(&to, perm);
    }
    Ok(done)
}

fn unique_workspace(parent: &Path) -> PathBuf {
    let stamp = wip_core::journal::now_secs();
    let mut n = 0;
    loop {
        let name = if n == 0 { format!("WinISOPilot-{stamp}") } else { format!("WinISOPilot-{stamp}-{n}") };
        let p = parent.join(name);
        if !p.exists() {
            return p;
        }
        n += 1;
    }
}

fn export(
    dism_exe: &Path,
    src: &Path,
    index: u32,
    dst: &Path,
    compression: &str,
    scratch: &Path,
    job: &Job,
) -> Result<()> {
    let args = vec![
        "/Export-Image".to_string(),
        opt("SourceImageFile", src),
        format!("/SourceIndex:{index}"),
        opt("DestinationImageFile", dst),
        format!("/Compress:{compression}"),
        "/CheckIntegrity".into(),
        opt("ScratchDir", scratch),
    ];
    expect_ok(run_with_progress(dism_exe, &args, job)?, "Exporting the image", "WIP-E051").map(|_| ())
}

fn read_appx(mount: &Path, job: &Job) -> Result<Vec<AppxPackage>> {
    let script = format!(
        "$ErrorActionPreference='Stop'; Get-AppxProvisionedPackage -Path {} | Select-Object DisplayName,PackageName | ConvertTo-Json -Compress",
        ps_quote(&mount.display().to_string())
    );
    let out = ps(&script, job)?;
    let text = out.trim();
    if text.is_empty() {
        return Ok(Vec::new());
    }
    let v: serde_json::Value = serde_json::from_str(text)
        .map_err(|e| EngineError::new("WIP-E047", format!("Couldn't read the app list: {e}")))?;
    let items = match v {
        serde_json::Value::Array(a) => a,
        other => vec![other],
    };
    Ok(items
        .into_iter()
        .filter_map(|i| {
            Some(AppxPackage {
                display_name: i.get("DisplayName")?.as_str()?.to_string(),
                package_name: i.get("PackageName")?.as_str()?.to_string(),
            })
        })
        .collect())
}

pub fn read_inventory(dism: &Dism, ws_mount: &Path, media: &Path, job: &Job) -> Result<Inventory> {
    let (features, capabilities) = {
        let s = dism.session(ws_mount)?;
        (s.features()?, s.capabilities())
    };
    job.log(format!("{} features, {} capabilities", features.len(), capabilities.len()));
    let appx = read_appx(ws_mount, job)?;
    job.log(format!("{} provisioned apps", appx.len()));
    let services = registry::services(ws_mount, job)?.into_iter().collect();
    Ok(Inventory { appx, capabilities, features, services, has_sxs: media.join(r"sources\sxs").is_dir() })
}

pub fn prepare(req: &PrepareRequest, host: &HostInfo, job: &Job) -> Result<(Workspace, Inventory)> {
    need_admin(host)?;
    let (dism, dism_exe) = need_dism(host)?;
    const STAGES: u32 = 4;

    let root = unique_workspace(&req.workspace_parent);
    let media = root.join("media");
    let work = root.join("work");
    let mount_dir = root.join("mount");
    let boot_mount_dir = root.join("mount-boot");
    let scratch = root.join("scratch");
    let log_file = root.join("winisopilot.log");
    for d in [&media, &work, &mount_dir, &boot_mount_dir, &scratch] {
        fs::create_dir_all(d)
            .map_err(|e| EngineError::new("WIP-E052", format!("Can't create {}: {e}", d.display())))?;
    }
    job.log(format!("Workspace: {}", root.display()));

    job.stage(1, STAGES, "Copying installation files");
    let m = IsoMount::attach(&req.iso, job)?;
    let src_image = m.root.join(&req.image_rel);
    let editions = dism.image_info(&src_image)?;
    let edition = editions
        .iter()
        .find(|e| e.index == req.index)
        .cloned()
        .ok_or_else(|| EngineError::new("WIP-E048", format!("Edition {} isn't in this ISO.", req.index)))?;
    let iso_bytes = fs::metadata(&req.iso).map(|m| m.len()).unwrap_or(0);
    let need = required_space(iso_bytes, edition.size_bytes);
    if let Some(free) = super::host::free_space(&root) {
        if free < need {
            return Err(EngineError::new(
                "WIP-E053",
                format!("Not enough free space: about {} needed, {} free.", human_bytes(need), human_bytes(free)),
            ));
        }
    }
    let skip = [r"sources\install.wim", r"sources\install.esd"];
    let copied = copy_tree(&m.root, &media, &skip, job)?;
    job.log(format!("Copied {}", human_bytes(copied)));

    job.stage(2, STAGES, "Extracting the edition");
    let work_wim = work.join("install.wim");
    let indexes: Vec<u32> = if req.single_edition { vec![req.index] } else { req.all_indexes.clone() };
    let mut work_index = 1;
    for (pos, &i) in indexes.iter().enumerate() {
        job.check()?;
        job.log(format!("Exporting edition {i} ({}/{})", pos + 1, indexes.len()));
        export(&dism_exe, &src_image, i, &work_wim, "max", &scratch, job)?;
        if i == req.index {
            work_index = pos as u32 + 1;
        }
    }
    drop(m);

    job.stage(3, STAGES, "Opening the edition for editing");
    journal::record_mount(
        &work_wim.display().to_string(),
        work_index,
        &mount_dir.display().to_string(),
        &root.display().to_string(),
    )
    .map_err(|e| EngineError::new("WIP-E054", format!("Couldn't write the recovery journal: {e}")))?;
    if let Err(e) = dism.mount(&work_wim, work_index, &mount_dir, job) {
        journal::clear_mount(&mount_dir.display().to_string());
        return Err(e);
    }

    job.stage(4, STAGES, "Reading what's inside");
    let inv = match read_inventory(&dism, &mount_dir, &media, job) {
        Ok(inv) => inv,
        Err(e) => {
            // Don't leave a half-prepared mount behind.
            job.log("Reading the image failed; closing it without saving.");
            match dism.unmount(&mount_dir, false, job) {
                Ok(()) => journal::clear_mount(&mount_dir.display().to_string()),
                Err(u) => job.log(format!("Couldn't close the image: {u}")),
            }
            return Err(e);
        }
    };
    let ws = Workspace { root, media, work_wim, work_index, mount_dir, boot_mount_dir, scratch, log_file, edition };
    let _ = wip_core::fsutil::atomic_write(
        &ws.root.join("workspace.json"),
        serde_json::to_string_pretty(&ws).unwrap_or_default().as_bytes(),
    );
    Ok((ws, inv))
}

/// Tracks what's open while applying operations, so failures clean up.
struct Applier<'a> {
    dism: &'a Dism,
    dism_exe: &'a Path,
    mount: &'a Path,
    media: &'a Path,
    scratch: &'a Path,
    session: Option<Session<'a>>,
    loaded: Vec<Hive>,
    /// Problems that didn't stop the build (e.g. one driver failing).
    warnings: Vec<String>,
}

impl<'a> Applier<'a> {
    fn session(&mut self) -> Result<&Session<'a>> {
        if self.session.is_none() {
            self.session = Some(self.dism.session(self.mount)?);
        }
        Ok(self.session.as_ref().expect("just set"))
    }

    fn close_session(&mut self) {
        self.session = None;
    }

    fn apply(&mut self, op: &Op, job: &Job) -> Result<()> {
        match op {
            Op::RemoveAppx { package_name, .. } => {
                self.close_session();
                let args = vec![
                    opt("Image", self.mount),
                    "/Remove-ProvisionedAppxPackage".into(),
                    format!("/PackageName:{package_name}"),
                ];
                expect_ok(run(self.dism_exe, &args, job)?, "Removing an app", "WIP-E060")?;
            }
            Op::RemoveCapability { name } => self.session()?.remove_capability(name, job)?,
            Op::DisableFeature { name, remove_payload } => {
                self.session()?.disable_feature(name, *remove_payload, job)?
            }
            Op::EnableFeature { name, source_sxs } => {
                let sxs = self.media.join(r"sources\sxs");
                let src = (*source_sxs && sxs.is_dir()).then_some(sxs);
                self.session()?.enable_feature(name, src.as_deref(), job)?
            }
            Op::AddDriver { inf } => {
                let p = Path::new(inf);
                if !p.is_file() {
                    return Err(EngineError::new(
                        "WIP-E063",
                        format!("Driver file {inf} is gone. Add the driver folder again, or remove it."),
                    ));
                }
                // One bad driver package must not fail the whole build.
                if let Err(e) = self.session()?.add_driver(p, job) {
                    if e.is_cancelled() {
                        return Err(e);
                    }
                    job.log(format!("Driver skipped: {e}"));
                    self.warnings.push(format!("A driver couldn't be added and was skipped: {e}"));
                }
            }
            Op::AddPackage { path, kind } => {
                self.close_session();
                let p = Path::new(path);
                if !p.is_file() {
                    return Err(EngineError::new(
                        "WIP-E064",
                        format!("Package {path} is gone. Add it again, or remove it from Updates."),
                    ));
                }
                let args = vec![
                    opt("Image", self.mount),
                    "/Add-Package".into(),
                    opt("PackagePath", p),
                    opt("ScratchDir", self.scratch),
                ];
                let what = format!("Adding {} {}", kind.label().to_lowercase(), wip_core::packages::file_name(p));
                expect_ok(run_with_progress(self.dism_exe, &args, job)?, &what, "WIP-E064")?;
            }
            Op::DeleteFile { path } => {
                self.close_session();
                let p = self.mount.join(path);
                match fs::remove_file(&p) {
                    Ok(()) => {}
                    Err(e) if e.kind() == std::io::ErrorKind::NotFound => job.log(format!("{path} isn't present.")),
                    Err(e) => return Err(EngineError::new("WIP-E061", format!("Couldn't delete {path}: {e}"))),
                }
            }
            Op::LoadHive { hive } => {
                self.close_session();
                registry::load(*hive, self.mount, job)?;
                self.loaded.push(*hive);
            }
            Op::RegSet { hive, key, value, data } => registry::set(*hive, key, value, data, job)?,
            Op::RegDelete { hive, key, value } => registry::delete(*hive, key, value, job)?,
            Op::UnloadHive { hive } => {
                registry::unload(*hive, job)?;
                self.loaded.retain(|h| h != hive);
            }
            Op::ComponentCleanup { reset_base } => {
                self.close_session();
                let mut args = vec![opt("Image", self.mount), "/Cleanup-Image".into(), "/StartComponentCleanup".into()];
                if *reset_base {
                    args.push("/ResetBase".into());
                }
                args.push(opt("ScratchDir", self.scratch));
                expect_ok(run_with_progress(self.dism_exe, &args, job)?, "Component cleanup", "WIP-E062")?;
            }
        }
        Ok(())
    }

    /// Best-effort release of everything still open.
    fn release(&mut self, job: &Job) {
        self.close_session();
        for h in std::mem::take(&mut self.loaded) {
            if let Err(e) = registry::unload(h, job) {
                job.log(format!("Cleanup: {e}"));
            }
        }
    }

    fn run_all(&mut self, ops: &[PlannedOp], job: &Job) -> Result<usize> {
        let total: u32 = ops.iter().map(|o| o.op.weight()).sum::<u32>().max(1);
        let mut done = 0u32;
        let mut applied = 0;
        for (n, p) in ops.iter().enumerate() {
            if let Err(e) = job.check() {
                self.release(job);
                return Err(e);
            }
            job.log(format!("[{}/{}] {}", n + 1, ops.len(), p.op.describe()));
            if let Err(e) = self.apply(&p.op, job) {
                self.release(job);
                return Err(e);
            }
            applied += 1;
            done += p.op.weight();
            job.progress(done as f32 / total as f32);
        }
        self.close_session();
        Ok(applied)
    }
}

/// Microsoft's Make2023BootableMedia.ps1 steps (microsoft/secureboot_objects):
/// from boot.wim's `Windows\Boot\{EFI_EX,FONTS_EX,DVD_EX}` copy
/// bootmgfw_EX.efi → efi\boot\bootx64.efi (bootaa64.efi on ARM64),
/// bootmgr_EX.efi → bootmgr.efi (if present), efisys_EX.bin →
/// efi\microsoft\boot\efisys_ex.bin (used by `make_iso`), fonts *_EX.ttf →
/// efi\microsoft\boot\fonts\*.ttf, and boot.stl. boot.wim is mounted
/// read-only. Returns `Ok(false)` when the image has no EFI_EX files.
fn use_2023_boot_files(dism: &Dism, ws: &Workspace, job: &Job) -> Result<bool> {
    let boot_wim = ws.media.join(r"sources\boot.wim");
    let mnt = &ws.boot_mount_dir;
    journal::record_mount(
        &boot_wim.display().to_string(),
        1,
        &mnt.display().to_string(),
        &ws.root.display().to_string(),
    )
    .map_err(|e| EngineError::new("WIP-E054", format!("Couldn't write the recovery journal: {e}")))?;
    dism.mount_read_only(&boot_wim, 1, mnt, job)?;
    let result = copy_2023_files(mnt, &ws.media);
    let unmounted = dism.unmount(mnt, false, job);
    journal::clear_mount(&mnt.display().to_string());
    unmounted?;
    result
}

fn copy_2023_files(mnt: &Path, media: &Path) -> Result<bool> {
    let ex = mnt.join(r"Windows\Boot\EFI_EX");
    let fonts = mnt.join(r"Windows\Boot\FONTS_EX");
    let efisys = mnt.join(r"Windows\Boot\DVD_EX\EFI\en-US\efisys_EX.bin");
    if !ex.join("bootmgfw_EX.efi").is_file() || !efisys.is_file() {
        return Ok(false);
    }
    let err = |what: &str, e: std::io::Error| EngineError::new("WIP-E074", format!("Couldn't copy {what}: {e}"));
    let arch_name = if media.join(r"efi\boot\bootaa64.efi").is_file() { "bootaa64.efi" } else { "bootx64.efi" };
    let ms_boot = media.join(r"efi\microsoft\boot");
    fs::create_dir_all(media.join(r"efi\boot")).map_err(|e| err("efi\\boot", e))?;
    fs::create_dir_all(ms_boot.join("fonts")).map_err(|e| err("fonts", e))?;
    fs::copy(ex.join("bootmgfw_EX.efi"), media.join(r"efi\boot").join(arch_name)).map_err(|e| err(arch_name, e))?;
    if ex.join("bootmgr_EX.efi").is_file() {
        fs::copy(ex.join("bootmgr_EX.efi"), media.join("bootmgr.efi")).map_err(|e| err("bootmgr.efi", e))?;
    }
    fs::copy(&efisys, ms_boot.join("efisys_ex.bin")).map_err(|e| err("efisys_ex.bin", e))?;
    if let Ok(entries) = fs::read_dir(&fonts) {
        for f in entries.flatten() {
            let name = f.file_name().to_string_lossy().into_owned();
            let target = name.replace("_EX", "");
            fs::copy(f.path(), ms_boot.join("fonts").join(&target)).map_err(|e| err(&name, e))?;
        }
    }
    let stl = mnt.join(r"Windows\Boot\EFI\boot.stl");
    if stl.is_file() {
        fs::copy(&stl, ms_boot.join("boot.stl")).map_err(|e| err("boot.stl", e))?;
    }
    Ok(true)
}

/// Issuer of the certificate that signed the media's UEFI boot manager
/// (e.g. "Microsoft Windows Production PCA 2011" or "Windows UEFI CA 2023").
fn boot_signer(media: &Path, job: &Job) -> Option<String> {
    let efi = ["bootx64.efi", "bootaa64.efi"].iter().map(|n| media.join(r"efi\boot").join(n)).find(|p| p.is_file())?;
    let script = format!(
        "$s = Get-AuthenticodeSignature -LiteralPath {}; $s.SignerCertificate.Issuer",
        wip_core::boottest::ps_lit(&efi.display().to_string())
    );
    let out = ps(&script, job).ok()?;
    wip_core::secureboot::issuer_cn(out.trim())
}

/// Refuses tools that aren't validly signed by Microsoft (catalog or
/// embedded signature). If PowerShell itself can't run the check, the build
/// continues and the log says why.
fn check_microsoft_signed(path: &Path, job: &Job) -> Result<()> {
    let script = format!(
        "$s = Get-AuthenticodeSignature -LiteralPath {}; \"$($s.Status)|$($s.SignerCertificate.Subject)\"",
        wip_core::boottest::ps_lit(&path.display().to_string())
    );
    let out = match ps(&script, job) {
        Ok(o) => o.trim().to_string(),
        Err(e) => {
            job.log(format!("Signature check skipped for {}: {e}", path.display()));
            return Ok(());
        }
    };
    let (status, subject) = out.split_once('|').unwrap_or((out.as_str(), ""));
    if status == "Valid" && subject.contains("O=Microsoft Corporation") {
        job.log(format!("Signed by Microsoft: {}", path.display()));
        return Ok(());
    }
    Err(EngineError::new(
        "WIP-E073",
        format!(
            "{} isn't validly signed by Microsoft (status: {status}). It may be damaged or replaced. Reinstall the Windows ADK, or pick a different DISM folder in Settings.",
            path.display()
        ),
    ))
}

pub fn build(
    ws: &Workspace,
    plan: &CompiledPlan,
    output: &OutputSettings,
    iso_out: &Path,
    host: &HostInfo,
    before: &Inventory,
    job: &Job,
) -> Result<BuildReport> {
    need_admin(host)?;
    let (dism, dism_exe) = need_dism(host)?;
    check_microsoft_signed(&dism_exe, job)?;
    if let Some(o) = &host.oscdimg {
        check_microsoft_signed(o, job)?;
    }
    let work_wim_bytes = fs::metadata(&ws.work_wim).map(|m| m.len()).unwrap_or(0);
    if iso_out.exists() {
        return Err(EngineError::new(
            "WIP-E070",
            format!("{} already exists. Choose a new file name.", iso_out.display()),
        ));
    }
    let boot = !plan.boot.is_empty();
    let stages = 4 + boot as u32 + output.boot_2023 as u32;
    let mut stage = 0;
    let mut next = |label: &str| {
        stage += 1;
        job.stage(stage, stages, label);
    };
    let mut warnings = Vec::new();

    next("Applying your changes");
    let mut applier = Applier {
        dism: &dism,
        dism_exe: &dism_exe,
        mount: &ws.mount_dir,
        media: &ws.media,
        scratch: &ws.scratch,
        session: None,
        loaded: Vec::new(),
        warnings: Vec::new(),
    };
    let mut applied = applier.run_all(&plan.install, job)?;
    warnings.append(&mut applier.warnings);
    drop(applier);

    // Measure what's left, for the footprint report. Never fails the build.
    let after_inv = match read_inventory(&dism, &ws.mount_dir, &ws.media, job) {
        Ok(i) => Some(i),
        Err(e) => {
            job.log(format!("Footprint: couldn't read the changed image: {e}"));
            None
        }
    };

    next("Saving the edition");
    dism.unmount(&ws.mount_dir, true, job)?;
    journal::clear_mount(&ws.mount_dir.display().to_string());

    if boot {
        next("Changing Windows Setup");
        let boot_wim = ws.media.join(r"sources\boot.wim");
        journal::record_mount(
            &boot_wim.display().to_string(),
            2,
            &ws.boot_mount_dir.display().to_string(),
            &ws.root.display().to_string(),
        )
        .map_err(|e| EngineError::new("WIP-E054", format!("Couldn't write the recovery journal: {e}")))?;
        dism.mount(&boot_wim, 2, &ws.boot_mount_dir, job)?;
        let mut a = Applier {
            dism: &dism,
            dism_exe: &dism_exe,
            mount: &ws.boot_mount_dir,
            media: &ws.media,
            scratch: &ws.scratch,
            session: None,
            loaded: Vec::new(),
            warnings: Vec::new(),
        };
        let result = a.run_all(&plan.boot, job);
        warnings.append(&mut a.warnings);
        drop(a);
        match result {
            Ok(n) => {
                applied += n;
                dism.unmount(&ws.boot_mount_dir, true, job)?;
            }
            Err(e) => {
                let _ = dism.unmount(&ws.boot_mount_dir, false, job);
                journal::clear_mount(&ws.boot_mount_dir.display().to_string());
                return Err(e);
            }
        }
        journal::clear_mount(&ws.boot_mount_dir.display().to_string());
    }

    if output.boot_2023 {
        next("Switching to the 2023-signed boot manager");
        match use_2023_boot_files(&dism, ws, job) {
            Ok(true) => job.log("Media now uses the boot manager signed with Windows UEFI CA 2023."),
            Ok(false) => warnings.push(
                "This ISO doesn't contain the 2023-signed boot files (they come with the April 2024 updates or later), so the boot manager wasn't changed.".into(),
            ),
            Err(e) => return Err(e),
        }
    }

    next("Rebuilding the installation image");
    let sources = ws.media.join("sources");
    for old in ["install.wim", "install.esd", "install.swm"] {
        let _ = fs::remove_file(sources.join(old));
    }
    let (dst, comp) = match output.compression {
        Compression::Maximum => (sources.join("install.wim"), "max"),
        Compression::Recovery => (sources.join("install.esd"), "recovery"),
    };
    let count = dism.image_info(&ws.work_wim)?.len() as u32;
    for i in 1..=count {
        job.log(format!("Exporting edition {i}/{count}"));
        export(&dism_exe, &ws.work_wim, i, &dst, comp, &ws.scratch, job)?;
    }
    let kept_index = if count == 1 { 1 } else { ws.work_index };
    let footprint = after_inv.and_then(|after| {
        let expanded_after = dism.image_info(&dst).ok()?.into_iter().find(|i| i.index == kept_index)?.size_bytes;
        let file_after = fs::metadata(&dst).ok()?.len();
        let settings_changed = plan
            .install
            .iter()
            .chain(plan.boot.iter())
            .filter(|p| matches!(p.op, Op::RegSet { .. } | Op::RegDelete { .. }))
            .count();
        Some(Footprint {
            schema: 1,
            edition: ws.edition.name.clone(),
            version: ws.edition.version_string(),
            before: Measure::of(before, ws.edition.size_bytes, work_wim_bytes),
            after: Measure::of(&after, expanded_after, file_after),
            changes: Changes::between(before, &after),
            recompressed: output.compression == Compression::Recovery,
            settings_changed,
        })
    });
    match &footprint {
        Some(f) => {
            let _ = wip_core::fsutil::atomic_write(&ws.root.join("footprint.md"), f.to_markdown().as_bytes());
            let _ = wip_core::fsutil::atomic_write(
                &ws.root.join("footprint.json"),
                serde_json::to_string_pretty(f).unwrap_or_default().as_bytes(),
            );
        }
        None => warnings.push("The before/after footprint couldn't be measured; see the log.".into()),
    }
    if output.compression == Compression::Maximum {
        if let Ok(m) = fs::metadata(&dst) {
            if m.len() > 4 * 1024 * 1024 * 1024 - 1 {
                warnings.push(
                    "install.wim is larger than 4 GB. The ISO is fine, but copying it to a FAT32 USB stick needs splitting."
                        .into(),
                );
            }
        }
    }

    next("Creating the ISO");
    let iso = match &host.oscdimg {
        Some(oscdimg) => {
            make_iso(oscdimg, &ws.media, iso_out, &sanitize_label(&output.volume_label), job)?;
            Some(iso_out.to_path_buf())
        }
        None => {
            warnings.push(format!(
                "oscdimg.exe wasn't found, so no ISO was created. The finished files are in {}.",
                ws.media.display()
            ));
            None
        }
    };
    let iso_bytes = iso.as_ref().and_then(|p| fs::metadata(p).ok()).map(|m| m.len());
    let boot_signer = boot_signer(&ws.media, job);
    // SHA-256 sidecar (`name.iso.sha256`, sha256sum format) so copies can be checked.
    let iso_sha256 = iso.as_ref().and_then(|p| match super::download::file_hash(p, "SHA256", job) {
        Ok(h) if h.len() == 64 => {
            let name = p.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
            let mut side = p.as_os_str().to_owned();
            side.push(".sha256");
            if let Err(e) = fs::write(&side, format!("{} *{name}\n", h.to_ascii_lowercase())) {
                job.log(format!("Couldn't write the .sha256 file: {e}"));
            }
            Some(h)
        }
        Ok(h) => {
            job.log(format!("Unexpected hash output: {h}"));
            None
        }
        Err(e) => {
            job.log(format!("Couldn't hash the ISO: {e}"));
            None
        }
    });
    let test_target = TestTarget {
        language: ws.edition.languages.first().cloned().unwrap_or_else(|| "en-US".into()),
        index: kept_index,
        edition_id: ws.edition.edition_id.clone(),
        arch: TestTarget::arch_from(&ws.edition.architecture),
    };
    Ok(BuildReport {
        iso,
        media_folder: ws.media.clone(),
        iso_bytes,
        iso_sha256,
        boot_signer,
        applied,
        footprint,
        test_target,
        warnings,
        log_file: ws.log_file.clone(),
    })
}

pub(crate) fn make_iso(oscdimg: &Path, media: &Path, out: &Path, label: &str, job: &Job) -> Result<()> {
    let etfs = media.join(r"boot\etfsboot.com");
    // efisys_ex.bin is present when the 2023-signed boot manager was applied.
    let efi = ["efisys_ex.bin", "efisys.bin"]
        .iter()
        .map(|n| media.join(r"efi\microsoft\boot").join(n))
        .find(|p| p.is_file())
        .unwrap_or_else(|| media.join(r"efi\microsoft\boot\efisys.bin"));
    let bootdata = match (etfs.is_file(), efi.is_file()) {
        (true, true) => format!("RAW:-bootdata:2#p0,e,b\"{}\"#pEF,e,b\"{}\"", etfs.display(), efi.display()),
        (false, true) => format!("RAW:-bootdata:1#pEF,e,b\"{}\"", efi.display()),
        (true, false) => format!("RAW:-b\"{}\"", etfs.display()),
        (false, false) => {
            return Err(EngineError::new("WIP-E071", "The media has no boot files (etfsboot.com / efisys.bin)."));
        }
    };
    let args = vec![
        "-m".into(),
        "-o".into(),
        "-u2".into(),
        "-udfver102".into(),
        format!("-l{label}"),
        bootdata,
        quoted(media),
        quoted(out),
    ];
    let result = expect_ok(run_with_progress(oscdimg, &args, job)?, "Creating the ISO", "WIP-E072");
    if result.is_err() {
        let _ = fs::remove_file(out);
    }
    result.map(|_| ())
}

pub fn discard_mount(dir: &Path, host: &HostInfo, job: &Job) -> Result<()> {
    need_admin(host)?;
    let (dism, _) = need_dism(host)?;
    job.stage(1, 1, "Closing the image without saving");
    dism.unmount(dir, false, job)?;
    journal::clear_mount(&dir.display().to_string());
    Ok(())
}
