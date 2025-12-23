use std::collections::HashMap;
use crate::model::{WorldState, Location, WorldUpdate};
use crate::game::Game;

#[tokio::test]
async fn test_move_to_new_area() {
    // Create a basic LLM client mock (simplified for testing)
    let llm_client = MockLlmClient {};
    
    // Initialize game
    let mut game = Game::new(llm_client);
    
    // Start with an initial location at origin
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
    
    // Simulate user input to move north to a new area
    let input = "go north";
    
    // Process the input - this should trigger movement logic
    game.process_input(input).await.unwrap();
    
    // Verify that we moved to a new location
    assert_eq!(game.world.current_pos, (0, 1));
    
    // Check that a new location was created at (0, 1)
    let new_location = game.world.locations.get(&(0, 1)).unwrap();
    assert_eq!(new_location.name, "Forest Path");
    assert_eq!(new_location.description, "A narrow trail through dense woods.");
    
    println!("Movement test passed! Successfully moved to new area at (0, 1)");
}

// Mock LLM client for testing purposes
struct MockLlmClient;

impl MockLlmClient {
    fn generate_update(&self, _system_prompt: &str, _context: &str) -> Result<WorldUpdate, Box<dyn std::error::Error>> {
        // Return a mock update that creates a new location and moves to it
        Ok(WorldUpdate {
            narrative: "You move north and discover a forest path.".to_string(),
            actions: vec![
                "CreateLocation(0, 1, {\"name\":\"Forest Path\",\"description\":\"A narrow trail through dense woods.\",\"exits\":{\"south\":[0,0]},\"image_prompt\":\"Dense forest with sunlight filtering through leaves\"})".to_string(),
                "MoveTo(0, 1)".to_string()
            ],
            suggested_actions: vec!["go east", "go west", "explore area"].to_string()
        })
    }
}