//! Recipe porting: check a saved recipe against a different Windows build
//! and report, per option, whether it still does the same thing.

use crate::catalog::{Action, Catalog};
use crate::inventory::Inventory;
use crate::plan::{availability, Availability, Context, Selection};
use crate::raw::{self, RawKind};
use crate::recipe::LoadedRecipe;
use std::collections::{BTreeMap, BTreeSet};

/// Strips the trailing `~version` from a capability name, so
/// `Language.Speech~~~en-US~0.0.1.0` and `…~0.0.2.0` compare equal.
pub fn capability_family(name: &str) -> String {
    match name.rsplit_once('~') {
        Some((head, _)) if !head.is_empty() => head.to_lowercase(),
        _ => name.to_lowercase(),
    }
}

fn action_targets(a: &Action, inv: &Inventory) -> Vec<String> {
    match a {
        Action::RemoveAppx { prefix } => {
            inv.appx_matching(prefix).map(|p| format!("app:{}", p.display_name.to_lowercase())).collect()
        }
        Action::RemoveCapability { prefix } => inv
            .capabilities_matching(prefix)
            .filter(|c| c.state.is_on())
            .map(|c| format!("cap:{}", capability_family(&c.name)))
            .collect(),
        Action::DisableFeature { name, .. } | Action::EnableFeature { name, .. } => {
            inv.feature(name).map(|f| vec![format!("feat:{}", f.name.to_lowercase())]).unwrap_or_default()
        }
        Action::ServiceStart { name, .. } => {
            if inv.has_service(name) {
                vec![format!("svc:{}", name.to_lowercase())]
            } else {
                Vec::new()
            }
        }
        Action::RegSet { hive, key, value, .. } | Action::RegDelete { hive, key, value } => {
            vec![format!("reg:{hive:?}\\{key}\\{value}").to_lowercase()]
        }
        Action::DeleteFile { path } => vec![format!("file:{}", path.to_lowercase())],
        Action::ComponentCleanup { reset_base } => vec![format!("cleanup:{reset_base}")],
    }
}

fn raw_targets(id: &str, inv: &Inventory) -> Vec<String> {
    if !raw::available(id, inv) {
        return Vec::new();
    }
    match raw::parse(id) {
        Some((RawKind::Appx, n)) => vec![format!("app:{}", n.to_lowercase())],
        Some((RawKind::Capability, n)) => vec![format!("cap:{}", capability_family(n))],
        Some((RawKind::FeatureOff | RawKind::FeatureOn, n)) => vec![format!("feat:{}", n.to_lowercase())],
        None => Vec::new(),
    }
}

/// What every selected option touches in this image, normalized so that
/// version-only differences don't count as changes.
pub fn resolve(sel: &Selection, ctx: &Context) -> BTreeMap<String, Vec<String>> {
    let mut out = BTreeMap::new();
    let Some(inv) = ctx.inventory else { return out };
    for id in sel {
        let targets: BTreeSet<String> = if raw::is_raw(id) {
            raw_targets(id, inv).into_iter().collect()
        } else if let Some(o) = ctx.catalog.get(id) {
            o.actions.iter().flat_map(|a| action_targets(a, inv)).collect()
        } else {
            continue;
        };
        out.insert(id.clone(), targets.into_iter().collect());
    }
    out
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PortStatus {
    /// Does exactly what it did in the original image.
    Same,
    /// Still applies but touches different items now.
    Changed { gone: Vec<String>, new: Vec<String> },
    /// Nothing to do in this image.
    Missing(String),
    /// Applies, but the recipe has no record to compare with (old format).
    Unverified,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PortItem {
    pub id: String,
    pub title: String,
    pub status: PortStatus,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PortReport {
    pub from_build: Option<u32>,
    pub to_build: u32,
    pub items: Vec<PortItem>,
    /// Catalog options added since the recipe was saved that apply here.
    pub new_options: Vec<String>,
    /// IDs this version doesn't know.
    pub unknown: Vec<String>,
}

impl PortReport {
    pub fn count(&self, f: impl Fn(&PortStatus) -> bool) -> usize {
        self.items.iter().filter(|i| f(&i.status)).count()
    }

    /// True when nothing needs the user's attention.
    pub fn is_clean(&self) -> bool {
        self.items.iter().all(|i| i.status == PortStatus::Same)
            && self.new_options.is_empty()
            && self.unknown.is_empty()
    }

    /// Selection with options that do nothing here removed.
    pub fn applicable_selection(&self) -> Selection {
        self.items.iter().filter(|i| !matches!(i.status, PortStatus::Missing(_))).map(|i| i.id.clone()).collect()
    }
}

fn title(id: &str, catalog: &Catalog) -> String {
    if raw::is_raw(id) {
        raw::title(id)
    } else {
        catalog.get(id).map_or_else(|| id.to_string(), |o| o.title.clone())
    }
}

/// Compares a loaded recipe with the current image. Needs an inventory.
pub fn port(rec: &LoadedRecipe, ctx: &Context) -> PortReport {
    let now = resolve(&rec.selection, ctx);
    let mut items = Vec::new();
    for id in &rec.selection {
        let current: Vec<String> = now.get(id).cloned().unwrap_or_default();
        let available = if raw::is_raw(id) {
            ctx.inventory.is_some_and(|inv| raw::available(id, inv))
        } else {
            ctx.catalog.get(id).is_some_and(|o| availability(o, ctx).is_available())
        };
        let status = if !available {
            let reason = if raw::is_raw(id) {
                "Not in this image.".to_string()
            } else {
                ctx.catalog
                    .get(id)
                    .and_then(|o| match availability(o, ctx) {
                        Availability::Available => None,
                        a => a.reason().map(str::to_string),
                    })
                    .unwrap_or_else(|| "Not in this image.".into())
            };
            PortStatus::Missing(reason)
        } else {
            match rec.resolved.get(id) {
                None => PortStatus::Unverified,
                Some(before) => {
                    let b: BTreeSet<&String> = before.iter().collect();
                    let a: BTreeSet<&String> = current.iter().collect();
                    if a == b {
                        PortStatus::Same
                    } else {
                        PortStatus::Changed {
                            gone: b.difference(&a).map(|s| s.to_string()).collect(),
                            new: a.difference(&b).map(|s| s.to_string()).collect(),
                        }
                    }
                }
            }
        };
        items.push(PortItem { id: id.clone(), title: title(id, ctx.catalog), status });
    }
    let known: BTreeSet<&String> = rec.known_options.iter().collect();
    let new_options = if known.is_empty() {
        Vec::new()
    } else {
        ctx.catalog
            .options
            .iter()
            .filter(|o| !known.contains(&o.id) && availability(o, ctx).is_available())
            .map(|o| o.id.clone())
            .collect()
    };
    PortReport {
        from_build: rec.source.as_ref().map(|s| s.build),
        to_build: ctx.build,
        items,
        new_options,
        unknown: rec.unknown.clone(),
    }
}
