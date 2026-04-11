use anyhow::Result;
use chrono::{Duration, Local};
use chrono_humanize::{Accuracy, HumanTime, Tense};
use flume::Sender;
use owo_colors::OwoColorize;
use std::time::Instant;
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::process::Child;

use crate::command::Process;
use crate::input_router::InputRouter;
use crate::message::{build_message_sender, Message, MessageType, SenderType};
use crate::MltiConfig;
use std::sync::Arc;

#[derive(Clone)]
pub struct TaskTiming {
  pub index: usize,
  pub raw_cmd: String,
  pub exit_code: i32,
  pub duration_secs: f64,
}

pub(crate) struct Task {
  process: Process,
  message_tx: Sender<Message>,
  shutdown_tx: Sender<Message>,
  mlti_config: MltiConfig,
  input_router: Option<Arc<InputRouter>>,
  exit_code: Option<i32>,
}

impl Task {
  pub fn new(
    process: Process,
    message_tx: Sender<Message>,
    shutdown_tx: Sender<Message>,
    mlti_config: MltiConfig,
    input_router: Option<Arc<InputRouter>>,
  ) -> Self {
    Self {
      process,
      message_tx,
      shutdown_tx,
      mlti_config,
      input_router,
      exit_code: None,
    }
  }
  pub fn index(&self) -> usize {
    self.process.index
  }
  pub fn process_info(&self) -> (usize, String) {
    (self.process.index, self.process.raw_cmd.clone())
  }

  fn make_timing(&self, exit_code: i32, duration_secs: f64) -> TaskTiming {
    TaskTiming {
      index: self.process.index,
      raw_cmd: self.process.raw_cmd.clone(),
      exit_code,
      duration_secs,
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
  pub async fn start(&mut self) -> Result<(i32, TaskTiming)> {
    let mut child: Option<Child> = None;

    let mut restart_attempts = self.mlti_config.restart_tries - 1;

    loop {
      // Deregister any previous stdin handle (no-op on first iteration)
      if let Some(ref router) = self.input_router {
        router.deregister(self.process.index).await;
      }
      let attempt_child = self.process.run(self.mlti_config.handle_input);
      match attempt_child {
        Ok(mut c) => {
          // Register stdin with the input router if enabled
          if let Some(ref router) = self.input_router {
            if let Some(stdin) = c.stdin.take() {
              router.register(self.process.index, stdin).await;
            }
          }
          child = Some(c);
          break;
        }
        Err(e) => {
          self
            .send_error(format!("{}: {}", "Encountered an Error".red(), e.red()))
            .await;
          if restart_attempts <= 0 {
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
              ))
              .await;
            restart_attempts -= 1;
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
          ))
          .await;
        return Ok((1, self.make_timing(1, 0.0)));
      }
    };

    let start_time = Instant::now();

    let stdout = child
      .stdout
      .take()
      .expect("child did not have a handle to stdout");

    let stderr = child
      .stderr
      .take()
      .expect("child did not have a handle to stderr");

    let mut stdout_reader = BufReader::new(stdout).lines();
    let mut stderr_reader = BufReader::new(stderr).lines();

    let handle = tokio::spawn(async move {
      child
        .wait()
        .await
        .expect("child process encountered an error")
    });

    let mut stdout_open = true;
    let mut stderr_open = true;

    loop {
      if !stdout_open && !stderr_open {
        break;
      }

      tokio::select! {
        result = stdout_reader.next_line(), if stdout_open => {
          match result {
            Ok(Some(line)) => {
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
            Ok(None) => stdout_open = false,
            Err(_) => stdout_open = false,
          }
        }
        result = stderr_reader.next_line(), if stderr_open => {
          match result {
            Ok(Some(line)) => {
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
            Ok(None) => stderr_open = false,
            Err(_) => stderr_open = false,
          }
        }
      }
    }
    let status = handle.await.unwrap();
    let code = status.code().unwrap_or(-1);
    self.exit_code = Some(code);
    // Deregister stdin from the input router
    if let Some(ref router) = self.input_router {
      router.deregister(self.process.index).await;
    }
    self
      .message_tx
      .send_async(Message::new(
        MessageType::Text,
        Some(self.process.name.clone()),
        Some(format!(
          "{} exited with code {}",
          self.process.raw_cmd, code
        )),
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

    let code = self.exit_code.unwrap_or(1);
    Ok((
      code,
      self.make_timing(code, start_time.elapsed().as_secs_f64()),
    ))
  }
}

fn get_relative_time_from_ms(ms: i64) -> String {
  let dt = Local::now() + Duration::milliseconds(ms);
  let ht = HumanTime::from(dt);

  ht.to_text_en(Accuracy::Precise, Tense::Present)
}
