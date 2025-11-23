use llm_text_adventure::{Game, LlmClient, Tui};
use anyhow::Result;
use std::env;

#[cfg(not(target_arch = "wasm32"))]
use crossterm::{
    execute,
    terminal::{enable_raw_mode, EnterAlternateScreen},
};
#[cfg(not(target_arch = "wasm32"))]
use ratatui::prelude::*;
#[cfg(not(target_arch = "wasm32"))]
use std::io;

#[cfg(not(target_arch = "wasm32"))]
#[tokio::main]
async fn main() -> Result<()> {
    // Default configuration (can be overridden by env vars)
    let base_url = env::var("LLM_BASE_URL").unwrap_or_else(|_| "http://localhost:1234".to_string());
    let model_name = env::var("LLM_MODEL_NAME").unwrap_or_else(|_| "local-model".to_string());

    let llm_client = LlmClient::new(base_url, model_name);
    let mut game = Game::new(llm_client);

    // Setup Terminal
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen)?;
    let backend = CrosstermBackend::new(stdout);
    let terminal = Terminal::new(backend)?;

    let event_source = llm_text_adventure::tui::CrosstermEventSource;
    let mut tui = Tui::new(terminal, event_source);

    tui.run(&mut game).await?;

    Ok(())
}

#[cfg(target_arch = "wasm32")]
fn main() {
    // WASM entry point is in lib.rs
}
