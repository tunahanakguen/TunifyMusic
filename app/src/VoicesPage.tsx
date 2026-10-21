import { useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { ask, open } from "@tauri-apps/plugin-dialog";

export type Voice = { id: string; name: string; model: string; index: string | null; created: number };

const DATE = new Intl.DateTimeFormat("de-DE", { dateStyle: "medium" });

/** „sänger_e300_s12000.pth“ → „sänger“: Trainings-Anhänge von Applio/RVC weglassen */
function suggestName(files: string[]) {
  const model = files.find((f) => /\.(pth|zip)$/i.test(f)) ?? files[0];
  const stem = (model.split(/[\\/]/).pop() ?? "").replace(/\.[^.]+$/, "");
  const cleaned = stem
    .replace(/_\d+e(_\d+s)?$/i, "")
    .replace(/[_-]+/g, " ")
    .trim();
  return cleaned ? cleaned.charAt(0).toUpperCase() + cleaned.slice(1) : "Neue Stimme";
}

export default function VoicesPage({ onUse }: { onUse: (id: string) => void }) {
  const [voices, setVoices] = useState<Voice[] | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [pending, setPending] = useState<{ files: string[]; name: string } | null>(null);
  const [importing, setImporting] = useState(false);
  const [editing, setEditing] = useState<{ id: string; name: string } | null>(null);

  async function load() {
    try {
      setVoices(await invoke<Voice[]>("list_voices"));
    } catch (e) {
      setError(String(e));
    }
  }

  useEffect(() => {
    load();
  }, []);

  async function choose() {
    setError(null);
    const files = await open({
      multiple: true,
      directory: false,
      filters: [{ name: "RVC-Stimme (.pth, .index oder .zip)", extensions: ["pth", "index", "zip"] }],
    });
    if (files && files.length) setPending({ files, name: suggestName(files) });
  }

  async function importVoice() {
    if (!pending) return;
    setImporting(true);
    setError(null);
    try {
      await invoke<Voice>("import_voice", { name: pending.name, files: pending.files });
      setPending(null);
      await load();
    } catch (e) {
      setError(String(e));
    } finally {
      setImporting(false);
    }
  }

  async function rename() {
    if (!editing) return;
    try {
      await invoke("rename_voice", { id: editing.id, name: editing.name });
      setEditing(null);
      await load();
    } catch (e) {
      setError(String(e));
    }
  }

  async function remove(voice: Voice) {
    const sure = await ask(`„${voice.name}“ aus der Bibliothek löschen? Die ursprünglich importierten Dateien bleiben erhalten.`, {
      title: "Stimme löschen",
      kind: "warning",
      okLabel: "Löschen",
      cancelLabel: "Behalten",
    });
    if (!sure) return;
    try {
      await invoke("delete_voice", { id: voice.id });
      await load();
    } catch (e) {
      setError(String(e));
    }
  }

  return (
    <>
      <h1>Stimmen</h1>
      <p className="muted">
        RVC-Stimmen für „Stimme umwandeln“. Importiere ein Modell (.pth, am besten mit passender .index-Datei) oder ein
        ZIP-Archiv mit beidem.
      </p>

      <section className="card wide">
        <div className="card-head">
          <h2>Stimme importieren</h2>
          {!pending && (
            <button onClick={choose} disabled={importing}>
              Dateien wählen …
            </button>
          )}
        </div>
        {pending ? (
          <div className="import-form">
            <ul className="file-list">
              {pending.files.map((f) => (
                <li key={f}>{f.split(/[\\/]/).pop()}</li>
              ))}
            </ul>
            <label htmlFor="voice-name">Name der Stimme</label>
            <input
              id="voice-name"
              type="text"
              value={pending.name}
              onChange={(e) => setPending({ ...pending, name: e.target.value })}
              onKeyDown={(e) => e.key === "Enter" && importVoice()}
              disabled={importing}
              autoFocus
            />
            <p className="muted small">
              Verwende nur Stimmen, an denen du die Rechte hast – deine eigene oder die einer Person, die zugestimmt hat.
            </p>
            <div className="actions">
              <button onClick={importVoice} disabled={importing || !pending.name.trim()}>
                {importing ? "Importiere …" : "Importieren"}
              </button>
              <button className="ghost" onClick={() => setPending(null)} disabled={importing}>
                Abbrechen
              </button>
            </div>
          </div>
        ) : (
          <p className="muted small">Eine eigene Stimme machst du unter „Stimme trainieren“.</p>
        )}
        {error && <p className="error">{error}</p>}
      </section>

      {voices && voices.length === 0 && <p className="muted empty">Noch keine Stimmen in deiner Bibliothek.</p>}
      <ul className="history voices">
        {(voices ?? []).map((v) => (
          <li key={v.id}>
            <div className="history-row">
              <div className="voice-avatar" aria-hidden="true">
                {v.name.charAt(0).toUpperCase()}
              </div>
              {editing?.id === v.id ? (
                <div className="history-name">
                  <input
                    type="text"
                    aria-label="Neuer Name"
                    value={editing.name}
                    onChange={(e) => setEditing({ ...editing, name: e.target.value })}
                    onKeyDown={(e) => {
                      if (e.key === "Enter") rename();
                      if (e.key === "Escape") setEditing(null);
                    }}
                    autoFocus
                  />
                </div>
              ) : (
                <div className="history-name">
                  <strong>{v.name}</strong>
                  <span className="muted">
                    {v.index ? "mit Index" : "ohne Index"} · importiert am {DATE.format(new Date(v.created * 1000))}
                  </span>
                </div>
              )}
              {editing?.id === v.id ? (
                <>
                  <button onClick={rename}>Speichern</button>
                  <button className="ghost" onClick={() => setEditing(null)}>
                    Abbrechen
                  </button>
                </>
              ) : (
                <>
                  <button onClick={() => onUse(v.id)}>Benutzen</button>
                  <button className="ghost" onClick={() => setEditing({ id: v.id, name: v.name })}>
                    Umbenennen
                  </button>
                  <button className="ghost" onClick={() => remove(v)}>
                    Löschen
                  </button>
                </>
              )}
            </div>
          </li>
        ))}
      </ul>
    </>
  );
}
