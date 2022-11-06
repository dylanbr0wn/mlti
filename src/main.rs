use command::Process;
use message::{build_message_sender, Message, MessageType};
use owo_colors::Style;
use rand::Rng;

use anyhow::Result;
use task::Task;
use argh::FromArgs;

use std::i32::MAX;

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


#[derive(FromArgs)]
/// Launch some commands concurrently
pub struct Commands {
    /// names of processes
    #[argh(option, short = 'n')]
    names: Option<String>,

    /// name seperator character
    #[argh(option, default="default_names_separator()")]
    names_seperator: String,

    /// kill other processes if one exits.
    #[argh(switch, short = 'k')]
    kill_others: bool,

    /// kill other processes if one exits with a non-zero exit code.
    #[argh(switch)]
   kill_others_on_fail: bool,

    /// how many times a process will attempt to restart.
    #[argh(option, default="default_restart_tries()")]
    restart_tries: i64,

    /// amount of time to delay between restart attempts.
    #[argh(option, default="default_restart_after()")]
    restart_after: i64,

    /// prefixed used in logging for each process.
    #[argh(option, short = 'p')]
    prefix: Option<String>,

    /// max number of characters of prefix that are shown.
    #[argh(option, short = 'l', default="default_prefix_length()")]
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
    version: bool,

    /// timestamp format for logging
    #[argh(option, short = 't', default = "String::from(\"%Y-%m-%d %H:%M:%S\")")]
    timestamp_format: String,
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
}

pub struct CommandParser {
  pub names: Vec<String>,
  pub processes: Vec<String>,
  pub mlti_config: MltiConfig,
}

impl CommandParser {
  pub fn new(commands:Commands) -> Self {
    Self {
      names: parse_names(commands.names, commands.names_seperator),
      processes: commands.processes,
      mlti_config: MltiConfig {
        group: commands.group,
        kill_others: commands.kill_others,
        kill_others_on_fail: commands.kill_others_on_fail,
        restart_tries: commands.restart_tries,
        restart_after: commands.restart_after,
        prefix:   commands.prefix,
        prefix_length: commands.prefix_length,
        max_processes: parse_max_processes(commands.max_processes),
        raw:    commands.raw,
        no_color: commands.no_color,
        timestamp_format: commands.timestamp_format,
      },
    }
  }

  pub fn len(&self) -> usize {
    self.processes.len()
  }
  pub fn get_mlti_config(&self) -> MltiConfig {
    self.mlti_config.clone()
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
    None => MAX, // fuck it why not
  }
}

#[tokio::main]
async fn main() -> Result<()> {
  let commands: Commands = argh::from_env();
  let red_style = Style::new().red();
  let bold_green_style = Style::new().bold().green();
  let arg_parser = CommandParser::new(commands);
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

  if arg_parser.len() == 0 {
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

  let scheduler = scheduler::Scheduler::new(
    shutdown_tx.clone(),
    mlti_config.max_processes,
    arg_parser.len() as i32,
  );

  // let mut unnamed_counter = -1;

  let mut rng = rand::thread_rng();

  // let mut tasks: Vec<Task> = vec![];
  let task_queue = scheduler.get_task_queue();
  let kill_all = scheduler.get_kill_all();

  let scheduler_handler = tokio::spawn(async move {
    scheduler.run().await;
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
  print_message(
    SenderType::Main,
    "".into(),
    format!("\n{}", "Goodbye! 👋"),
    bold_green_style,
    mlti_config.raw,
    mlti_config.no_color,
  );
  Ok(())
}
