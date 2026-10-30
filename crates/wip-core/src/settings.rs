//! Persistent app settings.

use crate::catalog::Level;
use serde::{Deserialize, Serialize};

pub const SETTINGS_SCHEMA: u32 = 1;

/// Guided = wizard with explainer pane. Advanced = dense table + inspector.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Mode {
    Guided,
    Advanced,
}

impl Mode {
    pub fn label(self) -> &'static str {
        match self {
            Mode::Guided => "Guided",
            Mode::Advanced => "Advanced",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Settings {
    pub schema: u32,
    /// `None` until the user picks a mode on first launch.
    pub mode: Option<Mode>,
    /// Which options Guided mode shows.
    pub level: Level,
    /// UI text scale multiplier (1.0 = 100%).
    pub text_scale: f32,
    /// Folder for workspaces; empty = ask each time.
    pub workspace_root: String,
    /// Optional folder holding ADK DISM (dismapi.dll / dism.exe).
    pub dism_folder: String,
}

impl Default for Settings {
    fn default() -> Self {
        Settings {
            schema: SETTINGS_SCHEMA,
            mode: None,
            level: Level::Beginner,
            text_scale: 1.0,
            workspace_root: String::new(),
            dism_folder: String::new(),
        }
    }
}

impl Settings {
    /// Clamps values from disk into safe ranges (settings files can be
    /// edited by hand).
    pub fn sanitized(mut self) -> Settings {
        if !(0.75..=2.0).contains(&self.text_scale) || !self.text_scale.is_finite() {
            self.text_scale = 1.0;
        }
        self.schema = SETTINGS_SCHEMA;
        self
    }
}
