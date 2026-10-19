import { useEffect, useState } from "react";
import { getCurrentWindow } from "@tauri-apps/api/window";

/** "system" folgt dem Hell/Dunkel-Modus von Windows. */
export type ThemeChoice = "light" | "dark" | "system";

const STORAGE_KEY = "tunify.theme";
const DEFAULT_CHOICE: ThemeChoice = "light";

function loadChoice(): ThemeChoice {
  try {
    const saved = localStorage.getItem(STORAGE_KEY);
    if (saved === "light" || saved === "dark" || saved === "system") return saved;
  } catch {
    // ohne Speicher einfach den Standard nehmen
  }
  return DEFAULT_CHOICE;
}

const systemQuery = () => window.matchMedia("(prefers-color-scheme: dark)");

/** Wählt das Design, merkt es sich und hält Seite und Fenster-Titelleiste im selben Modus. */
export function useTheme() {
  const [choice, setChoice] = useState<ThemeChoice>(loadChoice);
  const [systemDark, setSystemDark] = useState(() => systemQuery().matches);

  useEffect(() => {
    try {
      localStorage.setItem(STORAGE_KEY, choice);
    } catch {
      // nicht schlimm, dann gilt beim nächsten Start der Standard
    }
    // Titelleiste: fester Modus oder (null) wie Windows. Außerhalb von Tauri (Browser-Test) gibt es kein Fenster.
    try {
      getCurrentWindow()
        .setTheme(choice === "system" ? null : choice)
        .catch(() => {});
    } catch {
      // kein Tauri-Fenster
    }
  }, [choice]);

  useEffect(() => {
    const query = systemQuery();
    const update = () => setSystemDark(query.matches);
    query.addEventListener("change", update);
    window.addEventListener("focus", update);

    // Zusätzlich das Signal von Tauri: Im Modus „Wie Windows“ folgt das Fenster dem System.
    let unlisten: (() => void) | undefined;
    if (choice === "system") {
      try {
        getCurrentWindow()
          .onThemeChanged(({ payload }) => setSystemDark(payload === "dark"))
          .then((fn) => (unlisten = fn))
          .catch(() => {});
      } catch {
        // kein Tauri-Fenster
      }
    }
    return () => {
      query.removeEventListener("change", update);
      window.removeEventListener("focus", update);
      unlisten?.();
    };
  }, [choice]);

  const resolved = choice === "system" ? (systemDark ? "dark" : "light") : choice;

  useEffect(() => {
    document.documentElement.dataset.theme = resolved;
  }, [resolved]);

  return { choice, setChoice, resolved };
}
