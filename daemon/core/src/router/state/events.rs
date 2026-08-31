//! Event broadcast bus + connected client registry.

use std::sync::Arc;

use tokio::sync::broadcast;

use crate::clients::ClientRegistry;
use crate::events::Event;

#[derive(Clone)]
pub struct EventsLayer {
    pub event_tx: broadcast::Sender<Event>,
    pub clients: Arc<ClientRegistry>,
}

impl EventsLayer {
    pub fn new(event_tx: broadcast::Sender<Event>) -> Self {
        Self {
            event_tx,
            clients: Arc::new(ClientRegistry::new()),
        }
    }
}
