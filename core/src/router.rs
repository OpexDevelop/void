// добавить в конец router.rs

pub struct RouterTable {
    table:  HashMap<String, Vec<(String, Sender)>>,
}

impl RouterTable {
    pub fn route(&self, event: Event, dlq_tx: &mpsc::UnboundedSender<Event>) {
        match self.table.get(&event.meta.topic) {
            None => {
                debug!(topic = %event.meta.topic, "no subscribers");
            }
            Some(receivers) => {
                for (id, tx) in receivers {
                    debug!(topic = %event.meta.topic, plugin = %id, "routing");
                    if !tx.try_send(event.clone()) {
                        warn!(topic = %event.meta.topic, plugin = %id, "send failed → DLQ");
                        let _ = dlq_tx.send(event.clone());
                    }
                }
            }
        }
    }
}

impl Router {
    pub fn clone_table(&self) -> RouterTable {
        RouterTable { table: self.table.clone() }
    }
}
