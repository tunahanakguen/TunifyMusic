mod engine;

/// Meldet Python-, PyTorch- und Grafikkarten-Infos des Motors.
#[tauri::command]
async fn engine_info() -> Result<serde_json::Value, String> {
    tauri::async_runtime::spawn_blocking(|| {
        let events = engine::run(&["info"])?;
        events
            .into_iter()
            .find(|e| e["event"] == "result")
            .ok_or_else(|| "Motor hat kein Ergebnis gemeldet".to_string())
    })
    .await
    .map_err(|e| e.to_string())?
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .invoke_handler(tauri::generate_handler![engine_info])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
