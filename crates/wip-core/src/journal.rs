//! Operation journal: records every image WinISOPilot mounts so that an
//! interrupted session (crash, power loss) can be found and cleaned up on
//! the next launch. Written before mounting, cleared after unmounting.

use serde::{Deserialize, Serialize};

pub const JOURNAL_SCHEMA: u32 = 1;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MountRecord {
    /// The WIM file that was mounted.
    pub image_file: String,
    pub index: u32,
    pub mount_dir: String,
    /// Workspace folder that owns this mount.
    pub workspace: String,
    /// Unix seconds when the record was written.
    pub started: u64,
    /// Process ID that created the mount.
    pub pid: u32,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Journal {
    pub schema: u32,
    pub mounts: Vec<MountRecord>,
}

impl Journal {
    pub fn add(&mut self, rec: MountRecord) {
        self.mounts.retain(|m| !m.mount_dir.eq_ignore_ascii_case(&rec.mount_dir));
        self.mounts.push(rec);
        self.schema = JOURNAL_SCHEMA;
    }

    pub fn remove(&mut self, mount_dir: &str) {
        self.mounts.retain(|m| !m.mount_dir.eq_ignore_ascii_case(mount_dir));
    }
}

pub fn now_secs() -> u64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0)
}
