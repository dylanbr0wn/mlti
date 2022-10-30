use std::process::Stdio;

use anyhow::Result;
use chrono::{Duration, Local};
use chrono_humanize::{Accuracy, HumanTime, Tense};
use flume::Sender;
use owo_colors::OwoColorize;
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::process::Child;

use crate::command::Command;
use crate::message::{Message, MessageType};

pub(crate) struct Task {
    command: Command,
    sender: Sender<Message>,
    color: (u8, u8, u8),
    kill_others_on_fail: bool,
    kill_others: bool,
    restart_attemps: i64,
    restart_delay: i64,
    exit_code: Option<i32>,
}

impl Task {
    pub fn new(
        command: Command,
        sender: Sender<Message>,
        color: (u8, u8, u8),
        kill_others_on_fail: bool,
        kill_others: bool,
        restart_attemps: i64,
        restart_delay: i64,
    ) -> Self {
        Self {
            command,
            sender,
            color,
            kill_others_on_fail,
            kill_others,
            restart_attemps,
            restart_delay,
            exit_code: None,
        }
    }
    pub async fn start(&mut self) -> Result<i32> {
        let mut cmd = tokio::process::Command::new(self.command.cmd_string.clone());

        cmd.stdout(Stdio::piped());

        let mut child: Option<Child> = None;

        let mut restart_attemps = self.restart_attemps - 1;

        loop {
            let attempt_child = cmd.args(self.command.args.clone()).spawn();
            match attempt_child {
                Ok(c) => {
                    child = Some(c);
                    break;
                }
                Err(e) => {
                    self.sender
                        .send_async(Message::new(
                            MessageType::Error,
                            Some(self.command.name.clone()),
                            Some(format!("{}: {}", "Encountered an Error".red(), e.red())),
                            None,
                        ))
                        .await
                        .expect("Could not send message on channel.");
                    if restart_attemps <= 0 {
                        if self.kill_others_on_fail {
                            self.sender
                                .send_async(Message::new(
                                    MessageType::KillAllOnError,
                                    None,
                                    None,
                                    None,
                                ))
                                .await
                                .expect("Could not send message on channel.");
                        }
                        break;
                    } else {
                        self.sender
                            .send_async(Message::new(
                                MessageType::Error,
                                Some(self.command.name.clone()),
                                Some(format!(
                                    "{}{}",
                                    "Process failed to start, retrying in ".red(),
                                    get_relative_time_from_ms(self.restart_delay.clone())
                                )),
                                None,
                            ))
                            .await
                            .expect("Could not send message on channel.");
                        restart_attemps -= 1;
                        tokio::time::sleep(std::time::Duration::from_millis(
                            self.restart_delay as u64,
                        ))
                        .await;
                    }
                }
            }
        }

        let mut child = match child {
            Some(c) => c,
            None => {
                self.sender
                    .send_async(Message::new(
                        MessageType::Error,
                        Some(self.command.name.clone()),
                        Some(format!(
                            "{}",
                            "Encountered an Error: Could not start process.".red(),
                        )),
                        None,
                    ))
                    .await
                    .expect("Could not send message on channel.");
                return Ok(1);
            }
        };

        let stdout = child
            .stdout
            .take()
            .expect("child did not have a handle to stdout");

        let mut reader = BufReader::new(stdout).lines();
        // stdout

        let handle = tokio::spawn(async move {
            return child
                .wait()
                .await
                .expect("child process encountered an error");
        });

        while let Some(line) = reader.next_line().await.unwrap_or_default() {
            self.sender
                .send_async(Message::new(
                    MessageType::Text,
                    Some(self.command.name.clone()),
                    Some(line),
                    Some(self.color),
                ))
                .await
                .expect("Couldnt send message to main thread");
        }
        let status = handle.await.unwrap();
        self.exit_code = Some(status.code().unwrap_or(-1));
        self.sender
            .send_async(Message::new(
                MessageType::Text,
                Some(self.command.name.clone()),
                Some(format!(
                    "{}",
                    "Done!"
                        .bold()
                        .truecolor(self.color.0, self.color.1, self.color.2)
                )),
                Some(self.color),
            ))
            .await
            .expect("Couldnt send message to main thread");
        if self.kill_others {
            self.sender
                .send_async(Message::new(MessageType::KillAll, None, None, None))
                .await
                .expect("Could not send message on channel.");
        }

        Ok(self.exit_code.unwrap_or(1))
    }
}

fn get_relative_time_from_ms(ms: i64) -> String {
    let dt = Local::now() + Duration::milliseconds(ms);
    let ht = HumanTime::from(dt);
    let english = ht.to_text_en(Accuracy::Precise, Tense::Present);
    english
}
