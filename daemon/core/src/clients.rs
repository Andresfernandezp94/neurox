// Runtime registry of active client connections (HTTP + WS). EP-0004.
// In-memory; not persisted. The map is keyed by a unique connection id
// (uuid) and the entry is removed when the connection closes.

use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::Instant;
use uuid::Uuid;

pub use crate::router::http::ClientInfo;

#[derive(Default)]
pub struct ClientRegistry {
    inner: Arc<Mutex<HashMap<String, ClientEntry>>>,
}

struct ClientEntry {
    info: ClientInfo,
    last_active: Instant,
}

impl ClientRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn register(&self, info: ClientInfo) -> String {
        let id = Uuid::new_v4().to_string();
        let mut map = self.inner.lock().expect("client registry mutex poisoned");
        map.insert(
            id.clone(),
            ClientEntry {
                info,
                last_active: Instant::now(),
            },
        );
        id
    }

    pub fn touch(&self, id: &str) {
        let mut map = self.inner.lock().expect("client registry mutex poisoned");
        if let Some(entry) = map.get_mut(id) {
            entry.last_active = Instant::now();
            entry.info.requests_count = entry.info.requests_count.saturating_add(1);
        }
    }

    pub fn unregister(&self, id: &str) {
        let mut map = self.inner.lock().expect("client registry mutex poisoned");
        map.remove(id);
    }

    pub fn snapshot(&self) -> Vec<ClientInfo> {
        let map = self.inner.lock().expect("client registry mutex poisoned");
        map.values()
            .map(|e| {
                let mut info = e.info.clone();
                info.duration_seconds = e.last_active.elapsed().as_secs();
                info
            })
            .collect()
    }
}
