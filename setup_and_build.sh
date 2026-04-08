#!/bin/bash
set -e

echo "=== 1. Установка Java 17 ==="
sudo apt-get update
sudo apt-get install -y openjdk-17-jdk
export JAVA_HOME=$(find /usr/lib/jvm -maxdepth 1 -name "*java-17-openjdk*" | head -n 1)
export PATH=$JAVA_HOME/bin:$PATH

echo "=== 2. Настройка Rust и WASM ==="
rustup target add aarch64-linux-android
rustup target add wasm32-wasip1
cargo install cargo-ndk

echo "=== 3. Сборка ядра (Core) ==="
cd core
cargo ndk -t arm64-v8a -o ../flutter_app/android/app/src/main/jniLibs build --release
cd ..

echo "=== 4. Сборка плагинов ==="

cargo build --release -p storage_memory --target wasm32-wasip1
cargo build --release -p crypto_aes --target wasm32-wasip1
cp target/wasm32-wasip1/release/*.wasm flutter_app/assets/plugins/
cd ..

echo "=== 5. Настройка Gradle и сборка APK (с лимитами памяти) ==="
cd flutter_app
mkdir -p android
echo "org.gradle.jvmargs=-Xmx1536m -XX:MaxMetaspaceSize=256m" > android/gradle.properties
echo "org.gradle.parallel=false" >> android/gradle.properties
echo "org.gradle.workers.max=1" >> android/gradle.properties
echo "android.useAndroidX=true" >> android/gradle.properties
echo "android.enableJetifier=true" >> android/gradle.properties

flutter clean
flutter build apk --release --target-platform android-arm64 --no-daemon

echo "=== Готово! APK и плагины успешно собраны. ==="
