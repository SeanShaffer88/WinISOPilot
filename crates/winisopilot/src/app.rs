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
pub enum View {
    Cat(Category),
    /// Uncatalogued items (Advanced mode only).
    Raw,
}

pub struct App {
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
    pub undo_stack: Vec<Selection>,
    pub view: View,
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
    pub confirm_cancel: bool,
    pub confirm_close: bool,
    allow_close: bool,
    pub toast: Option<(String, Instant)>,
}

impl App {
    pub fn new(cc: &eframe::CreationContext<'_>, demo: bool) -> App {
        let data = wip_engine::app_data_dir();
        let settings_path = data.join("settings.json");
        let settings: Settings = read_json_or_quarantine(&settings_path).ok().flatten().unwrap_or_default();
        let settings = settings.sanitized();
        theme::apply(&cc.egui_ctx, settings.text_scale);
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
            undo_stack: Vec::new(),
            view: View::Cat(Category::Apps),
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
            confirm_cancel: false,
            confirm_close: false,
            allow_close: false,
            toast: None,
        };
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

    pub fn mode(&self) -> Mode {
        self.settings.mode.unwrap_or(Mode::Guided)
    }

    pub fn notify(&mut self, msg: impl Into<String>) {
        self.toast = Some((msg.into(), Instant::now()));
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
        self.undo_stack.push(self.selection.clone());
        if self.undo_stack.len() > 200 {
            self.undo_stack.remove(0);
        }
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
        if let Some(prev) = self.undo_stack.pop() {
            self.acks.retain(|a| prev.contains(a));
            self.selection = prev;
            self.notify("Undid the last change.");
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
            }
            Outcome::Built(report) => {
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
        let advanced = self.mode() == Mode::Advanced;
        let width = if advanced { 64.0 } else { 232.0 };
        egui::SidePanel::left("rail")
            .exact_width(width)
            .resizable(false)
            .frame(Frame::new().fill(theme::RAIL).inner_margin(Margin::symmetric(if advanced { 10 } else { 18 }, 22)))
            .show(ctx, |ui| {
                if !advanced {
                    ui.label(RichText::new("WinISOPilot").size(21.0).strong().color(theme::TEXT));
                    ui.add_space(14.0);
                }
                for s in Step::ALL {
                    self.rail_step(ui, s, advanced);
                    if s == Step::Customize && self.step == Step::Customize && !advanced {
                        self.rail_categories(ui);
                    }
                }
                ui.with_layout(egui::Layout::bottom_up(egui::Align::LEFT), |ui| {
                    let label = if advanced { "⚙" } else { "⚙  Settings" };
                    let b = egui::Button::new(RichText::new(label).size(15.0).color(theme::TEXT)).frame(false);
                    let r = ui.add_sized(Vec2::new(ui.available_width(), 36.0), b).on_hover_text("Settings");
                    if r.clicked() {
                        self.show_settings = true;
                    }
                    if self.demo {
                        ui.label(RichText::new("DEMO DATA").strong().color(theme::WARN_TEXT));
                    }
                });
            });
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
        let size = Vec2::new(ui.available_width(), if compact { 44.0 } else { 40.0 });
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
        let r = 13.0;
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
            for c in cats {
                let selected = self.view == View::Cat(c);
                let n = self.selected_in(c);
                let text = if n > 0 { format!("{}  · {n}", c.label()) } else { c.label().to_string() };
                let b = egui::Button::new(RichText::new(text).size(14.0).color(if selected {
                    theme::TEXT
                } else {
                    theme::RAIL_MUTED
                }))
                .fill(if selected { theme::RAIL_SELECTED } else { Color32::TRANSPARENT })
                .stroke(Stroke::NONE)
                .min_size(Vec2::new(ui.available_width(), 30.0));
                if ui.add(b).clicked() {
                    self.view = View::Cat(c);
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
        if ctx.input_mut(|i| i.consume_key(egui::Modifiers::COMMAND, egui::Key::Z)) && !self.busy() {
            self.undo();
        }
        self.rail(ctx);
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
        self.toast(ctx);
    }
}
