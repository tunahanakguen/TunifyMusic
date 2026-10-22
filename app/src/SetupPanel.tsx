import { useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";

export type SetupStatus = { core: boolean; music: boolean; folder: string };
type Progress = { percent: number; message: string };

const PARTS = {
  core: {
    title: "Tunify Music einrichten",
    text: "Beim ersten Start lädt Tunify den KI-Motor: Python mit PyTorch für die Grafikkarte, RVC (Applio), die Modelle zum Trennen von Gesang und Begleitung, FFmpeg und die Instrumenten-Klänge. Alles bleibt auf diesem PC.",
    size: "ca. 8 GB Download, 12 GB auf der Festplatte · 15–40 Minuten je nach Internet",
    button: "Jetzt einrichten",
  },
  music: {
    title: "Musik erzeugen einrichten",
    text: "Für „Lied erzeugen“ und „Instrumental erzeugen“ braucht Tunify zusätzlich ACE-Step mit eigener Umgebung und Musikmodellen.",
    size: "ca. 12 GB Download, 14 GB auf der Festplatte · 20–60 Minuten je nach Internet",
    button: "Musik erzeugen einrichten",
  },
};

type Props = {
  part: "core" | "music";
  onDone: (status: SetupStatus) => void;
};

export default function SetupPanel({ part, onDone }: Props) {
  const [busy, setBusy] = useState(false);
  const [progress, setProgress] = useState<Progress | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [cancelled, setCancelled] = useState(false);
  const info = PARTS[part];

  useEffect(() => {
    let unlisten: (() => void) | undefined;
    try {
      listen<Progress>("setup-progress", (e) => setProgress(e.payload))
        .then((fn) => (unlisten = fn))
        .catch(() => {});
    } catch {
      // außerhalb von Tauri
    }
    return () => unlisten?.();
  }, []);

  async function start() {
    setBusy(true);
    setError(null);
    setCancelled(false);
    setProgress({ percent: 0, message: "Einrichtung startet …" });
    try {
      onDone(await invoke<SetupStatus>("run_setup", { part }));
    } catch (e) {
      if (String(e) === "Abgebrochen") setCancelled(true);
      else setError(String(e));
    } finally {
      setBusy(false);
      setProgress(null);
    }
  }

  return (
    <section className="card wide setup">
      <h2>{info.title}</h2>
      <p>{info.text}</p>
      <p className="muted small">{info.size}</p>
      <div className="actions">
        <button onClick={start} disabled={busy}>
          {busy ? "Wird eingerichtet …" : cancelled || error ? "Weitermachen" : info.button}
        </button>
        {busy && (
          <button className="ghost" onClick={() => invoke("cancel_setup")}>
            Abbrechen
          </button>
        )}
      </div>
      {progress && (
        <div className="progress">
          <div className="bar">
            <div style={{ width: `${progress.percent}%` }} />
          </div>
          <span className="muted">{progress.message}</span>
        </div>
      )}
      {busy && <p className="muted small tip">Du kannst Tunify dabei offen lassen. Bei einer Unterbrechung geht es beim nächsten Mal dort weiter.</p>}
      {cancelled && <p className="muted">Abgebrochen. Schon Geladenes bleibt erhalten.</p>}
      {error && <p className="error">{error}</p>}
    </section>
  );
}
