use extism::{Manifest, Plugin, Wasm};
use std::path::Path;

fn main() {
    println!("🚀 Запускаем Хост...");

    // Путь к скомпилированному плагину (Cargo кладет его сюда)
    let wasm_path = "./plugins/plugin_test.wasm";

    if !Path::new(wasm_path).exists() {
        println!("⚠️ Файл плагина не найден: {}", wasm_path);
        println!("Остановись и скомпилируй плагин командой из инструкции!");
        return;
    }

    // 1. Загружаем wasm-файл
    let wasm = Wasm::file(wasm_path);
    let manifest = Manifest::new([wasm]);

    // 2. Создаем инстанс плагина 
    // (второй аргумент - это функции хоста, третий - поддержка WASI. Пока нам это не нужно)
    let mut plugin = match Plugin::new(&manifest, [], false) {
        Ok(p) => p,
        Err(e) => {
            eprintln!("❌ Ошибка загрузки плагина: {}", e);
            return;
        }
    };

    println!("✅ Плагин успешно загружен!");

    // 3. Вызываем функцию "greet" из плагина
    let input_text = "Создатель";
    println!("➡️ Отправляем в плагин текст: \"{}\"", input_text);

    // Вызываем функцию и указываем типы: передаем &str, ожидаем &str
    match plugin.call::<&str, &str>("greet", input_text) {
        Ok(response) => println!("⬅️ Ответ от плагина: \"{}\"", response),
        Err(e) => eprintln!("❌ Ошибка вызова функции: {}", e),
    }
}
