//! Modal dialogs: first-run mode choice, settings, confirmations.

use crate::app::{App, Step};
use crate::theme::{self, body, dim, h1, h2};
use eframe::egui::{self, RichText, Stroke};
use std::path::PathBuf;
use wip_core::inventory::{AppxPackage, Inventory, NamedState, PackageState};
use wip_core::settings::Mode;
use wip_core::{ImageSummary, Level};

pub fn show(app: &mut App, ctx: &egui::Context) {
    if app.settings.mode.is_none() || app.mode_prompt {
        first_run(app, ctx);
        return;
    }
    if app.show_settings {
        settings(app, ctx);
    }
    if app.confirm_cancel {
        confirm_cancel(app, ctx);
    }
    if app.confirm_close {
        confirm_close(app, ctx);
    }
}

/// A whole card that acts as one button (click, or Tab then Space/Enter).
fn mode_card(ui: &mut egui::Ui, title: &str, text: &str, selected: bool) -> bool {
    let id = ui.id().with(title);
    let hovered = ui.ctx().data(|d| d.get_temp::<bool>(id)).unwrap_or(false);
    let accent = selected || hovered;
    let stroke = Stroke::new(if accent { 2.0_f32 } else { 1.0 }, if accent { theme::ACCENT } else { theme::DIVIDER });
    let r = theme::card()
        .fill(if hovered { theme::ROW_SELECTED } else { theme::CARD })
        .stroke(stroke)
        .show(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.set_min_height(96.0);
            ui.vertical(|ui| {
                ui.label(RichText::new(title).size(15.0).strong());
                body(ui, text);
                ui.add_space(6.0);
                ui.label(
                    RichText::new(format!("Select to use {}", title.split(' ').next().unwrap_or(title)))
                        .color(theme::ACCENT),
                );
            });
        })
        .response;
    let r = r.interact(egui::Sense::click()).on_hover_cursor(egui::CursorIcon::PointingHand);
    let now = r.hovered() || r.has_focus();
    if now != hovered {
        ui.ctx().data_mut(|d| d.insert_temp(id, now));
        ui.ctx().request_repaint();
    }
    r.widget_info(|| egui::WidgetInfo::selected(egui::WidgetType::RadioButton, true, selected, title));
    r.clicked()
}

/// Layout question: on first run, and at every launch unless turned off.
fn first_run(app: &mut App, ctx: &egui::Context) {
    let current = app.settings.mode.unwrap_or(Mode::Guided);
    let mut pick: Option<Mode> = None;
    egui::Modal::new(egui::Id::new("first-run")).show(ctx, |ui| {
        ui.set_width(560.0);
        h1(ui, "How would you like to work?");
        body(ui, "Pick the layout for this session. Switch any time with Guided / Advanced in the left panel; your work is kept.");
        ui.add_space(10.0);
        ui.columns(2, |cols| {
            let (left, right) = cols.split_at_mut(1);
            if mode_card(
                &mut left[0],
                "Guided (recommended)",
                "For everyone. Simple questions, plain words, safe choices made for you. Riskier options stay hidden until you ask for them.",
                current == Mode::Guided,
            ) {
                pick = Some(Mode::Guided);
            }
            if mode_card(
                &mut right[0],
                "Advanced",
                "For experts. Every package, capability and setting in one table, editable paths, the exact DISM/registry operation plan, all output and USB options.",
                current == Mode::Advanced,
            ) {
                pick = Some(Mode::Advanced);
            }
        });
        ui.add_space(10.0);
        ui.horizontal(|ui| {
            dim(ui, "Click a card (or press Enter for the highlighted one).");
            if app.settings.mode.is_some() {
                ui.add_space(12.0);
                ui.checkbox(&mut app.mode_prompt_dont_ask, "Don't ask again")
                    .on_hover_text("Start with the layout you pick. Turn the question back on in Settings.");
            }
        });
        if pick.is_none() && ui.input(|i| i.key_pressed(egui::Key::Enter)) && ui.memory(|m| m.focused().is_none()) {
            pick = Some(current);
        }
    });
    if let Some(m) = pick {
        app.settings.mode = Some(m);
        if app.mode_prompt_dont_ask {
            app.settings.ask_mode_on_launch = false;
        }
        app.mode_prompt = false;
        app.save_settings();
    }
}

fn settings(app: &mut App, ctx: &egui::Context) {
    let mut changed = false;
    let mut diag = false;
    let mut close = ctx.input_mut(|i| i.consume_key(egui::Modifiers::NONE, egui::Key::Escape));
    // A modal keeps the whole window in view (title, Done button) and closes
    // with Done, Esc or a click outside.
    let resp = egui::Modal::new(egui::Id::new("settings")).show(ctx, |ui| {
        ui.set_width(640.0_f32.min(ctx.content_rect().width() - 40.0));
        ui.horizontal(|ui| {
            h1(ui, "Settings");
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if theme::primary(ui, "Done", true).on_hover_text("Esc").clicked() {
                    close = true;
                }
            });
        });
        dim(ui, "Changes are saved straight away.");
        ui.add_space(6.0);
        crate::pages::two_columns(ui, |i, ui| {
            if i == 0 {
                h2(ui, "Options shown in Guided mode");
                ui.horizontal(|ui| {
                    for l in Level::ALL {
                        changed |= ui.radio_value(&mut app.settings.level, l, l.label()).changed();
                    }
                });
                dim(ui, "Beginner: safe and low risk. Intermediate adds moderate risk. Expert shows everything.");
                ui.add_space(6.0);
                h2(ui, "Scale");
                // Applied when the slider is released, so the window doesn't
                // resize under the pointer while dragging.
                let id = egui::Id::new("scale_slider");
                let mut pct = ui
                    .data(|d| d.get_temp::<i32>(id))
                    .unwrap_or_else(|| (app.settings.text_scale * 100.0).round() as i32);
                let r = ui.add(egui::Slider::new(&mut pct, 75..=200).step_by(5.0).suffix("%"));
                if r.dragged() {
                    ui.data_mut(|d| d.insert_temp(id, pct));
                } else {
                    ui.data_mut(|d| d.remove::<i32>(id));
                    if r.changed() || r.drag_stopped() {
                        app.settings.text_scale = pct as f32 / 100.0;
                        changed = true;
                    }
                }
                dim(ui, "Resizes the window and everything in it. Ctrl + / Ctrl − / Ctrl 0 also work.");
                ui.add_space(6.0);
                h2(ui, "Layout");
                changed |= ui
                    .checkbox(&mut app.settings.ask_mode_on_launch, "Ask Guided or Advanced at every launch")
                    .changed();
                dim(ui, "Switch layout any time with Guided / Advanced in the left panel.");
            } else {
                h2(ui, "Accessibility and comfort");
                changed |= ui
                    .checkbox(&mut app.settings.high_contrast, "Stronger contrast (white text, yellow focus outline)")
                    .changed();
                changed |= ui.checkbox(&mut app.settings.reduce_motion, "Reduce motion (no animations)").changed();
                changed |= ui.checkbox(&mut app.settings.show_tips, "Show a tip at the top of each page").changed();
                ui.add_space(6.0);
                h2(ui, "Windows ADK DISM folder");
                dim(ui, "Optional: for customizing a newer Windows than this PC runs. Takes effect after a restart.");
                ui.horizontal(|ui| {
                    if ui.button("Choose…").clicked() {
                        if let Some(p) = rfd::FileDialog::new().pick_folder() {
                            app.settings.dism_folder = p.display().to_string();
                            changed = true;
                        }
                    }
                    if !app.settings.dism_folder.is_empty() && ui.button("Reset").clicked() {
                        app.settings.dism_folder.clear();
                        changed = true;
                    }
                    let shown = if app.settings.dism_folder.is_empty() {
                        "Automatic".to_string()
                    } else {
                        app.settings.dism_folder.clone()
                    };
                    crate::pages::path_label(ui, &shown);
                });
                ui.add_space(6.0);
                h2(ui, "Default working folder");
                ui.horizontal(|ui| {
                    if ui.button("Choose…").clicked() {
                        if let Some(p) = rfd::FileDialog::new().pick_folder() {
                            app.settings.workspace_root = p.display().to_string();
                            app.workspace_parent.get_or_insert(PathBuf::from(&app.settings.workspace_root));
                            changed = true;
                        }
                    }
                    let shown = if app.settings.workspace_root.is_empty() {
                        "Ask each time".to_string()
                    } else {
                        app.settings.workspace_root.clone()
                    };
                    crate::pages::path_label(ui, &shown);
                });
            }
        });
        ui.add_space(6.0);
        ui.separator();
        ui.horizontal(|ui| {
            dim(ui, &format!("WinISOPilot {} · MIT License", wip_core::APP_VERSION));
            if ui.link("Open log folder").clicked() {
                crate::pages::open_folder(wip_engine::app_data_dir().join("logs"));
            }
            if ui
                .link("Save diagnostics…")
                .on_hover_text("One redacted text file for support; nothing is sent anywhere.")
                .clicked()
            {
                diag = true;
            }
        });
    });
    if diag {
        app.save_diagnostics();
    }
    if changed {
        crate::theme::apply(ctx, &app.settings);
        app.save_settings();
    }
    if close || resp.should_close() {
        app.show_settings = false;
    }
}

fn confirm_cancel(app: &mut App, ctx: &egui::Context) {
    egui::Modal::new(egui::Id::new("confirm-cancel")).show(ctx, |ui| {
        ui.set_width(440.0);
        h2(ui, "Stop this task?");
        body(ui, "WinISOPilot finishes the current step safely, then stops. Nothing is saved to the image.");
        ui.horizontal(|ui| {
            if theme::secondary(ui, "Keep going", true).clicked() {
                app.confirm_cancel = false;
            }
            if theme::primary(ui, "Stop", true).clicked() {
                if let Some(j) = app.job.as_mut() {
                    j.request_cancel();
                }
                app.confirm_cancel = false;
            }
        });
    });
}

fn confirm_close(app: &mut App, ctx: &egui::Context) {
    egui::Modal::new(egui::Id::new("confirm-close")).show(ctx, |ui| {
        ui.set_width(480.0);
        if app.busy() {
            h2(ui, "A task is still running");
            body(ui, "Wait for it to finish, or cancel it first. Closing now could leave a damaged image.");
            if theme::primary(ui, "OK", true).clicked() {
                app.confirm_close = false;
            }
            return;
        }
        h2(ui, "An edition is still open");
        body(ui, "Discard it now to free the working files, or close and clean it up next time WinISOPilot starts.");
        ui.horizontal(|ui| {
            if theme::secondary(ui, "Back", true).clicked() {
                app.confirm_close = false;
            }
            if theme::secondary(ui, "Close anyway", true).clicked() {
                app.confirm_close = false;
                app.really_close(ctx);
            }
            if theme::primary(ui, "Discard edition", true).clicked() {
                app.confirm_close = false;
                app.discard_workspace();
            }
        });
    });
}

/// `--demo --done`: a sample finished build to show the result screen.
pub fn load_demo_done(app: &mut App) {
    use wip_core::footprint::{Changes, Footprint, Measure};
    let Some(inv) = app.inventory.clone() else { return };
    let Some(ctx) = app.plan_ctx() else { return };
    let Ok(plan) = app.compile_plan(&ctx) else { return };
    let mut after = inv.clone();
    after.appx.retain(|a| {
        !a.display_name.starts_with("Clipchamp")
            && !a.display_name.contains("Bing")
            && !a.display_name.contains("Solitaire")
    });
    let edition = app.edition().cloned().expect("demo edition");
    let fp = Footprint {
        schema: 1,
        edition: edition.name.clone(),
        version: edition.version_string(),
        before: Measure::of(&inv, 18_000_000_000, 5_400_000_000),
        after: Measure::of(&after, 17_200_000_000, 5_150_000_000),
        changes: Changes::between(&inv, &after),
        recompressed: false,
        settings_changed: 7,
    };
    app.extras.snapshot = Some(crate::extras::BuildSnapshot { plan, inventory: inv, edition });
    app.report = Some(wip_engine::BuildReport {
        iso: Some(PathBuf::from(r"D:\ISO\Windows-11-Pro-custom.iso")),
        media_folder: PathBuf::from(r"D:\ISO\WinISOPilot-demo\media"),
        iso_bytes: Some(5_300_000_000),
        iso_sha256: Some("3F9C0E5B7A1D2C4E6F8091A2B3C4D5E6F708192A3B4C5D6E7F8091A2B3C4D5E6".into()),
        boot_signer: Some("Microsoft Windows Production PCA 2011".into()),
        answer_file: false,
        undo_script: Some(PathBuf::from(r"D:\ISO\WinISOPilot-demo\undo.ps1")),
        applied: 14,
        footprint: Some(fp),
        test_target: wip_core::boottest::TestTarget {
            language: "en-US".into(),
            index: 1,
            edition_id: "Professional".into(),
            arch: "amd64".into(),
        },
        warnings: vec![],
        log_file: PathBuf::from(r"D:\ISO\WinISOPilot-demo\winisopilot.log"),
    });
    app.max_step = Step::Build;
    app.step = Step::Build;
}

/// Sample Microsoft catalog entries for `--demo`.
pub fn demo_catalog() -> Vec<wip_core::download::EsdFile> {
    let e = |code: &str, lang: &str, arch: &str, size: u64| wip_core::download::EsdFile {
        file_name: format!("26100.1742.demo_CLIENTCONSUMER_RET_{arch}FRE_{code}.esd"),
        language_code: code.into(),
        language: lang.into(),
        edition: "Professional".into(),
        architecture: arch.into(),
        size,
        sha1: Some("0123456789ABCDEF0123456789ABCDEF01234567".into()),
        sha256: None,
        url: "https://dl.delivery.mp.microsoft.com/demo.esd".into(),
    };
    vec![
        e("en-us", "English (United States)", "x64", 4_310_000_000),
        e("en-gb", "English (United Kingdom)", "x64", 4_300_000_000),
        e("de-de", "German", "x64", 4_320_000_000),
        e("en-us", "English (United States)", "ARM64", 4_100_000_000),
    ]
}

/// Sample drives for `--demo`.
pub fn demo_disks() -> Vec<wip_core::usb::Disk> {
    let d = |n: u32, name: &str, size: u64, bus: &str, sys: bool, l: &str| wip_core::usb::Disk {
        number: n,
        friendly_name: format!("{name} (demo)"),
        serial_number: None,
        unique_id: None,
        size,
        bus_type: bus.into(),
        is_boot: sys,
        is_system: sys,
        is_read_only: false,
        is_offline: false,
        letters: vec![l.into()],
    };
    vec![
        d(0, "NVMe SSD", 1_000_204_886_016, "NVMe", true, "C"),
        d(1, "SATA HDD", 2_000_398_934_016, "SATA", false, "D"),
        d(2, "USB Flash Drive", 31_037_849_600, "USB", false, "E"),
        d(3, "USB Flash Drive", 4_009_754_624, "USB", false, "F"),
    ]
}

/// `--demo`: sample data so the interface can be explored and screenshotted
/// without Windows tools. Clearly labelled in the rail.
pub fn load_demo(app: &mut App) {
    let img = ImageSummary {
        index: 6,
        name: "Windows 11 Pro (demo)".into(),
        description: "Demo data".into(),
        edition_id: "Professional".into(),
        installation_type: "Client".into(),
        architecture: "x64".into(),
        major: 10,
        minor: 0,
        build: 26100,
        sp_build: 1,
        size_bytes: 18_000_000_000,
        languages: vec!["en-US".into()],
    };
    app.source = Some(wip_engine::SourceInfo {
        iso: PathBuf::from("demo.iso"),
        iso_bytes: 5_800_000_000,
        image_rel: r"sources\install.wim".into(),
        images: vec![img],
    });
    app.chosen_index = Some(6);
    let appx = [
        "Clipchamp.Clipchamp",
        "Microsoft.BingNews",
        "Microsoft.BingWeather",
        "Microsoft.BingSearch",
        "Microsoft.MicrosoftSolitaireCollection",
        "Microsoft.Getstarted",
        "Microsoft.GetHelp",
        "Microsoft.WindowsFeedbackHub",
        "Microsoft.OutlookForWindows",
        "Microsoft.GamingApp",
        "Microsoft.XboxGamingOverlay",
        "Microsoft.XboxIdentityProvider",
        "Microsoft.YourPhone",
        "Microsoft.Copilot",
        "Microsoft.WindowsStore",
        "Microsoft.DesktopAppInstaller",
        "Microsoft.Windows.Photos",
        "MicrosoftWindows.Client.WebExperience",
        "Microsoft.Todos",
        "Microsoft.ZuneMusic",
        "Contoso.SampleApp",
    ];
    let caps = [
        "App.StepsRecorder~~~~0.0.1.0",
        "MathRecognizer~~~~0.0.1.0",
        "Media.WindowsMediaPlayer~~~~0.0.12.0",
        "Language.Speech~~~en-US~0.0.1.0",
    ];
    app.inventory = Some(Inventory {
        appx: appx
            .iter()
            .map(|d| AppxPackage { display_name: d.to_string(), package_name: format!("{d}_1.0.0.0_neutral_~_demo") })
            .collect(),
        capabilities: caps.iter().map(|c| NamedState { name: c.to_string(), state: PackageState::Installed }).collect(),
        features: vec![
            NamedState { name: "SMB1Protocol".into(), state: PackageState::Staged },
            NamedState { name: "NetFx3".into(), state: PackageState::Removed },
            NamedState { name: "Recall".into(), state: PackageState::Installed },
            NamedState { name: "Containers-DisposableClientVM".into(), state: PackageState::Staged },
        ],
        services: ["DiagTrack", "WSearch", "SysMain", "lfsvc", "MapsBroker", "WerSvc", "Spooler"]
            .iter()
            .map(|s| s.to_string())
            .collect(),
        has_sxs: true,
    });
    if let Some(inv) = &app.inventory {
        app.raw_items = wip_core::raw::items(inv, &app.catalog);
    }
    for id in ["apps.clipchamp", "apps.news", "apps.solitaire", "priv.tips"] {
        app.selection.insert(id.to_string());
    }
    app.focused = Some("apps.xbox".into());
    app.max_step = Step::Review;
    app.step = Step::Customize;
}

/// `--demo --drivers`: a sample driver folder.
pub fn demo_drivers() -> wip_core::drivers::DriverFolder {
    use wip_core::drivers::InfInfo;
    let inf = |name: &str, class: &str, provider: &str, ver: &str, archs: &[&str], signed: bool| InfInfo {
        path: std::path::PathBuf::from(format!(r"D:\Drivers\Laptop\{name}")),
        class: class.into(),
        provider: provider.into(),
        version: ver.into(),
        archs: archs.iter().map(|a| a.to_string()).collect(),
        signed,
    };
    wip_core::drivers::DriverFolder {
        path: std::path::PathBuf::from(r"D:\Drivers\Laptop"),
        truncated: false,
        infs: vec![
            inf("iaStorVD.inf", "SCSIAdapter", "Intel Corporation", "05/02/2025,20.2.1.1016", &["x64"], true),
            inf("netwtw14.inf", "Net", "Intel", "08/12/2025,23.110.0.5", &["x64"], true),
            inf("rtkaudio.inf", "MEDIA", "Realtek", "03/14/2025,6.0.9790.1", &["x64"], true),
            inf("oldtouch.inf", "HIDClass", "Vendor", "01/01/2012,1.0.0.1", &["x86"], true),
            inf("tool.inf", "System", "Vendor", "", &["x64"], false),
        ],
    }
}
