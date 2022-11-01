use std::sync::Arc;

use flume::{Receiver, Sender};
use tokio::sync::RwLock;
use tokio::task::JoinSet;

use crate::message::{MessageType, SenderType, build_message_sender};
use crate::{message::Message, task::Task};

pub(crate) struct Scheduler {
  pub tasks_rx: Receiver<Task>,
  pub tasks_tx: Sender<Task>,
  shutdown_tx: Sender<Message>,
  max_processes: i32,
  running_processes: Arc<RwLock<i32>>,
  number_of_tasks: i32,
  kill_all_tx: Sender<()>,
  kill_all_rx: Receiver<()>,
}

impl Scheduler {
  pub fn new(
    shutdown_tx: Sender<Message>,
    max_processes: i32,
    number_of_tasks: i32,
  ) -> Self {
    let (tasks_tx, tasks_rx) = flume::unbounded::<Task>();
    let (kill_all_tx, kill_all_rx) = flume::unbounded::<()>();

    Self {
      tasks_rx,
      tasks_tx,
      shutdown_tx,
      max_processes,
      running_processes: Arc::new(RwLock::new(0)),
      number_of_tasks,
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

  pub async fn run(&self) {
    let mut completed_tasks = 0;
    let mut join_set = JoinSet::new();

    loop {
      let mut running_processes = self.running_processes.write().await;
      loop {
        // If we cant run anything, shortcircuit
        if completed_tasks == self.number_of_tasks
          || *running_processes == self.number_of_tasks
          || completed_tasks + *running_processes == self.number_of_tasks
          || *running_processes >= self.max_processes
        {
          break;
        } else {
          let task = self.tasks_rx.recv_async().await.ok();
          if let Some(mut task) = task {
            *running_processes += 1;
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
            *running_processes -= 1;
            if completed_tasks == self.number_of_tasks {
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
