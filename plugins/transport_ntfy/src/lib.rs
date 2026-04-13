use base64::Engine;
use extism_pdk::*;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

const DEFAULT_NTFY_URL: &str = "https://ntfy.sh";
const CONFIG_KEY: &str = "ntfy_config";

// ──────────────────────────────────────────────────────────────
// Конфиг плагина
// ──────────────────────────────────────────────────────────────

#[derive(Serialize, Deserialize, Clone)]
struct NtfyConfig {
    base_url: String,
    my_topic: String,
}

impl Default for NtfyConfig {
    fn default() -> Self {
        Self {
            base_url: DEFAULT_NTFY_URL.to_string(),
            my_topic: String::new(),
        }
    }
}

fn load_config() -> NtfyConfig {
    var::get(CONFIG_KEY)
        .ok()
        .flatten()
        .and_then(|b| serde_json::from_slice(&b).ok())
        .unwrap_or_default()
}

fn save_config(cfg: &NtfyConfig) -> FnResult<()> {
    var::set(CONFIG_KEY, serde_json::to_vec(cfg)?)?;
    Ok(())
}

// ──────────────────────────────────────────────────────────────
// Wire-формат: что летит внутри ntfy сообщения
// ──────────────────────────────────────────────────────────────

#[derive(Serialize, Deserialize)]
struct WireMessage {
    /// topic отправителя — чтобы получатель знал кому отвечать
    from_topic: String,
    /// зашифрованный payload в base64 (шифрование — дело crypto плагина)
    payload_b64: String,
}

// ──────────────────────────────────────────────────────────────
// Входные/выходные типы
// ──────────────────────────────────────────────────────────────

/// configure: ядро передаёт { "address": "alice_42" }
/// для ntfy — address это наш topic
#[derive(Deserialize)]
struct ConfigureInput {
    address: String,
}

/// send: ядро передаёт { "to_topic": "...", "payload_b64": "..." }
#[derive(Deserialize)]
struct SendInput {
    to_topic: String,
    payload_b64: String,
}

/// get_pending: ядро передаёт { "since": 1234567890, "limit": 50 }
#[derive(Deserialize)]
struct GetPendingInput {
    since: Option<u64>,
    limit: Option<u32>,
}

#[derive(Serialize)]
struct SendOutput {
    ok: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    error: Option<String>,
}

#[derive(Serialize)]
struct IncomingMessage {
    from_topic: String,
    payload_b64: String,
    timestamp: u64,
}

#[derive(Serialize)]
struct GetPendingOutput {
    messages: Vec<IncomingMessage>,
}

// ──────────────────────────────────────────────────────────────
// Функции плагина
// ──────────────────────────────────────────────────────────────

/// Настройка: сохранить наш адрес (topic) в конфиг плагина
#[plugin_fn]
pub fn configure(input: String) -> FnResult<String> {
    let req: ConfigureInput = serde_json::from_str(&input)?;

    if req.address.is_empty() {
        return Ok(serde_json::to_string(&SendOutput {
            ok: false,
            error: Some("address cannot be empty".to_string()),
        })?);
    }

    let mut cfg = load_config();
    // Для ntfy плагина: address = topic
    cfg.my_topic = req.address;
    save_config(&cfg)?;

    Ok(serde_json::to_string(&SendOutput {
        ok: true,
        error: None,
    })?)
}

/// Отправить сообщение: POST на ntfy topic получателя
///
/// ntfy API: POST https://ntfy.sh/{topic}
/// Body: JSON { from_topic, payload_b64 }
/// Headers:
///   Content-Type: application/json
///   X-Title: msg
#[plugin_fn]
pub fn send(input: String) -> FnResult<String> {
    let req: SendInput = serde_json::from_str(&input)?;
    let cfg = load_config();

    if cfg.my_topic.is_empty() {
        return Ok(serde_json::to_string(&SendOutput {
            ok: false,
            error: Some("transport not configured: call configure first".to_string()),
        })?);
    }

    if req.to_topic.is_empty() {
        return Ok(serde_json::to_string(&SendOutput {
            ok: false,
            error: Some("to_topic cannot be empty".to_string()),
        })?);
    }

    let wire = WireMessage {
        from_topic: cfg.my_topic.clone(),
        payload_b64: req.payload_b64,
    };

    let body = serde_json::to_string(&wire)?;
    let url = format!(
        "{}/{}",
        cfg.base_url.trim_end_matches('/'),
        req.to_topic,
    );

    let mut headers = BTreeMap::new();
    headers.insert("Content-Type".to_string(), "application/json".to_string());
    // X-Title нужен чтобы ntfy не отклонил запрос как пустой
    headers.insert("X-Title".to_string(), "msg".to_string());

    let http_req = HttpRequest {
        url,
        headers,
        method: Some("POST".to_string()),
    };

    match http::request::<String>(&http_req, Some(body)) {
        Ok(resp) => {
            let status = resp.status_code();
            if status == 200 || status == 201 || status == 202 {
                Ok(serde_json::to_string(&SendOutput {
                    ok: true,
                    error: None,
                })?)
            } else {
                Ok(serde_json::to_string(&SendOutput {
                    ok: false,
                    error: Some(format!("ntfy returned status {}", status)),
                })?)
            }
        }
        Err(e) => Ok(serde_json::to_string(&SendOutput {
            ok: false,
            error: Some(e.to_string()),
        })?),
    }
}

/// Забрать входящие сообщения со своего topic
///
/// ntfy API: GET https://ntfy.sh/{topic}/json?poll=1&since={ts}&limit={n}
/// Возвращает NDJSON: одна строка = одно ntfy событие
#[plugin_fn]
pub fn get_pending(input: String) -> FnResult<String> {
    let req: GetPendingInput = serde_json::from_str(&input)
        .unwrap_or(GetPendingInput {
            since: None,
            limit: None,
        });

    let cfg = load_config();

    if cfg.my_topic.is_empty() {
        return Ok(serde_json::to_string(&GetPendingOutput {
            messages: vec![],
        })?);
    }

    let limit = req.limit.unwrap_or(50);

    // since: unix timestamp или "1h" если не задан
    let since_param = match req.since {
        Some(ts) if ts > 0 => ts.to_string(),
        _ => "1h".to_string(),
    };

    let url = format!(
        "{}/{}/json?poll=1&since={}&limit={}",
        cfg.base_url.trim_end_matches('/'),
        cfg.my_topic,
        since_param,
        limit,
    );

    let mut headers = BTreeMap::new();
    headers.insert("Accept".to_string(), "application/x-ndjson".to_string());

    let http_req = HttpRequest {
        url,
        headers,
        method: Some("GET".to_string()),
    };

    let resp = match http::request::<String>(&http_req, None::<String>) {
        Ok(r) => r,
        Err(_) => {
            // Сеть недоступна — возвращаем пустой список без ошибки
            return Ok(serde_json::to_string(&GetPendingOutput {
                messages: vec![],
            })?);
        }
    };

    if resp.status_code() != 200 {
        return Ok(serde_json::to_string(&GetPendingOutput {
            messages: vec![],
        })?);
    }

    let body = resp.body();
    let body_str = match std::str::from_utf8(&body) {
        Ok(s) => s,
        Err(_) => {
            return Ok(serde_json::to_string(&GetPendingOutput {
                messages: vec![],
            })?)
        }
    };

    let mut messages = Vec::new();

    // ntfy возвращает NDJSON: каждая строка — отдельный JSON объект
    for line in body_str.lines() {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }

        let event: serde_json::Value = match serde_json::from_str(line) {
            Ok(v) => v,
            Err(_) => continue,
        };

        // Нас интересуют только события типа "message"
        // ntfy также шлёт "open", "keepalive" — игнорируем
        if event["event"].as_str() != Some("message") {
            continue;
        }

        let ntfy_timestamp = event["time"].as_u64().unwrap_or(0);

        let ntfy_message = match event["message"].as_str() {
            Some(m) => m,
            None => continue,
        };

        // Парсим наш WireMessage из тела ntfy сообщения
        let wire: WireMessage = match serde_json::from_str(ntfy_message) {
            Ok(w) => w,
            Err(_) => {
                // Кто-то отправил на наш topic не наш формат — пропускаем
                continue;
            }
        };

        // Не принимаем сообщения от самих себя
        if wire.from_topic == cfg.my_topic {
            continue;
        }

        messages.push(IncomingMessage {
            from_topic: wire.from_topic,
            payload_b64: wire.payload_b64,
            timestamp: ntfy_timestamp,
        });
    }

    Ok(serde_json::to_string(&GetPendingOutput { messages })?)
}
