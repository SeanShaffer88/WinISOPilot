//! Small filesystem helpers shared by the app and engine.

use std::fs;
use std::io::{self, Write};
use std::path::Path;

/// Writes `bytes` to `path` atomically: temp file in the same folder,
/// flushed to disk, then renamed over the target.
pub fn atomic_write(path: &Path, bytes: &[u8]) -> io::Result<()> {
    let dir = path.parent().filter(|p| !p.as_os_str().is_empty()).unwrap_or(Path::new("."));
    fs::create_dir_all(dir)?;
    let name = path.file_name().ok_or_else(|| io::Error::other("path has no file name"))?;
    let tmp = dir.join(format!(".{}.tmp-{}", name.to_string_lossy(), std::process::id()));
    {
        let mut f = fs::File::create(&tmp)?;
        f.write_all(bytes)?;
        f.sync_all()?;
    }
    fs::rename(&tmp, path).inspect_err(|_| {
        let _ = fs::remove_file(&tmp);
    })
}

/// Reads a JSON file. A file that can't be parsed is moved aside to
/// `<name>.corrupt` (never deleted) and `Ok(None)` is returned.
pub fn read_json_or_quarantine<T: serde::de::DeserializeOwned>(path: &Path) -> io::Result<Option<T>> {
    let bytes = match fs::read(path) {
        Ok(b) => b,
        Err(e) if e.kind() == io::ErrorKind::NotFound => return Ok(None),
        Err(e) => return Err(e),
    };
    match serde_json::from_slice(&bytes) {
        Ok(v) => Ok(Some(v)),
        Err(_) => {
            let mut bad = path.as_os_str().to_owned();
            bad.push(".corrupt");
            let _ = fs::rename(path, &bad);
            Ok(None)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn atomic_roundtrip_and_quarantine() {
        let dir = std::env::temp_dir().join(format!("wip-fsutil-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        let p = dir.join("a.json");
        atomic_write(&p, br#"{"x":1}"#).unwrap();
        let v: Option<serde_json::Value> = read_json_or_quarantine(&p).unwrap();
        assert_eq!(v.unwrap()["x"], 1);
        fs::write(&p, b"{broken").unwrap();
        let v: Option<serde_json::Value> = read_json_or_quarantine(&p).unwrap();
        assert!(v.is_none());
        assert!(dir.join("a.json.corrupt").exists());
        let missing: Option<serde_json::Value> = read_json_or_quarantine(&dir.join("nope.json")).unwrap();
        assert!(missing.is_none());
        let _ = fs::remove_dir_all(&dir);
    }
}
