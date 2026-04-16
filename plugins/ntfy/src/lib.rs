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

// ── глобальное состояние (wasm однопоточен — static mut безопасен) ─────────
static mut NTFY_BASE: [u8; 128] = [0u8; 128];
static mut NTFY_BASE_LEN: usize  = 0;

const DEFAULT_BASE: &str = "https://ntfy.sh/wasm-messenger";

fn get_base() -> &'static str {
    unsafe {
        let len = NTFY_BASE_LEN;
        if len == 0 {
            DEFAULT_BASE
        } else {
            std::str::from_utf8(&NTFY_BASE[..len]).unwrap_or(DEFAULT_BASE)
        }
    }
}

fn set_base(chat_id: &str) {
    let url = format!("https://ntfy.sh/{}", chat_id);
    let bytes = url.as_bytes();
    let len   = bytes.len().min(127);
    unsafe {
        NTFY_BASE[..len].copy_from_slice(&bytes[..len]);
        NTFY_BASE_LEN = len;
    }
}

// ── ABI ────────────────────────────────────────────────────────────────────

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

/// Payload SYS_STARTUP: `{"chat_id":"<id>"}` (опционально).
/// Если поле отсутствует — используется DEFAULT_BASE.
#[derive(Deserialize, Default)]
struct StartupPayload {
    #[serde(default)]
    chat_id: Option<String>,
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
        "CRYPTO_ENCRYPTED" => send_over_network(payload_slice),
        "NET_RECEIVED"     => forward_to_bus(payload_slice),
        _                  => 0,
    }
}

fn on_startup(payload: &[u8]) -> i32 {
    // Читаем chat_id из SYS_STARTUP payload (если есть)
    if !payload.is_empty() {
        if let Ok(p) = serde_json::from_slice::<StartupPayload>(payload) {
            if let Some(id) = p.chat_id {
                if !id.is_empty() {
                    set_base(&id);
                }
            }
        }
    }
    // Запускаем SSE-слушателя на /<base>/sse
    let sse_url = format!("{}/sse", get_base());
    unsafe {
        host_sse_start(sse_url.as_ptr(), sse_url.len() as i32)
    }
}

fn send_over_network(payload: &[u8]) -> i32 {
    let send_url = get_base().to_string();
    unsafe {
        host_http_post(
            send_url.as_ptr(), send_url.len() as i32,
            payload.as_ptr(),  payload.len()  as i32,
        )
    }
}

fn forward_to_bus(payload: &[u8]) -> i32 {
    emit("NET_FORWARDED", payload);
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
