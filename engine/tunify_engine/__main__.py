"""Kommandozeile des Motors.

Beispiele (aus dem Ordner engine/):
  .venv\\Scripts\\python -m tunify_engine info
  .venv\\Scripts\\python -m tunify_engine prepare
  .venv\\Scripts\\python -m tunify_engine convert lied.mp3 --model models\\voices\\Klavier\\klavier.pth
      --isolate-vocals --dereverb --denoise --pitch 0 --formant 1.0 --out ausgabe\\lied_klavier.wav
"""

import argparse
import traceback

from . import protocol


def main() -> None:
    parser = argparse.ArgumentParser(prog="tunify_engine", description="Python-Motor von Tunify Music")
    sub = parser.add_subparsers(dest="command", required=True)

    sub.add_parser("info", help="Python, PyTorch und Grafikkarte melden")
    sub.add_parser("prepare", help="Grundmodelle laden (RMVPE, ContentVec)")

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

    tr = sub.add_parser("train", help="Stimme aus eigenen Aufnahmen trainieren (RVC)")
    tr.add_argument("inputs", nargs="+", help="Aufnahmen mit Gesang (MP3, WAV, …)")
    tr.add_argument("--epochs", type=int, default=200)
    tr.add_argument("--batch", type=int, default=8)
    tr.add_argument("--isolate-vocals", action="store_true", help="Instrumental entfernen")
    tr.add_argument("--dereverb", action="store_true", help="Hall/Echo entfernen")
    tr.add_argument("--denoise", action="store_true", help="Rauschen reduzieren")

    args = parser.parse_args()
    try:
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
        elif args.command == "train":
            from .train import train
            train(args)
        elif args.command == "notes":
            from .notes import notes_mode
            notes_mode(args)
    except Exception as error:  # noqa: BLE001 – jeder Fehler geht als JSON an die App
        traceback.print_exc()
        protocol.emit("error", message=str(error))
        raise SystemExit(1)


if __name__ == "__main__":
    main()
