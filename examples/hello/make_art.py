#!/usr/bin/env python3
"""Draws the sample's placeholder art (original, public domain): a walnut
ground with a brass band, at the sizes Steam's library uses. Pure Python."""
import struct
import zlib
from pathlib import Path

WALNUT, BRASS, CREAM = (0x34, 0x24, 0x18), (0xB0, 0x7C, 0x46), (0xF0, 0xE2, 0xC4)


def png(path, width, height, alpha=False):
    rows = []
    for y in range(height):
        row = bytearray([0])
        for x in range(width):
            band = abs(y - height // 2) < max(2, height // 12)
            edge = x < max(2, width // 40) or x >= width - max(2, width // 40)
            colour = BRASS if band else (CREAM if edge else WALNUT)
            row += bytes(colour) + (bytes([255 if band or not alpha else 0]) if alpha else b"")
        rows.append(bytes(row))
    raw = zlib.compress(b"".join(rows), 9)

    def chunk(kind, data):
        return struct.pack(">I", len(data)) + kind + data + struct.pack(">I", zlib.crc32(kind + data) & 0xFFFFFFFF)

    ihdr = struct.pack(">IIBBBBB", width, height, 8, 6 if alpha else 2, 0, 0, 0)
    Path(path).write_bytes(b"\x89PNG\r\n\x1a\n" + chunk(b"IHDR", ihdr) + chunk(b"IDAT", raw) + chunk(b"IEND", b""))


here = Path(__file__).parent / "art"
here.mkdir(exist_ok=True)
png(here / "icon.png", 256, 256)
png(here / "steam_portrait.png", 600, 900)
png(here / "steam_wide.png", 920, 430)
png(here / "steam_hero.png", 1920, 620)
png(here / "steam_logo.png", 640, 360, alpha=True)
png(here / "steam_icon.png", 256, 256)
print("art written")
