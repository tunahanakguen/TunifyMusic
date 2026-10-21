mod engine;

use std::path::{Path, PathBuf};

use tauri::{AppHandle, Emitter, Manager};

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

/// Instrumente des Noten-Modus (id, label, available).
#[tauri::command]
async fn list_instruments() -> Result<serde_json::Value, String> {
    tauri::async_runtime::spawn_blocking(|| {
        let events = engine::run(&["instruments"])?;
        events
            .into_iter()
            .find(|e| e["event"] == "result")
            .map(|e| e["instruments"].clone())
            .ok_or_else(|| "Motor hat keine Instrumente gemeldet".to_string())
    })
    .await
    .map_err(|e| e.to_string())?
}

/// Speichert eine Mikrofon-Aufnahme (WebM/Opus aus dem MediaRecorder) unter Musik\Tunify Music\Aufnahmen.
#[tauri::command]
fn save_recording(app: AppHandle, request: tauri::ipc::Request<'_>) -> Result<String, String> {
    let tauri::ipc::InvokeBody::Raw(bytes) = request.body() else {
        return Err("Aufnahme kam nicht als Binärdaten an".into());
    };
    let dir = output_dir(&app)?.join("Aufnahmen");
    std::fs::create_dir_all(&dir).map_err(|e| format!("Ordner für Aufnahmen nicht anlegbar: {e}"))?;
    let mut n = 1;
    let mut path = dir.join("Aufnahme 1.webm");
    while path.exists() {
        n += 1;
        path = dir.join(format!("Aufnahme {n}.webm"));
    }
    std::fs::write(&path, bytes).map_err(|e| format!("Aufnahme nicht gespeichert: {e}"))?;
    Ok(path.to_string_lossy().to_string())
}

/// Einstellungen für „Stimme umwandeln“ → Instrument (Noten-Modus).
#[derive(serde::Deserialize)]
#[serde(rename_all = "camelCase")]
struct InstrumentRequest {
    input: String,
    instrument: String,
    label: String,
    transpose: i32,
    isolate_vocals: bool,
    dereverb: bool,
    denoise: bool,
    mix_instrumental: bool,
}

/// Ausgabeordner: Musik\Tunify Music
fn output_dir(app: &AppHandle) -> Result<PathBuf, String> {
    let dir = app.path().audio_dir().map_err(|e| e.to_string())?.join("Tunify Music");
    std::fs::create_dir_all(&dir).map_err(|e| format!("Ausgabeordner nicht anlegbar: {e}"))?;
    Ok(dir)
}

/// „Lied (Klavier).wav“, bei Bedarf „Lied (Klavier 2).wav“ usw. – nichts wird überschrieben.
fn unique_output(dir: &Path, input: &str, label: &str) -> PathBuf {
    let stem = Path::new(input).file_stem().and_then(|s| s.to_str()).unwrap_or("Aufnahme");
    let mut path = dir.join(format!("{stem} ({label}).wav"));
    let mut n = 2;
    while path.exists() {
        path = dir.join(format!("{stem} ({label} {n}).wav"));
        n += 1;
    }
    path
}

/// Gesang → Instrument über den Noten-Modus. Fortschritt kommt als Ereignis `engine-progress`.
#[tauri::command]
async fn convert_instrument(app: AppHandle, request: InstrumentRequest) -> Result<serde_json::Value, String> {
    let out = unique_output(&output_dir(&app)?, &request.input, &request.label);
    tauri::async_runtime::spawn_blocking(move || {
        let out = out.to_string_lossy().to_string();
        let transpose = request.transpose.to_string();
        let mut args = vec![
            "notes",
            request.input.as_str(),
            "--instrument",
            request.instrument.as_str(),
            "--out",
            out.as_str(),
            "--transpose",
            transpose.as_str(),
        ];
        for (on, flag) in [
            (request.isolate_vocals, "--isolate-vocals"),
            (request.dereverb, "--dereverb"),
            (request.denoise, "--denoise"),
            (request.mix_instrumental, "--mix-instrumental"),
        ] {
            if on {
                args.push(flag);
            }
        }
        let events = engine::run_streaming(&args, |event| {
            if event["event"] == "progress" {
                let _ = app.emit("engine-progress", event);
            }
        })?;
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
        .plugin(tauri_plugin_dialog::init())
        .invoke_handler(tauri::generate_handler![engine_info, list_instruments, save_recording, convert_instrument])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
