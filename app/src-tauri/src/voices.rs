//! Stimmen-Bibliothek: RVC-Modelle (.pth, optional .index) im App-Datenordner.
//!
//! Jede Stimme liegt in `voices/<id>/` mit `model.pth`, optional `model.index` und `voice.json`
//! (Anzeigename, Erstellzeit). Importiert werden einzelne Dateien oder ein ZIP-Archiv.

use std::path::{Path, PathBuf};

#[derive(serde::Serialize, serde::Deserialize)]
struct Meta {
    name: String,
    created: u64,
    has_index: bool,
}

#[derive(serde::Serialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct Voice {
    pub id: String,
    pub name: String,
    pub model: String,
    pub index: Option<String>,
    /// Sekunden seit 1970
    pub created: u64,
}

const MODEL: &str = "model.pth";
const INDEX: &str = "model.index";
const META: &str = "voice.json";
/// Kleiner ist kein RVC-Modell, größer eher ein Trainings-Zwischenstand (G_/D_)
const MIN_MODEL_BYTES: u64 = 1_000_000;
const MAX_MODEL_BYTES: u64 = 600_000_000;

fn now() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_secs())
}

/// Ordnername aus dem Anzeigenamen: „Meine Stimme“ → „meine-stimme“
pub fn slug(name: &str) -> String {
    let mut out = String::new();
    for c in name.to_lowercase().chars() {
        match c {
            'ä' => out.push_str("ae"),
            'ö' => out.push_str("oe"),
            'ü' => out.push_str("ue"),
            'ß' => out.push_str("ss"),
            c if c.is_ascii_alphanumeric() => out.push(c),
            _ if !out.ends_with('-') => out.push('-'),
            _ => {}
        }
    }
    let out = out.trim_matches('-').chars().take(40).collect::<String>();
    if out.is_empty() { "stimme".into() } else { out }
}

fn valid_id(id: &str) -> bool {
    !id.is_empty() && id.chars().all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-')
}

fn read(dir: &Path, id: &str) -> Option<Voice> {
    let folder = dir.join(id);
    let meta: Meta = serde_json::from_str(&std::fs::read_to_string(folder.join(META)).ok()?).ok()?;
    let model = folder.join(MODEL);
    if !model.exists() {
        return None;
    }
    let index = folder.join(INDEX);
    Some(Voice {
        id: id.to_string(),
        name: meta.name,
        model: model.to_string_lossy().to_string(),
        index: (meta.has_index && index.exists()).then(|| index.to_string_lossy().to_string()),
        created: meta.created,
    })
}

/// Alle Stimmen, neueste zuerst.
pub fn list(dir: &Path) -> Vec<Voice> {
    let Ok(entries) = std::fs::read_dir(dir) else { return Vec::new() };
    let mut voices: Vec<Voice> = entries
        .flatten()
        .filter_map(|e| read(dir, &e.file_name().to_string_lossy()))
        .collect();
    voices.sort_by(|a, b| b.created.cmp(&a.created));
    voices
}

pub fn get(dir: &Path, id: &str) -> Result<Voice, String> {
    if !valid_id(id) {
        return Err(format!("Ungültige Stimme: {id}"));
    }
    read(dir, id).ok_or_else(|| format!("Stimme nicht gefunden: {id}"))
}

fn is_model(path: &Path) -> bool {
    let name = path.file_name().and_then(|n| n.to_str()).unwrap_or("");
    let is_pth = path.extension().and_then(|e| e.to_str()).is_some_and(|e| e.eq_ignore_ascii_case("pth"));
    // G_…/D_… sind Trainings-Zwischenstände von Applio, keine fertigen Stimmen
    is_pth && !name.starts_with("G_") && !name.starts_with("D_")
}

fn is_index(path: &Path) -> bool {
    path.extension().and_then(|e| e.to_str()).is_some_and(|e| e.eq_ignore_ascii_case("index"))
}

fn files_in(dir: &Path, out: &mut Vec<PathBuf>) {
    let Ok(entries) = std::fs::read_dir(dir) else { return };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            files_in(&path, out);
        } else {
            out.push(path);
        }
    }
}

/// ZIP mit dem in Windows eingebauten tar entpacken.
fn unzip(zip: &Path, into: &Path) -> Result<(), String> {
    std::fs::create_dir_all(into).map_err(|e| e.to_string())?;
    let mut tar = std::process::Command::new("tar");
    tar.arg("-xf").arg(zip).arg("-C").arg(into);
    #[cfg(windows)]
    std::os::windows::process::CommandExt::creation_flags(&mut tar, 0x0800_0000);
    let status = tar.status().map_err(|e| format!("Entpacken nicht möglich: {e}"))?;
    if !status.success() {
        return Err(format!("Archiv lässt sich nicht entpacken: {}", zip.display()));
    }
    Ok(())
}

/// Importiert eine Stimme aus .pth (+ .index) oder einem ZIP mit beidem.
pub fn import(dir: &Path, name: &str, files: &[String]) -> Result<Voice, String> {
    let name = name.trim();
    if name.is_empty() {
        return Err("Bitte einen Namen für die Stimme angeben".into());
    }
    let unpack = std::env::temp_dir().join(format!("tunify-import-{}-{}", std::process::id(), now()));
    let mut candidates = Vec::new();
    for file in files {
        let path = PathBuf::from(file);
        if path.extension().and_then(|e| e.to_str()).is_some_and(|e| e.eq_ignore_ascii_case("zip")) {
            let target = unpack.join(candidates.len().to_string());
            unzip(&path, &target)?;
            files_in(&target, &mut candidates);
        } else {
            candidates.push(path);
        }
    }
    let result = import_files(dir, name, &candidates);
    let _ = std::fs::remove_dir_all(&unpack);
    result
}

fn import_files(dir: &Path, name: &str, candidates: &[PathBuf]) -> Result<Voice, String> {
    let models: Vec<&PathBuf> = candidates.iter().filter(|p| is_model(p)).collect();
    let model = match models.as_slice() {
        [one] => *one,
        [] => return Err("Keine Modelldatei (.pth) gefunden".into()),
        _ => return Err("Mehrere Modelldateien (.pth) gefunden – bitte nur eine auswählen".into()),
    };
    let bytes = std::fs::metadata(model).map_err(|e| e.to_string())?.len();
    if !(MIN_MODEL_BYTES..=MAX_MODEL_BYTES).contains(&bytes) {
        return Err(format!("{} sieht nicht wie ein RVC-Modell aus ({} MB)", model.display(), bytes / 1_000_000));
    }
    // Applio/RVC nennt den passenden Index „added_…“; sonst den ersten nehmen
    let mut indexes: Vec<&PathBuf> = candidates.iter().filter(|p| is_index(p)).collect();
    indexes.sort_by_key(|p| !p.file_name().and_then(|n| n.to_str()).unwrap_or("").starts_with("added_"));
    let index = indexes.first();

    let base = slug(name);
    let mut id = base.clone();
    let mut n = 2;
    while dir.join(&id).exists() {
        id = format!("{base}-{n}");
        n += 1;
    }
    let folder = dir.join(&id);
    std::fs::create_dir_all(&folder).map_err(|e| format!("Stimmen-Ordner nicht anlegbar: {e}"))?;
    let copy = || -> Result<(), String> {
        std::fs::copy(model, folder.join(MODEL)).map_err(|e| format!("Modell nicht kopiert: {e}"))?;
        if let Some(index) = index {
            std::fs::copy(index, folder.join(INDEX)).map_err(|e| format!("Index nicht kopiert: {e}"))?;
        }
        let meta = Meta { name: name.to_string(), created: now(), has_index: index.is_some() };
        std::fs::write(folder.join(META), serde_json::to_string_pretty(&meta).unwrap()).map_err(|e| e.to_string())
    };
    if let Err(e) = copy() {
        let _ = std::fs::remove_dir_all(&folder);
        return Err(e);
    }
    get(dir, &id)
}

pub fn rename(dir: &Path, id: &str, name: &str) -> Result<Voice, String> {
    let voice = get(dir, id)?;
    let name = name.trim();
    if name.is_empty() {
        return Err("Der Name darf nicht leer sein".into());
    }
    let meta = Meta { name: name.to_string(), created: voice.created, has_index: voice.index.is_some() };
    std::fs::write(dir.join(id).join(META), serde_json::to_string_pretty(&meta).unwrap()).map_err(|e| e.to_string())?;
    get(dir, id)
}

/// Löscht die Kopie in der Bibliothek (nicht die ursprünglich importierten Dateien).
pub fn delete(dir: &Path, id: &str) -> Result<(), String> {
    get(dir, id)?;
    std::fs::remove_dir_all(dir.join(id)).map_err(|e| format!("Stimme nicht gelöscht: {e}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("tunify-voices-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    fn fake_file(path: &Path, bytes: usize) {
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(path, vec![7u8; bytes]).unwrap();
    }

    #[test]
    fn slug_macht_ordnernamen() {
        assert_eq!(slug("Meine Stimme"), "meine-stimme");
        assert_eq!(slug("  Jürgen's Größe!! "), "juergen-s-groesse");
        assert_eq!(slug("???"), "stimme");
    }

    #[test]
    fn import_umbenennen_loeschen() {
        let src = temp("src");
        let lib = temp("lib");
        fake_file(&src.join("sänger_200e.pth"), 2_000_000);
        fake_file(&src.join("trained_x.index"), 1000);
        fake_file(&src.join("added_x.index"), 1000);
        let files = ["sänger_200e.pth", "trained_x.index", "added_x.index"].map(|f| src.join(f).to_string_lossy().to_string());

        let voice = import(&lib, "Mein Sänger", &files).unwrap();
        assert_eq!(voice.id, "mein-saenger");
        assert!(voice.index.is_some());
        // gleicher Name → eigener Ordner
        assert_eq!(import(&lib, "Mein Sänger", &files[..1]).unwrap().id, "mein-saenger-2");
        assert_eq!(list(&lib).len(), 2);

        assert_eq!(rename(&lib, "mein-saenger", "Anderer Name").unwrap().name, "Anderer Name");
        delete(&lib, "mein-saenger").unwrap();
        assert_eq!(list(&lib).len(), 1);
        assert!(src.join("sänger_200e.pth").exists(), "Original bleibt erhalten");
        assert!(delete(&lib, "../src").is_err(), "nur Ordner der Bibliothek");

        std::fs::remove_dir_all(&src).unwrap();
        std::fs::remove_dir_all(&lib).unwrap();
    }

    #[test]
    fn import_aus_zip_und_fehlerfaelle() {
        let src = temp("zip");
        let lib = temp("ziplib");
        fake_file(&src.join("pack/model_e300.pth"), 2_000_000);
        fake_file(&src.join("pack/G_2333.pth"), 2_000_000);
        fake_file(&src.join("pack/added_ivf.index"), 1000);
        let zip = src.join("stimme.zip");
        let made = std::process::Command::new("tar")
            .args(["-a", "-cf"])
            .arg(&zip)
            .arg("-C")
            .arg(&src)
            .arg("pack")
            .status()
            .unwrap();
        assert!(made.success());

        let voice = import(&lib, "Aus Zip", &[zip.to_string_lossy().to_string()]).unwrap();
        assert!(voice.index.is_some(), "Index aus dem Archiv übernommen, G_ ignoriert");

        fake_file(&src.join("klein.pth"), 10);
        assert!(import(&lib, "Klein", &[src.join("klein.pth").to_string_lossy().to_string()]).is_err());
        assert!(import(&lib, "Nichts", &[src.join("pack/added_ivf.index").to_string_lossy().to_string()]).is_err());

        std::fs::remove_dir_all(&src).unwrap();
        std::fs::remove_dir_all(&lib).unwrap();
    }
}
