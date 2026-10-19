# Tunify Music – App (Tauri 2)

Oberfläche in `src/` (React + TypeScript), Rust-Kern in `src-tauri/`.
Der Rust-Kern startet den Python-Motor aus `../engine` (anderer Ort: Umgebungsvariable `TUNIFY_ENGINE_DIR`).

```powershell
npm install
npm run tauri dev      # App im Entwicklungsmodus starten
cd src-tauri; cargo test   # Tests der Motor-Brücke (braucht engine/.venv)
```
