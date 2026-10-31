//! Headless builds for automation:
//!
//! ```text
//! WinISOPilot.exe --build --iso Win11.iso --out Custom.iso [--recipe choices.json]
//!     [--edition 6 | --edition-id Professional] [--workspace D:\Work]
//!     [--drivers D:\Drivers]... [--updates D:\Updates]... [--all-editions]
//! ```
//!
//! Same engine, plan and safety rules as the app; the recipe supplies the
//! options and output settings. Exit codes: 0 success, 1 failure, 2 usage.

use std::path::PathBuf;
use std::sync::atomic::AtomicBool;
use std::sync::mpsc::channel;
use std::sync::Arc;
use wip_core::drivers::{DriverChoice, DriverFolder};
use wip_core::packages::PackageChoice;
use wip_core::plan::{compile, Context};
use wip_core::recipe::{OutputSettings, Recipe};
use wip_core::{Catalog, WindowsFamily};
use wip_engine::{Event, Job, PrepareRequest};

pub const USAGE: &str = "Usage: WinISOPilot.exe --build --iso <file.iso> --out <new.iso> [--recipe <choices.json>]
       [--edition <index> | --edition-id <EditionID>] [--workspace <folder>]
       [--drivers <folder>]... [--updates <folder>]... [--all-editions]
Run it from an administrator terminal so the output appears there. Exit codes: 0 success, 1 failure, 2 usage error.";

#[derive(Debug, Default, PartialEq)]
pub struct Args {
    pub iso: PathBuf,
    pub out: PathBuf,
    pub recipe: Option<PathBuf>,
    pub edition: Option<u32>,
    pub edition_id: Option<String>,
    pub workspace: Option<PathBuf>,
    pub drivers: Vec<PathBuf>,
    pub updates: Vec<PathBuf>,
    pub all_editions: bool,
}

pub fn parse(args: &[String]) -> Result<Args, String> {
    let mut a = Args::default();
    let mut it = args.iter();
    let mut iso = None;
    let mut out = None;
    while let Some(arg) = it.next() {
        let mut value = |name: &str| it.next().cloned().ok_or_else(|| format!("{name} needs a value"));
        match arg.as_str() {
            "--build" => {}
            "--iso" => iso = Some(PathBuf::from(value("--iso")?)),
            "--out" => out = Some(PathBuf::from(value("--out")?)),
            "--recipe" => a.recipe = Some(PathBuf::from(value("--recipe")?)),
            "--edition" => {
                let v = value("--edition")?;
                a.edition = Some(v.parse().map_err(|_| format!("--edition needs a number, not {v}"))?);
            }
            "--edition-id" => a.edition_id = Some(value("--edition-id")?),
            "--workspace" => a.workspace = Some(PathBuf::from(value("--workspace")?)),
            "--drivers" => a.drivers.push(PathBuf::from(value("--drivers")?)),
            "--updates" => a.updates.push(PathBuf::from(value("--updates")?)),
            "--all-editions" => a.all_editions = true,
            other => return Err(format!("Unknown argument {other}")),
        }
    }
    a.iso = iso.ok_or("--iso is required")?;
    a.out = out.ok_or("--out is required")?;
    if a.edition.is_some() && a.edition_id.is_some() {
        return Err("Use --edition or --edition-id, not both".into());
    }
    Ok(a)
}

fn job() -> (Job, std::thread::JoinHandle<()>) {
    let (tx, rx) = channel::<Event>();
    let printer = std::thread::spawn(move || {
        let mut last = -1i32;
        for ev in rx {
            match ev {
                Event::Stage(n, t, label) => {
                    last = -1;
                    println!("[{n}/{t}] {label}");
                }
                Event::Progress(Some(p)) => {
                    let pct = (p * 100.0) as i32 / 10 * 10;
                    if pct != last {
                        last = pct;
                        println!("      {pct}%");
                    }
                }
                Event::Progress(None) => {}
                Event::Log(l) => println!("      {l}"),
            }
        }
    });
    (Job::new(tx, Arc::new(AtomicBool::new(false))), printer)
}

pub fn run(raw: &[String]) -> i32 {
    wip_engine::attach_parent_console();
    let args = match parse(raw) {
        Ok(a) => a,
        Err(e) => {
            eprintln!("{e}\n\n{USAGE}");
            return 2;
        }
    };
    match build(&args) {
        Ok(()) => 0,
        Err(e) => {
            eprintln!("Failed: {e}");
            1
        }
    }
}

fn build(args: &Args) -> Result<(), String> {
    println!("WinISOPilot {} command-line build", wip_core::APP_VERSION);
    let catalog = Catalog::builtin().map_err(|e| e.to_string())?;
    let (selection, mut output) = match &args.recipe {
        Some(p) => {
            let bytes = std::fs::read(p).map_err(|e| format!("Can't read {}: {e}", p.display()))?;
            let r = Recipe::parse(&bytes, &catalog)?;
            for u in &r.unknown {
                println!("Ignoring unknown option {u}");
            }
            (r.selection, r.output)
        }
        None => (Default::default(), OutputSettings::default()),
    };
    if args.all_editions {
        output.single_edition = false;
    }
    if output.answers.enabled {
        if let Some(p) = output.answers.validate().into_iter().next() {
            return Err(format!("Setup answers: {p}"));
        }
    }
    let host = wip_engine::probe_host(None);
    let (job, printer) = job();
    let result = (|| -> Result<(), String> {
        let src = wip_engine::inspect_iso(&args.iso, &host, &job).map_err(|e| e.to_string())?;
        for i in &src.images {
            println!("  edition {}: {} ({})", i.index, i.name, i.edition_id);
        }
        let edition = match (args.edition, &args.edition_id) {
            (Some(n), _) => src.images.iter().find(|i| i.index == n),
            (None, Some(id)) => src.images.iter().find(|i| i.edition_id.eq_ignore_ascii_case(id)),
            (None, None) => src.images.first(),
        }
        .ok_or("That edition isn't in the ISO")?
        .clone();
        edition.support()?;
        println!("Using edition {}: {}", edition.index, edition.name);
        let parent = args
            .workspace
            .clone()
            .or_else(|| args.out.parent().map(|p| p.to_path_buf()))
            .ok_or("Choose a --workspace folder")?;
        let req = PrepareRequest {
            iso: src.iso.clone(),
            image_rel: src.image_rel.clone(),
            workspace_parent: parent,
            index: edition.index,
            all_indexes: src.images.iter().map(|i| i.index).collect(),
            single_edition: output.single_edition,
        };
        let (ws, inv) = wip_engine::prepare(&req, &host, &job).map_err(|e| e.to_string())?;
        let ctx = Context {
            catalog: &catalog,
            family: edition.family().unwrap_or(WindowsFamily::Win11),
            build: edition.build,
            inventory: Some(&inv),
        };
        let built = (|| -> Result<wip_engine::BuildReport, String> {
            let mut plan = compile(&selection, &ctx)?;
            let mut drivers = DriverChoice::new();
            for d in &args.drivers {
                let (infs, truncated) =
                    wip_core::drivers::scan(d).map_err(|e| format!("Can't read {}: {e}", d.display()))?;
                drivers.folders.push(DriverFolder { path: d.clone(), infs, truncated });
            }
            drivers.extend_plan(&mut plan, &edition.architecture);
            let mut packages = PackageChoice::default();
            for u in &args.updates {
                let files = std::fs::read_dir(u)
                    .map_err(|e| format!("Can't read {}: {e}", u.display()))?
                    .flatten()
                    .map(|e| e.path());
                packages.add(files);
            }
            packages.extend_plan(&mut plan, &edition.architecture);
            for (id, why) in &plan.skipped {
                println!("Skipping {id}: {why}");
            }
            println!("{} operations planned", plan.install.len() + plan.boot.len());
            wip_engine::build(&ws, &plan, &output, &args.out, &host, &inv, &job).map_err(|e| e.to_string())
        })();
        match built {
            Ok(r) => {
                for w in &r.warnings {
                    println!("Warning: {w}");
                }
                match &r.iso {
                    Some(p) => println!("Done: {}", p.display()),
                    None => println!("Done: files in {} (oscdimg not found, no ISO)", r.media_folder.display()),
                }
                if let Some(h) = &r.iso_sha256 {
                    println!("SHA-256 {h}");
                }
                Ok(())
            }
            Err(e) => {
                println!("Discarding the open image…");
                if let Err(d) = wip_engine::discard(&ws, &host, &job) {
                    println!("Couldn't discard: {d}");
                }
                Err(e)
            }
        }
    })();
    drop(job);
    let _ = printer.join();
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    fn v(s: &[&str]) -> Vec<String> {
        s.iter().map(|x| x.to_string()).collect()
    }

    #[test]
    fn parses_arguments() {
        let a = parse(&v(&[
            "--build",
            "--iso",
            "a.iso",
            "--out",
            "b.iso",
            "--edition",
            "6",
            "--drivers",
            "D1",
            "--drivers",
            "D2",
        ]))
        .unwrap();
        assert_eq!(a.iso, PathBuf::from("a.iso"));
        assert_eq!(a.edition, Some(6));
        assert_eq!(a.drivers.len(), 2);
        assert!(parse(&v(&["--build", "--out", "b.iso"])).is_err());
        assert!(parse(&v(&["--build", "--iso", "a", "--out", "b", "--edition", "x"])).is_err());
        assert!(parse(&v(&["--build", "--iso", "a", "--out", "b", "--edition", "1", "--edition-id", "Pro"])).is_err());
        assert!(parse(&v(&["--build", "--iso", "a", "--out", "b", "--bogus"])).is_err());
    }
}
