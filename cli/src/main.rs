use std::io::{self, BufRead};
use std::sync::Arc;

use anyhow::Result;
use tokio::sync::mpsc;
use tracing::{error, info};
use tracing_subscriber::EnvFilter;

use void_core::bus::{Event, EventMeta, DB_READ_CMD, SYS_STARTUP, UI_SEND_MSG};
use void_core::manifest::PluginManifest;
use void_core::supervisor::Supervisor;

#[cfg(feature = "wasmtime-backend")]
use void_core::engine::wasmtime_engine::WasmtimeRuntime;

#[cfg(all(feature = "wasmi-backend", not(feature = "wasmtime-backend")))]
use void_core::engine::wasmi_engine::WasmiRuntime;

struct CliArgs {
    /// ntfy topic — передаётся в plugin-ntfy через SYS_STARTUP payload
    chat_id:       String,
    /// директория с манифестами (по умолчанию "manifests")
    manifests_dir: String,
}

impl CliArgs {
    fn parse() -> Self {
        let args: Vec<String> = std::env::args().skip(1).collect();
        let mut chat_id       = String::from("wasm-messenger");
        let mut manifests_dir = String::from("manifests");
        let mut i = 0usize;

        while i < args.len() {
            match args[i].as_str() {
                "--chat" => {
                    i += 1;
                    chat_id = args.get(i)
                        .cloned()
                        .unwrap_or_else(|| { eprintln!("--chat requires a value"); std::process::exit(1); });
                }
                "--manifests" => {
                    i += 1;
                    manifests_dir = args.get(i)
                        .cloned()
                        .unwrap_or_else(|| { eprintln!("--manifests requires a value"); std::process::exit(1); });
                }
                "--help" | "-h" => {
                    println!("void [--chat <id>] [--manifests <dir>]");
                    println!();
                    println!("  --chat <id>        ntfy topic / chat room  (default: wasm-messenger)");
                    println!("  --manifests <dir>  manifest directory       (default: manifests)");
                    println!();
                    println!("REPL commands:");
                    println!("  /history   show stored history");
                    println!("  /quit      exit");
                    println!("  <text>     send message");
                    std::process::exit(0);
                }
                other => {
                    eprintln!("unknown argument `{other}`, try --help");
                    std::process::exit(1);
                }
            }
            i += 1;
        }

        Self { chat_id, manifests_dir }
    }
}

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| EnvFilter::new("void=debug,core=debug,warn")),
        )
        .init();

    let args = CliArgs::parse();

    info!(chat = %args.chat_id, "void starting");

    // ── runtime ───────────────────────────────────────────────────────────
    #[cfg(feature = "wasmtime-backend")]
    let runtime: Arc<dyn void_core::engine::PluginRuntime> = Arc::new(WasmtimeRuntime::new()?);

    #[cfg(all(feature = "wasmi-backend", not(feature = "wasmtime-backend")))]
    let runtime: Arc<dyn void_core::engine::PluginRuntime> = Arc::new(WasmiRuntime::new()?);

    // ── каналы ────────────────────────────────────────────────────────────
    let (global_tx, global_rx) = mpsc::unbounded_channel::<Event>();
    let (dlq_tx, mut dlq_rx)   = mpsc::unbounded_channel::<Event>();

    tokio::spawn(async move {
        while let Some(event) = dlq_rx.recv().await {
            tracing::warn!(
                topic = %event.meta.topic,
                id    = %event.meta.id,
                "[DLQ] undelivered event"
            );
        }
    });

    // ── загрузка плагинов ─────────────────────────────────────────────────
    let mut supervisor = Supervisor::new(Arc::clone(&runtime), global_tx.clone(), dlq_tx);

    let manifest_files = [
        format!("{}/crypto.toml",  args.manifests_dir),
        format!("{}/ntfy.toml",    args.manifests_dir),
        format!("{}/storage.toml", args.manifests_dir),
    ];

    for path in &manifest_files {
        match PluginManifest::from_file(path) {
            Ok(manifest) => {
                let wasm_path = manifest.plugin.wasm_path.clone();
                match std::fs::read(&wasm_path) {
                    Ok(bytes) => {
                        if let Err(e) = supervisor.load_plugin(manifest, bytes).await {
                            error!(manifest = %path, error = %e, "failed to load plugin");
                        }
                    }
                    Err(e) => error!(wasm = %wasm_path, error = %e, "failed to read wasm"),
                }
            }
            Err(e) => error!(manifest = %path, error = %e, "failed to parse manifest"),
        }
    }

    supervisor.start_routing(global_rx);
    let _watcher = supervisor.start_hot_swap_watcher().ok();

    // ── SYS_STARTUP: кладём chat_id в payload → plugin-ntfy прочитает ─────
    let startup_payload = serde_json::to_vec(&serde_json::json!({
        "chat_id": args.chat_id
    }))?;

    let _ = global_tx.send(Event {
        meta:    EventMeta::new(SYS_STARTUP),
        payload: startup_payload,
    });

    info!("ready  |  chat: {}  |  /history  /quit  <message>", args.chat_id);

    // ── REPL ──────────────────────────────────────────────────────────────
    let stdin      = io::stdin();
    let global_tx2 = global_tx.clone();

    for line in stdin.lock().lines() {
        let line    = line?;
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

    info!("shutting down");
    Ok(())
}
