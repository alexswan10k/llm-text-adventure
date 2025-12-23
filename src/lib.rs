pub mod model;
pub mod llm;
pub mod game;
pub mod tui;
pub mod cli;
pub mod image;
pub mod save;

#[cfg(target_arch = "wasm32")]
pub mod wasm;

// Re-export for convenience
pub use game::Game;
pub use llm::LlmClient;
pub use tui::Tui;
pub use cli::Cli;
pub use save::SaveManager;
