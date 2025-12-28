use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum Command {
    New,
    Load,
    Up,
    Down,
    Delete,
    Enter,
    Back,
    Backspace,
    MoveNorth,
    MoveSouth,
    MoveEast,
    MoveWest,
    SelectOption(usize),
    TextInput(String),
    None,
}

impl Command {
    pub fn from_str(input: &str) -> Self {
        let input = input.trim().to_lowercase();

        match input.as_str() {
            "new" => Command::New,
            "load" => Command::Load,
            "up" => Command::Up,
            "down" => Command::Down,
            "delete" => Command::Delete,
            "enter" => Command::Enter,
            "back" => Command::Back,
            "backspace" => Command::Backspace,
            "go north" | "north" => Command::MoveNorth,
            "go south" | "south" => Command::MoveSouth,
            "go east" | "east" => Command::MoveEast,
            "go west" | "west" => Command::MoveWest,
            _ => {
                if let Ok(num) = input.parse::<usize>() {
                    Command::SelectOption(num)
                } else {
                    Command::TextInput(input.to_string())
                }
            }
        }
    }
}
