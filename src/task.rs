use anyhow::{Result};
use chrono::{Duration, Local};
use chrono_humanize::{Accuracy, HumanTime, Tense};
use flume::Sender;
use owo_colors::OwoColorize;
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::process::Child;
use tokio::time::Instant;

use crate::command::Process;
use crate::message::{build_message_sender, Message, MessageType, SenderType, MessageSender};


pub(crate) struct Task {
  process: Process,
  message_tx: Sender<Message>,
  shutdown_tx: Sender<Message>,
  restart_after: i64,
  kill_others_on_fail: bool,
  kill_others: bool,
  restart_attempts: i64,
  exit_code: Option<i32>,
  signature: MessageSender
}

impl Task {
  pub fn new(
    process: Process,
    message_tx: Sender<Message>,
    shutdown_tx: Sender<Message>,
    restart_after: i64,
    kill_others_on_fail: bool,
  kill_others: bool,
  restart_attempts: i64,
  ) -> Self {
    Self {
      process,
      message_tx,
      shutdown_tx,
      restart_after,
      kill_others_on_fail,
  kill_others,
  restart_attempts,
      exit_code: None,
      signature: build_message_sender(SenderType::Task, None, None)
    }
  }
  pub async fn send_error(&self, error: String) {
    self
    .shutdown_tx
    .send_async(Message::new(
      MessageType::Error,
      Some(self.process.name.clone()),
      Some(error),
      None,
      self.signature.clone(),
    ))
    .await
    .expect("Could not send message on channel.");
  }
  pub async fn send_message(&self, message: Message) {
    if !self.process.hidden {
      self
      .message_tx
      .send_async(message)
      .await
      .expect("Couldnt send message to main thread");
    }
  }
  pub async fn start(&mut self) -> Result<i32> {
    let mut child: Option<Child> = None;

    let mut restart_attemps = self.restart_attempts - 1;

    loop {
      let attempt_child = self.process.run();
      match attempt_child {
        Ok(c) => {
          child = Some(c);
          break;
        }
        Err(e) => {
          self
            .send_error(format!("{}: {}", "Encountered an Error".red(), e.red())).await;
          if restart_attemps <= 0 {
            if self.kill_others_on_fail {
              self
                .shutdown_tx
                .send_async(Message::new(
                  MessageType::KillAllOnError,
                  None,
                  None,
                  None,
                  self.signature.clone()
                ))
                .await
                .expect("Could not send message on channel.");
            }
            break;
          } else {
            self
              .send_error(format!(
                "{}{}",
                "Process failed to start, retrying in ".red(),
                get_relative_time_from_ms(self.restart_after)
              )).await;
            restart_attemps -= 1;
            tokio::time::sleep(std::time::Duration::from_millis(
              self.restart_after as u64,
            ))
            .await;
          }
        }
      }
    }

    let mut child = match child {
      Some(c) => c,
      None => {
        self
          .send_error(format!(
            "{}",
            "Encountered an Error: Could not start process.".red(),
          )).await;
        return Ok(1);
      }
    };

    let stdout = child
      .stdout
      .take()
      .expect("child did not have a handle to stdout");

    let mut reader = BufReader::new(stdout).lines();
    let now = Instant::now();
    let handle = tokio::spawn(async move {
      child
        .wait()
        .await
        .expect("child process encountered an error")
    });

    // While running, send messages to the message_tx channel
    while let Some(line) = reader.next_line().await.unwrap_or_default() {
      self.send_message(Message::new(
        MessageType::Text,
        Some(self.process.name.clone()),
        Some(line),
        Some(self.process.color),
        build_message_sender(
          SenderType::Process,
          Some(self.process.index),
          Some(self.process.name.clone()),
        ),
      )).await;
    }
    let status = handle.await.unwrap();
    let elapsed = now.elapsed().as_millis();

    // Done running, send a message to the message_tx channel
    self.exit_code = Some(status.code().unwrap_or(-1));
    self.send_message(Message::new(
      MessageType::Text,
      Some(self.process.name.clone()),
      Some("Done!".into()),
      Some(self.process.color),
      self.signature.clone()
    )).await;

    //
    if self.kill_others {
      self
        .shutdown_tx
        .send_async(Message::new(
          MessageType::KillOthers,
          None,
          None,
          None,
          build_message_sender(SenderType::Task, None, None),
        ))
        .await
        .expect("Could not send message on channel.");
    }

    Ok(self.exit_code.unwrap_or(1))
  }
}

fn get_relative_time_from_ms(ms: i64) -> String {
  let dt = Local::now() + Duration::milliseconds(ms);
  let ht = HumanTime::from(dt);

  ht.to_text_en(Accuracy::Precise, Tense::Present)
}
