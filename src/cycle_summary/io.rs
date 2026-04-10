//! File I/O: load, save, default_path, write_default.

use std::path::{Path, PathBuf};
use super::types::{CycleSummary, CycleSummaryData};

impl CycleSummary {
    /// Default path: `.axonix/cycle_summary.json`
    pub fn default_path() -> Self {
        let home = std::env::var("HOME").unwrap_or_else(|_| "/workspace".to_string());
        let path = PathBuf::from(&home).join(".axonix").join("cycle_summary.json");
        Self::load(path)
    }

    /// Load from the given path. Returns an instance with data=None if file doesn't exist.
    pub fn load(path: impl AsRef<Path>) -> Self {
        let path = path.as_ref().to_path_buf();
        let data = std::fs::read_to_string(&path)
            .ok()
            .and_then(|s| serde_json::from_str::<CycleSummaryData>(&s).ok());
        Self { path, data }
    }

    /// Save to disk. Creates the parent directory if needed.
    pub fn save(&self) -> std::io::Result<()> {
        if let Some(parent) = self.path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let json = serde_json::to_string_pretty(
            self.data
                .as_ref()
                .ok_or_else(|| std::io::Error::new(std::io::ErrorKind::InvalidInput, "no data to save"))?,
        )
        .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;
        std::fs::write(&self.path, json)
    }

    /// Write the given data to the default path `.axonix/cycle_summary.json`.
    pub fn write_default(data: &CycleSummaryData) -> Result<(), String> {
        let home = std::env::var("HOME").unwrap_or_else(|_| "/workspace".to_string());
        let path = std::path::PathBuf::from(&home)
            .join(".axonix")
            .join("cycle_summary.json");
        let mut cs = CycleSummary::new(&path);
        cs.set(data.clone());
        cs.save().map_err(|e| e.to_string())
    }
}
