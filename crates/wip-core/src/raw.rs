//! "Raw" items for Advanced mode: anything in the image's inventory that the
//! catalog doesn't describe. They have no plain-language description, so
//! they are always treated as unrated and need explicit confirmation.

use crate::catalog::{Action, Catalog};
use crate::inventory::{starts_with_ci, Inventory};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum RawKind {
    Appx,
    Capability,
    FeatureOff,
    FeatureOn,
}

impl RawKind {
    fn prefix(self) -> &'static str {
        match self {
            RawKind::Appx => "raw.appx:",
            RawKind::Capability => "raw.cap:",
            RawKind::FeatureOff => "raw.featoff:",
            RawKind::FeatureOn => "raw.featon:",
        }
    }

    pub fn verb(self) -> &'static str {
        match self {
            RawKind::Appx => "Remove app",
            RawKind::Capability => "Remove capability",
            RawKind::FeatureOff => "Turn off feature",
            RawKind::FeatureOn => "Turn on feature",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RawItem {
    pub id: String,
    pub kind: RawKind,
    pub name: String,
}

/// Packages other apps depend on, plus the Windows Security UI. Never
/// offered for removal.
pub const PROTECTED_APPX: &[&str] = &[
    "Microsoft.VCLibs",
    "Microsoft.NET.Native",
    "Microsoft.UI.Xaml",
    "Microsoft.WindowsAppRuntime",
    "Microsoft.Services.Store.Engagement",
    "Microsoft.SecHealthUI",
];

pub fn is_protected_appx(display_name: &str) -> bool {
    PROTECTED_APPX.iter().any(|p| starts_with_ci(display_name, p))
}

pub fn make_id(kind: RawKind, name: &str) -> String {
    format!("{}{name}", kind.prefix())
}

pub fn parse(id: &str) -> Option<(RawKind, &str)> {
    [RawKind::Appx, RawKind::Capability, RawKind::FeatureOff, RawKind::FeatureOn]
        .into_iter()
        .find_map(|k| id.strip_prefix(k.prefix()).map(|n| (k, n)))
        .filter(|(_, n)| !n.trim().is_empty() && n.len() <= 260)
}

pub fn is_raw(id: &str) -> bool {
    id.starts_with("raw.")
}

pub fn title(id: &str) -> String {
    match parse(id) {
        Some((k, n)) => format!("{} {n}", k.verb()),
        None => id.to_string(),
    }
}

/// Whether the item exists in the inventory in a state where the change
/// would do something.
pub fn available(id: &str, inv: &Inventory) -> bool {
    match parse(id) {
        Some((RawKind::Appx, n)) => {
            !is_protected_appx(n) && inv.appx.iter().any(|a| a.display_name.eq_ignore_ascii_case(n))
        }
        Some((RawKind::Capability, n)) => {
            inv.capabilities.iter().any(|c| c.name.eq_ignore_ascii_case(n) && c.state.is_on())
        }
        Some((RawKind::FeatureOff, n)) => inv.feature(n).is_some_and(|f| f.state.is_on()),
        Some((RawKind::FeatureOn, n)) => inv.feature(n).is_some_and(|f| f.state.is_off()),
        None => false,
    }
}

fn covered(catalog: &Catalog, kind: RawKind, name: &str) -> bool {
    catalog.options.iter().flat_map(|o| &o.actions).any(|a| match (kind, a) {
        (RawKind::Appx, Action::RemoveAppx { prefix }) => starts_with_ci(name, prefix),
        (RawKind::Capability, Action::RemoveCapability { prefix }) => starts_with_ci(name, prefix),
        (RawKind::FeatureOff, Action::DisableFeature { name: n, .. })
        | (RawKind::FeatureOn, Action::EnableFeature { name: n, .. }) => n.eq_ignore_ascii_case(name),
        _ => false,
    })
}

/// Every uncatalogued, actionable item in the image, sorted by name.
pub fn items(inv: &Inventory, catalog: &Catalog) -> Vec<RawItem> {
    let mut out = Vec::new();
    let mut push = |kind: RawKind, name: &str| {
        if !covered(catalog, kind, name) {
            out.push(RawItem { id: make_id(kind, name), kind, name: name.to_string() });
        }
    };
    for a in &inv.appx {
        if !is_protected_appx(&a.display_name) {
            push(RawKind::Appx, &a.display_name);
        }
    }
    for c in inv.capabilities.iter().filter(|c| c.state.is_on()) {
        push(RawKind::Capability, &c.name);
    }
    for f in &inv.features {
        if f.state.is_on() {
            push(RawKind::FeatureOff, &f.name);
        } else if f.state.is_off() {
            push(RawKind::FeatureOn, &f.name);
        }
    }
    out.sort_by(|a, b| a.kind.cmp(&b.kind).then_with(|| a.name.to_lowercase().cmp(&b.name.to_lowercase())));
    out.dedup_by(|a, b| a.id == b.id);
    out
}
