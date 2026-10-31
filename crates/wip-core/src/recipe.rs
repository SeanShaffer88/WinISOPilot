//! Recipes: a saved, versioned selection that can be reused on other images.
//! Recipes contain only option IDs and output settings — never commands,
//! paths to source media, or credentials.

use crate::catalog::Catalog;
use crate::plan::Selection;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

pub const RECIPE_SCHEMA: u32 = 2;
/// Upper bound on recipe file size accepted on import.
pub const MAX_RECIPE_BYTES: usize = 256 * 1024;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum Compression {
    /// WIM with LZX ("maximum") compression. Works with all tools.
    #[default]
    Maximum,
    /// ESD with LZMS ("recovery") compression. Smaller, slower to build.
    Recovery,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct OutputSettings {
    /// Keep only the edition being customized in the final media.
    pub single_edition: bool,
    pub compression: Compression,
    /// ISO volume label (max 32 characters, ISO-safe characters only).
    pub volume_label: String,
    /// Put the boot manager signed with the "Windows UEFI CA 2023"
    /// certificate on the media (Microsoft's Make2023BootableMedia steps).
    #[serde(default)]
    pub boot_2023: bool,
}

impl Default for OutputSettings {
    fn default() -> Self {
        OutputSettings {
            single_edition: true,
            compression: Compression::Maximum,
            volume_label: "WINISOPILOT".into(),
            boot_2023: false,
        }
    }
}

/// Sanitizes a label for oscdimg: A–Z, 0–9 and underscore, max 32 chars.
pub fn sanitize_label(s: &str) -> String {
    let out: String = s
        .chars()
        .map(|c| c.to_ascii_uppercase())
        .map(|c| if c.is_ascii_alphanumeric() { c } else { '_' })
        .take(32)
        .collect();
    if out.is_empty() {
        "WINISOPILOT".into()
    } else {
        out
    }
}

/// The image a recipe was made for.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RecipeSource {
    pub build: u32,
    pub edition_id: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Recipe {
    pub schema: u32,
    pub app_version: String,
    pub options: Vec<String>,
    #[serde(default)]
    pub output: OutputSettings,
    /// Schema 2: the image the recipe was made against.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source: Option<RecipeSource>,
    /// Schema 2: what each option resolved to in that image (see `port`).
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub resolved: BTreeMap<String, Vec<String>>,
    /// Schema 2: every catalog option known when the recipe was saved.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub known_options: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LoadedRecipe {
    pub selection: Selection,
    pub output: OutputSettings,
    /// IDs this version of the app doesn't know; they are dropped.
    pub unknown: Vec<String>,
    pub source: Option<RecipeSource>,
    pub resolved: BTreeMap<String, Vec<String>>,
    pub known_options: Vec<String>,
}

impl Recipe {
    pub fn new(sel: &Selection, output: &OutputSettings) -> Recipe {
        Recipe {
            schema: RECIPE_SCHEMA,
            app_version: crate::APP_VERSION.into(),
            options: sel.iter().cloned().collect(),
            output: output.clone(),
            source: None,
            resolved: BTreeMap::new(),
            known_options: Vec::new(),
        }
    }

    /// A recipe that also records what each option resolved to in the
    /// current image, so it can be checked against future Windows builds.
    pub fn with_fingerprint(
        sel: &Selection,
        output: &OutputSettings,
        ctx: &crate::plan::Context,
        edition_id: &str,
    ) -> Recipe {
        let mut r = Recipe::new(sel, output);
        if ctx.inventory.is_some() {
            r.resolved = crate::port::resolve(sel, ctx);
            r.source = Some(RecipeSource { build: ctx.build, edition_id: edition_id.to_string() });
        }
        r.known_options = ctx.catalog.options.iter().map(|o| o.id.clone()).collect();
        r
    }

    pub fn to_json(&self) -> String {
        serde_json::to_string_pretty(self).expect("recipe serializes")
    }

    pub fn parse(bytes: &[u8], catalog: &Catalog) -> Result<LoadedRecipe, String> {
        if bytes.len() > MAX_RECIPE_BYTES {
            return Err("This file is too large to be a WinISOPilot recipe.".into());
        }
        let r: Recipe =
            serde_json::from_slice(bytes).map_err(|e| format!("This isn't a valid WinISOPilot recipe ({e})."))?;
        if r.schema == 0 || r.schema > RECIPE_SCHEMA {
            return Err(format!(
                "This recipe uses format {} but this version understands up to {RECIPE_SCHEMA}. Update WinISOPilot.",
                r.schema
            ));
        }
        let mut selection = Selection::new();
        let mut unknown = Vec::new();
        for id in r.options {
            if catalog.get(&id).is_some() || crate::raw::parse(&id).is_some() {
                selection.insert(id);
            } else {
                unknown.push(id);
            }
        }
        let mut output = r.output;
        output.volume_label = sanitize_label(&output.volume_label);
        Ok(LoadedRecipe {
            selection,
            output,
            unknown,
            source: r.source,
            resolved: r.resolved,
            known_options: r.known_options,
        })
    }
}
