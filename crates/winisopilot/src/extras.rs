//! Differentiating features in the UI: measured footprint, boot test in a
//! VM, and recipe porting across Windows builds.

use crate::app::App;
use crate::jobs::{Kind, Outcome};
use crate::theme::{self, body, check_line, dim, h2};
use eframe::egui::{self, RichText, ScrollArea};
use std::collections::BTreeSet;
use wip_core::boottest::checks;
use wip_core::inventory::Inventory;
use wip_core::plan::{CompiledPlan, Context};
use wip_core::port::{port, PortReport, PortStatus};
use wip_core::recipe::LoadedRecipe;
use wip_core::{ImageSummary, WindowsFamily};
use wip_engine::{BootOutcome, BootTestReport, BootTestRequest, BuildReport};

/// What the last build used, kept after the workspace is closed.
#[derive(Clone)]
pub struct BuildSnapshot {
    pub plan: CompiledPlan,
    pub inventory: Inventory,
    pub edition: ImageSummary,
}

pub struct ExtrasState {
    pub snapshot: Option<BuildSnapshot>,
    pub keep_vm: bool,
    pub timeout_minutes: u32,
    pub boot_report: Option<BootTestReport>,
    /// A recipe loaded before an image was open; checked once it is.
    pub pending_recipe: Option<LoadedRecipe>,
    pub port_report: Option<PortReport>,
    pub show_port: bool,
    pub add_new: BTreeSet<String>,
}

impl Default for ExtrasState {
    fn default() -> Self {
        ExtrasState {
            snapshot: None,
            keep_vm: false,
            timeout_minutes: 90,
            boot_report: None,
            pending_recipe: None,
            port_report: None,
            show_port: false,
            add_new: BTreeSet::new(),
        }
    }
}

/// Context for fingerprinting a recipe: the open image, or the last build.
pub fn fingerprint_ctx(app: &App) -> Option<(Context<'_>, String)> {
    if let (Some(ctx), Some(e)) = (app.plan_ctx(), app.edition()) {
        if ctx.inventory.is_some() {
            return Some((ctx, e.edition_id.clone()));
        }
    }
    let s = app.extras.snapshot.as_ref()?;
    let family = s.edition.family().unwrap_or(WindowsFamily::Win11);
    Some((
        Context { catalog: &app.catalog, family, build: s.edition.build, inventory: Some(&s.inventory) },
        s.edition.edition_id.clone(),
    ))
}

/// Runs the port check if a recipe is waiting and an image is open.
pub fn check_pending_recipe(app: &mut App) {
    let Some(rec) = app.extras.pending_recipe.take() else { return };
    let Some(ctx) = app.plan_ctx().filter(|c| c.inventory.is_some()) else {
        app.extras.pending_recipe = Some(rec);
        return;
    };
    let report = port(&rec, &ctx);
    app.extras.add_new.clear();
    app.extras.show_port = !report.is_clean();
    if report.is_clean() {
        app.notify("Recipe checked: every option does the same thing on this image.");
    }
    app.extras.port_report = Some(report);
}

// ------------------------------------------------------------- Footprint

pub fn footprint_card(ui: &mut egui::Ui, r: &BuildReport) {
    let Some(f) = &r.footprint else { return };
    ui.add_space(8.0);
    theme::card().show(ui, |ui| {
        ui.set_width(ui.available_width());
        h2(ui, "Measured result");
        dim(ui, "Read from the image before and after your changes. Nothing here is estimated.");
        for l in f.summary_lines() {
            body(ui, &l);
        }
        let c = &f.changes;
        egui::CollapsingHeader::new(format!("What changed ({} items)", c.count())).show(ui, |ui| {
            for (title, list) in [
                ("Apps removed", &c.apps_removed),
                ("Capabilities removed", &c.capabilities_removed),
                ("Features turned off", &c.features_turned_off),
                ("Features turned on", &c.features_turned_on),
            ] {
                if !list.is_empty() {
                    ui.label(RichText::new(title).strong());
                    for i in list {
                        ui.label(RichText::new(i).monospace().size(12.0).color(theme::TEXT_DIM));
                    }
                }
            }
        });
        if let Some(root) = r.media_folder.parent() {
            dim(ui, &format!("Full report: {}", root.join("footprint.md").display()));
        }
    });
}

// ------------------------------------------------------------- Boot test

pub fn boot_test_card(app: &mut App, ui: &mut egui::Ui, r: &BuildReport) {
    ui.add_space(8.0);
    theme::card().show(ui, |ui| {
        ui.set_width(ui.available_width());
        h2(ui, "Test it in a virtual machine");
        body(
            ui,
            "WinISOPilot installs your ISO in a temporary Hyper-V virtual machine, signs in, and checks from inside Windows that every change actually took effect. Your PC and disks aren't touched.",
        );
        let Some(iso) = r.iso.clone() else {
            dim(ui, "Needs an ISO file. Install the Windows ADK Deployment Tools to create one.");
            return;
        };
        if wip_core::boottest::setup_key(&r.test_target.edition_id).is_none() {
            theme::warn_box(
                ui,
                "Setup may stop at the product key page",
                &["Microsoft doesn't publish a setup key for this edition. If the test times out, open the VM in Hyper-V Manager and choose \"I don't have a product key\".".into()],
            );
        }
        dim(ui, "Needs Hyper-V (Windows Pro, Enterprise or Education), about 70 GB free and 4 GB of spare memory. Usually takes 15–40 minutes.");
        ui.checkbox(&mut app.extras.keep_vm, "Keep the VM afterwards (it's always kept if a check fails)");
        ui.horizontal(|ui| {
            ui.label("Give up after");
            ui.add(egui::DragValue::new(&mut app.extras.timeout_minutes).range(20..=240).suffix(" min"));
        });
        if theme::primary(ui, "Run boot test", !app.busy()).clicked() {
            let Some(snap) = app.extras.snapshot.clone() else { return };
            let Some(parent) = app
                .workspace_parent
                .clone()
                .or_else(|| r.media_folder.parent().and_then(|p| p.parent()).map(|p| p.to_path_buf()))
            else {
                return;
            };
            let req = BootTestRequest {
                iso,
                target: r.test_target.clone(),
                checks: checks(&snap.plan),
                work_parent: parent,
                keep_vm: app.extras.keep_vm,
                timeout_minutes: app.extras.timeout_minutes,
            };
            let host = app.host.clone();
            app.extras.boot_report = None;
            app.start(Kind::BootTest, move |job| wip_engine::boot_test(&req, &host, job).map(Outcome::BootTested));
        }
        if let Some(b) = app.extras.boot_report.clone() {
            ui.add_space(6.0);
            boot_result(ui, &b);
        }
    });
}

fn boot_result(ui: &mut egui::Ui, b: &BootTestReport) {
    match b.outcome {
        BootOutcome::Passed => {
            check_line(ui, true, &format!("Installed and booted in {} min. Every change checked out.", b.minutes))
        }
        BootOutcome::ChecksFailed => check_line(
            ui,
            false,
            &format!("Installed and booted in {} min, but some changes weren't found.", b.minutes),
        ),
        BootOutcome::SetupDidNotFinish => check_line(ui, false, "The VM stopped before Windows finished setting up."),
        BootOutcome::TimedOut => check_line(ui, false, &format!("Setup didn't finish within {} min.", b.minutes)),
    }
    if let Some(s) = &b.scored {
        dim(ui, &format!("{} · build {} · {} checks passed", s.caption, s.build, s.passed));
        if !s.failed.is_empty() {
            let lines: Vec<String> = s
                .failed
                .iter()
                .map(|f| if f.detail.is_empty() { f.what.clone() } else { format!("{} ({})", f.what, f.detail) })
                .collect();
            theme::warn_box(ui, "Not as expected", &lines);
        }
        if !s.missing.is_empty() {
            theme::warn_box(ui, "Not reported by the VM", &s.missing);
        }
    }
    if b.vm_kept {
        dim(ui, &format!("The VM \"{}\" is in Hyper-V Manager. Files: {}", b.vm_name, b.folder.display()));
    } else {
        dim(ui, "The test VM and its files were removed.");
    }
}

// ------------------------------------------------------------- Recipe port

pub fn port_dialog(app: &mut App, ctx: &egui::Context) {
    if !app.extras.show_port {
        return;
    }
    let Some(rep) = app.extras.port_report.clone() else {
        app.extras.show_port = false;
        return;
    };
    let mut close = false;
    let mut drop_missing = false;
    let mut add = false;
    egui::Modal::new(egui::Id::new("port")).show(ctx, |ui| {
        ui.set_width(620.0);
        h2(ui, "Recipe check");
        let from = rep.from_build.map_or("an unknown build".to_string(), |b| format!("build {b}"));
        body(ui, &format!("Your choices were saved for {from}. This image is build {}.", rep.to_build));
        let same = rep.count(|s| *s == PortStatus::Same);
        let changed: Vec<_> = rep.items.iter().filter(|i| matches!(i.status, PortStatus::Changed { .. })).collect();
        let missing: Vec<_> = rep.items.iter().filter(|i| matches!(i.status, PortStatus::Missing(_))).collect();
        let unverified = rep.count(|s| *s == PortStatus::Unverified);
        ScrollArea::vertical().max_height(380.0).show(ui, |ui| {
            check_line(ui, true, &format!("{same} options do exactly the same thing"));
            if unverified > 0 {
                dim(ui, &format!("{unverified} options apply, but this recipe is from an older version and can't be compared in detail."));
            }
            if !changed.is_empty() {
                ui.add_space(6.0);
                ui.label(RichText::new(format!("{} options now affect different items", changed.len())).strong().color(theme::WARN_TEXT));
                for i in changed {
                    if let PortStatus::Changed { gone, new } = &i.status {
                        ui.label(&i.title);
                        for g in gone {
                            ui.label(RichText::new(format!("   − {g}")).monospace().size(12.0).color(theme::TEXT_DIM));
                        }
                        for n in new {
                            ui.label(RichText::new(format!("   + {n}")).monospace().size(12.0).color(theme::TEXT_DIM));
                        }
                    }
                }
            }
            if !missing.is_empty() {
                ui.add_space(6.0);
                ui.label(RichText::new(format!("{} options don't apply to this image", missing.len())).strong().color(theme::WARN_TEXT));
                for i in &missing {
                    if let PortStatus::Missing(why) = &i.status {
                        ui.label(RichText::new(format!("{} — {why}", i.title)).color(theme::TEXT_DIM));
                    }
                }
            }
            if !rep.new_options.is_empty() {
                ui.add_space(6.0);
                ui.label(RichText::new(format!("{} new options since this recipe was saved", rep.new_options.len())).strong());
                for id in &rep.new_options {
                    let Some(o) = app.catalog.get(id) else { continue };
                    let mut on = app.extras.add_new.contains(id);
                    ui.horizontal(|ui| {
                        if ui.checkbox(&mut on, &o.title).changed() {
                            if on {
                                app.extras.add_new.insert(id.clone());
                            } else {
                                app.extras.add_new.remove(id);
                            }
                        }
                        theme::badge(ui, Some(o.risk));
                    });
                }
            }
            if !rep.unknown.is_empty() {
                ui.add_space(6.0);
                dim(ui, &format!("{} entries come from a newer WinISOPilot and were ignored.", rep.unknown.len()));
            }
        });
        ui.add_space(8.0);
        ui.horizontal(|ui| {
            if !missing.is_empty() && theme::secondary(ui, "Remove options that don't apply", true).clicked() {
                drop_missing = true;
            }
            if !app.extras.add_new.is_empty() && theme::secondary(ui, "Add ticked new options", true).clicked() {
                add = true;
            }
            if theme::primary(ui, "Done", true).clicked() {
                close = true;
            }
        });
    });
    if drop_missing {
        app.replace_selection(rep.applicable_selection());
        app.notify("Removed options that don't apply. Ctrl+Z undoes.");
    }
    if add {
        let mut sel = app.selection.clone();
        sel.extend(app.extras.add_new.iter().cloned());
        let n = app.extras.add_new.len();
        app.replace_selection(sel);
        app.extras.add_new.clear();
        app.notify(format!("Added {n} new options. Ctrl+Z undoes."));
    }
    if close {
        app.extras.show_port = false;
    }
}
