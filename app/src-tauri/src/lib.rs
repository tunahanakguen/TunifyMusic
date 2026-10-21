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
/// Sie wird gleich in WAV umgewandelt: WebM aus dem Browser hat keine Längenangabe, Abspieler zeigen sonst
/// keinen brauchbaren Fortschritt. Klappt das nicht, bleibt die WebM-Datei.
#[tauri::command]
fn save_recording(app: AppHandle, request: tauri::ipc::Request<'_>) -> Result<String, String> {
    let tauri::ipc::InvokeBody::Raw(bytes) = request.body() else {
        return Err("Aufnahme kam nicht als Binärdaten an".into());
    };
    let dir = output_dir(&app)?.join("Aufnahmen");
    std::fs::create_dir_all(&dir).map_err(|e| format!("Ordner für Aufnahmen nicht anlegbar: {e}"))?;
    let mut n = 1;
    let name = |n: u32| format!("Aufnahme {n}");
    while ["webm", "wav"].iter().any(|ext| dir.join(format!("{}.{ext}", name(n))).exists()) {
        n += 1;
    }
    let webm = dir.join(format!("{}.webm", name(n)));
    std::fs::write(&webm, bytes).map_err(|e| format!("Aufnahme nicht gespeichert: {e}"))?;
    let path = webm_to_wav(&webm).unwrap_or(webm);
    Ok(path.to_string_lossy().to_string())
}

/// WebM → WAV mit FFmpeg; die WebM-Datei wird danach gelöscht.
fn webm_to_wav(webm: &Path) -> Option<PathBuf> {
    let wav = webm.with_extension("wav");
    let mut ffmpeg = std::process::Command::new("ffmpeg");
    ffmpeg.args(["-y", "-loglevel", "error", "-i"]).arg(webm).arg(&wav);
    #[cfg(windows)]
    std::os::windows::process::CommandExt::creation_flags(&mut ffmpeg, 0x0800_0000);
    let ok = ffmpeg.status().map(|s| s.success()).unwrap_or(false);
    if ok && wav.exists() {
        let _ = std::fs::remove_file(webm);
        Some(wav)
    } else {
        let _ = std::fs::remove_file(&wav);
        None
    }
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
    /// "wav", "mp3" oder "flac"
    format: String,
    /// erkannte Noten zusätzlich als .mid neben die Ausgabe legen
    midi: bool,
}

const FORMATS: [&str; 3] = ["wav", "mp3", "flac"];
const AUDIO_EXTENSIONS: [&str; 4] = ["wav", "mp3", "flac", "webm"];

/// Ausgabeordner: Musik\Tunify Music
fn output_dir(app: &AppHandle) -> Result<PathBuf, String> {
    let dir = app.path().audio_dir().map_err(|e| e.to_string())?.join("Tunify Music");
    std::fs::create_dir_all(&dir).map_err(|e| format!("Ausgabeordner nicht anlegbar: {e}"))?;
    Ok(dir)
}

/// „Lied (Klavier).wav“, bei Bedarf „Lied (Klavier 2).wav“ usw. – nichts wird überschrieben,
/// auch keine gleichnamige Datei in einem anderen Format oder MIDI.
fn unique_output(dir: &Path, input: &str, label: &str, extension: &str) -> PathBuf {
    let stem = Path::new(input).file_stem().and_then(|s| s.to_str()).unwrap_or("Aufnahme");
    let taken = |name: &str| ["wav", "mp3", "flac", "mid"].iter().any(|ext| dir.join(format!("{name}.{ext}")).exists());
    let mut name = format!("{stem} ({label})");
    let mut n = 2;
    while taken(&name) {
        name = format!("{stem} ({label} {n})");
        n += 1;
    }
    dir.join(format!("{name}.{extension}"))
}

/// Gesang → Instrument über den Noten-Modus. Fortschritt kommt als Ereignis `engine-progress`.
#[tauri::command]
async fn convert_instrument(app: AppHandle, request: InstrumentRequest) -> Result<serde_json::Value, String> {
    if !FORMATS.contains(&request.format.as_str()) {
        return Err(format!("Unbekanntes Format: {}", request.format));
    }
    let out = unique_output(&output_dir(&app)?, &request.input, &request.label, &request.format);
    tauri::async_runtime::spawn_blocking(move || {
        let midi = out.with_extension("mid").to_string_lossy().to_string();
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
        if request.midi {
            args.extend(["--midi", midi.as_str()]);
        }
        let events = engine::run_job(&args, |event| {
            if event["event"] == "progress" {
                let _ = app.emit("engine-progress", event);
            }
        })?;
        let mut result = events
            .into_iter()
            .find(|e| e["event"] == "result")
            .ok_or_else(|| "Motor hat kein Ergebnis gemeldet".to_string())?;
        if request.midi {
            result["midi"] = midi.into();
        }
        Ok(result)
    })
    .await
    .map_err(|e| e.to_string())?
}

/// Bricht die laufende Umwandlung ab.
#[tauri::command]
fn cancel_conversion() -> bool {
    engine::cancel()
}

/// Ein Eintrag im Verlauf: eine Datei aus Musik\Tunify Music (Ergebnisse) oder …\Aufnahmen.
#[derive(serde::Serialize)]
struct HistoryItem {
    path: String,
    name: String,
    recording: bool,
    /// Sekunden seit 1970
    modified: u64,
    bytes: u64,
}

/// Alle Ergebnisse und Aufnahmen, neueste zuerst.
#[tauri::command]
fn list_history(app: AppHandle) -> Result<Vec<HistoryItem>, String> {
    let dir = output_dir(&app)?;
    let mut items = Vec::new();
    for (folder, recording) in [(dir.clone(), false), (dir.join("Aufnahmen"), true)] {
        let Ok(entries) = std::fs::read_dir(&folder) else { continue };
        for entry in entries.flatten() {
            let path = entry.path();
            let extension = path.extension().and_then(|e| e.to_str()).unwrap_or("").to_lowercase();
            let Ok(meta) = entry.metadata() else { continue };
            if !meta.is_file() || !AUDIO_EXTENSIONS.contains(&extension.as_str()) {
                continue;
            }
            let modified = meta
                .modified()
                .ok()
                .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
                .map_or(0, |d| d.as_secs());
            items.push(HistoryItem {
                name: path.file_name().unwrap_or_default().to_string_lossy().to_string(),
                path: path.to_string_lossy().to_string(),
                recording,
                modified,
                bytes: meta.len(),
            });
        }
    }
    items.sort_by(|a, b| b.modified.cmp(&a.modified));
    Ok(items)
}

/// Pfad des Ausgabeordners (für „Ordner öffnen“).
#[tauri::command]
fn output_folder(app: AppHandle) -> Result<String, String> {
    Ok(output_dir(&app)?.to_string_lossy().to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ausgabe_ueberschreibt_nichts() {
        let dir = std::env::temp_dir().join(format!("tunify-test-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let first = unique_output(&dir, r"C:\x\Lied.m4a", "Geige", "mp3");
        assert_eq!(first.file_name().unwrap(), "Lied (Geige).mp3");
        // Gleicher Name als WAV vorhanden → auch das MP3 bekommt eine neue Nummer
        std::fs::write(dir.join("Lied (Geige).wav"), b"").unwrap();
        assert_eq!(unique_output(&dir, "Lied.m4a", "Geige", "mp3").file_name().unwrap(), "Lied (Geige 2).mp3");
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn aufnahme_wird_zu_wav() {
        let dir = std::env::temp_dir().join(format!("tunify-webm-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let webm = dir.join("Aufnahme 1.webm");
        // 1 s Sinuston als WebM/Opus, wie ihn der MediaRecorder liefert
        let made = std::process::Command::new("ffmpeg")
            .args(["-y", "-loglevel", "error", "-f", "lavfi", "-i", "sine=frequency=220:duration=1", "-c:a", "libopus"])
            .arg(&webm)
            .status()
            .expect("FFmpeg vorhanden");
        assert!(made.success());
        let wav = webm_to_wav(&webm).expect("umgewandelt");
        assert_eq!(wav.file_name().unwrap(), "Aufnahme 1.wav");
        assert!(!webm.exists(), "WebM ist danach weg");
        std::fs::remove_dir_all(&dir).unwrap();
    }
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        .invoke_handler(tauri::generate_handler![
            engine_info,
            list_instruments,
            save_recording,
            convert_instrument,
            cancel_conversion,
            list_history,
            output_folder
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
