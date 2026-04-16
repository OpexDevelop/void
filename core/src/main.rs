mod bus;
mod engine;
mod manifest;
mod signing;
mod supervisor;

use std::io::{self, BufRead};
use std::sync::Arc;

use anyhow::Result;
use tokio::sync::mpsc;
use tracing::{error, info};
use tracing_subscriber::EnvFilter;

use bus::{Event, EventMeta, SYS_DLQ, SYS_STARTUP, UI_SEND_MSG, DB_READ_CMD};
use manifest::PluginManifest;
use supervisor::Supervisor;

#[cfg(feature = "wasmtime-backend")]
use engine::wasmtime_engine::WasmtimeRuntime;

#[cfg(feature = "wasmi-backend")]
use engine::wasmi_engine::WasmiRuntime;

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| EnvFilter::new("core=debug,warn")),
        )
        .init();

    info!("Wasm Plugin Host starting");

    #[cfg(feature = "wasmtime-backend")]
    let runtime: Arc<dyn engine::PluginRuntime> = Arc::new(WasmtimeRuntime::new()?);

    #[cfg(all(feature = "wasmi-backend", not(feature = "wasmtime-backend")))]
    let runtime: Arc<dyn engine::PluginRuntime> = Arc::new(WasmiRuntime::new()?);

    let (global_tx, global_rx) = mpsc::unbounded_channel::<Event>();
    let (dlq_tx,    mut dlq_rx) = mpsc::unbounded_channel::<Event>();

    tokio::spawn(async move {
        while let Some(event) = dlq_rx.recv().await {
            tracing::warn!(
                topic = %event.meta.topic,
                id    = %event.meta.id,
                "[DLQ] undelivered event"
            );
        }
    });

    let mut supervisor = Supervisor::new(Arc::clone(&runtime), global_tx.clone(), dlq_tx);

    let manifest_paths = [
        "manifests/crypto.toml",
        "manifests/ntfy.toml",
        "manifests/storage.toml",
    ];

    for path in &manifest_paths {
        match PluginManifest::from_file(path) {
            Ok(manifest) => {
                let wasm_path = manifest.plugin.wasm_path.clone();
                match std::fs::read(&wasm_path) {
                    Ok(bytes) => {
                        if let Err(e) = supervisor.load_plugin(manifest, bytes).await {
                            error!(manifest = path, error = %e, "Failed to load plugin");
                        }
                    }
                    Err(e) => {
                        error!(wasm = %wasm_path, error = %e, "Failed to read wasm file");
                    }
                }
            }
            Err(e) => {
                error!(manifest = path, error = %e, "Failed to parse manifest");
            }
        }
    }

    supervisor.start_routing(global_rx);

    let _watcher = supervisor.start_hot_swap_watcher().ok();

    let _ = global_tx.send(Event {
        meta:    EventMeta::new(SYS_STARTUP),
        payload: vec![],
    });

    info!("Ready. Commands: /history | /quit | <message>");

    let stdin       = io::stdin();
    let global_tx2  = global_tx.clone();

    for line in stdin.lock().lines() {
        let line = line?;
        let trimmed = line.trim();
        if trimmed.is_empty() { continue; }

        match trimmed {
            "/quit" => break,
            "/history" => {
                let _ = global_tx2.send(Event {
                    meta:    EventMeta::new(DB_READ_CMD),
                    payload: vec![],
                });
            }
            msg => {
                let _ = global_tx2.send(Event {
                    meta:    EventMeta::new(UI_SEND_MSG),
                    payload: msg.as_bytes().to_vec(),
                });
            }
        }
    }

    info!("Shutting down");
    Ok(())
}
