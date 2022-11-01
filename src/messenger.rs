use flume::{Receiver, Sender};
use owo_colors::{OwoColorize, Style};
use std::collections::VecDeque;

use crate::message::{Message, SenderType, MessageType};

pub struct Messenger {
  sender: Sender<Message>,
  receiver: Receiver<Message>,
  raw: bool,
  no_color: bool,
  group: bool,
  message_queue: Vec<VecDeque<Message>>,
}

impl Messenger {
  pub fn new(raw: bool, no_color: bool, num_commands: usize, group:bool) -> Self {
    let (sender, receiver) = flume::unbounded::<Message>();

    let message_queues = (0..num_commands)
      .map(|_| VecDeque::new())
      .collect::<Vec<_>>();

    Self {
      sender,
      receiver,
      raw,
      no_color,
      group,
      message_queue: message_queues,
    }
  }
  pub fn get_sender(&self) -> Sender<Message> {
    self.sender.clone()
  }
  pub async fn listen<F>(&mut self, handler: F)
  where
    F: Fn(Message, bool, bool) -> usize,
  {
    loop {
      let message = self.receiver.recv_async().await.ok();

        if let Some(message) = message {
          if self.group {
            match message.type_ {
              MessageType::Kill => {
                while let Some(m) = self.receiver.try_recv().ok() {
                  if let Some(i) = m.sender.index {
                    self.message_queue[i].push_back(m);
                  }
                }
               self.flush();
               break;
              }
              _ => {
                if let Some(i) = message.sender.index {
                  self.message_queue[i].push_back(message);
                }
              }
            }

          } else {
            let val = handler(message, self.raw, self.no_color);
            if val == 1 {
              break;
            }
          }
          // handler(message, self.raw, self.no_color);
        }


    }
  }
  pub fn flush(&mut self) {
    for queue in self.message_queue.iter_mut() {
      while let Some(message) = queue.pop_front() {
        print_message(message.sender.type_, message.name, message.data, message.style, self.raw, self.no_color);
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
  let mut message = String::new();
  if raw {
    if let SenderType::Process = sender_type {
      message = format!("{}", data);
    }
  } else {
    match sender_type {
      SenderType::Main => {
        message = format!("{}", print_color(data, style, no_color));
      }
      SenderType::Task => {

        message = format!(
          "[{}]: {}",
          print_color(name, style, no_color),
          print_color(data.bold().to_string(), style, no_color)
        );
      }
      _ => {
        message = format!("[{}]: {}", print_color(name, style, no_color), data);
      }
    }
  }
  println!("{}", message);
}

pub fn print_color(text: String, style: Style, no_color: bool) -> String {
  if no_color {
    text
  } else {
    return format!("{}", text.style(style));
  }
}

