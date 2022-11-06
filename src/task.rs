use anyhow::{Result, Error};
use chrono::{Duration, Local};
use chrono_humanize::{Accuracy, HumanTime, Tense};
use flume::Sender;
use owo_colors::OwoColorize;
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::process::Child;

use crate::MltiConfig;
use crate::command::Process;
use crate::message::{build_message_sender, Message, MessageType, SenderType};


pub(crate) struct Task {
  process: Process,
  message_tx: Sender<Message>,
  shutdown_tx: Sender<Message>,
  mlti_config: MltiConfig,
  exit_code: Option<i32>,
}

impl Task {
  pub fn new(
    process: Process,
    message_tx: Sender<Message>,
    shutdown_tx: Sender<Message>,
    mlti_config: MltiConfig,
  ) -> Self {
    Self {
      process,
      message_tx,
      shutdown_tx,
      mlti_config,
      exit_code: None,
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
      build_message_sender(SenderType::Task, None, None),
    ))
    .await
    .expect("Could not send message on channel.");
  }
  pub async fn start(&mut self) -> Result<i32> {
    let mut child: Option<Child> = None;

    let mut restart_attemps = self.mlti_config.restart_after - 1;

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
            if self.mlti_config.kill_others_on_fail {
              self
                .shutdown_tx
                .send_async(Message::new(
                  MessageType::KillAllOnError,
                  None,
                  None,
                  None,
                  build_message_sender(SenderType::Task, None, None),
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
                get_relative_time_from_ms(self.mlti_config.restart_after)
              )).await;
            restart_attemps -= 1;
            tokio::time::sleep(std::time::Duration::from_millis(
              self.mlti_config.restart_after as u64,
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

    let handle = tokio::spawn(async move {
      child
        .wait()
        .await
        .expect("child process encountered an error")
    });

    while let Some(line) = reader.next_line().await.unwrap_or_default() {
      self
        .message_tx
        .send_async(Message::new(
          MessageType::Text,
          Some(self.process.name.clone()),
          Some(line),
          Some(self.process.color),
          build_message_sender(
            SenderType::Process,
            Some(self.process.index),
            Some(self.process.name.clone()),
          ),
        ))
        .await
        .expect("Couldnt send message to main thread");
    }
    let status = handle.await.unwrap();
    self.exit_code = Some(status.code().unwrap_or(-1));
    self
      .message_tx
      .send_async(Message::new(
        MessageType::Text,
        Some(self.process.name.clone()),
        Some("Done!".into()),
        Some(self.process.color),
        build_message_sender(SenderType::Task, None, None),
      ))
      .await
      .expect("Couldnt send message to main thread");
    if self.mlti_config.kill_others {
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
