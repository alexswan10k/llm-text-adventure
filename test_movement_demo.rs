// Test demonstrating correct movement to new areas
// This shows what should happen when someone moves to a new area

use std::collections::HashMap;
use llm_text_adventure::{Game, LlmClient};
use llm_text_adventure::model::{WorldState, Location, WorldUpdate};

#[tokio::test]
async fn test_movement_to_new_area() {
    // Create mock LLM client for testing
    let llm_client = MockLlmClient {};
    
    // Initialize game with mock LLM
    let mut game = Game::new(llm_client);
    
    // Setup initial world state at origin (0, 0)
    let start_location = Location {
        name: "The Beginning".to_string(),
        description: "You stand in a void of potential.".to_string(),
        items: vec![],
        actors: vec![],
        exits: HashMap::new(), 
        cached_image_path: None,
        image_prompt: "A swirling void of colors and shapes.".to_string(),
        visited: true,
    };
    
    game.world.locations.insert((0, 0), start_location);
    game.world.current_pos = (0, 0);
    
    // Simulate user input to move north - this should trigger the correct movement logic
    let result = game.process_input("go north").await;
    
    assert!(result.is_ok(), "Movement processing failed");
    
    // Verify that we moved to a new location at (0, 1)
    assert_eq!(game.world.current_pos, (0, 1));
    
    // Check that the new location was created correctly
    let new_location = game.world.locations.get(&(0, 1)).expect("New location should exist");
    assert_eq!(new_location.name, "Forest Path");
    println!("✓ Successfully moved to new area at (0, 1)");
    println!("✓ Location name: {}", new_location.name);
    println!("✓ Location description: {}", new_location.description);
}

// Mock LLM client that returns the correct actions for movement
struct MockLlmClient;

impl MockLlmClient {
    fn generate_update(&self, _system_prompt: &str, context: &str) -> Result<WorldUpdate, Box<dyn std::error::Error>> {
        // Check if we're trying to move to a new location (adjacent cell is empty)
        if context.contains("Empty") && context.contains("go north") {
            println!("Mock LLM detected movement to new area");
            
            Ok(WorldUpdate {
                narrative: "You venture north and discover a forest path.".to_string(),
                actions: vec![
                    // First create the new location
                    "CreateLocation(0, 1, {\"name\":\"Forest Path\",\"description\":\"A narrow trail through dense woods.\",\"exits\":{\"south\":[0,0]},\"image_prompt\":\"Dense forest with sunlight filtering through leaves\"})".to_string(),
                    // Then move to it  
                    "MoveTo(0, 1)".to_string()
                ],
                suggested_actions: vec!["go east".to_string(), "go west".to_string(), "explore area".to_string()]
            })
        } else {
            Ok(WorldUpdate {
                narrative: "You examine your surroundings.".to_string(),
                actions: vec![
                    "MoveTo(0, 1)".to_string()
                ],
                suggested_actions: vec!["go north".to_string(), "go south".to_string()]
            })
        }
    }
}

// This demonstrates the correct behavior:
// When moving to a new area (adjacent cell is empty):
// 1. CreateLocation(x,y,{location details}) must be called FIRST
// 2. MoveTo(x,y) must be called SECOND 
//
// Example sequence from LLM response:
// [
//   "CreateLocation(0, 1, {\"name\":\"Forest Path\",\"description\":...})", 
//   "MoveTo(0, 1)"
// ]