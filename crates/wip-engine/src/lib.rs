//! WinISOPilot servicing engine.
//!
//! All Windows-specific work lives in `win`. On other platforms every entry
//! point returns [`EngineError::unsupported`], so the UI can still be built
//! and exercised during development.

use serde::{Deserialize, Serialize};
use std::fmt;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::Sender;
use std::sync::Arc;
use wip_core::inventory::Inventory;
use wip_core::journal::MountRecord;
use wip_core::plan::CompiledPlan;
use wip_core::recipe::OutputSettings;
use wip_core::ImageSummary;

#[cfg(windows)]
mod win;

/// Stable error codes shown in the UI and logs.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EngineError {
    pub code: &'static str,
    pub message: String,
}

impl EngineError {
    pub fn new(code: &'static str, message: impl Into<String>) -> Self {
        EngineError { code, message: message.into() }
    }
    pub fn unsupported() -> Self {
        EngineError::new("WIP-E000", "Image servicing is only available on Windows 10 or 11.")
    }
    pub fn cancelled() -> Self {
        EngineError::new("WIP-E001", "Stopped at your request. Nothing has been saved to the image.")
    }
    pub fn is_cancelled(&self) -> bool {
        self.code == "WIP-E001"
    }
}

impl fmt::Display for EngineError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "[{}] {}", self.code, self.message)
    }
}

impl std::error::Error for EngineError {}

pub type Result<T> = std::result::Result<T, EngineError>;

/// Progress events sent from a running job to the UI.
#[derive(Debug, Clone)]
pub enum Event {
    /// A new stage started: (stage number from 1, total stages, label).
    Stage(u32, u32, String),
    /// Progress within the current stage, 0.0–1.0. `None` = indeterminate.
    Progress(Option<f32>),
    Log(String),
}

/// Handle given to a running job: reports progress, checks for cancel.
#[derive(Clone)]
pub struct Job {
    tx: Sender<Event>,
    cancel: Arc<AtomicBool>,
}

impl Job {
    pub fn new(tx: Sender<Event>, cancel: Arc<AtomicBool>) -> Job {
        Job { tx, cancel }
    }
    pub fn stage(&self, n: u32, total: u32, label: impl Into<String>) {
        let label = label.into();
        self.log(format!("== {label}"));
        let _ = self.tx.send(Event::Stage(n, total, label));
        let _ = self.tx.send(Event::Progress(None));
    }
    pub fn progress(&self, p: f32) {
        let _ = self.tx.send(Event::Progress(Some(p.clamp(0.0, 1.0))));
    }
    pub fn indeterminate(&self) {
        let _ = self.tx.send(Event::Progress(None));
    }
    pub fn log(&self, line: impl Into<String>) {
        let _ = self.tx.send(Event::Log(line.into()));
    }
    pub fn cancelled(&self) -> bool {
        self.cancel.load(Ordering::Relaxed)
    }
    pub fn check(&self) -> Result<()> {
        if self.cancelled() {
            Err(EngineError::cancelled())
        } else {
            Ok(())
        }
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct HostInfo {
    pub is_admin: bool,
    pub os_build: u32,
    pub dism_api: Option<PathBuf>,
    pub dism_exe: Option<PathBuf>,
    /// DISM version as "major.minor.build.revision".
    pub dism_version: String,
    pub dism_build: u32,
    pub oscdimg: Option<PathBuf>,
    pub notes: Vec<String>,
}

/// What an ISO contains, read without changing it.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SourceInfo {
    pub iso: PathBuf,
    pub iso_bytes: u64,
    /// `sources\install.wim` or `sources\install.esd`.
    pub image_rel: String,
    pub images: Vec<ImageSummary>,
}

#[derive(Debug, Clone)]
pub struct PrepareRequest {
    pub iso: PathBuf,
    pub image_rel: String,
    /// Parent folder; a new uniquely named workspace is created inside.
    pub workspace_parent: PathBuf,
    /// Edition to customize (index in the source image).
    pub index: u32,
    /// All indexes in the source (used when keeping every edition).
    pub all_indexes: Vec<u32>,
    pub single_edition: bool,
}

/// A prepared workspace: media copied, edition exported and mounted.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Workspace {
    pub root: PathBuf,
    pub media: PathBuf,
    pub work_wim: PathBuf,
    /// Index of the customized edition inside `work_wim`.
    pub work_index: u32,
    pub mount_dir: PathBuf,
    pub boot_mount_dir: PathBuf,
    pub scratch: PathBuf,
    pub log_file: PathBuf,
    pub edition: ImageSummary,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BuildReport {
    pub iso: Option<PathBuf>,
    pub media_folder: PathBuf,
    pub iso_bytes: Option<u64>,
    pub applied: usize,
    pub warnings: Vec<String>,
    pub log_file: PathBuf,
}

/// Rough free-space requirement for a workspace: two copies of the ISO
/// contents, one expanded edition and headroom for DISM scratch.
pub fn required_space(iso_bytes: u64, edition_bytes: u64) -> u64 {
    iso_bytes.saturating_mul(2) + edition_bytes + 10 * 1024 * 1024 * 1024
}

pub fn probe_host(dism_folder: Option<&Path>) -> HostInfo {
    #[cfg(windows)]
    {
        win::host::probe(dism_folder)
    }
    #[cfg(not(windows))]
    {
        let _ = dism_folder;
        HostInfo { notes: vec![EngineError::unsupported().message], ..Default::default() }
    }
}

pub fn free_space(path: &Path) -> Option<u64> {
    #[cfg(windows)]
    {
        win::host::free_space(path)
    }
    #[cfg(not(windows))]
    {
        let _ = path;
        None
    }
}

/// Reads the editions inside an ISO. The ISO is attached read-only and
/// detached again before returning.
pub fn inspect_iso(iso: &Path, host: &HostInfo, job: &Job) -> Result<SourceInfo> {
    #[cfg(windows)]
    {
        win::workflow::inspect_iso(iso, host, job)
    }
    #[cfg(not(windows))]
    {
        let _ = (iso, host, job);
        Err(EngineError::unsupported())
    }
}

/// Copies the media, exports the edition(s) and mounts the edition for
/// editing, then reads what it contains.
pub fn prepare(req: &PrepareRequest, host: &HostInfo, job: &Job) -> Result<(Workspace, Inventory)> {
    #[cfg(windows)]
    {
        win::workflow::prepare(req, host, job)
    }
    #[cfg(not(windows))]
    {
        let _ = (req, host, job);
        Err(EngineError::unsupported())
    }
}

/// Applies the plan, saves the image and writes the new ISO.
pub fn build(
    ws: &Workspace,
    plan: &CompiledPlan,
    output: &OutputSettings,
    iso_out: &Path,
    host: &HostInfo,
    job: &Job,
) -> Result<BuildReport> {
    #[cfg(windows)]
    {
        win::workflow::build(ws, plan, output, iso_out, host, job)
    }
    #[cfg(not(windows))]
    {
        let _ = (ws, plan, output, iso_out, host, job);
        Err(EngineError::unsupported())
    }
}

/// Unmounts the workspace's image without saving any changes.
pub fn discard(ws: &Workspace, host: &HostInfo, job: &Job) -> Result<()> {
    #[cfg(windows)]
    {
        win::workflow::discard_mount(&ws.mount_dir, host, job)
    }
    #[cfg(not(windows))]
    {
        let _ = (ws, host, job);
        Err(EngineError::unsupported())
    }
}

/// Mounts recorded in the journal by an earlier session that never finished.
pub fn leftover_mounts() -> Vec<MountRecord> {
    #[cfg(windows)]
    {
        win::journal::load().mounts.into_iter().filter(|m| m.pid != std::process::id()).collect()
    }
    #[cfg(not(windows))]
    {
        Vec::new()
    }
}

/// Discards a leftover mount from the journal. Workspace files are kept.
pub fn clean_leftover(rec: &MountRecord, host: &HostInfo, job: &Job) -> Result<()> {
    #[cfg(windows)]
    {
        win::workflow::discard_mount(Path::new(&rec.mount_dir), host, job)
    }
    #[cfg(not(windows))]
    {
        let _ = (rec, host, job);
        Err(EngineError::unsupported())
    }
}

/// Removes a leftover record without touching the image (for records whose
/// mount no longer exists, e.g. after the workspace folder was deleted).
pub fn forget_leftover(rec: &MountRecord) {
    #[cfg(windows)]
    {
        win::journal::clear_mount(&rec.mount_dir);
    }
    #[cfg(not(windows))]
    {
        let _ = rec;
    }
}

pub fn app_data_dir() -> PathBuf {
    let base = std::env::var_os("LOCALAPPDATA")
        .or_else(|| std::env::var_os("XDG_DATA_HOME"))
        .map(PathBuf::from)
        .unwrap_or_else(std::env::temp_dir);
    base.join(wip_core::APP_NAME)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn space_estimate_has_headroom() {
        let gb = 1024u64 * 1024 * 1024;
        assert_eq!(required_space(5 * gb, 15 * gb), 35 * gb);
    }

    #[test]
    fn job_cancel() {
        let (tx, rx) = std::sync::mpsc::channel();
        let flag = Arc::new(AtomicBool::new(false));
        let job = Job::new(tx, flag.clone());
        assert!(job.check().is_ok());
        job.progress(2.0);
        assert!(matches!(rx.recv().unwrap(), Event::Progress(Some(p)) if p == 1.0));
        flag.store(true, Ordering::Relaxed);
        assert!(job.check().unwrap_err().is_cancelled());
    }
}
