use wasm_bindgen::prelude::*;
use crate::{Game, LlmClient, Tui};
use crate::tui::EventSource;
use ratatui::backend::TestBackend;
use ratatui::Terminal;
use std::cell::RefCell;
use std::collections::VecDeque;
use crossterm::event::{Event, KeyCode, KeyEvent, KeyEventKind, KeyEventState, KeyModifiers};
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;

// Thread-local event queue
thread_local! {
    static EVENT_QUEUE: RefCell<VecDeque<Event>> = RefCell::new(VecDeque::new());
}

#[wasm_bindgen]
pub fn send_input(key: String) {
    let code = match key.as_str() {
        "Enter" => KeyCode::Enter,
        "Backspace" => KeyCode::Backspace,
        "Escape" => KeyCode::Esc,
        c if c.len() == 1 => KeyCode::Char(c.chars().next().unwrap()),
        _ => return,
    };
    let event = Event::Key(KeyEvent {
        code,
        modifiers: KeyModifiers::empty(),
        kind: KeyEventKind::Press,
        state: KeyEventState::empty(),
    });
    EVENT_QUEUE.with(|q| q.borrow_mut().push_back(event));
}

pub struct WasmEventSource;

#[async_trait::async_trait]
impl EventSource for WasmEventSource {
    async fn next_event(&mut self) -> anyhow::Result<Option<Event>> {
        // Yield to JS loop to allow input events to be processed
        let promise = js_sys::Promise::resolve(&JsValue::NULL);
        wasm_bindgen_futures::JsFuture::from(promise).await.unwrap();

        Ok(EVENT_QUEUE.with(|q| q.borrow_mut().pop_front()))
    }
}

#[wasm_bindgen]
pub async fn start_game(base_url: String, model_name: String) -> Result<(), JsValue> {
    console_error_panic_hook::set_once();
    
    let llm_client = LlmClient::new(base_url, model_name);
    let mut game = Game::new(llm_client);

    // Use TestBackend to capture output
    // We need a custom wrapper to render to DOM on draw
    // But Tui takes ownership of Terminal.
    // We can't easily hook into draw unless we make a custom Backend.
    // Let's make a simple DomBackend that wraps TestBackend.
    
    let backend = DomBackend::new();
    let terminal = Terminal::new(backend).map_err(|e| JsValue::from_str(&e.to_string()))?;
    
    let event_source = WasmEventSource;
    let mut tui = Tui::new(terminal, event_source);

    tui.run(&mut game).await.map_err(|e| JsValue::from_str(&e.to_string()))?;

    Ok(())
}

struct DomBackend {
    inner: TestBackend,
}

impl DomBackend {
    fn new() -> Self {
        // Initialize with a reasonable size
        Self {
            inner: TestBackend::new(80, 24),
        }
    }

    fn render_to_dom(&self) {
        let buffer = self.inner.buffer();
        let mut html = String::new();
        
        for y in 0..buffer.area.height {
            for x in 0..buffer.area.width {
                let cell = buffer.get(x, y);
                html.push_str(cell.symbol());
            }
            html.push('\n');
        }

        let window = web_sys::window().unwrap();
        let document = window.document().unwrap();
        if let Some(element) = document.get_element_by_id("terminal") {
            element.set_inner_html(&html);
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
    fn get_cursor(&mut self) -> std::io::Result<(u16, u16)> { self.inner.get_cursor() }
    fn set_cursor(&mut self, x: u16, y: u16) -> std::io::Result<()> { self.inner.set_cursor(x, y) }
    fn clear(&mut self) -> std::io::Result<()> { self.inner.clear() }
    fn size(&self) -> std::io::Result<Rect> { self.inner.size() }
    fn window_size(&self) -> std::io::Result<ratatui::backend::WindowSize> { self.inner.window_size() }
    fn flush(&mut self) -> std::io::Result<()> { self.inner.flush() }
}
