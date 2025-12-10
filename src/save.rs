use anyhow::{Context, Result};
use crate::model::{WorldState, Location, Actor};
use std::path::PathBuf;
use std::fs;
use std::collections::HashMap;
use chrono::{DateTime, Local};
use serde_json::Value;

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
        
        // Try to load as new format first
        match serde_json::from_str::<WorldState>(&content) {
            Ok(world) => Ok(world),
            Err(_) => {
                // Try to migrate from old format
                self.migrate_old_save(&content)
                    .context("Failed to migrate old save format")
            }
        }
    }

    fn migrate_old_save(&self, content: &str) -> Result<WorldState> {
        let old_data: Value = serde_json::from_str(content)
            .context("Failed to parse old save format")?;
        
        // Extract old data
        let old_current_id = old_data.get("current_location_id")
            .and_then(|v| v.as_str())
            .unwrap_or("start");
        
        let empty_map = serde_json::Map::new();
        let old_locations = old_data.get("locations")
            .and_then(|v| v.as_object())
            .unwrap_or(&empty_map);
        
        // Convert to new format
        let mut new_locations = HashMap::new();
        let mut current_pos = (0, 0);
        
        for (loc_id, loc_data) in old_locations {
            if let Some(loc_obj) = loc_data.as_object() {
                let name = loc_obj.get("name")
                    .and_then(|v| v.as_str())
                    .unwrap_or("Unknown")
                    .to_string();
                
                let description = loc_obj.get("description")
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_string();
                
                let x = loc_obj.get("x")
                    .and_then(|v| v.as_i64())
                    .unwrap_or(0) as i32;
                
                let y = loc_obj.get("y")
                    .and_then(|v| v.as_i64())
                    .unwrap_or(0) as i32;
                
                let pos = (x, y);
                
                // Convert exits from string IDs to coordinates
                let mut new_exits = HashMap::new();
                if let Some(exits) = loc_obj.get("exits").and_then(|v| v.as_object()) {
                    for (dir, target) in exits {
                        if let Some(target_id) = target.as_str() {
                            // Try to find target location's coordinates
                            if let Some(target_loc) = old_locations.get(target_id) {
                                if let Some(target_obj) = target_loc.as_object() {
                                    let target_x = target_obj.get("x")
                                        .and_then(|v| v.as_i64())
                                        .unwrap_or(0) as i32;
                                    let target_y = target_obj.get("y")
                                        .and_then(|v| v.as_i64())
                                        .unwrap_or(0) as i32;
                                    new_exits.insert(dir.clone(), Some((target_x, target_y)));
                                }
                            } else {
                                new_exits.insert(dir.clone(), None); // Blocked exit
                            }
                        } else {
                            new_exits.insert(dir.clone(), None); // Null/blocked exit
                        }
                    }
                }
                
                let items = loc_obj.get("items")
                    .and_then(|v| v.as_array())
                    .map(|arr| {
                        arr.iter()
                            .filter_map(|v| v.as_str())
                            .map(|s| s.to_string())
                            .collect()
                    })
                    .unwrap_or_default();
                
                let actors = loc_obj.get("actors")
                    .and_then(|v| v.as_array())
                    .map(|arr| {
                        arr.iter()
                            .filter_map(|v| v.as_str())
                            .map(|s| s.to_string())
                            .collect()
                    })
                    .unwrap_or_default();
                
                let cached_image_path = loc_obj.get("cached_image_path")
                    .and_then(|v| v.as_str())
                    .map(|s| s.to_string());
                
                let image_prompt = loc_obj.get("image_prompt")
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_string();
                
                let location = Location {
                    name,
                    description,
                    items,
                    actors,
                    exits: new_exits,
                    cached_image_path,
                    image_prompt,
                    visited: true, // Assume old locations were visited
                };
                
                new_locations.insert(pos, location);
                
                if loc_id == old_current_id {
                    current_pos = pos;
                }
            }
        }
        
        // Migrate actors
        let mut new_actors = HashMap::new();
        if let Some(actors_data) = old_data.get("actors").and_then(|v| v.as_object()) {
            for (actor_id, actor_obj) in actors_data {
                if let Some(obj) = actor_obj.as_object() {
                    let name = obj.get("name")
                        .and_then(|v| v.as_str())
                        .unwrap_or("")
                        .to_string();
                    
                    let description = obj.get("description")
                        .and_then(|v| v.as_str())
                        .unwrap_or("")
                        .to_string();
                    
                    let inventory = obj.get("inventory")
                        .and_then(|v| v.as_array())
                        .map(|arr| {
                            arr.iter()
                                .filter_map(|v| v.as_str())
                                .map(|s| s.to_string())
                                .collect()
                        })
                        .unwrap_or_default();
                    
                    let money = obj.get("money")
                        .and_then(|v| v.as_u64())
                        .unwrap_or(0) as u32;
                    
                    // Find actor's current location
                    let mut actor_pos = (0, 0);
                    if let Some(loc_id) = obj.get("current_location_id").and_then(|v| v.as_str()) {
                        if let Some(loc_data) = old_locations.get(loc_id) {
                            if let Some(loc_obj) = loc_data.as_object() {
                                let x = loc_obj.get("x")
                                    .and_then(|v| v.as_i64())
                                    .unwrap_or(0) as i32;
                                let y = loc_obj.get("y")
                                    .and_then(|v| v.as_i64())
                                    .unwrap_or(0) as i32;
                                actor_pos = (x, y);
                            }
                        }
                    }
                    
                    let actor = Actor {
                        id: actor_id.clone(),
                        name,
                        description,
                        current_pos: actor_pos,
                        inventory,
                        money,
                    };
                    
                    new_actors.insert(actor_id.clone(), actor);
                }
            }
        }
        
        // Migrate items
        let mut new_items = HashMap::new();
        if let Some(items_data) = old_data.get("items").and_then(|v| v.as_object()) {
            for (item_id, item_obj) in items_data {
                if let Some(obj) = item_obj.as_object() {
                    let name = obj.get("name")
                        .and_then(|v| v.as_str())
                        .unwrap_or("")
                        .to_string();
                    
                    let description = obj.get("description")
                        .and_then(|v| v.as_str())
                        .unwrap_or("")
                        .to_string();
                    
                    let item = crate::model::Item {
                        id: item_id.clone(),
                        name,
                        description,
                    };
                    
                    new_items.insert(item_id.clone(), item);
                }
            }
        }
        
        // Migrate player
        let player_inventory = old_data.get("player")
            .and_then(|v| v.get("inventory"))
            .and_then(|v| v.as_array())
            .map(|arr| {
                arr.iter()
                    .filter_map(|v| v.as_str())
                    .map(|s| s.to_string())
                    .collect()
            })
            .unwrap_or_default();
        
        let player_money = old_data.get("player")
            .and_then(|v| v.get("money"))
            .and_then(|v| v.as_u64())
            .unwrap_or(0) as u32;
        
        let player = crate::model::Player {
            inventory: player_inventory,
            money: player_money,
        };
        
        Ok(WorldState {
            current_pos,
            locations: new_locations,
            actors: new_actors,
            items: new_items,
            player,
        })
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

    pub fn delete_save(&self, filename: &str) -> Result<()> {
        let path = self.save_dir.join(filename);
        if path.exists() {
            fs::remove_file(&path)
                .context(format!("Failed to delete save file: {:?}", path))?;
        }
        Ok(())
    }
}
