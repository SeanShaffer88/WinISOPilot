//! The "Setup answers" page: an optional autounattend.xml so Windows Setup
//! skips questions. Setup always still asks which disk to use.

use crate::app::App;
use crate::theme::{self, body, dim};
use eframe::egui::{self, RichText, Ui};
use wip_core::unattend::TIME_ZONES;

pub fn count(app: &App) -> usize {
    usize::from(app.output.answers.enabled)
}

pub fn panel(app: &mut App, ui: &mut Ui, guided: bool) {
    if guided {
        body(ui, "Optional. Answer Windows Setup's questions now so installing needs fewer clicks. Setup will still ask which disk to install to, so nothing is erased without you choosing it.");
    } else {
        dim(ui, "Writes autounattend.xml to the media root: International-Core(-WinPE), Setup UserData/AcceptEula, Shell-Setup OOBE/LocalAccounts/TimeZone. No DiskConfiguration, product key or ProtectYourPC. The VM boot test can't run on media with an answer file.");
    }
    ui.add_space(4.0);
    let a = &mut app.output.answers;
    ui.checkbox(&mut a.enabled, RichText::new("Answer Setup's questions for me").strong());
    if !a.enabled {
        return;
    }
    let langs: Vec<String> = app.edition().map(|e| e.languages.clone()).unwrap_or_default();
    let a = &mut app.output.answers;
    theme::card().show(ui, |ui| {
        ui.set_width(ui.available_width());
        egui::Grid::new("answers").num_columns(2).spacing([12.0, 6.0]).show(ui, |ui| {
            ui.label("Language and region");
            ui.horizontal(|ui| {
                let shown = if a.language.is_empty() { "Ask during Setup".to_string() } else { a.language.clone() };
                egui::ComboBox::from_id_salt("lang").selected_text(shown).show_ui(ui, |ui| {
                    ui.selectable_value(&mut a.language, String::new(), "Ask during Setup");
                    for l in &langs {
                        ui.selectable_value(&mut a.language, l.clone(), l);
                    }
                });
                if !guided {
                    ui.add(egui::TextEdit::singleline(&mut a.language).desired_width(80.0).hint_text("en-US"));
                }
            });
            ui.end_row();
            if !guided {
                ui.label("Keyboard");
                ui.add(egui::TextEdit::singleline(&mut a.keyboard).desired_width(140.0).hint_text("same as language"))
                    .on_hover_text("A language tag (en-GB) or a keyboard ID (0809:00000809)");
                ui.end_row();
            }
            ui.label("Time zone");
            ui.horizontal(|ui| {
                let shown = if a.time_zone.is_empty() { "Ask during Setup".to_string() } else { a.time_zone.clone() };
                egui::ComboBox::from_id_salt("tz").selected_text(shown).height(260.0).show_ui(ui, |ui| {
                    ui.selectable_value(&mut a.time_zone, String::new(), "Ask during Setup");
                    for z in TIME_ZONES {
                        ui.selectable_value(&mut a.time_zone, z.to_string(), *z);
                    }
                });
                if !guided {
                    ui.add(egui::TextEdit::singleline(&mut a.time_zone).desired_width(160.0));
                }
            });
            ui.end_row();
            ui.label("Local account");
            ui.add(egui::TextEdit::singleline(&mut a.account_name).desired_width(160.0).hint_text("leave empty to ask"))
                .on_hover_text("Creates this administrator account and skips the Microsoft account screens.");
            ui.end_row();
            if a.creates_account() {
                ui.label("Password");
                ui.add(egui::TextEdit::singleline(&mut a.account_password).password(true).desired_width(160.0).hint_text("optional"))
                    .on_hover_text("Stored in plain text on the USB stick / ISO. Windows deletes its copy at first sign-in. Never saved in recipes.");
                ui.end_row();
            }
            ui.label("Other");
            ui.horizontal(|ui| {
                ui.checkbox(&mut a.accept_eula, "Accept the license terms");
                if !guided {
                    ui.checkbox(&mut a.hide_wireless_setup, "Skip the Wi-Fi page");
                }
            });
            ui.end_row();
        });
    });
    let problems = app.output.answers.validate();
    if problems.is_empty() {
        dim(ui, &app.output.answers.summary().join(" · "));
    } else {
        for p in problems {
            ui.label(RichText::new(p).color(theme::ERR_TEXT));
        }
    }
}
