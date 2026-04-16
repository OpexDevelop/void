use std::path::PathBuf;
use std::sync::Arc;

use tokio::sync::mpsc;
use tokio::io::AsyncWriteExt;
use tracing::{error, warn};

use crate::event::Event;

pub struct DlqConfig {
    pub path: PathBuf,
}

impl Default for DlqConfig {
    fn default() -> Self {
        Self { path: PathBuf::from("./data/dlq.jsonl") }
    }
}

#[derive(serde::Serialize)]
struct DlqRecord<'a> {
    id:      &'a str,
    topic:   &'a str,
    payload: &'a [u8],
    ts:      u64,
}

pub fn spawn(config: DlqConfig) -> mpsc::UnboundedSender<Event> {
    let (tx, mut rx) = mpsc::unbounded_channel::<Event>();

    tokio::spawn(async move {
        if let Some(parent) = config.path.parent() {
            let _ = tokio::fs::create_dir_all(parent).await;
        }

        let file = tokio::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&config.path)
            .await;

        let mut file = match file {
            Ok(f)  => f,
            Err(e) => {
                error!(error = %e, path = ?config.path, "DLQ file open failed");
                // fallback: только логируем
                while let Some(ev) = rx.recv().await {
                    warn!(topic = %ev.meta.topic, id = %ev.meta.id, "[DLQ] lost event");
                }
                return;
            }
        };

        while let Some(ev) = rx.recv().await {
            warn!(topic = %ev.meta.topic, id = %ev.meta.id, "[DLQ] persisting");

            let record = DlqRecord {
                id:      &ev.meta.id,
                topic:   &ev.meta.topic,
                payload: &ev.payload,
                ts:      ev.meta.timestamp,
            };

            match serde_json::to_vec(&record) {
                Ok(mut line) => {
                    line.push(b'\n');
                    if let Err(e) = file.write_all(&line).await {
                        error!(error = %e, "[DLQ] write failed");
                    }
                }
                Err(e) => error!(error = %e, "[DLQ] serialize failed"),
            }
        }
    });

    tx
}
