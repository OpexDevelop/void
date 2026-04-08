#!/usr/bin/env bash
# Останавливаем скрипт при любой ошибке
set -e

echo "=================================================="
echo "1. УСТАНОВКА СИСТЕМНЫХ ЗАВИСИМОСТЕЙ И JAVA 17"
echo "=================================================="
sudo apt-get update
sudo apt-get install -y openjdk-17-jdk unzip curl git ninja-build pkg-config cmake clang

echo "=================================================="
echo "2. НАСТРОЙКА RUST И CARGO-NDK"
echo "=================================================="
# Устанавливаем таргеты для Android и WASM
rustup target add aarch64-linux-android armv7-linux-androideabi x86_64-linux-android
rustup target add wasm32-wasip1
# Устанавливаем cargo-ndk
cargo install cargo-ndk

echo "=================================================="
echo "3. УСТАНОВКА ANDROID SDK И NDK"
echo "=================================================="
export ANDROID_HOME=$HOME/android
export ANDROID_SDK_ROOT=$HOME/android

mkdir -p $ANDROID_HOME/cmdline-tools && cd $ANDROID_HOME/cmdline-tools
wget -q https://dl.google.com/android/repository/commandlinetools-linux-11076708_latest.zip -O tools.zip
unzip -q tools.zip && mv cmdline-tools latest && rm tools.zip

export PATH=$PATH:$ANDROID_HOME/cmdline-tools/latest/bin:$ANDROID_HOME/platform-tools

# Принимаем лицензии и ставим нужные компоненты
yes | sdkmanager --licenses > /dev/null
sdkmanager "platform-tools" "platforms;android-35" "build-tools;35.0.0" "ndk;27.0.12077973"

export ANDROID_NDK_HOME=$ANDROID_HOME/ndk/27.0.12077973

echo "=================================================="
echo "4. УСТАНОВКА FLUTTER"
echo "=================================================="
cd $HOME
if [ ! -d "flutter" ]; then
    git clone https://github.com/flutter/flutter.git -b stable --depth 1
fi
export PATH=$PATH:$HOME/flutter/bin

flutter config --android-sdk $ANDROID_HOME
yes | flutter doctor --android-licenses > /dev/null

echo "=================================================="
echo "5. СОХРАНЕНИЕ ПУТЕЙ ДЛЯ БУДУЩИХ СЕССИЙ ТЕРМИНАЛА"
echo "=================================================="
BASHRC="$HOME/.bashrc"
# Добавляем переменные только если их там еще нет
grep -q "ANDROID_HOME" "$BASHRC" || echo 'export ANDROID_HOME=$HOME/android' >> "$BASHRC"
grep -q "ANDROID_SDK_ROOT" "$BASHRC" || echo 'export ANDROID_SDK_ROOT=$HOME/android' >> "$BASHRC"
grep -q "ANDROID_NDK_HOME" "$BASHRC" || echo 'export ANDROID_NDK_HOME=$ANDROID_HOME/ndk/27.0.12077973' >> "$BASHRC"
grep -q "flutter/bin" "$BASHRC" || echo 'export PATH=$PATH:$ANDROID_HOME/cmdline-tools/latest/bin:$ANDROID_HOME/platform-tools:$HOME/flutter/bin' >> "$BASHRC"

echo "=================================================="
echo "6. СБОРКА ПРОЕКТА (WASM, RUST CORE, APK)"
echo "=================================================="
# Возвращаемся в папку проекта (GitHub Codespaces монтирует репозиторий в /workspaces/<repo-name>)
cd "$GITHUB_WORKSPACE"

echo "==> Собираем WASM плагины..."
cargo build --release -p storage_memory --target wasm32-wasip1
cargo build --release -p crypto_aes --target wasm32-wasip1

echo "==> Копируем плагины во Flutter assets..."
mkdir -p flutter_app/assets/plugins
cp target/wasm32-wasip1/release/storage_memory.wasm flutter_app/assets/plugins/
cp target/wasm32-wasip1/release/crypto_aes.wasm flutter_app/assets/plugins/
cp plugins/storage_memory/manifest.toml flutter_app/assets/plugins/storage_memory.manifest.toml
cp plugins/crypto_aes/manifest.toml flutter_app/assets/plugins/crypto_aes.manifest.toml

echo "==> Собираем нативное ядро (Core) под Android..."
cd core
cargo ndk -t arm64-v8a -t armeabi-v7a -t x86_64 -o ../flutter_app/android/app/src/main/jniLibs build --release
cd ..

echo "==> Собираем APK..."
cd flutter_app
flutter build apk --release

echo "=================================================="
echo "ГОТОВО! APK находится здесь:"
echo "flutter_app/build/app/outputs/flutter-apk/app-release.apk"
echo "=================================================="
