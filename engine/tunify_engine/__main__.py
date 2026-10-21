"""Kommandozeile des Motors.

Beispiele (aus dem Ordner engine/):
  .venv\\Scripts\\python -m tunify_engine info
  .venv\\Scripts\\python -m tunify_engine prepare
  .venv\\Scripts\\python -m tunify_engine convert lied.mp3 --model models\\voices\\Klavier\\klavier.pth
      --isolate-vocals --dereverb --denoise --pitch 0 --formant 1.0 --out ausgabe\\lied_klavier.wav
"""

import argparse
import json
import sys
import traceback

from . import protocol


class _ParseError(Exception):
    """argparse soll im Dauerbetrieb nicht den ganzen Motor beenden."""


class _Parser(argparse.ArgumentParser):
    def error(self, message):
        raise _ParseError(message)


def build_parser() -> argparse.ArgumentParser:
    parser = _Parser(prog="tunify_engine", description="Python-Motor von Tunify Music")
    sub = parser.add_subparsers(dest="command", required=True)

    sub.add_parser("info", help="Python, PyTorch und Grafikkarte melden")
    sub.add_parser("prepare", help="Grundmodelle laden (RMVPE, ContentVec)")
    sub.add_parser("serve", help="Dauerbetrieb für die App: Aufträge als JSON-Zeilen auf stdin, Modelle bleiben geladen")

    conv = sub.add_parser("convert", help="Stimme umwandeln (wie Musicfy Convert Voice)")
    conv.add_argument("input", help="Eingabedatei (MP3, WAV, …)")
    conv.add_argument("--model", required=True, help="RVC-Modell (.pth)")
    conv.add_argument("--index", default="", help="Index-Datei (.index), optional")
    conv.add_argument("--out", required=True, help="Ausgabedatei (.wav, .mp3, .flac)")
    conv.add_argument("--isolate-vocals", action="store_true", help="Instrumental entfernen")
    conv.add_argument("--dereverb", action="store_true", help="Hall/Echo entfernen")
    conv.add_argument("--denoise", action="store_true", help="Rauschen reduzieren")
    conv.add_argument("--pitch", type=int, default=0, help="Halbtöne, -12 … 12")
    conv.add_argument("--formant", type=float, default=1.0, help="Formant, 0.1 … 2.0")
    conv.add_argument("--index-rate", type=float, default=0.3)
    conv.add_argument("--protect", type=float, default=0.33)
    conv.add_argument("--mix-instrumental", action="store_true",
                      help="Begleitung wieder dazumischen (Cover); nur mit --isolate-vocals")
    conv.add_argument("--instrumental-pitch", type=int, default=0, help="Begleitung um Halbtöne verschieben, -12 … 12")

    an = sub.add_parser("analyze", help="Aufnahmen fürs Training bewerten (Länge, Pegel, Rauschen …)")
    an.add_argument("inputs", nargs="+", help="Aufnahmen (MP3, WAV, …)")

    sub.add_parser("instruments", help="Instrumente des Noten-Modus auflisten")
    notes = sub.add_parser("notes", help="Noten-Modus: Melodie erkennen und mit echtem Instrument nachspielen")
    notes.add_argument("input", help="Eingabedatei (MP3, WAV, …)")
    notes.add_argument("--instrument", default="klavier", help="z. B. klavier, geige, floete (siehe instruments)")
    notes.add_argument("--no-fit-range", dest="fit_range", action="store_false",
                       help="nicht automatisch in die Lage des Instruments oktavieren")
    notes.add_argument("--out", required=True, help="Ausgabedatei (.wav, .mp3, .flac)")
    notes.add_argument("--midi", default="", help="erkannte Melodie zusätzlich als .mid speichern")
    notes.add_argument("--transpose", type=int, default=0, help="Halbtöne, -24 … 24")
    notes.add_argument("--chromatic", action="store_true",
                       help="Töne nicht auf die erkannte Tonart ziehen (für Melodien mit vielen Halbtönen)")
    notes.add_argument("--isolate-vocals", action="store_true", help="Instrumental entfernen")
    notes.add_argument("--dereverb", action="store_true", help="Hall/Echo entfernen")
    notes.add_argument("--denoise", action="store_true", help="Rauschen reduzieren")
    notes.add_argument("--mix-instrumental", action="store_true",
                       help="Begleitung wieder dazumischen; nur mit --isolate-vocals")
    notes.add_argument("--instrumental-pitch", type=int, default=0, help="Begleitung um Halbtöne verschieben, -12 … 12")

    tr = sub.add_parser("train", help="Stimme aus eigenen Aufnahmen trainieren (RVC)")
    tr.add_argument("inputs", nargs="+", help="Aufnahmen mit Gesang (MP3, WAV, …)")
    tr.add_argument("--epochs", type=int, default=200)
    tr.add_argument("--batch", type=int, default=8)
    tr.add_argument("--isolate-vocals", action="store_true", help="Instrumental entfernen")
    tr.add_argument("--dereverb", action="store_true", help="Hall/Echo entfernen")
    tr.add_argument("--denoise", action="store_true", help="Rauschen reduzieren")

    so = sub.add_parser("song", help="Lied oder Instrumental aus Text erzeugen (ACE-Step)")
    so.add_argument("--caption", default="", help="Beschreibung: Stil, Instrumente, Stimmung")
    so.add_argument("--lyrics", default="", help="Liedtext (leer = ohne Text); Abschnitte wie [Verse], [Chorus]")
    so.add_argument("--instrumental", action="store_true", help="ohne Gesang")
    so.add_argument("--duration", type=float, default=90, help="Sekunden, 10 … 480")
    so.add_argument("--bpm", type=int, default=None, help="Tempo, leer = automatisch")
    so.add_argument("--key", default="", help="Tonart, z. B. „C major“; leer = automatisch")
    so.add_argument("--language", default="", help="Sprache des Gesangs, z. B. de, en; leer = automatisch")
    so.add_argument("--seed", type=int, default=-1, help="Zufallszahl; gleiche Zahl = gleiches Lied")
    so.add_argument("--reference", default="", help="Referenz-Lied als Vorlage für Klang und Stil")
    so.add_argument("--voice-model", default="", help="RVC-Stimme (.pth): Gesang in dieser Stimme")
    so.add_argument("--voice-index", default="", help="Index der Stimme (.index), optional")
    so.add_argument("--pitch", type=int, default=0, help="Tonhöhe der Stimme, -12 … 12")
    so.add_argument("--out", required=True, help="Ausgabedatei (.wav, .mp3, .flac)")

    return parser


def dispatch(args) -> None:
    if args.command == "info":
        from .info import run
        run()
    elif args.command == "prepare":
        from .convert import prepare
        prepare()
    elif args.command == "convert":
        from .convert import convert
        convert(args)
    elif args.command == "instruments":
        from .notes import list_instruments
        list_instruments()
    elif args.command == "analyze":
        from .analyze import analyze
        analyze(args)
    elif args.command == "train":
        from .train import train
        train(args)
    elif args.command == "notes":
        from .notes import notes_mode
        notes_mode(args)
    elif args.command == "song":
        from .song import song
        song(args)
    elif args.command == "serve":
        raise ValueError("serve läuft schon")


def warmup() -> None:
    """Bibliotheken und Grundmodelle vorab laden, damit der erste Auftrag nicht darauf wartet."""
    from .convert import _applio
    from .notes import rmvpe

    core = _applio()
    converter = core.import_voice_converter()
    converter.load_hubert("contentvec")
    converter.last_embedder_model = "contentvec"
    rmvpe()


def serve() -> None:
    """Ein Auftrag pro Zeile: {"args": ["convert", "lied.wav", …]}. Nach jedem Auftrag folgt {"event": "end"}.

    Geladene Modelle (ContentVec, RMVPE, zuletzt benutzte Stimme) bleiben im Speicher. Aufträge kommen
    nacheinander; Abbrechen heißt für die App: diesen Prozess beenden und beim nächsten Mal neu starten.
    """
    parser = build_parser()
    try:
        warmup()
        protocol.emit("ready")
    except Exception as error:  # noqa: BLE001 – ohne Vorladen geht es trotzdem, nur langsamer
        traceback.print_exc()
        protocol.emit("ready", warning=str(error))
    for line in sys.stdin:
        if not line.strip():
            continue
        try:
            dispatch(parser.parse_args(json.loads(line)["args"]))
        except Exception as error:  # noqa: BLE001 – jeder Fehler geht als JSON an die App
            traceback.print_exc()
            protocol.emit("error", message=str(error))
        protocol.emit("end")


def main() -> None:
    try:
        args = build_parser().parse_args()
        if args.command == "serve":
            serve()
        else:
            dispatch(args)
    except Exception as error:  # noqa: BLE001 – jeder Fehler geht als JSON an die App
        traceback.print_exc()
        protocol.emit("error", message=str(error))
        raise SystemExit(1)


if __name__ == "__main__":
    main()
