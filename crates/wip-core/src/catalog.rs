//! The option catalog: every customization the app can offer, with
//! plain-language descriptions, risk, applicability and concrete actions.

use serde::{Deserialize, Serialize};
use std::collections::{BTreeSet, HashMap};
use std::fmt;

const SOURCES: &[(&str, &str)] = &[
    ("apps.toml", include_str!("../catalog/apps.toml")),
    ("system.toml", include_str!("../catalog/system.toml")),
    ("tweaks.toml", include_str!("../catalog/tweaks.toml")),
];

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Category {
    Apps,
    Capabilities,
    Features,
    Services,
    Privacy,
    Desktop,
    Setup,
    Slimming,
}

impl Category {
    pub const ALL: [Category; 8] = [
        Category::Apps,
        Category::Capabilities,
        Category::Features,
        Category::Services,
        Category::Privacy,
        Category::Desktop,
        Category::Setup,
        Category::Slimming,
    ];

    pub fn label(self) -> &'static str {
        match self {
            Category::Apps => "Apps",
            Category::Capabilities => "Windows extras",
            Category::Features => "Optional features",
            Category::Services => "Services",
            Category::Privacy => "Privacy",
            Category::Desktop => "Desktop & system",
            Category::Setup => "Setup",
            Category::Slimming => "Slimming",
        }
    }

    pub fn blurb(self) -> &'static str {
        match self {
            Category::Apps => {
                "Tick apps you never want on new installs. Anything left unticked stays exactly as Microsoft ships it."
            }
            Category::Capabilities => {
                "Optional Windows components that can be added back later from Settings > Optional features."
            }
            Category::Features => "Turn Windows features on or off before installation.",
            Category::Services => "Background services. Disabled services never start until you turn them back on.",
            Category::Privacy => "Defaults applied to every new user. Most can be switched back later in Settings.",
            Category::Desktop => "How Windows looks and behaves for every new user.",
            Category::Setup => "Changes to the installer itself, before Windows is copied to the disk.",
            Category::Slimming => {
                "Make the installed system smaller. Some of these can't be undone after installation."
            }
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Risk {
    Safe,
    Low,
    Moderate,
    High,
}

impl Risk {
    pub fn label(self) -> &'static str {
        match self {
            Risk::Safe => "Safe",
            Risk::Low => "Low",
            Risk::Moderate => "Moderate",
            Risk::High => "High",
        }
    }

    /// The lowest experience level at which an option of this risk is shown.
    pub fn min_level(self) -> Level {
        match self {
            Risk::Safe | Risk::Low => Level::Beginner,
            Risk::Moderate => Level::Intermediate,
            Risk::High => Level::Expert,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum Level {
    #[default]
    Beginner,
    Intermediate,
    Expert,
}

impl Level {
    pub const ALL: [Level; 3] = [Level::Beginner, Level::Intermediate, Level::Expert];
    pub fn label(self) -> &'static str {
        match self {
            Level::Beginner => "Beginner",
            Level::Intermediate => "Intermediate",
            Level::Expert => "Expert",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum Applies {
    #[default]
    Both,
    Win10,
    Win11,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Hive {
    Software,
    System,
    DefaultUser,
    BootSystem,
}

impl Hive {
    /// Path of the hive file relative to the mounted image root.
    pub fn file(self) -> &'static str {
        match self {
            Hive::Software => r"Windows\System32\config\SOFTWARE",
            Hive::System | Hive::BootSystem => r"Windows\System32\config\SYSTEM",
            Hive::DefaultUser => r"Users\Default\NTUSER.DAT",
        }
    }

    /// Temporary key name under HKLM while the hive is loaded.
    pub fn mount_key(self) -> &'static str {
        match self {
            Hive::Software => "WIP_SOFTWARE",
            Hive::System => "WIP_SYSTEM",
            Hive::DefaultUser => "WIP_DEFAULT",
            Hive::BootSystem => "WIP_BOOTSYS",
        }
    }

    pub fn is_boot(self) -> bool {
        matches!(self, Hive::BootSystem)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", content = "value", rename_all = "snake_case")]
pub enum RegData {
    Dword(u32),
    String(String),
}

impl fmt::Display for RegData {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            RegData::Dword(v) => write!(f, "REG_DWORD {v}"),
            RegData::String(s) => write!(f, "REG_SZ \"{s}\""),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum StartType {
    Auto,
    Manual,
    Disabled,
}

impl StartType {
    pub fn value(self) -> u32 {
        match self {
            StartType::Auto => 2,
            StartType::Manual => 3,
            StartType::Disabled => 4,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Action {
    RemoveAppx {
        prefix: String,
    },
    RemoveCapability {
        prefix: String,
    },
    DisableFeature {
        name: String,
        #[serde(default)]
        remove_payload: bool,
    },
    EnableFeature {
        name: String,
        #[serde(default)]
        source_sxs: bool,
    },
    ServiceStart {
        name: String,
        start: StartType,
    },
    RegSet {
        hive: Hive,
        key: String,
        value: String,
        data: RegData,
    },
    RegDelete {
        hive: Hive,
        key: String,
        value: String,
    },
    DeleteFile {
        path: String,
    },
    ComponentCleanup {
        reset_base: bool,
    },
}

impl Action {
    /// The identifier this action targets (package prefix, feature name,
    /// registry value...), for Advanced mode's ID column.
    pub fn target(&self) -> String {
        match self {
            Action::RemoveAppx { prefix } | Action::RemoveCapability { prefix } => prefix.clone(),
            Action::DisableFeature { name, .. } | Action::EnableFeature { name, .. } => name.clone(),
            Action::ServiceStart { name, .. } => name.clone(),
            Action::RegSet { key, value, .. } | Action::RegDelete { key, value, .. } => {
                let leaf = key.rsplit('\\').next().unwrap_or(key);
                format!("{leaf}\\{}", display_value(value))
            }
            Action::DeleteFile { path } => path.clone(),
            Action::ComponentCleanup { .. } => "StartComponentCleanup".into(),
        }
    }

    /// One-line technical description shown in Advanced mode's inspector.
    pub fn technical(&self) -> String {
        match self {
            Action::RemoveAppx { prefix } => format!("Remove provisioned app {prefix}*"),
            Action::RemoveCapability { prefix } => format!("Remove capability {prefix}*"),
            Action::DisableFeature { name, .. } => format!("Disable feature {name}"),
            Action::EnableFeature { name, .. } => format!("Enable feature {name}"),
            Action::ServiceStart { name, start } => {
                format!("Service {name}: Start = {} ({start:?})", start.value())
            }
            Action::RegSet { hive, key, value, data } => {
                format!("{hive:?}\\{key} : {} = {data}", display_value(value))
            }
            Action::RegDelete { hive, key, value } => {
                format!("Delete {hive:?}\\{key} : {}", display_value(value))
            }
            Action::DeleteFile { path } => format!("Delete file {path}"),
            Action::ComponentCleanup { reset_base } => {
                if *reset_base {
                    "DISM StartComponentCleanup /ResetBase".into()
                } else {
                    "DISM StartComponentCleanup".into()
                }
            }
        }
    }
}

fn display_value(v: &str) -> &str {
    if v.is_empty() {
        "(Default)"
    } else {
        v
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CatalogOption {
    pub id: String,
    pub category: Category,
    pub title: String,
    pub summary: String,
    #[serde(default)]
    pub details: String,
    pub risk: Risk,
    #[serde(default)]
    pub breaks: Vec<String>,
    #[serde(default)]
    pub undo: String,
    #[serde(default)]
    pub applies: Applies,
    #[serde(default)]
    pub min_build: Option<u32>,
    #[serde(default)]
    pub requires: Vec<String>,
    #[serde(default)]
    pub conflicts: Vec<String>,
    pub actions: Vec<Action>,
}

impl CatalogOption {
    pub fn visible_at(&self, level: Level) -> bool {
        self.risk.min_level() <= level
    }

    pub fn applies_to(&self, family: crate::WindowsFamily, build: u32) -> bool {
        let family_ok = matches!(
            (self.applies, family),
            (Applies::Both, _)
                | (Applies::Win10, crate::WindowsFamily::Win10)
                | (Applies::Win11, crate::WindowsFamily::Win11)
        );
        family_ok && self.min_build.is_none_or(|m| build >= m)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Preset {
    pub id: String,
    pub title: String,
    pub summary: String,
    pub options: Vec<String>,
}

#[derive(Debug, Default, Deserialize)]
struct CatalogFile {
    #[serde(default)]
    option: Vec<CatalogOption>,
    #[serde(default)]
    preset: Vec<Preset>,
}

#[derive(Debug, Clone)]
pub struct Catalog {
    pub options: Vec<CatalogOption>,
    pub presets: Vec<Preset>,
    index: HashMap<String, usize>,
}

#[derive(Debug)]
pub struct CatalogError(pub String);

impl fmt::Display for CatalogError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl std::error::Error for CatalogError {}

impl Catalog {
    /// Loads and validates the catalog compiled into the binary.
    pub fn builtin() -> Result<Catalog, CatalogError> {
        let mut options = Vec::new();
        let mut presets = Vec::new();
        for (name, text) in SOURCES {
            let file: CatalogFile = toml::from_str(text).map_err(|e| CatalogError(format!("{name}: {e}")))?;
            options.extend(file.option);
            presets.extend(file.preset);
        }
        Catalog::from_parts(options, presets)
    }

    pub fn from_parts(options: Vec<CatalogOption>, presets: Vec<Preset>) -> Result<Catalog, CatalogError> {
        let mut index = HashMap::new();
        for (i, o) in options.iter().enumerate() {
            if index.insert(o.id.clone(), i).is_some() {
                return Err(CatalogError(format!("duplicate option id {}", o.id)));
            }
        }
        let cat = Catalog { options, presets, index };
        cat.validate()?;
        Ok(cat)
    }

    fn validate(&self) -> Result<(), CatalogError> {
        let err = |m: String| Err(CatalogError(m));
        for o in &self.options {
            if o.title.trim().is_empty() || o.summary.trim().is_empty() {
                return err(format!("{}: title and summary are required", o.id));
            }
            if o.actions.is_empty() {
                return err(format!("{}: no actions", o.id));
            }
            if o.risk >= Risk::Moderate && o.breaks.is_empty() {
                return err(format!("{}: moderate/high risk options must list what breaks", o.id));
            }
            for r in o.requires.iter().chain(o.conflicts.iter()) {
                if !self.index.contains_key(r) {
                    return err(format!("{}: references unknown option {r}", o.id));
                }
                if r == &o.id {
                    return err(format!("{}: references itself", o.id));
                }
            }
            for a in &o.actions {
                let boot = matches!(a, Action::RegSet { hive, .. } | Action::RegDelete { hive, .. } if hive.is_boot());
                if boot && o.category != Category::Setup {
                    return err(format!("{}: boot.wim changes belong in the Setup category", o.id));
                }
                match a {
                    Action::RemoveAppx { prefix } | Action::RemoveCapability { prefix } if prefix.trim().len() < 4 => {
                        return err(format!("{}: prefix too short", o.id));
                    }
                    Action::RegSet { key, .. } | Action::RegDelete { key, .. }
                        if key.starts_with('\\') || key.contains("..") || key.is_empty() =>
                    {
                        return err(format!("{}: invalid registry key", o.id));
                    }
                    Action::RegSet { hive: Hive::System | Hive::BootSystem, key, .. }
                        if key.starts_with("CurrentControlSet") =>
                    {
                        return err(format!("{}: offline SYSTEM hives have no CurrentControlSet", o.id));
                    }
                    Action::DeleteFile { path }
                        if path.contains("..") || path.starts_with('\\') || path.contains(':') =>
                    {
                        return err(format!("{}: invalid file path", o.id));
                    }
                    _ => {}
                }
            }
        }
        for p in &self.presets {
            let mut seen = BTreeSet::new();
            for id in &p.options {
                if !self.index.contains_key(id) {
                    return err(format!("preset {}: unknown option {id}", p.id));
                }
                if !seen.insert(id) {
                    return err(format!("preset {}: duplicate option {id}", p.id));
                }
            }
        }
        Ok(())
    }

    pub fn get(&self, id: &str) -> Option<&CatalogOption> {
        self.index.get(id).map(|&i| &self.options[i])
    }

    pub fn in_category(&self, c: Category) -> impl Iterator<Item = &CatalogOption> {
        self.options.iter().filter(move |o| o.category == c)
    }

    pub fn preset(&self, id: &str) -> Option<&Preset> {
        self.presets.iter().find(|p| p.id == id)
    }
}
