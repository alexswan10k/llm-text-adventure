pub mod model;
pub mod llm;
pub mod llm_tests;
pub mod game;
pub mod tui;
#[cfg(not(target_arch = "wasm32"))]
pub mod cli;
pub mod image;
pub mod save;
pub mod parsing;
pub mod tools;
pub mod agent;
pub mod commands;
pub mod input;

#[cfg(target_arch = "wasm32")]
pub mod wasm;

// Re-export for convenience
pub use game::Game;
pub use llm::LlmClient;
pub use tui::Tui;
#[cfg(not(target_arch = "wasm32"))]
pub use cli::Cli;
pub use save::SaveManager;
