use serde::Deserialize;

extern "C" {
    fn emit_event(
        topic_ptr:   *const u8, topic_len:   i32,
        payload_ptr: *const u8, payload_len: i32,
    );
    fn host_http_post(
        url_ptr:  *const u8, url_len:  i32,
        body_ptr: *const u8, body_len: i32,
    ) -> i32;
    fn host_sse_start(url_ptr: *const u8, url_len: i32) -> i32;
}

static mut NTFY_SEND: [u8; 256] = [0u8; 256];
static mut NTFY_SEND_LEN: usize = 0;
static mut NTFY_SSE: [u8; 260] = [0u8; 260];
static mut NTFY_SSE_LEN: usize = 0;

const DEFAULT_CHAT: &str = "wasm-messenger";

fn init_urls(chat_id: &str) {
    let send = format!("https://ntfy.sh/{}", chat_id);
    let sse  = format!("https://ntfy.sh/{}/sse", chat_id);
    unsafe {
        let sb = send.as_bytes();
        let sl = sb.len().min(255);
        NTFY_SEND[..sl].copy_from_slice(&sb[..sl]);
        NTFY_SEND_LEN = sl;
        let rb = sse.as_bytes();
        let rl = rb.len().min(259);
        NTFY_SSE[..rl].copy_from_slice(&rb[..rl]);
        NTFY_SSE_LEN = rl;
    }
}

fn send_url() -> &'static str {
    unsafe {
        if NTFY_SEND_LEN == 0 { return "https://ntfy.sh/wasm-messenger"; }
        core::str::from_utf8(&NTFY_SEND[..NTFY_SEND_LEN]).unwrap_or(DEFAULT_CHAT)
    }
}

fn sse_url() -> &'static str {
    unsafe {
        if NTFY_SSE_LEN == 0 { return "https://ntfy.sh/wasm-messenger/sse"; }
        core::str::from_utf8(&NTFY_SSE[..NTFY_SSE_LEN]).unwrap_or(DEFAULT_CHAT)
    }
}

#[no_mangle]
pub extern "C" fn alloc(size: i32) -> *mut u8 {
    let mut buf: Vec<u8> = Vec::with_capacity(size as usize);
    let ptr = buf.as_mut_ptr();
    std::mem::forget(buf);
    ptr
}

#[no_mangle]
pub extern "C" fn dealloc(ptr: *mut u8, size: i32) {
    unsafe { drop(Vec::from_raw_parts(ptr, 0, size as usize)) };
}

#[derive(Deserialize)]
struct EventMeta { topic: String }

#[derive(Deserialize, Default)]
struct StartupPayload {
    #[serde(default)]
    chat_id: Option<String>,
}

#[derive(Deserialize)]
struct NtfyEvent {
    #[serde(default)]
    message: String,
    #[serde(default)]
    event: String,
}

#[no_mangle]
pub extern "C" fn handle_event(
    meta_ptr:    *const u8, meta_len:    i32,
    payload_ptr: *const u8, payload_len: i32,
) -> i32 {
    let meta_slice    = unsafe { std::slice::from_raw_parts(meta_ptr,    meta_len    as usize) };
    let payload_slice = unsafe { std::slice::from_raw_parts(payload_ptr, payload_len as usize) };

    let meta: EventMeta = match serde_json::from_slice(meta_slice) {
        Ok(m)  => m,
        Err(_) => return 1,
    };

    match meta.topic.as_str() {
        "SYS_STARTUP"      => on_startup(payload_slice),
        "CRYPTO_ENCRYPTED" => send_encrypted(payload_slice),
        "NET_RECEIVED"     => on_net_received(payload_slice),
        _                  => 0,
    }
}

fn on_startup(payload: &[u8]) -> i32 {
    let chat_id = if !payload.is_empty() {
        serde_json::from_slice::<StartupPayload>(payload)
            .ok()
            .and_then(|p| p.chat_id)
            .filter(|s| !s.is_empty())
            .unwrap_or_else(|| DEFAULT_CHAT.to_string())
    } else {
        DEFAULT_CHAT.to_string()
    };
    init_urls(&chat_id);
    let url = sse_url();
    unsafe { host_sse_start(url.as_ptr(), url.len() as i32) }
}

fn send_encrypted(ciphertext: &[u8]) -> i32 {
    let encoded = base64_encode(ciphertext);
    let url = send_url();
    unsafe {
        host_http_post(
            url.as_ptr(),          url.len()          as i32,
            encoded.as_ptr(),      encoded.len()      as i32,
        )
    }
}

fn on_net_received(payload: &[u8]) -> i32 {
    let ntfy_ev = match serde_json::from_slice::<NtfyEvent>(payload) {
        Ok(e) => e,
        Err(_) => return 0,
    };

    if ntfy_ev.event == "keepalive" || ntfy_ev.event == "open" {
        return 0;
    }

    if ntfy_ev.message.is_empty() {
        return 0;
    }

    let decoded = match base64_decode(ntfy_ev.message.trim()) {
        Some(v) => v,
        None    => return 0,
    };

    emit("NET_RECEIVED_MSG", &decoded);
    0
}

fn emit(topic: &str, payload: &[u8]) {
    unsafe {
        emit_event(
            topic.as_ptr(),   topic.len()   as i32,
            payload.as_ptr(), payload.len() as i32,
        );
    }
}

const BASE64_CHARS: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";

fn base64_encode(input: &[u8]) -> alloc::vec::Vec<u8> {
    let mut out = alloc::vec::Vec::new();
    let mut i = 0usize;
    while i < input.len() {
        let b0 = input[i] as u32;
        let b1 = if i + 1 < input.len() { input[i + 1] as u32 } else { 0 };
        let b2 = if i + 2 < input.len() { input[i + 2] as u32 } else { 0 };
        let n = (b0 << 16) | (b1 << 8) | b2;
        out.push(BASE64_CHARS[((n >> 18) & 63) as usize]);
        out.push(BASE64_CHARS[((n >> 12) & 63) as usize]);
        out.push(if i + 1 < input.len() { BASE64_CHARS[((n >> 6) & 63) as usize] } else { b'=' });
        out.push(if i + 2 < input.len() { BASE64_CHARS[(n & 63) as usize] } else { b'=' });
        i += 3;
    }
    out
}

fn base64_decode(input: &str) -> Option<alloc::vec::Vec<u8>> {
    let mut table = [0xffu8; 256];
    for (i, &c) in BASE64_CHARS.iter().enumerate() {
        table[c as usize] = i as u8;
    }
    let input = input.trim_end_matches('=');
    let bytes = input.as_bytes();
    let mut out = alloc::vec::Vec::new();
    let mut i = 0usize;
    while i < bytes.len() {
        let c0 = table.get(bytes[i] as usize).copied().unwrap_or(0xff);
        let c1 = table.get(bytes.get(i + 1).copied().unwrap_or(0) as usize).copied().unwrap_or(0xff);
        if c0 == 0xff || c1 == 0xff { return None; }
        out.push((c0 << 2) | (c1 >> 4));
        if i + 2 < bytes.len() {
            let c2 = table.get(bytes[i + 2] as usize).copied().unwrap_or(0xff);
            if c2 == 0xff { return None; }
            out.push((c1 << 4) | (c2 >> 2));
            if i + 3 < bytes.len() {
                let c3 = table.get(bytes[i + 3] as usize).copied().unwrap_or(0xff);
                if c3 == 0xff { return None; }
                out.push((c2 << 6) | c3);
            }
        }
        i += 4;
    }
    Some(out)
}

extern crate alloc;
