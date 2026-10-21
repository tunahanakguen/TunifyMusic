import { useEffect, useState } from "react";
import { convertFileSrc, invoke } from "@tauri-apps/api/core";
import { ask } from "@tauri-apps/plugin-dialog";
import { openPath, revealItemInDir } from "@tauri-apps/plugin-opener";

type Item = { path: string; name: string; recording: boolean; favorite: boolean; modified: number; bytes: number };
type Filter = "results" | "favorites" | "recordings";

const FILTERS: { id: Filter; label: string; test: (i: Item) => boolean }[] = [
  { id: "results", label: "Ergebnisse", test: (i) => !i.recording },
  { id: "favorites", label: "Favoriten", test: (i) => i.favorite },
  { id: "recordings", label: "Aufnahmen", test: (i) => i.recording },
];

const EMPTY: Record<Filter, string> = {
  results: "Noch keine Ergebnisse – wandle unter „Stimme umwandeln“ eine Aufnahme um.",
  favorites: "Noch keine Favoriten – markiere ein Ergebnis mit dem Stern.",
  recordings: "Noch keine Aufnahmen.",
};

const DATE = new Intl.DateTimeFormat("de-DE", { dateStyle: "medium", timeStyle: "short" });

function size(bytes: number) {
  return bytes >= 1_000_000 ? `${(bytes / 1_000_000).toFixed(1).replace(".", ",")} MB` : `${Math.round(bytes / 1000)} kB`;
}

export default function HistoryPage({ onConvert }: { onConvert: (path: string) => void }) {
  const [items, setItems] = useState<Item[] | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [filter, setFilter] = useState<Filter>("results");
  const [search, setSearch] = useState("");
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

  async function toggleFavorite(item: Item) {
    try {
      await invoke("set_favorite", { path: item.path, favorite: !item.favorite });
      setItems((list) => list?.map((i) => (i.path === item.path ? { ...i, favorite: !i.favorite } : i)) ?? null);
    } catch (e) {
      setError(String(e));
    }
  }

  async function remove(item: Item) {
    const sure = await ask(`„${item.name}“ in den Papierkorb verschieben?`, {
      title: "Löschen",
      kind: "warning",
      okLabel: "In den Papierkorb",
      cancelLabel: "Behalten",
    });
    if (!sure) return;
    try {
      await invoke("delete_history_item", { path: item.path });
      if (playing === item.path) setPlaying(null);
      setItems((list) => list?.filter((i) => i.path !== item.path) ?? null);
    } catch (e) {
      setError(String(e));
    }
  }

  const words = search.toLowerCase().split(/\s+/).filter(Boolean);
  const test = FILTERS.find((f) => f.id === filter)!.test;
  const shown = items?.filter((i) => test(i) && words.every((w) => i.name.toLowerCase().includes(w))) ?? [];

  return (
    <>
      <h1>Verlauf</h1>
      <p className="muted">Alle Ergebnisse und Aufnahmen aus dem Ordner Musik\Tunify Music.</p>

      <div className="history-bar">
        <div className="segmented" role="radiogroup" aria-label="Anzeigen">
          {FILTERS.map(({ id, label, test }) => {
            const count = items?.filter(test).length;
            return (
              <button
                key={id}
                role="radio"
                aria-label={items ? `${label} (${count})` : label}
                aria-checked={filter === id}
                className={filter === id ? "active" : ""}
                onClick={() => setFilter(id)}
              >
                {label} {items && <small>{count}</small>}
              </button>
            );
          })}
        </div>
        <input
          type="search"
          className="history-search"
          aria-label="Suchen"
          placeholder="Suchen …"
          value={search}
          onChange={(e) => setSearch(e.target.value)}
        />
        <button className="ghost" onClick={load}>
          Aktualisieren
        </button>
        <button className="ghost" onClick={async () => openPath(await invoke<string>("output_folder"))}>
          Ordner öffnen
        </button>
      </div>

      {error && <p className="error">{error}</p>}
      {items && shown.length === 0 && (
        <p className="muted empty">{words.length ? `Nichts gefunden für „${search.trim()}“.` : EMPTY[filter]}</p>
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
              <button
                className={`ghost star ${item.favorite ? "on" : ""}`}
                aria-label={item.favorite ? `${item.name} aus Favoriten entfernen` : `${item.name} zu Favoriten`}
                aria-pressed={item.favorite}
                title={item.favorite ? "Aus Favoriten entfernen" : "Zu Favoriten"}
                onClick={() => toggleFavorite(item)}
              >
                {item.favorite ? "★" : "☆"}
              </button>
              {item.recording && (
                <button className="ghost" onClick={() => onConvert(item.path)}>
                  Umwandeln
                </button>
              )}
              <button className="ghost" onClick={() => revealItemInDir(item.path)}>
                Im Ordner zeigen
              </button>
              <button className="ghost" aria-label={`${item.name} löschen`} onClick={() => remove(item)}>
                Löschen
              </button>
            </div>
            {playing === item.path && <audio controls autoPlay src={convertFileSrc(item.path)} />}
          </li>
        ))}
      </ul>
    </>
  );
}
