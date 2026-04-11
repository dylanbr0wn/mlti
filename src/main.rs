use command::Process;
use message::{build_message_sender, Message, MessageType};
use owo_colors::Style;
use rand::Rng;

use anyhow::Result;
use argh::FromArgs;
use task::Task;

use crate::{message::SenderType, messenger::print_message};

mod command;
mod message;
mod messenger;
mod scheduler;
mod task;

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
fn default_success() -> String {
  "all".to_string()
}
fn default_timestamp_format() -> String {
  String::from("%Y-%m-%d %H:%M:%S")
}

#[derive(FromArgs)]
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

  /// print version
  #[argh(switch, short = 'v')]
  #[allow(dead_code)]
  version: bool,

  /// timestamp format for logging
  #[argh(option, short = 't', default = "default_timestamp_format()")]
  timestamp_format: String,

  /// success condition: all, first, last, command-{{index|name}}, !command-{{index|name}}
  #[argh(option, short = 's', default = "default_success()")]
  success: String,

  /// print a duration summary for each process after completion
  #[argh(switch)]
  timings: bool,
}

#[derive(Clone)]
pub struct MltiConfig {
  pub kill_others: bool,
  pub kill_others_on_fail: bool,
  pub restart_tries: i64,
  pub restart_after: i64,
  pub prefix: Option<String>,
  pub prefix_length: i16,
  pub max_processes: i32,
  pub raw: bool,
  pub no_color: bool,
  pub group: bool,
  pub timestamp_format: String,
  pub timings: bool,
}

pub struct CommandParser {
  pub names: Vec<String>,
  pub processes: Vec<String>,
  pub mlti_config: MltiConfig,
  success_condition: SuccessCondition,
}

/// Parse a boolean from an environment variable value.
/// Treats "true" and "1" (case-insensitive) as true, everything else as false.
/// Returns None if the variable is missing or empty.
#[cfg_attr(not(test), allow(dead_code))]
fn env_bool(key: &str) -> Option<bool> {
  std::env::var(key).ok().filter(|v| !v.is_empty()).map(|v| {
    let v = v.to_lowercase();
    v == "true" || v == "1"
  })
}

/// Read an environment variable and parse it, returning None on missing or invalid values.
/// Prints a warning to stderr if the value is present but cannot be parsed.
#[cfg_attr(not(test), allow(dead_code))]
fn env_parse<T: std::str::FromStr>(key: &str) -> Option<T> {
  match std::env::var(key) {
    Ok(v) if v.is_empty() => None,
    Ok(v) => match v.parse::<T>() {
      Ok(parsed) => Some(parsed),
      Err(_) => {
        eprintln!("[mlti] warning: ignoring invalid value for {key}: {v:?}");
        None
      }
    },
    Err(_) => None,
  }
}

impl CommandParser {
  pub fn new(commands: Commands) -> Result<Self, String> {
    let success_condition = SuccessCondition::parse(&commands.success)?;

    // For boolean switches: CLI true means explicitly set; otherwise fall back to env var.
    let kill_others =
      commands.kill_others || env_bool("MLTI_KILL_OTHERS").unwrap_or(false);
    let kill_others_on_fail = commands.kill_others_on_fail
      || env_bool("MLTI_KILL_OTHERS_ON_FAIL").unwrap_or(false);
    let raw = commands.raw || env_bool("MLTI_RAW").unwrap_or(false);
    let no_color = commands.no_color
      || env_bool("MLTI_NO_COLOR").unwrap_or(false)
      || std::env::var("NO_COLOR").is_ok_and(|v| !v.is_empty());
    let group = commands.group || env_bool("MLTI_GROUP").unwrap_or(false);

    // For options with defaults: if CLI value equals the default, try the env var.
    let restart_tries = if commands.restart_tries != default_restart_tries() {
      commands.restart_tries
    } else {
      env_parse::<i64>("MLTI_RESTART_TRIES").unwrap_or(commands.restart_tries)
    };

    let restart_after = if commands.restart_after != default_restart_after() {
      commands.restart_after
    } else {
      env_parse::<i64>("MLTI_RESTART_AFTER").unwrap_or(commands.restart_after)
    };

    let prefix_length = if commands.prefix_length != default_prefix_length() {
      commands.prefix_length
    } else {
      env_parse::<i16>("MLTI_PREFIX_LENGTH").unwrap_or(commands.prefix_length)
    };

    // For Option<String> fields: CLI Some wins; otherwise try env var.
    let prefix = commands
      .prefix
      .or_else(|| std::env::var("MLTI_PREFIX").ok());
    let names = commands.names.or_else(|| std::env::var("MLTI_NAMES").ok());
    let names_separator = if commands.names_seperator != default_names_separator() {
      commands.names_seperator
    } else {
      std::env::var("MLTI_NAMES_SEPARATOR").unwrap_or(commands.names_seperator)
    };
    let max_processes = commands
      .max_processes
      .or_else(|| std::env::var("MLTI_MAX_PROCESSES").ok());

    // For timestamp_format: if CLI value equals the default, try the env var.
    let timestamp_format = if commands.timestamp_format != default_timestamp_format()
    {
      commands.timestamp_format
    } else {
      std::env::var("MLTI_TIMESTAMP_FORMAT").unwrap_or(commands.timestamp_format)
    };

    Ok(Self {
      names: parse_names(names, names_separator),
      processes: commands.processes,
      success_condition,
      mlti_config: MltiConfig {
        group,
        kill_others,
        kill_others_on_fail,
        restart_tries,
        restart_after,
        prefix,
        prefix_length,
        max_processes: parse_max_processes(max_processes),
        raw,
        no_color,
        timestamp_format,
        timings: commands.timings,
      },
    })
  }

  pub fn len(&self) -> usize {
    self.processes.len()
  }
  pub fn is_empty(&self) -> bool {
    self.processes.is_empty()
  }
  pub fn get_mlti_config(&self) -> MltiConfig {
    self.mlti_config.clone()
  }

  /// Compute the overall exit code from a collection of per-task
  /// `(index, code)` pairs, applying the configured success condition.
  pub fn evaluate_exit_code(&self, exit_codes: &[(usize, i32)]) -> i32 {
    self.success_condition.evaluate(exit_codes, &self.names)
  }
}

#[cfg_attr(test, derive(Debug, PartialEq))]
enum SuccessCondition {
  All,
  First,
  Last,
  CommandIndex(usize),
  CommandName(String),
  NotCommandIndex(usize),
  NotCommandName(String),
}

/// Extract the exit code from an optional `(index, code)` pair,
/// or return `default` if the pair is absent (e.g. command never ran).
fn code_at(pair: Option<&(usize, i32)>, default: i32) -> i32 {
  pair.map_or(default, |(_, code)| *code)
}

/// Find the first non-zero exit code among `exit_codes`, optionally skipping
/// the entry whose index matches `exclude`. Returns 0 if all remaining are 0.
fn first_nonzero(exit_codes: &[(usize, i32)], exclude: Option<usize>) -> i32 {
  exit_codes
    .iter()
    .filter(|(i, _)| exclude != Some(*i))
    .find(|(_, code)| *code != 0)
    .map_or(0, |(_, code)| *code)
}

impl SuccessCondition {
  fn parse(s: &str) -> Result<Self, String> {
    match s {
      "all" => Ok(Self::All),
      "first" => Ok(Self::First),
      "last" => Ok(Self::Last),
      s if s.starts_with("!command-") => {
        let val = &s["!command-".len()..];
        if let Ok(idx) = val.parse::<usize>() {
          Ok(Self::NotCommandIndex(idx))
        } else {
          Ok(Self::NotCommandName(val.to_string()))
        }
      }
      s if s.starts_with("command-") => {
        let val = &s["command-".len()..];
        if let Ok(idx) = val.parse::<usize>() {
          Ok(Self::CommandIndex(idx))
        } else {
          Ok(Self::CommandName(val.to_string()))
        }
      }
      other => Err(format!(
        "Invalid success condition: '{}'. Expected: all, first, last, \
         command-{{name|index}}, !command-{{name|index}}",
        other
      )),
    }
  }

  fn evaluate(&self, exit_codes: &[(usize, i32)], names: &[String]) -> i32 {
    if exit_codes.is_empty() {
      return 1;
    }
    match self {
      Self::All => first_nonzero(exit_codes, None),
      Self::First => code_at(exit_codes.first(), 1),
      Self::Last => code_at(exit_codes.last(), 1),
      Self::CommandIndex(idx) => {
        code_at(exit_codes.iter().find(|(i, _)| i == idx), 1)
      }
      Self::CommandName(name) => match names.iter().position(|n| n == name) {
        Some(idx) => code_at(exit_codes.iter().find(|(i, _)| *i == idx), 1),
        None => 1,
      },
      Self::NotCommandIndex(idx) => first_nonzero(exit_codes, Some(*idx)),
      Self::NotCommandName(name) => match names.iter().position(|n| n == name) {
        // Unknown name is a misconfiguration — fail rather than silently
        // degenerating to `all`, which hid bugs in practice.
        Some(idx) => first_nonzero(exit_codes, Some(idx)),
        None => 1,
      },
    }
  }
}

pub fn parse_names(names: Option<String>, seperator: String) -> Vec<String> {
  let names = match names {
    Some(names) => names.split(&seperator).map(|x| x.to_string()).collect(),
    None => vec![],
  };
  names
}

pub fn parse_max_processes(max_processes: Option<String>) -> i32 {
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

#[tokio::main]
async fn main() -> Result<()> {
  let commands: Commands = argh::from_env();
  let red_style = Style::new().red();
  let bold_green_style = Style::new().bold().green();
  let arg_parser = CommandParser::new(commands).unwrap_or_else(|e| {
    eprintln!("{}", e);
    std::process::exit(1);
  });
  let mlti_config = arg_parser.get_mlti_config();

  let mut shutdown_messenger = messenger::Messenger::new(
    mlti_config.raw,
    mlti_config.no_color,
    arg_parser.len(),
    false,
  );
  let shutdown_tx = shutdown_messenger.get_sender();
  let mut messenger = messenger::Messenger::new(
    mlti_config.raw,
    mlti_config.no_color,
    arg_parser.len(),
    mlti_config.group,
  );
  let message_tx = messenger.get_sender();

  let messenger_handle = tokio::spawn(async move {
    messenger
      .listen(
        |message: Message, raw: bool, no_color: bool| match message.type_ {
          MessageType::Error => {
            print_message(
              message.sender.type_,
              message.name,
              message.data,
              message.style,
              raw,
              no_color,
            );
            0
          }
          MessageType::Text => {
            print_message(
              message.sender.type_,
              message.name,
              message.data,
              message.style,
              raw,
              no_color,
            );
            0
          }
          MessageType::Kill => 1,
          _ => 0,
        },
      )
      .await;
  });

  let ctrlx_tx = shutdown_tx.clone();
  ctrlc::set_handler(move || {
    ctrlx_tx
      .send(message::Message::new(
        message::MessageType::KillAll,
        None,
        None,
        None,
        build_message_sender(SenderType::Other, None, None),
      ))
      .expect("Could not send signal on channel.")
  })
  .expect("Error setting Ctrl-C handler");

  if arg_parser.is_empty() {
    print_message(
      SenderType::Main,
      "".into(),
      "No processes to run. Goodbye! 👋".into(),
      bold_green_style,
      mlti_config.raw,
      mlti_config.no_color,
    );
    messenger_handle.abort();
    return Ok(());
  }

  print_message(
    SenderType::Main,
    "".into(),
    format!("\n{} {}\n", arg_parser.len(), "processes to run ✅"),
    bold_green_style,
    mlti_config.raw,
    mlti_config.no_color,
  );

  let scheduler = std::sync::Arc::new(scheduler::Scheduler::new(
    shutdown_tx.clone(),
    mlti_config.max_processes,
    arg_parser.len() as i32,
  ));

  // let mut unnamed_counter = -1;

  let mut rng = rand::thread_rng();

  // let mut tasks: Vec<Task> = vec![];
  let task_queue = scheduler.get_task_queue();
  let kill_all = scheduler.get_kill_all();
  let scheduler_clone = scheduler.clone();

  let scheduler_handler = tokio::spawn(async move {
    scheduler_clone.run().await;
  });

  for i in 0..arg_parser.len() {
    let r = rng.gen_range(75..255);
    let g = rng.gen_range(75..255);
    let b = rng.gen_range(75..255);
    let name = arg_parser.names.get(i).map(|name| name.to_string());

    let my_cmd = Process::new(
      arg_parser.processes[i].clone(),
      name,
      i,
      mlti_config.prefix.clone(),
      mlti_config.prefix_length,
      (r, g, b),
      mlti_config.timestamp_format.clone(),
    );

    task_queue
      .send_async(Task::new(
        my_cmd,
        message_tx.clone(),
        shutdown_tx.clone(),
        mlti_config.to_owned(),
      ))
      .await
      .expect("Could not send task on channel.");
  }

  shutdown_messenger
    .listen(
      |message: Message, raw: bool, no_color: bool| match message.type_ {
        MessageType::KillAll => {
          print_message(
            SenderType::Main,
            "".into(),
            format!("\n{}", "Killing all processes"),
            red_style,
            raw,
            no_color,
          );
          message_tx
            .send(message::Message::new(
              message::MessageType::Kill,
              None,
              None,
              None,
              build_message_sender(SenderType::Main, None, None),
            ))
            .expect("Could not send kill signal on channel.");
          kill_all
            .send(())
            .expect("Could not send kill signal on channel.");

          1
        }

        MessageType::KillOthers => {
          print_message(
            SenderType::Main,
            "".into(),
            format!(
              "\n{}",
              "Kill others flag present, stopping other processes."
            ),
            red_style,
            raw,
            no_color,
          );
          message_tx
            .send(message::Message::new(
              message::MessageType::Kill,
              None,
              None,
              None,
              build_message_sender(SenderType::Main, None, None),
            ))
            .expect("Could not send kill signal on channel.");
          // messenger_handle.abort();
          kill_all
            .send(())
            .expect("Could not send kill signal on channel.");

          1
        }
        MessageType::KillAllOnError => {
          print_message(
            SenderType::Main,
            "".into(),
            format!(
              "\n{}",
              "Kill others on fail flag present, stopping other processes."
            ),
            red_style,
            raw,
            no_color,
          );
          message_tx
            .send(message::Message::new(
              message::MessageType::Kill,
              None,
              None,
              None,
              build_message_sender(SenderType::Main, None, None),
            ))
            .expect("Could not send kill signal on channel.");
          // messenger_handle.abort();
          kill_all
            .send(())
            .expect("Could not send kill signal on channel.");
          1
        }
        MessageType::Complete => {
          message_tx
            .send(message::Message::new(
              message::MessageType::Kill,
              None,
              None,
              None,
              build_message_sender(SenderType::Main, None, None),
            ))
            .expect("Could not send kill signal on channel.");
          kill_all.send(()).ok();
          1
        }
        _ => 0,
      },
    )
    .await;
  messenger_handle.await.ok();
  scheduler_handler.await.ok();

  let exit_codes = scheduler.get_exit_codes().await;
  let exit_code = arg_parser.evaluate_exit_code(&exit_codes);

  if mlti_config.timings {
    let mut timings = scheduler.get_timings().await;
    let total_processes = arg_parser.len();
    timings.sort_by_key(|t| t.index);
    print_message(
      SenderType::Main,
      "".into(),
      "\nTimings:".into(),
      bold_green_style,
      mlti_config.raw,
      mlti_config.no_color,
    );
    for t in &timings {
      let style = if t.exit_code != 0 {
        red_style
      } else {
        bold_green_style
      };
      print_message(
        SenderType::Main,
        "".into(),
        format!(
          "  [{}] {} \u{2014} {:.2}s",
          t.index, t.raw_cmd, t.duration_secs
        ),
        style,
        mlti_config.raw,
        mlti_config.no_color,
      );
    }
    if timings.len() < total_processes {
      print_message(
        SenderType::Main,
        "".into(),
        format!(
          "  ({} process(es) killed before completion)",
          total_processes - timings.len()
        ),
        red_style,
        mlti_config.raw,
        mlti_config.no_color,
      );
    }
  }

  print_message(
    SenderType::Main,
    "".into(),
    format!("\n{}", "Goodbye! 👋"),
    bold_green_style,
    mlti_config.raw,
    mlti_config.no_color,
  );

  if exit_code != 0 {
    std::process::exit(exit_code);
  }

  Ok(())
}

#[cfg(test)]
mod tests {
  use super::*;

  // ---- SuccessCondition::parse ----

  #[test]
  fn parse_simple_variants() {
    assert_eq!(
      SuccessCondition::parse("all").unwrap(),
      SuccessCondition::All
    );
    assert_eq!(
      SuccessCondition::parse("first").unwrap(),
      SuccessCondition::First
    );
    assert_eq!(
      SuccessCondition::parse("last").unwrap(),
      SuccessCondition::Last
    );
  }

  #[test]
  fn parse_command_index_and_name() {
    assert_eq!(
      SuccessCondition::parse("command-0").unwrap(),
      SuccessCondition::CommandIndex(0)
    );
    assert_eq!(
      SuccessCondition::parse("command-42").unwrap(),
      SuccessCondition::CommandIndex(42)
    );
    assert_eq!(
      SuccessCondition::parse("command-server").unwrap(),
      SuccessCondition::CommandName("server".to_string())
    );
  }

  #[test]
  fn parse_not_command_index_and_name() {
    assert_eq!(
      SuccessCondition::parse("!command-0").unwrap(),
      SuccessCondition::NotCommandIndex(0)
    );
    assert_eq!(
      SuccessCondition::parse("!command-watcher").unwrap(),
      SuccessCondition::NotCommandName("watcher".to_string())
    );
  }

  #[test]
  fn parse_rejects_invalid() {
    assert!(SuccessCondition::parse("").is_err());
    assert!(SuccessCondition::parse("nope").is_err());
    assert!(SuccessCondition::parse("commands-0").is_err());
  }

  // ---- SuccessCondition::evaluate ----

  fn codes(pairs: &[(usize, i32)]) -> Vec<(usize, i32)> {
    pairs.to_vec()
  }

  #[test]
  fn evaluate_empty_returns_error_code() {
    // Empty exit_codes is a defensive case; the main loop short-circuits
    // earlier, but evaluate should still return a non-zero sentinel.
    assert_eq!(SuccessCondition::All.evaluate(&[], &[]), 1);
  }

  #[test]
  fn evaluate_all_returns_zero_when_all_succeed() {
    let exit_codes = codes(&[(0, 0), (1, 0), (2, 0)]);
    assert_eq!(SuccessCondition::All.evaluate(&exit_codes, &[]), 0);
  }

  #[test]
  fn evaluate_all_returns_first_nonzero() {
    // Order is completion order, not definition order.
    let exit_codes = codes(&[(2, 0), (0, 7), (1, 3)]);
    assert_eq!(SuccessCondition::All.evaluate(&exit_codes, &[]), 7);
  }

  #[test]
  fn evaluate_first_and_last_follow_completion_order() {
    let exit_codes = codes(&[(2, 5), (0, 0), (1, 9)]);
    assert_eq!(SuccessCondition::First.evaluate(&exit_codes, &[]), 5);
    assert_eq!(SuccessCondition::Last.evaluate(&exit_codes, &[]), 9);
  }

  #[test]
  fn evaluate_command_index_returns_that_commands_code() {
    let exit_codes = codes(&[(0, 0), (1, 42), (2, 0)]);
    assert_eq!(
      SuccessCondition::CommandIndex(1).evaluate(&exit_codes, &[]),
      42
    );
  }

  #[test]
  fn evaluate_command_index_missing_returns_one() {
    // e.g. --kill-others-on-fail killed the target command before it exited.
    let exit_codes = codes(&[(0, 0), (2, 0)]);
    assert_eq!(
      SuccessCondition::CommandIndex(1).evaluate(&exit_codes, &[]),
      1
    );
  }

  #[test]
  fn evaluate_command_name_resolves_via_names() {
    let names = vec!["build".to_string(), "serve".to_string(), "test".to_string()];
    let exit_codes = codes(&[(0, 0), (1, 7), (2, 0)]);
    assert_eq!(
      SuccessCondition::CommandName("serve".to_string())
        .evaluate(&exit_codes, &names),
      7
    );
  }

  #[test]
  fn evaluate_command_name_unknown_returns_one() {
    let names = vec!["build".to_string(), "serve".to_string()];
    let exit_codes = codes(&[(0, 0), (1, 0)]);
    assert_eq!(
      SuccessCondition::CommandName("missing".to_string())
        .evaluate(&exit_codes, &names),
      1
    );
  }

  #[test]
  fn evaluate_not_command_index_excludes_one() {
    // Command 1 failed but we don't care — index 0 also failed and should win.
    let exit_codes = codes(&[(0, 3), (1, 7), (2, 0)]);
    assert_eq!(
      SuccessCondition::NotCommandIndex(1).evaluate(&exit_codes, &[]),
      3
    );
  }

  #[test]
  fn evaluate_not_command_index_success_when_only_excluded_failed() {
    let exit_codes = codes(&[(0, 0), (1, 9), (2, 0)]);
    assert_eq!(
      SuccessCondition::NotCommandIndex(1).evaluate(&exit_codes, &[]),
      0
    );
  }

  #[test]
  fn evaluate_not_command_name_resolves_and_excludes() {
    let names = vec!["build".to_string(), "flaky".to_string(), "test".to_string()];
    let exit_codes = codes(&[(0, 4), (1, 9), (2, 0)]);
    assert_eq!(
      SuccessCondition::NotCommandName("flaky".to_string())
        .evaluate(&exit_codes, &names),
      4
    );
  }

  #[test]
  fn evaluate_not_command_name_unknown_returns_one() {
    // Regression: previously this silently degenerated to `all`, hiding
    // typos in CI configs. The unknown name must now fail loudly.
    let names = vec!["build".to_string(), "serve".to_string()];
    let exit_codes = codes(&[(0, 0), (1, 0)]);
    assert_eq!(
      SuccessCondition::NotCommandName("typo".to_string())
        .evaluate(&exit_codes, &names),
      1
    );
  }

  // ── env_bool ────────────────────────────────────────────────────────────────

  fn with_env_var<F: FnOnce()>(key: &str, value: &str, f: F) {
    std::env::set_var(key, value);
    f();
    std::env::remove_var(key);
  }

  fn without_env_var<F: FnOnce()>(key: &str, f: F) {
    std::env::remove_var(key);
    f();
  }

  #[test]
  fn env_bool_true_values() {
    with_env_var("MLTI_TEST_BOOL_TRUE_LOWER", "true", || {
      assert_eq!(env_bool("MLTI_TEST_BOOL_TRUE_LOWER"), Some(true));
    });
    with_env_var("MLTI_TEST_BOOL_TRUE_UPPER", "TRUE", || {
      assert_eq!(env_bool("MLTI_TEST_BOOL_TRUE_UPPER"), Some(true));
    });
    with_env_var("MLTI_TEST_BOOL_TRUE_MIXED", "True", || {
      assert_eq!(env_bool("MLTI_TEST_BOOL_TRUE_MIXED"), Some(true));
    });
    with_env_var("MLTI_TEST_BOOL_ONE", "1", || {
      assert_eq!(env_bool("MLTI_TEST_BOOL_ONE"), Some(true));
    });
  }

  #[test]
  fn env_bool_false_values() {
    with_env_var("MLTI_TEST_BOOL_FALSE_LOWER", "false", || {
      assert_eq!(env_bool("MLTI_TEST_BOOL_FALSE_LOWER"), Some(false));
    });
    with_env_var("MLTI_TEST_BOOL_FALSE_UPPER", "FALSE", || {
      assert_eq!(env_bool("MLTI_TEST_BOOL_FALSE_UPPER"), Some(false));
    });
    with_env_var("MLTI_TEST_BOOL_ZERO", "0", || {
      assert_eq!(env_bool("MLTI_TEST_BOOL_ZERO"), Some(false));
    });
    with_env_var("MLTI_TEST_BOOL_NO", "no", || {
      assert_eq!(env_bool("MLTI_TEST_BOOL_NO"), Some(false));
    });
    with_env_var("MLTI_TEST_BOOL_ANYTHING", "anything", || {
      assert_eq!(env_bool("MLTI_TEST_BOOL_ANYTHING"), Some(false));
    });
  }

  #[test]
  fn env_bool_empty_returns_none() {
    with_env_var("MLTI_TEST_BOOL_EMPTY", "", || {
      assert_eq!(env_bool("MLTI_TEST_BOOL_EMPTY"), None);
    });
  }

  #[test]
  fn env_bool_missing_returns_none() {
    without_env_var("MLTI_TEST_BOOL_MISSING_XYZ", || {
      assert_eq!(env_bool("MLTI_TEST_BOOL_MISSING_XYZ"), None);
    });
  }

  // ── env_parse ────────────────────────────────────────────────────────────────

  #[test]
  fn env_parse_valid_i64() {
    with_env_var("MLTI_TEST_PARSE_42", "42", || {
      assert_eq!(env_parse::<i64>("MLTI_TEST_PARSE_42"), Some(42));
    });
  }

  #[test]
  fn env_parse_negative_i64() {
    with_env_var("MLTI_TEST_PARSE_NEG5", "-5", || {
      assert_eq!(env_parse::<i64>("MLTI_TEST_PARSE_NEG5"), Some(-5));
    });
  }

  #[test]
  fn env_parse_invalid_returns_none() {
    with_env_var("MLTI_TEST_PARSE_INVALID", "notanumber", || {
      assert_eq!(env_parse::<i64>("MLTI_TEST_PARSE_INVALID"), None);
    });
  }

  #[test]
  fn env_parse_empty_returns_none() {
    with_env_var("MLTI_TEST_PARSE_EMPTY", "", || {
      assert_eq!(env_parse::<i64>("MLTI_TEST_PARSE_EMPTY"), None);
    });
  }

  #[test]
  fn env_parse_missing_returns_none() {
    without_env_var("MLTI_TEST_PARSE_MISSING_XYZ", || {
      assert_eq!(env_parse::<i64>("MLTI_TEST_PARSE_MISSING_XYZ"), None);
    });
  }

  // ── parse_names ──────────────────────────────────────────────────────────────

  #[test]
  fn parse_names_comma_separated() {
    assert_eq!(
      parse_names(Some("foo,bar,baz".to_string()), ",".to_string()),
      vec!["foo", "bar", "baz"]
    );
  }

  #[test]
  fn parse_names_custom_separator() {
    assert_eq!(
      parse_names(Some("foo|bar".to_string()), "|".to_string()),
      vec!["foo", "bar"]
    );
  }

  #[test]
  fn parse_names_none_returns_empty() {
    assert_eq!(parse_names(None, ",".to_string()), Vec::<String>::new());
  }

  // ── parse_max_processes ──────────────────────────────────────────────────────

  #[test]
  fn parse_max_processes_none_returns_i32_max() {
    assert_eq!(parse_max_processes(None), i32::MAX);
  }

  #[test]
  fn parse_max_processes_numeric() {
    assert_eq!(parse_max_processes(Some("4".to_string())), 4);
  }

  #[test]
  fn parse_max_processes_percentage() {
    let cpus = num_cpus::get();
    let expected = (cpus as f32 * 0.5) as i32;
    assert_eq!(parse_max_processes(Some("50%".to_string())), expected);
  }
}
