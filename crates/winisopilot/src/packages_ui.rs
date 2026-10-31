//! The Updates page of the Customize step: Windows updates, language packs
//! and Features on Demand added to the install image.

use crate::app::App;
use crate::theme::{self, body, dim};
use eframe::egui::{self, RichText, Ui};
use wip_core::packages::PackageKind;

fn add_files(app: &mut App) {
    let Some(files) = rfd::FileDialog::new()
        .set_title("Windows updates or language packs")
        .add_filter("Windows packages", &["msu", "cab"])
        .pick_files()
    else {
        return;
    };
    let n = app.packages.add(files);
    app.notify(format!("Added {n} packages."));
}

fn add_folder(app: &mut App) {
    let Some(dir) = rfd::FileDialog::new().set_title("Folder with .msu / .cab files").pick_folder() else { return };
    let files: Vec<_> = match std::fs::read_dir(&dir) {
        Ok(e) => e.flatten().map(|e| e.path()).filter(|p| p.is_file()).collect(),
        Err(e) => return app.notify(format!("Couldn't read {}: {e}", dir.display())),
    };
    let n = app.packages.add(files);
    if n == 0 {
        app.notify("No .msu or .cab files in that folder.");
    } else {
        app.notify(format!("Added {n} packages."));
    }
}

pub fn count(app: &App) -> usize {
    let arch = crate::drivers_ui::arch(app);
    app.packages.packages.iter().filter(|p| p.arch_mismatch(&arch).is_none()).count()
}

pub fn panel(app: &mut App, ui: &mut Ui, guided: bool) {
    let arch = crate::drivers_ui::arch(app);
    if guided {
        body(ui, "Optional. Add Windows updates so the new Windows starts up to date, or language packs for other display languages.");
        dim(ui, "Get updates from the Microsoft Update Catalog (.msu files). On Windows 11 24H2 and later, keep every file the catalog lists for an update in one folder.");
    } else {
        dim(ui, &format!("Microsoft's Dynamic Update order: install.wim gets SSU, LP, FoD, LCU, (cleanup), .NET; boot.wim #2 gets SSU + LCU; Winre.wim gets SSU + Safe OS; Setup updates are expanded into sources. Set the kind for Safe OS / Setup updates (catalog names don't show it). 24H2+: keep checkpoint MSUs together. Arch: {arch}."));
    }
    ui.add_space(4.0);
    ui.horizontal(|ui| {
        if ui.add_enabled(!app.busy(), egui::Button::new("Add files…")).clicked() {
            add_files(app);
        }
        if ui.add_enabled(!app.busy(), egui::Button::new("Add folder…")).clicked() {
            add_folder(app);
        }
        if !app.packages.is_empty() && ui.add_enabled(!app.busy(), egui::Button::new("Remove all")).clicked() {
            app.packages.packages.clear();
        }
    });
    ui.add_space(4.0);
    if app.packages.is_empty() {
        dim(ui, "No updates or language packs added.");
        return;
    }
    let mut remove = None;
    let mut retype: Option<(usize, PackageKind)> = None;
    let order: Vec<std::path::PathBuf> = app.packages.ordered().iter().map(|p| p.path.clone()).collect();
    egui::ScrollArea::vertical().auto_shrink([false, true]).show(ui, |ui| {
        theme::card().show(ui, |ui| {
            ui.set_width(ui.available_width());
            egui::Grid::new("packages").striped(true).spacing([10.0, 3.0]).show(ui, |ui| {
                for h in ["#", "FILE", "KIND", "INTO", "STATUS", ""] {
                    ui.label(RichText::new(h).monospace().size(10.5).color(theme::TEXT_DIM));
                }
                ui.end_row();
                for (n, path) in order.iter().enumerate() {
                    let Some(i) = app.packages.packages.iter().position(|p| &p.path == path) else { continue };
                    let p = &app.packages.packages[i];
                    let why = p.arch_mismatch(&arch);
                    let color = if why.is_none() { theme::TEXT_BODY } else { theme::TEXT_DIM };
                    ui.label(RichText::new(format!("{}", n + 1)).monospace().size(11.0).color(theme::TEXT_DIM));
                    let name = p.file_name();
                    let short: String = if name.chars().count() > 44 {
                        format!("{}…", name.chars().take(43).collect::<String>())
                    } else {
                        name.clone()
                    };
                    ui.label(RichText::new(short).monospace().size(11.0).color(color))
                        .on_hover_text(p.path.display().to_string());
                    if guided {
                        ui.label(RichText::new(p.kind.label()).size(12.0).color(color));
                    } else {
                        let mut k = p.kind;
                        egui::ComboBox::from_id_salt(("kind", i)).selected_text(k.label()).width(130.0).show_ui(
                            ui,
                            |ui| {
                                for c in PackageKind::ALL {
                                    ui.selectable_value(&mut k, c, c.label());
                                }
                            },
                        );
                        if k != p.kind {
                            retype = Some((i, k));
                        }
                    }
                    let kind_now = retype.filter(|(j, _)| *j == i).map_or(p.kind, |(_, k)| k);
                    ui.label(RichText::new(kind_now.targets()).size(11.0).color(theme::TEXT_DIM));
                    match &why {
                        None => ui.label(RichText::new("add").size(12.0).color(theme::OK_TEXT)),
                        Some(w) => ui.label(RichText::new("skip").size(12.0).color(theme::TEXT_DIM)).on_hover_text(w),
                    };
                    if ui.small_button("✖").on_hover_text("Remove").clicked() {
                        remove = Some(i);
                    }
                    ui.end_row();
                }
            });
        });
        ui.add_space(4.0);
        dim(ui, "Language packs change the languages Windows can show after installing; Windows Setup itself stays in the ISO's language.");
    });
    if let Some((i, k)) = retype {
        app.packages.packages[i].kind = k;
    }
    if let Some(i) = remove {
        app.packages.packages.remove(i);
    }
}
