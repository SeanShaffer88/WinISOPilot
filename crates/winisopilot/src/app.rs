//! Application state, navigation and frame loop.

use crate::jobs::{Kind, Outcome, Running};
use crate::theme;
use eframe::egui::{self, Color32, CornerRadius, Frame, Margin, RichText, Stroke, Vec2};
use std::collections::BTreeSet;
use std::io::Write;
use std::path::PathBuf;
use std::time::Instant;
use wip_core::fsutil::{atomic_write, read_json_or_quarantine};
use wip_core::inventory::Inventory;
use wip_core::journal::MountRecord;
use wip_core::plan::{Context, Selection};
use wip_core::raw::RawItem;
use wip_core::recipe::OutputSettings;
use wip_core::settings::{Mode, Settings};
use wip_core::{Catalog, Category, ImageSummary, Risk, WindowsFamily};
use wip_engine::{BuildReport, EngineError, HostInfo, SourceInfo, Workspace};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Step {
    Welcome,
    Source,
    Edition,
    Customize,
    Review,
    Build,
}

impl Step {
    pub const ALL: [Step; 6] = [Step::Welcome, Step::Source, Step::Edition, Step::Customize, Step::Review, Step::Build];
    pub fn number(self) -> usize {
        self as usize + 1
    }
    pub fn label(self) -> &'static str {
        match self {
            Step::Welcome => "Welcome",
            Step::Source => "Choose ISO",
            Step::Edition => "Edition",
            Step::Customize => "Customize",
            Step::Review => "Review",
            Step::Build => "Build",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Tool {
    Customize,
    Usb,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum View {
    Cat(Category),
    /// Uncatalogued items (Advanced mode only).
    Raw,
    /// Driver folders to add.
    Drivers,
    /// Updates, language packs and Features on Demand.
    Packages,
    /// Answers for Windows Setup (autounattend.xml).
    Answers,
}

/// Window size in points at 100% scale. The Scale setting multiplies it.
pub const BASE_SIZE: [f32; 2] = [960.0, 456.0];

pub struct App {
    pub tool: Tool,
    pub usb: crate::usb_page::UsbState,
    pub extras: crate::extras::ExtrasState,
    pub dl: crate::download_ui::DownloadState,
    pub settings: Settings,
    settings_path: PathBuf,
    pub catalog: Catalog,
    pub host: HostInfo,
    pub demo: bool,
    pub step: Step,
    pub max_step: Step,
    pub leftovers: Vec<MountRecord>,

    pub iso_path: Option<PathBuf>,
    pub workspace_parent: Option<PathBuf>,
    pub source: Option<SourceInfo>,
    pub chosen_index: Option<u32>,
    pub single_edition: bool,

    pub workspace: Option<Workspace>,
    pub inventory: Option<Inventory>,
    pub raw_items: Vec<RawItem>,

    pub selection: Selection,
    pub history: wip_core::history::History<Selection>,
    pub view: View,
    pub drivers: wip_core::drivers::DriverChoice,
    pub packages: wip_core::packages::PackageChoice,
    pub focused: Option<String>,
    pub search: String,
    pub risk_filter: Option<Risk>,
    pub hide_missing: bool,
    pub acks: BTreeSet<String>,
    pub output: OutputSettings,
    pub iso_out: Option<PathBuf>,

    pub job: Option<Running>,
    pub log: Vec<String>,
    log_file: Option<std::fs::File>,
    pub last_error: Option<EngineError>,
    pub report: Option<BuildReport>,

    pub show_settings: bool,
    /// The layout question is showing (first run, or every launch if enabled).
    pub mode_prompt: bool,
    /// "Don't ask again" box in the layout question.
    pub mode_prompt_dont_ask: bool,
    pub show_help: bool,
    window_title: String,
    free_cache: Option<(PathBuf, Instant, Option<u64>)>,
    pub confirm_cancel: bool,
    pub confirm_close: bool,
    allow_close: bool,
    pub toast: Option<(String, Instant)>,
}

/// Two-option switch used in the rail (ISO/USB, Guided/Advanced): side by
/// side in the Guided rail, stacked in the narrow Advanced rail. Returns the
/// newly picked index.
fn segmented(ui: &mut egui::Ui, compact: bool, opts: &[(&str, &str)], current: usize, busy: bool) -> Option<usize> {
    let mut pick = None;
    Frame::new()
        .fill(theme::RAIL_SELECTED.gamma_multiply(0.6))
        .corner_radius(CornerRadius::same(8))
        .inner_margin(Margin::same(3))
        .show(ui, |ui| {
            ui.spacing_mut().item_spacing = Vec2::splat(3.0);
            ui.spacing_mut().button_padding = Vec2::new(2.0, 4.0);
            let mut button = |ui: &mut egui::Ui, i: usize, w: f32| {
                let (label, hint) = opts[i];
                let on = i == current;
                let text =
                    RichText::new(label).size(11.5).strong().color(if on { theme::TEXT } else { theme::RAIL_MUTED });
                let b = egui::Button::new(text)
                    .fill(if on { theme::PRIMARY } else { Color32::TRANSPARENT })
                    .stroke(Stroke::NONE)
                    .corner_radius(CornerRadius::same(6))
                    .min_size(Vec2::new(w, 26.0));
                let r = ui.add_enabled(!busy || on, b).on_hover_text(hint);
                r.widget_info(|| egui::WidgetInfo::selected(egui::WidgetType::RadioButton, true, on, label));
                if r.clicked() && !on {
                    pick = Some(i);
                }
            };
            if compact {
                ui.vertical(|ui| {
                    let w = ui.available_width();
                    for i in 0..opts.len() {
                        button(ui, i, w);
                    }
                });
            } else {
                ui.horizontal(|ui| {
                    let w = (ui.available_width() - 3.0 * (opts.len() - 1) as f32) / opts.len() as f32;
                    for i in 0..opts.len() {
                        button(ui, i, w);
                    }
                });
            }
        });
    pick
}

impl App {
    pub fn new(cc: &eframe::CreationContext<'_>, demo: bool) -> App {
        let data = wip_engine::app_data_dir();
        let settings_path = data.join("settings.json");
        let settings: Settings = read_json_or_quarantine(&settings_path).ok().flatten().unwrap_or_default();
        let settings = settings.sanitized();
        theme::apply(&cc.egui_ctx, &settings);
        let catalog = Catalog::builtin().expect("the built-in catalog is validated by tests");
        let dism_folder = (!settings.dism_folder.is_empty()).then(|| PathBuf::from(&settings.dism_folder));
        let host = wip_engine::probe_host(dism_folder.as_deref());
        let log_file = {
            let dir = data.join("logs");
            let _ = std::fs::create_dir_all(&dir);
            std::fs::OpenOptions::new()
                .create(true)
                .append(true)
                .open(dir.join(format!("session-{}.log", wip_core::journal::now_secs())))
                .ok()
        };
        let workspace_parent = (!settings.workspace_root.is_empty()).then(|| PathBuf::from(&settings.workspace_root));
        let mut app = App {
            tool: Tool::Customize,
            usb: Default::default(),
            extras: Default::default(),
            dl: Default::default(),
            settings,
            settings_path,
            catalog,
            host,
            demo,
            step: Step::Welcome,
            max_step: Step::Source,
            leftovers: wip_engine::leftover_mounts(),
            iso_path: None,
            workspace_parent,
            source: None,
            chosen_index: None,
            single_edition: true,
            workspace: None,
            inventory: None,
            raw_items: Vec::new(),
            selection: Selection::new(),
            history: wip_core::history::History::new(200),
            view: View::Cat(Category::Apps),
            drivers: wip_core::drivers::DriverChoice::new(),
            packages: wip_core::packages::PackageChoice::default(),
            focused: None,
            search: String::new(),
            risk_filter: None,
            hide_missing: true,
            acks: BTreeSet::new(),
            output: OutputSettings::default(),
            iso_out: None,
            job: None,
            log: Vec::new(),
            log_file,
            last_error: None,
            report: None,
            show_settings: false,
            mode_prompt: false,
            mode_prompt_dont_ask: false,
            show_help: false,
            window_title: String::new(),
            free_cache: None,
            confirm_cancel: false,
            confirm_close: false,
            allow_close: false,
            toast: None,
        };
        app.mode_prompt = app.settings.mode.is_none() || app.settings.ask_mode_on_launch;
        if demo {
            crate::dialogs::load_demo(&mut app);
        }
        app
    }

    pub fn save_settings(&mut self) {
        let json = serde_json::to_string_pretty(&self.settings).unwrap_or_default();
        if let Err(e) = atomic_write(&self.settings_path, json.as_bytes()) {
            self.notify(format!("Couldn't save settings: {e}"));
        }
    }

    /// Keeps the window the same size in points whatever the scale, so the
    /// whole window grows or shrinks with its contents. Also picks up
    /// Ctrl + / Ctrl − / Ctrl 0, which change egui's zoom directly.
    fn sync_scale(&mut self, ctx: &egui::Context) {
        let zoom = ctx.zoom_factor();
        if (zoom - self.settings.text_scale).abs() > 0.001 {
            // Biggest scale whose window still fits the screen.
            let fit = ctx
                .input(|i| i.viewport().monitor_size)
                .map(|m| zoom * (m.x / BASE_SIZE[0]).min((m.y - 48.0) / BASE_SIZE[1]))
                .unwrap_or(2.0);
            let z = ((zoom * 20.0).round() / 20.0).clamp(0.75, fit.clamp(0.75, 2.0));
            self.settings.text_scale = z;
            if (z - zoom).abs() > 0.001 {
                ctx.set_zoom_factor(z);
            }
            self.save_settings();
        }
        let id = egui::Id::new("applied_pixels_per_point");
        let ppp = ctx.pixels_per_point();
        if ctx.data(|d| d.get_temp::<f32>(id)) != Some(ppp) {
            ctx.data_mut(|d| d.insert_temp(id, ppp));
            ctx.send_viewport_cmd(egui::ViewportCommand::InnerSize(egui::Vec2::from(BASE_SIZE)));
        }
    }

    pub fn mode(&self) -> Mode {
        self.settings.mode.unwrap_or(Mode::Guided)
    }

    pub fn notify(&mut self, msg: impl Into<String>) {
        self.toast = Some((msg.into(), Instant::now()));
    }

    /// Saves one redacted diagnostics text file where the user chooses.
    pub fn save_diagnostics(&mut self) {
        use wip_core::diagnostics::{bundle, redact, tail};
        let name = format!("WinISOPilot-diagnostics-{}.txt", wip_core::APP_VERSION);
        let Some(dest) = rfd::FileDialog::new()
            .set_title("Save diagnostics")
            .set_file_name(&name)
            .add_filter("Text", &["txt"])
            .save_file()
        else {
            return;
        };
        let mut sections: Vec<(String, String)> = Vec::new();
        sections.push(("Host".into(), serde_json::to_string_pretty(&self.host).unwrap_or_default()));
        sections.push(("Settings".into(), serde_json::to_string_pretty(&self.settings).unwrap_or_default()));
        if let Some(e) = &self.last_error {
            sections.push(("Last error".into(), format!("{}: {}", e.code, e.message)));
        }
        if let Some(e) = self.edition() {
            sections.push(("Edition".into(), format!("{} {} {}", e.name, e.version_string(), e.architecture)));
        }
        if let Some(ctx) = self.plan_ctx().filter(|c| c.inventory.is_some()) {
            if let Ok(p) = self.compile_plan(&ctx) {
                let lines: Vec<String> = p
                    .install
                    .iter()
                    .map(|o| format!("[install] {}", o.op.describe()))
                    .chain(p.boot.iter().map(|o| format!("[boot] {}", o.op.describe())))
                    .collect();
                sections.push(("Plan".into(), lines.join("\n")));
            }
        }
        sections.push(("Session log".into(), self.log[self.log.len().saturating_sub(400)..].join("\n")));
        let logs = wip_engine::app_data_dir().join("logs");
        if let Ok(entries) = std::fs::read_dir(&logs) {
            let mut files: Vec<_> = entries.flatten().map(|e| e.path()).filter(|p| p.is_file()).collect();
            files.sort();
            for f in files.iter().rev().take(6) {
                if let Ok(bytes) = std::fs::read(f) {
                    let text = String::from_utf8_lossy(&bytes);
                    let name = f.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
                    sections.push((format!("File {name}"), tail(&text, 256 * 1024).to_string()));
                }
            }
        }
        let mut pairs: Vec<(String, &str)> = Vec::new();
        if let Some(p) = std::env::var_os("USERPROFILE").or_else(|| std::env::var_os("HOME")) {
            pairs.push((p.to_string_lossy().into_owned(), "%USERPROFILE%"));
        }
        if let Some(u) = std::env::var_os("USERNAME").or_else(|| std::env::var_os("USER")) {
            pairs.push((u.to_string_lossy().into_owned(), "<user>"));
        }
        if let Some(c) = std::env::var_os("COMPUTERNAME") {
            pairs.push((c.to_string_lossy().into_owned(), "<pc>"));
        }
        let text = redact(&bundle(&sections), &pairs);
        match wip_core::fsutil::atomic_write(&dest, text.as_bytes()) {
            Ok(()) => self.notify(format!("Saved {}. Review it before sharing.", dest.display())),
            Err(e) => self.notify(format!("Couldn't save diagnostics: {e}")),
        }
    }

    pub fn log_line(&mut self, line: String) {
        if let Some(f) = self.log_file.as_mut() {
            let _ = writeln!(f, "{line}");
        }
        if let Some(ws) = &self.workspace {
            if let Ok(mut f) = std::fs::OpenOptions::new().create(true).append(true).open(&ws.log_file) {
                let _ = writeln!(f, "{line}");
            }
        }
        self.log.push(line);
        if self.log.len() > 5000 {
            self.log.drain(..1000);
        }
    }

    /// The edition being customized.
    pub fn edition(&self) -> Option<&ImageSummary> {
        if let Some(ws) = &self.workspace {
            return Some(&ws.edition);
        }
        let idx = self.chosen_index?;
        self.source.as_ref()?.images.iter().find(|i| i.index == idx)
    }

    pub fn plan_ctx(&self) -> Option<Context<'_>> {
        let e = self.edition()?;
        let family = e.family().unwrap_or(WindowsFamily::Win11);
        Some(Context { catalog: &self.catalog, family, build: e.build, inventory: self.inventory.as_ref() })
    }

    /// The selection compiled for the open image, plus driver operations.
    pub fn compile_plan(&self, ctx: &Context) -> Result<wip_core::plan::CompiledPlan, String> {
        let mut p = wip_core::plan::compile(&self.selection, ctx)?;
        let arch = crate::drivers_ui::arch(self);
        self.drivers.extend_plan(&mut p, &arch);
        self.packages.extend_plan(&mut p, &arch);
        Ok(p)
    }

    pub fn busy(&self) -> bool {
        self.job.is_some()
    }

    pub fn goto(&mut self, s: Step) {
        if s > self.max_step {
            self.max_step = s;
        }
        self.step = s;
        self.last_error = None;
    }

    // ---- selection editing with undo ----

    fn push_undo(&mut self) {
        self.history.record(&self.selection);
    }

    /// Free space at `dir`, refreshed at most every 5 seconds.
    pub fn free_space_cached(&mut self, dir: &std::path::Path) -> Option<u64> {
        if let Some((p, at, v)) = &self.free_cache {
            if p == dir && at.elapsed().as_secs() < 5 {
                return *v;
            }
        }
        let v = wip_engine::free_space(dir);
        self.free_cache = Some((dir.to_path_buf(), Instant::now(), v));
        v
    }

    pub fn set_selected(&mut self, id: &str, on: bool) {
        if self.selection.contains(id) == on {
            return;
        }
        self.push_undo();
        if on {
            self.selection.insert(id.to_string());
        } else {
            self.selection.remove(id);
            self.acks.remove(id);
        }
    }

    pub fn replace_selection(&mut self, sel: Selection) {
        self.push_undo();
        self.acks.retain(|a| sel.contains(a));
        self.selection = sel;
    }

    pub fn undo(&mut self) {
        if let Some(prev) = self.history.undo(&self.selection) {
            self.acks.retain(|a| prev.contains(a));
            self.selection = prev;
            self.notify("Undid the last change. Ctrl+Y redoes it.");
        }
    }

    pub fn redo(&mut self) {
        if let Some(next) = self.history.redo(&self.selection) {
            self.acks.retain(|a| next.contains(a));
            self.selection = next;
            self.notify("Redid the change.");
        }
    }

    // ---- jobs ----

    pub fn start(
        &mut self,
        kind: Kind,
        f: impl FnOnce(&wip_engine::Job) -> Result<Outcome, EngineError> + Send + 'static,
    ) {
        if self.job.is_some() {
            return;
        }
        self.last_error = None;
        self.log_line(format!("--- {} ---", kind.title()));
        self.job = Some(Running::spawn(kind, f));
    }

    fn pump_job(&mut self, ctx: &egui::Context) {
        let Some(job) = self.job.as_mut() else { return };
        let lines = job.poll();
        let done = job.finished();
        for l in lines {
            self.log_line(l);
        }
        if !done {
            ctx.request_repaint_after(std::time::Duration::from_millis(100));
            return;
        }
        let mut job = self.job.take().expect("present");
        for l in job.poll() {
            self.log_line(l);
        }
        let kind = job.kind;
        // Long jobs: flash the taskbar button if the window isn't in front.
        if job.started.elapsed().as_secs() > 20 && !ctx.input(|i| i.viewport().focused.unwrap_or(true)) {
            ctx.send_viewport_cmd(egui::ViewportCommand::RequestUserAttention(egui::UserAttentionType::Informational));
        }
        match job.take_result() {
            Ok(out) => self.on_outcome(out),
            Err(e) => {
                self.log_line(format!("Error: {e}"));
                if kind == Kind::Prepare && !e.is_cancelled() {
                    // A failed prepare may leave a mount; it's in the journal.
                    self.leftovers = wip_engine::leftover_mounts();
                }
                self.last_error = Some(e);
            }
        }
    }

    fn on_outcome(&mut self, out: Outcome) {
        match out {
            Outcome::Inspected(src) => {
                self.chosen_index = src
                    .images
                    .iter()
                    .find(|i| i.support().is_ok() && i.edition_id.eq_ignore_ascii_case("Professional"))
                    .or_else(|| src.images.iter().find(|i| i.support().is_ok()))
                    .map(|i| i.index);
                self.source = Some(src);
                self.goto(Step::Edition);
            }
            Outcome::Prepared(ws, inv) => {
                self.raw_items = wip_core::raw::items(&inv, &self.catalog);
                self.inventory = Some(*inv);
                self.workspace = Some(*ws);
                self.view = View::Cat(Category::Apps);
                self.goto(Step::Customize);
                crate::extras::check_pending_recipe(self);
            }
            Outcome::Built(report) => {
                let report = *report;
                self.report = Some(report);
                self.workspace = None;
                self.inventory = None;
            }
            Outcome::Discarded => {
                self.workspace = None;
                self.inventory = None;
                self.raw_items.clear();
                self.report = None;
                self.max_step = Step::Edition;
                self.goto(Step::Edition);
                self.notify("The open image was discarded. Your choices are kept.");
            }
            Outcome::Disks(d) => self.usb.set_disks(d),
            Outcome::UsbWritten(r) => self.usb.report = Some(r),
            Outcome::UsbRestored(letter) => {
                self.notify(format!("Drive {letter}: is a normal USB drive again (exFAT)."));
                self.refresh_disks();
            }
            Outcome::BootTested(r) => self.extras.boot_report = Some(r),
            Outcome::Catalog(entries, win10) => self.dl.set_entries(entries, win10),
            Outcome::Downloaded(iso) => {
                self.notify(format!("Downloaded and saved {}. Select Read ISO to continue.", iso.display()));
                self.iso_path = Some(iso);
                self.source = None;
                self.chosen_index = None;
            }
            Outcome::AdkInstalled => {
                self.recheck_host();
                if self.host.oscdimg.is_some() {
                    self.notify("Deployment Tools installed. WinISOPilot can now make ISO files.");
                } else {
                    self.notify("The installer finished, but oscdimg.exe still wasn't found. Restart WinISOPilot.");
                }
            }
            Outcome::LeftoverCleaned(dir) => {
                self.leftovers.retain(|l| l.mount_dir != dir);
                self.notify("Cleaned up the earlier session.");
            }
        }
    }

    pub fn discard_workspace(&mut self) {
        let Some(ws) = self.workspace.clone() else { return };
        let host = self.host.clone();
        self.start(Kind::Discard, move |job| wip_engine::discard(&ws, &host, job).map(|_| Outcome::Discarded));
    }

    // ---- frame ----

    fn handle_close(&mut self, ctx: &egui::Context) {
        if ctx.input(|i| i.viewport().close_requested())
            && !self.allow_close
            && (self.busy() || self.workspace.is_some())
        {
            ctx.send_viewport_cmd(egui::ViewportCommand::CancelClose);
            self.confirm_close = true;
        }
    }

    pub fn really_close(&mut self, ctx: &egui::Context) {
        self.allow_close = true;
        ctx.send_viewport_cmd(egui::ViewportCommand::Close);
    }

    fn rail(&mut self, ctx: &egui::Context) {
        let advanced = true; // one slim rail in both layouts
        let width = 72.0;
        egui::SidePanel::left("rail")
            .exact_width(width)
            .resizable(false)
            .frame(Frame::new().fill(theme::RAIL).inner_margin(Margin::symmetric(if advanced { 6 } else { 14 }, 8)))
            .show(ctx, |ui| {
                if !advanced {
                    ui.horizontal(|ui| {
                        ui.label(RichText::new("WinISOPilot").size(16.0).strong().color(theme::TEXT));
                        if self.demo {
                            ui.label(RichText::new("DEMO").strong().color(theme::WARN_TEXT));
                        }
                    });
                    ui.add_space(10.0);
                }
                self.tool_switch(ui, advanced);
                ui.add_space(6.0);
                self.mode_switch(ui, advanced);
                ui.add_space(10.0);
                let steps: &[Step] = if self.tool == Tool::Customize { &Step::ALL } else { &[] };
                ui.spacing_mut().item_spacing.y = 4.0;
                for &s in steps {
                    self.rail_step(ui, s, advanced);
                    if s == Step::Customize && self.step == Step::Customize && !advanced {
                        self.rail_categories(ui);
                    }
                }
                ui.with_layout(egui::Layout::bottom_up(egui::Align::LEFT), |ui| {
                    let mut settings = false;
                    let mut help = false;
                    let button = |ui: &mut egui::Ui, text: &str, name: &str, hint: &str, w: f32| {
                        let b = egui::Button::new(RichText::new(text).size(13.0).color(theme::TEXT)).frame(false);
                        let r = ui.add_sized(Vec2::new(w, 36.0), b).on_hover_text(hint);
                        r.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Button, true, name));
                        r.clicked()
                    };
                    if advanced {
                        let w = ui.available_width();
                        settings = button(ui, "⚙", "Settings", "Settings", w);
                        help = button(ui, "?", "Help", "Help for this page (F1)", w);
                    } else {
                        ui.horizontal(|ui| {
                            let w = (ui.available_width() - 8.0) / 2.0;
                            help = button(ui, "?  Help", "Help", "Help for this page (F1)", w);
                            settings = button(ui, "⚙  Settings", "Settings", "Settings", w);
                        });
                    }
                    if settings {
                        self.show_settings = true;
                    }
                    if help {
                        self.show_help = !self.show_help;
                    }
                });
            });
    }

    /// ISO / USB switch.
    fn tool_switch(&mut self, ui: &mut egui::Ui, compact: bool) {
        let opts = [("ISO", "Customize an ISO"), ("USB", "Write to a USB drive")];
        let cur = if self.tool == Tool::Customize { 0 } else { 1 };
        if let Some(i) = segmented(ui, compact, &opts, cur, self.busy()) {
            self.switch_tool(if i == 0 { Tool::Customize } else { Tool::Usb });
        }
    }

    /// Guided / Advanced switch, right under ISO / USB.
    fn mode_switch(&mut self, ui: &mut egui::Ui, compact: bool) {
        let opts = [("Guided", "Simple questions, safe choices"), ("Advanced", "Every detail and option")];
        let cur = if self.mode() == Mode::Guided { 0 } else { 1 };
        if let Some(i) = segmented(ui, compact, &opts, cur, false) {
            self.settings.mode = Some(if i == 0 { Mode::Guided } else { Mode::Advanced });
            self.save_settings();
            self.layout_changed();
        }
    }

    /// Called after the layout (Guided/Advanced) changes: back to step 1 of
    /// Customize ISO. Everything entered so far (ISO, working folder, opened
    /// edition, ticked changes, search text, output settings) is shared by
    /// both layouts and kept. While a task runs the page stays put, so its
    /// progress stays in view.
    pub fn layout_changed(&mut self) {
        if self.busy() {
            return;
        }
        self.tool = Tool::Customize;
        self.step = Step::Welcome;
        self.last_error = None;
        if let View::Raw = self.view {
            if self.mode() == Mode::Guided {
                self.view = View::Cat(Category::Apps);
            }
        }
    }

    /// Looks for DISM and oscdimg again (after installing the ADK).
    pub fn recheck_host(&mut self) {
        let dism_folder = (!self.settings.dism_folder.is_empty()).then(|| PathBuf::from(&self.settings.dism_folder));
        self.host = wip_engine::probe_host(dism_folder.as_deref());
    }

    pub fn start_adk_install(&mut self) {
        self.start(Kind::InstallAdk, |job| wip_engine::install_adk(job).map(|_| Outcome::AdkInstalled));
    }

    pub fn switch_tool(&mut self, t: Tool) {
        self.tool = t;
        self.last_error = None;
        if t == Tool::Usb {
            self.usb.default_source(self.report.as_ref(), self.iso_path.as_deref());
            if !self.usb.listed && !self.busy() {
                self.refresh_disks();
            }
        }
    }

    pub fn refresh_disks(&mut self) {
        self.usb.listed = true;
        if self.demo {
            self.usb.set_disks(crate::dialogs::demo_disks());
            return;
        }
        self.start(Kind::ListDisks, |job| wip_engine::list_disks(job).map(Outcome::Disks));
    }

    fn rail_step(&mut self, ui: &mut egui::Ui, s: Step, compact: bool) {
        let current = s == self.step;
        let done = s < self.max_step && !current && s < self.step.max(self.max_step);
        let reachable = s <= self.max_step && !self.busy();
        let (circle_fill, circle_text, text_color) = if current || done {
            (theme::TEXT, theme::RAIL, theme::TEXT)
        } else {
            (Color32::TRANSPARENT, theme::RAIL_MUTED, theme::RAIL_MUTED)
        };
        let glyph = if done { "✔".to_string() } else { s.number().to_string() };
        let size = Vec2::new(ui.available_width(), if compact { 32.0 } else { 36.0 });
        let (rect, resp) = ui.allocate_exact_size(size, egui::Sense::click());
        let resp = resp.on_hover_text(format!("Step {}: {}", s.number(), s.label()));
        resp.widget_info(|| {
            egui::WidgetInfo::labeled(
                egui::WidgetType::Button,
                reachable,
                format!("Step {}: {}", s.number(), s.label()),
            )
        });
        let p = ui.painter();
        if current && compact {
            p.rect(
                rect.shrink(2.0),
                CornerRadius::same(8),
                theme::RAIL_SELECTED,
                Stroke::new(2.0_f32, theme::ACCENT),
                egui::StrokeKind::Inside,
            );
        }
        let center = if compact { rect.center() } else { egui::pos2(rect.left() + 14.0, rect.center().y) };
        let r = 11.0;
        p.circle(
            center,
            r,
            circle_fill,
            Stroke::new(2.0_f32, if current || done { theme::TEXT } else { theme::RAIL_MUTED }),
        );
        if current && !compact {
            p.circle_stroke(center, r + 3.5, Stroke::new(2.0_f32, theme::ACCENT));
        }
        p.text(center, egui::Align2::CENTER_CENTER, glyph, egui::FontId::proportional(13.0), circle_text);
        if !compact {
            let font = egui::FontId::proportional(15.0);
            p.text(
                egui::pos2(rect.left() + 38.0, rect.center().y),
                egui::Align2::LEFT_CENTER,
                s.label(),
                font,
                text_color,
            );
        }
        if resp.has_focus() {
            p.rect_stroke(rect, CornerRadius::same(6), Stroke::new(2.0_f32, theme::ACCENT), egui::StrokeKind::Inside);
        }
        if resp.clicked() && reachable && s != self.step {
            self.goto(s);
        }
    }

    fn rail_categories(&mut self, ui: &mut egui::Ui) {
        let cats = self.visible_categories();
        ui.indent("cats", |ui| {
            ui.spacing_mut().item_spacing.y = 2.0;
            for c in cats {
                let selected = self.view == View::Cat(c);
                let n = self.selected_in(c);
                let text = if n > 0 { format!("{}  · {n}", c.label()) } else { c.label().to_string() };
                let b = egui::Button::new(RichText::new(text).size(12.5).color(if selected {
                    theme::TEXT
                } else {
                    theme::RAIL_MUTED
                }))
                .fill(if selected { theme::RAIL_SELECTED } else { Color32::TRANSPARENT })
                .stroke(Stroke::NONE)
                .min_size(Vec2::new(ui.available_width(), 27.0));
                if ui.add(b).clicked() {
                    self.view = View::Cat(c);
                    self.focused = None;
                }
            }
            let extras = [
                (View::Packages, "Updates", crate::packages_ui::count(self)),
                (View::Drivers, "Drivers", crate::drivers_ui::count(self)),
                (View::Answers, "Setup answers", crate::answers_ui::count(self)),
            ];
            for (view, label, n) in extras {
                let selected = self.view == view;
                let text = if n > 0 { format!("{label}  · {n}") } else { label.to_string() };
                let b = egui::Button::new(RichText::new(text).size(12.5).color(if selected {
                    theme::TEXT
                } else {
                    theme::RAIL_MUTED
                }))
                .fill(if selected { theme::RAIL_SELECTED } else { Color32::TRANSPARENT })
                .stroke(Stroke::NONE)
                .min_size(Vec2::new(ui.available_width(), 27.0));
                if ui.add(b).clicked() {
                    self.view = view;
                    self.focused = None;
                }
            }
        });
        ui.add_space(6.0);
    }

    /// Categories with at least one option for the current edition.
    pub fn visible_categories(&self) -> Vec<Category> {
        let Some(ctx) = self.plan_ctx() else { return Category::ALL.to_vec() };
        Category::ALL
            .into_iter()
            .filter(|c| self.catalog.in_category(*c).any(|o| o.applies_to(ctx.family, ctx.build)))
            .collect()
    }

    pub fn selected_in(&self, c: Category) -> usize {
        self.selection.iter().filter(|id| self.catalog.get(id).is_some_and(|o| o.category == c)).count()
    }

    fn toast(&mut self, ctx: &egui::Context) {
        if let Some((msg, at)) = &self.toast {
            if at.elapsed().as_secs() >= 5 {
                self.toast = None;
                return;
            }
            let msg = msg.clone();
            egui::Area::new(egui::Id::new("toast")).anchor(egui::Align2::CENTER_BOTTOM, Vec2::new(0.0, -90.0)).show(
                ctx,
                |ui| {
                    Frame::new()
                        .fill(theme::CHROME)
                        .stroke(Stroke::new(1.0_f32, theme::BOUNDARY))
                        .corner_radius(CornerRadius::same(8))
                        .inner_margin(Margin::symmetric(16, 10))
                        .show(ui, |ui| ui.label(RichText::new(msg).color(theme::TEXT)));
                },
            );
            ctx.request_repaint_after(std::time::Duration::from_millis(500));
        }
    }
}

impl eframe::App for App {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        self.pump_job(ctx);
        self.handle_close(ctx);
        self.sync_scale(ctx);
        if !self.busy() {
            let redo = ctx.input_mut(|i| {
                i.consume_key(egui::Modifiers::COMMAND, egui::Key::Y)
                    || i.consume_key(egui::Modifiers::COMMAND | egui::Modifiers::SHIFT, egui::Key::Z)
            });
            if redo {
                self.redo();
            } else if ctx.input_mut(|i| i.consume_key(egui::Modifiers::COMMAND, egui::Key::Z)) {
                self.undo();
            }
        }
        let mut title = match (self.tool, self.step) {
            (Tool::Usb, _) => "WinISOPilot — Write to USB".to_string(),
            (_, s) => format!("WinISOPilot — {} (step {} of 6)", s.label(), s.number()),
        };
        if self.demo {
            title.push_str(" [DEMO]");
        }
        if self.window_title != title {
            ctx.send_viewport_cmd(egui::ViewportCommand::Title(title.clone()));
            self.window_title = title;
        }
        self.rail(ctx);
        if self.tool == Tool::Usb {
            crate::usb_page::show(self, ctx);
            crate::dialogs::show(self, ctx);
            crate::help::window(self, ctx);
            self.toast(ctx);
            return;
        }
        match self.step {
            Step::Customize if self.inventory.is_some() || self.demo => crate::customize::show(self, ctx),
            Step::Customize => crate::pages::need_prepare(self, ctx),
            Step::Welcome => crate::pages::welcome(self, ctx),
            Step::Source => crate::pages::source(self, ctx),
            Step::Edition => crate::pages::edition(self, ctx),
            Step::Review => crate::pages::review(self, ctx),
            Step::Build => crate::pages::build(self, ctx),
        }
        crate::dialogs::show(self, ctx);
        crate::extras::port_dialog(self, ctx);
        crate::help::window(self, ctx);
        self.toast(ctx);
    }
}
