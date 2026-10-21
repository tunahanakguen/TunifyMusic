import { useEffect, useState } from "react";
import { convertFileSrc, invoke } from "@tauri-apps/api/core";
import { openPath, revealItemInDir } from "@tauri-apps/plugin-opener";

type Item = { path: string; name: string; recording: boolean; modified: number; bytes: number };
type Filter = "results" | "recordings";

const DATE = new Intl.DateTimeFormat("de-DE", { dateStyle: "medium", timeStyle: "short" });

function size(bytes: number) {
  return bytes >= 1_000_000 ? `${(bytes / 1_000_000).toFixed(1).replace(".", ",")} MB` : `${Math.round(bytes / 1000)} kB`;
}

export default function HistoryPage({ onConvert }: { onConvert: (path: string) => void }) {
  const [items, setItems] = useState<Item[] | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [filter, setFilter] = useState<Filter>("results");
  const [playing, setPlaying] = useState<string | null>(null);

  async function load() {
    try {
      setItems(await invoke<Item[]>("list_history"));
      setError(null);
    } catch (e) {
      setError(String(e));
    }
  }

  useEffect(() => {
    load();
  }, []);

  const shown = items?.filter((i) => i.recording === (filter === "recordings")) ?? [];

  return (
    <>
      <h1>Verlauf</h1>
      <p className="muted">Alle Ergebnisse und Aufnahmen aus dem Ordner Musik\Tunify Music.</p>

      <div className="history-bar">
        <div className="segmented" role="radiogroup" aria-label="Anzeigen">
          {(
            [
              ["results", "Ergebnisse"],
              ["recordings", "Aufnahmen"],
            ] as const
          ).map(([id, label]) => (
            <button
              key={id}
              role="radio"
              aria-label={items ? `${label} (${items.filter((i) => i.recording === (id === "recordings")).length})` : label}
              aria-checked={filter === id}
              className={filter === id ? "active" : ""}
              onClick={() => setFilter(id)}
            >
              {label} {items && <small>{items.filter((i) => i.recording === (id === "recordings")).length}</small>}
            </button>
          ))}
        </div>
        <button className="ghost" onClick={load}>
          Aktualisieren
        </button>
        <button className="ghost" onClick={async () => openPath(await invoke<string>("output_folder"))}>
          Ordner öffnen
        </button>
      </div>

      {error && <p className="error">{error}</p>}
      {items && shown.length === 0 && (
        <p className="muted empty">
          {filter === "results" ? "Noch keine Ergebnisse – wandle unter „Stimme umwandeln“ eine Aufnahme um." : "Noch keine Aufnahmen."}
        </p>
      )}

      <ul className="history">
        {shown.map((item) => (
          <li key={item.path} className={playing === item.path ? "open" : ""}>
            <div className="history-row">
              <button
                className="play"
                aria-label={playing === item.path ? `${item.name} schließen` : `${item.name} anhören`}
                onClick={() => setPlaying(playing === item.path ? null : item.path)}
              >
                {playing === item.path ? "■" : "▶"}
              </button>
              <div className="history-name">
                <strong>{item.name}</strong>
                <span className="muted">
                  {DATE.format(new Date(item.modified * 1000))} · {size(item.bytes)}
                </span>
              </div>
              {item.recording && (
                <button className="ghost" onClick={() => onConvert(item.path)}>
                  Umwandeln
                </button>
              )}
              <button className="ghost" onClick={() => revealItemInDir(item.path)}>
                Im Ordner zeigen
              </button>
            </div>
            {playing === item.path && <audio controls autoPlay src={convertFileSrc(item.path)} />}
          </li>
        ))}
      </ul>
    </>
  );
}
