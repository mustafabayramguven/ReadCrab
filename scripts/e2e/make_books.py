#!/usr/bin/env python3
"""E2E test kitaplarını (deneme.cbz ve deneme.epub) üretir."""

import os
import struct
import sys
import zipfile
import zlib


def make_png(width, height, rgb):
    r, g, b = rgb
    raw_rows = []
    row_bytes = bytes([r, g, b]) * width
    for _ in range(height):
        raw_rows.append(b"\x00" + row_bytes)
    raw = b"".join(raw_rows)

    def chunk(tag, data):
        crc = zlib.crc32(tag + data) & 0xFFFFFFFF
        return struct.pack(">I", len(data)) + tag + data + struct.pack(">I", crc)

    ihdr = struct.pack(">IIBBBBB", width, height, 8, 2, 0, 0, 0)
    idat = zlib.compress(raw, 9)
    return (
        b"\x89PNG\r\n\x1a\n"
        + chunk(b"IHDR", ihdr)
        + chunk(b"IDAT", idat)
        + chunk(b"IEND", b"")
    )


def build_cbz(path):
    colors = [(103, 80, 164), (11, 87, 208), (46, 125, 50)]
    with zipfile.ZipFile(path, "w", compression=zipfile.ZIP_DEFLATED) as zf:
        for idx, col in enumerate(colors, start=1):
            zf.writestr(f"p{idx}.png", make_png(64, 64, col))


def build_chapter_xhtml(title, ch_num, with_img):
    paras = []
    for i in range(1, 90):
        paras.append(
            f"<p>Bölüm {ch_num} paragraf {i}: ReadCrab metin okuyucu uçtan uca "
            f"kaydırma ve erişilebilirlik doğrulama metni.</p>"
        )
    img_html = '<p><img src="img1.png" alt="Bölüm görseli"/></p>\n' if with_img else ""
    body = "\n".join(paras)
    return f"""<?xml version="1.0" encoding="utf-8"?>
<!DOCTYPE html>
<html xmlns="http://www.w3.org/1999/xhtml" lang="tr">
<head>
  <meta charset="utf-8"/>
  <title>{title}</title>
</head>
<body>
  <h1>{title}</h1>
  {img_html}
  {body}
</body>
</html>
"""


def build_epub(path):
    container_xml = """<?xml version="1.0" encoding="utf-8"?>
<container version="1.0" xmlns="urn:oasis:names:tc:opendocument:xmlns:container">
  <rootfiles>
    <rootfile full-path="content.opf" media-type="application/oebps-package+xml"/>
  </rootfiles>
</container>
"""
    content_opf = """<?xml version="1.0" encoding="utf-8"?>
<package xmlns="http://www.idpf.org/2007/opf" version="3.0" unique-identifier="uid">
  <metadata xmlns:dc="http://purl.org/dc/elements/1.1/">
    <dc:identifier id="uid">urn:uuid:12345678-1234-1234-1234-1234567890ab</dc:identifier>
    <dc:title>deneme</dc:title>
    <dc:language>tr</dc:language>
  </metadata>
  <manifest>
    <item id="nav" href="nav.xhtml" media-type="application/xhtml+xml" properties="nav"/>
    <item id="ch1" href="ch1.xhtml" media-type="application/xhtml+xml"/>
    <item id="ch2" href="ch2.xhtml" media-type="application/xhtml+xml"/>
    <item id="img1" href="img1.png" media-type="image/png" properties="cover-image"/>
  </manifest>
  <spine>
    <itemref idref="ch1"/>
    <itemref idref="ch2"/>
  </spine>
</package>
"""
    nav_xhtml = """<?xml version="1.0" encoding="utf-8"?>
<!DOCTYPE html>
<html xmlns="http://www.w3.org/1999/xhtml" xmlns:epub="http://www.idpf.org/2007/ops" lang="tr">
<head><meta charset="utf-8"/><title>İçindekiler</title></head>
<body>
  <nav epub:type="toc">
    <ol>
      <li><a href="ch1.xhtml">Birinci Bölüm</a></li>
      <li><a href="ch2.xhtml">İkinci Bölüm</a></li>
    </ol>
  </nav>
</body>
</html>
"""
    with zipfile.ZipFile(path, "w") as zf:
        zf.writestr(
            zipfile.ZipInfo("mimetype"),
            b"application/epub+zip",
            compress_type=zipfile.ZIP_STORED,
        )
        zf.writestr(
            "META-INF/container.xml",
            container_xml.encode("utf-8"),
            compress_type=zipfile.ZIP_DEFLATED,
        )
        zf.writestr(
            "content.opf",
            content_opf.encode("utf-8"),
            compress_type=zipfile.ZIP_DEFLATED,
        )
        zf.writestr(
            "nav.xhtml",
            nav_xhtml.encode("utf-8"),
            compress_type=zipfile.ZIP_DEFLATED,
        )
        zf.writestr(
            "ch1.xhtml",
            build_chapter_xhtml("Birinci Bölüm", 1, False).encode("utf-8"),
            compress_type=zipfile.ZIP_DEFLATED,
        )
        zf.writestr(
            "ch2.xhtml",
            build_chapter_xhtml("İkinci Bölüm", 2, True).encode("utf-8"),
            compress_type=zipfile.ZIP_DEFLATED,
        )
        zf.writestr(
            "img1.png",
            make_png(64, 64, (194, 94, 0)),
            compress_type=zipfile.ZIP_DEFLATED,
        )


def main():
    if len(sys.argv) < 2:
        sys.exit("Kullanım: make_books.py <çıktı_dizini>")
    out_dir = sys.argv[1]
    series_dir = os.path.join(out_dir, "Test Serisi")
    os.makedirs(series_dir, exist_ok=True)
    build_cbz(os.path.join(series_dir, "deneme.cbz"))
    build_epub(os.path.join(series_dir, "deneme.epub"))


if __name__ == "__main__":
    main()
