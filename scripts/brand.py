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
# Mirrors globals.css, so the cover matches the page it links to.
DARK = "#050506"
MUTED = "#8a8783"

# The lit disc, and the body eclipsing it.
DISC = (128.0, 128.0, 88.0)
SHADOW = (186.0, 70.0, 74.0)

# Band edges in the 256 unit glyph space, as (y, height).
BANDS = ((24, 92), (128, 34), (174, 58))

ROOT = Path(__file__).resolve().parent.parent
OUT = ROOT / "public" / "brand"
APPLE_ICON = ROOT / "src" / "app" / "apple-icon.png"
APPLE_ICON_SIZE = 180
OPENGRAPH_IMAGE = ROOT / "src" / "app" / "opengraph-image.png"
OPENGRAPH_SIZE = (1200, 630)
# GitHub crops its social preview to 1280x640.
SOCIAL_IMAGE = OUT / "erebus-cover.png"
SOCIAL_SIZE = (1280, 640)


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


def topology(width: float, height: float) -> str:
    """Three columns of nodes, fully connected between adjacent layers.

    The same picture as the network section of the site: it is the product, and
    it fills space without needing a stock illustration.
    """
    columns = 3
    rows = 5
    left = width * 0.575
    span = width * 0.335
    top = height * 0.245
    gap = height * 0.60 / (rows - 1)

    def point(col: int, row: int) -> tuple[float, float]:
        return (left + col * span / (columns - 1), top + row * gap)

    lines = []
    for col in range(columns - 1):
        for row in range(rows):
            for other in range(rows):
                x1, y1 = point(col, row)
                x2, y2 = point(col + 1, other)
                lines.append(
                    f'  <line x1="{x1:.1f}" y1="{y1:.1f}" x2="{x2:.1f}" y2="{y2:.1f}"'
                    f' stroke="{FOREGROUND}" stroke-opacity="0.09" stroke-width="1" />'
                )

    dots = [
        f'  <circle cx="{point(col, row)[0]:.1f}" cy="{point(col, row)[1]:.1f}" r="4"'
        f' fill="{FOREGROUND}" fill-opacity="{0.55 if col == 1 else 0.35}" />'
        for col in range(columns)
        for row in range(rows)
    ]
    labels = [
        f'  <text x="{point(col, 0)[0]:.1f}" y="{top - 34:.0f}" fill="{FOREGROUND}"'
        f' fill-opacity="0.35" font-family="JetBrains Mono, monospace" font-size="15"'
        f' letter-spacing="3" text-anchor="middle">{name}</text>'
        for col, name in enumerate(("ENTRY", "RELAY", "EXIT"))
    ]
    return "\n".join(lines + dots + labels)


def cover(width: int = 1200, height: int = 630) -> str:
    """Social preview: the mark, the name, and what it is for."""
    glyph = height * 0.185 / 256

    return f"""<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 {width} {height}" fill="none">
  <title>Erebus — network-layer privacy for tokenized finance</title>
  <defs>
    <clipPath id="erebus-bands">
{bands("      ")}
    </clipPath>
  </defs>
  <rect width="{width}" height="{height}" fill="{DARK}" />
{topology(width, height)}
  <g transform="translate({width * 0.072:.0f} {height * 0.175:.0f}) scale({glyph:.4f})">
    <path fill="{BACKGROUND}" clip-path="url(#erebus-bands)" d="{crescent()}" />
  </g>
  <text x="{width * 0.07:.0f}" y="{height * 0.555:.0f}" fill="{FOREGROUND}"
    font-family="Inter, system-ui, sans-serif" font-size="{height * 0.115:.0f}"
    font-weight="500" letter-spacing="{height * 0.019:.0f}">EREBUS</text>
  <text x="{width * 0.075:.0f}" y="{height * 0.685:.0f}" fill="{MUTED}"
    font-family="Inter, system-ui, sans-serif" font-size="{height * 0.042:.0f}"
    font-weight="300">Privacy at the network layer for tokenized finance.</text>
  <text x="{width * 0.075:.0f}" y="{height * 0.885:.0f}" fill="{BACKGROUND}"
    font-family="JetBrains Mono, monospace" font-size="{height * 0.028:.0f}"
    letter-spacing="2">SPHINX MIXNET · SHIELDED FEES · ROBINHOOD CHAIN</text>
</svg>
"""


def main() -> None:
    OUT.mkdir(parents=True, exist_ok=True)
    for name, svg in (
        ("erebus-mark.svg", mark()),
        ("erebus-icon.svg", icon()),
        ("erebus-wordmark.svg", wordmark()),
        ("erebus-cover.svg", cover(*SOCIAL_SIZE)),
    ):
        (OUT / name).write_text(svg)
        print(f"wrote {OUT / name}")

    (ROOT / "src" / "app" / "icon.svg").write_text(icon())
    print(f"wrote {ROOT / 'src' / 'app' / 'icon.svg'}")

    try:
        import cairosvg
    except ImportError:
        print("cairosvg not installed; left the rasters as they were")
        return

    rasters = (
        (APPLE_ICON, icon(), (APPLE_ICON_SIZE, APPLE_ICON_SIZE)),
        (OPENGRAPH_IMAGE, cover(*OPENGRAPH_SIZE), OPENGRAPH_SIZE),
        (SOCIAL_IMAGE, cover(*SOCIAL_SIZE), SOCIAL_SIZE),
    )
    for path, svg, (width, height) in rasters:
        cairosvg.svg2png(
            bytestring=svg.encode(),
            write_to=str(path),
            output_width=width,
            output_height=height,
        )
        print(f"wrote {path}")


if __name__ == "__main__":
    main()
