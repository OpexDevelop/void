use std::time::Duration;

use futures_util::StreamExt;

use crate::event::{BusTx, Event, EventMeta};

pub async fn http_post(url: String, body: Vec<u8>) {
    let client = reqwest::Client::new();
    match client
        .post(&url)
        .header("Content-Type", "text/plain; charset=utf-8")
        .body(body)
        .send()
        .await
    {
        Ok(r)  => tracing::info!(url = %url, status = %r.status(), "http_post ok"),
        Err(e) => tracing::error!(url = %url, error = %e, "http_post failed"),
    }
}

pub async fn sse_loop(url: String, tx: BusTx) {
    tracing::info!(url = %url, "SSE stream starting");
    loop {
        let client = match reqwest::Client::builder()
            .timeout(Duration::from_secs(300))
            .build()
        {
            Ok(c)  => c,
            Err(e) => { tracing::error!(error = %e, "reqwest build failed"); return; }
        };

        let response = match client
            .get(&url)
            .header("Accept", "text/event-stream")
            .send()
            .await
        {
            Ok(r)  => r,
            Err(e) => {
                tracing::warn!(error = %e, "SSE connect failed, retry in 5s");
                tokio::time::sleep(Duration::from_secs(5)).await;
                continue;
            }
        };

        let mut stream = response.bytes_stream();
        let mut buf    = String::new();

        while let Some(chunk) = stream.next().await {
            match chunk {
                Ok(bytes) => {
                    let text = match std::str::from_utf8(&bytes) {
                        Ok(t)  => t,
                        Err(_) => continue,
                    };
                    buf.push_str(text);

                    while let Some(pos) = buf.find("\n\n") {
                        let block = buf[..pos].to_string();
                        buf = buf[pos + 2..].to_string();

                        for line in block.lines() {
                            if let Some(data) = line.strip_prefix("data: ") {
                                if data.trim().is_empty() || data.trim() == "{}" {
                                    continue;
                                }
                                let ev = Event {
                                    meta:    EventMeta::new("NET_RECEIVED"),
                                    payload: data.as_bytes().to_vec(),
                                };
                                if tx.send(ev).await.is_err() {
                                    tracing::warn!("SSE: bus closed");
                                    return;
                                }
                            }
                        }
                    }
                }
                Err(e) => {
                    tracing::warn!(error = %e, "SSE chunk error, reconnecting");
                    break;
                }
            }
        }

        tracing::info!("SSE ended, reconnecting in 3s");
        tokio::time::sleep(Duration::from_secs(3)).await;
    }
}
