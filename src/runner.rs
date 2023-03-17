use std::sync::Arc;

use crate::{command_parser::Process, logger::Logger};
use anyhow::Result;
use chrono::{DateTime, Utc};
use futures::stream::FuturesUnordered;
use tokio::{sync::RwLock, task::JoinSet};

pub(crate) struct Runner {
  processes: Vec<Process>,
  completed: Arc<RwLock<i32>>,
  running: Arc<RwLock<i32>>,
  failed: Arc<RwLock<i32>>,
}

impl Runner {
  pub fn new(processes: Vec<Process>) -> Self {
    Self {
      processes,
      completed: Arc::new(RwLock::new(0)),
      running: Arc::new(RwLock::new(0)),
      failed: Arc::new(RwLock::new(0)),
    }
  }

  pub async fn run(self) -> Result<()> {
    // let mut handles = vec![]
    let mut handles = JoinSet::new();
    let number_procs = self.processes.len() as i32;

    for process in self.processes {
      handles.spawn(async move { process.run(0, 0).await });
    }

    loop {
      tokio::select! {
        Some(completed_proc) = handles.join_next() => {
          if let Ok(proc) = completed_proc {
            println!("Completed process: {:?}", proc);
            let mut complete = self.completed.write().await;
            *complete +=1;
            let mut running = self.running.write().await;
            *running -= 1;
            if *complete == number_procs {
              break;
            }
          }


          let mut complete = self.completed.write().await;
            *complete +=1;
            let mut running = self.running.write().await;
            *running -= 1;
            if *complete == number_procs {
                break;
            }
        }
      }
    }
    Ok(())
  }
}
