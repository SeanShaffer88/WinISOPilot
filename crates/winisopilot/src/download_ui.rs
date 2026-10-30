//! "Download Windows" card on the Choose ISO page: official ESDs from
//! Microsoft's Media Creation Tool catalog, turned into an ISO.

use crate::app::App;
use crate::jobs::{Kind, Outcome};
use crate::theme::{self, body, dim, h2};
use eframe::egui;
use std::collections::BTreeSet;
use std::path::PathBuf;
use wip_core::download::EsdFile;

#[derive(Default)]
pub struct DownloadState {
    pub win10: bool,
    pub entries: Vec<EsdFile>,
    pub listed_win10: bool,
    pub language: Option<String>,
    pub arch: Option<String>,
    pub edition: Option<String>,
}

impl DownloadState {
    pub fn set_entries(&mut self, entries: Vec<EsdFile>, win10: bool) {
        self.entries = entries;
        self.listed_win10 = win10;
        self.language =
            self.entries.iter().find(|e| e.language_code.eq_ignore_ascii_case("en-us")).map(|e| e.language.clone());
        self.arch =
            self.entries.iter().find(|e| e.architecture.eq_ignore_ascii_case("x64")).map(|e| e.architecture.clone());
        self.edition = None;
        self.fix();
    }

    fn values(&self, f: impl Fn(&EsdFile) -> bool, pick: impl Fn(&EsdFile) -> String) -> Vec<String> {
        let set: BTreeSet<String> = self.entries.iter().filter(|e| f(e)).map(pick).collect();
        set.into_iter().collect()
    }

    /// Keeps the three choices consistent with what exists.
    fn fix(&mut self) {
        let langs = self.values(|_| true, |e| e.language.clone());
        if !self.language.as_ref().is_some_and(|l| langs.contains(l)) {
            self.language = langs.first().cloned();
        }
        let lang = self.language.clone();
        let arches = self.values(|e| Some(&e.language) == lang.as_ref(), |e| e.architecture.clone());
        if !self.arch.as_ref().is_some_and(|a| arches.contains(a)) {
            self.arch = arches.first().cloned();
        }
        let arch = self.arch.clone();
        let eds = self.values(
            |e| Some(&e.language) == lang.as_ref() && Some(&e.architecture) == arch.as_ref(),
            |e| e.edition.clone(),
        );
        if !self.edition.as_ref().is_some_and(|x| eds.contains(x)) {
            // Prefer the consumer package (Home, Pro, Education…) over Enterprise.
            let preferred = self
                .entries
                .iter()
                .find(|e| {
                    Some(&e.language) == lang.as_ref()
                        && Some(&e.architecture) == arch.as_ref()
                        && e.is_default_package()
                })
                .map(|e| e.edition.clone());
            self.edition = preferred.or_else(|| eds.first().cloned());
        }
    }

    pub fn chosen(&self) -> Option<&EsdFile> {
        self.entries.iter().find(|e| {
            Some(&e.language) == self.language.as_ref()
                && Some(&e.architecture) == self.arch.as_ref()
                && Some(&e.edition) == self.edition.as_ref()
        })
    }
}

fn combo(
    ui: &mut egui::Ui,
    id: &str,
    label: &str,
    value: &mut Option<String>,
    options: &[String],
    shown: impl Fn(&str) -> String,
) -> bool {
    let before = value.clone();
    ui.label(label);
    let text = value.as_deref().map(&shown).unwrap_or_default();
    // Long lists (languages) are shown in columns so the popup never scrolls.
    const ROWS: usize = 14;
    egui::ComboBox::from_id_salt(id).selected_text(text).width(240.0).height(2000.0).show_ui(ui, |ui| {
        if options.len() <= ROWS {
            for o in options {
                ui.selectable_value(value, Some(o.clone()), shown(o));
            }
        } else {
            ui.horizontal_top(|ui| {
                for col in options.chunks(ROWS) {
                    ui.vertical(|ui| {
                        for o in col {
                            ui.selectable_value(value, Some(o.clone()), shown(o));
                        }
                    });
                }
            });
        }
    });
    *value != before
}

fn unique_iso(dir: &std::path::Path, base: &str) -> PathBuf {
    let mut n = 0;
    loop {
        let name = if n == 0 { format!("{base}.iso") } else { format!("{base}-{n}.iso") };
        let p = dir.join(name);
        if !p.exists() {
            return p;
        }
        n += 1;
    }
}

pub fn card(app: &mut App, ui: &mut egui::Ui, locked: bool) {
    let guided = app.mode() == wip_core::settings::Mode::Guided;
    theme::card().show(ui, |ui| {
        theme::card_body(ui);
        if guided {
            h2(ui, "Download Windows for me");
            body(ui, "Don't have an ISO? WinISOPilot gets the latest Windows straight from Microsoft and checks that it arrived intact.");
        } else {
            h2(ui, "Download from Microsoft (MCT catalog)");
            dim(ui, "ESD from products.cab, verified against the catalog hash, converted to a bootable ISO with DISM + oscdimg.");
        }
        ui.horizontal(|ui| {
            ui.radio_value(&mut app.dl.win10, false, "Windows 11");
            ui.radio_value(&mut app.dl.win10, true, "Windows 10");
            let loaded = !app.dl.entries.is_empty() && app.dl.listed_win10 == app.dl.win10;
            let label = match (loaded, guided) {
                (false, true) => "Show choices",
                (false, false) => "Load list",
                (true, _) => "Reload list",
            };
            if theme::secondary(ui, label, !locked).clicked() {
                let win11 = !app.dl.win10;
                if app.demo {
                    app.dl.set_entries(crate::dialogs::demo_catalog(), !win11);
                    return;
                }
                app.start(Kind::Catalog, move |job| {
                    wip_engine::fetch_catalog(win11, job).map(|e| Outcome::Catalog(e, !win11))
                });
            }
        });
        if app.dl.entries.is_empty() || app.dl.listed_win10 != app.dl.win10 {
            dim(
                ui,
                if guided {
                    "Pick Windows 11 (or 10), then select Show choices. Nothing is downloaded yet."
                } else {
                    "Nothing is downloaded until you choose Load list."
                },
            );
            return;
        }
        let langs = app.dl.values(|_| true, |e| e.language.clone());
        let lang = app.dl.language.clone();
        let arches = app.dl.values(|e| Some(&e.language) == lang.as_ref(), |e| e.architecture.clone());
        let arch = app.dl.arch.clone();
        let eds = app.dl.values(
            |e| Some(&e.language) == lang.as_ref() && Some(&e.architecture) == arch.as_ref(),
            |e| e.edition.clone(),
        );
        let entries = app.dl.entries.clone();
        let package = |ed: &str| {
            entries
                .iter()
                .find(|e| e.edition == ed && Some(&e.language) == lang.as_ref() && Some(&e.architecture) == arch.as_ref())
                .map_or(ed.to_string(), |e| e.package_label())
        };
        let mut changed = false;
        egui::Grid::new("dl").num_columns(2).spacing([12.0, 8.0]).show(ui, |ui| {
            changed |= combo(ui, "dl-lang", "Language", &mut app.dl.language, &langs, |s| s.to_string());
            ui.end_row();
            if !guided {
                changed |= combo(ui, "dl-arch", "Architecture", &mut app.dl.arch, &arches, |s| s.to_string());
                ui.end_row();
                changed |= combo(ui, "dl-ed", "Package", &mut app.dl.edition, &eds, |s| package(s));
                ui.end_row();
            }
        });
        if changed {
            app.dl.fix();
        }
        let Some(f) = app.dl.chosen().cloned() else { return };
        if guided {
            dim(ui, &format!("Includes Windows {} · {} download", f.package_label(), wip_core::human_bytes(f.size)));
        } else {
            dim(ui, &format!("{} · build {}", f.label(), f.build().map_or("?".into(), |b| b.to_string())));
            let hash = f.expected_hash().map_or("none".to_string(), |(a, h)| format!("{a} {h}"));
            ui.add(
                egui::Label::new(
                    egui::RichText::new(format!("{}\n{hash}", f.file_name)).monospace().size(11.5).color(theme::TEXT_DIM),
                )
                .truncate(),
            )
            .on_hover_text(format!("{}\n{}\n{hash}", f.file_name, f.url));
        }
        let parent = app.workspace_parent.clone();
        let blocked = if app.host.oscdimg.is_none() {
            Some(if guided {
                "One free Microsoft tool is needed first (Windows ADK \"Deployment Tools\")."
            } else {
                "oscdimg.exe not found (Windows ADK Deployment Tools)."
            })
        } else if parent.is_none() {
            Some("Choose a working folder first (left); the download and ISO are saved there.")
        } else if f.expected_hash().is_none() {
            Some("Microsoft's catalog has no usable hash for this file, so WinISOPilot won't download it.")
        } else {
            None
        };
        if let Some(why) = blocked {
            dim(ui, why);
        }
        if app.host.oscdimg.is_none() {
            crate::pages::adk_offer(app, ui);
        }
        let button = if guided { "Download Windows" } else { "Download and make ISO" };
        if theme::primary(ui, button, blocked.is_none() && !locked).clicked() {
            let Some(dir) = parent else { return };
            let base = format!(
                "Windows{}_{}_{}",
                if app.dl.win10 { "10" } else { "11" },
                f.architecture,
                f.language_code
            );
            let iso = unique_iso(&dir, &base);
            let dl_dir = dir.join("WinISOPilot-downloads");
            let host = app.host.clone();
            app.start(Kind::DownloadIso, move |job| {
                wip_engine::download_iso(&f, &dl_dir, &iso, &host, job).map(Outcome::Downloaded)
            });
        }
        if !guided {
            dim(ui, "Partial downloads resume from WinISOPilot-downloads; delete that folder when done.");
        }
    });
}
