//! Wizard pages other than Customize.

use crate::app::{App, Step};
use crate::jobs::{Kind, Outcome};
use crate::theme::{self, body, check_line, dim, eyebrow, h1, h2};
use eframe::egui::{self, Frame, Margin, RichText, ScrollArea, Ui};
use std::path::PathBuf;
use wip_core::plan::{self, Severity};
use wip_core::recipe::{sanitize_label, Compression, Recipe};
use wip_core::{human_bytes, Category};
use wip_engine::PrepareRequest;

pub enum Nav {
    None,
    Back,
    Next,
}

/// Bottom bar: quiet link on the left, Back / primary on the right.
pub fn footer(ctx: &egui::Context, status: &str, back: bool, next: Option<(&str, bool)>) -> Nav {
    let mut nav = Nav::None;
    // Keyboard: Alt+Right = Next, Alt+Left = Back (only when the button is enabled).
    if next.is_some_and(|(_, enabled)| enabled)
        && ctx.input_mut(|i| i.consume_key(egui::Modifiers::ALT, egui::Key::ArrowRight))
    {
        nav = Nav::Next;
    } else if back && ctx.input_mut(|i| i.consume_key(egui::Modifiers::ALT, egui::Key::ArrowLeft)) {
        nav = Nav::Back;
    }
    egui::TopBottomPanel::bottom("footer")
        .exact_height(68.0)
        .frame(Frame::new().fill(theme::CHROME).inner_margin(Margin::symmetric(32, 12)))
        .show(ctx, |ui| {
            ui.horizontal_centered(|ui| {
                ui.label(RichText::new(status).color(theme::TEXT_DIM));
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if let Some((label, enabled)) = next {
                        if theme::primary(ui, label, enabled).on_hover_text("Alt+Right").clicked() {
                            nav = Nav::Next;
                        }
                    }
                    if back && theme::secondary(ui, "Back", true).on_hover_text("Alt+Left").clicked() {
                        nav = Nav::Back;
                    }
                });
            });
        });
    nav
}

pub fn central(ctx: &egui::Context, add: impl FnOnce(&mut Ui)) {
    egui::CentralPanel::default()
        .frame(Frame::new().fill(theme::PAGE).inner_margin(Margin { left: 44, right: 44, top: 30, bottom: 12 }))
        .show(ctx, |ui| {
            ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
                ui.set_max_width(ui.available_width().min(980.0));
                add(ui);
            });
        });
}

fn header(app: &App, ui: &mut Ui, step: Step, title: &str, blurb: &str) {
    eyebrow(ui, &format!("Step {} of 6", step.number()));
    h1(ui, title);
    if !blurb.is_empty() {
        body(ui, blurb);
    }
    crate::help::tip(app, ui);
    ui.add_space(10.0);
}

/// Progress, log tail and Cancel for the running job.
pub fn job_panel(app: &mut App, ui: &mut Ui) {
    let reduce_motion = app.settings.reduce_motion;
    let Some(job) = app.job.as_mut() else { return };
    theme::card().show(ui, |ui| {
        ui.set_width(ui.available_width());
        let (n, t, label) = job.stage.clone();
        h2(ui, job.kind.title());
        if t > 0 {
            eyebrow(ui, &format!("Stage {n} of {t}"));
        }
        body(ui, &label);
        let bar = match job.progress {
            Some(p) => egui::ProgressBar::new(p).show_percentage(),
            None if reduce_motion => egui::ProgressBar::new(0.0).text("Working…"),
            None => egui::ProgressBar::new(0.0).animate(true),
        };
        ui.add(bar.desired_height(14.0));
        let secs = job.started.elapsed().as_secs();
        dim(ui, &format!("Elapsed {}:{:02}", secs / 60, secs % 60));
        if job.kind.cancellable() {
            if job.cancel_requested {
                dim(ui, "Stopping after the current step finishes…");
            } else if theme::secondary(ui, "Cancel", true).clicked() {
                app.confirm_cancel = true;
            }
        }
    });
    log_tail(app, ui, 8);
}

pub fn log_tail(app: &App, ui: &mut Ui, lines: usize) {
    egui::CollapsingHeader::new("Details").default_open(false).show(ui, |ui| {
        let start = app.log.len().saturating_sub(lines.max(200));
        ScrollArea::vertical().max_height(220.0).stick_to_bottom(true).show(ui, |ui| {
            for l in &app.log[start..] {
                ui.label(RichText::new(l).monospace().size(12.0).color(theme::TEXT_DIM));
            }
        });
    });
}

fn show_error(app: &App, ui: &mut Ui) {
    if let Some(e) = &app.last_error {
        if e.is_cancelled() {
            theme::warn_box(ui, "Stopped", std::slice::from_ref(&e.message));
        } else {
            theme::error_box(ui, &format!("Something went wrong ({})", e.code), &e.message);
            ui.horizontal(|ui| {
                if ui.button("Copy details").on_hover_text("Copy the error code and message").clicked() {
                    ui.ctx().copy_text(format!("WinISOPilot {} {}: {}", wip_core::APP_VERSION, e.code, e.message));
                }
                if ui.button("Open log folder").clicked() {
                    open_folder(wip_engine::app_data_dir().join("logs"));
                }
                ui.label(
                    RichText::new("The Details section below shows what happened step by step.").color(theme::TEXT_DIM),
                );
            });
        }
        ui.add_space(8.0);
    }
}

fn workspace_banner(app: &mut App, ui: &mut Ui) -> bool {
    if app.workspace.is_none() {
        return false;
    }
    theme::warn_box(
        ui,
        "An edition is open for editing",
        &["To choose a different ISO or edition, discard it first. Your choices are kept.".into()],
    );
    if theme::secondary(ui, "Discard open edition", !app.busy()).clicked() {
        app.discard_workspace();
    }
    ui.add_space(8.0);
    true
}

// ---------------------------------------------------------------- Welcome

pub fn welcome(app: &mut App, ctx: &egui::Context) {
    let ready = app.host.is_admin && app.host.dism_api.is_some();
    let status = if ready { "This PC is ready." } else { "Some checks need attention." };
    if let Nav::Next = footer(ctx, status, false, Some(("Get started", !app.busy()))) {
        app.goto(Step::Source);
    }
    central(ctx, |ui| {
        header(app, ui,
            Step::Welcome,
            "Make your own Windows installer",
            "WinISOPilot walks you through removing apps, turning features on or off, and changing settings in a Windows 10 or 11 installation ISO. Your original ISO is never changed.",
        );
        theme::card().show(ui, |ui| {
            ui.set_width(ui.available_width());
            h2(ui, "How it works");
            for (n, t) in [
                "Choose an official Windows 10 or 11 ISO.",
                "Pick the edition, for example Windows 11 Pro.",
                "Choose what to remove or change. Each option explains what it does and what could stop working.",
                "Review everything in one list. Nothing has changed yet.",
                "Build. You get a new ISO to put on a USB stick or use in a virtual machine.",
            ]
            .iter()
            .enumerate()
            {
                body(ui, &format!("{}.  {t}", n + 1));
            }
        });
        ui.add_space(10.0);
        theme::card().show(ui, |ui| {
            ui.set_width(ui.available_width());
            h2(ui, "This PC");
            let h = &app.host;
            check_line(
                ui,
                h.is_admin,
                if h.is_admin {
                    "Running as administrator"
                } else {
                    "Not running as administrator (needed to open images)"
                },
            );
            match &h.dism_api {
                Some(p) => check_line(ui, true, &format!("DISM {} — {}", h.dism_version, p.display())),
                None => check_line(ui, false, "DISM not found"),
            }
            match &h.oscdimg {
                Some(p) => check_line(ui, true, &format!("ISO tool found — {}", p.display())),
                None => check_line(
                    ui,
                    false,
                    "ISO tool (oscdimg.exe from the Windows ADK) not found: you'll get a folder instead of an ISO",
                ),
            }
            if h.os_build > 0 {
                dim(ui, &format!("Windows build {}", h.os_build));
            }
            if h.dism_api.is_none() || h.oscdimg.is_none() {
                ui.hyperlink_to(
                    "Get the Windows ADK (Deployment Tools) from Microsoft",
                    "https://learn.microsoft.com/windows-hardware/get-started/adk-install",
                );
            }
        });
        if !app.leftovers.is_empty() {
            ui.add_space(10.0);
            leftovers(app, ui);
        }
        ui.add_space(10.0);
        job_panel(app, ui);
        show_error(app, ui);
    });
}

fn leftovers(app: &mut App, ui: &mut Ui) {
    theme::warn_box(
        ui,
        "An earlier session didn't finish",
        &["WinISOPilot found an image that was left open. Discard it to free the files; your ISO is not affected."
            .into()],
    );
    let recs = app.leftovers.clone();
    for rec in recs {
        ui.horizontal(|ui| {
            ui.label(RichText::new(&rec.mount_dir).monospace().size(12.5));
            if theme::secondary(ui, "Discard", !app.busy()).clicked() {
                let host = app.host.clone();
                let r = rec.clone();
                app.start(Kind::Leftover, move |job| {
                    wip_engine::clean_leftover(&r, &host, job).map(|_| Outcome::LeftoverCleaned(r.mount_dir.clone()))
                });
            }
            if theme::quiet(ui, "Forget").on_hover_text("Remove this record without touching the image").clicked() {
                wip_engine::forget_leftover(&rec);
                app.leftovers.retain(|l| l.mount_dir != rec.mount_dir);
            }
        });
    }
}

// ---------------------------------------------------------------- Source

pub fn source(app: &mut App, ctx: &egui::Context) {
    let have_source = app.source.as_ref().is_some_and(|s| Some(&s.iso) == app.iso_path.as_ref());
    let can_read = app.iso_path.is_some() && app.workspace_parent.is_some() && !app.busy();
    let label = if have_source { "Next" } else { "Read ISO" };
    match footer(ctx, "", true, Some((label, can_read || (have_source && !app.busy())))) {
        Nav::Back => app.goto(Step::Welcome),
        Nav::Next if have_source => app.goto(Step::Edition),
        Nav::Next => {
            let iso = app.iso_path.clone().expect("checked");
            let host = app.host.clone();
            app.start(Kind::Inspect, move |job| wip_engine::inspect_iso(&iso, &host, job).map(Outcome::Inspected));
        }
        Nav::None => {}
    }
    central(ctx, |ui| {
        header(
            app,
            ui,
            Step::Source,
            "Choose a Windows ISO",
            "Use an official Windows 10 or 11 ISO from Microsoft. WinISOPilot only reads it.",
        );
        let locked = workspace_banner(app, ui) || app.busy();
        theme::card().show(ui, |ui| {
            ui.set_width(ui.available_width());
            h2(ui, "Windows ISO");
            ui.horizontal(|ui| {
                let shown = app.iso_path.as_ref().map_or("No ISO chosen".to_string(), |p| p.display().to_string());
                ui.label(RichText::new(shown).monospace().size(13.0));
                if theme::secondary(ui, "Browse…", !locked).clicked() {
                    if let Some(p) = rfd::FileDialog::new()
                        .add_filter("Disc image", &["iso"])
                        .set_title("Choose a Windows ISO")
                        .pick_file()
                    {
                        if app.workspace_parent.is_none() {
                            app.workspace_parent = p.parent().map(|d| d.to_path_buf());
                        }
                        app.iso_path = Some(p);
                        app.source = None;
                        app.chosen_index = None;
                    }
                }
            });
            ui.add_space(4.0);
            dim(ui, "Or get the ISO from Microsoft's website, then choose it here:");
            ui.horizontal(|ui| {
                ui.hyperlink_to("Windows 11 download page", "https://www.microsoft.com/software-download/windows11");
                ui.label("·");
                ui.hyperlink_to("Windows 10 download page", "https://www.microsoft.com/software-download/windows10ISO");
            });
        });
        ui.add_space(10.0);
        crate::download_ui::card(app, ui, locked);
        ui.add_space(10.0);
        theme::card().show(ui, |ui| {
            ui.set_width(ui.available_width());
            h2(ui, "Working folder");
            body(
                ui,
                "WinISOPilot needs space to unpack Windows while you customize it. Pick a fast local drive (NTFS).",
            );
            ui.horizontal(|ui| {
                let shown =
                    app.workspace_parent.as_ref().map_or("No folder chosen".to_string(), |p| p.display().to_string());
                ui.label(RichText::new(shown).monospace().size(13.0));
                if theme::secondary(ui, "Browse…", !locked).clicked() {
                    if let Some(p) = rfd::FileDialog::new().set_title("Choose a working folder").pick_folder() {
                        app.workspace_parent = Some(p);
                    }
                }
            });
            space_line(app, ui);
        });
        ui.add_space(10.0);
        job_panel(app, ui);
        show_error(app, ui);
    });
}

fn space_line(app: &mut App, ui: &mut Ui) {
    let Some(dir) = app.workspace_parent.clone() else { return };
    let free = app.free_space_cached(&dir);
    let iso = app.iso_path.as_ref().and_then(|p| std::fs::metadata(p).ok()).map(|m| m.len());
    let edition = app.edition().map(|e| e.size_bytes).unwrap_or(0);
    match (free, iso) {
        (Some(f), Some(i)) => {
            let need = wip_engine::required_space(i, edition);
            check_line(
                ui,
                f >= need,
                &format!("{} free; about {} needed (estimate)", human_bytes(f), human_bytes(need)),
            );
        }
        (Some(f), None) => dim(ui, &format!("{} free", human_bytes(f))),
        _ => {}
    }
}

// ---------------------------------------------------------------- Edition

pub fn edition(app: &mut App, ctx: &egui::Context) {
    let ready = app.workspace.as_ref().is_some_and(|w| Some(w.edition.index) == app.chosen_index);
    let supported = app.edition().is_some_and(|e| e.support().is_ok());
    let label = if ready { "Next" } else { "Open edition" };
    match footer(ctx, "", true, Some((label, supported && !app.busy()))) {
        Nav::Back => app.goto(Step::Source),
        Nav::Next if ready => app.goto(Step::Customize),
        Nav::Next => start_prepare(app),
        Nav::None => {}
    }
    central(ctx, |ui| {
        header(
            app,
            ui,
            Step::Edition,
            "Pick the edition to customize",
            "An ISO usually holds several editions. Choose the one you'll install. Opening it takes a few minutes.",
        );
        let locked = workspace_banner(app, ui) || app.busy();
        let Some(src) = app.source.clone() else {
            body(ui, "Read an ISO first.");
            return;
        };
        for img in &src.images {
            let support = img.support();
            let selected = app.chosen_index == Some(img.index);
            let frame = theme::card().stroke(egui::Stroke::new(
                if selected { 2.0_f32 } else { 1.0 },
                if selected { theme::ACCENT } else { theme::DIVIDER },
            ));
            let r = frame
                .show(ui, |ui| {
                    ui.set_width(ui.available_width());
                    ui.horizontal(|ui| {
                        let mut sel = selected;
                        let rb = ui.add_enabled(support.is_ok() && !locked, egui::RadioButton::new(sel, ""));
                        if rb.clicked() {
                            sel = true;
                        }
                        ui.vertical(|ui| {
                            ui.label(RichText::new(&img.name).size(16.0).strong());
                            let fam = img.family().map_or("Unsupported", |f| f.label());
                            dim(
                                ui,
                                &format!(
                                    "{fam} · {} · {} · {} · {}",
                                    img.version_string(),
                                    img.architecture,
                                    human_bytes(img.size_bytes),
                                    img.languages.join(", ")
                                ),
                            );
                            if let Err(why) = &support {
                                ui.label(RichText::new(why).color(theme::ERR_TEXT));
                            }
                        });
                        sel
                    })
                    .inner
                })
                .inner;
            if r && !selected && support.is_ok() && !locked {
                app.chosen_index = Some(img.index);
            }
            ui.add_space(6.0);
        }
        ui.add_space(4.0);
        ui.add_enabled_ui(!locked, |ui| {
            ui.checkbox(&mut app.single_edition, "Keep only this edition in the new ISO");
        });
        dim(ui, "Recommended. The ISO gets smaller and Setup won't ask which edition to install. Untick to keep every edition (only the chosen one is customized).");
        ui.add_space(6.0);
        space_line(app, ui);
        ui.add_space(10.0);
        job_panel(app, ui);
        show_error(app, ui);
    });
}

fn start_prepare(app: &mut App) {
    let (Some(src), Some(index), Some(parent)) = (app.source.clone(), app.chosen_index, app.workspace_parent.clone())
    else {
        return;
    };
    let req = PrepareRequest {
        iso: src.iso.clone(),
        image_rel: src.image_rel.clone(),
        workspace_parent: parent,
        index,
        all_indexes: src.images.iter().map(|i| i.index).collect(),
        single_edition: app.single_edition,
    };
    app.output.single_edition = app.single_edition;
    let host = app.host.clone();
    app.start(Kind::Prepare, move |job| {
        wip_engine::prepare(&req, &host, job).map(|(ws, inv)| Outcome::Prepared(Box::new(ws), Box::new(inv)))
    });
}

pub fn need_prepare(app: &mut App, ctx: &egui::Context) {
    if let Nav::Back = footer(ctx, "", true, None) {
        app.goto(Step::Edition);
    }
    central(ctx, |ui| {
        header(
            app,
            ui,
            Step::Customize,
            "Open an edition first",
            "Choose an edition and select Open edition. WinISOPilot then shows exactly what's inside it.",
        );
        if theme::primary(ui, "Go to Edition", true).clicked() {
            app.goto(Step::Edition);
        }
    });
}

// ---------------------------------------------------------------- Review

pub fn review(app: &mut App, ctx: &egui::Context) {
    let issues = app.plan_ctx().map(|c| plan::validate(&app.selection, &c)).unwrap_or_default();
    let acks_needed = plan::needs_ack(&app.selection, &app.catalog);
    let all_acked = acks_needed.iter().all(|(id, _)| app.acks.contains(id));
    let has_ws = app.workspace.is_some();
    let out_ok = app.iso_out.is_some() || app.host.oscdimg.is_none();
    let can_build = has_ws && !plan::has_errors(&issues) && all_acked && out_ok && !app.busy();
    let status = format!("{} changes", app.selection.len());
    match footer(ctx, &status, true, Some(("Build", can_build))) {
        Nav::Back => app.goto(Step::Customize),
        Nav::Next => start_build(app),
        Nav::None => {}
    }
    central(ctx, |ui| {
        header(
            app,
            ui,
            Step::Review,
            "Review your changes",
            "Nothing has been changed yet. Remove anything you're unsure about, then build.",
        );
        if !has_ws {
            theme::warn_box(ui, "No edition is open", &["Open an edition on the Edition step before building.".into()]);
        }
        ui.horizontal(|ui| {
            if theme::secondary(ui, "Undo", app.history.can_undo()).on_hover_text("Ctrl+Z").clicked() {
                app.undo();
            }
            if theme::secondary(ui, "Save choices…", !app.selection.is_empty()).clicked() {
                save_recipe(app);
            }
            if theme::secondary(ui, "Load choices…", !app.busy()).clicked() {
                load_recipe(app);
            }
        });
        ui.add_space(6.0);

        if app.selection.is_empty() {
            body(ui, "You haven't chosen any changes. The new ISO will match the original edition.");
        }
        let mut remove: Option<String> = None;
        for c in Category::ALL {
            let ids: Vec<String> = app
                .selection
                .iter()
                .filter(|id| app.catalog.get(id).is_some_and(|o| o.category == c))
                .cloned()
                .collect();
            if ids.is_empty() {
                continue;
            }
            eyebrow(ui, c.label());
            for id in ids {
                let o = app.catalog.get(&id).expect("filtered");
                ui.horizontal(|ui| {
                    theme::badge(ui, Some(o.risk));
                    ui.label(RichText::new(&o.title).size(14.5));
                    if ui.small_button("✖").on_hover_text("Remove from changes").clicked() {
                        remove = Some(id.clone());
                    }
                });
            }
            ui.add_space(6.0);
        }
        let raw: Vec<String> = app.selection.iter().filter(|id| wip_core::raw::is_raw(id)).cloned().collect();
        if !raw.is_empty() {
            eyebrow(ui, "Unrated (Advanced)");
            for id in raw {
                ui.horizontal(|ui| {
                    theme::badge(ui, None);
                    ui.label(RichText::new(wip_core::raw::title(&id)).size(14.5));
                    if ui.small_button("✖").on_hover_text("Remove from changes").clicked() {
                        remove = Some(id.clone());
                    }
                });
            }
        }
        if let Some(id) = remove {
            app.set_selected(&id, false);
        }

        let errors: Vec<_> = issues.iter().filter(|i| i.severity == Severity::Error).collect();
        let skipped: Vec<String> = issues
            .iter()
            .filter(|i| i.severity == Severity::Warning && i.message.contains("skipped"))
            .map(|i| i.message.clone())
            .collect();
        for e in errors {
            theme::error_box(ui, "Needs fixing", &e.message);
        }
        if !skipped.is_empty() {
            ui.add_space(6.0);
            theme::warn_box(ui, "These will be skipped", &skipped);
        }
        if !acks_needed.is_empty() {
            ui.add_space(10.0);
            theme::warn_box(
                ui,
                "Confirm risky changes",
                &["These can break things other people rely on. Tick each one to confirm.".into()],
            );
            for (id, title) in acks_needed {
                let mut on = app.acks.contains(&id);
                if ui.checkbox(&mut on, format!("I understand the risks of: {title}")).changed() {
                    if on {
                        app.acks.insert(id);
                    } else {
                        app.acks.remove(&id);
                    }
                }
            }
        }

        what_happens(app, ui);
        ui.add_space(12.0);
        theme::card().show(ui, |ui| {
            ui.set_width(ui.available_width());
            h2(ui, "Output");
            let single = app.workspace.as_ref().map_or(app.single_edition, |_| app.output.single_edition);
            dim(
                ui,
                if single {
                    "Editions: only the customized edition"
                } else {
                    "Editions: all editions (only one customized)"
                },
            );
            ui.horizontal(|ui| {
                ui.label("Compression:");
                ui.radio_value(&mut app.output.compression, Compression::Maximum, "Standard (install.wim)");
                ui.radio_value(&mut app.output.compression, Compression::Recovery, "Smallest (install.esd, slower)");
            });
            ui.horizontal(|ui| {
                ui.label("Disc label:");
                let r = ui
                    .add(egui::TextEdit::singleline(&mut app.output.volume_label).desired_width(240.0).char_limit(32));
                if r.lost_focus() {
                    app.output.volume_label = sanitize_label(&app.output.volume_label);
                }
            });
            if app.host.oscdimg.is_some() {
                ui.horizontal(|ui| {
                    ui.label("Save ISO as:");
                    let shown = app.iso_out.as_ref().map_or("Not chosen".to_string(), |p| p.display().to_string());
                    ui.label(RichText::new(shown).monospace().size(13.0));
                    if theme::secondary(ui, "Choose…", !app.busy()).clicked() {
                        choose_output(app);
                    }
                });
            } else {
                dim(ui, "No ISO tool found: the finished files will be left in the working folder's media folder.");
            }
        });
        show_error(app, ui);
    });
}

fn choose_output(app: &mut App) {
    let name = app
        .edition()
        .map(|e| format!("{}-custom.iso", e.name.replace(' ', "-")))
        .unwrap_or_else(|| "Windows-custom.iso".into());
    let mut d =
        rfd::FileDialog::new().add_filter("Disc image", &["iso"]).set_file_name(name).set_title("Save the new ISO as");
    if let Some(dir) = &app.workspace_parent {
        d = d.set_directory(dir);
    }
    if let Some(mut p) = d.save_file() {
        if p.extension().is_none() {
            p.set_extension("iso");
        }
        if p.exists() {
            app.notify("That file already exists. Choose a new name; WinISOPilot never overwrites files.");
        } else {
            app.iso_out = Some(p);
        }
    }
}

fn save_recipe(app: &mut App) {
    if let Some(p) = rfd::FileDialog::new()
        .add_filter("WinISOPilot choices", &["json"])
        .set_file_name("winisopilot-choices.json")
        .save_file()
    {
        let recipe = match crate::extras::fingerprint_ctx(app) {
            Some((ctx, edition_id)) => Recipe::with_fingerprint(&app.selection, &app.output, &ctx, &edition_id),
            None => Recipe::new(&app.selection, &app.output),
        };
        let json = recipe.to_json();
        match wip_core::fsutil::atomic_write(&p, json.as_bytes()) {
            Ok(()) => app.notify(format!("Saved {} choices.", app.selection.len())),
            Err(e) => app.notify(format!("Couldn't save: {e}")),
        }
    }
}

pub fn load_recipe(app: &mut App) {
    let Some(p) = rfd::FileDialog::new().add_filter("WinISOPilot choices", &["json"]).pick_file() else { return };
    let bytes = match std::fs::read(&p) {
        Ok(b) => b,
        Err(e) => return app.notify(format!("Couldn't open: {e}")),
    };
    match Recipe::parse(&bytes, &app.catalog) {
        Ok(r) => {
            let n = r.selection.len();
            app.replace_selection(r.selection.clone());
            app.output.compression = r.output.compression;
            app.output.volume_label = r.output.volume_label.clone();
            app.extras.pending_recipe = Some(r.clone());
            crate::extras::check_pending_recipe(app);
            let extra = if r.unknown.is_empty() {
                String::new()
            } else {
                format!(" {} unknown entries were ignored.", r.unknown.len())
            };
            app.notify(format!("Loaded {n} choices. Undo restores your previous choices.{extra}"));
        }
        Err(e) => app.notify(e),
    }
}

fn start_build(app: &mut App) {
    let Some(ctx) = app.plan_ctx() else { return };
    let compiled = match plan::compile(&app.selection, &ctx) {
        Ok(p) => p,
        Err(e) => return app.notify(e),
    };
    let Some(ws) = app.workspace.clone() else { return };
    let Some(before) = app.inventory.clone() else { return };
    app.extras.snapshot = Some(crate::extras::BuildSnapshot {
        plan: compiled.clone(),
        inventory: before.clone(),
        edition: ws.edition.clone(),
    });
    app.extras.boot_report = None;
    let out = app.iso_out.clone().unwrap_or_else(|| ws.root.join("unused.iso"));
    let output = app.output.clone();
    let host = app.host.clone();
    for (id, why) in &compiled.skipped {
        app.log_line(format!("Skipping {id}: {why}"));
    }
    app.report = None;
    app.goto(Step::Build);
    app.start(Kind::Build, move |job| {
        wip_engine::build(&ws, &compiled, &output, &out, &host, &before, job).map(|r| Outcome::Built(Box::new(r)))
    });
}

// ---------------------------------------------------------------- Build

pub fn build(app: &mut App, ctx: &egui::Context) {
    let finished = app.report.is_some();
    let next = if finished { Some(("Start another", !app.busy())) } else { None };
    match footer(ctx, "", !app.busy() && !finished, next) {
        Nav::Back => app.goto(Step::Review),
        Nav::Next => {
            app.report = None;
            app.iso_out = None;
            app.acks.clear();
            app.max_step = Step::Edition;
            app.goto(Step::Edition);
        }
        Nav::None => {}
    }
    central(ctx, |ui| {
        if app.busy() {
            let testing = app.job.as_ref().is_some_and(|j| j.kind == Kind::BootTest);
            let (title, blurb) = if testing {
                (
                    "Testing in a virtual machine",
                    "Windows is installing in a temporary VM. This can take 15–40 minutes.",
                )
            } else {
                ("Building your Windows", "This can take 10–40 minutes. You can keep using your PC.")
            };
            header(app, ui, Step::Build, title, blurb);
            job_panel(app, ui);
            return;
        }
        if let Some(r) = app.report.clone() {
            header(app, ui, Step::Build, "Your Windows is ready", "The original ISO was not changed.");
            theme::card().show(ui, |ui| {
                ui.set_width(ui.available_width());
                match &r.iso {
                    Some(p) => {
                        check_line(ui, true, &format!("New ISO: {}", p.display()));
                        if let Some(b) = r.iso_bytes {
                            dim(ui, &format!("Size: {}", human_bytes(b)));
                        }
                        if theme::secondary(ui, "Show in folder", true).clicked() {
                            open_folder(p.parent().map(PathBuf::from).unwrap_or_default());
                        }
                    }
                    None => {
                        check_line(ui, true, &format!("Finished files: {}", r.media_folder.display()));
                        if theme::secondary(ui, "Open folder", true).clicked() {
                            open_folder(r.media_folder.clone());
                        }
                    }
                }
                dim(ui, &format!("{} operations applied. Log: {}", r.applied, r.log_file.display()));
                if let Some(root) = r.media_folder.parent() {
                    dim(ui, &format!("You can delete the working folder when you're done: {}", root.display()));
                }
            });
            if !r.warnings.is_empty() {
                ui.add_space(8.0);
                theme::warn_box(ui, "Notes", &r.warnings);
            }
            crate::extras::footprint_card(ui, &r);
            crate::extras::boot_test_card(app, ui, &r);
            ui.add_space(8.0);
            if theme::primary(ui, "Write it to a USB drive", true).clicked() {
                app.usb.source = None;
                app.usb.report = None;
                app.switch_tool(crate::app::Tool::Usb);
            }
            if theme::secondary(ui, "Save these choices for next time…", true).clicked() {
                save_recipe(app);
            }
            log_tail(app, ui, 200);
            return;
        }
        header(app, ui, Step::Build, "The build stopped", "");
        show_error(app, ui);
        if app.workspace.is_some() {
            body(ui, "The edition is still open. Discard it to start again from a clean copy; your choices are kept.");
            if theme::primary(ui, "Discard and start again", !app.busy()).clicked() {
                app.discard_workspace();
            }
        }
        log_tail(app, ui, 200);
    });
}

fn open_folder(p: PathBuf) {
    #[cfg(windows)]
    {
        let _ = std::process::Command::new("explorer.exe").arg(p).spawn();
    }
    #[cfg(not(windows))]
    {
        let _ = p;
    }
}

/// Plain-language summary of what Build will do, from the compiled plan.
fn what_happens(app: &App, ui: &mut Ui) {
    let Some(ctx) = app.plan_ctx().filter(|c| c.inventory.is_some()) else { return };
    let Ok(p) = plan::compile(&app.selection, &ctx) else { return };
    use wip_core::plan::Op;
    let count = |f: &dyn Fn(&Op) -> bool| p.install.iter().chain(p.boot.iter()).filter(|o| f(&o.op)).count();
    let apps = count(&|o| matches!(o, Op::RemoveAppx { .. }));
    let caps = count(&|o| matches!(o, Op::RemoveCapability { .. }));
    let feats = count(&|o| matches!(o, Op::DisableFeature { .. } | Op::EnableFeature { .. }));
    let regs = count(&|o| matches!(o, Op::RegSet { .. } | Op::RegDelete { .. }));
    let files = count(&|o| matches!(o, Op::DeleteFile { .. }));
    let cleanup = count(&|o| matches!(o, Op::ComponentCleanup { .. }));
    ui.add_space(12.0);
    theme::card().show(ui, |ui| {
        ui.set_width(ui.available_width());
        h2(ui, "What happens when you select Build");
        let mut lines = Vec::new();
        if apps + caps + feats > 0 {
            lines.push(format!("1. Remove {apps} app packages and {caps} capabilities, and switch {feats} features"));
        } else {
            lines.push("1. No apps, capabilities or features change".to_string());
        }
        lines.push(format!(
            "2. Write {regs} registry and service settings{}",
            if files > 0 { " and delete WinRE" } else { "" }
        ));
        if cleanup > 0 {
            lines.push("3. Clean up the component store (the slowest step)".into());
        }
        lines.push(format!(
            "{}. Save the edition and rebuild the installation image{}",
            lines.len() + 1,
            if p.boot.is_empty() { "" } else { ", then change Windows Setup" }
        ));
        let out = match (&app.iso_out, &app.host.oscdimg) {
            (Some(o), Some(_)) => format!("write the ISO to {}", o.display()),
            (None, Some(_)) => "write the ISO (choose where below)".into(),
            _ => "leave the finished files in the working folder".into(),
        };
        lines.push(format!("{}. Then {out}", lines.len() + 1));
        for l in lines {
            body(ui, &l);
        }
        if !p.skipped.is_empty() {
            dim(ui, &format!("{} chosen options have nothing to do in this image and are skipped.", p.skipped.len()));
        }
        dim(ui, "Your original ISO is never changed. Cancel stops after the current step without saving.");
    });
}
