//! Verlauf: Ergebnisse in `Musik\Tunify Music`, Aufnahmen in `…\Aufnahmen`.
//!
//! Favoriten stehen als Dateinamen (relativ zum Ausgabeordner) in `favorites.json` im App-Datenordner.
//! Löschen verschiebt in den Papierkorb und ist nur innerhalb des Ausgabeordners erlaubt.

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

pub const AUDIO_EXTENSIONS: [&str; 4] = ["wav", "mp3", "flac", "webm"];
const RECORDINGS: &str = "Aufnahmen";

/// Ein Eintrag im Verlauf
#[derive(serde::Serialize, Debug)]
pub struct Item {
    pub path: String,
    pub name: String,
    pub recording: bool,
    pub favorite: bool,
    /// Sekunden seit 1970
    pub modified: u64,
    pub bytes: u64,
}

fn load_favorites(file: &Path) -> BTreeSet<String> {
    std::fs::read_to_string(file).ok().and_then(|s| serde_json::from_str(&s).ok()).unwrap_or_default()
}

fn save_favorites(file: &Path, favorites: &BTreeSet<String>) -> Result<(), String> {
    let json = serde_json::to_string_pretty(favorites).map_err(|e| e.to_string())?;
    std::fs::write(file, json).map_err(|e| format!("Favoriten nicht speicherbar: {e}"))
}

/// Schlüssel einer Datei im Ausgabeordner („Lied (Geige).mp3“ oder „Aufnahmen/Aufnahme 1.wav“).
/// Fehler, wenn die Datei nicht direkt im Ausgabeordner oder in „Aufnahmen“ liegt.
fn key(dir: &Path, path: &Path) -> Result<String, String> {
    let outside = || "Datei liegt nicht im Ordner von Tunify Music".to_string();
    let dir = dir.canonicalize().map_err(|_| outside())?;
    let path = path.canonicalize().map_err(|_| "Datei nicht gefunden".to_string())?;
    let relative = path.strip_prefix(&dir).map_err(|_| outside())?;
    let parts: Vec<_> = relative.iter().map(|p| p.to_string_lossy().to_string()).collect();
    match parts.as_slice() {
        [name] => Ok(name.clone()),
        [folder, name] if folder == RECORDINGS => Ok(format!("{RECORDINGS}/{name}")),
        _ => Err(outside()),
    }
}

/// Alle Ergebnisse und Aufnahmen, neueste zuerst.
pub fn list(dir: &Path, favorites_file: &Path) -> Vec<Item> {
    let favorites = load_favorites(favorites_file);
    let mut items = Vec::new();
    for (folder, recording) in [(dir.to_path_buf(), false), (dir.join(RECORDINGS), true)] {
        let Ok(entries) = std::fs::read_dir(&folder) else { continue };
        for entry in entries.flatten() {
            let path = entry.path();
            let extension = path.extension().and_then(|e| e.to_str()).unwrap_or("").to_lowercase();
            let Ok(meta) = entry.metadata() else { continue };
            if !meta.is_file() || !AUDIO_EXTENSIONS.contains(&extension.as_str()) {
                continue;
            }
            let name = path.file_name().unwrap_or_default().to_string_lossy().to_string();
            let key = if recording { format!("{RECORDINGS}/{name}") } else { name.clone() };
            let modified = meta
                .modified()
                .ok()
                .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
                .map_or(0, |d| d.as_secs());
            items.push(Item {
                favorite: favorites.contains(&key),
                name,
                path: path.to_string_lossy().to_string(),
                recording,
                modified,
                bytes: meta.len(),
            });
        }
    }
    items.sort_by(|a, b| b.modified.cmp(&a.modified));
    items
}

pub fn set_favorite(dir: &Path, favorites_file: &Path, path: &str, favorite: bool) -> Result<(), String> {
    let key = key(dir, Path::new(path))?;
    let mut favorites = load_favorites(favorites_file);
    if favorite {
        favorites.insert(key);
    } else {
        favorites.remove(&key);
    }
    save_favorites(favorites_file, &favorites)
}

/// Verschiebt die Datei (und eine gleichnamige .mid daneben) in den Papierkorb.
pub fn delete(dir: &Path, favorites_file: &Path, path: &str) -> Result<(), String> {
    remove(dir, favorites_file, path, |files| {
        trash::delete_all(files).map_err(|e| format!("Konnte nicht in den Papierkorb verschieben: {e}"))
    })
}

fn remove(
    dir: &Path,
    favorites_file: &Path,
    path: &str,
    discard: impl Fn(&[PathBuf]) -> Result<(), String>,
) -> Result<(), String> {
    let path = PathBuf::from(path);
    let key = key(dir, &path)?;
    let midi = path.with_extension("mid");
    let mut files = vec![path];
    if midi.is_file() && !files.contains(&midi) {
        files.push(midi);
    }
    discard(&files)?;
    let mut favorites = load_favorites(favorites_file);
    if favorites.remove(&key) {
        save_favorites(favorites_file, &favorites)?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("tunify-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(dir.join(RECORDINGS)).unwrap();
        dir
    }

    #[test]
    fn favoriten_bleiben_gespeichert() {
        let dir = temp("fav");
        let favorites = dir.join("favorites.json");
        std::fs::write(dir.join("Lied (Geige).mp3"), b"x").unwrap();
        std::fs::write(dir.join(RECORDINGS).join("Aufnahme 1.wav"), b"x").unwrap();
        std::fs::write(dir.join("notiz.txt"), b"x").unwrap();

        let song = dir.join("Lied (Geige).mp3").to_string_lossy().to_string();
        let recording = dir.join(RECORDINGS).join("Aufnahme 1.wav").to_string_lossy().to_string();
        set_favorite(&dir, &favorites, &song, true).unwrap();
        set_favorite(&dir, &favorites, &recording, true).unwrap();
        set_favorite(&dir, &favorites, &recording, false).unwrap();

        let items = list(&dir, &favorites);
        assert_eq!(items.len(), 2, "nur Audiodateien");
        assert!(items.iter().find(|i| i.name == "Lied (Geige).mp3").unwrap().favorite);
        assert!(!items.iter().find(|i| i.name == "Aufnahme 1.wav").unwrap().favorite);
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn nichts_ausserhalb_des_ordners() {
        let dir = temp("outside");
        let other = temp("other");
        let favorites = dir.join("favorites.json");
        let foreign = other.join("fremd.mp3");
        std::fs::write(&foreign, b"x").unwrap();
        let foreign = foreign.to_string_lossy().to_string();
        assert!(delete(&dir, &favorites, &foreign).is_err());
        assert!(set_favorite(&dir, &favorites, &foreign, true).is_err());
        // auch nicht über „..“ aus dem Ordner heraus
        let sneaky = dir.join("..").join(other.file_name().unwrap()).join("fremd.mp3");
        assert!(delete(&dir, &favorites, &sneaky.to_string_lossy()).is_err());
        assert!(Path::new(&foreign).exists());
        std::fs::remove_dir_all(&dir).unwrap();
        std::fs::remove_dir_all(&other).unwrap();
    }

    #[test]
    fn loeschen_mit_midi_und_favorit() {
        let dir = temp("delete");
        let favorites = dir.join("favorites.json");
        let song = dir.join("Lied (Klavier).wav");
        std::fs::write(&song, b"x").unwrap();
        std::fs::write(dir.join("Lied (Klavier).mid"), b"x").unwrap();
        let song = song.to_string_lossy().to_string();
        set_favorite(&dir, &favorites, &song, true).unwrap();
        // im Test endgültig löschen statt den echten Papierkorb zu füllen
        remove(&dir, &favorites, &song, |files| {
            files.iter().try_for_each(|f| std::fs::remove_file(f).map_err(|e| e.to_string()))
        })
        .unwrap();
        assert!(!Path::new(&song).exists());
        assert!(!dir.join("Lied (Klavier).mid").exists());
        assert!(load_favorites(&favorites).is_empty());
        std::fs::remove_dir_all(&dir).unwrap();
    }
}
