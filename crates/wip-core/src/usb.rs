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
    /// MBR + FAT32: boots on BIOS and UEFI PCs (Secure Boot included).
    #[default]
    Mbr,
    /// GPT + FAT32: UEFI PCs only.
    Gpt,
    /// GPT + NTFS: UEFI only, and only on firmware that can read NTFS.
    /// No 4 GB file limit, so install.wim is copied whole.
    GptNtfs,
}

impl Scheme {
    pub const ALL: [Scheme; 3] = [Scheme::Mbr, Scheme::Gpt, Scheme::GptNtfs];

    pub fn label(self) -> &'static str {
        match self {
            Scheme::Mbr => "MBR + FAT32 — BIOS and UEFI (recommended)",
            Scheme::Gpt => "GPT + FAT32 — UEFI only",
            Scheme::GptNtfs => "GPT + NTFS — UEFI only, some PCs can't boot it",
        }
    }

    pub fn hint(self) -> &'static str {
        match self {
            Scheme::Mbr => "Works on old and new PCs. A Windows image over 4 GB is split automatically.",
            Scheme::Gpt => "For UEFI PCs without legacy (CSM) support. Large images are split.",
            Scheme::GptNtfs => {
                "No file splitting, but the PC's UEFI firmware must be able to read NTFS; many can't. Not for BIOS PCs."
            }
        }
    }

    /// FAT32 (with its 4 GB file and 32 GB format limits) or NTFS.
    pub fn is_fat32(self) -> bool {
        !matches!(self, Scheme::GptNtfs)
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

/// Volume label for the chosen file system (NTFS allows 32 characters).
pub fn sanitize_label(s: &str, scheme: Scheme) -> String {
    if scheme.is_fat32() {
        return sanitize_fat_label(s);
    }
    let out: String =
        s.chars().filter(|c| c.is_ascii_alphanumeric() || matches!(c, ' ' | '_' | '-')).take(32).collect();
    let out = out.trim().to_string();
    if out.is_empty() {
        "WINUSB".into()
    } else {
        out
    }
}

/// Smallest leftover worth turning into a second (data) partition.
pub const MIN_DATA_PARTITION_MB: u64 = 1024;

/// Size of the setup partition in MB, or `None` for the whole disk.
/// diskpart can't format FAT32 above 32 GB, so FAT32 is capped there.
pub fn partition_size_mb(disk_bytes: u64, scheme: Scheme) -> Option<u64> {
    let mb = disk_bytes / (1024 * 1024);
    (scheme.is_fat32() && mb > FAT32_MAX_PARTITION_MB).then_some(FAT32_MAX_PARTITION_MB)
}

/// Usable bytes of the setup partition that will be created.
pub fn partition_bytes(disk_bytes: u64, scheme: Scheme) -> u64 {
    partition_size_mb(disk_bytes, scheme).map_or(disk_bytes, |mb| mb * 1024 * 1024)
}

/// Space (MB) left after the capped FAT32 setup partition, if it is worth a
/// second partition.
pub fn leftover_mb(disk_bytes: u64, scheme: Scheme) -> Option<u64> {
    let cap = partition_size_mb(disk_bytes, scheme)?;
    let left = (disk_bytes / (1024 * 1024)).saturating_sub(cap + 8);
    (left >= MIN_DATA_PARTITION_MB).then_some(left)
}

/// The diskpart script that erases and formats the disk. With
/// `data_partition`, space beyond the 32 GB FAT32 cap becomes a second
/// exFAT partition labelled DATA (Windows 10 1703+ mounts both).
pub fn diskpart_script(disk: &Disk, scheme: Scheme, label: &str, data_partition: bool) -> String {
    let mut s = String::new();
    s.push_str(&format!("select disk {}\r\n", disk.number));
    s.push_str("clean\r\n");
    s.push_str(if scheme == Scheme::Mbr { "convert mbr\r\n" } else { "convert gpt\r\n" });
    match partition_size_mb(disk.size, scheme) {
        Some(mb) => s.push_str(&format!("create partition primary size={mb}\r\n")),
        None => s.push_str("create partition primary\r\n"),
    }
    let fs = if scheme.is_fat32() { "fat32" } else { "ntfs" };
    s.push_str(&format!("format quick fs={fs} label=\"{}\"\r\n", sanitize_label(label, scheme)));
    if scheme == Scheme::Mbr {
        s.push_str("active\r\n");
    }
    s.push_str("assign\r\n");
    if data_partition && leftover_mb(disk.size, scheme).is_some() {
        s.push_str("create partition primary\r\n");
        s.push_str("format quick fs=exfat label=\"DATA\"\r\n");
        s.push_str("assign\r\n");
    }
    s.push_str("exit\r\n");
    s
}

/// diskpart script that turns a stick back into one ordinary exFAT drive
/// (MBR, so every PC and camera reads it).
pub fn restore_script(disk: &Disk, label: &str) -> String {
    let label: String = sanitize_fat_label(label).chars().take(11).collect();
    let label = if label.is_empty() { "USB".to_string() } else { label };
    format!(
        "select disk {}\r\nclean\r\nconvert mbr\r\ncreate partition primary\r\nformat quick fs=exfat label=\"{label}\"\r\nassign\r\nexit\r\n",
        disk.number
    )
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
pub fn plan_files(files: &[(String, u64)], partition_bytes: u64, fat32: bool) -> Result<FilePlan, String> {
    let mut out = Vec::new();
    let mut total = 0u64;
    for (path, size) in files {
        let norm = path.replace('/', "\\");
        let place = if fat32 && *size > FAT32_MAX_FILE {
            if norm.eq_ignore_ascii_case(r"sources\install.wim") {
                Placement::SplitWim
            } else {
                return Err(format!(
                    "{norm} is {} — too large for a FAT32 USB drive and it can't be split. \
                     Rebuild the ISO with Standard compression, keep only one edition, or use GPT + NTFS.",
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
        let s = diskpart_script(&disk(2, 16 * GB, "USB"), Scheme::Mbr, "my win 11!", true);
        assert!(s.starts_with("select disk 2\r\nclean\r\nconvert mbr\r\n"));
        assert!(s.contains("create partition primary\r\n"));
        assert!(s.contains("label=\"MYWIN11\""));
        assert!(s.contains("active\r\n"));
        assert!(!s.contains("exfat"), "16 GB leaves nothing for a data partition");
        let g = diskpart_script(&disk(5, 128 * GB, "USB"), Scheme::Gpt, "x", false);
        assert!(g.contains("convert gpt"));
        assert!(g.contains("size=32768"));
        assert!(!g.contains("active"));
        assert!(!g.contains("exfat"));
        let d = diskpart_script(&disk(5, 128 * GB, "USB"), Scheme::Mbr, "x", true);
        let first = d.find("fs=fat32").unwrap();
        let active = d.find("active").unwrap();
        let data = d.find("fs=exfat label=\"DATA\"").unwrap();
        assert!(first < active && active < data, "active must apply to the setup partition");
        let n = diskpart_script(&disk(5, 128 * GB, "USB"), Scheme::GptNtfs, "Windows 11 Pro", true);
        assert!(n.contains("fs=ntfs label=\"Windows 11 Pro\""));
        assert!(!n.contains("size="), "NTFS uses the whole disk");
        assert!(!n.contains("exfat"));
    }

    #[test]
    fn restore_script_is_one_exfat_partition() {
        let s = restore_script(&disk(3, 32 * 1_000_000_000, "USB"), "my stick!");
        assert!(s.starts_with("select disk 3\r\nclean\r\nconvert mbr\r\n"));
        assert!(s.contains("fs=exfat"));
        assert_eq!(s.matches("create partition").count(), 1);
        assert!(!s.contains("active"));
    }

    #[test]
    fn partition_sizes() {
        assert_eq!(partition_bytes(16 * GB, Scheme::Mbr), 16 * GB);
        assert_eq!(partition_bytes(128 * GB, Scheme::Mbr), 32 * 1024 * 1024 * 1024);
        assert_eq!(partition_bytes(128 * GB, Scheme::GptNtfs), 128 * GB);
        assert!(leftover_mb(128 * GB, Scheme::Mbr).unwrap() > 80_000);
        assert!(leftover_mb(33 * 1024 * 1024 * 1024, Scheme::Mbr).is_none(), "under 1 GB left over");
        assert!(leftover_mb(128 * GB, Scheme::GptNtfs).is_none());
    }

    #[test]
    fn labels() {
        assert_eq!(sanitize_fat_label("Windows 11 Pro"), "WINDOWS11PR");
        assert_eq!(sanitize_fat_label("!!"), "WINUSB");
        assert_eq!(sanitize_label("Windows 11 Pro (x64)", Scheme::GptNtfs), "Windows 11 Pro x64");
        assert_eq!(sanitize_label("Windows 11 Pro", Scheme::Mbr), "WINDOWS11PR");
    }

    #[test]
    fn file_plan_splits_only_install_wim() {
        let files = vec![
            ("sources\\install.wim".to_string(), 5_500_000_000),
            ("sources\\boot.wim".to_string(), 700_000_000),
            ("setup.exe".to_string(), 100_000),
        ];
        let p = plan_files(&files, 16 * GB, true).unwrap();
        assert_eq!(p.files[0].2, Placement::SplitWim);
        assert_eq!(p.files[1].2, Placement::Copy);
        let bad = vec![("sources\\install.esd".to_string(), 5_000_000_000)];
        assert!(plan_files(&bad, 16 * GB, true).is_err());
        assert!(plan_files(&files, 5 * GB, true).is_err());
        // NTFS: nothing is split and large files are fine.
        let n = plan_files(&files, 16 * GB, false).unwrap();
        assert!(n.files.iter().all(|f| f.2 == Placement::Copy));
        assert!(plan_files(&bad, 16 * GB, false).is_ok());
    }
}
