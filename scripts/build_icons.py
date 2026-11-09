"""web/icons.svg sprite üretici.

Kaynak: Material Symbols, Google, Apache License 2.0,
https://github.com/google/material-design-icons
Paket: @material-symbols/svg-400@0.48.3 (outlined), jsDelivr üzerinden.
Lisans metni: https://www.apache.org/licenses/LICENSE-2.0 (web/ICONS-LICENSE.txt).

Kullanım (depo kökünden): python3 -I scripts/build_icons.py web/icons.svg
"""
import os
import re
import sys
import tempfile
import urllib.request

BASE = "https://cdn.jsdelivr.net/npm/@material-symbols/svg-400@0.48.3/outlined/"
NAMES = [
    "menu_book", "history", "bar_chart", "person", "admin_panel_settings",
    "menu_book-fill", "history-fill", "bar_chart-fill", "person-fill", "admin_panel_settings-fill",
    "search", "close", "arrow_back", "arrow_forward", "chevron_left", "chevron_right",
    "add", "delete", "check", "check_circle", "logout", "lock", "error", "sync", "sort",
    "more_vert", "keyboard_arrow_up",
    "format_list_bulleted", "text_fields", "tune", "fit_screen", "fullscreen", "auto_stories",
    "light_mode", "dark_mode", "palette",
    "schedule", "flag", "local_fire_department",
    "bookmark", "bookmark-fill", "fullscreen_exit", "image",
]


def main():
    out = sys.argv[1]
    work = tempfile.mkdtemp()
    symbols = []
    for name in NAMES:
        raw = os.path.join(work, name + ".svg")
        urllib.request.urlretrieve(BASE + name + ".svg", raw)
        with open(raw, encoding="utf-8") as f:
            text = f.read()
        if text.count("<path") != 1:
            sys.exit(f"{name}: tek <path> yok")
        m = re.search(r'<path d="([^"]+)"', text)
        if not m:
            sys.exit(f"{name}: d özniteliği yok")
        symbols.append(f'<symbol id="{name}" viewBox="0 -960 960 960"><path d="{m.group(1)}"/></symbol>')
    with open(out, "w", encoding="utf-8") as f:
        f.write('<svg xmlns="http://www.w3.org/2000/svg">\n')
        f.write("\n".join(symbols) + "\n")
        f.write("</svg>\n")
    print(out, len(NAMES), "symbol")


if __name__ == "__main__":
    main()
