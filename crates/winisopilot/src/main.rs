#![cfg_attr(all(windows, not(debug_assertions)), windows_subsystem = "windows")]
//! WinISOPilot: a guided Windows 10/11 installation media customizer.

mod answers_ui;
mod app;
mod cli;
mod customize;
mod dialogs;
mod download_ui;
mod drivers_ui;
mod extras;
mod help;
mod jobs;
mod packages_ui;
mod pager;
mod pages;
mod theme;
mod usb_page;

use eframe::egui;

fn self_test() -> i32 {
    let mut ok = true;
    match wip_core::Catalog::builtin() {
        Ok(c) => println!("catalog: {} options, {} presets", c.options.len(), c.presets.len()),
        Err(e) => {
            println!("catalog: FAILED {e}");
            ok = false;
        }
    }
    let host = wip_engine::probe_host(None);
    println!("{}", serde_json::to_string_pretty(&host).unwrap_or_default());
    println!("version: {}", wip_core::APP_VERSION);
    if ok {
        0
    } else {
        1
    }
}

fn icon() -> Option<egui::IconData> {
    let bytes = include_bytes!("../../../assets/icon-256.rgba");
    (bytes.len() == 256 * 256 * 4).then(|| egui::IconData { rgba: bytes.to_vec(), width: 256, height: 256 })
}

fn main() -> eframe::Result<()> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.first().is_some_and(|a| a == "--build") {
        std::process::exit(cli::run(&args));
    }
    if args.iter().any(|a| a == "--help" || a == "/?") {
        wip_engine::attach_parent_console();
        println!("{}", cli::USAGE);
        std::process::exit(0);
    }
    if args.iter().any(|a| a == "--self-test") {
        wip_engine::attach_parent_console();
        std::process::exit(self_test());
    }
    let demo = args.iter().any(|a| a == "--demo");
    let usb = args.iter().any(|a| a == "--usb");
    let done = args.iter().any(|a| a == "--done");
    let source = args.iter().any(|a| a == "--source");
    let review = args.iter().any(|a| a == "--review");
    let help_open = args.iter().any(|a| a == "--help-panel");
    let mut viewport = egui::ViewportBuilder::default()
        .with_title("WinISOPilot")
        .with_inner_size(app::BASE_SIZE)
        .with_resizable(false)
        .with_maximize_button(false);
    if let Some(i) = icon() {
        viewport = viewport.with_icon(i);
    }
    let options = eframe::NativeOptions { viewport, ..Default::default() };
    eframe::run_native(
        "WinISOPilot",
        options,
        Box::new(move |cc| {
            let mut app = app::App::new(cc, demo);
            if usb {
                app.switch_tool(app::Tool::Usb);
            }
            if source && demo {
                app.dl.set_entries(dialogs::demo_catalog(), false);
                app.workspace_parent = Some(std::path::PathBuf::from(r"D:\ISO"));
                app.step = app::Step::Source;
            }
            if review && demo {
                app.step = app::Step::Review;
            }
            app.show_help = help_open;
            if done && demo {
                dialogs::load_demo_done(&mut app);
            }
            if demo {
                // Screenshot helpers: skip the first-run choice unless asked,
                // and open a given step, layout or the Settings window.
                let flag = |name: &str| args.iter().any(|a| a == name);
                // --first-run: no layout chosen yet; --ask: the every-launch question.
                if !flag("--first-run") {
                    app.mode_prompt = flag("--ask");
                    app.settings.mode = Some(if flag("--advanced") {
                        wip_core::settings::Mode::Advanced
                    } else {
                        wip_core::settings::Mode::Guided
                    });
                }
                app.show_settings = flag("--settings");
                // --busy: a long-running sample job (screenshots of progress).
                if flag("--busy") {
                    app.start(jobs::Kind::InstallAdk, |job| {
                        job.stage(3, 3, "Installing the Deployment Tools (several minutes)");
                        job.progress(0.4);
                        std::thread::sleep(std::time::Duration::from_secs(30));
                        Ok(jobs::Outcome::AdkInstalled)
                    });
                }
                // --drivers: the Drivers page with a sample folder.
                if flag("--answers") {
                    app.step = app::Step::Customize;
                    app.view = app::View::Answers;
                    let a = &mut app.output.answers;
                    a.enabled = true;
                    a.language = "en-US".into();
                    a.time_zone = "Pacific Standard Time".into();
                    a.account_name = "Sam".into();
                }
                if flag("--updates") {
                    app.step = app::Step::Customize;
                    app.view = app::View::Packages;
                    app.packages.add(
                        [
                            r"D:\Updates\windows11.0-kb5043080-x64_953449672073f8fb99badb4cc6d5d7849b9c83e8.msu",
                            r"D:\Updates\windows11.0-kb5065789-x64_ndp481.msu",
                            r"D:\Updates\Microsoft-Windows-Client-Language-Pack_x64_de-de.cab",
                            r"D:\Updates\windows11.0-kb5065426-arm64.msu",
                        ]
                        .map(std::path::PathBuf::from),
                    );
                }
                if flag("--drivers") {
                    app.step = app::Step::Customize;
                    app.view = app::View::Drivers;
                    app.drivers.folders.push(dialogs::demo_drivers());
                }
                // --scale=150: open at that scale.
                if let Some(pct) = args.iter().find_map(|a| a.strip_prefix("--scale=")?.parse::<f32>().ok()) {
                    app.settings.text_scale = (pct / 100.0).clamp(0.75, 2.0);
                    theme::apply(&cc.egui_ctx, &app.settings);
                }
                for (name, step) in [
                    ("--welcome", app::Step::Welcome),
                    ("--edition", app::Step::Edition),
                    ("--customize", app::Step::Customize),
                    ("--build", app::Step::Build),
                ] {
                    if flag(name) {
                        app.step = step;
                    }
                }
            }
            Ok(Box::new(app))
        }),
    )
}
