use std::i32::MAX;

use flume::{Receiver, Sender};
use tokio::task::JoinSet;

use crate::Commands;
use crate::message::{build_message_sender, MessageType, SenderType};
use crate::{message::Message, task::Task};

pub(crate) struct Scheduler {
  pub tasks_rx: Receiver<Task>,
  pub tasks_tx: Sender<Task>,
  shutdown_tx: Sender<Message>,
  commands: Commands,
  // running_processes: Arc<usize>,
  running_processes: usize,
  kill_all_tx: Sender<()>,
  kill_all_rx: Receiver<()>,
}

impl Scheduler {
  pub fn new(
    shutdown_tx: Sender<Message>,
    commands: Commands
  ) -> Self {
    let (tasks_tx, tasks_rx) = flume::unbounded::<Task>();
    let (kill_all_tx, kill_all_rx) = flume::unbounded::<()>();

    Self {
      tasks_rx,
      tasks_tx,
      shutdown_tx,
      commands,
      running_processes: 0,
      kill_all_tx,
      kill_all_rx,
    }
  }
  pub fn get_task_queue(&self) -> Sender<Task> {
    self.tasks_tx.clone()
  }
  pub fn get_kill_all(&self) -> Sender<()> {
    self.kill_all_tx.clone()
  }

  pub async fn run(&mut self) {
    let mut completed_tasks = 0;
    let mut join_set = JoinSet::new();

    loop {
      // let mut running_processes = self.running_processes;


      let num_tasks = self.commands.processes.len();
      let max_processes =parse_max_processes(&self.commands.max_processes);

      loop {
        // If we cant run anything, shortcircuit
        if completed_tasks == num_tasks
          || self.running_processes == num_tasks
          || completed_tasks + self.running_processes == num_tasks
          || self.running_processes >= max_processes.try_into().unwrap()
        {
          break;
        } else {
          let task = self.tasks_rx.recv_async().await.ok();
          if let Some(mut task) = task {
            self.running_processes += 1;
            join_set.spawn(async move {
              match task.start().await {
                Ok(_code) => {}
                Err(e) => {
                  println!("{}", e);
                }
              }
            });
          }
        }
      }
      tokio::select! {
        _ = join_set.join_next() => {
            completed_tasks +=1;
            self.running_processes -= 1;
            if completed_tasks == num_tasks {
                break;
            }
        }
        _ = self.kill_all_rx.recv_async() => {

            join_set.shutdown().await;
            return;
        }
      }
    }
    self
      .shutdown_tx
      .send_async(Message::new(
        MessageType::Complete,
        None,
        None,
        None,
        build_message_sender(SenderType::Scheduler, None, None),
      ))
      .await
      .expect("Could not send message on channel.");
  }
}

pub fn parse_max_processes(max_processes: &Option<String>) -> i32 {

  if let Some(max) = max_processes {
    if max.contains('%') {
      let percentage = str::parse::<i32>(&max.replace('%', ""))
        .expect("Could not parse percentage");
      let cpus = num_cpus::get();

      (cpus as f32 * (percentage as f32 / 100.0)) as i32
    } else {
      str::parse::<i32>(max).expect("Could not parse max processes")

    }
  } else {
    MAX
  }

}
