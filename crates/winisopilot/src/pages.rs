//! Wizard pages other than Customize.

use crate::app::{App, Step};
use crate::jobs::{Kind, Outcome};
use crate::theme::{self, body, check_line, dim, eyebrow, h1, h2};
use eframe::egui::{self, Frame, Margin, RichText, Ui};
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
        .exact_height(38.0)
        .frame(Frame::new().fill(theme::CHROME).inner_margin(Margin::symmetric(10, 5)))
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

/// Page body. The window has a fixed size and every page is laid out to fit
/// it, so there is no scrolling anywhere.
pub fn central(ctx: &egui::Context, add: impl FnOnce(&mut Ui)) {
    egui::CentralPanel::default()
        .frame(Frame::new().fill(theme::PAGE).inner_margin(Margin { left: 10, right: 10, top: 8, bottom: 8 }))
        .show(ctx, |ui| {
            add(ui);
        });
}

/// Two equal columns with a 16 px gap.
/// `add` is called with 0 for the left column and 1 for the right.
pub fn two_columns(ui: &mut Ui, mut add: impl FnMut(usize, &mut Ui)) {
    ui.spacing_mut().item_spacing.x = 8.0;
    ui.columns(2, |c| {
        for (i, col) in c.iter_mut().enumerate() {
            col.spacing_mut().item_spacing.x = 6.0;
            // Plain left-aligned text (columns default to justified).
            col.with_layout(egui::Layout::top_down(egui::Align::LEFT), |ui| add(i, ui));
        }
    });
    ui.spacing_mut().item_spacing.x = 6.0;
}

fn header(app: &App, ui: &mut Ui, step: Step, title: &str, blurb: &str) {
    eyebrow(ui, &format!("Step {} of 6", step.number()));
    h1(ui, title);
    if !blurb.is_empty() {
        body(ui, blurb);
    }
    crate::help::tip(app, ui);
    ui.add_space(4.0);
}

/// Progress, log tail and Cancel for the running job.
pub fn job_panel(app: &mut App, ui: &mut Ui) {
    let reduce_motion = app.settings.reduce_motion;
    let Some(job) = app.job.as_mut() else { return };
    theme::card().show(ui, |ui| {
        theme::card_body(ui);
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
    log_tail(app, ui, 6);
}

/// The last few log lines (no scrolling; the full log is in the log folder).
pub fn log_tail(app: &App, ui: &mut Ui, lines: usize) {
    if app.log.is_empty() {
        return;
    }
    // Advanced users see the log straight away; Guided keeps it folded.
    egui::CollapsingHeader::new("Details").default_open(!guided(app)).show(ui, |ui| {
        let start = app.log.len().saturating_sub(lines);
        for l in &app.log[start..] {
            ui.add(egui::Label::new(RichText::new(l).monospace().size(12.0).color(theme::TEXT_DIM)).truncate())
                .on_hover_text(l);
        }
        if ui.link("Open log folder").clicked() {
            open_folder(wip_engine::app_data_dir().join("logs"));
        }
    });
}

fn show_error(app: &App, ui: &mut Ui) {
    if let Some(e) = &app.last_error {
        if e.is_cancelled() {
            theme::warn_box(ui, "Stopped", std::slice::from_ref(&e.message));
        } else {
            theme::error_box(ui, &format!("Something went wrong ({})", e.code), &e.message);
            ui.horizontal(|ui| {
                if ui.button("Copy details").on_hover_text("Copy the error and the last 40 log lines").clicked() {
                    let tail = app.log[app.log.len().saturating_sub(40)..].join("\n");
                    ui.ctx().copy_text(format!(
                        "WinISOPilot {} {}: {}\n\nLog:\n{tail}",
                        wip_core::APP_VERSION,
                        e.code,
                        e.message
                    ));
                }
                if ui.button("Open log folder").clicked() {
                    open_folder(wip_engine::app_data_dir().join("logs"));
                }
            });
            if app.job.is_none() {
                log_tail(app, ui, 6);
            }
        }
        ui.add_space(4.0);
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
    ui.add_space(4.0);
    true
}

// ---------------------------------------------------------------- Welcome

/// Height kept free under a column's last card for the progress panel,
/// an error box and the Details line.
pub fn tail_reserve(app: &App) -> f32 {
    let mut r = 0.0;
    if app.busy() {
        r += 200.0;
    }
    if app.last_error.is_some() {
        r += 150.0;
    }
    if !app.leftovers.is_empty() {
        r += 90.0;
    }
    r
}

/// Guided = plain language, fewest choices. Advanced = every detail.
pub fn guided(app: &App) -> bool {
    app.mode() == wip_core::settings::Mode::Guided
}

pub fn welcome(app: &mut App, ctx: &egui::Context) {
    let ready = app.host.is_admin && app.host.dism_api.is_some();
    let status = match (ready, app.host.oscdimg.is_some()) {
        (true, true) => "This PC is ready.",
        (true, false) if guided(app) => "Ready, but one optional tool is missing (see Before you start).",
        (true, false) => "Ready; oscdimg missing (output will be a media folder).",
        _ => "Some checks need attention.",
    };
    let g = guided(app);
    // Guided has its own Start button on the page; only Advanced needs one in the footer.
    let next = if g { None } else { Some(("Get started", !app.busy())) };
    if let Nav::Next = footer(ctx, status, false, next) {
        app.goto(Step::Source);
    }
    central(ctx, |ui| {
        if g {
            header(
                app,
                ui,
                Step::Welcome,
                "Let's make your own Windows installer",
                "Answer a few simple questions. Nothing on this PC, and none of your files, is changed.",
            );
            two_columns(ui, |i, ui| {
                let (title, text, button, steps): (_, _, _, &[&str]) = if i == 0 {
                    (
                        "Make a cleaner Windows",
                        "Remove apps, ads and extras you don't want, then put the result on a USB stick to install.",
                        "Start",
                        &[
                            "Choose a Windows file, or let WinISOPilot download it",
                            "Pick your Windows (Home, Pro…)",
                            "Tick what you don't want; each item says what it does",
                            "Check the list, then make your new Windows",
                            "Put it on a USB stick and install",
                        ],
                    )
                } else {
                    (
                        "Just make a USB stick",
                        "Put Windows on a USB stick exactly as Microsoft ships it, so you can install it on a PC.",
                        "Make a USB stick",
                        &[
                            "Choose the Windows file (.iso)",
                            "Plug in a USB stick of 8 GB or more",
                            "Pick it and type ERASE to confirm",
                            "Wait for the copy and the check to finish",
                        ],
                    )
                };
                theme::card().show(ui, |ui| {
                    theme::card_body(ui);
                    ui.set_min_height(150.0);
                    h2(ui, title);
                    body(ui, text);
                    ui.spacing_mut().item_spacing.y = 0.0;
                    for (n, st) in steps.iter().enumerate() {
                        ui.label(RichText::new(format!("{}.  {st}", n + 1)).size(12.0).color(theme::TEXT_DIM));
                    }
                    ui.spacing_mut().item_spacing.y = 4.0;
                    ui.add_space(3.0);
                    let clicked = if i == 0 {
                        theme::primary(ui, button, !app.busy()).clicked()
                    } else {
                        theme::secondary(ui, button, !app.busy()).clicked()
                    };
                    if clicked && i == 0 {
                        app.goto(Step::Source);
                    } else if clicked {
                        app.switch_tool(crate::app::Tool::Usb);
                    }
                });
            });
            ui.add_space(4.0);
            theme::fill_next(ui, tail_reserve(app));
            simple_ready(app, ui);
        } else {
            header(
                app,
                ui,
                Step::Welcome,
                "Host diagnostics",
                "Offline servicing of install.wim / install.esd via the DISM API, reg.exe hive edits and oscdimg. The source ISO is mounted read-only.",
            );
            let reserve = tail_reserve(app);
            two_columns(ui, |i, ui| {
                theme::fill_next(ui, reserve);
                match i {
                    0 => diagnostics(app, ui),
                    _ => shortcuts_card(ui),
                }
            });
        }
        if !app.leftovers.is_empty() {
            ui.add_space(4.0);
            leftovers(app, ui);
        }
        ui.add_space(4.0);
        job_panel(app, ui);
        show_error(app, ui);
    });
}

/// Offer to install the ADK Deployment Tools (oscdimg) for the user.
pub fn adk_offer(app: &mut App, ui: &mut Ui) {
    if app.host.oscdimg.is_some() {
        return;
    }
    let g = guided(app);
    ui.horizontal_wrapped(|ui| {
        let label = if g { "Install it for me" } else { "Install Deployment Tools (silent)" };
        if theme::primary(ui, label, !app.busy() && app.host.is_admin)
            .on_hover_text("Downloads Microsoft's ADK installer, checks Microsoft's signature, installs only the Deployment Tools (about 100 MB).")
            .clicked()
        {
            app.start_adk_install();
        }
        if ui.button(if g { "I installed it, check again" } else { "Re-probe" }).clicked() {
            app.recheck_host();
            if app.host.oscdimg.is_none() {
                app.notify("Still not found. If you just installed it, wait for the installer to finish.");
            }
        }
        ui.hyperlink_to(
            if g { "or install it yourself" } else { "Manual download" },
            "https://learn.microsoft.com/windows-hardware/get-started/adk-install",
        );
    });
}

/// Guided: one plain line when everything is fine, otherwise what to do.
fn simple_ready(app: &mut App, ui: &mut Ui) {
    let (admin, dism, iso_tool) = (app.host.is_admin, app.host.dism_api.is_some(), app.host.oscdimg.is_some());
    theme::card().show(ui, |ui| {
        theme::card_body(ui);
        if admin && dism && iso_tool {
            check_line(ui, true, "This PC has everything needed.");
            return;
        }
        h2(ui, "Before you start");
        if !admin {
            check_line(
                ui,
                false,
                "WinISOPilot needs permission to change Windows files. Close it, right-click it and choose \"Run as administrator\".",
            );
        }
        if !dism {
            check_line(ui, false, "A part of Windows that WinISOPilot needs (DISM) wasn't found. Installing the tool below adds it.");
        }
        if !iso_tool {
            check_line(
                ui,
                false,
                "One free Microsoft tool is missing (Windows ADK \"Deployment Tools\"). It's needed to make a single ISO file and to download Windows.",
            );
            adk_offer(app, ui);
        }
    });
}

/// Advanced: everything WinISOPilot detected about the host.
fn diagnostics(app: &mut App, ui: &mut Ui) {
    let h = app.host.clone();
    let h = &h;
    theme::card().show(ui, |ui| {
        theme::card_body(ui);
        h2(ui, "Host");
        let opt = |p: &Option<PathBuf>| p.as_ref().map_or("not found".to_string(), |p| p.display().to_string());
        let rows = [
            ("Elevated", if h.is_admin { "yes".to_string() } else { "NO — restart as administrator".into() }),
            ("OS build", if h.os_build > 0 { h.os_build.to_string() } else { "unknown".into() }),
            ("dismapi.dll", opt(&h.dism_api)),
            ("DISM version", if h.dism_version.is_empty() { "—".into() } else { h.dism_version.clone() }),
            ("dism.exe", opt(&h.dism_exe)),
            ("oscdimg.exe", opt(&h.oscdimg)),
            (
                "ADK DISM override",
                if app.settings.dism_folder.is_empty() { "none".into() } else { app.settings.dism_folder.clone() },
            ),
            ("App data", wip_engine::app_data_dir().display().to_string()),
        ];
        egui::Grid::new("diag").num_columns(2).spacing([12.0, 1.0]).striped(true).show(ui, |ui| {
            for (k, v) in rows {
                ui.label(RichText::new(k).size(12.0).color(theme::TEXT_DIM));
                ui.add(egui::Label::new(RichText::new(&v).monospace().size(11.5)).truncate()).on_hover_text(&v);
                ui.end_row();
            }
        });
        for n in &h.notes {
            ui.label(RichText::new(n).size(11.5).color(theme::WARN_TEXT));
        }
        if h.dism_api.is_none() || h.oscdimg.is_none() {
            ui.hyperlink_to(
                "Windows ADK Deployment Tools",
                "https://learn.microsoft.com/windows-hardware/get-started/adk-install",
            );
        }
    });
}

fn shortcuts_card(ui: &mut Ui) {
    theme::card().show(ui, |ui| {
        theme::card_body(ui);
        h2(ui, "Keyboard");
        egui::Grid::new("keys-welcome").num_columns(2).spacing([12.0, 1.0]).show(ui, |ui| {
            for (k, what) in crate::help::SHORTCUTS {
                ui.label(RichText::new(*k).monospace().size(11.5));
                ui.label(RichText::new(*what).size(12.0).color(theme::TEXT_BODY));
                ui.end_row();
            }
        });
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
            ui.label(RichText::new(&rec.mount_dir).monospace().size(11.5));
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
    let g = guided(app);
    if g && app.workspace_parent.is_none() {
        // Beginners get a sensible place for temporary files without asking.
        if let Some(home) = std::env::var_os("USERPROFILE") {
            let d = PathBuf::from(home).join("Downloads");
            if d.is_dir() {
                app.workspace_parent = Some(d);
            }
        }
    }
    central(ctx, |ui| {
        if g {
            header(
                app,
                ui,
                Step::Source,
                "Where is Windows coming from?",
                "You need a Windows installation file (an \"ISO\"). Use one you already have, or let WinISOPilot download it for you.",
            );
        } else {
            header(
                app,
                ui,
                Step::Source,
                "Source ISO and workspace",
                "Retail/MCT Windows 10 or 11 media with sources\\install.wim or install.esd. Mounted read-only.",
            );
        }
        let locked = workspace_banner(app, ui) || app.busy();
        two_columns(ui, |i, ui| {
            if i == 0 {
                iso_card(app, ui, locked);
                ui.add_space(4.0);
                theme::fill_next(ui, tail_reserve(app));
                working_folder_card(app, ui, locked);
                ui.add_space(4.0);
                job_panel(app, ui);
                show_error(app, ui);
            } else {
                theme::fill_next(ui, 0.0);
                crate::download_ui::card(app, ui, locked);
            }
        });
    });
}

fn set_iso(app: &mut App, p: PathBuf) {
    if app.workspace_parent.is_none() || guided(app) {
        if let Some(d) = p.parent().filter(|d| !d.as_os_str().is_empty()) {
            app.workspace_parent = Some(d.to_path_buf());
        }
    }
    app.iso_path = Some(p);
    app.source = None;
    app.chosen_index = None;
}

fn pick_iso(app: &mut App) {
    if let Some(p) =
        rfd::FileDialog::new().add_filter("Disc image", &["iso"]).set_title("Choose a Windows ISO").pick_file()
    {
        set_iso(app, p);
    }
}

fn iso_card(app: &mut App, ui: &mut Ui, locked: bool) {
    let g = guided(app);
    theme::card().show(ui, |ui| {
        theme::card_body(ui);
        if g {
            h2(ui, "I already have a Windows ISO file");
            match app.iso_path.as_ref().and_then(|p| p.file_name()) {
                Some(n) => check_line(ui, true, &format!("Chosen: {}", n.to_string_lossy())),
                None => dim(ui, "It's a file whose name ends in .iso, often in your Downloads folder."),
            }
            let label = if app.iso_path.is_some() { "Choose a different file…" } else { "Choose the ISO file…" };
            if theme::primary(ui, label, !locked).clicked() {
                pick_iso(app);
            }
            return;
        }
        h2(ui, "ISO");
        let mut text = app.iso_path.as_ref().map(|p| p.display().to_string()).unwrap_or_default();
        ui.horizontal(|ui| {
            let r = ui.add_enabled(
                !locked,
                egui::TextEdit::singleline(&mut text)
                    .hint_text("Path to .iso (paste or type)")
                    .desired_width(ui.available_width() - 112.0)
                    .font(egui::TextStyle::Monospace),
            );
            if r.lost_focus() {
                let t = text.trim().trim_matches('"');
                if t.is_empty() {
                    app.iso_path = None;
                } else if Some(PathBuf::from(t)) != app.iso_path {
                    set_iso(app, PathBuf::from(t));
                }
            }
            if theme::secondary(ui, "Browse…", !locked).clicked() {
                pick_iso(app);
            }
        });
        if let Some(p) = &app.iso_path {
            match std::fs::metadata(p) {
                Ok(m) => dim(ui, &format!("{} bytes ({})", m.len(), human_bytes(m.len()))),
                Err(e) => {
                    ui.label(RichText::new(format!("Not readable: {e}")).color(theme::ERR_TEXT));
                }
            }
        }
        ui.horizontal_wrapped(|ui| {
            ui.hyperlink_to("Windows 11 ISO page", "https://www.microsoft.com/software-download/windows11");
            ui.hyperlink_to("Windows 10 ISO page", "https://www.microsoft.com/software-download/windows10ISO");
        });
    });
}

fn working_folder_card(app: &mut App, ui: &mut Ui, locked: bool) {
    let g = guided(app);
    theme::card().show(ui, |ui| {
        theme::card_body(ui);
        if g {
            h2(ui, "Where to keep temporary files");
            let shown = app.workspace_parent.as_ref().map_or("Not chosen yet".to_string(), |p| p.display().to_string());
            path_label(ui, &shown);
            ui.horizontal(|ui| {
                if theme::secondary(ui, "Change…", !locked).clicked() {
                    if let Some(p) = rfd::FileDialog::new().set_title("Where to keep temporary files").pick_folder() {
                        app.workspace_parent = Some(p);
                    }
                }
                space_line(app, ui);
            });
            return;
        }
        h2(ui, "Workspace parent");
        dim(
            ui,
            "Holds the extracted media, mount directory and output. Local NTFS, not a network or FAT32/exFAT drive.",
        );
        let mut text = app.workspace_parent.as_ref().map(|p| p.display().to_string()).unwrap_or_default();
        ui.horizontal(|ui| {
            let r = ui.add_enabled(
                !locked,
                egui::TextEdit::singleline(&mut text)
                    .hint_text("Folder path")
                    .desired_width(ui.available_width() - 112.0)
                    .font(egui::TextStyle::Monospace),
            );
            if r.lost_focus() {
                let t = text.trim().trim_matches('"');
                app.workspace_parent = (!t.is_empty()).then(|| PathBuf::from(t));
            }
            if theme::secondary(ui, "Browse…", !locked).clicked() {
                if let Some(p) = rfd::FileDialog::new().set_title("Workspace parent").pick_folder() {
                    app.workspace_parent = Some(p);
                }
            }
        });
        space_line(app, ui);
    });
}

/// A file path on one line, shortened with "…" when it doesn't fit (full path on hover).
pub fn path_label(ui: &mut Ui, text: &str) {
    ui.add(egui::Label::new(RichText::new(text).monospace().size(12.0)).truncate()).on_hover_text(text);
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
    let g = guided(app);
    let label = match (ready, g) {
        (true, _) => "Next",
        (false, true) => "Continue",
        (false, false) => "Open edition",
    };
    let status = if g && !ready { "The next step takes a few minutes." } else { "" };
    match footer(ctx, status, true, Some((label, supported && !app.busy()))) {
        Nav::Back => app.goto(Step::Source),
        Nav::Next if ready => app.goto(Step::Customize),
        Nav::Next => start_prepare(app),
        Nav::None => {}
    }
    central(ctx, |ui| {
        if g {
            header(
                app,
                ui,
                Step::Edition,
                "Which Windows do you want?",
                "Pick the one you're allowed to install: it's shown in Settings > System > About on your current PC, or on its sticker. Most people have Home or Pro.",
            );
        } else {
            header(
                app,
                ui,
                Step::Edition,
                "Image index",
                "Select the index to service. Open edition exports it, mounts it read-write and inventories packages, capabilities and features.",
            );
        }
        let locked = workspace_banner(app, ui) || app.busy();
        let Some(src) = app.source.clone() else {
            body(ui, "Read an ISO first.");
            return;
        };
        two_columns(ui, |col, ui| {
            if col == 0 {
                edition_list(app, ui, &src, locked, g);
                job_panel(app, ui);
                show_error(app, ui);
            } else {
                theme::fill_next(ui, 0.0);
                edition_side(app, ui, &src, locked, g);
            }
        });
    });
}

/// One compact row per edition; paged if an ISO holds more than fit.
fn edition_list(app: &mut App, ui: &mut Ui, src: &wip_engine::SourceInfo, locked: bool, g: bool) {
    const ROW: f32 = 30.0;
    let page = crate::pager::layout(ui, "editions", src.images.len(), ROW, tail_reserve(app));
    let list = ui.scope(|ui| {
        ui.spacing_mut().item_spacing.y = 4.0;
        for img in &src.images[page.range.clone()] {
            let support = img.support();
            let selected = app.chosen_index == Some(img.index);
            let frame = theme::card().inner_margin(Margin::symmetric(8, 2)).stroke(egui::Stroke::new(
                if selected { 2.0_f32 } else { 1.0 },
                if selected { theme::ACCENT } else { theme::DIVIDER },
            ));
            let enabled = support.is_ok() && !locked;
            let r = frame.show(ui, |ui| {
                ui.set_width(ui.available_width());
                ui.set_height(ROW - 8.0);
                ui.horizontal_centered(|ui| {
                    let rb = ui.add_enabled(enabled, egui::RadioButton::new(selected, ""));
                    let name = RichText::new(&img.name).size(13.0).strong();
                    let l = ui.add(egui::Label::new(name).sense(egui::Sense::click()));
                    let facts = match (&support, g) {
                        (Ok(_), true) if img.edition_id.eq_ignore_ascii_case("Professional") => {
                            "Recommended if you're not sure".to_string()
                        }
                        (Ok(_), true) => String::new(),
                        (Ok(_), false) => format!("#{} · {}", img.index, img.edition_id),
                        (Err(why), _) => why.to_string(),
                    };
                    let color = if support.is_ok() { theme::TEXT_DIM } else { theme::ERR_TEXT };
                    ui.add(egui::Label::new(RichText::new(&facts).size(12.0).color(color)).truncate())
                        .on_hover_text(&facts);
                    rb.clicked() || (enabled && l.clicked())
                })
                .inner
            });
            let clicked = r.inner || (enabled && r.response.interact(egui::Sense::click()).clicked());
            if clicked && !selected && enabled {
                app.chosen_index = Some(img.index);
            }
        }
    });
    page.controls(ui, list.response.rect);
}

/// Right column of the Edition page: plain advice (Guided) or the selected
/// index's metadata (Advanced), plus the output choice and space check.
fn edition_side(app: &mut App, ui: &mut Ui, src: &wip_engine::SourceInfo, locked: bool, g: bool) {
    theme::card().show(ui, |ui| {
        theme::card_body(ui);
        if g {
            // Beginners always get a single-edition ISO: smaller, and Setup asks one question less.
            app.single_edition = true;
            h2(ui, "Home, Pro or Education?");
            for (name, text) in [
                ("Home", "For most home PCs. Everything you need for everyday use."),
                ("Pro", "Home plus extras for work: BitLocker drive encryption, Remote Desktop, joining a company network."),
                ("Education", "Like Pro, for schools. Only pick it if your school gave you a key."),
                ("N editions", "The same, without Media Player and some media apps (made for Europe)."),
            ] {
                ui.label(RichText::new(name).strong().color(theme::TEXT));
                dim(ui, text);
            }
            ui.add_space(3.0);
            body(ui, "Windows will only activate with the edition your key or PC is licensed for. If unsure, pick Pro: Setup activates Home or Pro automatically on most PCs that came with Windows.");
        } else {
            h2(ui, "Selected index");
            match src.images.iter().find(|i| Some(i.index) == app.chosen_index) {
                Some(img) => {
                    let fam = img.family().map_or("Unsupported", |f| f.label());
                    let rows = [
                        ("Index", img.index.to_string()),
                        ("Name", img.name.clone()),
                        ("Description", img.description.clone()),
                        ("EditionID", img.edition_id.clone()),
                        ("Installation type", img.installation_type.clone()),
                        ("Family", fam.to_string()),
                        ("Version", img.version_string()),
                        ("Architecture", img.architecture.clone()),
                        ("Installed size", format!("{} ({} bytes)", human_bytes(img.size_bytes), img.size_bytes)),
                        ("Languages", img.languages.join(", ")),
                        ("Image file", src.image_rel.clone()),
                    ];
                    egui::Grid::new("index-meta").num_columns(2).spacing([12.0, 1.0]).striped(true).show(ui, |ui| {
                        for (k, v) in rows {
                            ui.label(RichText::new(k).size(12.0).color(theme::TEXT_DIM));
                            ui.add(egui::Label::new(RichText::new(&v).monospace().size(11.5)).truncate()).on_hover_text(&v);
                            ui.end_row();
                        }
                    });
                }
                None => dim(ui, "Select an index."),
            }
            ui.add_space(3.0);
            ui.add_enabled_ui(!locked, |ui| {
                ui.checkbox(&mut app.single_edition, "Export only this index (single-edition install image)");
            });
            dim(ui, "Untick to keep every index in the output; only the selected one is serviced.");
        }
        ui.add_space(3.0);
        space_line(app, ui);
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
    let g = guided(app);
    if g && app.iso_out.is_none() && app.host.oscdimg.is_some() {
        app.iso_out = default_output(app);
    }
    let out_ok = app.iso_out.is_some() || app.host.oscdimg.is_none();
    let can_build = has_ws && !plan::has_errors(&issues) && all_acked && out_ok && !app.busy();
    let status = format!("{} changes", app.selection.len());
    let build_label = if g { "Make my Windows" } else { "Build" };
    match footer(ctx, &status, true, Some((build_label, can_build))) {
        Nav::Back => app.goto(Step::Customize),
        Nav::Next => start_build(app),
        Nav::None => {}
    }
    central(ctx, |ui| {
        if g {
            header(
                app,
                ui,
                Step::Review,
                "Check your changes",
                "Nothing has been changed yet. Remove anything you're unsure about with ✖, then select Make my Windows.",
            );
        } else {
            header(
                app,
                ui,
                Step::Review,
                "Review and plan",
                "Selection, validation, compiled operation plan and output settings. Nothing is applied until Build.",
            );
        }
        let ack_ids: std::collections::BTreeSet<String> = acks_needed.iter().map(|(id, _)| id.clone()).collect();
        two_columns(ui, |i, ui| {
            if i == 0 {
                if !has_ws {
                    theme::warn_box(
                        ui,
                        "No edition is open",
                        &["Open one on the Edition step before building.".into()],
                    );
                    ui.add_space(4.0);
                }
                theme::fill_next(ui, 0.0);
                theme::card().show(ui, |ui| {
                    theme::card_body(ui);
                    changes_list(app, ui, &ack_ids);
                });
            } else {
                let errors: Vec<_> = issues.iter().filter(|i| i.severity == Severity::Error).collect();
                let skipped: Vec<String> = issues
                    .iter()
                    .filter(|i| i.severity == Severity::Warning && i.message.contains("skipped"))
                    .map(|i| i.message.clone())
                    .collect();
                for e in errors {
                    theme::error_box(ui, "Needs fixing", &e.message);
                    ui.add_space(4.0);
                }
                if !skipped.is_empty() {
                    theme::warn_box(ui, "These will be skipped", &skipped);
                    ui.add_space(4.0);
                }
                if g {
                    what_happens(app, ui);
                } else {
                    operation_plan(app, ui);
                }
                theme::fill_next(ui, if app.last_error.is_some() { 150.0 } else { 0.0 });
                output_card(app, ui);
                show_error(app, ui);
            }
        });
    });
}

/// Every chosen change, one row each: risk, name, a confirmation tick for
/// risky ones, and ✖ to remove. Paged so it never needs a scrollbar.
fn changes_list(app: &mut App, ui: &mut Ui, ack_ids: &std::collections::BTreeSet<String>) {
    let g = guided(app);
    ui.horizontal(|ui| {
        if theme::secondary(ui, "Undo", app.history.can_undo()).on_hover_text("Ctrl+Z").clicked() {
            app.undo();
        }
        if !g {
            if theme::secondary(ui, "Save recipe…", !app.selection.is_empty()).clicked() {
                save_recipe(app);
            }
            if theme::secondary(ui, "Load recipe…", !app.busy()).clicked() {
                load_recipe(app);
            }
        }
    });
    ui.add_space(3.0);
    if app.selection.is_empty() {
        body(ui, "You haven't chosen any changes. The new ISO will match the original edition.");
        return;
    }
    // Catalog options grouped by section, then unrated items.
    let mut items: Vec<(String, String, Option<wip_core::Risk>, &'static str)> = Vec::new();
    for c in Category::ALL {
        for id in app.selection.iter().filter(|id| app.catalog.get(id).is_some_and(|o| o.category == c)) {
            let o = app.catalog.get(id).expect("filtered");
            items.push((id.clone(), o.title.clone(), Some(o.risk), c.label()));
        }
    }
    for id in app.selection.iter().filter(|id| wip_core::raw::is_raw(id)) {
        items.push((id.clone(), wip_core::raw::title(id), None, "Unrated (Advanced)"));
    }
    if !ack_ids.is_empty() {
        let left = ack_ids.iter().filter(|id| !app.acks.contains(*id)).count();
        let text = if left == 0 {
            "Risky changes confirmed.".to_string()
        } else {
            format!("Tick Confirm on {left} risky change(s): they can break things other people rely on.")
        };
        ui.label(RichText::new(text).color(if left == 0 { theme::OK_TEXT } else { theme::WARN_TEXT }));
    }
    const ROW: f32 = 24.0;
    let page = crate::pager::layout(ui, "review", items.len(), ROW, 0.0);
    let mut remove: Option<String> = None;
    let list = ui.scope(|ui| {
        ui.spacing_mut().item_spacing.y = 0.0;
        for (id, title, risk, section) in &items[page.range.clone()] {
            let size = egui::vec2(ui.available_width(), ROW);
            ui.allocate_ui_with_layout(size, egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if ui.small_button("✖").on_hover_text("Remove from changes").clicked() {
                    remove = Some(id.clone());
                }
                if ack_ids.contains(id) {
                    let mut on = app.acks.contains(id);
                    let r = ui.checkbox(&mut on, "Confirm").on_hover_text("I understand what this could break");
                    if r.changed() {
                        if on {
                            app.acks.insert(id.clone());
                        } else {
                            app.acks.remove(id);
                        }
                    }
                }
                ui.with_layout(egui::Layout::left_to_right(egui::Align::Center), |ui| {
                    theme::badge(ui, *risk);
                    ui.add(egui::Label::new(RichText::new(title).size(13.0)).truncate())
                        .on_hover_text(format!("{section}: {title}"));
                });
            });
        }
    });
    page.controls(ui, list.response.rect);
    if let Some(id) = remove {
        app.set_selected(&id, false);
    }
}

fn output_card(app: &mut App, ui: &mut Ui) {
    let g = guided(app);
    theme::card().show(ui, |ui| {
        theme::card_body(ui);
        if g {
            h2(ui, "Where your new Windows is saved");
            if app.host.oscdimg.is_some() {
                let shown = app.iso_out.as_ref().map_or("Not chosen".to_string(), |p| p.display().to_string());
                path_label(ui, &shown);
                if theme::secondary(ui, "Change…", !app.busy()).clicked() {
                    choose_output(app);
                }
            } else {
                dim(ui, "You'll get a folder of setup files. For a single ISO file, install one free Microsoft tool:");
                adk_offer(app, ui);
            }
            return;
        }
        h2(ui, "Output");
        let single = app.workspace.as_ref().map_or(app.single_edition, |_| app.output.single_edition);
        egui::Grid::new("output").num_columns(2).spacing([12.0, 8.0]).show(ui, |ui| {
            ui.label("Indexes");
            dim(ui, if single { "Serviced index only" } else { "All indexes (one serviced)" });
            ui.end_row();
            ui.label("Compression");
            let text = |c: Compression| match c {
                Compression::Recovery => "Recovery: install.esd (smallest, slow)",
                _ => "Maximum: install.wim",
            };
            egui::ComboBox::from_id_salt("compression").selected_text(text(app.output.compression)).show_ui(ui, |ui| {
                for c in [Compression::Maximum, Compression::Recovery] {
                    ui.selectable_value(&mut app.output.compression, c, text(c));
                }
            });
            ui.end_row();
            ui.label("Volume label");
            let r =
                ui.add(egui::TextEdit::singleline(&mut app.output.volume_label).desired_width(200.0).char_limit(32));
            if r.lost_focus() {
                app.output.volume_label = sanitize_label(&app.output.volume_label);
            }
            ui.end_row();
            ui.label("Secure Boot");
            ui.checkbox(&mut app.output.boot_2023, "Use the 2023-signed boot manager").on_hover_text(
                "Microsoft's newer boot manager (Windows UEFI CA 2023), taken from this ISO's own files (April 2024 updates or later). Needed on PCs that have applied Microsoft's boot manager revocation. PCs whose firmware doesn't trust the 2023 certificate then need Secure Boot off to start this media.",
            );
            ui.end_row();
            if app.host.oscdimg.is_some() {
                ui.label("ISO path");
                ui.horizontal(|ui| {
                    if theme::secondary(ui, "Choose…", !app.busy()).clicked() {
                        choose_output(app);
                    }
                    let shown = app.iso_out.as_ref().map_or("Not chosen".to_string(), |p| p.display().to_string());
                    path_label(ui, &shown);
                });
                ui.end_row();
            }
        });
        if app.host.oscdimg.is_none() {
            dim(ui, "oscdimg not found: output is the media folder in the workspace.");
        }
    });
}

/// A new, unused ISO path next to the working folder, for Guided mode.
fn default_output(app: &App) -> Option<PathBuf> {
    let dir = app.workspace_parent.clone()?;
    let base = app.edition().map_or("Windows-custom".to_string(), |e| format!("{}-custom", e.name.replace(' ', "-")));
    (0..100)
        .map(|n| dir.join(if n == 0 { format!("{base}.iso") } else { format!("{base}-{n}.iso") }))
        .find(|p| !p.exists())
}

/// Advanced: the exact compiled operations, in execution order.
fn operation_plan(app: &mut App, ui: &mut Ui) {
    let Some(ctx) = app.plan_ctx().filter(|c| c.inventory.is_some()) else {
        theme::card().show(ui, |ui| {
            theme::card_body(ui);
            h2(ui, "Operation plan");
            dim(ui, "Open an index to compile the plan.");
        });
        ui.add_space(4.0);
        return;
    };
    let p = match app.compile_plan(&ctx) {
        Ok(p) => p,
        Err(e) => {
            theme::error_box(ui, "Plan doesn't compile", &e);
            ui.add_space(4.0);
            return;
        }
    };
    let lines: Vec<String> = p
        .install
        .iter()
        .map(|o| ("install", o))
        .chain(p.boot.iter().map(|o| ("boot.wim#2", o)))
        .enumerate()
        .map(|(n, (target, o))| format!("{:>3} [{target}] {}  ← {}", n + 1, o.op.describe(), o.origin))
        .collect();
    theme::card().show(ui, |ui| {
        theme::card_body(ui);
        ui.horizontal(|ui| {
            h2(ui, &format!("Operation plan ({})", lines.len()));
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if ui.button("Copy").on_hover_text("Copy the whole plan as text").clicked() {
                    let mut text = lines.join("\n");
                    for (id, why) in &p.skipped {
                        text.push_str(&format!("\nskipped {id}: {why}"));
                    }
                    ui.ctx().copy_text(text);
                }
            });
        });
        const ROW: f32 = 16.0;
        let page = crate::pager::layout(ui, "plan", lines.len(), ROW, 190.0);
        let area = ui
            .scope(|ui| {
                ui.spacing_mut().item_spacing.y = 0.0;
                for l in &lines[page.range.clone()] {
                    let size = egui::vec2(ui.available_width(), ROW);
                    ui.allocate_ui_with_layout(size, egui::Layout::left_to_right(egui::Align::Center), |ui| {
                        ui.add(
                            egui::Label::new(RichText::new(l).monospace().size(11.5).color(theme::TEXT_BODY))
                                .truncate(),
                        )
                        .on_hover_text(l);
                    });
                }
            })
            .response
            .rect;
        page.controls(ui, area);
        if !p.skipped.is_empty() {
            dim(ui, &format!("{} selected options compile to nothing on this image (see Copy).", p.skipped.len()));
        }
    });
    ui.add_space(4.0);
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
            app.output.boot_2023 = r.output.boot_2023;
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
    let compiled = match app.compile_plan(&ctx) {
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
            two_columns(ui, |i, ui| {
                if i == 0 {
                    result_card(ui, &r);
                    if !r.warnings.is_empty() {
                        ui.add_space(4.0);
                        theme::warn_box(ui, "Notes", &r.warnings);
                    }
                    theme::fill_next(ui, 0.0);
                    crate::extras::footprint_card(ui, &r);
                } else {
                    theme::fill_next(ui, 80.0);
                    crate::extras::boot_test_card(app, ui, &r);
                    ui.add_space(4.0);
                    ui.horizontal_wrapped(|ui| {
                        if theme::primary(ui, "Write it to a USB drive", true).clicked() {
                            app.usb.source = None;
                            app.usb.report = None;
                            app.switch_tool(crate::app::Tool::Usb);
                        }
                        if theme::secondary(ui, "Save choices…", true)
                            .on_hover_text("Keep this list of changes to reuse on another ISO")
                            .clicked()
                        {
                            save_recipe(app);
                        }
                    });
                    log_tail(app, ui, 6);
                }
            });
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
    });
}

fn result_card(ui: &mut Ui, r: &wip_engine::BuildReport) {
    theme::card().show(ui, |ui| {
        theme::card_body(ui);
        match &r.iso {
            Some(p) => {
                check_line(ui, true, "New ISO saved");
                path_label(ui, &p.display().to_string());
                ui.horizontal(|ui| {
                    if theme::secondary(ui, "Show in folder", true).clicked() {
                        open_folder(p.parent().map(PathBuf::from).unwrap_or_default());
                    }
                    let size = r.iso_bytes.map(|b| format!("{} · ", human_bytes(b))).unwrap_or_default();
                    dim(ui, &format!("{size}{} operations applied", r.applied));
                    if let Some(h) = &r.iso_sha256 {
                        if ui
                            .link("Copy SHA-256")
                            .on_hover_text(format!("SHA-256 {h}\nAlso saved next to the ISO as .sha256"))
                            .clicked()
                        {
                            ui.ctx().copy_text(h.clone());
                        }
                    }
                });
            }
            None => {
                check_line(ui, true, "Finished files saved");
                path_label(ui, &r.media_folder.display().to_string());
                ui.horizontal(|ui| {
                    if theme::secondary(ui, "Open folder", true).clicked() {
                        open_folder(r.media_folder.clone());
                    }
                    dim(ui, &format!("{} operations applied", r.applied));
                });
            }
        }
        if let Some(root) = r.media_folder.parent() {
            let text = format!("You can delete the working folder when you're done: {}", root.display());
            ui.add(egui::Label::new(RichText::new(&text).size(12.0).color(theme::TEXT_DIM)).truncate())
                .on_hover_text(&text);
        }
    });
}

pub fn open_folder(p: PathBuf) {
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
    let Ok(p) = app.compile_plan(&ctx) else { return };
    use wip_core::plan::Op;
    let count = |f: &dyn Fn(&Op) -> bool| p.install.iter().chain(p.boot.iter()).filter(|o| f(&o.op)).count();
    let apps = count(&|o| matches!(o, Op::RemoveAppx { .. }));
    let caps = count(&|o| matches!(o, Op::RemoveCapability { .. }));
    let feats = count(&|o| matches!(o, Op::DisableFeature { .. } | Op::EnableFeature { .. }));
    let regs = count(&|o| matches!(o, Op::RegSet { .. } | Op::RegDelete { .. }));
    let files = count(&|o| matches!(o, Op::DeleteFile { .. }));
    let cleanup = count(&|o| matches!(o, Op::ComponentCleanup { .. }));
    let drivers = p.install.iter().filter(|o| matches!(o.op, Op::AddDriver { .. })).count();
    theme::card().show(ui, |ui| {
        theme::card_body(ui);
        h2(ui, "What happens when you select Build");
        let mut lines = Vec::new();
        if apps + caps + feats > 0 {
            lines.push(format!("1. Remove {apps} app packages and {caps} capabilities, and switch {feats} features"));
        } else {
            lines.push("1. No apps, capabilities or features change".to_string());
        }
        if drivers > 0 {
            lines.push(format!("{}. Add {drivers} drivers", lines.len() + 1));
        }
        lines.push(format!(
            "{}. Write {regs} registry and service settings{}",
            lines.len() + 1,
            if files > 0 { " and delete WinRE" } else { "" }
        ));
        if cleanup > 0 {
            lines.push(format!("{}. Clean up the component store (the slowest step)", lines.len() + 1));
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
        dim(ui, "Good to know: big Windows feature updates can bring back some removed apps, and per-user settings apply to accounts created after installing.");
    });
    ui.add_space(4.0);
}
