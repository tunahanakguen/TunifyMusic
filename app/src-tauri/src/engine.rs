//! Startet den Python-Motor (engine/) und liest seine JSON-Zeilen.

use std::path::PathBuf;
use std::process::Command;

#[cfg(windows)]
use std::os::windows::process::CommandExt;

/// Ohne eigenes Konsolenfenster starten.
#[cfg(windows)]
const CREATE_NO_WINDOW: u32 = 0x0800_0000;

/// Ordner des Motors. In der Entwicklung liegt er neben `app/` im Repo.
/// Mit `TUNIFY_ENGINE_DIR` lässt er sich überschreiben.
pub fn engine_dir() -> PathBuf {
    if let Ok(dir) = std::env::var("TUNIFY_ENGINE_DIR") {
        return PathBuf::from(dir);
    }
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("..").join("..").join("engine")
}

fn python() -> PathBuf {
    engine_dir().join(".venv").join("Scripts").join("python.exe")
}

/// Führt einen Motor-Befehl aus und gibt alle Ereignisse zurück.
/// Ein `error`-Ereignis wird zum Fehler.
pub fn run(args: &[&str]) -> Result<Vec<serde_json::Value>, String> {
    let python = python();
    if !python.exists() {
        return Err(format!("Python-Motor nicht eingerichtet: {} fehlt", python.display()));
    }
    let mut command = Command::new(&python);
    command.arg("-m").arg("tunify_engine").args(args).current_dir(engine_dir());
    #[cfg(windows)]
    command.creation_flags(CREATE_NO_WINDOW);

    let output = command.output().map_err(|e| format!("Motor startet nicht: {e}"))?;
    let events: Vec<serde_json::Value> = String::from_utf8_lossy(&output.stdout)
        .lines()
        .filter_map(|line| serde_json::from_str(line).ok())
        .collect();

    if let Some(error) = events.iter().find(|e| e["event"] == "error") {
        return Err(error["message"].as_str().unwrap_or("Unbekannter Fehler").to_string());
    }
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        let tail: Vec<&str> = stderr.lines().rev().take(5).collect();
        return Err(format!("Motor beendet mit {}: {}", output.status, tail.into_iter().rev().collect::<Vec<_>>().join(" | ")));
    }
    Ok(events)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Braucht die eingerichtete Umgebung engine/.venv.
    #[test]
    fn info_meldet_python_und_torch() {
        let events = run(&["info"]).expect("Motor läuft");
        let result = events.iter().find(|e| e["event"] == "result").expect("Ergebnis");
        assert!(result["python"].as_str().unwrap().starts_with("3.12"));
        assert!(result["torch"].as_str().unwrap().starts_with("2."));
    }

    #[test]
    fn fehler_des_motors_wird_zum_err() {
        let error = run(&["convert", "gibt-es-nicht.wav", "--model", "x.pth", "--out", "y.wav"]).unwrap_err();
        assert!(error.contains("Eingabe fehlt"), "{error}");
    }
}
