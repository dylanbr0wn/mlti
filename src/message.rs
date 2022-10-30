use std::time::{SystemTime, UNIX_EPOCH};

use owo_colors::OwoColorize;

#[derive(Debug)]
pub enum MessageType {
    Kill,
    Text,
    Error,
    KillAll,
    KillAllOnError,
    Complete,
}

pub struct Message {
    pub name: String,
    pub timestamp: u64,
    pub data: String,
    color: (u8, u8, u8),
    pub type_: MessageType,
}

impl Message {
    pub fn new(
        type_: MessageType,
        name: Option<String>,
        data: Option<String>,
        color: Option<(u8, u8, u8)>,
    ) -> Self {
        Self {
            name: name.unwrap_or_default(),
            timestamp: SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_millis() as u64,
            data: data.unwrap_or_default(),
            color: color.unwrap_or((255, 255, 255)),
            type_,
        }
    }
    pub fn print_message(&self) {
        println!(
            "[{}]: {}",
            self.name
                .truecolor(self.color.0, self.color.1, self.color.2),
            self.data
        );
    }
}
