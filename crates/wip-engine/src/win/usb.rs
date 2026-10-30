//! Bootable USB writer: diskpart (erase + FAT32), file copy, DISM split for
//! large install.wim, bootsect for BIOS, optional read-back verification.

use super::proc::{expect_ok, opt, run, run_with_progress};
use super::workflow::{ps, walk, IsoMount};
use crate::{EngineError, HostInfo, Job, Result, UsbReport, UsbRequest, UsbSource};
use std::fs;
use std::io::Read;
use std::path::{Path, PathBuf};
use wip_core::usb::{self, Disk, Placement, Scheme};

const LIST_SCRIPT: &str = "$ErrorActionPreference='Stop'; \
$d = Get-Disk | ForEach-Object { $n = $_.Number; \
$v = @(Get-Partition -DiskNumber $n -ErrorAction SilentlyContinue | Where-Object DriveLetter | ForEach-Object { [string]$_.DriveLetter }); \
[pscustomobject]@{ Number = $n; FriendlyName = $_.FriendlyName; SerialNumber = $_.SerialNumber; UniqueId = $_.UniqueId; \
Size = $_.Size; BusType = [string]$_.BusType; IsBoot = $_.IsBoot; IsSystem = $_.IsSystem; IsReadOnly = $_.IsReadOnly; \
IsOffline = $_.IsOffline; Letters = $v } }; \
ConvertTo-Json -InputObject @($d) -Compress -Depth 3";

pub fn list_disks(job: &Job) -> Result<Vec<Disk>> {
    let out = ps(LIST_SCRIPT, job)?;
    usb::parse_disks(&out).map_err(|e| EngineError::new("WIP-E080", e))
}

fn drive_letter_of(p: &Path) -> Option<char> {
    let s = p.to_string_lossy();
    let mut c = s.chars();
    match (c.next(), c.next()) {
        (Some(l), Some(':')) if l.is_ascii_alphabetic() => Some(l.to_ascii_uppercase()),
        _ => None,
    }
}

fn diskpart() -> PathBuf {
    super::host::system32().map(|s| s.join("diskpart.exe")).unwrap_or_else(|| PathBuf::from("diskpart.exe"))
}

/// Drive letters of the new setup partition (FAT32/NTFS) and, if made, the
/// exFAT data partition. Windows can take a moment to assign them.
fn find_letters(number: u32, job: &Job) -> Result<(char, Option<char>)> {
    let script = format!(
        "$r=@(); for($i=0; $i -lt 40; $i++){{ \
         $r = @(Get-Partition -DiskNumber {number} -ErrorAction SilentlyContinue | Where-Object DriveLetter | \
         ForEach-Object {{ $v = Get-Volume -Partition $_ -ErrorAction SilentlyContinue; \
         '{{0}}={{1}}' -f $_.DriveLetter, $v.FileSystemType }}); \
         if($r.Count -gt 0 -and @($r -match '=$').Count -eq 0){{ break }}; Start-Sleep -Milliseconds 250 }}; $r -join ';'"
    );
    let out = ps(&script, job)?;
    job.log(format!("Partitions: {}", out.trim()));
    let mut setup = None;
    let mut data = None;
    for part in out.trim().split(';') {
        let (l, fs) = part.split_once('=').unwrap_or((part, ""));
        let Some(c) = l.chars().find(|c| c.is_ascii_alphabetic()) else { continue };
        if fs.eq_ignore_ascii_case("exFAT") {
            data = Some(c);
        } else if setup.is_none() {
            setup = Some(c);
        }
    }
    let setup = setup.ok_or_else(|| {
        EngineError::new("WIP-E083", "The USB drive was formatted but Windows didn't give it a drive letter.")
    })?;
    Ok((setup, data))
}

const FILE_FLAG_WRITE_THROUGH: u32 = 0x8000_0000;
const FILE_FLAG_NO_BUFFERING: u32 = 0x2000_0000;
const ALIGN: usize = 4096;

/// A buffer whose start is aligned for unbuffered (direct) reads.
struct Aligned {
    raw: Vec<u8>,
    off: usize,
    len: usize,
}

impl Aligned {
    fn new(len: usize) -> Aligned {
        let raw = vec![0u8; len + ALIGN];
        let off = (ALIGN - (raw.as_ptr() as usize % ALIGN)) % ALIGN;
        Aligned { raw, off, len }
    }
    fn buf(&mut self) -> &mut [u8] {
        &mut self.raw[self.off..self.off + self.len]
    }
}

/// Copies the media to the stick with write-through I/O, so the progress
/// bar shows data that is really on the drive rather than in the cache.
fn copy_to_usb(src: &Path, dst: &Path, skip: &[&str], job: &Job) -> Result<u64> {
    use std::io::Write;
    use std::os::windows::fs::OpenOptionsExt;
    let io = |e: std::io::Error| EngineError::new("WIP-E094", format!("Copying to the USB drive failed: {e}"));
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
        let mut w = fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .custom_flags(FILE_FLAG_WRITE_THROUGH)
            .open(&to)
            .map_err(io)?;
        loop {
            let n = r.read(&mut buf).map_err(io)?;
            if n == 0 {
                break;
            }
            w.write_all(&buf[..n]).map_err(io)?;
            done += n as u64;
            job.progress(done as f32 / total as f32);
        }
        w.sync_all().map_err(io)?;
    }
    Ok(done)
}

/// Compares a source file with its copy on the stick. The copy is read with
/// unbuffered I/O so the bytes come from the drive, not the file cache.
fn same_bytes(src: &Path, on_usb: &Path, a: &mut [u8], b: &mut Aligned) -> std::io::Result<bool> {
    use std::os::windows::fs::OpenOptionsExt;
    let mut fa = fs::File::open(src)?;
    let mut fb = fs::OpenOptions::new().read(true).custom_flags(FILE_FLAG_NO_BUFFERING).open(on_usb)?;
    if fa.metadata()?.len() != fb.metadata()?.len() {
        return Ok(false);
    }
    loop {
        // Unbuffered reads must ask for whole sectors; the last read returns
        // only what's left of the file.
        let n = fb.read(b.buf())?;
        if n == 0 {
            let mut probe = [0u8; 1];
            return Ok(fa.read(&mut probe)? == 0);
        }
        fa.read_exact(&mut a[..n])?;
        if a[..n] != b.buf()[..n] {
            return Ok(false);
        }
    }
}

pub fn write(req: &UsbRequest, host: &HostInfo, job: &Job) -> Result<UsbReport> {
    if !host.is_admin {
        return Err(EngineError::new("WIP-E002", "Administrator rights are needed to write a USB drive."));
    }
    let mut warnings = Vec::new();
    let stages = if req.verify { 5 } else { 4 };

    // 1. Re-check the drive and the source before touching anything.
    job.stage(1, stages, "Checking the drive and the files");
    let now = list_disks(job)?;
    let current = now
        .iter()
        .find(|d| d.number == req.disk.number)
        .ok_or_else(|| EngineError::new("WIP-E081", "The chosen USB drive is no longer connected."))?;
    if !current.same_disk(&req.disk) {
        return Err(EngineError::new(
            "WIP-E082",
            "The drive list changed since you chose a drive. Refresh and choose the drive again.",
        ));
    }
    let src_path = match &req.source {
        UsbSource::Iso(p) | UsbSource::Folder(p) => p.clone(),
    };
    let mut protected: Vec<char> = drive_letter_of(&src_path).into_iter().collect();
    protected.extend(req.protected_paths.iter().filter_map(|p| drive_letter_of(p)));
    if let Some(why) = usb::ineligible(current, &protected) {
        return Err(EngineError::new("WIP-E084", why));
    }
    let _mount;
    let root = match &req.source {
        UsbSource::Iso(p) => {
            let m = IsoMount::attach(p, job)?;
            let r = m.root.clone();
            _mount = Some(m);
            r
        }
        UsbSource::Folder(p) => {
            _mount = None;
            p.clone()
        }
    };
    if !root.join(r"sources\boot.wim").is_file() {
        return Err(EngineError::new(
            "WIP-E085",
            "This doesn't look like Windows installation media (no sources\\boot.wim).",
        ));
    }
    let mut files = Vec::new();
    walk(&root, Path::new(""), &mut files)
        .map_err(|e| EngineError::new("WIP-E086", format!("Can't read the media: {e}")))?;
    let listing: Vec<(String, u64)> = files.iter().map(|(p, s)| (p.display().to_string(), *s)).collect();
    let plan = usb::plan_files(&listing, usb::partition_bytes(current.size, req.scheme), req.scheme.is_fat32())
        .map_err(|e| EngineError::new("WIP-E087", e))?;
    let split = plan.files.iter().any(|f| f.2 == Placement::SplitWim);
    let dism_exe = if split {
        Some(
            host.dism_exe
                .clone()
                .ok_or_else(|| EngineError::new("WIP-E013", "dism.exe wasn't found (needed to split install.wim)."))?,
        )
    } else {
        None
    };
    job.check()?;

    // 2. Erase and format. Not cancellable once started.
    job.stage(2, stages, format!("Erasing and formatting {}", current.label()));
    let script = usb::diskpart_script(current, req.scheme, &req.label, req.data_partition);
    let script_file = crate::app_data_dir().join(format!("diskpart-{}.txt", std::process::id()));
    wip_core::fsutil::atomic_write(&script_file, script.as_bytes())
        .map_err(|e| EngineError::new("WIP-E088", format!("Couldn't write the diskpart script: {e}")))?;
    job.log(format!("diskpart script:\n{script}"));
    let out = run(&diskpart(), &["/s".into(), script_file.display().to_string()], job);
    let _ = fs::remove_file(&script_file);
    let text = expect_ok(out?, "Formatting the USB drive", "WIP-E089")?;
    job.log(text.trim().to_string());
    let (letter, data_letter) = find_letters(current.number, job)?;
    let target = PathBuf::from(format!("{letter}:\\"));
    job.log(format!("USB drive is {letter}:"));
    if req.data_partition && usb::leftover_mb(current.size, req.scheme).is_some() && data_letter.is_none() {
        warnings.push(
            "The extra DATA partition was created but has no drive letter; assign one in Disk Management.".into(),
        );
    }

    // 3. Copy (and split install.wim if needed).
    job.stage(3, stages, "Copying Windows files");
    let skip: Vec<&str> = if split { vec![r"sources\install.wim"] } else { Vec::new() };
    let incomplete = |e: EngineError| {
        if e.is_cancelled() {
            EngineError::new("WIP-E001", "Stopped. The USB drive is incomplete; write it again before using it.")
        } else {
            e
        }
    };
    let written = copy_to_usb(&root, &target, &skip, job).map_err(incomplete)?;
    if let Some(dism) = dism_exe {
        job.log("Splitting install.wim into install.swm parts");
        job.indeterminate();
        let args = vec![
            "/Split-Image".to_string(),
            opt("ImageFile", &root.join(r"sources\install.wim")),
            opt("SWMFile", &target.join(r"sources\install.swm")),
            format!("/FileSize:{}", usb::SWM_SPLIT_MB),
        ];
        expect_ok(run_with_progress(&dism, &args, job)?, "Splitting install.wim", "WIP-E090")?;
    }

    // 4. Boot code.
    job.stage(4, stages, "Making the drive bootable");
    if req.scheme == Scheme::GptNtfs {
        warnings.push(
            "GPT + NTFS boots only on UEFI PCs whose firmware can read NTFS. If a PC doesn't list the drive, write it again with MBR + FAT32."
                .into(),
        );
    }
    if req.scheme == Scheme::Mbr {
        let bootsect = root.join(r"boot\bootsect.exe");
        if bootsect.is_file() {
            let args = vec!["/nt60".into(), format!("{letter}:"), "/force".into(), "/mbr".into()];
            expect_ok(run(&bootsect, &args, job)?, "Writing BIOS boot code", "WIP-E091")?;
        } else {
            warnings.push(
                "boot\\bootsect.exe wasn't on the media, so BIOS boot code wasn't written. UEFI boot still works."
                    .into(),
            );
        }
    }
    let efi_ok = ["efi\\boot\\bootx64.efi", "efi\\boot\\bootaa64.efi"].iter().any(|p| target.join(p).is_file());
    if !efi_ok {
        warnings.push(
            "No UEFI boot loader (efi\\boot\\bootx64.efi) was found on the media; UEFI PCs may not boot it.".into(),
        );
    }

    // 5. Verify.
    let mut verified = false;
    if req.verify {
        job.stage(5, stages, "Checking every copied file");
        let to_check: Vec<_> = plan.files.iter().filter(|f| f.2 == Placement::Copy).collect();
        let total: u64 = to_check.iter().map(|f| f.1).sum::<u64>().max(1);
        let mut done = 0u64;
        let (mut a, mut b) = (vec![0u8; 4 << 20], Aligned::new(4 << 20));
        for (rel, size, _) in to_check {
            job.check().map_err(incomplete)?;
            let ok = same_bytes(&root.join(rel), &target.join(rel), &mut a, &mut b)
                .map_err(|e| EngineError::new("WIP-E092", format!("Couldn't read back {rel}: {e}")))?;
            if !ok {
                return Err(EngineError::new(
                    "WIP-E093",
                    format!(
                        "{rel} on the USB drive doesn't match the original. The drive may be faulty; try another one."
                    ),
                ));
            }
            done += size;
            job.progress(done as f32 / total as f32);
        }
        verified = true;
    }

    Ok(UsbReport {
        letter: letter.to_string(),
        bytes_written: written,
        split,
        verified,
        data_letter: data_letter.map(|c| c.to_string()),
        warnings,
    })
}
