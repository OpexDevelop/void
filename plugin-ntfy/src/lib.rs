use serde::Deserialize;

extern "C" {
    fn emit_event(
        topic_ptr: *const u8, topic_len: i32,
        payload_ptr: *const u8, payload_len: i32,
    );
    fn host_http_post(
        url_ptr: *const u8, url_len: i32,
        body_ptr: *const u8, body_len: i32,
    ) -> i32;
    fn host_sse_start(url_ptr: *const u8, url_len: i32) -> i32;
}

const NTFY_SEND_URL: &str = "https://ntfy.sh/wasm-messenger";
const NTFY_RECV_URL: &str = "https://ntfy.sh/wasm-messenger/sse";

#[derive(Deserialize)]
struct EventMeta {
    topic: String,
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
        "SYS_STARTUP"      => start_sse_listener(),
        "CRYPTO_ENCRYPTED" => send_over_network(payload_slice),
        "NET_RECEIVED"     => forward_to_bus(payload_slice),
        _                  => 0,
    }
}

fn start_sse_listener() -> i32 {
    unsafe {
        host_sse_start(NTFY_RECV_URL.as_ptr(), NTFY_RECV_URL.len() as i32)
    }
}

fn send_over_network(payload: &[u8]) -> i32 {
    unsafe {
        host_http_post(
            NTFY_SEND_URL.as_ptr(), NTFY_SEND_URL.len() as i32,
            payload.as_ptr(),       payload.len()       as i32,
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
