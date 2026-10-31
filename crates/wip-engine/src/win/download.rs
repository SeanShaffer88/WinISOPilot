//! Download official Windows from Microsoft (Media Creation Tool catalog)
//! with curl.exe, verify the catalog hash, and turn the ESD into an ISO.

use super::host::system32;
use super::proc::{expect_ok, opt, run, run_with_progress};
use super::workflow::{make_iso, ps};
use crate::{EngineError, HostInfo, Job, Result};
use std::fs;
use std::io::Read;
use std::os::windows::process::CommandExt;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use wip_core::download::{self, EsdFile};
use wip_core::recipe::sanitize_label;

const CREATE_NO_WINDOW: u32 = 0x0800_0000;

fn curl() -> Result<PathBuf> {
    system32().map(|s| s.join("curl.exe")).filter(|p| p.is_file()).ok_or_else(|| {
        EngineError::new("WIP-E110", "curl.exe wasn't found in System32 (needs Windows 10 1803 or newer).")
    })
}

/// Runs curl, forwarding its progress bar. Kills curl on cancel (a partial
/// download is kept and resumed next time).
fn curl_download(url: &str, out: &Path, job: &Job) -> Result<()> {
    if !download::host_allowed(url) {
        return Err(EngineError::new(
            "WIP-E111",
            format!("Refusing to download from {url}: not a Microsoft download host."),
        ));
    }
    let exe = curl()?;
    job.log(format!("> curl {url}"));
    let mut child = Command::new(&exe)
        .args(["-fL", "--retry", "3", "--retry-delay", "2", "-C", "-", "--proto", "=https,http", "-#", "-o"])
        .arg(out)
        .arg(url)
        .creation_flags(CREATE_NO_WINDOW)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| EngineError::new("WIP-E020", format!("Couldn't start curl: {e}")))?;
    let mut err = child.stderr.take().expect("piped");
    let mut buf = [0u8; 2048];
    let mut tail = String::new();
    loop {
        if job.cancelled() {
            let _ = child.kill();
            let _ = child.wait();
            return Err(EngineError::new(
                "WIP-E001",
                "Download stopped. It will continue where it left off next time.",
            ));
        }
        match err.read(&mut buf) {
            Ok(0) | Err(_) => break,
            Ok(n) => {
                let s = String::from_utf8_lossy(&buf[..n]);
                if let Some(p) = super::proc::last_percent(&s) {
                    job.progress(p / 100.0);
                }
                tail.push_str(&s);
                if tail.len() > 4000 {
                    tail.drain(..tail.len() - 2000);
                }
            }
        }
    }
    let status = child.wait().map_err(|e| EngineError::new("WIP-E020", e.to_string()))?;
    if !status.success() {
        let msg = tail.lines().rfind(|l| l.contains("curl:")).unwrap_or("").trim().to_string();
        return Err(EngineError::new(
            "WIP-E112",
            format!(
                "The download failed (curl exit {}). {msg} Check your internet connection and try again.",
                status.code().unwrap_or(-1)
            ),
        ));
    }
    Ok(())
}

/// Downloads and reads Microsoft's catalog for Windows 10 or 11.
pub fn fetch_catalog(win11: bool, job: &Job) -> Result<Vec<EsdFile>> {
    job.stage(1, 1, "Getting the list of downloads from Microsoft");
    let dir = crate::app_data_dir().join("catalog");
    fs::create_dir_all(&dir).map_err(|e| EngineError::new("WIP-E113", e.to_string()))?;
    let name = if win11 { "products-11" } else { "products-10" };
    let cab = dir.join(format!("{name}.cab"));
    let _ = fs::remove_file(&cab);
    curl_download(if win11 { download::CATALOG_WIN11 } else { download::CATALOG_WIN10 }, &cab, job)?;
    let out_dir = dir.join(name);
    let _ = fs::remove_dir_all(&out_dir);
    fs::create_dir_all(&out_dir).map_err(|e| EngineError::new("WIP-E113", e.to_string()))?;
    let raw = fs::read(&cab).map_err(|e| EngineError::new("WIP-E114", format!("Couldn't read the catalog: {e}")))?;
    job.log(format!("Catalog download: {} bytes", raw.len()));
    let xml = if download::is_cab(&raw) {
        // Unpack everything: the XML inside isn't always named products.xml.
        let expand = system32().map(|s| s.join("expand.exe")).unwrap_or_else(|| PathBuf::from("expand.exe"));
        let args = vec![cab.display().to_string(), "-F:*".into(), out_dir.display().to_string()];
        expect_ok(run(&expand, &args, job)?, "Unpacking the catalog", "WIP-E114")?;
        let mut entries = Vec::new();
        let _ = crate::win::workflow::walk(&out_dir, Path::new(""), &mut entries);
        let found: Vec<PathBuf> = entries.into_iter().map(|(rel, _)| out_dir.join(rel)).collect();
        job.log(format!(
            "Catalog contains: {}",
            found
                .iter()
                .filter_map(|p| p.file_name())
                .map(|n| n.to_string_lossy().into_owned())
                .collect::<Vec<_>>()
                .join(", ")
        ));
        let mut xmls: Vec<String> = Vec::new();
        for p in &found {
            let Ok(b) = fs::read(p) else { continue };
            let text = download::decode_text(&b);
            let start: String = text.chars().take(160).map(|c| if c.is_control() { ' ' } else { c }).collect();
            job.log(format!("{}: {} bytes: {start}", p.display(), b.len()));
            if download::looks_like_catalog(&text) {
                xmls.push(text);
            }
        }
        if xmls.is_empty() {
            return Err(EngineError::new(
                "WIP-E114",
                "Microsoft's catalog file didn't contain a product list. Try again later; the Details log lists what it contained.",
            ));
        }
        xmls.swap_remove(0)
    } else {
        let text = download::decode_text(&raw);
        if !download::looks_like_catalog(&text) {
            let start: String = text.chars().take(120).filter(|c| !c.is_control()).collect();
            job.log(format!("Unexpected catalog content: {start}"));
            return Err(EngineError::new(
                "WIP-E114",
                "Microsoft's server didn't send a catalog (perhaps a network filter or proxy page). The Details log shows what arrived.",
            ));
        }
        text
    };
    download::parse_products(&xml).map_err(|e| EngineError::new("WIP-E115", e))
}

/// Downloads adksetup.exe from Microsoft, requires a valid Authenticode
/// signature from Microsoft Corporation, and installs Deployment Tools.
pub fn install_adk(job: &Job) -> Result<()> {
    job.stage(1, 3, "Downloading Microsoft's installer");
    let dir = crate::app_data_dir().join("adk");
    fs::create_dir_all(&dir).map_err(|e| EngineError::new("WIP-E113", e.to_string()))?;
    let exe = dir.join("adksetup.exe");
    let _ = fs::remove_file(&exe);
    curl_download(download::ADK_SETUP, &exe, job)?;
    job.stage(2, 3, "Checking it's really from Microsoft");
    let script = format!(
        "$s = Get-AuthenticodeSignature -LiteralPath {}; \"$($s.Status)|$($s.SignerCertificate.Subject)\"",
        wip_core::boottest::ps_lit(&exe.display().to_string())
    );
    let sig = ps(&script, job)?;
    job.log(format!("Signature: {}", sig.trim()));
    let (status, subject) = sig.trim().split_once('|').unwrap_or(("", ""));
    if status != "Valid" || !subject.contains("O=Microsoft Corporation") {
        let _ = fs::remove_file(&exe);
        return Err(EngineError::new(
            "WIP-E121",
            "The downloaded installer isn't validly signed by Microsoft, so WinISOPilot deleted it without running it.",
        ));
    }
    job.stage(3, 3, "Installing the Deployment Tools (several minutes)");
    job.indeterminate();
    let args: Vec<String> = download::ADK_SETUP_ARGS.iter().map(|s| s.to_string()).collect();
    let out = run(&exe, &args, job)?;
    // 3010 = success, restart required (not needed for oscdimg).
    if out.code != 0 && out.code != 3010 {
        return Err(EngineError::new(
            "WIP-E122",
            format!(
                "Microsoft's installer stopped with code {}. Its logs are in %TEMP%\\adk. You can also install it by hand from the link.",
                out.code
            ),
        ));
    }
    Ok(())
}

pub(crate) fn file_hash(path: &Path, algo: &str, job: &Job) -> Result<String> {
    let script = format!(
        "(Get-FileHash -Algorithm {algo} -LiteralPath {}).Hash",
        wip_core::boottest::ps_lit(&path.display().to_string())
    );
    Ok(ps(&script, job)?.trim().to_ascii_uppercase())
}

/// Downloads the ESD (resuming a partial file) and verifies its hash.
fn download_esd(f: &EsdFile, dir: &Path, job: &Job) -> Result<PathBuf> {
    let name = download::safe_file_name(&f.file_name)
        .ok_or_else(|| EngineError::new("WIP-E116", "The catalog entry has an unsafe file name."))?;
    let (algo, expected) = f.expected_hash().ok_or_else(|| {
        EngineError::new("WIP-E117", "Microsoft's catalog has no usable hash for this file, so it can't be verified.")
    })?;
    fs::create_dir_all(dir).map_err(|e| EngineError::new("WIP-E113", e.to_string()))?;
    let done = dir.join(&name);
    if done.is_file() {
        job.log("Already downloaded; checking it");
        if file_hash(&done, algo, job)? == expected {
            return Ok(done);
        }
        let mut bad = done.as_os_str().to_owned();
        bad.push(".mismatch");
        let _ = fs::rename(&done, &bad);
    }
    let part = dir.join(format!("{name}.part"));
    let url = download::https(&f.url);
    if let Err(e) = curl_download(&url, &part, job) {
        if e.is_cancelled() || url == f.url {
            return Err(e);
        }
        job.log("HTTPS download failed; retrying over HTTP (the file is still verified by its hash)");
        curl_download(&f.url, &part, job)?;
    }
    job.log(format!("Verifying {algo}"));
    job.indeterminate();
    let got = file_hash(&part, algo, job)?;
    if got != expected {
        let _ = fs::remove_file(&part);
        return Err(EngineError::new(
            "WIP-E118",
            format!("The download is damaged ({algo} doesn't match Microsoft's). It was deleted; try again."),
        ));
    }
    fs::rename(&part, &done).map_err(|e| EngineError::new("WIP-E113", e.to_string()))?;
    Ok(done)
}

fn export(dism: &Path, src: &Path, index: u32, dst: &Path, compress: &str, bootable: bool, job: &Job) -> Result<()> {
    let mut args = vec![
        "/Export-Image".to_string(),
        opt("SourceImageFile", src),
        format!("/SourceIndex:{index}"),
        opt("DestinationImageFile", dst),
        format!("/Compress:{compress}"),
    ];
    if bootable {
        args.push("/Bootable".into());
    }
    expect_ok(run_with_progress(dism, &args, job)?, "Exporting an image", "WIP-E119").map(|_| ())
}

/// Downloads the chosen Windows and writes a bootable ISO next to it.
pub fn download_iso(f: &EsdFile, dest_dir: &Path, iso_out: &Path, host: &HostInfo, job: &Job) -> Result<PathBuf> {
    if !host.is_admin {
        return Err(EngineError::new("WIP-E002", "Administrator rights are needed to build the ISO."));
    }
    let dism_dll = host.dism_api.clone().ok_or_else(|| EngineError::new("WIP-E012", "DISM wasn't found."))?;
    let dism_exe = host.dism_exe.clone().ok_or_else(|| EngineError::new("WIP-E013", "dism.exe wasn't found."))?;
    let oscdimg = host.oscdimg.clone().ok_or_else(|| {
        EngineError::new("WIP-E102", "Making an ISO needs oscdimg.exe from the Windows ADK Deployment Tools.")
    })?;
    if iso_out.exists() {
        return Err(EngineError::new("WIP-E070", format!("{} already exists.", iso_out.display())));
    }
    const STAGES: u32 = 3;
    job.stage(1, STAGES, format!("Downloading {} from Microsoft", wip_core::human_bytes(f.size)));
    let esd = download_esd(f, dest_dir, job)?;
    job.check()?;

    job.stage(2, STAGES, "Unpacking Windows Setup");
    let dism = super::dism::Dism::open(&dism_dll)?;
    let layout = download::classify(&dism.image_info(&esd)?).map_err(|e| EngineError::new("WIP-E120", e))?;
    let build_dir = dest_dir.join(format!(
        "{}.build",
        iso_out.file_stem().map(|s| s.to_string_lossy().to_string()).unwrap_or_else(|| "windows".into())
    ));
    let _ = fs::remove_dir_all(&build_dir);
    let media = build_dir.join("media");
    fs::create_dir_all(&media).map_err(|e| EngineError::new("WIP-E113", e.to_string()))?;
    let result = (|| -> Result<()> {
        let args = vec![
            "/Apply-Image".to_string(),
            opt("ImageFile", &esd),
            format!("/Index:{}", layout.media),
            opt("ApplyDir", &media),
        ];
        expect_ok(run_with_progress(&dism_exe, &args, job)?, "Unpacking setup files", "WIP-E119")?;
        job.check()?;
        let boot = media.join(r"sources\boot.wim");
        let _ = fs::remove_file(&boot);
        export(&dism_exe, &esd, layout.pe, &boot, "max", false, job)?;
        export(&dism_exe, &esd, layout.setup, &boot, "max", true, job)?;
        job.check()?;
        job.stage(3, STAGES, "Writing the Windows editions and the ISO");
        let install = media.join(r"sources\install.esd");
        let _ = fs::remove_file(&install);
        for (n, i) in layout.editions.iter().enumerate() {
            job.log(format!("Edition {}/{}", n + 1, layout.editions.len()));
            export(&dism_exe, &esd, *i, &install, "recovery", false, job)?;
            job.check()?;
        }
        let label = sanitize_label(&format!(
            "WIN{}_{}_{}",
            if f.build().unwrap_or(0) >= wip_core::WIN11_FIRST_BUILD { "11" } else { "10" },
            f.architecture,
            f.language_code
        ));
        make_iso(&oscdimg, &media, iso_out, &label, job)
    })();
    let _ = fs::remove_dir_all(&build_dir);
    result?;
    Ok(iso_out.to_path_buf())
}
