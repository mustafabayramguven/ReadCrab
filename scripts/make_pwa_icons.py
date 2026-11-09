"""PWA simgeleri (192 ve 512 px): #6750A4 zemin üzerinde beyaz "açık kitap".

Kullanım (depo kökünden): python3 -I scripts/make_pwa_icons.py web
Yalnız standart kütüphane. Çıktı: icon-192.png, icon-512.png.
"""
import os
import struct
import sys
import zlib

BG = (0x67, 0x50, 0xA4)
FG = (255, 255, 255)


def in_rounded_rect(x, y, x0, y0, x1, y1, r):
    if x < x0 or x > x1 or y < y0 or y > y1:
        return False
    cx = min(max(x, x0 + r), x1 - r)
    cy = min(max(y, y0 + r), y1 - r)
    return (x - cx) ** 2 + (y - cy) ** 2 <= r * r


def render(s):
    w, h, gap, r = 0.56 * s, 0.40 * s, 0.03 * s, 0.05 * s
    left, top = (s - w) / 2, (s - h) / 2
    right, bottom, mid = left + w, top + h, s / 2
    pages = [(left, top, mid - gap / 2, bottom), (mid + gap / 2, top, right, bottom)]
    raw = bytearray()
    for y in range(s):
        raw += b"\x00"
        for x in range(s):
            hit = any(in_rounded_rect(x + 0.5, y + 0.5, *p, r) for p in pages)
            raw += bytes(FG if hit else BG)
    return bytes(raw)


def png(s, raw):
    def chunk(tag, data):
        crc = zlib.crc32(tag + data) & 0xFFFFFFFF
        return struct.pack(">I", len(data)) + tag + data + struct.pack(">I", crc)

    ihdr = struct.pack(">IIBBBBB", s, s, 8, 2, 0, 0, 0)
    return (
        b"\x89PNG\r\n\x1a\n"
        + chunk(b"IHDR", ihdr)
        + chunk(b"IDAT", zlib.compress(raw, 9))
        + chunk(b"IEND", b"")
    )


def main():
    out = sys.argv[1]
    for s in (192, 512):
        data = png(s, render(s))
        assert data[:8] == b"\x89PNG\r\n\x1a\n"
        with open(os.path.join(out, f"icon-{s}.png"), "wb") as f:
            f.write(data)
        print(f"icon-{s}.png", len(data), "bayt")


if __name__ == "__main__":
    main()
