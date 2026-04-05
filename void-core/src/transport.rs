use std::collections::VecDeque;
use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::sync::{Arc, Mutex};
use std::thread;

pub trait Transport: Send + Sync {
    fn send(&self, addr: &str, data: &[u8]) -> Result<(), String>;
    fn recv(&self) -> Option<Vec<u8>>;
}

pub struct TcpTransport {
    inbox: Arc<Mutex<VecDeque<Vec<u8>>>>,
}

impl TcpTransport {
    pub fn new(port: u16) -> Self {
        let inbox: Arc<Mutex<VecDeque<Vec<u8>>>> = Arc::new(Mutex::new(VecDeque::new()));
        let rx = inbox.clone();

        thread::spawn(move || {
            let listener = match TcpListener::bind(format!("0.0.0.0:{port}")) {
                Ok(l) => l,
                Err(e) => { eprintln!("bind error: {e}"); return; }
            };
            for stream in listener.incoming().flatten() {
                if let Some(data) = Self::read_frame(stream) {
                    rx.lock().unwrap().push_back(data);
                }
            }
        });

        Self { inbox }
    }

    fn read_frame(mut s: TcpStream) -> Option<Vec<u8>> {
        let mut len_buf = [0u8; 4];
        s.read_exact(&mut len_buf).ok()?;
        let len = u32::from_be_bytes(len_buf) as usize;
        if len > 10_000_000 { return None; }
        let mut buf = vec![0u8; len];
        s.read_exact(&mut buf).ok()?;
        Some(buf)
    }
}

impl Transport for TcpTransport {
    fn send(&self, addr: &str, data: &[u8]) -> Result<(), String> {
        let mut stream = TcpStream::connect(addr).map_err(|e| e.to_string())?;
        stream.write_all(&(data.len() as u32).to_be_bytes()).map_err(|e| e.to_string())?;
        stream.write_all(data).map_err(|e| e.to_string())
    }

    fn recv(&self) -> Option<Vec<u8>> {
        self.inbox.lock().unwrap().pop_front()
    }
}
