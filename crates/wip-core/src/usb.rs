//! Bootable USB planning: which disks may be written, the diskpart script,
//! and how the media files are laid out on FAT32. No Windows APIs here.

use serde::{Deserialize, Serialize};

/// Largest single file FAT32 can hold.
pub const FAT32_MAX_FILE: u64 = 4 * 1024 * 1024 * 1024 - 1;
/// diskpart won't format FAT32 volumes larger than 32 GB.
pub const FAT32_MAX_PARTITION_MB: u64 = 32 * 1024;
/// Split size for install.wim, in MB (stays well under the FAT32 limit).
pub const SWM_SPLIT_MB: u32 = 3800;
/// Smallest drive WinISOPilot will write to.
pub const MIN_DISK_BYTES: u64 = 7 * 1000 * 1000 * 1000;

/// A disk as reported by `Get-Disk` (see the engine's PowerShell query).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct Disk {
    pub number: u32,
    #[serde(default)]
    pub friendly_name: String,
    #[serde(default)]
    pub serial_number: Option<String>,
    #[serde(default)]
    pub unique_id: Option<String>,
    pub size: u64,
    #[serde(default)]
    pub bus_type: String,
    #[serde(default)]
    pub is_boot: bool,
    #[serde(default)]
    pub is_system: bool,
    #[serde(default)]
    pub is_read_only: bool,
    #[serde(default)]
    pub is_offline: bool,
    /// Drive letters of volumes on the disk.
    #[serde(default)]
    pub letters: Vec<String>,
}

impl Disk {
    pub fn label(&self) -> String {
        let letters = if self.letters.is_empty() {
            String::new()
        } else {
            format!(" ({})", self.letters.iter().map(|l| format!("{l}:")).collect::<Vec<_>>().join(", "))
        };
        let name = if self.friendly_name.trim().is_empty() { "Unnamed drive" } else { self.friendly_name.trim() };
        format!("Disk {} · {} · {}{letters}", self.number, name, crate::human_bytes(self.size))
    }

    /// True when two snapshots describe the same physical disk.
    pub fn same_disk(&self, other: &Disk) -> bool {
        self.number == other.number
            && self.size == other.size
            && self.friendly_name == other.friendly_name
            && self.unique_id == other.unique_id
            && self.serial_number == other.serial_number
    }
}

/// Parses the engine's PowerShell JSON (always an array).
pub fn parse_disks(json: &str) -> Result<Vec<Disk>, String> {
    let text = json.trim();
    if text.is_empty() {
        return Ok(Vec::new());
    }
    serde_json::from_str::<Vec<Disk>>(text).map_err(|e| format!("Couldn't read the drive list: {e}"))
}

/// Why a disk can't be used, or `None` when it can.
/// Drives above this size are almost always external hard drives or SSDs
/// (backups), not USB sticks.
pub const LARGE_DISK_BYTES: u64 = 300 * 1000 * 1000 * 1000;

/// A USB disk large enough that it's probably someone's backup drive.
pub fn likely_backup_drive(d: &Disk) -> bool {
    d.size > LARGE_DISK_BYTES
}

/// Like `ineligible`, and in `strict` (beginner) mode also refuses likely
/// backup drives.
pub fn ineligible_for(d: &Disk, protected_letters: &[char], strict: bool) -> Option<String> {
    ineligible(d, protected_letters).or_else(|| {
        (strict && likely_backup_drive(d)).then(|| {
            format!(
                "{} is too big to be a USB stick; it looks like an external or backup drive. (Advanced layout can use it.)",
                crate::human_bytes(d.size)
            )
        })
    })
}

pub fn ineligible(d: &Disk, protected_letters: &[char]) -> Option<String> {
    if d.is_system || d.is_boot {
        return Some("This is the disk Windows is running from.".into());
    }
    if !d.bus_type.eq_ignore_ascii_case("USB") {
        return Some(format!("Not a USB drive ({}).", if d.bus_type.is_empty() { "unknown" } else { &d.bus_type }));
    }
    if d.is_read_only {
        return Some("The drive is write-protected.".into());
    }
    if d.is_offline {
        return Some("The drive is offline.".into());
    }
    if d.size < MIN_DISK_BYTES {
        return Some(format!("Too small: at least {} is needed.", crate::human_bytes(MIN_DISK_BYTES)));
    }
    for l in &d.letters {
        if let Some(c) = l.chars().next() {
            if protected_letters.iter().any(|p| p.eq_ignore_ascii_case(&c)) {
                return Some(format!("Drive {c}: holds the source ISO or the working folder."));
            }
        }
    }
    None
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum Scheme {
    /// MBR + FAT32: boots on BIOS and UEFI PCs.
    #[default]
    Mbr,
    /// GPT + FAT32: UEFI PCs only.
    Gpt,
}

impl Scheme {
    pub fn label(self) -> &'static str {
        match self {
            Scheme::Mbr => "MBR — BIOS and UEFI (recommended)",
            Scheme::Gpt => "GPT — UEFI only",
        }
    }
}

/// FAT32 volume label: up to 11 characters, A–Z, 0–9, `_` and `-`.
pub fn sanitize_fat_label(s: &str) -> String {
    let out: String = s
        .chars()
        .map(|c| c.to_ascii_uppercase())
        .filter(|c| c.is_ascii_alphanumeric() || *c == '_' || *c == '-')
        .take(11)
        .collect();
    if out.is_empty() {
        "WINUSB".into()
    } else {
        out
    }
}

/// Size of the partition to create in MB, or `None` for the whole disk.
pub fn partition_size_mb(disk_bytes: u64) -> Option<u64> {
    let mb = disk_bytes / (1024 * 1024);
    (mb > FAT32_MAX_PARTITION_MB).then_some(FAT32_MAX_PARTITION_MB)
}

/// Usable bytes of the partition that will be created.
pub fn partition_bytes(disk_bytes: u64) -> u64 {
    partition_size_mb(disk_bytes).map_or(disk_bytes, |mb| mb * 1024 * 1024)
}

/// The diskpart script that erases and formats the disk.
pub fn diskpart_script(disk: &Disk, scheme: Scheme, label: &str) -> String {
    let mut s = String::new();
    s.push_str(&format!("select disk {}\r\n", disk.number));
    s.push_str("clean\r\n");
    s.push_str(match scheme {
        Scheme::Mbr => "convert mbr\r\n",
        Scheme::Gpt => "convert gpt\r\n",
    });
    match partition_size_mb(disk.size) {
        Some(mb) => s.push_str(&format!("create partition primary size={mb}\r\n")),
        None => s.push_str("create partition primary\r\n"),
    }
    s.push_str(&format!("format quick fs=fat32 label=\"{}\"\r\n", sanitize_fat_label(label)));
    if scheme == Scheme::Mbr {
        s.push_str("active\r\n");
    }
    s.push_str("assign\r\n");
    s.push_str("exit\r\n");
    s
}

/// One media file and how it gets onto the stick.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Placement {
    Copy,
    /// install.wim over 4 GB: split into install.swm parts by DISM.
    SplitWim,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FilePlan {
    pub files: Vec<(String, u64, Placement)>,
    pub total_bytes: u64,
}

/// Decides how each file is written. Fails if a file is too large for
/// FAT32 and can't be split, or the media won't fit.
pub fn plan_files(files: &[(String, u64)], partition_bytes: u64) -> Result<FilePlan, String> {
    let mut out = Vec::new();
    let mut total = 0u64;
    for (path, size) in files {
        let norm = path.replace('/', "\\");
        let place = if *size > FAT32_MAX_FILE {
            if norm.eq_ignore_ascii_case(r"sources\install.wim") {
                Placement::SplitWim
            } else {
                return Err(format!(
                    "{norm} is {} — too large for a FAT32 USB drive and it can't be split. \
                     Rebuild the ISO with Standard compression, or keep only one edition.",
                    crate::human_bytes(*size)
                ));
            }
        } else {
            Placement::Copy
        };
        total += size;
        out.push((norm, *size, place));
    }
    // Split parts and FAT32 overhead: keep 3% headroom.
    let needed = total + total / 33;
    if needed > partition_bytes {
        return Err(format!(
            "The files need about {} but the USB partition holds {}.",
            crate::human_bytes(needed),
            crate::human_bytes(partition_bytes)
        ));
    }
    Ok(FilePlan { files: out, total_bytes: total })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn disk(n: u32, size: u64, bus: &str) -> Disk {
        Disk {
            number: n,
            friendly_name: "SanDisk Ultra".into(),
            serial_number: Some("ABC".into()),
            unique_id: Some("USBSTOR\\X".into()),
            size,
            bus_type: bus.into(),
            is_boot: false,
            is_system: false,
            is_read_only: false,
            is_offline: false,
            letters: vec!["E".into()],
        }
    }

    const GB: u64 = 1_000_000_000;

    #[test]
    fn parse_powershell_json() {
        let json = r#"[{"Number":2,"FriendlyName":"SanDisk Ultra","SerialNumber":"4C53","UniqueId":"USBSTOR\\DISK","Size":61530439680,"BusType":"USB","IsBoot":false,"IsSystem":false,"IsReadOnly":false,"IsOffline":false,"Letters":["E"]},
                       {"Number":0,"FriendlyName":"NVMe","SerialNumber":null,"UniqueId":null,"Size":1000204886016,"BusType":"NVMe","IsBoot":true,"IsSystem":true,"IsReadOnly":false,"IsOffline":false,"Letters":["C"]}]"#;
        let d = parse_disks(json).unwrap();
        assert_eq!(d.len(), 2);
        assert_eq!(d[0].letters, vec!["E".to_string()]);
        assert!(ineligible(&d[0], &[]).is_none());
        assert!(ineligible(&d[1], &[]).is_some());
        assert!(parse_disks("").unwrap().is_empty());
        assert!(parse_disks("{bad").is_err());
    }

    #[test]
    fn eligibility_rules() {
        assert!(ineligible(&disk(1, 16 * GB, "USB"), &[]).is_none());
        assert!(ineligible(&disk(1, 16 * GB, "SATA"), &[]).is_some());
        assert!(ineligible(&disk(1, 4 * GB, "USB"), &[]).is_some());
        assert!(ineligible(&disk(1, 16 * GB, "USB"), &['e']).unwrap().contains("source ISO"));
        let mut sys = disk(1, 16 * GB, "USB");
        sys.is_system = true;
        assert!(ineligible(&sys, &[]).is_some());
        let mut ro = disk(1, 16 * GB, "USB");
        ro.is_read_only = true;
        assert!(ineligible(&ro, &[]).is_some());
        let big = disk(1, 1000 * GB, "USB");
        assert!(likely_backup_drive(&big));
        assert!(ineligible_for(&big, &[], false).is_none());
        assert!(ineligible_for(&big, &[], true).unwrap().contains("backup"));
        assert!(ineligible_for(&disk(1, 64 * GB, "USB"), &[], true).is_none());
    }

    #[test]
    fn identity_check() {
        let a = disk(2, 16 * GB, "USB");
        let mut b = a.clone();
        assert!(a.same_disk(&b));
        b.size += 1;
        assert!(!a.same_disk(&b));
        let mut c = a.clone();
        c.number = 3;
        assert!(!a.same_disk(&c));
    }

    #[test]
    fn script_mbr_small_and_gpt_large() {
        let s = diskpart_script(&disk(2, 16 * GB, "USB"), Scheme::Mbr, "my win 11!");
        assert!(s.starts_with("select disk 2\r\nclean\r\nconvert mbr\r\n"));
        assert!(s.contains("create partition primary\r\n"));
        assert!(s.contains("label=\"MYWIN11\""));
        assert!(s.contains("active\r\n"));
        let g = diskpart_script(&disk(5, 128 * GB, "USB"), Scheme::Gpt, "x");
        assert!(g.contains("convert gpt"));
        assert!(g.contains("size=32768"));
        assert!(!g.contains("active"));
    }

    #[test]
    fn labels() {
        assert_eq!(sanitize_fat_label("Windows 11 Pro"), "WINDOWS11PR");
        assert_eq!(sanitize_fat_label("!!"), "WINUSB");
    }

    #[test]
    fn file_plan_splits_only_install_wim() {
        let files = vec![
            ("sources\\install.wim".to_string(), 5_500_000_000),
            ("sources\\boot.wim".to_string(), 700_000_000),
            ("setup.exe".to_string(), 100_000),
        ];
        let p = plan_files(&files, 16 * GB).unwrap();
        assert_eq!(p.files[0].2, Placement::SplitWim);
        assert_eq!(p.files[1].2, Placement::Copy);
        let bad = vec![("sources\\install.esd".to_string(), 5_000_000_000)];
        assert!(plan_files(&bad, 16 * GB).is_err());
        assert!(plan_files(&files, 5 * GB).is_err());
    }
}
