//! Selection checking and compilation into concrete operations.

use crate::catalog::{Action, Catalog, CatalogOption, Hive, RegData, Risk};
use crate::inventory::{Inventory, PackageState};
use crate::WindowsFamily;
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;

pub type Selection = BTreeSet<String>;

#[derive(Debug, Clone, Copy)]
pub struct Context<'a> {
    pub catalog: &'a Catalog,
    pub family: WindowsFamily,
    pub build: u32,
    /// `None` until the image has been analyzed.
    pub inventory: Option<&'a Inventory>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Availability {
    Available,
    /// The image is already in the requested state.
    AlreadyDone(String),
    NotInImage(String),
    NotForThisWindows(String),
}

impl Availability {
    pub fn is_available(&self) -> bool {
        matches!(self, Availability::Available)
    }

    pub fn reason(&self) -> Option<&str> {
        match self {
            Availability::Available => None,
            Availability::AlreadyDone(r) | Availability::NotInImage(r) | Availability::NotForThisWindows(r) => Some(r),
        }
    }
}

enum ActionState {
    Work,
    Done,
    Missing(String),
}

fn action_state(a: &Action, inv: &Inventory) -> ActionState {
    match a {
        Action::RemoveAppx { prefix } => {
            if inv.appx_matching(prefix).next().is_some() {
                ActionState::Work
            } else {
                ActionState::Missing(format!("No app named {prefix} in this image."))
            }
        }
        Action::RemoveCapability { prefix } => {
            let mut any = false;
            for c in inv.capabilities_matching(prefix) {
                any = true;
                if c.state.is_on() {
                    return ActionState::Work;
                }
            }
            if any {
                ActionState::Done
            } else {
                ActionState::Missing(format!("Capability {prefix} is not in this image."))
            }
        }
        Action::DisableFeature { name, .. } => match inv.feature(name) {
            Some(f) if f.state.is_on() => ActionState::Work,
            Some(_) => ActionState::Done,
            None => ActionState::Missing(format!("Feature {name} is not in this image.")),
        },
        Action::EnableFeature { name, source_sxs } => match inv.feature(name) {
            Some(f) if f.state.is_on() => ActionState::Done,
            Some(f) if *source_sxs && f.state == PackageState::Removed && !inv.has_sxs => {
                ActionState::Missing(format!("{name} needs the sources\\sxs folder, which this media doesn't have."))
            }
            Some(f) if f.state.is_off() => ActionState::Work,
            Some(_) => ActionState::Missing(format!("{name} is in an unexpected state.")),
            None => ActionState::Missing(format!("Feature {name} is not in this image.")),
        },
        Action::ServiceStart { name, .. } => {
            if inv.has_service(name) {
                ActionState::Work
            } else {
                ActionState::Missing(format!("Service {name} is not in this image."))
            }
        }
        Action::RegSet { .. }
        | Action::RegDelete { .. }
        | Action::DeleteFile { .. }
        | Action::ComponentCleanup { .. } => ActionState::Work,
    }
}

pub fn availability(opt: &CatalogOption, ctx: &Context) -> Availability {
    if !opt.applies_to(ctx.family, ctx.build) {
        return Availability::NotForThisWindows(format!(
            "Doesn't apply to {} build {}.",
            ctx.family.label(),
            ctx.build
        ));
    }
    let Some(inv) = ctx.inventory else {
        return Availability::Available;
    };
    let mut done = 0;
    let mut missing = None;
    for a in &opt.actions {
        match action_state(a, inv) {
            ActionState::Work => return Availability::Available,
            ActionState::Done => done += 1,
            ActionState::Missing(r) => {
                missing.get_or_insert(r);
            }
        }
    }
    if done > 0 {
        Availability::AlreadyDone("This image is already set up this way.".into())
    } else {
        Availability::NotInImage(missing.unwrap_or_else(|| "Not in this image.".into()))
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub enum Severity {
    Warning,
    Error,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Issue {
    pub severity: Severity,
    pub option: Option<String>,
    pub message: String,
}

impl Issue {
    fn error(option: &str, message: String) -> Issue {
        Issue { severity: Severity::Error, option: Some(option.into()), message }
    }
    fn warning(option: &str, message: String) -> Issue {
        Issue { severity: Severity::Warning, option: Some(option.into()), message }
    }
}

/// Checks a selection. Errors block building; warnings are shown on Review.
pub fn validate(sel: &Selection, ctx: &Context) -> Vec<Issue> {
    let mut issues = Vec::new();
    for id in sel {
        if crate::raw::is_raw(id) {
            let title = crate::raw::title(id);
            if crate::raw::parse(id).is_none() {
                issues.push(Issue::error(id, format!("Unknown option '{id}'.")));
            } else if !ctx.inventory.is_some_and(|inv| crate::raw::available(id, inv)) {
                issues.push(Issue::warning(id, format!("{title}: skipped. Not present in this image.")));
            } else {
                issues.push(Issue::warning(id, format!("{title} has no safety rating. Confirm on the Review page.")));
            }
            continue;
        }
        let Some(opt) = ctx.catalog.get(id) else {
            issues.push(Issue::error(id, format!("Unknown option '{id}'.")));
            continue;
        };
        match availability(opt, ctx) {
            Availability::Available => {}
            other => issues
                .push(Issue::warning(id, format!("{}: skipped. {}", opt.title, other.reason().unwrap_or_default()))),
        }
        for r in &opt.requires {
            if !sel.contains(r) {
                let name = ctx.catalog.get(r).map_or(r.as_str(), |o| o.title.as_str());
                issues.push(Issue::error(id, format!("{} also needs \"{name}\".", opt.title)));
            }
        }
        for c in &opt.conflicts {
            if sel.contains(c) {
                let name = ctx.catalog.get(c).map_or(c.as_str(), |o| o.title.as_str());
                issues.push(Issue::error(id, format!("\"{}\" and \"{name}\" can't both be chosen.", opt.title)));
            }
        }
        if opt.risk == Risk::High {
            issues.push(Issue::warning(id, format!("{} is high risk. Confirm on the Review page.", opt.title)));
        }
    }
    issues
}

pub fn has_errors(issues: &[Issue]) -> bool {
    issues.iter().any(|i| i.severity == Severity::Error)
}

/// Selected IDs that need an explicit acknowledgment (high risk or
/// unrated raw items), with a display title.
pub fn needs_ack(sel: &Selection, catalog: &Catalog) -> Vec<(String, String)> {
    sel.iter()
        .filter_map(|id| {
            if crate::raw::is_raw(id) {
                return Some((id.clone(), crate::raw::title(id)));
            }
            catalog.get(id).filter(|o| o.risk == Risk::High).map(|o| (id.clone(), o.title.clone()))
        })
        .collect()
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "op", rename_all = "snake_case")]
pub enum Op {
    RemoveAppx { package_name: String, display_name: String },
    RemoveCapability { name: String },
    DisableFeature { name: String, remove_payload: bool },
    EnableFeature { name: String, source_sxs: bool },
    DeleteFile { path: String },
    LoadHive { hive: Hive },
    RegSet { hive: Hive, key: String, value: String, data: RegData },
    RegDelete { hive: Hive, key: String, value: String },
    UnloadHive { hive: Hive },
    ComponentCleanup { reset_base: bool },
}

impl Op {
    pub fn describe(&self) -> String {
        match self {
            Op::RemoveAppx { display_name, .. } => format!("Removing app {display_name}"),
            Op::RemoveCapability { name } => format!("Removing capability {name}"),
            Op::DisableFeature { name, .. } => format!("Turning off feature {name}"),
            Op::EnableFeature { name, .. } => format!("Turning on feature {name}"),
            Op::DeleteFile { path } => format!("Deleting {path}"),
            Op::LoadHive { hive } => format!("Opening registry hive {hive:?}"),
            Op::RegSet { key, value, .. } => {
                let v = if value.is_empty() { "(Default)" } else { value };
                format!("Setting {key}\\{v}")
            }
            Op::RegDelete { key, value, .. } => format!("Deleting {key}\\{value}"),
            Op::UnloadHive { hive } => format!("Closing registry hive {hive:?}"),
            Op::ComponentCleanup { reset_base: true } => "Cleaning component store (ResetBase)".into(),
            Op::ComponentCleanup { reset_base: false } => "Cleaning component store".into(),
        }
    }

    /// Relative cost weight for overall progress.
    pub fn weight(&self) -> u32 {
        match self {
            Op::ComponentCleanup { .. } => 40,
            Op::EnableFeature { .. } => 15,
            Op::RemoveCapability { .. } | Op::DisableFeature { .. } => 6,
            Op::RemoveAppx { .. } => 3,
            _ => 1,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PlannedOp {
    pub origin: String,
    pub op: Op,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct CompiledPlan {
    /// Operations on the mounted install image, in execution order.
    pub install: Vec<PlannedOp>,
    /// Operations on boot.wim index 2 (Windows Setup).
    pub boot: Vec<PlannedOp>,
    /// Options that were selected but have nothing to do, with the reason.
    pub skipped: Vec<(String, String)>,
}

impl CompiledPlan {
    pub fn is_empty(&self) -> bool {
        self.install.is_empty() && self.boot.is_empty()
    }
}

/// Turns a validated selection into ordered operations. Requires an
/// inventory, because app and capability names are resolved against it.
pub fn compile(sel: &Selection, ctx: &Context) -> Result<CompiledPlan, String> {
    let inv = ctx.inventory.ok_or("The image must be analyzed before building.")?;
    let issues = validate(sel, ctx);
    if let Some(e) = issues.iter().find(|i| i.severity == Severity::Error) {
        return Err(e.message.clone());
    }

    let mut appx = Vec::new();
    let mut caps = Vec::new();
    let mut disable = Vec::new();
    let mut enable = Vec::new();
    let mut files = Vec::new();
    let mut regs: Vec<(Hive, PlannedOp)> = Vec::new();
    let mut cleanup: Option<(String, bool)> = None;
    let mut plan = CompiledPlan::default();

    // Catalog order gives a stable, reviewable execution order.
    for opt in ctx.catalog.options.iter().filter(|o| sel.contains(&o.id)) {
        let avail = availability(opt, ctx);
        if !avail.is_available() {
            plan.skipped.push((opt.id.clone(), avail.reason().unwrap_or_default().to_string()));
            continue;
        }
        let origin = || opt.id.clone();
        for a in &opt.actions {
            match a {
                Action::RemoveAppx { prefix } => {
                    for p in inv.appx_matching(prefix) {
                        appx.push(PlannedOp {
                            origin: origin(),
                            op: Op::RemoveAppx {
                                package_name: p.package_name.clone(),
                                display_name: p.display_name.clone(),
                            },
                        });
                    }
                }
                Action::RemoveCapability { prefix } => {
                    for c in inv.capabilities_matching(prefix).filter(|c| c.state.is_on()) {
                        caps.push(PlannedOp { origin: origin(), op: Op::RemoveCapability { name: c.name.clone() } });
                    }
                }
                Action::DisableFeature { name, remove_payload } => {
                    if let Some(f) = inv.feature(name).filter(|f| f.state.is_on()) {
                        disable.push(PlannedOp {
                            origin: origin(),
                            op: Op::DisableFeature { name: f.name.clone(), remove_payload: *remove_payload },
                        });
                    }
                }
                Action::EnableFeature { name, source_sxs } => {
                    if let Some(f) = inv.feature(name).filter(|f| f.state.is_off()) {
                        enable.push(PlannedOp {
                            origin: origin(),
                            op: Op::EnableFeature { name: f.name.clone(), source_sxs: *source_sxs },
                        });
                    }
                }
                Action::ServiceStart { name, start } => {
                    if inv.has_service(name) {
                        regs.push((
                            Hive::System,
                            PlannedOp {
                                origin: origin(),
                                op: Op::RegSet {
                                    hive: Hive::System,
                                    key: format!(r"ControlSet001\Services\{name}"),
                                    value: "Start".into(),
                                    data: RegData::Dword(start.value()),
                                },
                            },
                        ));
                    }
                }
                Action::RegSet { hive, key, value, data } => regs.push((
                    *hive,
                    PlannedOp {
                        origin: origin(),
                        op: Op::RegSet { hive: *hive, key: key.clone(), value: value.clone(), data: data.clone() },
                    },
                )),
                Action::RegDelete { hive, key, value } => regs.push((
                    *hive,
                    PlannedOp {
                        origin: origin(),
                        op: Op::RegDelete { hive: *hive, key: key.clone(), value: value.clone() },
                    },
                )),
                Action::DeleteFile { path } => {
                    files.push(PlannedOp { origin: origin(), op: Op::DeleteFile { path: path.clone() } })
                }
                Action::ComponentCleanup { reset_base } => {
                    let rb = cleanup.as_ref().is_some_and(|c| c.1) || *reset_base;
                    cleanup = Some((opt.id.clone(), rb));
                }
            }
        }
    }

    for id in sel.iter().filter(|id| crate::raw::is_raw(id)) {
        if !crate::raw::available(id, inv) {
            plan.skipped.push((id.clone(), "Not present in this image.".into()));
            continue;
        }
        let origin = id.clone();
        match crate::raw::parse(id) {
            Some((crate::raw::RawKind::Appx, n)) => {
                for p in inv.appx.iter().filter(|a| a.display_name.eq_ignore_ascii_case(n)) {
                    appx.push(PlannedOp {
                        origin: origin.clone(),
                        op: Op::RemoveAppx {
                            package_name: p.package_name.clone(),
                            display_name: p.display_name.clone(),
                        },
                    });
                }
            }
            Some((crate::raw::RawKind::Capability, n)) => {
                caps.push(PlannedOp { origin, op: Op::RemoveCapability { name: n.to_string() } })
            }
            Some((crate::raw::RawKind::FeatureOff, n)) => disable
                .push(PlannedOp { origin, op: Op::DisableFeature { name: n.to_string(), remove_payload: false } }),
            Some((crate::raw::RawKind::FeatureOn, n)) => {
                enable.push(PlannedOp { origin, op: Op::EnableFeature { name: n.to_string(), source_sxs: false } })
            }
            None => {}
        }
    }

    let mut install = Vec::new();
    for group in [appx, caps, disable, enable, files] {
        push_unique(&mut install, group);
    }
    for hive in [Hive::Software, Hive::System, Hive::DefaultUser] {
        push_hive_group(&mut install, hive, &regs);
    }
    if let Some((origin, reset_base)) = cleanup {
        install.push(PlannedOp { origin, op: Op::ComponentCleanup { reset_base } });
    }
    let mut boot = Vec::new();
    push_hive_group(&mut boot, Hive::BootSystem, &regs);

    plan.install = install;
    plan.boot = boot;
    Ok(plan)
}

fn push_unique(out: &mut Vec<PlannedOp>, items: Vec<PlannedOp>) {
    for item in items {
        if !out.iter().any(|o| o.op == item.op) {
            out.push(item);
        }
    }
}

fn push_hive_group(out: &mut Vec<PlannedOp>, hive: Hive, regs: &[(Hive, PlannedOp)]) {
    let ops: Vec<PlannedOp> = regs.iter().filter(|(h, _)| *h == hive).map(|(_, o)| o.clone()).collect();
    if ops.is_empty() {
        return;
    }
    out.push(PlannedOp { origin: String::new(), op: Op::LoadHive { hive } });
    push_unique(out, ops);
    out.push(PlannedOp { origin: String::new(), op: Op::UnloadHive { hive } });
}
