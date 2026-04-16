# Wasm Plugin Host — Event-Driven Actor Model

Высокопроизводительное, отказоустойчивое ядро для управления WebAssembly плагинами через асинхронную шину событий.

## Архитектура

```
┌─────────────────────────────────────────────┐
│           CLI Host (Rust)                    │
│     ┌──────────────────────────────────┐   │
│     │  Event Bus (tokio mpsc)          │   │
│     │  - SYS_STARTUP, SYS_SHUTDOWN    │   │
│     │  - UI_SEND_MSG, CRYPTO_*, DB_*  │   │
│     └──────────────────────────────────┘   │
└─────────────────────────────────────────────┘
         ↑                    ↑
    ┌────┴────┐      ┌───────┴───────┐
    │ Wasm    │      │   Wasm        │
    │ Crypto  │      │   Ntfy        │
    │ ChaCha20│      │   (Network)   │
    └─────────┘      └───────────────┘
         ↓                    ↓
    CRYPTO_ENCRYPTED → NET_SEND
    CRYPTO_DECRYPTED ← NET_RECEIVED
         ↓                    ↓
    ┌─────────────────────────────┐
    │  Wasm Storage (File DB)     │
    │  - Idempotency via event.id │
    │  - Atomic writes (WAL)      │
    └─────────────────────────────┘
```

## Компоненты

### Core (Host)
- **Supervisor** — управление жизненным циклом плагинов
- **Event Bus** — маршрутизация событий между плагинами
- **PluginRuntime** — абстракция над Wasm-движком
  - **WasmtimeRuntime** — JIT, полная поддержка (по умолчанию)
  - **WasmiRuntime** — интерпретатор для iOS (feature flag)
- **Signing** — верификация подписей Ed25519 и SHA256

### Плагины

1. **plugin-crypto** — AES-GCM шифрование
   - Подписка: `UI_SEND_MSG`, `NET_RECEIVED`
   - Издает: `CRYPTO_ENCRYPTED`, `CRYPTO_DECRYPTED`
   - Права: нет сети, нет FS

2. **plugin-ntfy** — сетевой транспорт (ntfy.sh)
   - Подписка: `SYS_STARTUP`, `CRYPTO_ENCRYPTED`, `NET_RECEIVED`
   - Издает: `NET_RECEIVED`
   - Права: сеть ✓, FS ✗

3. **plugin-storage** — файловое хранилище
   - Подписка: `UI_SEND_MSG`, `CRYPTO_DECRYPTED`, `DB_READ_CMD`
   - Издает: `DB_HISTORY_RESULT`
   - Права: FS ✓ (./data), сеть ✗
   - Идемпотентность по `event.meta.id`
   - Atomic writes через rename

## Требования

- **Rust** 1.70+
- **wasm32-wasip1** target (`rustup target add wasm32-wasip1`)
- **openssl** (для signing, опционально)

## Быстрый старт

### 1. Скомпилировать плагины

```bash
cargo build --release --target wasm32-wasip1 -p plugin-crypto
cargo build --release --target wasm32-wasip1 -p plugin-ntfy
cargo build --release --target wasm32-wasip1 -p plugin-storage
```

### 2. Запустить хост

```bash
# Default (Wasmtime)
cargo run --release -p core

# iOS (Wasmi)
cargo run --release -p core --no-default-features --features wasmi-backend
```

### 3. Взаимодействие

```
Ready. Commands: /history | /quit | <message>

> hello world
[crypto] CRYPTO_ENCRYPTED → [ntfy] → ntfy.sh
[storage] stored (idempotent by event.id)

> /history
[storage] DB_READ_CMD → DB_HISTORY_RESULT
[ntfy] NET_RECEIVED (SSE from ntfy.sh)

> /quit
```

## Hot Swap

Обновление файла `.wasm` автоматически:
1. Отправляет `SYS_SHUTDOWN` плагину (500ms на graceful shutdown)
2. Перезагружает новый `.wasm` из диска
3. Переключает входящие события на новый инстанс

## Подписание плагинов

### Генерация ключевой пары

```bash
openssl genpkey -algorithm ED25519 -out private.pem
openssl pkey -in private.pem -pubout -out public.pem

# Base64 для конфигов
openssl pkey -in private.pem -format DER | base64
openssl pkey -pubin -in public.pem -format DER | base64
```

### Подписание .wasm

```bash
bash keys/sign_plugin.sh target/wasm32-wasip1/release/plugin_crypto.wasm "<PRIVKEY_B64>"
```

Результат подставить в `manifests/crypto.toml`:
```toml
sha256    = "..."
signature = "..."
```

## Конфигурация

### manifest.toml

```toml
[plugin]
id        = "plugin_name"
version   = "1.0.0"
sha256    = "hex_hash"
signature = "base64_ed25519_sig"
wasm_path = "target/wasm32-wasip1/release/plugin_name.wasm"

[events]
subscriptions  = ["TOPIC1", "TOPIC2"]
max_queue_size = 1000

[supervisor]
restart_policy = "always" | "on_failure" | "never"
max_retries    = 3

[permissions]
network      = true/false
filesystem   = true/false
allowed_dirs = ["./data", "./config"]
```

## Telemetry

Логирование через `tracing`:

```bash
RUST_LOG=core=debug,warn cargo run -p core
```

Вывод:
```
2024-04-16T16:30:00Z INFO core: Loading plugin id=crypto version=1.0.0
2024-04-16T16:30:00Z DEBUG core: routing topic=UI_SEND_MSG id=550e8400-e29b-41d4-a716-446655440000
2024-04-16T16:30:00Z DEBUG core: handled plugin=crypto topic=UI_SEND_MSG fuel=1234567
```

## Ограничения ресурсов

| Параметр      | Значение        |
|---------------|-----------------|
| Fuel limit    | 50,000,000 ins  |
| Memory limit  | 50 MB           |
| Max queue     | configurable    |

Превышение → автоматическая остановка и переход в DLQ.

## Dead Letter Queue

События, обработанные с ошибками:
- Десериализация meta.json
- Паника плагина
- Overflow очереди

Попадают в `SYS_DLQ` для аудита и retry.

## CI/CD

GitHub Actions (`.github/workflows/build.yml`):
- ✓ Build core + tests
- ✓ Build wasm plugins
- ✓ Clippy + fmt

```bash
git push → Actions → artifacts (core, plugin*.wasm)
```

## Производство

Для production:
1. Включить проверку подписей в `main.rs`:
   ```rust
   let verifying_key = signing::load_verifying_key(&public_key_b64)?;
   signing::verify_wasm(&bytes, &manifest.plugin.sha256, &manifest.plugin.signature, &verifying_key)?;
   ```

2. Заменить stub `host_http_post`/`host_sse_start` на реальный `reqwest`:
   ```rust
   tokio::spawn(async move {
       let client = reqwest::Client::new();
       let _ = client.post(&url).body(body).send().await;
   });
   ```

3. Настроить graceful shutdown в `supervisor.rs`

4. Мониторинг через `tracing` → OpenTelemetry

## Лицензия

MIT
