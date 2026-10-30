//! Modal dialogs: first-run mode choice, settings, confirmations.

use crate::app::{App, Step};
use crate::theme::{self, body, dim, h1, h2};
use eframe::egui::{self, RichText, Stroke};
use std::path::PathBuf;
use wip_core::inventory::{AppxPackage, Inventory, NamedState, PackageState};
use wip_core::settings::Mode;
use wip_core::{ImageSummary, Level};

pub fn show(app: &mut App, ctx: &egui::Context) {
    if app.settings.mode.is_none() {
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

fn mode_card(ui: &mut egui::Ui, title: &str, text: &str, selected: bool) -> bool {
    let stroke =
        Stroke::new(if selected { 2.0_f32 } else { 1.0 }, if selected { theme::ACCENT } else { theme::DIVIDER });
    let r = theme::card()
        .stroke(stroke)
        .show(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.set_min_height(150.0);
            ui.vertical(|ui| {
                ui.label(RichText::new(title).size(18.0).strong());
                body(ui, text);
            });
        })
        .response;
    let r = r.interact(egui::Sense::click());
    r.widget_info(|| egui::WidgetInfo::selected(egui::WidgetType::RadioButton, true, selected, title));
    r.clicked()
}

fn first_run(app: &mut App, ctx: &egui::Context) {
    let mut pick: Option<Mode> = None;
    egui::Modal::new(egui::Id::new("first-run")).show(ctx, |ui| {
        ui.set_width(660.0);
        h1(ui, "How would you like to work?");
        body(ui, "You can change this any time in Settings.");
        ui.add_space(10.0);
        ui.columns(2, |cols| {
            let (left, right) = cols.split_at_mut(1);
            let ui = &mut left[0];
            if mode_card(
                ui,
                "Guided (recommended)",
                "For everyone. Simple questions, plain words, safe choices made for you. Riskier options stay hidden until you ask for them.",
                true,
            ) {
                pick = Some(Mode::Guided);
            }
            let ui = &mut right[0];
            if mode_card(
                ui,
                "Advanced",
                "For experts. Every package, capability and setting in one table, editable paths, the exact DISM/registry operation plan, all output and USB options.",
                false,
            ) {
                pick = Some(Mode::Advanced);
            }
        });
        ui.add_space(10.0);
        ui.horizontal(|ui| {
            if theme::primary(ui, "Use Guided", true).clicked() {
                pick = Some(Mode::Guided);
            }
            if theme::secondary(ui, "Use Advanced", true).clicked() {
                pick = Some(Mode::Advanced);
            }
        });
    });
    if let Some(m) = pick {
        app.settings.mode = Some(m);
        if m == Mode::Advanced {
            app.settings.level = Level::Expert;
        }
        app.save_settings();
    }
}

fn settings(app: &mut App, ctx: &egui::Context) {
    let mut changed = false;
    let mut close = ctx.input_mut(|i| i.consume_key(egui::Modifiers::NONE, egui::Key::Escape));
    // A modal keeps the whole window in view (title, Done button) and closes
    // with Done, Esc or a click outside.
    let resp = egui::Modal::new(egui::Id::new("settings")).show(ctx, |ui| {
        ui.set_width(760.0_f32.min(ctx.content_rect().width() - 60.0));
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
                h2(ui, "Layout");
                let mut mode = app.mode();
                ui.horizontal(|ui| {
                    changed |= ui.radio_value(&mut mode, Mode::Guided, "Guided").changed();
                    changed |= ui.radio_value(&mut mode, Mode::Advanced, "Advanced").changed();
                });
                if Some(mode) != app.settings.mode {
                    app.settings.mode = Some(mode);
                    app.layout_changed();
                }
                dim(ui, "Guided explains one section at a time. Advanced shows a dense table with technical details. Switching returns to step 1; your choices are kept.");
                ui.add_space(6.0);
                h2(ui, "Options shown in Guided mode");
                ui.horizontal(|ui| {
                    for l in Level::ALL {
                        changed |= ui.radio_value(&mut app.settings.level, l, l.label()).changed();
                    }
                });
                dim(ui, "Beginner: safe and low risk. Intermediate adds moderate risk. Expert shows everything.");
                ui.add_space(6.0);
                h2(ui, "Text size");
                let mut pct = (app.settings.text_scale * 100.0).round() as i32;
                if ui.add(egui::Slider::new(&mut pct, 75..=200).suffix("%")).changed() {
                    app.settings.text_scale = pct as f32 / 100.0;
                    changed = true;
                }
                dim(ui, "Ctrl + and Ctrl − also change the size.");
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
        });
    });
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
    let Ok(plan) = wip_core::plan::compile(&app.selection, &ctx) else { return };
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
