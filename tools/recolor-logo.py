"""Färbt das TUNAS-Logo für Tunify Music ein: schwarzer Grund wird rot, Weiß bleibt weiß.

Aufruf (aus dem Repo-Ordner):
  engine\\.venv\\Scripts\\python -I tools\\recolor-logo.py
"""

from pathlib import Path

from PIL import Image

ROOT = Path(__file__).resolve().parent.parent
BRANDING = ROOT / "branding"
RED = (200, 16, 46)  # #C8102E – Erkennungsfarbe von Tunify Music


def recolor(source: Path) -> Image.Image:
    image = Image.open(source).convert("RGBA")
    luminance = image.convert("L")
    red = Image.new("RGBA", image.size, RED + (255,))
    white = Image.new("RGBA", image.size, (255, 255, 255, 255))
    # Helligkeit als Maske: dunkel → Rot, hell → Weiß; Zwischentöne werden gemischt
    result = Image.composite(white, red, luminance)
    result.putalpha(image.getchannel("A"))
    return result


def main() -> None:
    # Großes Logo (mit Rand) für README und Installer
    recolor(BRANDING / "tunas-logo-1024.png").save(BRANDING / "tunify-logo-1024.png")
    # Eng zugeschnittenes Logo für Symbole, Kopfzeile und Taskleiste
    small = recolor(BRANDING / "TunasLogo.png")
    small.save(BRANDING / "TunifyLogo.png")
    small.save(BRANDING / "TunifyMusic.ico", sizes=[(16, 16), (24, 24), (32, 32), (48, 48), (64, 64), (128, 128), (256, 256)])
    print("Rotes Logo erzeugt in", BRANDING)


if __name__ == "__main__":
    main()
