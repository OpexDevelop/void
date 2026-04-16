use std::io::{self, BufRead};
use std::sync::Arc;

use anyhow::Result;
use tokio::sync::mpsc;
use tracing::{error, info};
use tracing_subscriber::EnvFilter;

use core::bus::{Event, EventMeta, DB_READ_CMD, SYS_STARTUP, UI_SEND_MSG};
use core::manifest::PluginManifest;
use core::supervisor::Supervisor;

#[cfg(feature = "wasmtime-backend")]
use core::engine::wasmtime_engine::WasmtimeRuntime;

#[cfg(feature = "wasmi-backend")]
use core::engine::wasmi_engine::WasmiRuntime;

/// Парсит аргументы командной строки.
/// Использование:
///   void [--chat <chat_id>] [--manifests <dir>]
///
/// Примеры:
///   void --chat my-secret-room
///   void --chat team-alpha --manifests ./conf/manifests
struct CliArgs {
    /// Идентификатор ntfy-топика (канала чата).
    /// Плагин ntfy подставит его в URL: https://ntfy.sh/<chat_id>
    chat_id:       String,
    /// Директория с манифестами плагинов (по умолчанию "manifests")
    manifests_dir: String,
}

impl CliArgs {
    fn parse() -> Self {
        let args: Vec<String> = std::env::args().skip(1).collect();
        let mut chat_id       = String::from("wasm-messenger");
        let mut manifests_dir = String::from("manifests");

        let mut i = 0;
        while i < args.len() {
            match args[i].as_str() {
                "--chat" => {
                    i += 1;
                    if let Some(v) = args.get(i) {
                        chat_id = v.clone();
                    } else {
                        eprintln!("ERROR: --chat requires a value");
                        std::process::exit(1);
                    }
                }
                "--manifests" => {
                    i += 1;
                    if let Some(v) = args.get(i) {
                        manifests_dir = v.clone();
                    } else {
                        eprintln!("ERROR: --manifests requires a value");
                        std::process::exit(1);
                    }
                }
                "--help" | "-h" => {
                    println!("Usage: void [--chat <id>] [--manifests <dir>]");
                    println!();
                    println!("  --chat <id>        ntfy topic / chat room ID (default: wasm-messenger)");
                    println!("  --manifests <dir>  path to plugin manifests   (default: manifests)");
                    println!();
                    println!("Commands inside the REPL:");
                    println!("  /history   — show stored message history");
                    println!("  /quit      — exit");
                    println!("  <text>     — send a message");
                    std::process::exit(0);
                }
                other => {
                    eprintln!("ERROR: unknown argument `{other}`");
                    eprintln!("Run `void --help` for usage.");
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
                .unwrap_or_else(|_| EnvFilter::new("cli=debug,core=debug,warn")),
        )
        .init();

    let args = CliArgs::parse();

    info!(chat_id = %args.chat_id, "Wasm Plugin Host starting");

    // ── runtime ───────────────────────────────────────────────────────────
    #[cfg(feature = "wasmtime-backend")]
    let runtime: Arc<dyn core::engine::PluginRuntime> = Arc::new(WasmtimeRuntime::new()?);

    #[cfg(all(feature = "wasmi-backend", not(feature = "wasmtime-backend")))]
    let runtime: Arc<dyn core::engine::PluginRuntime> = Arc::new(WasmiRuntime::new()?);

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
                            error!(manifest = %path, error = %e, "Failed to load plugin");
                        }
                    }
                    Err(e) => error!(wasm = %wasm_path, error = %e, "Failed to read wasm"),
                }
            }
            Err(e) => error!(manifest = %path, error = %e, "Failed to parse manifest"),
        }
    }

    supervisor.start_routing(global_rx);
    let _watcher = supervisor.start_hot_swap_watcher().ok();

    // ── SYS_STARTUP несёт chat_id в payload ───────────────────────────────
    // plugin-ntfy читает его и строит URL динамически
    let startup_payload = serde_json::to_vec(&serde_json::json!({
        "chat_id": args.chat_id
    }))?;

    let _ = global_tx.send(Event {
        meta:    EventMeta::new(SYS_STARTUP),
        payload: startup_payload,
    });

    // ── REPL ──────────────────────────────────────────────────────────────
    info!(
        "Ready. Chat: {}  |  Commands: /history | /quit | <message>",
        args.chat_id
    );

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

    info!("Shutting down");
    Ok(())
}
