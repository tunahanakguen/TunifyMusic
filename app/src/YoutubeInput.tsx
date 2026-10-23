import { useState } from "react";
import { invoke } from "@tauri-apps/api/core";

type Props = {
  disabled: boolean;
  onLoaded: (path: string) => void;
  onError: (message: string) => void;
};

/** YouTube-Link einfügen → Tonspur laden → wie eine gewählte Datei weiterverwenden. */
export default function YoutubeInput({ disabled, onLoaded, onError }: Props) {
  const [url, setUrl] = useState("");
  const [loading, setLoading] = useState(false);

  async function load() {
    if (!url.trim()) return;
    setLoading(true);
    try {
      onLoaded(await invoke<string>("fetch_youtube", { url: url.trim() }));
      setUrl("");
    } catch (e) {
      onError(String(e));
    } finally {
      setLoading(false);
    }
  }

  return (
    <div className="youtube-row">
      <input
        type="url"
        aria-label="YouTube-Link"
        placeholder="oder YouTube-Link einfügen (bis 20 Minuten)"
        value={url}
        onChange={(e) => setUrl(e.target.value)}
        onKeyDown={(e) => e.key === "Enter" && load()}
        disabled={disabled || loading}
      />
      <button className="ghost" onClick={load} disabled={disabled || loading || !url.trim()}>
        {loading ? "Lädt …" : "Laden"}
      </button>
    </div>
  );
}
