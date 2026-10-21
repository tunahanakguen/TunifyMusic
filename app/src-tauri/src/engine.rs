//! Startet den Python-Motor (engine/) und liest seine JSON-Zeilen.
//!
//! Zwei Arten:
//! - `run`: ein eigener Prozess je Befehl, für Kurzes wie `info` oder `instruments`.
//! - `run_job`: Umwandeln und Trainieren über den Motor im Dauerbetrieb (`serve`). Er lädt beim
//!   App-Start die Modelle und behält sie; eine Umwandlung dauert dann Sekunden statt einer halben Minute.
//!   Abbrechen beendet diesen Prozess, beim nächsten Auftrag startet er neu.

use std::collections::VecDeque;
use std::io::{BufRead, BufReader, Write};
use std::path::PathBuf;
use std::process::{Child, ChildStdin, Command, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{Receiver, channel};
use std::sync::{Arc, Mutex};

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

fn python() -> Result<PathBuf, String> {
    let python = engine_dir().join(".venv").join("Scripts").join("python.exe");
    if python.exists() {
        Ok(python)
    } else {
        Err(format!("Python-Motor nicht eingerichtet: {} fehlt", python.display()))
    }
}

fn command(args: &[&str]) -> Result<Command, String> {
    let mut command = Command::new(python()?);
    command
        .arg("-m")
        .arg("tunify_engine")
        .args(args)
        .current_dir(engine_dir())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    #[cfg(windows)]
    command.creation_flags(CREATE_NO_WINDOW);
    Ok(command)
}

/// Letzte Zeilen von stderr (für Fehlermeldungen); liest nebenher, damit der Motor nie blockiert.
fn drain_stderr(child: &mut Child) -> Arc<Mutex<VecDeque<String>>> {
    let tail = Arc::new(Mutex::new(VecDeque::new()));
    let stderr = child.stderr.take().expect("stderr umgeleitet");
    let keep = tail.clone();
    std::thread::spawn(move || {
        for line in BufReader::new(stderr).lines().map_while(Result::ok) {
            let mut tail = keep.lock().unwrap();
            if tail.len() == 5 {
                tail.pop_front();
            }
            tail.push_back(line);
        }
    });
    tail
}

fn tail_text(tail: &Arc<Mutex<VecDeque<String>>>) -> String {
    tail.lock().unwrap().iter().cloned().collect::<Vec<_>>().join(" | ")
}

fn first_error(events: &[serde_json::Value]) -> Option<String> {
    events
        .iter()
        .find(|e| e["event"] == "error")
        .map(|e| e["message"].as_str().unwrap_or("Unbekannter Fehler").to_string())
}

/// Führt einen Motor-Befehl in einem eigenen Prozess aus und gibt alle Ereignisse zurück.
/// Ein `error`-Ereignis wird zum Fehler.
pub fn run(args: &[&str]) -> Result<Vec<serde_json::Value>, String> {
    run_once(args, |_| {})
}

fn run_once(args: &[&str], mut on_event: impl FnMut(&serde_json::Value)) -> Result<Vec<serde_json::Value>, String> {
    let mut child = command(args)?.spawn().map_err(|e| format!("Motor startet nicht: {e}"))?;
    let tail = drain_stderr(&mut child);
    let mut events = Vec::new();
    for line in BufReader::new(child.stdout.take().expect("stdout umgeleitet")).lines().map_while(Result::ok) {
        if let Ok(event) = serde_json::from_str::<serde_json::Value>(&line) {
            on_event(&event);
            events.push(event);
        }
    }
    let status = child.wait().map_err(|e| format!("Motor nicht beendet: {e}"))?;
    if let Some(error) = first_error(&events) {
        return Err(error);
    }
    if !status.success() {
        return Err(format!("Motor beendet mit {status}: {}", tail_text(&tail)));
    }
    Ok(events)
}

// ---- Dauerbetrieb ----

struct Server {
    child: Child,
    stdin: ChildStdin,
    events: Receiver<serde_json::Value>,
    stderr: Arc<Mutex<VecDeque<String>>>,
}

static SERVER: Mutex<Option<Server>> = Mutex::new(None);
/// Prozess-ID des Motors, solange ein Auftrag läuft (es läuft immer höchstens einer); 0 = startet gerade.
static JOB: Mutex<Option<u32>> = Mutex::new(None);
static CANCELLED: AtomicBool = AtomicBool::new(false);

pub const CANCELLED_MESSAGE: &str = "Abgebrochen";
pub const BUSY_MESSAGE: &str = "Es läuft schon eine Umwandlung oder ein Training. Bitte warten oder dort abbrechen.";

fn spawn_server() -> Result<Server, String> {
    let mut child = command(&["serve"])?
        .stdin(Stdio::piped())
        .spawn()
        .map_err(|e| format!("Motor startet nicht: {e}"))?;
    let stderr = drain_stderr(&mut child);
    let stdin = child.stdin.take().expect("stdin umgeleitet");
    let stdout = child.stdout.take().expect("stdout umgeleitet");
    let (send, events) = channel();
    std::thread::spawn(move || {
        for line in BufReader::new(stdout).lines().map_while(Result::ok) {
            // Unterprozesse von Applio schreiben teils auf dieselbe Ausgabe: nur JSON zählt
            if let Ok(event) = serde_json::from_str::<serde_json::Value>(&line) {
                if send.send(event).is_err() {
                    break;
                }
            }
        }
    });
    Ok(Server { child, stdin, events, stderr })
}

/// Motor im Dauerbetrieb starten (falls nicht schon da), damit er beim ersten Auftrag fertig geladen ist.
pub fn start_server() -> Result<(), String> {
    let mut server = SERVER.lock().unwrap();
    let alive = server.as_mut().is_some_and(|s| matches!(s.child.try_wait(), Ok(None)));
    if !alive {
        *server = Some(spawn_server()?);
    }
    Ok(())
}

/// Umwandeln oder Trainieren über den Motor im Dauerbetrieb. Meldet jedes Ereignis sofort an `on_event`
/// (z. B. Fortschritt für die Oberfläche) und lässt sich mit `cancel` abbrechen (dann `Err(CANCELLED_MESSAGE)`).
pub fn run_job(args: &[&str], mut on_event: impl FnMut(&serde_json::Value)) -> Result<Vec<serde_json::Value>, String> {
    {
        // Immer nur ein Auftrag zur Zeit: teilt sich Grafikkarte und Abbruch-Knopf
        let mut running = JOB.lock().unwrap();
        if running.is_some() {
            return Err(BUSY_MESSAGE.to_string());
        }
        *running = Some(0);
        CANCELLED.store(false, Ordering::SeqCst);
    }
    let result = run_on_server(args, &mut on_event);
    *JOB.lock().unwrap() = None;
    if CANCELLED.swap(false, Ordering::SeqCst) {
        return Err(CANCELLED_MESSAGE.to_string());
    }
    result
}

fn run_on_server(
    args: &[&str],
    on_event: &mut impl FnMut(&serde_json::Value),
) -> Result<Vec<serde_json::Value>, String> {
    start_server()?;
    let mut guard = SERVER.lock().unwrap();
    let server = guard.as_mut().expect("eben gestartet");
    *JOB.lock().unwrap() = Some(server.child.id());
    if CANCELLED.load(Ordering::SeqCst) {
        return Err(CANCELLED_MESSAGE.to_string());
    }

    let request = serde_json::json!({ "args": args }).to_string() + "\n";
    let sent = server.stdin.write_all(request.as_bytes()).and_then(|_| server.stdin.flush());
    let mut events = Vec::new();
    let finished = sent.is_ok()
        && loop {
            match server.events.recv() {
                Ok(event) if event["event"] == "end" => break true,
                Ok(event) if event["event"] == "ready" => {}
                Ok(event) => {
                    on_event(&event);
                    events.push(event);
                }
                Err(_) => break false, // Prozess beendet (abgebrochen oder abgestürzt)
            }
        };
    if !finished {
        let reason = tail_text(&server.stderr);
        let _ = server.child.kill();
        let _ = server.child.wait();
        *guard = None;
        return Err(format!("Motor unerwartet beendet: {reason}"));
    }
    if let Some(error) = first_error(&events) {
        return Err(error);
    }
    Ok(events)
}

/// Bricht den laufenden Auftrag ab, samt Unterprozessen (FFmpeg, FluidSynth, Training).
/// Gibt `false` zurück, wenn gerade keiner läuft.
pub fn cancel() -> bool {
    let Some(pid) = *JOB.lock().unwrap() else { return false };
    CANCELLED.store(true, Ordering::SeqCst);
    if pid != 0 {
        let mut kill = Command::new("taskkill");
        kill.args(["/T", "/F", "/PID", &pid.to_string()]).stdout(Stdio::null()).stderr(Stdio::null());
        #[cfg(windows)]
        kill.creation_flags(CREATE_NO_WINDOW);
        let _ = kill.status();
    }
    true
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
        let events = run_once(&["info"], |_| seen += 1).expect("Motor läuft");
        assert_eq!(seen, events.len());
    }

    #[test]
    fn fehler_des_motors_wird_zum_err() {
        let error = run(&["convert", "gibt-es-nicht.wav", "--model", "x.pth", "--out", "y.wav"]).unwrap_err();
        assert!(error.contains("Eingabe fehlt"), "{error}");
    }

    /// Dauerbetrieb: Abbrechen, Besetzt, Fehler und Erfolg nacheinander.
    #[test]
    fn dauerbetrieb_abbrechen_besetzt_fehler_erfolg() {
        assert!(!cancel(), "ohne laufenden Auftrag gibt es nichts abzubrechen");

        // Der erste Auftrag wartet, bis der frische Motor seine Modelle geladen hat – Zeit zum Abbrechen
        let worker = std::thread::spawn(|| run_job(&["info"], |_| {}));
        let started = std::time::Instant::now();
        while !matches!(*JOB.lock().unwrap(), Some(p) if p != 0) {
            assert!(started.elapsed().as_secs() < 30, "Auftrag startet nicht");
            std::thread::sleep(std::time::Duration::from_millis(20));
        }
        assert_eq!(run_job(&["info"], |_| {}).unwrap_err(), BUSY_MESSAGE, "zweiter Auftrag muss warten");
        assert!(cancel());
        assert_eq!(worker.join().unwrap().unwrap_err(), CANCELLED_MESSAGE);

        // Danach startet der Motor neu; ein Fehler beendet nur den Auftrag, nicht den Motor
        let missing = std::env::temp_dir().join("tunify-gibt-es-nicht.wav");
        let error = run_job(&["convert", &missing.to_string_lossy(), "--model", "x.pth", "--out", "y.wav"], |_| {})
            .unwrap_err();
        assert!(error.contains("Eingabe fehlt"), "{error}");
        let pid = SERVER.lock().unwrap().as_ref().map(|s| s.child.id());

        let events = run_job(&["info"], |_| {}).expect("zweiter Auftrag auf demselben Motor");
        assert!(events.iter().any(|e| e["event"] == "result"));
        assert_eq!(SERVER.lock().unwrap().as_ref().map(|s| s.child.id()), pid, "Motor blieb derselbe");
    }
}
