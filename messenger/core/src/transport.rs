use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::thread;
use crate::bus::{push_event, Event};
use crate::plugin_manager::{call_plugin_fn, find_plugin_by_category};

pub fn start_tcp_server(port: u16) {
    thread::spawn(move || {
        let listener = match TcpListener::bind(format!("0.0.0.0:{}", port)) {
            Ok(l) => l,
            Err(e) => {
                eprintln!("TCP bind error on port {}: {}", port, e);
                return;
            }
        };
        for stream in listener.incoming().flatten() {
            thread::spawn(|| handle_connection(stream));
        }
    });
}

fn handle_connection(mut stream: TcpStream) {
    let from = stream
        .peer_addr()
        .map(|a| a.to_string())
        .unwrap_or_else(|_| "unknown".to_string());

    let mut len_buf = [0u8; 4];
    if stream.read_exact(&mut len_buf).is_err() {
        return;
    }
    let len = u32::from_be_bytes(len_buf) as usize;

    if len > 10 * 1024 * 1024 {
        return;
    }

    let mut data = vec![0u8; len];
    if stream.read_exact(&mut data).is_err() {
        return;
    }

    let text = crate::decrypt_bytes(&data);
    let ts = crate::current_timestamp();

    if let Some(storage_id) = find_plugin_by_category("storage") {
        let input = serde_json::json!({
            "from": from,
            "to": "me",
            "text": text,
            "timestamp": ts,
        })
        .to_string();
        let _ = call_plugin_fn(&storage_id, "store_message", &input);
    }

    push_event(Event {
        kind: "message_received".to_string(),
        payload: serde_json::json!({
            "from": from,
            "text": text,
            "timestamp": ts,
        }),
    });
}

pub fn send_tcp_message(to_addr: &str, payload: &[u8]) -> Result<(), String> {
    let mut stream = TcpStream::connect(to_addr).map_err(|e| e.to_string())?;
    let len = payload.len() as u32;
    stream.write_all(&len.to_be_bytes()).map_err(|e| e.to_string())?;
    stream.write_all(payload).map_err(|e| e.to_string())?;
    stream.flush().map_err(|e| e.to_string())?;
    Ok(())
}
