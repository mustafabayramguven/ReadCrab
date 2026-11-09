Okuyucu yazı tipleri (web/fonts/, 30 dosya, toplam 1.488.592 bayt)

Yeniden indirme: boş bir geçici dizin kullan (mktemp -d), dosyaları oraya indir,
her birinin ilk 4 baytının "wOF2" olduğunu ve boyutunu doğrula, sonra web/fonts/'a kopyala.
Dosyalar web/fonts/ içinde değiştirilmeden tutulur.

Taban URL: https://cdn.jsdelivr.net/npm/<paket>@<sürüm>/files/<ad>

Paketler (OFL-1.1, lisans: web/fonts/LICENSE-reader.txt):
- @fontsource-variable/literata@5.3.0: literata-{latin,latin-ext}-wght-{normal,italic}.woff2
- @fontsource-variable/merriweather@5.3.0: merriweather-{latin,latin-ext}-wght-{normal,italic}.woff2
- @fontsource-variable/source-serif-4@5.3.1: source-serif-4-{latin,latin-ext}-wght-{normal,italic}.woff2
- @fontsource-variable/eb-garamond@5.3.0: eb-garamond-{latin,latin-ext}-wght-{normal,italic}.woff2
- @fontsource-variable/inter@5.3.0: inter-{latin,latin-ext}-wght-{normal,italic}.woff2
- @fontsource-variable/lexend@5.3.0: lexend-{latin,latin-ext}-wght-normal.woff2
- @fontsource/atkinson-hyperlegible@5.3.0: atkinson-hyperlegible-{latin,latin-ext}-{400,700}-{normal,italic}.woff2

web/reader-fonts.css bu dosyaları unicode-range ve font-weight değerleriyle bağlar.
Atkinson'un statik CSS'inde (5.3.0) unicode-range yoktur; aynı alt küme tanımları kullanıldı.
