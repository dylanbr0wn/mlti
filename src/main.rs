use ctrlc;
use owo_colors::OwoColorize;
use rand::Rng;

use anyhow::Result;

mod arg_parser;
mod command;
mod message;
mod messenger;
mod scheduler;
mod task;

#[tokio::main]
async fn main() -> Result<()> {
  let (shutdown_tx, shutdown_rx) = flume::unbounded::<message::Message>();

  let messenger = messenger::Messenger::new(shutdown_tx.clone());

  let tx = messenger.get_sender();
  let messenger_handle = messenger.listen();

  let ctrlx_tx = shutdown_tx.clone();
  ctrlc::set_handler(move || {
    ctrlx_tx
      .send(message::Message::new(
        message::MessageType::Kill,
        None,
        None,
        None,
      ))
      .expect("Could not send signal on channel.")
  })
  .expect("Error setting Ctrl-C handler");

  // let mut task_set = JoinSet::new();

  let arg_parser = arg_parser::ArgParser::new();

  if arg_parser.len() == 0 {
    println!("{}", "No processes to run. Goodbye! 👋".green().bold());
    messenger_handle.abort();
    return Ok(());
  }

  println!(
    "\n{} {}",
    arg_parser.len().to_string().bold().green(),
    "processes to run ✅\n".bold().green()
  );

  let mut scheduler =
    scheduler::Scheduler::new(tx.clone(), 2, arg_parser.len() as i32);

  let mut unnamed_counter = -1;

  let mut rng = rand::thread_rng();

  // let mut tasks: Vec<Task> = vec![];
  let scheduler_handler = scheduler.run().await;

  for i in 0..arg_parser.len() {
    let r = rng.gen_range(75..255);
    let g = rng.gen_range(75..255);
    let b = rng.gen_range(75..255);
    let name = match arg_parser.names.get(i) {
      Some(name) => Some(name.to_string()),
      None => None,
    };
    if name.is_none() {
      unnamed_counter += 1;
    }

    let raw_cmd = arg_parser.processes[i].clone();

    let new_tx = tx.clone();
    let my_cmd = command::Command::new(
      raw_cmd.to_string(),
      name,
      unnamed_counter as i32,
      arg_parser.prefix.clone(),
      arg_parser.prefix_length,
    );

    scheduler
      .add_task(task::Task::new(
        my_cmd,
        new_tx,
        (r, g, b),
        arg_parser.kill_others_on_fail,
        arg_parser.kill_others,
        arg_parser.restart_tries,
        arg_parser.restart_after,
      ))
      .await;
  }

  loop {
    let message = shutdown_rx.recv_async().await;

    match message {
      Ok(message) => {
        match message.type_ {
          message::MessageType::Kill => {
            println!();
            println!("{}", "Killing all processes".red());
            messenger_handle.abort();
            scheduler.shutdown().await;
            scheduler_handler.abort();
            println!();
            println!("{}", "Goodbye! 👋".bold().green());

            std::process::exit(0);
          }

          message::MessageType::KillAll => {
            println!(
              "\n{}",
              "Kill others flag present, stopping other processes.".red()
            );
            messenger_handle.abort();
            scheduler.shutdown().await;
            scheduler_handler.abort();
            println!("\n{}", "All processes stopped. Goodbye! 👋".bold().green());
            std::process::exit(0);
          }
          message::MessageType::KillAllOnError => {
            println!(
              "\n{}",
              "Kill others on fail flag present, stopping other processes.".red()
            );
            messenger_handle.abort();
            scheduler.shutdown().await;
            scheduler_handler.abort();
            println!("\n{}", "All processes stopped. Goodbye! 👋".bold().green());
            std::process::exit(1);
          }
          message::MessageType::Complete => {
            messenger_handle.abort();
            // scheduler.shutdown().await;
            scheduler_handler.abort();
            println!("\n{}", "All done. Goodbye! 👋".bold().green());
            // println!("\n{}", "All processes stopped. Goodbye! 👋".bold().green());
            std::process::exit(0);
          }
          _ => {}
        }
      }
      Err(_) => {
        println!("{}", "Channel closed".red());
        break;
      }
    }
  }
  Ok(())
}
