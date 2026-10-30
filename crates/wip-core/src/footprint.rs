//! Measured before/after footprint of a build. Every number comes from
//! DISM or the file system; nothing is estimated.

use crate::human_bytes;
use crate::inventory::Inventory;
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Measure {
    /// Expanded (installed) size of the edition as recorded by DISM.
    pub expanded_bytes: u64,
    /// Size of the install image file holding the kept edition(s).
    pub image_file_bytes: u64,
    pub apps: usize,
    pub capabilities_installed: usize,
    pub features_enabled: usize,
}

impl Measure {
    pub fn of(inv: &Inventory, expanded_bytes: u64, image_file_bytes: u64) -> Measure {
        Measure {
            expanded_bytes,
            image_file_bytes,
            apps: inv.appx.len(),
            capabilities_installed: inv.capabilities.iter().filter(|c| c.state.is_on()).count(),
            features_enabled: inv.features.iter().filter(|f| f.state.is_on()).count(),
        }
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Changes {
    pub apps_removed: Vec<String>,
    pub capabilities_removed: Vec<String>,
    pub features_turned_off: Vec<String>,
    pub features_turned_on: Vec<String>,
}

impl Changes {
    pub fn between(before: &Inventory, after: &Inventory) -> Changes {
        let after_apps: BTreeSet<String> = after.appx.iter().map(|a| a.display_name.to_lowercase()).collect();
        let mut c = Changes::default();
        for a in &before.appx {
            if !after_apps.contains(&a.display_name.to_lowercase()) {
                c.apps_removed.push(a.display_name.clone());
            }
        }
        for cap in before.capabilities.iter().filter(|c| c.state.is_on()) {
            let still = after.capabilities.iter().any(|x| x.name.eq_ignore_ascii_case(&cap.name) && x.state.is_on());
            if !still {
                c.capabilities_removed.push(cap.name.clone());
            }
        }
        for f in &before.features {
            if let Some(a) = after.feature(&f.name) {
                if f.state.is_on() && !a.state.is_on() {
                    c.features_turned_off.push(f.name.clone());
                } else if !f.state.is_on() && a.state.is_on() {
                    c.features_turned_on.push(f.name.clone());
                }
            }
        }
        for v in
            [&mut c.apps_removed, &mut c.capabilities_removed, &mut c.features_turned_off, &mut c.features_turned_on]
        {
            v.sort_by_key(|s| s.to_lowercase());
            v.dedup();
        }
        c
    }

    pub fn count(&self) -> usize {
        self.apps_removed.len()
            + self.capabilities_removed.len()
            + self.features_turned_off.len()
            + self.features_turned_on.len()
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Footprint {
    pub schema: u32,
    pub edition: String,
    pub version: String,
    pub before: Measure,
    pub after: Measure,
    pub changes: Changes,
    /// The output was recompressed to ESD, so file-size change includes
    /// the compression change, not only removed content.
    pub recompressed: bool,
    /// Registry and service edits made (not visible in sizes).
    pub settings_changed: usize,
}

fn signed(before: u64, after: u64) -> String {
    if after <= before {
        format!("−{}", human_bytes(before - after))
    } else {
        format!("+{}", human_bytes(after - before))
    }
}

impl Footprint {
    pub fn expanded_delta(&self) -> String {
        signed(self.before.expanded_bytes, self.after.expanded_bytes)
    }

    pub fn file_delta(&self) -> String {
        signed(self.before.image_file_bytes, self.after.image_file_bytes)
    }

    /// Short lines for the result screen.
    pub fn summary_lines(&self) -> Vec<String> {
        let mut v = vec![
            format!(
                "Installed size: {}, now {} ({})",
                human_bytes(self.before.expanded_bytes),
                human_bytes(self.after.expanded_bytes),
                self.expanded_delta()
            ),
            format!(
                "Image file: {}, now {} ({}{})",
                human_bytes(self.before.image_file_bytes),
                human_bytes(self.after.image_file_bytes),
                self.file_delta(),
                if self.recompressed { ", includes ESD recompression" } else { "" }
            ),
            format!("Apps: {}, now {}", self.before.apps, self.after.apps),
            format!(
                "Capabilities installed: {}, now {}",
                self.before.capabilities_installed, self.after.capabilities_installed
            ),
            format!("Features enabled: {}, now {}", self.before.features_enabled, self.after.features_enabled),
        ];
        if self.settings_changed > 0 {
            v.push(format!("{} registry and service settings changed", self.settings_changed));
        }
        v
    }

    pub fn to_markdown(&self) -> String {
        let mut s = format!("# WinISOPilot footprint report\n\n{} ({})\n\n", self.edition, self.version);
        s.push_str("| | Before | After | Change |\n|---|---|---|---|\n");
        s.push_str(&format!(
            "| Installed size | {} | {} | {} |\n",
            human_bytes(self.before.expanded_bytes),
            human_bytes(self.after.expanded_bytes),
            self.expanded_delta()
        ));
        s.push_str(&format!(
            "| Image file | {} | {} | {} |\n",
            human_bytes(self.before.image_file_bytes),
            human_bytes(self.after.image_file_bytes),
            self.file_delta()
        ));
        s.push_str(&format!("| Apps | {} | {} | |\n", self.before.apps, self.after.apps));
        s.push_str(&format!(
            "| Capabilities installed | {} | {} | |\n",
            self.before.capabilities_installed, self.after.capabilities_installed
        ));
        s.push_str(&format!(
            "| Features enabled | {} | {} | |\n\n",
            self.before.features_enabled, self.after.features_enabled
        ));
        if self.recompressed {
            s.push_str("The image file was recompressed to ESD; its size change includes that.\n\n");
        }
        for (title, list) in [
            ("Apps removed", &self.changes.apps_removed),
            ("Capabilities removed", &self.changes.capabilities_removed),
            ("Features turned off", &self.changes.features_turned_off),
            ("Features turned on", &self.changes.features_turned_on),
        ] {
            if !list.is_empty() {
                s.push_str(&format!("## {title}\n\n"));
                for i in list {
                    s.push_str(&format!("- {i}\n"));
                }
                s.push('\n');
            }
        }
        s.push_str(&format!("Registry and service settings changed: {}\n", self.settings_changed));
        s
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::inventory::{AppxPackage, NamedState, PackageState};

    fn inv(apps: &[&str], caps: &[(&str, PackageState)], feats: &[(&str, PackageState)]) -> Inventory {
        Inventory {
            appx: apps
                .iter()
                .map(|a| AppxPackage { display_name: a.to_string(), package_name: format!("{a}_1_x64__x") })
                .collect(),
            capabilities: caps.iter().map(|(n, s)| NamedState { name: n.to_string(), state: *s }).collect(),
            features: feats.iter().map(|(n, s)| NamedState { name: n.to_string(), state: *s }).collect(),
            ..Default::default()
        }
    }

    #[test]
    fn diff_and_report() {
        use PackageState::*;
        let before = inv(
            &["A.One", "B.Two", "C.Three"],
            &[("Cap.X~~~~1", Installed), ("Cap.Y~~~~1", Installed)],
            &[("SMB1Protocol", Installed), ("NetFx3", Removed)],
        );
        let after = inv(
            &["C.Three"],
            &[("Cap.X~~~~1", NotPresent), ("Cap.Y~~~~1", Installed)],
            &[("SMB1Protocol", Staged), ("NetFx3", Installed)],
        );
        let c = Changes::between(&before, &after);
        assert_eq!(c.apps_removed, vec!["A.One", "B.Two"]);
        assert_eq!(c.capabilities_removed, vec!["Cap.X~~~~1"]);
        assert_eq!(c.features_turned_off, vec!["SMB1Protocol"]);
        assert_eq!(c.features_turned_on, vec!["NetFx3"]);
        assert_eq!(c.count(), 5);

        let f = Footprint {
            schema: 1,
            edition: "Windows 11 Pro".into(),
            version: "10.0.26100.1".into(),
            before: Measure::of(&before, 20_000_000_000, 5_000_000_000),
            after: Measure::of(&after, 18_000_000_000, 5_200_000_000),
            changes: c,
            recompressed: false,
            settings_changed: 7,
        };
        assert!(f.expanded_delta().starts_with('−'));
        assert!(f.file_delta().starts_with('+'));
        let md = f.to_markdown();
        assert!(md.contains("## Apps removed"));
        assert!(md.contains("- B.Two"));
        assert_eq!(f.summary_lines().len(), 6);
    }
}
