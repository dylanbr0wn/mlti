use message::{Message, MessageType};
use owo_colors::{OwoColorize, Style};
use rand::Rng;

use anyhow::Result;

use crate::{message::SenderType, messenger::print_message};

mod arg_parser;
mod command;
mod message;
mod messenger;
mod scheduler;
mod task;

#[tokio::main]
async fn main() -> Result<()> {
  let red_style = Style::new().red();
  let bold_green_style = Style::new().bold().green();

  let arg_parser = arg_parser::CommandParser::new();

  let command_args = arg_parser.get_command_args();

  let shutdown_messenger =
    messenger::Messenger::new(command_args.raw, command_args.no_color);
  let shutdown_tx = shutdown_messenger.get_sender();
  let shutdown_rx = shutdown_messenger.get_receiver();
  let messenger = messenger::Messenger::new(command_args.raw, command_args.no_color);
  let message_tx = messenger.get_sender();

  let messenger_handle = tokio::spawn(async move {
    messenger
      .listen(
        |message: Message, raw: bool, no_color: bool| match message.type_ {
          MessageType::Error => {
            print_message(
              message.sender,
              message.name,
              message.data,
              message.style,
              raw,
              no_color,
            );
          }
          MessageType::Text => {
            print_message(
              message.sender,
              message.name,
              message.data,
              message.style,
              raw,
              no_color,
            );
          }
          _ => {}
        },
      )
      .await;
  });

  let ctrlx_tx = shutdown_tx.clone();
  ctrlc::set_handler(move || {
    ctrlx_tx
      .send(message::Message::new(
        message::MessageType::Kill,
        None,
        None,
        None,
        message::SenderType::Other,
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
      command_args.raw,
      command_args.no_color,
    );
    messenger_handle.abort();
    return Ok(());
  }

  print_message(
    SenderType::Main,
    "".into(),
    format!("\n{} {}\n", arg_parser.len(), "processes to run ✅"),
    bold_green_style,
    command_args.raw,
    command_args.no_color,
  );

  let scheduler = scheduler::Scheduler::new(
    shutdown_tx.clone(),
    command_args.max_processes,
    arg_parser.len() as i32,
  );

  let mut unnamed_counter = -1;

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
    if name.is_none() {
      unnamed_counter += 1;
    }

    let raw_cmd = arg_parser.processes[i].clone();

    let my_cmd = command::Command::new(
      raw_cmd.to_string(),
      name,
      unnamed_counter as i32,
      command_args.prefix.clone(),
      command_args.prefix_length,
    );

    task_queue
      .send_async(task::Task::new(
        my_cmd,
        message_tx.clone(),
        shutdown_tx.clone(),
        (r, g, b),
        command_args.to_owned(),
      ))
      .await
      .expect("Could not send task on channel.");
  }

  shutdown_messenger
    .listen(
      |message: Message, raw: bool, no_color: bool| match message.type_ {
        MessageType::Kill => {
          print_message(
            SenderType::Main,
            "".into(),
            format!("\n{}", "Killing all processes"),
            red_style,
            raw,
            no_color,
          );
          messenger_handle.abort();
          kill_all
            .send(())
            .expect("Could not send kill signal on channel.");
          scheduler_handler.abort();
          print_message(
            SenderType::Main,
            "".into(),
            format!("\n{}", "Goodbye! 👋"),
            bold_green_style,
            raw,
            no_color,
          );

          std::process::exit(0);
        }

        MessageType::KillAll => {
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
          messenger_handle.abort();
          kill_all
            .send(())
            .expect("Could not send kill signal on channel.");
          scheduler_handler.abort();
          print_message(
            SenderType::Main,
            "".into(),
            format!("\n{}", "All processes stopped. Goodbye! 👋"),
            bold_green_style,
            raw,
            no_color,
          );
          std::process::exit(0);
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

          messenger_handle.abort();
          kill_all
            .send(())
            .expect("Could not send kill signal on channel.");
          scheduler_handler.abort();
          print_message(
            SenderType::Main,
            "".into(),
            format!("\n{}", "All processes stopped. Goodbye! 👋"),
            bold_green_style,
            raw,
            no_color,
          );
          std::process::exit(1);
        }
        MessageType::Complete => {
          messenger_handle.abort();
          kill_all.send(()).ok();
          scheduler_handler.abort();
          print_message(
            SenderType::Main,
            "".into(),
            format!("\n{}", "All done. Goodbye! 👋"),
            bold_green_style,
            raw,
            no_color,
          );
          std::process::exit(0);
        }
        _ => {}
      },
    )
    .await;

  loop {
    let message = shutdown_rx.recv_async().await;

    match message {
      Ok(message) => match message.type_ {
        MessageType::Kill => {
          println!();
          println!("{}", "Killing all processes".red());
          messenger_handle.abort();
          kill_all
            .send_async(())
            .await
            .expect("Could not send kill signal on channel.");
          scheduler_handler.await.ok();
          println!();
          println!("{}", "Goodbye! 👋".bold().green());

          std::process::exit(0);
        }

        MessageType::KillAll => {
          println!(
            "\n{}",
            "Kill others flag present, stopping other processes.".red()
          );
          messenger_handle.abort();
          kill_all
            .send_async(())
            .await
            .expect("Could not send kill signal on channel.");
          scheduler_handler.await.ok();
          println!("\n{}", "All processes stopped. Goodbye! 👋".bold().green());
          std::process::exit(0);
        }
        MessageType::KillAllOnError => {
          println!(
            "\n{}",
            "Kill others on fail flag present, stopping other processes.".red()
          );
          messenger_handle.abort();
          kill_all
            .send_async(())
            .await
            .expect("Could not send kill signal on channel.");
          scheduler_handler.await.ok();
          println!("\n{}", "All processes stopped. Goodbye! 👋".bold().green());
          std::process::exit(1);
        }
        MessageType::Complete => {
          messenger_handle.abort();
          kill_all.send_async(()).await.ok();
          scheduler_handler.await.ok();
          println!("\n{}", "All done. Goodbye! 👋".bold().green());
          std::process::exit(0);
        }
        _ => {}
      },
      Err(_) => {
        println!("{}", "Channel closed".red());
        break;
      }
    }
  }
  Ok(())
}
