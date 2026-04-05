Полный код void 0.0.1
это тестовая монолитная самая первая версия
просто для проверки смогу ли я чтото собрать вообще на раст + флаттер




# Void Messenger 


---

## Шаг 1: Установка всего окружения

```bash
# ─── Rust ───
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y
source ~/.cargo/env
rustup target add aarch64-linux-android
cargo install cargo-ndk

# ─── Java 21 через SDKMAN ───
curl -s "https://get.sdkman.io" | bash
source "$HOME/.sdkman/bin/sdkman-init.sh"
sdk install java 21.0.3-tem
sdk use java 21.0.3-tem

# ─── Android SDK + NDK ───
sudo apt-get update && sudo apt-get install -y unzip wget
mkdir -p ~/android/cmdline-tools && cd ~/android/cmdline-tools
wget -q https://dl.google.com/android/repository/commandlinetools-linux-11076708_latest.zip -O tools.zip
unzip -q tools.zip && mv cmdline-tools latest && rm tools.zip

export ANDROID_HOME=~/android
export ANDROID_SDK_ROOT=~/android
export PATH=$PATH:$ANDROID_HOME/cmdline-tools/latest/bin:$ANDROID_HOME/platform-tools

yes | sdkmanager --licenses > /dev/null 2>&1
sdkmanager "platform-tools" "platforms;android-36" "build-tools;36.0.0" "ndk;27.0.12077973"
export ANDROID_NDK_HOME=$ANDROID_HOME/ndk/27.0.12077973

# ─── Flutter ───
cd ~
git clone https://github.com/flutter/flutter.git -b stable --depth 1
export PATH=$PATH:~/flutter/bin
flutter config --android-sdk ~/android
yes | flutter doctor --android-licenses > /dev/null 2>&1

# ─── Сохранить переменные навсегда ───
cat >> ~/.bashrc << 'ENVEOF'
export ANDROID_HOME=~/android
export ANDROID_SDK_ROOT=~/android
export ANDROID_NDK_HOME=~/android/ndk/27.0.12077973
export PATH=$PATH:$ANDROID_HOME/cmdline-tools/latest/bin:$ANDROID_HOME/platform-tools:~/flutter/bin
source "$HOME/.sdkman/bin/sdkman-init.sh"
ENVEOF

# ─── Проверка ───
flutter doctor
```

Должно быть:
```
[✓] Flutter
[✓] Android toolchain
```
Chrome и Linux toolchain — игнорируй, для APK не нужны.

---

## Шаг 2: Rust ядро

```bash
cd ~ && cargo new --lib void-core && cd void-core
```

### Cargo.toml

```bash
cat > Cargo.toml << 'EOF'
[package]
name = "void-core"
version = "0.1.0"
edition = "2026"

[lib]
crate-type = ["cdylib"]

[dependencies]
serde = { version = "1", features = ["derive"] }
serde_json = "1"
aes-gcm = "0.10"
rand = "0.8"
uuid = { version = "1", features = ["v4"] }
EOF
```

### src/types.rs

```bash
cat > src/types.rs << 'RUSTEOF'
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct Message {
    pub id: String,
    pub chat_id: String,
    pub text: String,
    pub timestamp: u64,
    pub incoming: bool,
}
RUSTEOF
```

### src/storage.rs

```bash
cat > src/storage.rs << 'RUSTEOF'
use std::collections::HashMap;
use std::sync::Mutex;
use crate::types::Message;

pub trait Storage: Send + Sync {
    fn save(&self, msg: &Message);
    fn history(&self, chat_id: &str) -> Vec<Message>;
    fn chats(&self) -> Vec<String>;
}

pub struct MemStorage {
    data: Mutex<HashMap<String, Vec<Message>>>,
}

impl MemStorage {
    pub fn new() -> Self {
        Self { data: Mutex::new(HashMap::new()) }
    }
}

impl Storage for MemStorage {
    fn save(&self, msg: &Message) {
        self.data.lock().unwrap()
            .entry(msg.chat_id.clone())
            .or_default()
            .push(msg.clone());
    }

    fn history(&self, chat_id: &str) -> Vec<Message> {
        self.data.lock().unwrap()
            .get(chat_id).cloned().unwrap_or_default()
    }

    fn chats(&self) -> Vec<String> {
        self.data.lock().unwrap().keys().cloned().collect()
    }
}
RUSTEOF
```

### src/crypto.rs

```bash
cat > src/crypto.rs << 'RUSTEOF'
use aes_gcm::aead::{Aead, KeyInit};
use aes_gcm::{Aes256Gcm, Nonce};

pub trait Crypto: Send + Sync {
    fn encrypt(&self, data: &[u8]) -> Vec<u8>;
    fn decrypt(&self, data: &[u8]) -> Option<Vec<u8>>;
}

pub struct AesCrypto {
    cipher: Aes256Gcm,
}

impl AesCrypto {
    pub fn new(key: &[u8; 32]) -> Self {
        Self { cipher: Aes256Gcm::new(key.into()) }
    }
}

impl Crypto for AesCrypto {
    fn encrypt(&self, data: &[u8]) -> Vec<u8> {
        let nonce_bytes: [u8; 12] = rand::random();
        let nonce = Nonce::from_slice(&nonce_bytes);
        let ct = self.cipher.encrypt(nonce, data).unwrap();
        let mut out = nonce_bytes.to_vec();
        out.extend(ct);
        out
    }

    fn decrypt(&self, data: &[u8]) -> Option<Vec<u8>> {
        if data.len() < 13 { return None; }
        let nonce = Nonce::from_slice(&data[..12]);
        self.cipher.decrypt(nonce, &data[12..]).ok()
    }
}
RUSTEOF
```

### src/transport.rs

```bash
cat > src/transport.rs << 'RUSTEOF'
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
RUSTEOF
```

### src/engine.rs

```bash
cat > src/engine.rs << 'RUSTEOF'
use std::time::{SystemTime, UNIX_EPOCH};
use crate::types::Message;
use crate::storage::Storage;
use crate::crypto::Crypto;
use crate::transport::Transport;

pub struct VoidEngine {
    storage: Box<dyn Storage>,
    crypto: Box<dyn Crypto>,
    transports: Vec<Box<dyn Transport>>,
}

impl VoidEngine {
    pub fn new(
        storage: Box<dyn Storage>,
        crypto: Box<dyn Crypto>,
        transports: Vec<Box<dyn Transport>>,
    ) -> Self {
        Self { storage, crypto, transports }
    }

    pub fn send_msg(&self, chat_id: &str, text: &str, peer_addr: &str) {
        let msg = Message {
            id: uuid::Uuid::new_v4().to_string(),
            chat_id: chat_id.into(),
            text: text.into(),
            timestamp: SystemTime::now()
                .duration_since(UNIX_EPOCH).unwrap().as_secs(),
            incoming: false,
        };
        self.storage.save(&msg);
        let encrypted = self.crypto.encrypt(&serde_json::to_vec(&msg).unwrap());
        for t in &self.transports {
            let _ = t.send(peer_addr, &encrypted);
        }
    }

    pub fn poll(&self) -> Option<Message> {
        for t in &self.transports {
            if let Some(data) = t.recv() {
                let plain = self.crypto.decrypt(&data)?;
                let mut msg: Message = serde_json::from_slice(&plain).ok()?;
                msg.incoming = true;
                self.storage.save(&msg);
                return Some(msg);
            }
        }
        None
    }

    pub fn history(&self, chat_id: &str) -> Vec<Message> {
        self.storage.history(chat_id)
    }
}
RUSTEOF
```

### src/lib.rs

```bash
cat > src/lib.rs << 'RUSTEOF'
mod types;
mod storage;
mod crypto;
mod transport;
mod engine;

use std::ffi::{CStr, CString};
use std::os::raw::c_char;
use std::sync::OnceLock;

static CORE: OnceLock<engine::VoidEngine> = OnceLock::new();

#[no_mangle]
pub extern "C" fn void_init(port: i32, key: *const u8) -> i32 {
    let key: [u8; 32] = unsafe { std::slice::from_raw_parts(key, 32) }
        .try_into().unwrap();

    let core = engine::VoidEngine::new(
        Box::new(storage::MemStorage::new()),
        Box::new(crypto::AesCrypto::new(&key)),
        vec![Box::new(transport::TcpTransport::new(port as u16))],
    );
    CORE.set(core).is_ok() as i32
}

#[no_mangle]
pub extern "C" fn void_send(
    chat_id: *const c_char,
    text: *const c_char,
    addr: *const c_char,
) -> i32 {
    let core = match CORE.get() { Some(c) => c, None => return -1 };
    let chat_id = unsafe { CStr::from_ptr(chat_id) }.to_str().unwrap_or("");
    let text = unsafe { CStr::from_ptr(text) }.to_str().unwrap_or("");
    let addr = unsafe { CStr::from_ptr(addr) }.to_str().unwrap_or("");
    core.send_msg(chat_id, text, addr);
    0
}

#[no_mangle]
pub extern "C" fn void_poll() -> *mut c_char {
    let core = match CORE.get() { Some(c) => c, None => return std::ptr::null_mut() };
    match core.poll() {
        Some(msg) => CString::new(serde_json::to_string(&msg).unwrap_or_default())
            .unwrap_or_default().into_raw(),
        None => std::ptr::null_mut(),
    }
}

#[no_mangle]
pub extern "C" fn void_history(chat_id: *const c_char) -> *mut c_char {
    let core = match CORE.get() { Some(c) => c, None => return std::ptr::null_mut() };
    let chat_id = unsafe { CStr::from_ptr(chat_id) }.to_str().unwrap_or("");
    let msgs = core.history(chat_id);
    CString::new(serde_json::to_string(&msgs).unwrap_or_default())
        .unwrap_or_default().into_raw()
}

#[no_mangle]
pub extern "C" fn void_free(ptr: *mut c_char) {
    if !ptr.is_null() {
        unsafe { drop(CString::from_raw(ptr)) };
    }
}
RUSTEOF
```

### Проверка + сборка под Android

```bash
# Проверка что код верный
cargo build --release
echo "=== Rust OK ==="

# Сборка под Android
cargo ndk -t arm64-v8a build --release
echo "=== Android .so OK ==="
```

---

## Шаг 3: Flutter проект

```bash
cd ~
flutter create void_app
cd void_app
```

### pubspec.yaml

```bash
cat > pubspec.yaml << 'EOF'
name: void_app
description: Void Messenger
publish_to: 'none'
version: 1.0.0

environment:
  sdk: ^3.0.0

dependencies:
  flutter:
    sdk: flutter
  ffi: ^2.1.0

flutter:
  uses-material-design: true
EOF

flutter pub get
```

### Скопировать .so

```bash
mkdir -p android/app/src/main/jniLibs/arm64-v8a
cp ~/void-core/target/aarch64-linux-android/release/libvoid_core.so \
   android/app/src/main/jniLibs/arm64-v8a/
ls -lh android/app/src/main/jniLibs/arm64-v8a/libvoid_core.so
```

### Разрешение на интернет

```bash
sed -i '/<application/i\    <uses-permission android:name="android.permission.INTERNET"/>' \
  android/app/src/main/AndroidManifest.xml
```

### lib/core_bridge.dart

```bash
cat > lib/core_bridge.dart << 'DARTEOF'
import 'dart:convert';
import 'dart:ffi';
import 'package:ffi/ffi.dart';

typedef _InitC = Int32 Function(Int32 port, Pointer<Uint8> key);
typedef _InitDart = int Function(int port, Pointer<Uint8> key);

typedef _SendC = Int32 Function(Pointer<Utf8> chatId, Pointer<Utf8> text, Pointer<Utf8> addr);
typedef _SendDart = int Function(Pointer<Utf8> chatId, Pointer<Utf8> text, Pointer<Utf8> addr);

typedef _PollC = Pointer<Utf8> Function();
typedef _PollDart = Pointer<Utf8> Function();

typedef _HistoryC = Pointer<Utf8> Function(Pointer<Utf8> chatId);
typedef _HistoryDart = Pointer<Utf8> Function(Pointer<Utf8> chatId);

typedef _FreeC = Void Function(Pointer<Utf8> ptr);
typedef _FreeDart = void Function(Pointer<Utf8> ptr);

class VoidCore {
  late final _SendDart _send;
  late final _PollDart _poll;
  late final _HistoryDart _history;
  late final _FreeDart _free;

  static VoidCore? _instance;
  VoidCore._();

  static VoidCore init({required int port, required List<int> key}) {
    if (_instance != null) return _instance!;

    final lib = DynamicLibrary.open('libvoid_core.so');
    final core = VoidCore._();

    final initFn = lib.lookupFunction<_InitC, _InitDart>('void_init');
    core._send = lib.lookupFunction<_SendC, _SendDart>('void_send');
    core._poll = lib.lookupFunction<_PollC, _PollDart>('void_poll');
    core._history = lib.lookupFunction<_HistoryC, _HistoryDart>('void_history');
    core._free = lib.lookupFunction<_FreeC, _FreeDart>('void_free');

    final keyPtr = calloc<Uint8>(32);
    for (var i = 0; i < 32; i++) {
      keyPtr[i] = i < key.length ? key[i] : 0;
    }
    initFn(port, keyPtr);
    calloc.free(keyPtr);

    _instance = core;
    return core;
  }

  void sendMessage(String chatId, String text, String peerAddr) {
    final c1 = chatId.toNativeUtf8();
    final c2 = text.toNativeUtf8();
    final c3 = peerAddr.toNativeUtf8();
    _send(c1, c2, c3);
    calloc.free(c1);
    calloc.free(c2);
    calloc.free(c3);
  }

  Map<String, dynamic>? poll() {
    final ptr = _poll();
    if (ptr == nullptr) return null;
    final json = ptr.toDartString();
    _free(ptr);
    return Map<String, dynamic>.from(jsonDecode(json) as Map);
  }
}
DARTEOF
```

### lib/chat_screen.dart

```bash
cat > lib/chat_screen.dart << 'DARTEOF'
import 'dart:async';
import 'package:flutter/material.dart';
import 'core_bridge.dart';

class ChatScreen extends StatefulWidget {
  const ChatScreen({super.key});

  @override
  State<ChatScreen> createState() => _ChatScreenState();
}

class _ChatScreenState extends State<ChatScreen> {
  VoidCore? _core;
  Timer? _timer;
  bool _connected = false;

  final _msgCtrl = TextEditingController();
  final _portCtrl = TextEditingController(text: '9001');
  final _peerCtrl = TextEditingController(text: '127.0.0.1:9002');
  final _scrollCtrl = ScrollController();
  final List<Map<String, dynamic>> _msgs = [];

  void _connect() {
    final port = int.tryParse(_portCtrl.text) ?? 9001;
    _core = VoidCore.init(port: port, key: List.filled(32, 0x42));
    _connected = true;

    _timer = Timer.periodic(const Duration(milliseconds: 200), (_) {
      final msg = _core?.poll();
      if (msg != null) {
        setState(() => _msgs.add(msg));
        _scroll();
      }
    });
    setState(() {});
  }

  void _send() {
    final text = _msgCtrl.text.trim();
    if (text.isEmpty || _core == null) return;

    _core!.sendMessage('chat', text, _peerCtrl.text);
    setState(() => _msgs.add({'text': text, 'incoming': false}));
    _msgCtrl.clear();
    _scroll();
  }

  void _scroll() {
    Future.delayed(const Duration(milliseconds: 50), () {
      if (_scrollCtrl.hasClients) {
        _scrollCtrl.animateTo(
          _scrollCtrl.position.maxScrollExtent,
          duration: const Duration(milliseconds: 100),
          curve: Curves.easeOut,
        );
      }
    });
  }

  @override
  void dispose() {
    _timer?.cancel();
    super.dispose();
  }

  @override
  Widget build(BuildContext context) {
    return Scaffold(
      backgroundColor: Colors.black,
      appBar: AppBar(
        title: const Text('Void', style: TextStyle(color: Colors.white)),
        backgroundColor: Colors.grey[900],
      ),
      body: Column(
        children: [
          if (!_connected)
            Padding(
              padding: const EdgeInsets.all(16),
              child: Column(children: [
                TextField(
                  controller: _portCtrl,
                  style: const TextStyle(color: Colors.white),
                  decoration: _deco('Мой порт'),
                  keyboardType: TextInputType.number,
                ),
                const SizedBox(height: 8),
                TextField(
                  controller: _peerCtrl,
                  style: const TextStyle(color: Colors.white),
                  decoration: _deco('Адрес пира (ip:port)'),
                ),
                const SizedBox(height: 12),
                SizedBox(
                  width: double.infinity,
                  child: ElevatedButton(
                    onPressed: _connect,
                    style: ElevatedButton.styleFrom(backgroundColor: Colors.deepPurple),
                    child: const Text('Подключиться'),
                  ),
                ),
              ]),
            ),
          Expanded(
            child: ListView.builder(
              controller: _scrollCtrl,
              padding: const EdgeInsets.all(12),
              itemCount: _msgs.length,
              itemBuilder: (_, i) {
                final m = _msgs[i];
                final inc = m['incoming'] == true;
                return Align(
                  alignment: inc ? Alignment.centerLeft : Alignment.centerRight,
                  child: Container(
                    margin: const EdgeInsets.symmetric(vertical: 3),
                    padding: const EdgeInsets.symmetric(horizontal: 14, vertical: 9),
                    decoration: BoxDecoration(
                      color: inc ? Colors.grey[800] : Colors.deepPurple,
                      borderRadius: BorderRadius.circular(16),
                    ),
                    child: Text(
                      m['text'] ?? '',
                      style: const TextStyle(color: Colors.white, fontSize: 15),
                    ),
                  ),
                );
              },
            ),
          ),
          if (_connected)
            Container(
              color: Colors.grey[900],
              padding: const EdgeInsets.symmetric(horizontal: 8, vertical: 6),
              child: Row(children: [
                Expanded(
                  child: TextField(
                    controller: _msgCtrl,
                    style: const TextStyle(color: Colors.white),
                    decoration: _deco('Сообщение...'),
                    onSubmitted: (_) => _send(),
                  ),
                ),
                IconButton(
                  icon: const Icon(Icons.send, color: Colors.deepPurple),
                  onPressed: _send,
                ),
              ]),
            ),
        ],
      ),
    );
  }

  InputDecoration _deco(String hint) => InputDecoration(
    hintText: hint,
    hintStyle: const TextStyle(color: Colors.white30),
    isDense: true,
    contentPadding: const EdgeInsets.symmetric(horizontal: 10, vertical: 10),
    border: OutlineInputBorder(borderRadius: BorderRadius.circular(8)),
    enabledBorder: OutlineInputBorder(
      borderRadius: BorderRadius.circular(8),
      borderSide: BorderSide(color: Colors.grey[700]!),
    ),
  );
}
DARTEOF
```

### lib/main.dart

```bash
cat > lib/main.dart << 'DARTEOF'
import 'package:flutter/material.dart';
import 'chat_screen.dart';

void main() => runApp(const MaterialApp(
  debugShowCheckedModeBanner: false,
  home: ChatScreen(),
));
DARTEOF
```

---

## Шаг 4: Сборка APK

```bash
cd ~/void_app
flutter build apk --release --target-platform android-arm64
```

Готовый APK:

```
~/void_app/build/app/outputs/flutter-apk/app-release.apk
```

---

## Быстрая пересборка после изменений

Если меняешь Rust код:

```bash
cd ~/void-core
cargo ndk -t arm64-v8a build --release
cp target/aarch64-linux-android/release/libvoid_core.so \
   ~/void_app/android/app/src/main/jniLibs/arm64-v8a/
cd ~/void_app
flutter build apk --release --target-platform android-arm64
```

Если меняешь только Dart код:

```bash
cd ~/void_app
flutter build apk --release --target-platform android-arm64
```





