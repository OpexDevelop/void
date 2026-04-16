wit_bindgen::generate!({
    path:  "../../wit/plugin.wit",
    world: "network-plugin-world",
});

use core::cell::UnsafeCell;
use serde::Deserialize;

struct Plugin;

impl Guest for Plugin {
    fn handle_event(meta: EventMeta, payload: Vec<u8>) -> i32 {
        match meta.topic.as_str() {
            "SYS_STARTUP"      => on_startup(&payload),
            "CRYPTO_ENCRYPTED" => send_encrypted(&payload),
            "NET_RECEIVED"     => on_net_received(&payload),
            _                  => 0,
        }
    }
}

export!(Plugin);

// ── URL storage ───────────────────────────────────────────────────────────────

const MAX_URL: usize = 256;

struct UrlBuf {
    data: [u8; MAX_URL],
    len:  usize,
}

impl UrlBuf {
    const fn empty() -> Self { Self { data: [0u8; MAX_URL], len: 0 } }

    fn set(&mut self, s: &str) {
        let b = s.as_bytes();
        let l = b.len().min(MAX_URL - 1);
        self.data[..l].copy_from_slice(&b[..l]);
        self.len = l;
    }

    fn as_str(&self) -> &str {
        core::str::from_utf8(&self.data[..self.len]).unwrap_or("")
    }
}

struct WasmStatic<T>(UnsafeCell<T>);
unsafe impl<T> Sync for WasmStatic<T> {}

static SEND_URL: WasmStatic<UrlBuf> = WasmStatic(UnsafeCell::new(UrlBuf::empty()));
static SSE_URL:  WasmStatic<UrlBuf> = WasmStatic(UnsafeCell::new(UrlBuf::empty()));

fn get_send_url() -> &'static str { unsafe { (*SEND_URL.0.get()).as_str() } }
fn get_sse_url()  -> &'static str { unsafe { (*SSE_URL.0.get()).as_str()  } }

fn init_urls(chat_id: &str) {
    unsafe {
        (*SEND_URL.0.get()).set(&alloc::format!("https://ntfy.sh/{}", chat_id));
        (*SSE_URL.0.get()).set(&alloc::format!("https://ntfy.sh/{}/sse", chat_id));
    }
}

// ── handlers ──────────────────────────────────────────────────────────────────

const DEFAULT_CHAT: &str = "wasm-messenger";

#[derive(Deserialize, Default)]
struct StartupPayload {
    #[serde(default)]
    chat_id: Option<alloc::string::String>,
}

#[derive(Deserialize)]
struct NtfyEvent {
    #[serde(default)]
    message: alloc::string::String,
    #[serde(default)]
    event:   alloc::string::String,
}

fn on_startup(payload: &[u8]) -> i32 {
    let chat_id = if !payload.is_empty() {
        serde_json::from_slice::<StartupPayload>(payload)
            .ok()
            .and_then(|p| p.chat_id)
            .filter(|s| !s.is_empty())
            .unwrap_or_else(|| DEFAULT_CHAT.into())
    } else {
        DEFAULT_CHAT.into()
    };

    init_urls(&chat_id);
    host_sse_start(get_sse_url())
}

fn send_encrypted(ciphertext: &[u8]) -> i32 {
    let encoded = base64_encode(ciphertext);
    host_http_post(get_send_url(), &encoded)
}

fn on_net_received(payload: &[u8]) -> i32 {
    let ev = match serde_json::from_slice::<NtfyEvent>(payload) {
        Ok(e)  => e,
        Err(_) => return 0,
    };
    if ev.event == "keepalive" || ev.event == "open" || ev.message.is_empty() {
        return 0;
    }
    match base64_decode(ev.message.trim()) {
        Some(decoded) => { emit_event("NET_RECEIVED_MSG", &decoded); 0 }
        None          => 0,
    }
}

// ── base64 ────────────────────────────────────────────────────────────────────

const B64: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";

fn base64_encode(input: &[u8]) -> alloc::vec::Vec<u8> {
    let mut out = alloc::vec::Vec::new();
    let mut i = 0usize;
    while i < input.len() {
        let b0 = input[i] as u32;
        let b1 = if i + 1 < input.len() { input[i + 1] as u32 } else { 0 };
        let b2 = if i + 2 < input.len() { input[i + 2] as u32 } else { 0 };
        let n  = (b0 << 16) | (b1 << 8) | b2;
        out.push(B64[((n >> 18) & 63) as usize]);
        out.push(B64[((n >> 12) & 63) as usize]);
        out.push(if i + 1 < input.len() { B64[((n >> 6) & 63) as usize] } else { b'=' });
        out.push(if i + 2 < input.len() { B64[(n & 63) as usize]        } else { b'=' });
        i += 3;
    }
    out
}

fn base64_decode(input: &str) -> Option<alloc::vec::Vec<u8>> {
    let mut table = [0xffu8; 256];
    for (i, &c) in B64.iter().enumerate() { table[c as usize] = i as u8; }
    let input = input.trim_end_matches('=');
    let bytes = input.as_bytes();
    let mut out = alloc::vec::Vec::new();
    let mut i = 0usize;
    while i < bytes.len() {
        let c0 = table[bytes[i] as usize];
        let c1 = table[bytes.get(i + 1).copied().unwrap_or(0) as usize];
        if c0 == 0xff || c1 == 0xff { return None; }
        out.push((c0 << 2) | (c1 >> 4));
        if i + 2 < bytes.len() {
            let c2 = table[bytes[i + 2] as usize];
            if c2 == 0xff { return None; }
            out.push((c1 << 4) | (c2 >> 2));
            if i + 3 < bytes.len() {
                let c3 = table[bytes[i + 3] as usize];
                if c3 == 0xff { return None; }
                out.push((c2 << 6) | c3);
            }
        }
        i += 4;
    }
    Some(out)
}

extern crate alloc;
