use std::time::{SystemTime, UNIX_EPOCH};

use owo_colors::Style;

#[derive(Debug)]
pub enum MessageType {
  Kill,
  Text,
  Error,
  KillAll,
  KillOthers,
  KillAllOnError,
  Complete,
}

#[derive(Clone)]
pub struct MessageSender {
  pub index: Option<usize>,
  pub name: String,
  pub type_: SenderType,
}

#[derive(Clone)]
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
  pub sender: MessageSender,
}

impl Message {
  pub fn new(
    type_: MessageType,
    name: Option<String>,
    data: Option<String>,
    color: Option<(u8, u8, u8)>,
    sender: MessageSender,
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

pub fn build_message_sender(
  sender_type: SenderType,
  index: Option<usize>,
  name: Option<String>,
) -> MessageSender {
  let name = name.unwrap_or_else(|| match sender_type {
    SenderType::Process => "Process".to_string(),
    SenderType::Scheduler => "Scheduler".to_string(),
    SenderType::Task => "Task".to_string(),
    SenderType::Other => "Other".to_string(),
    SenderType::Main => "Main".to_string(),
  });
  MessageSender {
    index,
    name,
    type_: sender_type,
  }
}
