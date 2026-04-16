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
struct EventMeta {
    topic: String,
}

#[derive(Deserialize, Default)]
struct StartupPayload {
    #[serde(default)]
    chat_id: Option<String>,
}

#[derive(Deserialize)]
struct NtfyEvent {
    #[serde(default)]
    message: String,
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

fn send_encrypted(payload: &[u8]) -> i32 {
    let url = send_url();
    unsafe {
        host_http_post(
            url.as_ptr(),     url.len()     as i32,
            payload.as_ptr(), payload.len() as i32,
        )
    }
}

fn on_net_received(payload: &[u8]) -> i32 {
    let message_bytes = if let Ok(ev) = serde_json::from_slice::<NtfyEvent>(payload) {
        if !ev.message.is_empty() {
            ev.message.into_bytes()
        } else {
            payload.to_vec()
        }
    } else {
        payload.to_vec()
    };

    emit("NET_RECEIVED_MSG", &message_bytes);
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
