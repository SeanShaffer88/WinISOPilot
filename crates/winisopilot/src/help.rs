//! Context help: a one-line tip at the top of each page and a Help panel
//! (F1) that explains the current page, what happens next, and shortcuts.

use crate::app::{App, Step, Tool};
use crate::theme::{self, body, dim, h2};
use eframe::egui::{self, RichText};
use wip_core::settings::Mode;

pub struct Topic {
    pub title: &'static str,
    /// One sentence shown under the page heading when tips are on.
    pub tip: &'static str,
    pub paragraphs: &'static [&'static str],
    pub next: &'static str,
}

pub fn topic(app: &App) -> Topic {
    if app.tool == Tool::Usb {
        return Topic {
            title: "Write to USB",
            tip: "Tip: plug in the USB drive first, then select Refresh. Only USB drives are ever offered.",
            paragraphs: &[
                "Choose what to write: an ISO (for example one you just built) or a folder with Windows setup files.",
                "Choose the USB drive. Internal disks, the disk Windows runs from, and the drive holding your ISO or working folder are never offered.",
                "MBR works on almost every PC. Choose GPT only if the PC uses UEFI without legacy support and you know you need it.",
                "Checking every file afterwards takes a few extra minutes but catches faulty USB drives.",
            ],
            next: "You'll be asked to type ERASE. Then the drive is erased, formatted, filled with Windows setup, made bootable, and optionally checked.",
        };
    }
    match app.step {
        Step::Welcome => Topic {
            title: "Welcome",
            tip: "Tip: you can come back to any finished step from the list on the left.",
            paragraphs: &[
                "WinISOPilot makes a new Windows installer with your changes. Your original ISO is never modified.",
                "The checks show whether this PC has what's needed. Missing items come with a link to fix them.",
                "Prefer to just make a bootable USB from an existing ISO? Choose Write to USB at the top of the left panel.",
            ],
            next: "Next you choose or download a Windows ISO.",
        },
        Step::Source => Topic {
            title: "Choose a Windows ISO",
            tip: "Tip: don't have an ISO? Use Download Windows for me on the right.",
            paragraphs: &[
                "Choose an official Windows 10 or 11 ISO, or download the current release from Microsoft inside the app.",
                "The working folder holds a temporary copy of Windows while you customize it. Use a fast local drive with plenty of space; the needed amount is shown.",
            ],
            next: "Read ISO lists the editions inside. Nothing is changed.",
        },
        Step::Edition => Topic {
            title: "Pick the edition",
            tip: "Tip: not sure? Pick the edition printed on your PC's sticker or shown in Settings > System > About.",
            paragraphs: &[
                "An ISO usually contains several editions (Home, Pro…). Pick the one you'll install.",
                "Keeping only this edition makes the ISO smaller and skips Setup's edition question.",
            ],
            next: "Open edition copies the files and reads what's inside the edition. This takes a few minutes.",
        },
        Step::Customize if app.mode() == Mode::Advanced => Topic {
            title: "Customize (Advanced)",
            tip: "Tip: Ctrl+F searches, Ctrl+Z undoes, and the inspector on the right shows exactly what each option changes.",
            paragraphs: &[
                "Every option found in this image is listed by section. Select a row to see its details in the inspector.",
                "Unrated items come straight from the image and have no description. Each one must be confirmed on Review.",
                "Use the Risk filter to focus, and Presets to start from a common set.",
            ],
            next: "Review shows every change in one list before anything is applied.",
        },
        Step::Customize => Topic {
            title: "Customize",
            tip: "Tip: click an option's name to read what it does. Not sure? Leave it unticked.",
            paragraphs: &[
                "Tick the changes you want. Anything left unticked stays as Microsoft ships it.",
                "Each option has a risk badge. Safe options don't affect how Windows works for most people; higher risks list what could stop working.",
                "Riskier options are hidden. \"Show them\" (or Settings) reveals more. Quick picks tick a sensible set in one click.",
                "Made a mistake? Ctrl+Z undoes it, or \"Untick all here\" clears the page.",
            ],
            next: "Use Next to move through the sections, then Review before building.",
        },
        Step::Review => Topic {
            title: "Review",
            tip: "Tip: remove anything you're unsure about with ✖. Nothing has been changed yet.",
            paragraphs: &[
                "This is every change that will be applied, grouped by section.",
                "Risky and unrated changes need a tick to confirm you understand them.",
                "Save choices keeps this list in a small file so you can reuse it on another ISO or a newer Windows.",
            ],
            next: "Build applies the changes to the private copy, saves it, and writes the new ISO. You can cancel between steps.",
        },
        Step::Build => Topic {
            title: "Build and results",
            tip: "Tip: test the new ISO in a virtual machine before installing it on a PC you rely on.",
            paragraphs: &[
                "While building, each stage is shown with progress. Cancel stops after the current step without saving.",
                "When it's done you'll see measured results, and you can test the ISO in a virtual machine or write it to a USB drive.",
            ],
            next: "Start another goes back to the Edition step with your choices kept.",
        },
    }
}

pub const SHORTCUTS: &[(&str, &str)] = &[
    ("F1", "Show or hide this help"),
    ("Alt+Right / Alt+Left", "Next / Back"),
    ("Ctrl+Z / Ctrl+Y", "Undo / Redo a change to your choices"),
    ("Ctrl+F", "Search options (Customize)"),
    ("Tab / Shift+Tab", "Move between controls"),
    ("Space / Enter", "Tick a box or press a button"),
    ("Ctrl + / Ctrl −", "Bigger / smaller text"),
    ("PageUp / PageDown", "Previous / next page of a list"),
];

/// One-line tip under the page heading.
pub fn tip(app: &App, ui: &mut egui::Ui) {
    // Tips are for beginners; Advanced pages carry their own technical subtitle.
    if app.settings.show_tips && (app.mode() == Mode::Guided || app.step == Step::Customize) {
        ui.label(RichText::new(topic(app).tip).size(13.0).italics().color(theme::TEXT_DIM));
    }
}

pub fn window(app: &mut App, ctx: &egui::Context) {
    if ctx.input_mut(|i| i.consume_key(egui::Modifiers::NONE, egui::Key::F1)) {
        app.show_help = !app.show_help;
    }
    if !app.show_help {
        return;
    }
    let t = topic(app);
    let mut open = true;
    egui::Window::new("Help")
        .open(&mut open)
        .collapsible(false)
        .resizable(true)
        .default_width(420.0)
        .anchor(egui::Align2::RIGHT_TOP, egui::vec2(-16.0, 16.0))
        .show(ctx, |ui| {
            h2(ui, t.title);
            for p in t.paragraphs {
                body(ui, p);
            }
            ui.add_space(6.0);
            ui.label(RichText::new("What happens next").strong());
            body(ui, t.next);
            ui.add_space(8.0);
            ui.separator();
            ui.label(RichText::new("Keyboard shortcuts").strong());
            egui::Grid::new("keys").num_columns(2).spacing([16.0, 4.0]).show(ui, |ui| {
                for (k, what) in SHORTCUTS {
                    ui.label(RichText::new(*k).monospace());
                    ui.label(*what);
                    ui.end_row();
                }
            });
            ui.add_space(8.0);
            dim(ui, "More: the user guide (docs/user-guide.md) and the log folder in Settings.");
        });
    if !open {
        app.show_help = false;
    }
}
