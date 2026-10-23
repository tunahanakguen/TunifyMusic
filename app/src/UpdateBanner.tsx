import { useEffect, useState } from "react";
import { check, Update } from "@tauri-apps/plugin-updater";
import { relaunch } from "@tauri-apps/plugin-process";

/**
 * Beim Start auf GitHub nach einer neuen Version sehen. Updates sind signiert; die App installiert nur,
 * was zum Schlüssel in tauri.conf.json passt. Ohne Internet oder bei Fehlern bleibt der Hinweis einfach weg.
 */
export default function UpdateBanner() {
  const [update, setUpdate] = useState<Update | null>(null);
  const [dismissed, setDismissed] = useState(false);
  const [percent, setPercent] = useState<number | null>(null);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    check()
      .then((found) => setUpdate(found))
      .catch(() => {});
  }, []);

  async function install() {
    if (!update) return;
    setError(null);
    setPercent(0);
    let total = 0;
    let loaded = 0;
    try {
      await update.downloadAndInstall((event) => {
        if (event.event === "Started") total = event.data.contentLength ?? 0;
        if (event.event === "Progress") {
          loaded += event.data.chunkLength;
          if (total) setPercent(Math.round((loaded / total) * 100));
        }
      });
      // Unter Windows beendet der Installer die App selbst; sonst neu starten
      await relaunch();
    } catch (e) {
      setPercent(null);
      setError(`Update fehlgeschlagen: ${e}`);
    }
  }

  if (!update || dismissed) return null;
  return (
    <section className="update-banner" role="status">
      <div>
        <strong>Tunify Music {update.version} ist da</strong>
        <span className="muted"> · du hast {update.currentVersion}</span>
        {update.body && <p className="muted small">{update.body}</p>}
        {percent !== null && (
          <p className="muted small">
            {percent < 100 ? `Lädt … ${percent} %` : "Installiert – Windows fragt gleich nach Administratorrechten"}
          </p>
        )}
        {error && <p className="error">{error}</p>}
      </div>
      <div className="actions">
        <button onClick={install} disabled={percent !== null}>
          Jetzt aktualisieren
        </button>
        <button className="ghost" onClick={() => setDismissed(true)} disabled={percent !== null}>
          Später
        </button>
      </div>
    </section>
  );
}
