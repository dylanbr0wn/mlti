use flume::{Receiver, Sender};
use tokio::task::JoinHandle;

use crate::message::{Message, MessageType};

pub struct Messenger {
    sender: Sender<Message>,
    receiver: Receiver<Message>,
    shutdown_tx: Sender<Message>,
}

impl Messenger {
    pub fn new(shutdown_tx: Sender<Message>) -> Self {
        let (sender, receiver) = flume::unbounded::<Message>();
        Self {
            sender,
            receiver,
            shutdown_tx,
        }
    }
    pub fn get_sender(&self) -> Sender<Message> {
        self.sender.clone()
    }
    pub fn listen(&self) -> JoinHandle<()> {
        let receiver = self.receiver.clone();
        let shutdown_tx = self.shutdown_tx.clone();
        let handle: JoinHandle<()> = tokio::spawn(async move {
            loop {
                let message = receiver.recv_async().await.unwrap();
                match message.type_ {
                    MessageType::Kill => {
                        shutdown_tx
                            .send_async(Message::new(MessageType::Kill, None, None, None))
                            .await
                            .expect("Could not send message on channel.");
                        // break;
                    }
                    MessageType::Error => {
                        message.print_message();
                    }
                    MessageType::Text => {
                        message.print_message();
                    }
                    MessageType::KillAll => {
                        shutdown_tx
                            .send_async(Message::new(MessageType::KillAll, None, None, None))
                            .await
                            .expect("Could not send message on channel.");
                        // break;
                    }
                    MessageType::KillAllOnError => {
                        shutdown_tx
                            .send_async(Message::new(MessageType::KillAllOnError, None, None, None))
                            .await
                            .expect("Could not send message on channel.");
                        // break;
                    }
                    MessageType::Complete => {
                        shutdown_tx
                            .send_async(Message::new(MessageType::Complete, None, None, None))
                            .await
                            .expect("Could not send message on channel.");
                        // break;
                    }
                }
            }
        });
        handle
        // loop {
        //     let message = self.receiver.recv().unwrap();
        //     message.print_message();
        // }
    }
}
