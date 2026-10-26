//! Einrichtung beim ersten Start: kopiert den Motor-Quellcode in den Datenordner und führt `setup.ps1` aus,
//! das Python, Pakete und Modelle lädt.

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
const SOURCES: [&str; 3] = ["tunify_engine", "requirements.lock.txt", "setup.ps1"];

#[derive(serde::Serialize)]
pub struct Status {
    /// Umwandeln, Trennen, Bereinigen, Trainieren, Noten-Modus
    pub core: bool,
    pub folder: String,
}

pub fn status() -> Status {
    let engine = engine::engine_dir();
    let root = engine::data_root();
    let installed = engine == root.join("engine");
    let core = if installed {
        root.join("ready-core.txt").exists()
    } else {
        engine.join(".venv").join("Scripts").join("python.exe").exists()
    };
    Status { core, folder: root.to_string_lossy().to_string() }
}

/// Grafikkarte, bevor etwas geladen wird: Der Motor braucht eine NVIDIA-Karte mit CUDA.
#[derive(serde::Serialize, Debug, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Gpu {
    pub name: Option<String>,
    pub vram_gb: f32,
    pub driver: String,
    /// Grund, warum Tunify hier kaum brauchbar läuft (keine Karte, zu wenig Speicher, alter Treiber)
    pub problem: Option<String>,
}

/// CUDA 12.8 (PyTorch cu128) läuft ab diesem Windows-Treiber
const MIN_DRIVER: f32 = 528.33;
const MIN_VRAM_GB: f32 = 4.0;

pub fn gpu() -> Gpu {
    let mut command = Command::new("nvidia-smi");
    command.args(["--query-gpu=name,memory.total,driver_version", "--format=csv,noheader,nounits"]);
    #[cfg(windows)]
    command.creation_flags(CREATE_NO_WINDOW);
    let output = command.output().ok().filter(|o| o.status.success());
    parse_gpu(&output.map(|o| String::from_utf8_lossy(&o.stdout).to_string()).unwrap_or_default())
}

/// Erste Zeile von `nvidia-smi --query-gpu=name,memory.total,driver_version` (MiB) auswerten.
fn parse_gpu(text: &str) -> Gpu {
    let fields: Vec<&str> = text.lines().next().unwrap_or("").split(',').map(str::trim).collect();
    let [name, mib, driver] = fields[..] else {
        return Gpu {
            name: None,
            vram_gb: 0.0,
            driver: String::new(),
            problem: Some(
                "Keine NVIDIA-Grafikkarte gefunden. Tunify Music braucht eine NVIDIA-Karte mit mindestens 4 GB \
                 Grafikspeicher und aktuellem Treiber – ohne läuft der KI-Motor nur sehr langsam oder gar nicht."
                    .into(),
            ),
        };
    };
    let vram_gb = (mib.parse::<f32>().unwrap_or(0.0) / 1024.0 * 10.0).round() / 10.0;
    let driver_number = driver.split('.').take(2).collect::<Vec<_>>().join(".").parse::<f32>().unwrap_or(0.0);
    let problem = if vram_gb < MIN_VRAM_GB {
        Some(format!("{name} hat nur {vram_gb} GB Grafikspeicher; Tunify Music braucht mindestens 4 GB."))
    } else if driver_number < MIN_DRIVER {
        Some(format!(
            "Der Grafiktreiber ({driver}) ist zu alt. Bitte zuerst den aktuellen NVIDIA-Treiber installieren \
             (mindestens {MIN_DRIVER})."
        ))
    } else {
        None
    };
    Gpu { name: Some(name.to_string()), vram_gb, driver: driver.to_string(), problem }
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
pub fn run(sources: &Path, mut on_progress: impl FnMut(u32, &str)) -> Result<(), String> {
    {
        let mut running = RUNNING.lock().unwrap();
        if running.is_some() {
            return Err("Die Einrichtung läuft schon".into());
        }
        *running = Some(0);
    }
    let result = run_script(sources, &mut on_progress);
    *RUNNING.lock().unwrap() = None;
    result
}

fn run_script(sources: &Path, on_progress: &mut impl FnMut(u32, &str)) -> Result<(), String> {
    install_sources(sources)?;
    let root = engine::data_root();
    let script = root.join("engine").join("setup.ps1");
    let mut command = Command::new("powershell.exe");
    command
        .args(["-NoProfile", "-NonInteractive", "-ExecutionPolicy", "Bypass", "-File"])
        .arg(&script)
        .arg("-Root")
        .arg(&root)
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
    use super::{error_text, parse_gpu};

    #[test]
    fn grafikkarte_bewerten() {
        let ok = parse_gpu("NVIDIA GeForce RTX 5070 Laptop GPU, 8151, 595.97\n");
        assert_eq!(ok.name.as_deref(), Some("NVIDIA GeForce RTX 5070 Laptop GPU"));
        assert_eq!(ok.vram_gb, 8.0);
        assert!(ok.problem.is_none());
        assert!(parse_gpu("NVIDIA GeForce GTX 1060, 6144, 560.94").problem.is_none());
        assert!(parse_gpu("NVIDIA GeForce GT 1030, 2048, 560.94").problem.is_some());
        assert!(parse_gpu("NVIDIA GeForce RTX 3060, 12288, 472.12").problem.unwrap().contains("Treiber"));
        assert!(parse_gpu("").problem.unwrap().contains("Keine NVIDIA"));
    }

    #[test]
    fn fehlertext_mit_werkzeugmeldung() {
        let lines = ["error: Netz weg".to_string(), "@@! Fehlgeschlagen (2): uv".to_string()];
        assert_eq!(error_text(&lines), "Fehlgeschlagen (2): uv – error: Netz weg");
        assert_eq!(error_text(&lines[1..]), "Fehlgeschlagen (2): uv");
        assert_eq!(error_text(&lines[..1]), "error: Netz weg");
    }
}
