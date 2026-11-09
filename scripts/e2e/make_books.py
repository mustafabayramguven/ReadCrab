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


def build_volume_epub(path, volume):
    """İki bölüm, her bölümde bir PNG; 60 Türkçe paragraf, 30. pakette 'kelime' ve 'şeker'."""
    # Cilt 1'in sonuna tek paragraflık kısa bir üçüncü bölüm eklenir (21.1)
    count = 3 if volume == 1 else 2
    chapters = []
    for n in range(1, count + 1):
        paras = [
            f"<p>Cilt {volume}, bölüm {n}, paragraf {i}: ışık, çiçek ve gölge sözcükleri "
            f"Türkçe okuma testinde kullanılır.</p>"
            for i in range(1, 61)
        ]
        paras[29] = "<p>Bu cümlede kelime ve şeker birlikte geçer.</p>"
        img_line = f'<p><img src="img{n}.png" alt="Bölüm {n} görseli"/></p>' if n < 3 else ""
        if n == 3:
            paras = ["<p>Son kısa bölüm.</p>"]
        chapters.append(
            f"""<?xml version="1.0" encoding="utf-8"?>
<!DOCTYPE html>
<html xmlns="http://www.w3.org/1999/xhtml" lang="tr">
<head><meta charset="utf-8"/><title>Bölüm {n}</title></head>
<body>
  <h1>Bölüm {n}</h1>
  {img_line}
  {chr(10).join(paras)}
</body>
</html>
"""
        )
    ch3_item = '<item id="ch3" href="ch3.xhtml" media-type="application/xhtml+xml"/>' if count == 3 else ""
    ch3_ref = '<itemref idref="ch3"/>' if count == 3 else ""
    nav_ch3 = '<li><a href="ch3.xhtml">Bölüm 3</a></li>' if count == 3 else ""
    opf = f"""<?xml version="1.0" encoding="utf-8"?>
<package xmlns="http://www.idpf.org/2007/opf" version="3.0" unique-identifier="uid">
  <metadata xmlns:dc="http://purl.org/dc/elements/1.1/">
    <dc:identifier id="uid">urn:uuid:12345678-1234-1234-1234-12345678000{volume}</dc:identifier>
    <dc:title>Cilt {volume}</dc:title>
    <dc:language>tr</dc:language>
  </metadata>
  <manifest>
    <item id="nav" href="nav.xhtml" media-type="application/xhtml+xml" properties="nav"/>
    <item id="ch1" href="ch1.xhtml" media-type="application/xhtml+xml"/>
    <item id="ch2" href="ch2.xhtml" media-type="application/xhtml+xml"/>
    {ch3_item}
    <item id="img1" href="img1.png" media-type="image/png" properties="cover-image"/>
    <item id="img2" href="img2.png" media-type="image/png"/>
  </manifest>
  <spine>
    <itemref idref="ch1"/>
    <itemref idref="ch2"/>
    {ch3_ref}
  </spine>
</package>
"""
    nav = f"""<?xml version="1.0" encoding="utf-8"?>
<!DOCTYPE html>
<html xmlns="http://www.w3.org/1999/xhtml" xmlns:epub="http://www.idpf.org/2007/ops" lang="tr">
<head><meta charset="utf-8"/><title>İçindekiler</title></head>
<body>
  <nav epub:type="toc">
    <ol>
      <li><a href="ch1.xhtml">Bölüm 1</a></li>
      <li><a href="ch2.xhtml">Bölüm 2</a></li>
      {nav_ch3}
    </ol>
  </nav>
</body>
</html>
"""
    container = """<?xml version="1.0" encoding="utf-8"?>
<container version="1.0" xmlns="urn:oasis:names:tc:opendocument:xmlns:container">
  <rootfiles>
    <rootfile full-path="content.opf" media-type="application/oebps-package+xml"/>
  </rootfiles>
</container>
"""
    with zipfile.ZipFile(path, "w") as zf:
        zf.writestr(zipfile.ZipInfo("mimetype"), b"application/epub+zip", compress_type=zipfile.ZIP_STORED)
        zf.writestr("META-INF/container.xml", container.encode("utf-8"))
        zf.writestr("content.opf", opf.encode("utf-8"))
        zf.writestr("nav.xhtml", nav.encode("utf-8"))
        zf.writestr("ch1.xhtml", chapters[0].encode("utf-8"))
        zf.writestr("ch2.xhtml", chapters[1].encode("utf-8"))
        if count == 3:
            zf.writestr("ch3.xhtml", chapters[2].encode("utf-8"))
        zf.writestr("img1.png", make_png(64, 64, (103, 80, 164)))
        zf.writestr("img2.png", make_png(64, 64, (194, 94, 0)))


def build_webtoon_cbz(path):
    """Dikey mod için 8 uzun sayfa (600x1800)."""
    with zipfile.ZipFile(path, "w", compression=zipfile.ZIP_DEFLATED) as zf:
        for idx in range(1, 9):
            # son sayfa kısadır (600x300): alta kaydırınca görünür sayfa o olmalı (21.1)
            height = 300 if idx == 8 else 1800
            zf.writestr(f"p{idx}.png", make_png(600, height, (20 * idx, 80, 160)))


def main():
    if len(sys.argv) < 2:
        sys.exit("Kullanım: make_books.py <çıktı_dizini>")
    out_dir = sys.argv[1]
    series_dir = os.path.join(out_dir, "Test Serisi")
    os.makedirs(series_dir, exist_ok=True)
    build_cbz(os.path.join(series_dir, "deneme.cbz"))
    build_epub(os.path.join(series_dir, "deneme.epub"))

    long_dir = os.path.join(out_dir, "Üçüncü Uzun Seri")
    os.makedirs(long_dir, exist_ok=True)
    for volume in (1, 2):
        build_volume_epub(os.path.join(long_dir, f"Cilt {volume}.epub"), volume)

    tall_dir = os.path.join(out_dir, "Üçüncü Dikey")
    os.makedirs(tall_dir, exist_ok=True)
    build_webtoon_cbz(os.path.join(tall_dir, "dikey.cbz"))


if __name__ == "__main__":
    main()
