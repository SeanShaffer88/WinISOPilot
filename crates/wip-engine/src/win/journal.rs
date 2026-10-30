use std::path::PathBuf;
use std::sync::Mutex;
use wip_core::fsutil::{atomic_write, read_json_or_quarantine};
use wip_core::journal::{now_secs, Journal, MountRecord};

static LOCK: Mutex<()> = Mutex::new(());

fn path() -> PathBuf {
    crate::app_data_dir().join("journal.json")
}

pub fn load() -> Journal {
    read_json_or_quarantine(&path()).ok().flatten().unwrap_or_default()
}

fn save(j: &Journal) -> std::io::Result<()> {
    atomic_write(&path(), serde_json::to_string_pretty(j).expect("serializes").as_bytes())
}

/// Must succeed before mounting: no record, no mount.
pub fn record_mount(image_file: &str, index: u32, mount_dir: &str, workspace: &str) -> std::io::Result<()> {
    let _g = LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let mut j = load();
    j.add(MountRecord {
        image_file: image_file.into(),
        index,
        mount_dir: mount_dir.into(),
        workspace: workspace.into(),
        started: now_secs(),
        pid: std::process::id(),
    });
    save(&j)
}

pub fn clear_mount(mount_dir: &str) {
    let _g = LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let mut j = load();
    j.remove(mount_dir);
    let _ = save(&j);
}
