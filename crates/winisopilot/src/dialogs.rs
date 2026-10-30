//! Modal dialogs: first-run mode choice, settings, confirmations.

use crate::app::{App, Step};
use crate::theme::{self, body, dim, h1, h2};
use eframe::egui::{self, Frame, Margin, RichText, Stroke, Vec2};
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
                "One section at a time, with a plain-language explanation of every option and what it could break. Riskier options stay hidden until you ask for them.",
                true,
            ) {
                pick = Some(Mode::Guided);
            }
            let ui = &mut right[0];
            if mode_card(
                ui,
                "Advanced",
                "A dense table of everything in the image, with package names, risk filters and technical details. For people who know Windows internals.",
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
    let mut open = true;
    let mut changed = false;
    egui::Window::new("Settings")
        .open(&mut open)
        .collapsible(false)
        .resizable(false)
        .default_width(520.0)
        .anchor(egui::Align2::CENTER_CENTER, Vec2::ZERO)
        .frame(Frame::window(&ctx.style()).inner_margin(Margin::same(20)))
        .show(ctx, |ui| {
            h2(ui, "Layout");
            let mut mode = app.mode();
            ui.horizontal(|ui| {
                changed |= ui.radio_value(&mut mode, Mode::Guided, "Guided").changed();
                changed |= ui.radio_value(&mut mode, Mode::Advanced, "Advanced").changed();
            });
            app.settings.mode = Some(mode);
            dim(ui, "Guided explains one section at a time. Advanced shows a dense table with technical details.");
            ui.add_space(8.0);
            h2(ui, "Options shown in Guided mode");
            ui.horizontal(|ui| {
                for l in Level::ALL {
                    changed |= ui.radio_value(&mut app.settings.level, l, l.label()).changed();
                }
            });
            dim(ui, "Beginner: safe and low-risk options. Intermediate adds moderate risk. Expert shows everything.");
            ui.add_space(8.0);
            h2(ui, "Text size");
            let mut pct = (app.settings.text_scale * 100.0).round() as i32;
            if ui.add(egui::Slider::new(&mut pct, 75..=200).suffix("%")).changed() {
                app.settings.text_scale = pct as f32 / 100.0;
                ctx.set_zoom_factor(app.settings.text_scale);
                changed = true;
            }
            ui.add_space(8.0);
            h2(ui, "Windows ADK DISM folder");
            dim(
                ui,
                "Optional. Use the ADK's DISM when your PC's own DISM is older than the Windows version you customize.",
            );
            ui.horizontal(|ui| {
                let shown = if app.settings.dism_folder.is_empty() {
                    "Automatic".to_string()
                } else {
                    app.settings.dism_folder.clone()
                };
                ui.label(RichText::new(shown).monospace().size(12.5));
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
            });
            dim(ui, "A new DISM folder takes effect the next time WinISOPilot starts.");
            ui.add_space(8.0);
            h2(ui, "Default working folder");
            ui.horizontal(|ui| {
                let shown = if app.settings.workspace_root.is_empty() {
                    "Ask each time".to_string()
                } else {
                    app.settings.workspace_root.clone()
                };
                ui.label(RichText::new(shown).monospace().size(12.5));
                if ui.button("Choose…").clicked() {
                    if let Some(p) = rfd::FileDialog::new().pick_folder() {
                        app.settings.workspace_root = p.display().to_string();
                        app.workspace_parent.get_or_insert(PathBuf::from(&app.settings.workspace_root));
                        changed = true;
                    }
                }
            });
            ui.add_space(8.0);
            ui.separator();
            dim(
                ui,
                &format!(
                    "WinISOPilot {} · MIT License · Logs: {}",
                    wip_core::APP_VERSION,
                    wip_engine::app_data_dir().join("logs").display()
                ),
            );
        });
    if changed {
        app.save_settings();
    }
    if !open {
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
