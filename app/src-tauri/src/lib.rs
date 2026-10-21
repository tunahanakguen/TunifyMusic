mod engine;
mod history;
mod voices;

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
    /// Begleitung beim Dazumischen um Halbtöne verschieben (Cover in anderer Tonlage)
    #[serde(default)]
    instrumental_pitch: i32,
    /// "wav", "mp3" oder "flac"
    format: String,
    /// erkannte Noten zusätzlich als .mid neben die Ausgabe legen
    midi: bool,
}

const FORMATS: [&str; 3] = ["wav", "mp3", "flac"];

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
        let instrumental_pitch = request.instrumental_pitch.to_string();
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
        if request.mix_instrumental && request.instrumental_pitch != 0 {
            args.extend(["--instrumental-pitch", instrumental_pitch.as_str()]);
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

/// Stimmen-Bibliothek im App-Datenordner (%APPDATA%\de.tunas.tunifymusic\voices)
fn voices_dir(app: &AppHandle) -> Result<PathBuf, String> {
    let dir = app.path().app_data_dir().map_err(|e| e.to_string())?.join("voices");
    std::fs::create_dir_all(&dir).map_err(|e| format!("Stimmen-Ordner nicht anlegbar: {e}"))?;
    Ok(dir)
}

#[tauri::command]
fn list_voices(app: AppHandle) -> Result<Vec<voices::Voice>, String> {
    Ok(voices::list(&voices_dir(&app)?))
}

/// Kopiert lange: im Hintergrund, damit das Fenster nicht hängt.
#[tauri::command]
async fn import_voice(app: AppHandle, name: String, files: Vec<String>) -> Result<voices::Voice, String> {
    let dir = voices_dir(&app)?;
    tauri::async_runtime::spawn_blocking(move || voices::import(&dir, &name, &files))
        .await
        .map_err(|e| e.to_string())?
}

#[tauri::command]
fn rename_voice(app: AppHandle, id: String, name: String) -> Result<voices::Voice, String> {
    voices::rename(&voices_dir(&app)?, &id, &name)
}

#[tauri::command]
fn delete_voice(app: AppHandle, id: String) -> Result<(), String> {
    voices::delete(&voices_dir(&app)?, &id)
}

/// Einstellungen für „Stimme umwandeln“ → andere Stimme (RVC).
#[derive(serde::Deserialize)]
#[serde(rename_all = "camelCase")]
struct VoiceRequest {
    input: String,
    voice: String,
    pitch: i32,
    formant: f64,
    index_rate: f64,
    protect: f64,
    isolate_vocals: bool,
    dereverb: bool,
    denoise: bool,
    mix_instrumental: bool,
    #[serde(default)]
    instrumental_pitch: i32,
    format: String,
}

/// Zeichen, die in Windows-Dateinamen verboten sind
fn file_label(name: &str) -> String {
    name.chars().map(|c| if r#"\/:*?"<>|"#.contains(c) { '_' } else { c }).collect()
}

/// Stimme → Stimme mit RVC. Fortschritt kommt als Ereignis `engine-progress`.
#[tauri::command]
async fn convert_voice(app: AppHandle, request: VoiceRequest) -> Result<serde_json::Value, String> {
    if !FORMATS.contains(&request.format.as_str()) {
        return Err(format!("Unbekanntes Format: {}", request.format));
    }
    let voice = voices::get(&voices_dir(&app)?, &request.voice)?;
    let out = unique_output(&output_dir(&app)?, &request.input, &file_label(&voice.name), &request.format);
    tauri::async_runtime::spawn_blocking(move || {
        let out = out.to_string_lossy().to_string();
        let (pitch, formant) = (request.pitch.to_string(), request.formant.to_string());
        let (index_rate, protect) = (request.index_rate.to_string(), request.protect.to_string());
        let instrumental_pitch = request.instrumental_pitch.to_string();
        let mut args = vec![
            "convert",
            request.input.as_str(),
            "--model",
            voice.model.as_str(),
            "--out",
            out.as_str(),
            "--pitch",
            pitch.as_str(),
            "--formant",
            formant.as_str(),
            "--protect",
            protect.as_str(),
        ];
        if let Some(index) = &voice.index {
            args.extend(["--index", index.as_str(), "--index-rate", index_rate.as_str()]);
        }
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
        if request.mix_instrumental && request.instrumental_pitch != 0 {
            args.extend(["--instrumental-pitch", instrumental_pitch.as_str()]);
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
        result["voice"] = voice.name.into();
        Ok(result)
    })
    .await
    .map_err(|e| e.to_string())?
}

/// Einstellungen für „Stimme trainieren“.
#[derive(serde::Deserialize)]
#[serde(rename_all = "camelCase")]
struct TrainRequest {
    name: String,
    inputs: Vec<String>,
    epochs: u32,
    isolate_vocals: bool,
    dereverb: bool,
    denoise: bool,
}

/// Arbeitsordner eines Trainings löschen (enthält über 1 GB Zwischenstände) – nur innerhalb von Applios logs/.
fn remove_workspace(path: &str) {
    let logs = engine::engine_dir().join("vendor").join("applio").join("logs");
    let path = PathBuf::from(path);
    let inside = match (path.canonicalize(), logs.canonicalize()) {
        (Ok(p), Ok(l)) => p.starts_with(&l) && p != l,
        _ => false,
    };
    if inside {
        let _ = std::fs::remove_dir_all(&path);
    }
}

/// Trainiert eine Stimme aus eigenen Aufnahmen und legt sie in die Bibliothek.
/// Fortschritt kommt als Ereignis `train-progress`; abbrechen wie beim Umwandeln.
#[tauri::command]
async fn train_voice(app: AppHandle, request: TrainRequest) -> Result<voices::Voice, String> {
    let name = request.name.trim().to_string();
    if name.is_empty() {
        return Err("Bitte einen Namen für die Stimme angeben".into());
    }
    if request.inputs.is_empty() {
        return Err("Bitte mindestens eine Aufnahme hinzufügen".into());
    }
    if !(1..=1000).contains(&request.epochs) {
        return Err("Epochen müssen zwischen 1 und 1000 liegen".into());
    }
    let library = voices_dir(&app)?;
    tauri::async_runtime::spawn_blocking(move || {
        let epochs = request.epochs.to_string();
        let mut args = vec!["train"];
        args.extend(request.inputs.iter().map(String::as_str));
        args.extend(["--epochs", epochs.as_str()]);
        for (on, flag) in [
            (request.isolate_vocals, "--isolate-vocals"),
            (request.dereverb, "--dereverb"),
            (request.denoise, "--denoise"),
        ] {
            if on {
                args.push(flag);
            }
        }
        let mut workspace = None;
        let outcome = engine::run_job(&args, |event| match event["event"].as_str() {
            Some("workspace") => workspace = event["path"].as_str().map(str::to_string),
            Some("progress") => {
                let _ = app.emit("train-progress", event);
            }
            _ => {}
        })
        .and_then(|events| {
            let result = events
                .into_iter()
                .find(|e| e["event"] == "result")
                .ok_or_else(|| "Motor hat kein Ergebnis gemeldet".to_string())?;
            let mut files = vec![result["model"].as_str().unwrap_or_default().to_string()];
            if let Some(index) = result["index"].as_str() {
                files.push(index.to_string());
            }
            voices::import(&library, &name, &files)
        });
        if let Some(path) = workspace {
            remove_workspace(&path);
        }
        outcome
    })
    .await
    .map_err(|e| e.to_string())?
}

/// Einstellungen für „Lied erzeugen“ und „Instrumental erzeugen“ (ACE-Step).
#[derive(serde::Deserialize)]
#[serde(rename_all = "camelCase")]
struct SongRequest {
    caption: String,
    lyrics: String,
    instrumental: bool,
    duration: u32,
    bpm: Option<u32>,
    key: String,
    language: String,
    seed: Option<i64>,
    /// Stimme aus der Bibliothek, in der gesungen wird (nur mit Gesang)
    voice: Option<String>,
    pitch: i32,
    format: String,
}

/// Dateiname aus der Beschreibung: die ersten Wörter, ohne verbotene Zeichen
fn song_title(caption: &str, instrumental: bool) -> String {
    let words: Vec<&str> = caption
        .split_whitespace()
        .map(|w| w.trim_matches(|c: char| ",.;!?".contains(c)))
        .filter(|w| !w.is_empty())
        .take(5)
        .collect();
    let title = file_label(&words.join(" "));
    match (title.is_empty(), instrumental) {
        (true, true) => "Instrumental".into(),
        (true, false) => "Lied".into(),
        (false, _) => title,
    }
}

/// Erzeugt ein Lied oder Instrumental. Fortschritt kommt als Ereignis `song-progress`.
#[tauri::command]
async fn generate_song(app: AppHandle, request: SongRequest) -> Result<serde_json::Value, String> {
    if !FORMATS.contains(&request.format.as_str()) {
        return Err(format!("Unbekanntes Format: {}", request.format));
    }
    let voice = match &request.voice {
        Some(id) if !request.instrumental => Some(voices::get(&voices_dir(&app)?, id)?),
        _ => None,
    };
    let kind = match (&voice, request.instrumental) {
        (_, true) => "Instrumental".to_string(),
        (Some(v), false) => file_label(&v.name),
        (None, false) => "Lied".to_string(),
    };
    let out = unique_output(&output_dir(&app)?, &song_title(&request.caption, request.instrumental), &kind, &request.format);
    tauri::async_runtime::spawn_blocking(move || {
        let out = out.to_string_lossy().to_string();
        let duration = request.duration.to_string();
        let bpm = request.bpm.map(|b| b.to_string());
        let seed = request.seed.unwrap_or(-1).to_string();
        let pitch = request.pitch.to_string();
        let mut args = vec![
            "song",
            "--caption",
            request.caption.as_str(),
            "--lyrics",
            request.lyrics.as_str(),
            "--duration",
            duration.as_str(),
            "--key",
            request.key.as_str(),
            "--language",
            request.language.as_str(),
            "--seed",
            seed.as_str(),
            "--out",
            out.as_str(),
        ];
        if request.instrumental {
            args.push("--instrumental");
        }
        if let Some(bpm) = &bpm {
            args.extend(["--bpm", bpm.as_str()]);
        }
        if let Some(voice) = &voice {
            args.extend(["--voice-model", voice.model.as_str(), "--pitch", pitch.as_str()]);
            if let Some(index) = &voice.index {
                args.extend(["--voice-index", index.as_str()]);
            }
        }
        let events = engine::run_job(&args, |event| {
            if event["event"] == "progress" {
                let _ = app.emit("song-progress", event);
            }
        })?;
        let mut result = events
            .into_iter()
            .find(|e| e["event"] == "result")
            .ok_or_else(|| "Motor hat kein Ergebnis gemeldet".to_string())?;
        if let Some(voice) = voice {
            result["voice"] = voice.name.into();
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

/// Favoriten des Verlaufs im App-Datenordner
fn favorites_file(app: &AppHandle) -> Result<PathBuf, String> {
    let dir = app.path().app_data_dir().map_err(|e| e.to_string())?;
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    Ok(dir.join("favorites.json"))
}

/// Alle Ergebnisse und Aufnahmen, neueste zuerst.
#[tauri::command]
fn list_history(app: AppHandle) -> Result<Vec<history::Item>, String> {
    Ok(history::list(&output_dir(&app)?, &favorites_file(&app)?))
}

#[tauri::command]
fn set_favorite(app: AppHandle, path: String, favorite: bool) -> Result<(), String> {
    history::set_favorite(&output_dir(&app)?, &favorites_file(&app)?, &path, favorite)
}

/// In den Papierkorb verschieben (nur Dateien im Ordner von Tunify Music).
#[tauri::command]
fn delete_history_item(app: AppHandle, path: String) -> Result<(), String> {
    history::delete(&output_dir(&app)?, &favorites_file(&app)?, &path)
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
    fn liedtitel_aus_beschreibung() {
        assert_eq!(song_title("Lofi hip hop beat, mellow piano, soft drums", true), "Lofi hip hop beat mellow");
        assert_eq!(song_title("Rock: laut/schnell", false), "Rock_ laut_schnell");
        assert_eq!(song_title("   ", true), "Instrumental");
        assert_eq!(song_title("", false), "Lied");
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
        .setup(|_| {
            // Motor gleich beim Start laden (ca. 20 s), damit die erste Umwandlung nicht darauf wartet
            std::thread::spawn(|| {
                let _ = engine::start_server();
            });
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            engine_info,
            list_instruments,
            save_recording,
            convert_instrument,
            list_voices,
            import_voice,
            rename_voice,
            delete_voice,
            convert_voice,
            train_voice,
            generate_song,
            cancel_conversion,
            list_history,
            set_favorite,
            delete_history_item,
            output_folder
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
