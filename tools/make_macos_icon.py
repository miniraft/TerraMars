#!/usr/bin/env python3
"""Build the macOS ``.icns`` icon from ``assets/icons/app_logo/logo.png``.

macOS accepts PNG-compressed ICNS elements (10.7 and later), so this writes the
container directly instead of shelling out to ``sips``/``iconutil``. That keeps
the committed icon reproducible on any host, including Linux CI, and lets the
packager copy it without needing the Apple tools.

The source logo is a circular badge on a transparent background. It is placed
on Apple's 1024 px icon grid (824 px of artwork, centred) so the Dock and
Finder render it at the same visual weight as native applications.

Regenerate after changing the logo:

    python3 tools/make_macos_icon.py
"""

from __future__ import annotations

import argparse
import struct
from pathlib import Path

from PIL import Image

# (ICNS type, pixel size). The ``ic??`` types are PNG-compressed raster icons;
# each size appears once, and the @2x types let Retina pick a sharp bitmap.
ICNS_TYPES = (
    ("icp4", 16),
    ("icp5", 32),
    ("icp6", 64),
    ("ic07", 128),
    ("ic08", 256),
    ("ic09", 512),
    ("ic10", 1024),
    ("ic11", 32),
    ("ic12", 64),
    ("ic13", 256),
    ("ic14", 512),
)

# Apple's macOS icon grid: the icon artwork occupies 824 of 1024 points.
CANVAS = 1024
ARTWORK = 824


def load_artwork(source: Path) -> Image.Image:
    """Convert the palette logo to RGBA and centre it on a transparent canvas."""
    logo = Image.open(source).convert("RGBA")
    logo.thumbnail((ARTWORK, ARTWORK), Image.Resampling.LANCZOS)
    canvas = Image.new("RGBA", (CANVAS, CANVAS), (0, 0, 0, 0))
    canvas.alpha_composite(
        logo, ((CANVAS - logo.width) // 2, (CANVAS - logo.height) // 2)
    )
    return canvas


def png_element(image: Image.Image, size: int) -> bytes:
    """Encode one square PNG frame without depending on a temp directory."""
    import io

    frame = image.resize((size, size), Image.Resampling.LANCZOS)
    buffer = io.BytesIO()
    frame.save(buffer, format="PNG", optimize=True)
    return buffer.getvalue()


def write_icns(source: Path, destination: Path) -> None:
    artwork = load_artwork(source)
    chunks = []
    for kind, size in ICNS_TYPES:
        data = png_element(artwork, size)
        chunks.append(kind.encode("ascii") + struct.pack(">I", len(data) + 8) + data)
    body = b"".join(chunks)
    destination.parent.mkdir(parents=True, exist_ok=True)
    destination.write_bytes(b"icns" + struct.pack(">I", len(body) + 8) + body)


def main() -> None:
    root = Path(__file__).resolve().parents[1]
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument(
        "--source",
        type=Path,
        default=root / "assets/icons/app_logo/logo.png",
    )
    parser.add_argument(
        "--output",
        type=Path,
        default=root / "assets/icons/app_logo/TerraMars.icns",
    )
    args = parser.parse_args()
    write_icns(args.source, args.output)
    print(f"wrote {args.output} ({args.output.stat().st_size} bytes)")


if __name__ == "__main__":
    main()
