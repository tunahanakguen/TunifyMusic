//! Einrichtung beim ersten Start: kopiert den Motor-Quellcode in den Datenordner und führt `setup.ps1` aus,
//! das Python, Pakete und Modelle lädt. Zwei Teile: `core` (alles außer Musik erzeugen) und `music` (ACE-Step).

use std::collections::VecDeque;
use std::io::{BufRead, BufReader};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::Mutex;

#[cfg(windows)]
use std::os::windows::process::CommandExt;

use crate::engine;

#[cfg(windows)]
const CREATE_NO_WINDOW: u32 = 0x0800_0000;

/// Prozess-ID der laufenden Einrichtung (für Abbrechen)
static RUNNING: Mutex<Option<u32>> = Mutex::new(None);

/// Quelldateien des Motors, die mit der App kommen (Rest lädt `setup.ps1`)
const SOURCES: [&str; 5] = ["tunify_engine", "acestep_worker.py", "acestep_download.py", "requirements.lock.txt", "setup.ps1"];

#[derive(serde::Serialize)]
pub struct Status {
    /// Umwandeln, Trainieren, Noten-Modus
    pub core: bool,
    /// Lied und Instrumental erzeugen
    pub music: bool,
    pub folder: String,
}

pub fn status() -> Status {
    let engine = engine::engine_dir();
    let root = engine::data_root();
    let installed = engine == root.join("engine");
    let models = engine.parent().unwrap_or(&engine).join("models");
    let core = if installed {
        root.join("ready-core.txt").exists()
    } else {
        engine.join(".venv").join("Scripts").join("python.exe").exists()
    };
    let music = if installed {
        root.join("ready-music.txt").exists()
    } else {
        engine.join("vendor").join("ace-step").join(".venv").join("Scripts").join("python.exe").exists()
            && models.join("ace-step").join("acestep-5Hz-lm-0.6B").exists()
    };
    Status { core, music, folder: root.to_string_lossy().to_string() }
}

fn copy_dir(from: &Path, to: &Path) -> std::io::Result<()> {
    std::fs::create_dir_all(to)?;
    for entry in std::fs::read_dir(from)? {
        let entry = entry?;
        let name = entry.file_name();
        if name == "__pycache__" {
            continue;
        }
        if entry.file_type()?.is_dir() {
            copy_dir(&entry.path(), &to.join(&name))?;
        } else {
            std::fs::copy(entry.path(), to.join(&name))?;
        }
    }
    Ok(())
}

/// Motor-Quellcode aus `sources` (Ressourcen der App oder Repo) in `<Datenordner>\engine` kopieren.
/// Bei jedem App-Start, damit nach einem Update der neue Code läuft.
pub fn install_sources(sources: &Path) -> Result<(), String> {
    let target = engine::data_root().join("engine");
    if !sources.join("setup.ps1").exists() || sources == target {
        return Err(format!("Motor-Quellcode fehlt in {}", sources.display()));
    }
    for name in SOURCES {
        let from = sources.join(name);
        let to = target.join(name);
        let result = if from.is_dir() {
            copy_dir(&from, &to)
        } else {
            std::fs::create_dir_all(&target).and_then(|_| std::fs::copy(&from, &to).map(|_| ()))
        };
        result.map_err(|e| format!("Kopieren von {name} fehlgeschlagen: {e}"))?;
    }
    Ok(())
}

/// Ressourcen der installierten App, sonst (Entwicklung) der Motor im Repo
pub fn sources_dir(resource_dir: Option<PathBuf>) -> PathBuf {
    resource_dir
        .map(|r| r.join("engine"))
        .filter(|r| r.join("setup.ps1").exists())
        .unwrap_or_else(engine::repo_engine)
}

/// Führt `setup.ps1` aus. Fortschritt („@@ <Prozent> <Text>“) geht an `on_progress`.
pub fn run(part: &str, sources: &Path, mut on_progress: impl FnMut(u32, &str)) -> Result<(), String> {
    if !["core", "music"].contains(&part) {
        return Err(format!("Unbekannter Teil: {part}"));
    }
    {
        let mut running = RUNNING.lock().unwrap();
        if running.is_some() {
            return Err("Die Einrichtung läuft schon".into());
        }
        *running = Some(0);
    }
    let result = run_script(part, sources, &mut on_progress);
    *RUNNING.lock().unwrap() = None;
    result
}

fn run_script(part: &str, sources: &Path, on_progress: &mut impl FnMut(u32, &str)) -> Result<(), String> {
    install_sources(sources)?;
    let root = engine::data_root();
    let script = root.join("engine").join("setup.ps1");
    let mut command = Command::new("powershell.exe");
    command
        .args(["-NoProfile", "-NonInteractive", "-ExecutionPolicy", "Bypass", "-File"])
        .arg(&script)
        .arg("-Root")
        .arg(&root)
        .args(["-Part", part])
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .stdin(Stdio::null());
    #[cfg(windows)]
    command.creation_flags(CREATE_NO_WINDOW);
    let mut child = command.spawn().map_err(|e| format!("Einrichtung startet nicht: {e}"))?;
    *RUNNING.lock().unwrap() = Some(child.id());

    // stderr nebenher lesen (Fehlermeldungen), damit nichts blockiert
    let stderr = child.stderr.take().expect("stderr umgeleitet");
    let errors = std::thread::spawn(move || {
        let mut tail = VecDeque::new();
        for line in BufReader::new(stderr).lines().map_while(Result::ok) {
            let line = line.trim().to_string();
            if line.is_empty() {
                continue;
            }
            if tail.len() == 6 {
                tail.pop_front();
            }
            tail.push_back(line);
        }
        error_text(&tail.into_iter().collect::<Vec<_>>())
    });
    for line in BufReader::new(child.stdout.take().expect("stdout umgeleitet")).lines().map_while(Result::ok) {
        if let Some((percent, text)) = line.strip_prefix("@@ ").and_then(|rest| rest.split_once(' ')) {
            on_progress(percent.parse().unwrap_or(0), text);
        }
    }
    let status = child.wait().map_err(|e| e.to_string())?;
    let errors = errors.join().unwrap_or_default();
    if CANCELLED.swap(false, std::sync::atomic::Ordering::SeqCst) {
        return Err(engine::CANCELLED_MESSAGE.into());
    }
    if status.success() {
        Ok(())
    } else if errors.is_empty() {
        Err(format!("Einrichtung fehlgeschlagen (Code {:?})", status.code()))
    } else {
        Err(format!("Einrichtung fehlgeschlagen: {errors}"))
    }
}

/// Fehlertext aus den letzten stderr-Zeilen: die Meldung des Skripts („@@! …“) und davor die des Werkzeugs.
fn error_text(lines: &[String]) -> String {
    match lines.iter().rposition(|line| line.starts_with("@@! ")) {
        Some(at) => {
            let message = &lines[at][4..];
            match at.checked_sub(1).map(|before| &lines[before]) {
                Some(tool) if !tool.starts_with("@@! ") => format!("{message} – {tool}"),
                _ => message.to_string(),
            }
        }
        None => lines.join(" | "),
    }
}

static CANCELLED: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

/// Bricht die Einrichtung ab. Fertige Schritte bleiben, beim nächsten Start geht es dort weiter.
pub fn cancel() -> bool {
    let Some(pid) = *RUNNING.lock().unwrap() else { return false };
    if pid == 0 {
        return false;
    }
    CANCELLED.store(true, std::sync::atomic::Ordering::SeqCst);
    let mut kill = Command::new("taskkill");
    kill.args(["/T", "/F", "/PID", &pid.to_string()]).stdout(Stdio::null()).stderr(Stdio::null());
    #[cfg(windows)]
    kill.creation_flags(CREATE_NO_WINDOW);
    let _ = kill.status();
    true
}

#[cfg(test)]
mod tests {
    use super::error_text;

    #[test]
    fn fehlertext_mit_werkzeugmeldung() {
        let lines = ["error: Netz weg".to_string(), "@@! Fehlgeschlagen (2): uv".to_string()];
        assert_eq!(error_text(&lines), "Fehlgeschlagen (2): uv – error: Netz weg");
        assert_eq!(error_text(&lines[1..]), "Fehlgeschlagen (2): uv");
        assert_eq!(error_text(&lines[..1]), "error: Netz weg");
    }
}
