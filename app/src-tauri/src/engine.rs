//! Startet den Python-Motor (engine/) und liest seine JSON-Zeilen.

use std::io::{BufRead, BufReader, Read};
use std::path::PathBuf;
use std::process::{Command, Stdio};
use std::sync::Mutex;
use std::sync::atomic::{AtomicBool, Ordering};

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

/// Prozess-ID der laufenden Umwandlung (es läuft immer höchstens eine), damit sie abgebrochen werden kann.
static JOB: Mutex<Option<u32>> = Mutex::new(None);
static CANCELLED: AtomicBool = AtomicBool::new(false);

pub const CANCELLED_MESSAGE: &str = "Abgebrochen";
pub const BUSY_MESSAGE: &str = "Es läuft schon eine Umwandlung oder ein Training. Bitte warten oder dort abbrechen.";

/// Führt einen Motor-Befehl aus und gibt alle Ereignisse zurück.
/// Ein `error`-Ereignis wird zum Fehler.
pub fn run(args: &[&str]) -> Result<Vec<serde_json::Value>, String> {
    run_inner(args, |_| {}, false)
}

/// Wie `run`, meldet aber jedes Ereignis sofort an `on_event` (z. B. Fortschritt für die Oberfläche)
/// und lässt sich mit `cancel` abbrechen (dann `Err(CANCELLED_MESSAGE)`).
pub fn run_job(args: &[&str], on_event: impl FnMut(&serde_json::Value)) -> Result<Vec<serde_json::Value>, String> {
    run_inner(args, on_event, true)
}

/// Bricht die laufende Umwandlung ab, samt ihren Unterprozessen (FFmpeg, FluidSynth).
/// Gibt `false` zurück, wenn gerade keine läuft.
pub fn cancel() -> bool {
    let Some(pid) = *JOB.lock().unwrap() else { return false };
    CANCELLED.store(true, Ordering::SeqCst);
    let mut kill = Command::new("taskkill");
    kill.args(["/T", "/F", "/PID", &pid.to_string()]).stdout(Stdio::null()).stderr(Stdio::null());
    #[cfg(windows)]
    kill.creation_flags(CREATE_NO_WINDOW);
    let _ = kill.status();
    true
}

fn run_inner(
    args: &[&str],
    mut on_event: impl FnMut(&serde_json::Value),
    job: bool,
) -> Result<Vec<serde_json::Value>, String> {
    let python = python();
    if !python.exists() {
        return Err(format!("Python-Motor nicht eingerichtet: {} fehlt", python.display()));
    }
    let mut command = Command::new(&python);
    command
        .arg("-m")
        .arg("tunify_engine")
        .args(args)
        .current_dir(engine_dir())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    #[cfg(windows)]
    command.creation_flags(CREATE_NO_WINDOW);

    // Immer nur eine Umwandlung oder ein Training zur Zeit: teilt sich Grafikkarte und Abbruch-Knopf
    let mut running = JOB.lock().unwrap();
    if job && running.is_some() {
        return Err(BUSY_MESSAGE.to_string());
    }
    let mut child = command.spawn().map_err(|e| format!("Motor startet nicht: {e}"))?;
    if job {
        CANCELLED.store(false, Ordering::SeqCst);
        *running = Some(child.id());
    }
    drop(running);
    // stderr nebenher leeren, sonst blockiert der Motor bei viel Bibliotheks-Ausgabe
    let mut stderr = child.stderr.take().expect("stderr umgeleitet");
    let stderr_reader = std::thread::spawn(move || {
        let mut text = String::new();
        let _ = stderr.read_to_string(&mut text);
        text
    });

    let mut events = Vec::new();
    for line in BufReader::new(child.stdout.take().expect("stdout umgeleitet")).lines() {
        let Ok(line) = line else { break };
        if let Ok(event) = serde_json::from_str::<serde_json::Value>(&line) {
            on_event(&event);
            events.push(event);
        }
    }
    let status = child.wait().map_err(|e| format!("Motor nicht beendet: {e}"))?;
    let stderr = stderr_reader.join().unwrap_or_default();
    if job {
        *JOB.lock().unwrap() = None;
        if CANCELLED.swap(false, Ordering::SeqCst) {
            return Err(CANCELLED_MESSAGE.to_string());
        }
    }

    if let Some(error) = events.iter().find(|e| e["event"] == "error") {
        return Err(error["message"].as_str().unwrap_or("Unbekannter Fehler").to_string());
    }
    if !status.success() {
        let tail: Vec<&str> = stderr.lines().rev().take(5).collect();
        return Err(format!("Motor beendet mit {status}: {}", tail.into_iter().rev().collect::<Vec<_>>().join(" | ")));
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
    fn ereignisse_kommen_einzeln_an() {
        let mut seen = 0;
        // nicht als Job, sonst kommt er dem parallel laufenden Abbruch-Test in die Quere
        let events = run_inner(&["info"], |_| seen += 1, false).expect("Motor läuft");
        assert_eq!(seen, events.len());
    }

    #[test]
    fn abbrechen_beendet_den_motor() {
        assert!(!cancel(), "ohne laufende Umwandlung gibt es nichts abzubrechen");
        let worker = std::thread::spawn(|| {
            // Bibliotheken laden dauert einige Sekunden – genug Zeit zum Abbrechen
            run_job(&["prepare"], |_| {})
        });
        let started = std::time::Instant::now();
        while JOB.lock().unwrap().is_none() {
            assert!(started.elapsed().as_secs() < 10, "Motor startet nicht");
            std::thread::sleep(std::time::Duration::from_millis(20));
        }
        assert_eq!(run_job(&["info"], |_| {}).unwrap_err(), BUSY_MESSAGE, "zweiter Auftrag muss warten");
        assert!(cancel());
        assert_eq!(worker.join().unwrap().unwrap_err(), CANCELLED_MESSAGE);
    }

    #[test]
    fn fehler_des_motors_wird_zum_err() {
        let error = run(&["convert", "gibt-es-nicht.wav", "--model", "x.pth", "--out", "y.wav"]).unwrap_err();
        assert!(error.contains("Eingabe fehlt"), "{error}");
    }
}
