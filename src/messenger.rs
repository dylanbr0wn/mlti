use flume::{Receiver, Sender};
use owo_colors::{OwoColorize, Style};

use crate::message::{Message, SenderType};

pub struct Messenger {
  sender: Sender<Message>,
  receiver: Receiver<Message>,
  raw: bool,
  no_color: bool,
}

impl Messenger {
  pub fn new(raw: bool, no_color: bool) -> Self {
    let (sender, receiver) = flume::unbounded::<Message>();

    Self {
      sender,
      receiver,
      raw,
      no_color,
    }
  }
  pub fn get_sender(&self) -> Sender<Message> {
    self.sender.clone()
  }
  pub fn get_receiver(&self) -> Receiver<Message> {
    self.receiver.clone()
  }
  pub async fn listen<F>(&self, handler: F)
  where
    F: Fn(Message, bool, bool),
  {
    loop {
      let message = self.receiver.recv_async().await.ok();
      if let Some(message) = message {
        handler(message, self.raw, self.no_color);
      }
    }
  }
}

pub fn print_message(
  sender_type: SenderType,
  name: String,
  data: String,
  style: Style,
  raw: bool,
  no_color: bool,
) {
  if raw {
    match sender_type {
      SenderType::Process => {
        println!("{}", data);
      }

      _ => {}
    }
  } else {
    match sender_type {
      SenderType::Main => {
        println!("{}", print_color(data, style, no_color));
      }
      SenderType::Task => {
        println!(
          "[{}]: {}",
          print_color(name, style, no_color),
          print_color(data.bold().to_string(), style, no_color)
        );
      }
      _ => {
        println!("[{}]: {}", print_color(name, style, no_color), data);
      }
    }
  }
}

pub fn print_color(text: String, style: Style, no_color: bool) -> String {
  if no_color {
    return format!("{}", text);
  } else {
    return format!("{}", text.style(style));
  }
}
