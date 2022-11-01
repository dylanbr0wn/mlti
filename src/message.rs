use std::time::{SystemTime, UNIX_EPOCH};

use owo_colors::Style;

#[derive(Debug)]
pub enum MessageType {
  Kill,
  Text,
  Error,
  KillAll,
  KillAllOnError,
  Complete,
}

pub enum SenderType {
  Process,
  Scheduler,
  Task,
  Other,
  Main,
}

pub struct Message {
  pub name: String,
  pub timestamp: u64,
  pub data: String,
  pub style: Style,
  pub type_: MessageType,
  pub sender: SenderType,
}

impl Message {
  pub fn new(
    type_: MessageType,
    name: Option<String>,
    data: Option<String>,
    color: Option<(u8, u8, u8)>,
    sender: SenderType,
  ) -> Self {
    let color = color.unwrap_or((255, 255, 255));

    let style = Style::new().truecolor(color.0, color.1, color.2);

    Self {
      name: name.unwrap_or_default(),
      timestamp: SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_millis() as u64,
      data: data.unwrap_or_default(),
      style,
      type_,
      sender,
    }
  }
}
