use crate::{ batcher::signer::DigitalSignatureService, crypto::merkle};
use std::time::Duration;
use tokio::sync::mpsc;
use tokio::time::Instant;
pub struct BatcherConfig {
    pub max_batch_size: usize,
    pub max_wait_time: u64,
}

pub struct BatcherEngine<T> {
    config: BatcherConfig,
    receiver: mpsc::Receiver<T>,
    sender: mpsc::Sender<Vec<T>>,
}

impl<T: Send + 'static> BatcherEngine<T> {
    pub fn new(
        config: BatcherConfig,
        receiver: mpsc::Receiver<T>,
        sender: mpsc::Sender<Vec<T>>,
    ) -> Self {
        Self {
            config,
            receiver,
            sender,
        }
    }

    pub async fn run(&mut self) {
        let max_batch_size = self.config.max_batch_size;
        let max_wait_time = Duration::from_millis(self.config.max_wait_time);
        let mut batch: Vec<T> = Vec::with_capacity(max_batch_size);
        let sleeper = tokio::time::sleep(max_wait_time);
        tokio::pin!(sleeper);
        loop {
            tokio::select! {
                maybe_item = self.receiver.recv() => {
                    match maybe_item {
                        Some(item) => {
                            if batch.is_empty(){
                                sleeper.as_mut().reset(Instant::now()+max_wait_time);
                            }
                            batch.push(item);

                            if batch.len()>=max_batch_size && self.flush(&mut batch).await.is_err(){
                                break 
                            }
                        }
                        None => {
                            let _ = self.flush(&mut batch).await;
                            break;
                        }
                    };
                }

                
            }
        }
    }

    async fn flush(&self, batch: &mut Vec<T>) -> Result<(), mpsc::error::SendError<Vec<T>>> {
        if batch.is_empty() {
            return Ok(());
        }
        let full_batch = std::mem::replace(batch, Vec::with_capacity(self.config.max_batch_size));
        self.sender.send(full_batch).await
    }
}
