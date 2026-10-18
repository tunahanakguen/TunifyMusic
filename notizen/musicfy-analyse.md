# Analyse: Musicfy „Convert Voice“ (Piano)

Stand: 18.10.2026 · Seite: https://create.musicfy.lol/create/voice/Piano

Untersucht wurde nur, was der Browser ohne Anmeldung bekommt: die Oberfläche, der JavaScript-Code
der Seite und die öffentliche Modell-Liste. Es wurde nichts hochgeladen und nichts generiert.
Die eigentliche Umwandlung läuft auf den Servern von Musicfy und ist von außen nicht einsehbar.

## Oberfläche

| Bereich | Inhalt |
|---|---|
| Kopf | Name der Stimme („Piano“, *By Team Musicfy*), Vorschaubild, **Switch Voice**, **Share** |
| Eingabe | **Upload Audio** (MP3/WAV, Drag & Drop) oder **Record Audio** (Mikrofon); laut FAQ auch YouTube-Link |
| Vorverarbeitung (Schalter) | **Remove Instrumentals**, **Remove Reverb/Echo**, **Reduce Background Noise** |
| Advanced Settings | **Set Pitch** −12 … +12 Halbtöne (Schritt 1, Standard 0) · **Formant Shift** 0,1 … 2,0 (Schritt 0,05, Standard 1) |
| Aktion | **Generate** |
| Rechts | Tageskontingent (3 Umwandlungen ohne Konto), **Recent Generations** |
| Navigation | Copyright-Free, Instrumentals, Convert Voice, Create a Song, Text to Instrumental, Stem Splitter (Pro), My Library/History/Voices, API |

## Auftrag an den Server

Im Seiten-Code (`pages/create/[...voice]-….js`) stehen die Standardwerte eines Auftrags:

```js
{
  shouldIsolateVocals: true,   // "Remove Instrumentals"
  shouldEnhanceVocals: false,  // vermutlich "Remove Reverb/Echo"
  cleanAudio: false,           // "Reduce Background Noise"
  cleanStrength: 0.5,
  f0Method: "rmvpe",           // Tonhöhen-Erkennung
  transpose: 0,                // "Set Pitch" in Halbtönen
  formantFactor: 1             // "Formant Shift"
}
```

Abgeschickt wird das über tRPC: `audioGenerations.createAudioGenerations`.
Der Browser lädt außerdem `ffmpeg.wasm` (`createFFmpegCore`), wahrscheinlich, um Dateien vor dem Hochladen
umzuwandeln oder zu kürzen.

## Modelle

Die öffentliche Liste (`models.getModels`) enthält 3187 Stimmen. Die Felder zeigen eindeutig
**RVC** (Retrieval-based Voice Conversion):

- `f0Method`: rmvpe 3061 · harvest 121 · crepe 2
- `version`: RVC v2 3144 · v1 42
- `sampleRate`: 40 kHz 1925 · 48 kHz 1231 · 32 kHz 30
- `epochCount`, `batchSize`, `isClusterTrained` (Index), `modelAId`/`modelBId` (Mischung zweier Modelle)

**Piano**: RVC v2, rmvpe, 48 kHz, 500 Epochen, Batch 16, `isInstrumental: true`.

38 Modelle sind als Instrument markiert, u. a. E- und Akustikgitarre, Piano, Violine, Saxophon, Cello,
Trompete, Flöte, Bass, Banjo, Saz, Oud, Duduk, Schlagzeug.

## Schluss

Musicfy ist im Kern eine Kette aus freien Bausteinen:

1. Gesang vom Song trennen (UVR-Modelle: MDX/Roformer)
2. optional Hall/Echo und Rauschen entfernen (UVR-DeEcho-DeReverb, Denoise)
3. RVC-v2-Umwandlung mit rmvpe, Transponieren und Formant-Verschiebung
4. bei „Cover“ wieder mit dem Instrumental zusammenmischen

Das lässt sich lokal nachbauen. Die Musicfy-Modelle selbst bekommen und verwenden wir **nicht**:
Ein Klavier-Modell trainieren wir selbst aus freien Klavieraufnahmen oder nehmen ein frei lizenziertes.
