use std::{collections::VecDeque, sync::Arc, time::Duration};

use axum::extract::ws::Message;
use tokio::{sync::Mutex, time::sleep};

use crate::{MediaMessage, WsMessage};

#[derive(Default)]
pub struct MessageQueue {
    queue: VecDeque<MediaMessage>,
}

impl MessageQueue {
    pub fn new() -> Self {
        Self {
            queue: VecDeque::new(),
        }
    }

    // Add a message to the queue
    pub fn enqueue(&mut self, message: MediaMessage) {
        self.queue.push_back(message);
    }

    // Take the next message out of the queue (if available)
    pub fn dequeue(&mut self) -> Option<MediaMessage> {
        self.queue.pop_front()
    }

    // Check if the queue is empty
    pub fn is_empty(&self) -> bool {
        self.queue.is_empty()
    }
}

pub struct MediaProcessor {
    queue: Arc<Mutex<MessageQueue>>,
}

impl MediaProcessor {
    pub fn new() -> Self {
        Self {
            queue: Arc::new(Mutex::new(MessageQueue::new())),
        }
    }

    // Enqueue a message into the queue
    pub async fn add_message(&self, message: MediaMessage) {
        let mut queue = self.queue.lock().await;
        queue.enqueue(message);
    }

    // Process the queue: send a message and wait for the timeout
    pub async fn process_messages(&self) -> Result<(), ()> {
        loop {
            let next_message = {
                let mut queue = self.queue.lock().await;
                queue.dequeue()
            };

            if let Some(message) = next_message {
                // Send the message
                let ws_message =
                    Message::Text(serde_json::to_string(&WsMessage::Media(media)).unwrap());
                message.send().await?;

                // Wait for the message timeout
                sleep(Duration::from_millis(message.timeout)).await;
            } else {
                // If no messages are available, wait a little before checking again
                sleep(Duration::from_millis(100)).await;
            }
        }
    }
}
