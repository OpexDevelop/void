use std::sync::{Mutex, OnceLock};

struct CoreState {
    listen_port: u16,
    my_addr: String,
}

static STATE: OnceLock<Mutex<CoreState>> = OnceLock::new();

fn state() -> &'static Mutex<CoreState> {
    STATE.get_or_init(|| {
        Mutex::new(CoreState {
            listen_port: 7777,
            my_addr: "127.0.0.1:7777".to_string(),
        })
    })
}

pub fn init(port: u16) {
    let mut s = state().lock().unwrap();
    s.listen_port = port;
    s.my_addr = format!("127.0.0.1:{}", port);
}

pub fn get_my_addr() -> String {
    state().lock().unwrap().my_addr.clone()
}
