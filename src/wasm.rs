use wasm_bindgen::prelude::*;
use crate::{Game, LlmClient, Tui};
use crate::tui::EventSource;
use ratatui::backend::TestBackend;
use ratatui::Terminal;
use std::cell::RefCell;
use std::collections::VecDeque;
use crate::input::{InputEvent, KeyCode, KeyEvent, KeyEventKind};
use ratatui::layout::Size;

// Thread-local event queue
thread_local! {
    static EVENT_QUEUE: RefCell<VecDeque<InputEvent>> = RefCell::new(VecDeque::new());
}

#[wasm_bindgen]
pub fn send_input(key: String) {
    let code = match key.as_str() {
        "Enter" => KeyCode::Enter,
        "Backspace" => KeyCode::Backspace,
        "Escape" => KeyCode::Esc,
        "ArrowUp" => KeyCode::Up,
        "ArrowDown" => KeyCode::Down,
        "ArrowLeft" => KeyCode::Left,
        "ArrowRight" => KeyCode::Right,
        c if c.len() == 1 => KeyCode::Char(c.chars().next().unwrap()),
        _ => return,
    };
    let event = InputEvent::Key(KeyEvent {
        code,
        kind: KeyEventKind::Press,
    });
    EVENT_QUEUE.with(|q| q.borrow_mut().push_back(event));
}

pub struct WasmEventSource;

#[async_trait::async_trait(?Send)]
impl EventSource for WasmEventSource {
    async fn next_event(&mut self) -> anyhow::Result<Option<InputEvent>> {
        if let Some(event) = EVENT_QUEUE.with(|q| q.borrow_mut().pop_front()) {
            web_sys::console::log_1(&format!("WASM: Processing event {:?}", event).into());
            return Ok(Some(event));
        }

        // Yield to JS loop to allow input events to be processed and browser to render
        let promise = js_sys::Promise::new(&mut |resolve, _| {
            web_sys::window().unwrap()
                .set_timeout_with_callback_and_timeout_and_arguments_0(&resolve, 10).unwrap();
        });
        let _ = wasm_bindgen_futures::JsFuture::from(promise).await;

        Ok(None)
    }
}

#[wasm_bindgen]
pub async fn start_game(base_url: String, model_name: String) -> Result<(), JsValue> {
    console_error_panic_hook::set_once();
    web_sys::console::log_1(&format!("WASM: start_game starting. URL: {}, Model: {}", base_url, model_name).into());
    
    let llm_client = LlmClient::new(base_url, model_name);
    let mut game = Game::new(llm_client);

    let backend = DomBackend::new();
    let terminal = Terminal::new(backend).map_err(|e| JsValue::from_str(&e.to_string()))?;
    
    let event_source = WasmEventSource;
    let mut tui = Tui::new(terminal, event_source);

    web_sys::console::log_1(&"WASM: Entering TUI run loop".into());
    tui.run(&mut game).await.map_err(|e| JsValue::from_str(&e.to_string()))?;

    Ok(())
}

struct DomBackend {
    inner: TestBackend,
}

impl DomBackend {
    fn new() -> Self {
        Self {
            inner: TestBackend::new(80, 24),
        }
    }

    fn render_to_dom(&self) {
        let buffer = self.inner.buffer();
        let mut full_text = String::new();
        
        for y in 0..buffer.area.height {
            for x in 0..buffer.area.width {
                let cell = &buffer[(x, y)];
                full_text.push_str(cell.symbol());
            }
            full_text.push('\n');
        }

        let window = web_sys::window().expect("no window");
        let document = window.document().expect("no document");
        
        if let Some(element) = document.get_element_by_id("terminal") {
            element.set_text_content(Some(&full_text));
        }

        // Also update state status if we can find it in the text (crude)
        if full_text.contains("Thinking...") {
            if let Some(el) = document.get_element_by_id("state-status") {
                el.set_text_content(Some("State: Thinking..."));
            }
        } else if let Some(el) = document.get_element_by_id("state-status") {
            el.set_text_content(Some("State: Ready"));
        }
    }
}

use ratatui::backend::Backend;

impl Backend for DomBackend {
    fn draw<'a, I>(&mut self, content: I) -> std::io::Result<()>
    where
        I: Iterator<Item = (u16, u16, &'a ratatui::buffer::Cell)>,
    {
        self.inner.draw(content)?;
        self.render_to_dom();
        Ok(())
    }

    fn hide_cursor(&mut self) -> std::io::Result<()> { self.inner.hide_cursor() }
    fn show_cursor(&mut self) -> std::io::Result<()> { self.inner.show_cursor() }
    fn get_cursor_position(&mut self) -> std::io::Result<ratatui::layout::Position> { self.inner.get_cursor_position() }
    fn set_cursor_position<P: Into<ratatui::layout::Position>>(&mut self, position: P) -> std::io::Result<()> { 
        self.inner.set_cursor_position(position) 
    }
    fn clear(&mut self) -> std::io::Result<()> { self.inner.clear() }
    fn size(&self) -> std::io::Result<Size> { self.inner.size() }
    fn window_size(&mut self) -> std::io::Result<ratatui::backend::WindowSize> { self.inner.window_size() }
    fn flush(&mut self) -> std::io::Result<()> { self.inner.flush() }
}
