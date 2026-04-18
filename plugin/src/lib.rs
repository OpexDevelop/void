use extism_pdk::*;

// Этот макрос автоматически делает функцию доступной для Хоста
#[plugin_fn]
pub fn greet(name: String) -> FnResult<String> {
    let result = format!("Привет, {}! Я работаю изнутри Wasm, и я не сломал память!", name);
    Ok(result)
}
