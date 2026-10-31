//! "Write to USB": a single-page, Rufus-style bootable USB writer.

use crate::app::App;
use crate::jobs::{Kind, Outcome};
use crate::pages::{central, footer, job_panel, log_tail, Nav};
use crate::theme::{self, body, check_line, dim, eyebrow, h1, h2};
use eframe::egui::{self, RichText};
use std::path::{Path, PathBuf};
use wip_core::human_bytes;
use wip_core::usb::{ineligible, ineligible_for, leftover_mb, likely_backup_drive, sanitize_label, Disk, Scheme};
use wip_engine::{BuildReport, UsbReport, UsbRequest, UsbSource};

const CONFIRM_WORD: &str = "ERASE";

pub struct UsbState {
    pub source: Option<UsbSource>,
    pub disks: Vec<Disk>,
    pub listed: bool,
    pub chosen: Option<u32>,
    pub scheme: Scheme,
    pub label: String,
    pub verify: bool,
    /// Make a second exFAT partition from space beyond the 32 GB FAT32 cap.
    pub data_partition: bool,
    pub confirm: bool,
    pub confirm_text: String,
    /// The confirmation is for "restore to a normal drive", not writing.
    pub restore: bool,
    pub report: Option<UsbReport>,
}

impl Default for UsbState {
    fn default() -> Self {
        UsbState {
            source: None,
            disks: Vec::new(),
            listed: false,
            chosen: None,
            scheme: Scheme::Mbr,
            label: "WINSETUP".into(),
            verify: true,
            data_partition: false,
            confirm: false,
            confirm_text: String::new(),
            restore: false,
            report: None,
        }
    }
}

impl UsbState {
    /// Picks the newest build (or the chosen ISO) when nothing is set yet.
    pub fn default_source(&mut self, build: Option<&BuildReport>, iso: Option<&Path>) {
        if self.source.is_some() {
            return;
        }
        self.source = match build {
            Some(r) => Some(match &r.iso {
                Some(p) => UsbSource::Iso(p.clone()),
                None => UsbSource::Folder(r.media_folder.clone()),
            }),
            None => iso.map(|p| UsbSource::Iso(p.to_path_buf())),
        };
    }

    pub fn set_disks(&mut self, disks: Vec<Disk>) {
        if let Some(n) = self.chosen {
            if !disks.iter().any(|d| d.number == n && ineligible(d, &[]).is_none()) {
                self.chosen = None;
            }
        }
        self.disks = disks;
    }

    fn chosen_disk(&self) -> Option<&Disk> {
        self.chosen.and_then(|n| self.disks.iter().find(|d| d.number == n))
    }
}

fn source_path(s: &UsbSource) -> &Path {
    match s {
        UsbSource::Iso(p) | UsbSource::Folder(p) => p,
    }
}

fn protected_paths(app: &App) -> Vec<PathBuf> {
    let mut v = Vec::new();
    if let Some(s) = &app.usb.source {
        v.push(source_path(s).to_path_buf());
    }
    if let Some(p) = &app.workspace_parent {
        v.push(p.clone());
    }
    if let Some(w) = &app.workspace {
        v.push(w.root.clone());
    }
    v
}

fn letters(paths: &[PathBuf]) -> Vec<char> {
    paths
        .iter()
        .filter_map(|p| {
            let s = p.to_string_lossy();
            let mut c = s.chars();
            match (c.next(), c.next()) {
                (Some(l), Some(':')) if l.is_ascii_alphabetic() => Some(l.to_ascii_uppercase()),
                _ => None,
            }
        })
        .collect()
}

pub fn show(app: &mut App, ctx: &egui::Context) {
    let protected = protected_paths(app);
    let prot_letters = letters(&protected);
    let strict = guided(app);
    let disk_ok = app.usb.chosen_disk().is_some_and(|d| ineligible_for(d, &prot_letters, strict).is_none());
    let can_write = app.usb.source.is_some() && disk_ok && !app.busy();
    let done = app.usb.report.is_some();
    let next = if done { ("Write another", !app.busy()) } else { ("Write to USB", can_write) };
    if let Nav::Next = footer(ctx, "", false, Some(next)) {
        if done {
            app.usb.report = None;
            app.refresh_disks();
        } else {
            app.usb.confirm = true;
            app.usb.restore = false;
            app.usb.confirm_text.clear();
        }
    }
    central(ctx, |ui| {
        eyebrow(ui, "Write to USB");
        h1(ui, "Make a bootable USB drive");
        body(ui, "Puts Windows installation media on a USB stick that boots on BIOS and UEFI PCs. Everything on the stick is erased.");
        crate::help::tip(app, ui);
        ui.add_space(4.0);

        if let Some(r) = app.usb.report.clone() {
            done_view(app, ui, &r);
            return;
        }
        if app.busy() {
            job_panel(app, ui);
            show_error(app, ui);
            return;
        }

        crate::pages::two_columns(ui, |i, ui| {
            if i == 0 {
                let chosen = app.usb.chosen_disk().is_some() && disk_ok;
                let reserve =
                    30.0 + if chosen { 90.0 } else { 0.0 } + if app.last_error.is_some() { 110.0 } else { 0.0 };
                theme::fill_next(ui, reserve);
                source_card(app, ui);
                if let Some(d) = app.usb.chosen_disk().filter(|_| disk_ok) {
                    ui.add_space(4.0);
                    let mut lines = vec![d.label()];
                    if likely_backup_drive(d) {
                        lines.push(format!(
                            "{} is much bigger than a USB stick. Make sure this isn't a backup or external hard drive.",
                            human_bytes(d.size)
                        ));
                    }
                    theme::warn_box(ui, "Everything on this drive will be erased", &lines);
                }
                show_error(app, ui);
                log_tail(app, ui, 6);
            } else {
                drive_card(app, ui, &prot_letters);
                ui.add_space(4.0);
                theme::fill_next(ui, 0.0);
                options_card(app, ui);
            }
        });
    });
    if app.usb.confirm {
        confirm(app, ctx, protected);
    }
}

fn show_error(app: &App, ui: &mut egui::Ui) {
    if let Some(e) = &app.last_error {
        ui.add_space(4.0);
        if e.is_cancelled() {
            theme::warn_box(ui, "Stopped", std::slice::from_ref(&e.message));
        } else {
            theme::error_box(ui, &format!("Something went wrong ({})", e.code), &e.message);
        }
    }
}

fn guided(app: &App) -> bool {
    crate::pages::guided(app)
}

fn source_card(app: &mut App, ui: &mut egui::Ui) {
    let g = guided(app);
    theme::card().show(ui, |ui| {
        theme::card_body(ui);
        h2(ui, if g { "1. Which Windows file?" } else { "Source" });
        let shown = match &app.usb.source {
            Some(UsbSource::Iso(p)) => format!("ISO: {}", p.display()),
            Some(UsbSource::Folder(p)) => format!("Folder: {}", p.display()),
            None => "Nothing chosen".into(),
        };
        crate::pages::path_label(ui, &shown);
        ui.horizontal(|ui| {
            if theme::secondary(ui, "Choose ISO…", true).clicked() {
                if let Some(p) = rfd::FileDialog::new().add_filter("Disc image", &["iso"]).pick_file() {
                    app.usb.source = Some(UsbSource::Iso(p));
                }
            }
            if !g && theme::secondary(ui, "Choose folder…", true).clicked() {
                if let Some(p) = rfd::FileDialog::new().set_title("Choose a Windows media folder").pick_folder() {
                    app.usb.source = Some(UsbSource::Folder(p));
                }
            }
        });
        dim(
            ui,
            if g {
                "The .iso file you made with WinISOPilot, or one from Microsoft."
            } else {
                "ISO (mounted read-only) or an extracted media folder containing sources\\install.wim/.esd."
            },
        );
    });
}

fn drive_card(app: &mut App, ui: &mut egui::Ui, prot: &[char]) {
    theme::card().show(ui, |ui| {
        theme::card_body(ui);
        let g = guided(app);
        ui.horizontal(|ui| {
            h2(ui, if g { "2. Which USB stick?" } else { "Target disk" });
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if theme::secondary(ui, "Refresh", !app.busy()).clicked() {
                    app.refresh_disks();
                }
            });
        });
        let strict = g;
        let (ok, bad): (Vec<&Disk>, Vec<&Disk>) =
            app.usb.disks.iter().partition(|d| ineligible_for(d, prot, strict).is_none());
        if ok.is_empty() {
            body(ui, "No usable USB drive found. Plug in a USB drive of 8 GB or more, then select Refresh.");
        }
        let mut pick = app.usb.chosen;
        for d in &ok {
            ui.radio_value(&mut pick, Some(d.number), RichText::new(d.label()).size(13.0));
        }
        app.usb.chosen = pick;
        if !bad.is_empty() {
            egui::CollapsingHeader::new(format!("Other drives ({}) can't be used", bad.len())).show(ui, |ui| {
                for d in bad {
                    dim(ui, &format!("{} — {}", d.label(), ineligible_for(d, prot, strict).unwrap_or_default()));
                }
            });
        }
        dim(ui, "Internal disks and the disk Windows runs from are never listed as choices.");
        let chosen_ok = app.usb.chosen_disk().is_some_and(|d| ineligible_for(d, prot, strict).is_none());
        if ui
            .add_enabled(chosen_ok && !app.busy(), egui::Button::new("Restore to a normal drive…").small())
            .on_hover_text("Erase the chosen stick and make it one ordinary exFAT drive again (for files, not for installing Windows).")
            .clicked()
        {
            app.usb.confirm = true;
            app.usb.restore = true;
            app.usb.confirm_text.clear();
        }
    });
}

/// Optional second partition for files on FAT32 sticks over 32 GB.
fn data_partition_option(app: &mut App, ui: &mut egui::Ui, scheme: Scheme) {
    let Some(left) = app.usb.chosen_disk().and_then(|d| leftover_mb(d.size, scheme)) else { return };
    let size = human_bytes(left * 1024 * 1024);
    ui.checkbox(&mut app.usb.data_partition, format!("Use the remaining {size} as a second drive for your files"))
        .on_hover_text(
            "FAT32 setup partitions stop at 32 GB. The rest becomes an exFAT drive named DATA (Windows 10 1703 or later shows both).",
        );
}

fn options_card(app: &mut App, ui: &mut egui::Ui) {
    let g = guided(app);
    theme::card().show(ui, |ui| {
        theme::card_body(ui);
        if g {
            // Safe defaults for beginners: MBR + FAT32 boots on old and new PCs.
            h2(ui, "3. Check the copy?");
            ui.checkbox(&mut app.usb.verify, "Yes, check every file after copying (recommended)");
            dim(ui, "Takes a few extra minutes but catches faulty USB sticks.");
            data_partition_option(app, ui, Scheme::Mbr);
            return;
        }
        h2(ui, "Options");
        for sch in Scheme::ALL {
            if ui.radio_value(&mut app.usb.scheme, sch, sch.label()).on_hover_text(sch.hint()).changed() {
                app.usb.label = sanitize_label(&app.usb.label, sch);
            }
        }
        dim(ui, app.usb.scheme.hint());
        let limit = if app.usb.scheme.is_fat32() { 11 } else { 32 };
        ui.horizontal(|ui| {
            ui.label("Volume label:");
            let r = ui.add(egui::TextEdit::singleline(&mut app.usb.label).char_limit(limit).desired_width(200.0));
            if r.lost_focus() {
                app.usb.label = sanitize_label(&app.usb.label, app.usb.scheme);
            }
        });
        ui.checkbox(&mut app.usb.verify, "Verify (reads every copied file back from the drive, uncached)")
            .on_hover_text("Copies use write-through I/O, so progress reflects data actually on the drive.");
        let scheme = app.usb.scheme;
        data_partition_option(app, ui, scheme);
    });
}

fn confirm(app: &mut App, ctx: &egui::Context, protected: Vec<PathBuf>) {
    let Some(disk) = app.usb.chosen_disk().cloned() else {
        app.usb.confirm = false;
        return;
    };
    let mut go = false;
    egui::Modal::new(egui::Id::new("usb-confirm")).show(ctx, |ui| {
        ui.set_width(500.0);
        h2(ui, &format!("Erase Disk {}?", disk.number));
        body(ui, &disk.label());
        body(
            ui,
            if app.usb.restore {
                "All files and partitions on this drive will be permanently deleted, and it becomes one empty exFAT drive."
            } else {
                "All files and partitions on this drive will be permanently deleted."
            },
        );
        if likely_backup_drive(&disk) {
            ui.label(
                RichText::new(format!(
                    "This drive is {}: far bigger than a USB stick. If it holds backups, they will be lost.",
                    human_bytes(disk.size)
                ))
                .strong()
                .color(theme::ERR_TEXT),
            );
        }
        ui.add_space(3.0);
        ui.label(format!("Type {CONFIRM_WORD} to confirm:"));
        let r = ui.add(egui::TextEdit::singleline(&mut app.usb.confirm_text).desired_width(200.0));
        if ui.ctx().memory(|m| m.focused().is_none()) {
            r.request_focus();
        }
        let ready = app.usb.confirm_text.trim() == CONFIRM_WORD;
        ui.add_space(3.0);
        ui.horizontal(|ui| {
            if theme::secondary(ui, "Cancel", true).clicked() {
                app.usb.confirm = false;
            }
            if theme::primary(ui, if app.usb.restore { "Erase and restore" } else { "Erase and write" }, ready).clicked() {
                go = true;
            }
        });
    });
    if go && app.usb.restore {
        app.usb.confirm = false;
        app.usb.restore = false;
        let host = app.host.clone();
        app.start(Kind::RestoreUsb, move |job| {
            wip_engine::restore_usb(&disk, &protected, &host, job).map(Outcome::UsbRestored)
        });
        return;
    }
    if go {
        app.usb.confirm = false;
        let Some(source) = app.usb.source.clone() else { return };
        let g = guided(app);
        let req = UsbRequest {
            source,
            disk,
            scheme: if g { Scheme::Mbr } else { app.usb.scheme },
            label: if g { "WINSETUP".to_string() } else { sanitize_label(&app.usb.label, app.usb.scheme) },
            verify: app.usb.verify,
            data_partition: app.usb.data_partition,
            protected_paths: protected,
        };
        let host = app.host.clone();
        app.start(Kind::WriteUsb, move |job| wip_engine::write_usb(&req, &host, job).map(Outcome::UsbWritten));
    }
}

fn done_view(app: &mut App, ui: &mut egui::Ui, r: &UsbReport) {
    theme::card().show(ui, |ui| {
        theme::card_body(ui);
        h2(ui, &format!("USB drive {}: is ready", r.letter));
        check_line(ui, true, &format!("{} copied", human_bytes(r.bytes_written)));
        if r.split {
            check_line(ui, true, "Windows image split into install.swm parts for FAT32");
        }
        check_line(
            ui,
            r.verified,
            if r.verified { "Every copied file checked" } else { "Files not checked (verification was off)" },
        );
        if let Some(d) = &r.data_letter {
            check_line(ui, true, &format!("Extra drive {d}: (DATA) is free for your files"));
        }
        body(ui, "Eject the drive from the taskbar before unplugging it.");
    });
    if !r.warnings.is_empty() {
        ui.add_space(4.0);
        theme::warn_box(ui, "Notes", &r.warnings);
    }
    log_tail(app, ui, 6);
}
