use std::sync::Arc;

use flume::{Receiver, Sender};
use tokio::sync::RwLock;
use tokio::task::{JoinHandle, JoinSet};

use crate::message::MessageType;
use crate::{message::Message, task::Task};

pub(crate) struct Scheduler {
    pub tasks_rx: Receiver<Task>,
    pub tasks_tx: Sender<Task>,
    shutdown_tx: Sender<Message>,
    max_processes: i32,
    running_processes: Arc<RwLock<i32>>,
    number_of_tasks: i32,
    tasks_queued: Arc<RwLock<i32>>,
    kill_all_tx: Sender<()>,
    kill_all_rx: Receiver<()>,
}

impl Scheduler {
    pub fn new(shutdown_tx: Sender<Message>, max_processes: i32, number_of_tasks: i32) -> Self {
        let (tasks_tx, tasks_rx) = flume::unbounded::<Task>();
        let (kill_all_tx, kill_all_rx) = flume::unbounded::<()>();
        Self {
            tasks_rx,
            tasks_tx,
            shutdown_tx,
            max_processes,
            running_processes: Arc::new(RwLock::new(0)),
            tasks_queued: Arc::new(RwLock::new(0)),
            number_of_tasks,
            kill_all_tx,
            kill_all_rx,
        }
    }
    pub async fn add_task(&mut self, task: Task) {
        let mut tasks_queued = self.tasks_queued.write().await;
        self.tasks_tx
            .send_async(task)
            .await
            .expect("Could not send task on channel.");
        *tasks_queued += 1;
    }
    pub async fn run(&self) -> JoinHandle<()> {
        let max_processes = self.max_processes.clone();
        let rx = self.tasks_rx.clone();
        let running_processes = self.running_processes.clone();
        let kill_all_rx = self.kill_all_rx.clone();
        let shutdown_tx = self.shutdown_tx.clone();
        let number_of_tasks = self.number_of_tasks.clone();
        return tokio::spawn(async move {
            let mut completed_tasks = 0;
            let mut join_set = JoinSet::new();

            loop {
                let mut running_processes = running_processes.write().await;
                loop {
                    // If we cant run anything, shortcircuit
                    if completed_tasks == number_of_tasks
                        || *running_processes == number_of_tasks
                        || completed_tasks + *running_processes == number_of_tasks
                        || *running_processes >= max_processes
                    {
                        break;
                    } else {
                        let task = rx.recv_async().await.ok();
                        if let Some(mut task) = task {
                            *running_processes += 1;
                            join_set.spawn(async move {
                                match task.start().await {
                                    Ok(code) => {}
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
                      if completed_tasks == number_of_tasks {
                          break;
                      }
                  }
                  _ = kill_all_rx.recv_async() => {

                      join_set.shutdown().await;
                      break;
                  }
                }
            }
            shutdown_tx
                .send_async(Message::new(MessageType::Complete, None, None, None))
                .await
                .expect("Could not send message on channel.");
        });
    }
    pub async fn shutdown(&mut self) {
        self.kill_all_tx
            .send_async(())
            .await
            .expect("Could not send kill signal on channel.");
    }
}
