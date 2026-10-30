//! Running Windows tools (dism.exe, reg.exe, powershell.exe, oscdimg.exe)
//! without console windows, with progress parsing and cancellation.

use crate::{EngineError, Job, Result};
use std::io::Read;
use std::os::windows::process::CommandExt;
use std::path::Path;
use std::process::{Command, Stdio};

const CREATE_NO_WINDOW: u32 = 0x0800_0000;

pub struct Output {
    pub code: i32,
    pub text: String,
}

/// Arguments starting with `RAW:` are passed verbatim (needed for tools
/// like dism.exe and oscdimg.exe that expect `/Opt:"path"` quoting).
fn command(exe: &Path, args: &[String]) -> Command {
    let mut c = Command::new(exe);
    for a in args {
        match a.strip_prefix("RAW:") {
            Some(raw) => c.raw_arg(raw),
            None => c.arg(a),
        };
    }
    c.creation_flags(CREATE_NO_WINDOW).stdin(Stdio::null());
    c
}

/// `/Name:"value"` as a raw argument.
pub fn opt(name: &str, value: &std::path::Path) -> String {
    format!("RAW:/{name}:\"{}\"", value.display())
}

/// A quoted raw path argument.
pub fn quoted(value: &std::path::Path) -> String {
    format!("RAW:\"{}\"", value.display())
}

fn shown(args: &[String]) -> String {
    args.iter().map(|a| a.strip_prefix("RAW:").unwrap_or(a)).collect::<Vec<_>>().join(" ")
}

/// Runs a tool to completion and captures its output.
pub fn run(exe: &Path, args: &[String], job: &Job) -> Result<Output> {
    job.log(format!("> {} {}", exe.display(), shown(args)));
    let out = command(exe, args)
        .output()
        .map_err(|e| EngineError::new("WIP-E020", format!("Couldn't start {}: {e}", exe.display())))?;
    let mut text = String::from_utf8_lossy(&out.stdout).into_owned();
    text.push_str(&String::from_utf8_lossy(&out.stderr));
    Ok(Output { code: out.status.code().unwrap_or(-1), text })
}

/// Runs a tool, forwarding "NN.N%" progress from its output. The tool is
/// not killed on cancel (DISM must finish cleanly); cancellation is honored
/// after it exits.
pub fn run_with_progress(exe: &Path, args: &[String], job: &Job) -> Result<Output> {
    job.log(format!("> {} {}", exe.display(), shown(args)));
    let mut child = command(exe, args)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| EngineError::new("WIP-E020", format!("Couldn't start {}: {e}", exe.display())))?;
    let mut stdout = child.stdout.take().expect("piped");
    let mut stderr = child.stderr.take().expect("piped");
    let err_thread = std::thread::spawn(move || {
        let mut s = Vec::new();
        let _ = stderr.read_to_end(&mut s);
        s
    });
    let mut all = Vec::new();
    let mut buf = [0u8; 4096];
    loop {
        match stdout.read(&mut buf) {
            Ok(0) | Err(_) => break,
            Ok(n) => {
                all.extend_from_slice(&buf[..n]);
                if let Some(p) = last_percent(&String::from_utf8_lossy(&buf[..n])) {
                    job.progress(p / 100.0);
                }
            }
        }
    }
    let status = child.wait().map_err(|e| EngineError::new("WIP-E020", e.to_string()))?;
    all.extend(err_thread.join().unwrap_or_default());
    Ok(Output { code: status.code().unwrap_or(-1), text: String::from_utf8_lossy(&all).into_owned() })
}

/// Finds the last "12.3%" style number in a chunk of tool output.
pub fn last_percent(s: &str) -> Option<f32> {
    let bytes = s.as_bytes();
    let mut best = None;
    for (i, &b) in bytes.iter().enumerate() {
        if b == b'%' {
            let mut j = i;
            while j > 0 && (bytes[j - 1].is_ascii_digit() || bytes[j - 1] == b'.') {
                j -= 1;
            }
            if j < i {
                if let Ok(v) = s[j..i].parse::<f32>() {
                    if (0.0..=100.0).contains(&v) {
                        best = Some(v);
                    }
                }
            }
        }
    }
    best
}

/// Fails with the tool's own message when the exit code isn't 0.
pub fn expect_ok(out: Output, what: &str, code: &'static str) -> Result<String> {
    if out.code == 0 {
        Ok(out.text)
    } else {
        let tail: Vec<&str> = out.text.lines().filter(|l| !l.trim().is_empty()).rev().take(4).collect();
        let msg = tail.into_iter().rev().collect::<Vec<_>>().join(" ");
        Err(EngineError::new(code, format!("{what} failed (exit {}). {msg}", out.code)))
    }
}

/// Quotes a string for a PowerShell single-quoted literal.
pub fn ps_quote(s: &str) -> String {
    format!("'{}'", s.replace('\'', "''"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn percent_parsing() {
        assert_eq!(last_percent("[=====  10.0%   ]"), Some(10.0));
        assert_eq!(last_percent("[ 10.0% ] [ 55.5% ]"), Some(55.5));
        assert_eq!(last_percent("no progress"), None);
        assert_eq!(last_percent("100%"), Some(100.0));
    }

    #[test]
    fn quoting() {
        assert_eq!(ps_quote("C:\\a'b"), "'C:\\a''b'");
    }
}
