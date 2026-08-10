#!/usr/bin/env python3
"""Generates the Erebus brand marks.

The glyph is an eclipse cut into three bands: the disc is still there, almost
none of it is legible, and what is left of it is the three mix layers a packet
is routed through. Geometry is generated rather than hand-written because the
crescent is the intersection of two circles, and the arc endpoints have to be
exact for the shape to close.

    python3 scripts/brand.py

The Apple touch icon has to be a raster, because `apple-icon` does not accept
SVG. Rendering it needs `cairosvg`; without it the SVGs are still written and
the existing PNG is left alone.
"""

from __future__ import annotations

import math
from pathlib import Path

BACKGROUND = "#b8452b"
FOREGROUND = "#f5f3f0"

# The lit disc, and the body eclipsing it.
DISC = (128.0, 128.0, 88.0)
SHADOW = (186.0, 70.0, 74.0)

# Band edges in the 256 unit glyph space, as (y, height).
BANDS = ((24, 92), (128, 34), (174, 58))

ROOT = Path(__file__).resolve().parent.parent
OUT = ROOT / "public" / "brand"
APPLE_ICON = ROOT / "src" / "app" / "apple-icon.png"
APPLE_ICON_SIZE = 180


def crescent() -> str:
    """The eclipsed disc, as a closed path of two arcs."""
    (x1, y1, r1), (x2, y2, r2) = DISC, SHADOW
    d = math.dist((x1, y1), (x2, y2))
    # Distance from the disc's centre to the chord where the circles meet.
    along = (d * d + r1 * r1 - r2 * r2) / (2 * d)
    across = math.sqrt(r1 * r1 - along * along)

    ux, uy = (x2 - x1) / d, (y2 - y1) / d
    mx, my = x1 + along * ux, y1 + along * uy
    end = (mx - across * uy, my + across * ux)
    start = (mx + across * uy, my - across * ux)

    return (
        f"M{start[0]:.2f} {start[1]:.2f}"
        f"A{r1:.0f} {r1:.0f} 0 1 0 {end[0]:.2f} {end[1]:.2f}"
        f"A{r2:.0f} {r2:.0f} 0 0 1 {start[0]:.2f} {start[1]:.2f}Z"
    )


def bands(indent: str) -> str:
    return "\n".join(
        f'{indent}<rect x="24" y="{y}" width="208" height="{height}" />'
        for y, height in BANDS
    )


def mark() -> str:
    return f"""<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 256 256" fill="none">
  <title>Erebus</title>
  <defs>
    <clipPath id="erebus-bands">
{bands("      ")}
    </clipPath>
  </defs>
  <path fill="currentColor" clip-path="url(#erebus-bands)" d="{crescent()}" />
</svg>
"""


def icon() -> str:
    return f"""<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 512 512" fill="none">
  <title>Erebus</title>
  <defs>
    <clipPath id="erebus-bands">
{bands("      ")}
    </clipPath>
  </defs>
  <rect width="512" height="512" fill="{BACKGROUND}" />
  <g transform="translate(92 84) scale(1.34)">
    <path fill="{FOREGROUND}" clip-path="url(#erebus-bands)" d="{crescent()}" />
  </g>
</svg>
"""


def wordmark() -> str:
    return f"""<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 900 256" fill="none">
  <title>Erebus</title>
  <defs>
    <clipPath id="erebus-bands">
{bands("      ")}
    </clipPath>
  </defs>
  <path fill="currentColor" clip-path="url(#erebus-bands)" d="{crescent()}" />
  <text
    x="272"
    y="128"
    fill="currentColor"
    font-family="Inter, system-ui, sans-serif"
    font-size="104"
    font-weight="500"
    letter-spacing="18"
    dominant-baseline="central"
  >EREBUS</text>
</svg>
"""


def main() -> None:
    OUT.mkdir(parents=True, exist_ok=True)
    for name, svg in (
        ("erebus-mark.svg", mark()),
        ("erebus-icon.svg", icon()),
        ("erebus-wordmark.svg", wordmark()),
    ):
        (OUT / name).write_text(svg)
        print(f"wrote {OUT / name}")

    (ROOT / "src" / "app" / "icon.svg").write_text(icon())
    print(f"wrote {ROOT / 'src' / 'app' / 'icon.svg'}")

    try:
        import cairosvg
    except ImportError:
        print("cairosvg not installed; left apple-icon.png as it was")
        return

    cairosvg.svg2png(
        bytestring=icon().encode(),
        write_to=str(APPLE_ICON),
        output_width=APPLE_ICON_SIZE,
        output_height=APPLE_ICON_SIZE,
    )
    print(f"wrote {APPLE_ICON}")


if __name__ == "__main__":
    main()
