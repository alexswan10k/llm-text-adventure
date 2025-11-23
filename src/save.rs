use anyhow::{Context, Result};
use crate::model::WorldState;
use std::path::PathBuf;
use std::fs;
use chrono::{DateTime, Local};

pub struct SaveManager {
    save_dir: PathBuf,
}

#[derive(Debug, Clone)]
pub struct SaveInfo {
    pub filename: String,
    pub path: PathBuf,
    pub modified: DateTime<Local>,
}

impl SaveManager {
    pub fn new() -> Self {
        let save_dir = PathBuf::from("saves");
        // Ensure directory exists
        if !save_dir.exists() {
            fs::create_dir_all(&save_dir).unwrap_or_default();
        }
        Self { save_dir }
    }

    pub fn list_saves(&self) -> Result<Vec<SaveInfo>> {
        let mut saves = Vec::new();
        if !self.save_dir.exists() {
            return Ok(saves);
        }

        for entry in fs::read_dir(&self.save_dir)? {
            let entry = entry?;
            let path = entry.path();
            if path.extension().and_then(|s| s.to_str()) == Some("json") {
                let metadata = fs::metadata(&path)?;
                let modified: DateTime<Local> = metadata.modified()?.into();
                let filename = path.file_name()
                    .and_then(|s| s.to_str())
                    .unwrap_or("unknown")
                    .to_string();

                saves.push(SaveInfo {
                    filename,
                    path,
                    modified,
                });
            }
        }

        // Sort by newest first
        saves.sort_by(|a, b| b.modified.cmp(&a.modified));
        Ok(saves)
    }

    pub fn load_save(&self, filename: &str) -> Result<WorldState> {
        let path = self.save_dir.join(filename);
        let content = fs::read_to_string(&path)
            .context(format!("Failed to read save file: {:?}", path))?;
        let world: WorldState = serde_json::from_str(&content)
            .context("Failed to deserialize save file")?;
        Ok(world)
    }

    pub fn save_game(&self, filename: &str, world: &WorldState) -> Result<()> {
        let path = self.save_dir.join(filename);
        let content = serde_json::to_string_pretty(world)
            .context("Failed to serialize world state")?;
        fs::write(&path, content)
            .context(format!("Failed to write save file: {:?}", path))?;
        Ok(())
    }

    pub fn create_new_save(&self, name: &str, world: &WorldState) -> Result<String> {
        // Sanitize name or just use it. 
        // If name doesn't end in .json, add it.
        let mut filename = name.to_string();
        if !filename.ends_with(".json") {
            filename.push_str(".json");
        }
        self.save_game(&filename, world)?;
        Ok(filename)
    }
}
