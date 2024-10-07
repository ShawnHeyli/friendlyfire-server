use std::collections::VecDeque;
use std::fmt::Debug;
use std::sync::Arc;
use tokio::sync::{broadcast, Mutex};
use tokio::time::{sleep, Duration};

#[derive(Debug)]
pub struct TimedQueue<T> {
    queue: Arc<Mutex<VecDeque<(T, Duration)>>>,
    sender: broadcast::Sender<T>,
}

impl<T> TimedQueue<T>
where
    T: Send + Clone + 'static + Debug,
{
    pub fn new() -> Self {
        let (sender, _receiver) = broadcast::channel(100);
        let queue = Arc::new(Mutex::new(VecDeque::new()));

        // Spawn a task to process the queue
        tokio::spawn(Self::process_queue(queue.clone(), sender.clone()));

        TimedQueue { queue, sender }
    }

    pub async fn process_queue(
        queue: Arc<Mutex<VecDeque<(T, Duration)>>>,
        sender: broadcast::Sender<T>,
    ) {
        loop {
            let mut queue_guard = queue.lock().await;
            if let Some((item, duration)) = queue_guard.pop_front() {
                println!("Sending item: {:?}", item);
                drop(queue_guard); // Release the lock while sending and sleeping

                // Send the item immediately
                if sender.send(item).is_err() {
                    break; // No active receivers
                }

                // Wait for the specified duration
                sleep(duration).await;
            } else {
                // If the queue is empty, wait a bit before checking again
                sleep(Duration::from_millis(100)).await;
            }
        }
    }

    pub async fn add(&self, item: T, delay: Duration) {
        let mut queue_guard = self.queue.lock().await;
        queue_guard.push_back((item, delay));
    }

    pub fn subscribe(&self) -> broadcast::Receiver<T> {
        self.sender.subscribe()
    }
}

#[cfg(test)]
mod tests {
    use tokio::time::timeout;

    use super::*;

    #[tokio::test]
    async fn test_add_and_receive() {
        let queue = TimedQueue::new();
        let mut receiver = queue.subscribe();
        let delay = Duration::from_millis(50);

        queue.add(1, delay).await;
        queue.add(2, delay).await;

        let received1 = timeout(Duration::from_millis(100), receiver.recv())
            .await
            .unwrap();
        assert_eq!(received1.unwrap(), 1);

        let received2 = timeout(Duration::from_millis(100), receiver.recv())
            .await
            .unwrap();
        assert_eq!(received2.unwrap(), 2);
    }

    #[tokio::test]
    async fn test_empty_queue() {
        let queue: TimedQueue<i32> = TimedQueue::new();

        let mut receiver = queue.subscribe();
        let received = timeout(Duration::from_millis(100), receiver.recv()).await;
        assert!(received.is_err()); // Should timeout as the queue is empty
    }

    #[tokio::test]
    async fn test_multiple_adds() {
        let queue = TimedQueue::new();
        let delay = Duration::from_millis(10);

        for i in 0..10 {
            queue.add(i, delay).await;
        }

        let mut receiver = queue.subscribe();
        for i in 0..10 {
            let received = timeout(Duration::from_millis(20), receiver.recv())
                .await
                .unwrap();
            assert_eq!(received.unwrap(), i);
        }
    }

    #[tokio::test]
    async fn test_timed_add() {
        let queue = TimedQueue::new();
        let delay = Duration::from_millis(100);

        let start = tokio::time::Instant::now();
        queue.add(42, delay).await;
        let mut receiver = queue.subscribe();
        let received = timeout(Duration::from_secs(1), receiver.recv())
            .await
            .unwrap();
        let elapsed = start.elapsed();

        assert_eq!(received.unwrap(), 42);
        assert!(elapsed <= delay);
    }

    #[tokio::test]
    async fn test_add_after_receive() {
        let queue = TimedQueue::new();
        let delay = Duration::from_millis(50);
        let mut receiver = queue.subscribe();

        queue.add(1, delay).await;
        let received1 = timeout(Duration::from_millis(100), receiver.recv())
            .await
            .unwrap();
        assert_eq!(received1.unwrap(), 1);

        queue.add(2, delay).await;

        let received2 = timeout(Duration::from_millis(100), receiver.recv())
            .await
            .unwrap();
        assert_eq!(received2.unwrap(), 2);
    }
}
