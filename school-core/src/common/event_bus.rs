use crate::common::domain::event::DomainEvent;
use async_trait::async_trait;
use std::sync::Arc;
use tokio::sync::broadcast;

#[async_trait]
pub trait EventBus: Send + Sync {
    async fn publish(&self, event: Arc<dyn DomainEvent>);
    async fn publish_batch(&self, events: Vec<Arc<dyn DomainEvent>>);
}

pub type SharedEventBus = Arc<dyn EventBus>;

pub struct InMemoryEventBus {
    sender: broadcast::Sender<Arc<dyn DomainEvent>>,
}

impl InMemoryEventBus {
    pub fn new(capacity: usize) -> Self {
        let (sender, _) = broadcast::channel(capacity);
        Self { sender }
    }

    pub fn subscribe(&self) -> broadcast::Receiver<Arc<dyn DomainEvent>> {
        self.sender.subscribe()
    }
}

#[async_trait]
impl EventBus for InMemoryEventBus {
    async fn publish(&self, event: Arc<dyn DomainEvent>) {
        // We ignore the error if there are no receivers
        let _ = self.sender.send(event);
    }

    async fn publish_batch(&self, events: Vec<Arc<dyn DomainEvent>>) {
        for event in events {
            let _ = self.sender.send(event);
        }
    }
}
