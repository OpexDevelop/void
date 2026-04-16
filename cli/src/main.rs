use std::io::{self, BufRead};
use std::sync::Arc;

use anyhow::Result;
use tokio::sync::mpsc;
use tracing::info;
use tracing_subscriber::EnvFilter;

use void_core::event::{Event, EventMeta};
use void_core::hotswap::HotSwapConfig;
use void_core::manifest::PluginManifest;
use void_core::supervisor::Supervisor;

#[cfg(feature = "wasmtime-backend")]
use void_core::engine::wasmtime_engine::WasmtimeRuntime;

#[cfg(all(feature = "wasmi-backend", not(feature = "wasmtime-backend")))]
use void_core::engine::wasmi_engine::WasmiRuntime;

const TOPIC_SYS_STARTUP:      &str = "SYS_STARTUP";
const TOPIC_UI_SEND_MSG:      &str = "UI_SEND_MSG";
const TOPIC_DB_READ_CMD:      &str = "DB_READ_CMD";
const TOPIC_CRYPTO_DECRYPTED: &str = "CRYPTO_DECRYPTED";
const TOPIC_DB_HISTORY_RESULT:&str = "DB_HISTORY_RESULT";

struct CliArgs {
    chat_id:       String,
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
                    chat_id = args.get(i).cloned().unwrap_or_else(|| {
                        eprintln!("--chat requires a value");
                        std::process::exit(1);
                    });
                }
                "--manifests" => {
                    i += 1;
                    manifests_dir = args.get(i).cloned().unwrap_or_else(|| {
                        eprintln!("--manifests requires a value");
                        std::process::exit(1);
                    });
                }
                "--help" | "-h" => {
                    println!("void [--chat <id>] [--manifests <dir>]");
                    std::process::exit(0);
                }
                other => {
                    eprintln!("unknown argument `{other}`");
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
                .unwrap_or_else(|_| EnvFilter::new("void=debug,void_core=debug,warn")),
        )
        .init();

    let args = CliArgs::parse();
    info!(chat = %args.chat_id, "void starting");

    #[cfg(feature = "wasmtime-backend")]
    let runtime: Arc<dyn void_core::engine::PluginRuntime> =
        Arc::new(WasmtimeRuntime::new()?);

    #[cfg(all(feature = "wasmi-backend", not(feature = "wasmtime-backend")))]
    let runtime: Arc<dyn void_core::engine::PluginRuntime> =
        Arc::new(WasmiRuntime::new()?);

    let (global_tx, global_rx) = mpsc::unbounded_channel::<Event>();
    let (dlq_tx, mut dlq_rx)   = mpsc::unbounded_channel::<Event>();
    let (host_tx, mut host_rx) = mpsc::unbounded_channel::<Event>();

    tokio::spawn(async move {
        while let Some(ev) = dlq_rx.recv().await {
            tracing::warn!(topic = %ev.meta.topic, id = %ev.meta.id, "[DLQ]");
        }
    });

    tokio::spawn(async move {
        while let Some(ev) = host_rx.recv().await {
            match ev.meta.topic.as_str() {
                t if t == TOPIC_CRYPTO_DECRYPTED => {
                    match String::from_utf8(ev.payload.clone()) {
                        Ok(msg) => println!("\n[incoming] {}", msg),
                        Err(_)  => println!("\n[incoming] <binary {} bytes>", ev.payload.len()),
                    }
                }
                t if t == TOPIC_DB_HISTORY_RESULT => {
                    print_history(&ev.payload);
                }
                _ => {}
            }
        }
    });

    let mut supervisor = Supervisor::new(
        Arc::clone(&runtime),
        global_tx.clone(),
        dlq_tx,
    );

    supervisor.subscribe_host(
        &[TOPIC_CRYPTO_DECRYPTED.to_string(), TOPIC_DB_HISTORY_RESULT.to_string()],
        host_tx,
    );

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
                            tracing::error!(manifest = %path, error = %e, "load failed");
                        }
                    }
                    Err(e) => tracing::error!(wasm = %wasm_path, error = %e, "read failed"),
                }
            }
            Err(e) => tracing::error!(manifest = %path, error = %e, "parse failed"),
        }
    }

    supervisor.start_routing(global_rx);
    let _watcher = supervisor.start_hot_swap(HotSwapConfig::default()).ok();

    let startup_payload = serde_json::to_vec(&serde_json::json!({
        "chat_id": args.chat_id
    }))?;

    let _ = global_tx.send(Event {
        meta:    EventMeta::new(TOPIC_SYS_STARTUP),
        payload: startup_payload,
    });

    info!("ready  |  chat: {}  |  /history  /quit  <message>", args.chat_id);

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
                    meta:    EventMeta::new(TOPIC_DB_READ_CMD),
                    payload: vec![],
                });
            }
            msg => {
                let _ = global_tx2.send(Event {
                    meta:    EventMeta::new(TOPIC_UI_SEND_MSG),
                    payload: msg.as_bytes().to_vec(),
                });
            }
        }
    }

    info!("shutting down");
    Ok(())
}

fn print_history(payload: &[u8]) {
    match serde_json::from_slice::<serde_json::Value>(payload) {
        Ok(arr) => {
            println!("\n── history ──");
            if let Some(messages) = arr.as_array() {
                if messages.is_empty() {
                    println!("  (empty)");
                }
                for msg in messages {
                    let ts      = msg["ts"].as_u64().unwrap_or(0);
                    let payload = msg["payload"].as_array();
                    if let Some(bytes_arr) = payload {
                        let bytes: Vec<u8> = bytes_arr
                            .iter()
                            .filter_map(|v| v.as_u64().map(|n| n as u8))
                            .collect();
                        match String::from_utf8(bytes) {
                            Ok(text) => println!("  [{ts}] {text}"),
                            Err(_)   => println!("  [{ts}] <binary>"),
                        }
                    }
                }
            }
            println!("─────────────");
        }
        Err(_) => println!("[history] parse failed"),
    }
}
