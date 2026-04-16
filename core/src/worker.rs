use std::sync::Arc;
use std::time::Duration;

use tokio::sync::mpsc;
use tracing::{debug, error, info, warn};

use crate::engine::{HostContext, PluginInstance, PluginRuntime, Permissions};
use crate::event::{Event, SYS_SHUTDOWN};
use crate::manifest::{PluginManifest, RestartPolicy};

fn backoff(attempt: u32) -> Duration {
    let ms = 100u64
        .saturating_mul(1u64 << attempt.min(10))
        .min(30_000);
    Duration::from_millis(ms)
}

pub fn spawn(
    runtime:   Arc<dyn PluginRuntime>,
    bytes:     Vec<u8>,
    manifest:  PluginManifest,
    rx:        mpsc::Receiver<Event>,
    global_tx: mpsc::UnboundedSender<Event>,
    dlq_tx:    mpsc::UnboundedSender<Event>,
) {
    tokio::spawn(async move {
        run(runtime, bytes, manifest, rx, global_tx, dlq_tx).await;
    });
}

async fn run(
    runtime:   Arc<dyn PluginRuntime>,
    bytes:     Vec<u8>,
    manifest:  PluginManifest,
    mut rx:    mpsc::Receiver<Event>,
    global_tx: mpsc::UnboundedSender<Event>,
    dlq_tx:    mpsc::UnboundedSender<Event>,
) {
    let id = manifest.plugin.id.clone();

    let make_instance = || {
        let rt  = Arc::clone(&runtime);
        let b   = bytes.clone();
        let ctx = build_ctx(&manifest, global_tx.clone());
        tokio::task::block_in_place(|| rt.instantiate(&b, ctx))
    };

    let mut instance: Box<dyn PluginInstance> = match make_instance() {
        Ok(i)  => i,
        Err(e) => {
            error!(plugin = %id, error = %e, "instantiation failed");
            return;
        }
    };

    let mut retries = 0u32;

    while let Some(event) = rx.recv().await {
        if event.meta.topic == SYS_SHUTDOWN {
            info!(plugin = %id, "SYS_SHUTDOWN, stopping");
            break;
        }

        let meta_json = match serde_json::to_vec(&event.meta) {
            Ok(v)  => v,
            Err(e) => {
                error!(plugin = %id, error = %e, "meta serialize failed");
                let _ = dlq_tx.send(event);
                continue;
            }
        };

        let fuel_before = instance.fuel_consumed();

        let result = tokio::task::block_in_place(|| {
            instance.handle_event(&meta_json, &event.payload)
        });

        match result {
            Ok(_) => {
                let fuel = instance.fuel_consumed().saturating_sub(fuel_before);
                debug!(plugin = %id, topic = %event.meta.topic, fuel, "handled");
                retries = 0;
            }
            Err(e) => {
                error!(plugin = %id, topic = %event.meta.topic, error = %e, "handler error");
                let _ = dlq_tx.send(event);

                let should_restart = matches!(
                    manifest.supervisor.restart_policy,
                    RestartPolicy::Always | RestartPolicy::OnFailure
                );

                if should_restart && retries < manifest.supervisor.max_retries {
                    retries += 1;
                    let delay = backoff(retries);
                    warn!(plugin = %id, attempt = retries, delay_ms = delay.as_millis(), "restarting");
                    tokio::time::sleep(delay).await;
                    match make_instance() {
                        Ok(new) => { instance = new; }
                        Err(e2) => {
                            error!(plugin = %id, error = %e2, "restart failed");
                            return;
                        }
                    }
                } else {
                    error!(plugin = %id, "max retries exceeded");
                    return;
                }
            }
        }
    }

    info!(plugin = %id, "worker stopped");
}

fn build_ctx(manifest: &PluginManifest, event_tx: mpsc::UnboundedSender<Event>) -> HostContext {
    use std::path::PathBuf;

    HostContext {
        event_tx,
        permissions: Permissions {
            network:      manifest.permissions.network,
            filesystem:   manifest.permissions.filesystem,
            allowed_dirs: manifest.permissions.allowed_dirs
                .iter()
                .map(PathBuf::from)
                .collect(),
        },
    }
}
