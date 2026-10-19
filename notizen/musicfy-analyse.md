# Analyse: Musicfy (alle Bereiche)

Stand: 19.10.2026 · Seiten: https://create.musicfy.lol (alle Bereiche) und https://docs.musicfy.lol

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

## Schluss (Convert Voice)

Musicfy ist im Kern eine Kette aus freien Bausteinen:

1. Gesang vom Song trennen (UVR-Modelle: MDX/Roformer)
2. optional Hall/Echo und Rauschen entfernen (UVR-DeEcho-DeReverb, Denoise)
3. RVC-v2-Umwandlung mit rmvpe, Transponieren und Formant-Verschiebung
4. bei „Cover“ wieder mit dem Instrumental zusammenmischen

Das lässt sich lokal nachbauen. Die Musicfy-Modelle selbst bekommen und verwenden wir **nicht**:
Ein Klavier-Modell trainieren wir selbst aus freien Klavieraufnahmen oder nehmen ein frei lizenziertes.

---

# Weitere Bereiche (Stand 19.10.2026)

Wieder nur das, was ohne Anmeldung sichtbar ist: Oberfläche, Seiten-Code und die öffentliche API-Doku.
Für „My Voices“, „Library“ und „History“ braucht man ein Konto. Dort wurde nur der Code gelesen, den der Browser
trotzdem lädt.

## Menü

| Bereich | Adresse | Inhalt |
|---|---|---|
| Copyright-Free / Instrumentals | `/create/voice?models_category=…` | Filter der Stimmenliste (`copyright_free`, `instrumentals`, außerdem `vocals`, `trending`, `favorites`, `my_voices`) |
| Convert Voice | `/create/voice/<Name>` | siehe oben |
| Create a Song | `/create/song` | ganzes Lied aus Text, mit Gesang |
| Text to Instrumental | `/create/text-to-music` | Instrumental aus Text |
| Stem Splitter (Pro) | `/pro-tools/stem-splitter` | Lied in Spuren zerlegen |
| My Library / My History | `/manage/library`, `/manage/history` | gespeicherte bzw. alle Ergebnisse |
| My Voices | `/manage/voices`, `/manage/voices/model/<id>` | eigene Stimmen trainieren |
| API | `/api-access/usage`, `…/tokens`, `…/billing`, docs.musicfy.lol | Schnittstelle für Entwickler |

## Create a Song

Zwei Modi: **Simple** und **Pro**.

- **Simple:** ein Textfeld („Describe the song you want — style, mood, what it's about…“), Schalter
  *Instrumental (no vocals)*, Länge 45–240 s (Standard 1:30), *Sing in my voice* (eigene Stimme, „adds about two minutes“).
- **Pro:** *Lyrics* (optional, max. 4096 Zeichen, „used exactly as written“, leer = KI schreibt den Text),
  *Style* („Genre, instruments, mood…“), Instrumental, Sing in my voice.
  Unter **Advanced**: Dauer 45–240 s, Tempo 30–200 BPM, Taktart 2/3/4/6, Tonart,
  Schreibstil *Creative / Literal*, **Referenz-Audio** (MP3/WAV bis 20 MB, „a track whose style you want“) mit Stärke.

Auftrag (`songGenerations.createSongGeneration`):

```js
{ prompt, duration, mode: "simple" | "pro", lyrics, tags, bpm, musicalKey, timeSignature,
  instrumental, referenceAudioPath, referenceStrength, thinkingMode: "creative" | "literal",
  voiceId }   // voiceId nur mit Gesang
```

**Motor: ACE-Step** (im Code: `engine: "ace-step"`). Fortschrittsanzeige: „Planning your arrangement“ →
„Writing the lyrics“ → „Rendering your song“ → mit eigener Stimme zusätzlich „Singing it in your voice“.
Das heißt: ACE-Step erzeugt das Lied, danach wandelt **RVC** den Gesang in die gewählte Stimme um
(Feld `voiceConversionStatus`). Ergebnis als WAV.

## Text to Instrumental

- Textfeld („Describe the music you want to generate“), vier Vorschläge, Länge 10–240 s (Standard 0:30).
- Auswahl des Motors:
  - **New = ACE-Step** („faster 48kHz stereo instrumentals“), läuft intern über dieselbe Lied-Funktion mit
    `mode: simple, instrumental: true`.
  - **Classic = MusicGen** von Meta („the original Musicfy instrumental engine“).
- Hinweis „Leap x Musicfy“ (Partner tryleap.ai). Die Rechenaufträge laufen über Modal (Cloud-GPUs, `modalToken`).

## My Voices (eigene Stimme trainieren)

Ablauf in vier Schritten: **Upload → Info → Preview → Train**.

- **Upload:** mehrere Audiodateien („with different pitches for maximum accuracy“). Jede Datei bekommt eine
  Qualitätsbewertung (`quality`) und wird danach sortiert.
- **Info:** Name, Stimmtyp (*Artist / Parody / Instrumental*), Geschlecht, Genre, Sichtbarkeit (öffentlich/privat), Abtastrate.
- **Preview:** Vorschaubild und Hörprobe.
- **Train:** Status *Draft → Training → Ready* (oder Fehler).

Standardwerte eines neuen Modells: **RVC v2, rmvpe, 200 Epochen, Batch 16, 40 kHz**, privat, ohne Index.
Datensatz-Länge je Abo: 10 min (Starter), 30 min (Pro), 1 h (Studio). Ab Pro „Vocal Cleanup“ (Gesang vor dem
Training säubern). Außerdem gibt es Modell-Mischungen (`modelAId`/`modelBId`).

## Library und History

- **History:** alle Ergebnisse (Umwandlungen, Lieder, Instrumentals, Stems).
- **Library:** die gespeicherten („These Audio Clips Are Saved Forever“), mit Suche.
- Pro Ergebnis: Abspielen, Speichern, Herunterladen, Neu erzeugen, Löschen.

## Stem Splitter

„Separate vocals, melodies, bass, and drums“: Datei hochladen → *Split Stems* → einzelne Spuren und *Download All*.
Also vier Spuren, sehr wahrscheinlich Demucs. Nur mit Abo.

## API (docs.musicfy.lol)

Server `https://api.musicfy.lol/v1/`, Abrechnung **0,07 $ pro Minute** erzeugtem Audio.

| Route | Parameter |
|---|---|
| `GET /voices` | `voice_types` |
| `POST /convert-voice` | `file` oder `file_url`, `voice_id`, `pitch_shift` (−12…12), `formant_shift` (0,1…2,0), `isolate_vocals`, `background_pitch_shift`, `background_formant_shift` |
| `POST /generate-music` | `prompt` |

Neu gegenüber der Oberfläche: Wenn der Gesang abgetrennt wird, kann auch die **Begleitung** transponiert werden.
Danach werden Gesang und Begleitung wieder zusammengemischt, also ein fertiges „Cover“.

## Schluss (alle Bereiche)

Musicfy besteht aus drei freien Verfahren:

| Musicfy-Bereich | Verfahren | Frei verfügbar |
|---|---|---|
| Convert Voice, My Voices | **RVC v2** (Umwandeln, Trainieren) | ja (Applio, MIT) |
| Create a Song, Text to Instrumental (New) | **ACE-Step** | ja (ACE-Step 1.5, MIT) |
| Text to Instrumental (Classic) | **MusicGen** | Gewichte nur CC BY-NC 4.0 → nicht für eine GPL-App |
| Remove Instrumentals, Stem Splitter | **Trennmodelle** (Roformer/Demucs) | ja |

Alles lässt sich lokal nachbauen. Für ACE-Step reicht die 8-GB-Grafikkarte nur mit den kleineren Turbo-Modellen
bzw. im Sparmodus.
