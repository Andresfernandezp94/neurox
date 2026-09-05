//! Event broadcast bus + connected client registry.

use std::collections::HashMap;
use std::sync::Arc;

use parking_lot::Mutex;
use tokio::sync::broadcast;
use uuid::Uuid;

use crate::clients::ClientRegistry;
use crate::events::Event;

#[derive(Clone)]
pub struct EventsLayer {
    pub event_tx: broadcast::Sender<Event>,
    pub clients: Arc<ClientRegistry>,
    /// EP-2026-09-05 (stream seq): per-session monotonic counter used
    /// to stamp a strictly-increasing `seq` on every stream chunk
    /// (Content / Thinking / ToolCall / ToolResult) before it hits the
    /// broadcast bus. The client dedupes/orders stream chunks by this
    /// number, replacing the old text-based deduper. Sequences start at
    /// 1 (0 is reserved as the "unstamped / legacy" sentinel).
    seq_counters: Arc<Mutex<HashMap<Uuid, u64>>>,
}

impl EventsLayer {
    pub fn new(event_tx: broadcast::Sender<Event>) -> Self {
        Self {
            event_tx,
            clients: Arc::new(ClientRegistry::new()),
            seq_counters: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    /// Return the next strictly-increasing sequence number for a
    /// session. First call for a given `session_id` returns 1.
    #[must_use]
    pub fn next_seq(&self, session_id: Uuid) -> u64 {
        let mut map = self.seq_counters.lock();
        let counter = map.entry(session_id).or_insert(0);
        *counter += 1;
        *counter
    }
}
