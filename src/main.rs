use command::Process;
use message::{build_message_sender, Message, MessageType};
use owo_colors::Style;
use rand::Rng;

use anyhow::Result;
use task::Task;
use argh::FromArgs;

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


#[derive(FromArgs, Clone)]
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

    /// hide the output of the processes.
    #[argh(option)]
    hide: Option<String>,

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

    // /// print version
    // #[argh(switch, short = 'v')]
    // version: bool,

    /// timestamp format for logging
    #[argh(option, short = 't', default = "String::from(\"%Y-%m-%d %H:%M:%S\")")]
    timestamp_format: String,
}


pub fn parse_names(names: &Option<String>, seperator: &String) -> Vec<String> {

  let names = match names {
    Some(names) => names.split(seperator).map(|x| x.to_string()).collect(),
    None => vec![],
  };
  names
}


#[tokio::main]
async fn main() -> Result<()> {
  let commands: Commands = argh::from_env();
  let red_style = Style::new().red();
  let bold_green_style = Style::new().bold().green();

  let mut shutdown_messenger = messenger::Messenger::new(
    commands.raw,
    commands.no_color,
    commands.processes.len(),
    false,
  );
  let shutdown_tx = shutdown_messenger.get_sender();
  let mut messenger = messenger::Messenger::new(
    commands.raw,
    commands.no_color,
    commands.processes.len(),
    commands.group,
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

  if commands.processes.is_empty() {
    print_message(
      SenderType::Main,
      "".into(),
      "No processes to run. Goodbye! 👋".into(),
      bold_green_style,
      commands.raw,
      commands.no_color,
    );
    messenger_handle.abort();
    return Ok(());
  }

  print_message(
    SenderType::Main,
    "".into(),
    format!("\n{} {}\n", commands.processes.len(), "processes to run ✅"),
    bold_green_style,
    commands.raw,
    commands.no_color,
  );

  let mut scheduler = scheduler::Scheduler::new(
    shutdown_tx.clone(),
    commands.clone()
  );
  let mut rng = rand::thread_rng();

  // let mut tasks: Vec<Task> = vec![];
  let task_queue = scheduler.get_task_queue();
  let kill_all = scheduler.get_kill_all();

  let scheduler_handler = tokio::spawn(async move {
    scheduler.run().await;
  });

  let names = parse_names(&commands.names, &commands.names_seperator);
  let hidden_cmds = parse_hidden(&commands.hide);

  for (i,cmd) in commands.processes.iter().enumerate() {
    let r = rng.gen_range(75..255);
    let g = rng.gen_range(75..255);
    let b = rng.gen_range(75..255);
    let name = names.get(i).map(|x| x.to_string());


    let mut my_cmd = Process::new(
      cmd.clone(),
      name,
      i,
      commands.prefix.clone(),
      commands.prefix_length,
      (r, g, b),
      commands.timestamp_format.clone(),
    );

    my_cmd.set_hidden(hidden_cmds.contains(&my_cmd.name));

    task_queue
      .send_async(Task::new(
        my_cmd,
        message_tx.clone(),
        shutdown_tx.clone(),
        commands.restart_after,
        commands.kill_others_on_fail,
        commands.kill_others,
        commands.restart_tries
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
    commands.raw,
    commands.no_color,
  );
  Ok(())
}


fn parse_hidden(hide: &Option<String>) -> Vec<String> {
  match hide {
    Some(h) => h.split(",").map(|x| x.to_string()).collect(),
    None => vec![],
  }
}
