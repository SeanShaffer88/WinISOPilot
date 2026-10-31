//! The Drivers page of the Customize step (both layouts): add folders of
//! unzipped driver packages; WinISOPilot adds the signed ones that match
//! the edition's architecture.

use crate::app::App;
use crate::theme::{self, body, dim};
use eframe::egui::{self, RichText, Ui};
use wip_core::drivers::{self, DriverFolder};

/// Architecture of the open edition (`x64` when unknown).
pub fn arch(app: &App) -> String {
    app.edition().map(|e| e.architecture.clone()).filter(|a| a != "unknown").unwrap_or_else(|| "x64".into())
}

pub fn add_folder(app: &mut App) {
    let Some(path) = rfd::FileDialog::new().set_title("Folder with unzipped drivers (.inf files)").pick_folder() else {
        return;
    };
    if app.drivers.folders.iter().any(|f| f.path == path) {
        return app.notify("That folder is already added.");
    }
    match drivers::scan(&path) {
        Ok((infs, _)) if infs.is_empty() => {
            app.notify(format!(
                "No .inf files in {}. Unzip the driver download first; .exe installers can't be added.",
                path.display()
            ));
        }
        Ok((infs, truncated)) => {
            let (ok, skip) = {
                let a = arch(app);
                let ok = infs.iter().filter(|i| i.unusable(&a).is_none()).count();
                (ok, infs.len() - ok)
            };
            app.log_line(format!("Drivers: {} ({ok} usable, {skip} skipped)", path.display()));
            app.drivers.folders.push(DriverFolder { path, infs, truncated });
            app.notify(format!(
                "Added {ok} drivers{}.",
                if skip > 0 { format!(", {skip} skipped") } else { String::new() }
            ));
        }
        Err(e) => app.notify(format!("Couldn't read {}: {e}", path.display())),
    }
}

/// The page body. `guided` hides INF-level detail behind tooltips.
pub fn panel(app: &mut App, ui: &mut Ui, guided: bool) {
    let a = arch(app);
    if guided {
        body(
            ui,
            "Most PCs don't need this. Add drivers if Windows Setup can't find your disk (common on laptops with Intel RST or VMD), or if Wi-Fi doesn't work right after installing.",
        );
        dim(ui, "Download the driver from your PC maker's support page and unzip it into a folder. Installers (.exe) can't be added.");
    } else {
        dim(ui, &format!("Signed .inf packages for {a} are added with DISM (one per INF, folders searched). Unsigned and other-architecture INFs are skipped; unsigned drivers are never forced."));
    }
    ui.add_space(4.0);
    ui.horizontal(|ui| {
        if ui.add_enabled(!app.busy(), egui::Button::new("Add driver folder…")).clicked() {
            add_folder(app);
        }
        ui.checkbox(&mut app.drivers.setup_storage, "Also give Windows Setup the disk drivers")
            .on_hover_text("Storage-controller drivers (class SCSIAdapter or HDC) are also added to boot.wim, so Setup can see your disk.");
    });
    ui.add_space(4.0);
    if app.drivers.folders.is_empty() {
        dim(ui, "No driver folders added.");
        return;
    }
    let mut remove = None;
    egui::ScrollArea::vertical().auto_shrink([false, true]).show(ui, |ui| {
        for (n, f) in app.drivers.folders.iter().enumerate() {
            theme::card().show(ui, |ui| {
                ui.set_width(ui.available_width());
                let ok = f.infs.iter().filter(|i| i.unusable(&a).is_none()).count();
                let storage = f.infs.iter().filter(|i| i.unusable(&a).is_none() && i.is_storage()).count();
                ui.horizontal(|ui| {
                    ui.add(egui::Label::new(RichText::new(f.path.display().to_string()).strong()).truncate());
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if ui.add_enabled(!app.busy(), egui::Button::new("Remove")).clicked() {
                            remove = Some(n);
                        }
                    });
                });
                let mut summary = format!("{ok} of {} drivers will be added", f.infs.len());
                if storage > 0 {
                    summary.push_str(&format!(" · {storage} disk drivers"));
                }
                if f.truncated {
                    summary.push_str(&format!(" · only the first {} .inf files were read", drivers::MAX_INFS));
                }
                let skipped: Vec<String> = f
                    .infs
                    .iter()
                    .filter_map(|i| i.unusable(&a).map(|why| format!("{}: {why}", i.file_name())))
                    .collect();
                let r = ui.label(RichText::new(summary).size(12.0).color(theme::TEXT_DIM));
                if !skipped.is_empty() {
                    r.on_hover_text(format!("Skipped:\n{}", skipped.join("\n")));
                }
                if !guided {
                    ui.add_space(2.0);
                    for i in &f.infs {
                        let status = i.unusable(&a).unwrap_or_else(|| "add".into());
                        let color = if i.unusable(&a).is_none() { theme::OK_TEXT } else { theme::TEXT_DIM };
                        let line = format!(
                            "{:<28} {:<12} {:<20} {:<24} {}",
                            i.file_name(),
                            i.class,
                            i.provider,
                            i.version,
                            status
                        );
                        ui.add(egui::Label::new(RichText::new(line).monospace().size(11.0).color(color)).truncate())
                            .on_hover_text(i.path.display().to_string());
                    }
                }
            });
            ui.add_space(4.0);
        }
    });
    if let Some(n) = remove {
        let f = app.drivers.folders.remove(n);
        app.notify(format!("Removed {}.", f.path.display()));
    }
}

/// Short line for footers and the rail.
pub fn count(app: &App) -> usize {
    app.drivers.counts(&arch(app)).0
}
