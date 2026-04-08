# Messenger MVP

Микроядерный мессенджер: Rust ядро + Extism WASM плагины + Flutter UI.

## Структура

```
messenger/
├── core/               Rust ядро (C FFI для Flutter)
├── plugins/
│   ├── storage_memory/ WASM плагин — хранилище в RAM
│   └── crypto_aes/     WASM плагин — AES-256-GCM шифрование
├── flutter_app/        Flutter UI
├── build.sh            Скрипт сборки всего
└── Cargo.toml          Workspace
```

## Требования

- Rust 1.78+ с `wasm32-wasip1` таргетом
- Flutter 3.22+
- На Linux: `gcc`, `cmake`, `pkg-config`

## Сборка

```bash
chmod +x build.sh
./build.sh
```

Скрипт:
1. Компилирует ядро в нативную библиотеку (`.so` / `.dylib`)
2. Компилирует плагины в `.wasm`
3. Копирует всё в Flutter assets и linux/ папку

## Запуск двух экземпляров для теста

**Терминал 1:**
```bash
cd flutter_app
flutter run --dart-define=PORT=7777
```

**Терминал 2:**
```bash
cd flutter_app
flutter run -d linux --dart-define=PORT=8888
```

В каждом приложении при старте введи свой порт (7777 или 8888).
Добавь контакт `127.0.0.1:8888` в первом и `127.0.0.1:7777` во втором.

## Добавление своего плагина

1. Создай крейт с `crate-type = ["cdylib"]`
2. Добавь `extism-pdk` в зависимости
3. Реализуй функции через `#[plugin_fn]`
4. Создай `manifest.toml` по шаблону ниже
5. Скомпилируй: `cargo build --release --target wasm32-wasip1`
6. В приложении: Plugins → Add Plugin → выбери `.wasm`

### Шаблон manifest.toml

```toml
[plugin]
id = "my_plugin"
name = "My Plugin"
version = "0.1.0"
category = "storage"   # storage | crypto | transport | ui
description = "Описание"

[capabilities]
provides = ["my_function"]
subscribes_to = []
emits = []

[permissions]
network = false
filesystem = false
contacts = false
clipboard = false
notifications = false

[limits]
max_memory_mb = 32
timeout_ms = 3000
```

## Категории плагинов

| Категория   | Ядро ищет плагин для               | Функции           |
|-------------|-------------------------------------|-------------------|
| `crypto`    | шифрования перед отправкой         | `encrypt`, `decrypt` |
| `storage`   | хранения сообщений                 | `store_message`, `get_messages` |
| `transport` | (будущее) кастомная доставка       | `deliver`         |

## Архитектура

```
Flutter UI
  ↕ dart:ffi (C ABI)
Rust Core
  ├── Event Bus (poll-based)
  ├── Command Bus (sync call)
  ├── Plugin Registry
  └── TCP Transport (встроенный в ядро для MVP)
        ↕ Extism WASM sandbox
  [storage_memory] [crypto_aes] [your_plugin]
```
