use command::Process;
use message::{Message, MessageType, build_message_sender};
use owo_colors::{ Style};
use rand::Rng;

use anyhow::Result;
use task::Task;

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

  let mlti_config = arg_parser.get_mlti_config();

  let mut shutdown_messenger =
    messenger::Messenger::new(mlti_config.raw, mlti_config.no_color, arg_parser.len(), false);
  let shutdown_tx = shutdown_messenger.get_sender();
  let mut messenger = messenger::Messenger::new(mlti_config.raw, mlti_config.no_color, arg_parser.len(), mlti_config.group);
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
          MessageType::Kill => {
            1
          }
          _ => { 0}
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
          message_tx.send(message::Message::new(
            message::MessageType::Kill,
            None,
            None,
            None,
            build_message_sender(SenderType::Main, None, None),
          )).expect("Could not send kill signal on channel.");
          // messenger_handle.abort();
          kill_all
            .send(())
            .expect("Could not send kill signal on channel.");
          // scheduler_handler.abort();
          // print_message(
          //   SenderType::Main,
          //   "".into(),
          //   format!("\n{}", "Goodbye! 👋"),
          //   bold_green_style,
          //   raw,
          //   no_color,
          // );

          // std::process::exit(0);
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
          message_tx.send(message::Message::new(
            message::MessageType::Kill,
            None,
            None,
            None,
            build_message_sender(SenderType::Main, None, None),
          )).expect("Could not send kill signal on channel.");
          // messenger_handle.abort();
          kill_all
            .send(())
            .expect("Could not send kill signal on channel.");
          // scheduler_handler.abort();
          // print_message(
          //   SenderType::Main,
          //   "".into(),
          //   format!("\n{}", "All processes stopped. Goodbye! 👋"),
          //   bold_green_style,
          //   raw,
          //   no_color,
          // );
          // std::process::exit(0);
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
          message_tx.send(message::Message::new(
            message::MessageType::Kill,
            None,
            None,
            None,
            build_message_sender(SenderType::Main, None, None),
          )).expect("Could not send kill signal on channel.");
          // messenger_handle.abort();
          kill_all
            .send(())
            .expect("Could not send kill signal on channel.");
          // scheduler_handler.abort();
          // print_message(
          //   SenderType::Main,
          //   "".into(),
          //   format!("\n{}", "All processes stopped. Goodbye! 👋"),
          //   bold_green_style,
          //   raw,
          //   no_color,
          // );
          // std::process::exit(1);
          1
        }
        MessageType::Complete => {
          message_tx.send(message::Message::new(
            message::MessageType::Kill,
            None,
            None,
            None,
            build_message_sender(SenderType::Main, None, None),
          )).expect("Could not send kill signal on channel.");
          // messenger_handle.abort();
          kill_all.send(()).ok();

          // scheduler_handler.abort();
          // print_message(
          //   SenderType::Main,
          //   "".into(),
          //   format!("\n{}", "All done. Goodbye! 👋"),
          //   bold_green_style,
          //   raw,
          //   no_color,
          // );
          // std::process::exit(0);
          1
        }
        _ => { 0}
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
