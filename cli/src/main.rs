use std::path::PathBuf;
use tokio::io::{self, AsyncBufReadExt, BufReader};
use void_core::{Engine, Event};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    println!("🚀 Запуск voidchat...");

    let engine = Engine::new();
    let tx = engine.tx.clone();
    
    let plugins_dir = PathBuf::from("./cli/plugins");
    engine.load_plugins(&plugins_dir).await?;
    
    engine.run().await;

    let tx_tick = tx.clone();
    tokio::spawn(async move {
        loop {
            tokio::time::sleep(tokio::time::Duration::from_secs(3)).await;
            let _ = tx_tick.send(Event {
                topic: "SYS_TICK".to_string(),
                data: "".to_string(),
                ts: 0,
            }).await;
        }
    });
    
    println!("✅ voidchat готов. Введите сообщение:");

    let stdin = io::stdin();
    let mut reader = BufReader::new(stdin).lines();

    while let Ok(Some(line)) = reader.next_line().await {
        let text = line.trim();
        if text.is_empty() { continue; }
        if text == "/quit" { break; }
        
        let _ = tx.send(Event {
            topic: "UI_SEND_MSG".to_string(),
            data: text.to_string(),
            ts: 0,
        }).await;
    }

    Ok(())
}
