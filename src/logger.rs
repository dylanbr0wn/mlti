use anyhow::Result;
use tokio::{
  io::{AsyncBufReadExt, BufReader},
  process::ChildStdout,
};

use crate::command_parser::Process;

pub struct Logger<'a> {
  pub process: &'a Process,
}

impl<'a> Logger<'a> {
  pub fn new(process: &'a Process) -> Self {
    Self { process }
  }

  pub fn log(&self, line: String) {
    println!("{}", line);
  }

  pub fn error(&self, line: String) {
    eprintln!("{}", line);
  }

  pub async fn pipe(&self, mut stdout: Option<ChildStdout>) {
    let mut reader = BufReader::new(
      stdout
        .take()
        .expect("child did not have a handle to stdout"),
    )
    .lines();
    while let Some(line) = reader.next_line().await.unwrap() {
      self.log(line);
    }
  }
}
