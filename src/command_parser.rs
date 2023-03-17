use std::{ops::Index, process::Stdio};

use anyhow::Result;
use argh::FromArgs;

use chrono::{DateTime, Utc};
use tokio::process::Child;

use crate::logger::Logger;

fn default_restart_tries() -> i64 {
  0
}
fn default_restart_after() -> i64 {
  0
}
fn default_prefix_length() -> i16 {
  10
}
fn default_names_separator() -> String {
  ",".to_string()
}

#[derive(FromArgs, Clone)]
/// Launch some commands concurrently
pub struct Commands {
  /// names of processes
  #[argh(option, short = 'n')]
  names: Option<String>,

  /// name seperator character
  #[argh(option, default = "default_names_separator()")]
  names_seperator: String,

  /// kill other processes if one exits.
  #[argh(switch, short = 'k')]
  kill_others: bool,

  /// kill other processes if one exits with a non-zero exit code.
  #[argh(switch)]
  kill_others_on_fail: bool,

  /// hide the output of the processes.
  #[argh(option)]
  hide: Option<String>,

  /// how many times a process will attempt to restart.
  #[argh(option, default = "default_restart_tries()")]
  restart_tries: i64,

  /// amount of time to delay between restart attempts.
  #[argh(option, default = "default_restart_after()")]
  restart_after: i64,

  /// prefixed used in logging for each process.
  #[argh(option, short = 'p')]
  prefix: Option<String>,

  /// max number of characters of prefix that are shown.
  #[argh(option, short = 'l', default = "default_prefix_length()")]
  prefix_length: i16,

  /// how many process should run at once.
  #[argh(option, short = 'm')]
  max_processes: Option<String>,

  /// print raw output of process only.
  #[argh(switch, short = 'r')]
  raw: bool,

  /// disable color output.
  #[argh(switch)]
  no_color: bool,

  /// group outputs together as if processes where run sequentially.
  #[argh(switch, short = 'g')]
  group: bool,

  /// processes to run
  #[argh(positional)]
  processes: Vec<String>,

  // /// print version
  // #[argh(switch, short = 'v')]
  // version: bool,
  /// timestamp format for logging
  #[argh(option, short = 't', default = "String::from(\"%Y-%m-%d %H:%M:%S\")")]
  timestamp_format: String,
}

pub struct CommandParser {
  pub commands: Commands,
}

impl CommandParser {
  pub fn new() -> Self {
    Self {
      commands: argh::from_env(),
    }
  }

  pub fn parse(&self) -> Result<Vec<Process>> {
    let mut processes = vec![];

    let cmds = &self.commands;

    let names = parse_names(&cmds.names, &cmds.names_seperator);
    let max_processes = parse_max_processes(&cmds.max_processes);

    let mut index = 0;

    for cmd in &cmds.processes {
      let name = match names.get(index) {
        Some(name) => name.to_string(),
        None => cmd.clone(),
      };

      let new_cmd = cmd.to_string();

      let process = Process::new(name, new_cmd);

      processes.push(process);
      index += 1;
    }

    Ok(processes)
  }

  pub fn len(&self) -> usize {
    self.commands.processes.len()
  }
}

pub fn parse_names(names: &Option<String>, seperator: &String) -> Vec<String> {
  let names = match names {
    Some(names) => names.split(seperator).map(|x| x.to_string()).collect(),
    None => vec![],
  };
  names
}

pub fn parse_max_processes(max_processes: &Option<String>) -> i32 {
  match max_processes {
    Some(max) => {
      if max.contains('%') {
        let percentage = str::parse::<i32>(&max.replace('%', ""))
          .expect("Could not parse percentage");
        let cpus = num_cpus::get();

        (cpus as f32 * (percentage as f32 / 100.0)) as i32
      } else {
        str::parse::<i32>(&max).expect("Could not parse max processes")
      }
    }
    None => i32::MAX, // fuck it why not
  }
}

#[derive(Debug)]
pub struct Command {
  pub name: String,
  pub cmd: String,
}

impl Command {
  pub fn new(name: String, cmd: String) -> Self {
    Self { name, cmd }
  }
  pub fn expand_npm(mut self) -> Command {
    self.cmd = self.cmd.replace("npm:", "npm run ");
    self
  }
  pub fn expand_pnpm(mut self) -> Command {
    self.cmd = self.cmd.replace("pnpm:", "pnpm ");
    self
  }
  pub fn expand_yarn(mut self) -> Command {
    self.cmd = self.cmd.replace("yarn:", "yarn ");
    self
  }
  pub fn get_command(&self) -> String {
    self.cmd.split(" ").next().unwrap().to_string()
  }
  pub fn get_args(&self) -> Vec<String> {
    self.cmd.split(" ").skip(1).map(|x| x.to_string()).collect()
  }
}

#[derive(Debug)]
enum ProcessStatus {
  Running,
  Completed,
  Failed,
}

#[derive(Debug)]
pub struct CompleteProcess {
  restart_attempts: i32,
  started_time: DateTime<Utc>,
  ended_time: DateTime<Utc>,
  status: ProcessStatus,
}

// impl CompleteProcess {
//   pub fn new(
//     restart_attempts: i32,
//     started_time: DateTime<Utc>,
//     ended_time: DateTime<Utc>,
//     status: ProcessStatus,
//   ) -> Self {
//     Self {
//       restart_attempts,
//       started_time,
//       ended_time,
//       status,
//     }
//   }
// }

#[derive(Debug)]
pub struct Process {
  pub cmd: Command,
}
impl Process {
  pub fn new(name: String, cmd: String) -> Self {
    Self {
      cmd: Command::new(name, cmd)
        .expand_npm()
        .expand_pnpm()
        .expand_yarn(),
    }
  }

  pub async fn run(
    &self,
    attempts: i32,
    retry_delay: u64,
  ) -> Result<CompleteProcess> {
    let command = self.cmd.get_command();
    let args = self.cmd.get_args();
    let mut cmd = tokio::process::Command::new(&command);
    cmd.stdout(Stdio::piped()).args(args);

    let mut restart_attempts = attempts;

    let logger = Logger::new(self);

    let time_start = chrono::offset::Utc::now();

    let mut child: Option<Child> = None;

    loop {
      match cmd.spawn() {
        Ok(c) => {
          child = Some(c);
          break;
        }
        Err(e) => {
          if restart_attempts <= 0 {
            if e.kind() == std::io::ErrorKind::NotFound {
              println!("Command not found: {}", &command);
            } else {
              println!("Error: {}", e);
            }
            break;
          } else {
            restart_attempts -= 1;
            tokio::time::sleep(std::time::Duration::from_millis(retry_delay)).await;
          }
        }
      }
    }
    // attempt to start

    // if started, read stdout, else increment failed
    if let Some(c) = child {
      logger.pipe(c.stdout).await
    } else {
      return Ok(CompleteProcess {
        restart_attempts,
        started_time: time_start,
        ended_time: chrono::offset::Utc::now(),
        status: ProcessStatus::Failed,
      });
    }
    return Ok(CompleteProcess {
      restart_attempts,
      started_time: time_start,
      ended_time: chrono::offset::Utc::now(),
      status: ProcessStatus::Completed,
    });
  }
}

// impl Process {
//   pub fn new(
//     raw_cmd: String,
//     name: Option<String>,
//     index: usize,
//     prefix: Option<String>,
//     length: i16,
//     color: (u8, u8, u8),
//     timestamp_format: String
//   ) -> Self {
//     let parsed_cmd = parse(&raw_cmd).unwrap();

//     let mut args = parsed_cmd.split_whitespace();

//     let cmd_string = args.next().unwrap_or("");

//     let args = args.map(|x| x.to_string()).collect::<Vec<String>>();

//     let name = get_name(&raw_cmd, name, index, prefix, length, timestamp_format);

//     Self {
//       color,
//       index,
//       name,
//       args,
//       cmd: cmd_string.to_string(),
//     }
//   }
//   pub fn run(&self) -> Result<Child, std::io::Error> {
//     let mut cmd = tokio::process::Command::new(self.cmd.clone());
//     cmd.stdout(Stdio::piped());
//     cmd.args(self.args.clone());

//     cmd.spawn()
//   }
// }

// pub fn expand(cmd: &str) -> String {
//   let cmd = pnpm_expander(cmd);
//   npm_expander(&cmd)
// }

// pub fn parse(raw_cmd: &str) -> Result<String> {
//   let parts = expand(raw_cmd);
//   Ok(parts)
// }

// fn replace_prefix(prefix: String, key: String, value: String) -> String {
//   if prefix == key {
//     value
//   } else {
//     let format_str = format!("{{{}}}", key);
//     prefix.replace(&format_str, &value)
//   }
// }

// fn get_name(
//   raw_cmd: &str,
//   name: Option<String>,
//   index: usize,
//   prefix: Option<String>,
//   length: i16,
//   timestamp_format: String
// ) -> String {
//   // if Prefix template parse it

//   if let Some(prefix) = prefix {
//     let mut prefix = prefix;
//     let replace_list = vec![
//       ("index", index.to_string()),
//       ("command", raw_cmd.to_string()),
//       ("name", (&raw_cmd).to_string()),
//       ("pid", process::id().to_string()),
//       ("time", chrono::Local::now().format(&timestamp_format).to_string()),
//       ("none", "".to_string()),
//     ];

//     for (key, value) in replace_list {
//       prefix = replace_prefix(prefix, key.to_string(), value);
//     }

//     let prefix = truncate(&prefix, length.try_into().unwrap());
//     return prefix.to_string();
//   }

//   // if explicitly named, use that

//   if let Some(name) = name {
//     return name;
//   }

//   // if not, check for a npm command.

//   let is_pnpm_cmd: bool = raw_cmd.contains("pnpm:");
//   let is_npm_cmd: bool = raw_cmd.starts_with("npm:");
//   let is_yarn_cmd: bool = raw_cmd.contains("yarn:");
//   let default_name = format!("{}", index);

//   let backup_name: &str;

//   if is_pnpm_cmd {
//     backup_name = raw_cmd.split("pnpm:").collect::<Vec<&str>>()[1]
//   } else if is_yarn_cmd {
//     backup_name = raw_cmd.split("yarn:").collect::<Vec<&str>>()[1]
//   } else if is_npm_cmd {
//     backup_name = raw_cmd.split("npm:").collect::<Vec<&str>>()[1]
//   } else {
//     backup_name = &default_name;
//   }

//   backup_name.to_string()
// }

// fn truncate(s: &str, max_chars: usize) -> &str {
//   match s.char_indices().nth(max_chars) {
//     None => s,
//     Some((idx, _)) => &s[..idx],
//   }
// }
