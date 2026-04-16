use std::collections::{HashMap, HashSet};
use std::sync::{Arc, Mutex};

use tokio::sync::mpsc;
use tracing::{debug, warn};

use crate::event::Event;

#[derive(Clone)]
pub enum Sender {
    Bounded(mpsc::Sender<Event>),
    Unbounded(mpsc::UnboundedSender<Event>),
}

impl Sender {
    pub fn try_send(&self, event: Event) -> bool {
        match self {
            Sender::Bounded(tx) => match tx.try_send(event) {
                Ok(_)                                     => true,
                Err(mpsc::error::TrySendError::Full(_))   => false,
                Err(mpsc::error::TrySendError::Closed(_)) => false,
            },
            Sender::Unbounded(tx) => tx.send(event).is_ok(),
        }
    }
}

// Кольцевой буфер уже обработанных ID — без роста памяти
#[derive(Clone)]
struct SeenIds {
    ids:      Arc<Mutex<(HashSet<String>, std::collections::VecDeque<String>)>>,
    capacity: usize,
}

impl SeenIds {
    fn new(capacity: usize) -> Self {
        Self {
            ids: Arc::new(Mutex::new((HashSet::new(), std::collections::VecDeque::new()))),
            capacity,
        }
    }

    // true = уже видели (дубликат)
    fn check_and_insert(&self, id: &str) -> bool {
        let mut guard = self.ids.lock().unwrap();
        let (set, queue) = &mut *guard;
        if set.contains(id) {
            return true;
        }
        if queue.len() >= self.capacity {
            if let Some(old) = queue.pop_front() {
                set.remove(&old);
            }
        }
        set.insert(id.to_string());
        queue.push_back(id.to_string());
        false
    }
}

pub struct Router {
    table:    HashMap<String, Vec<(String, Sender)>>,
    dlq_tx:   mpsc::UnboundedSender<Event>,
    seen_ids: SeenIds,
}

impl Router {
    pub fn new(dlq_tx: mpsc::UnboundedSender<Event>) -> Self {
        Self {
            table:    HashMap::new(),
            dlq_tx,
            seen_ids: SeenIds::new(10_000),
        }
    }

    pub fn register(&mut self, id: &str, topics: &[String], tx: Sender) {
        for topic in topics {
            self.table
                .entry(topic.clone())
                .or_default()
                .push((id.to_string(), tx.clone()));
        }
    }

    pub fn deregister(&mut self, id: &str) {
        for receivers in self.table.values_mut() {
            receivers.retain(|(rid, _)| rid != id);
        }
        self.table.retain(|_, v| !v.is_empty());
    }

    pub fn route(&self, event: &Event) {
        if self.seen_ids.check_and_insert(&event.meta.id) {
            debug!(id = %event.meta.id, topic = %event.meta.topic, "duplicate event, skipping");
            return;
        }

        match self.table.get(&event.meta.topic) {
            None => {
                debug!(topic = %event.meta.topic, "no subscribers");
            }
            Some(receivers) => {
                for (id, tx) in receivers {
                    debug!(topic = %event.meta.topic, plugin = %id, "routing");
                    if !tx.try_send(event.clone()) {
                        warn!(topic = %event.meta.topic, plugin = %id, "send failed → DLQ");
                        let _ = self.dlq_tx.send(event.clone());
                    }
                }
            }
        }
    }

    pub fn clone_table(&self) -> RouterTable {
        RouterTable {
            table:    self.table.clone(),
            dlq_tx:   self.dlq_tx.clone(),
            seen_ids: self.seen_ids.clone(),
        }
    }
}

pub struct RouterTable {
    table:    HashMap<String, Vec<(String, Sender)>>,
    dlq_tx:   mpsc::UnboundedSender<Event>,
    seen_ids: SeenIds,
}

impl RouterTable {
    pub fn route(&self, event: &Event) {
        if self.seen_ids.check_and_insert(&event.meta.id) {
            debug!(id = %event.meta.id, topic = %event.meta.topic, "duplicate, skipping");
            return;
        }

        match self.table.get(&event.meta.topic) {
            None => {
                debug!(topic = %event.meta.topic, "no subscribers");
            }
            Some(receivers) => {
                for (id, tx) in receivers {
                    debug!(topic = %event.meta.topic, plugin = %id, "routing");
                    if !tx.try_send(event.clone()) {
                        warn!(topic = %event.meta.topic, plugin = %id, "send failed → DLQ");
                        let _ = self.dlq_tx.send(event.clone());
                    }
                }
            }
        }
    }
}
