#![cfg_attr(all(windows, not(debug_assertions)), windows_subsystem = "windows")]
//! WinISOPilot: a guided Windows 10/11 installation media customizer.

mod app;
mod customize;
mod dialogs;
mod jobs;
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
    let mut viewport = egui::ViewportBuilder::default()
        .with_title("WinISOPilot")
        .with_inner_size([1200.0, 760.0])
        .with_min_inner_size([960.0, 640.0]);
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
            Ok(Box::new(app))
        }),
    )
}
