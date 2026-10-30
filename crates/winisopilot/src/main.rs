#![cfg_attr(all(windows, not(debug_assertions)), windows_subsystem = "windows")]
//! WinISOPilot: a guided Windows 10/11 installation media customizer.

mod app;
mod customize;
mod dialogs;
mod download_ui;
mod extras;
mod help;
mod jobs;
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
    if args.iter().any(|a| a == "--self-test") {
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
