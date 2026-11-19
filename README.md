# ReadCrab

Kendi sunucunuzda çalışan çizgi roman ve e-kitap kütüphanesi.

## Çalıştırma

```
docker compose up -d
```

`http://localhost:3435` adresini açın. İlk açılışta kurulum ekranı gelir: ilk oluşturulan hesap yöneticidir, kayıt bundan sonra kapanır.

Kitaplarınızı `./books` altına koyun; her seri için bir klasör oluşturun. Tarama yönetici tarafından Kütüphane sayfasından başlatılır.

### Güncelleme

```
docker compose pull && docker compose up -d
```

Veritabanı ilk açılışta otomatik olarak yeni şemaya geçer (`user_version` 1'den 2'ye); kitaplar, ilerleme ve hesaplar korunur. Güncellemeden önce `readcrab-data` volume'unun bir yedeğini almanız önerilir.

## Özellikler

- **Okumaya devam:** Kütüphane sayfasının üstünde yarım kalan kitapların rafı.
- **Metin okuyucu:** Sayfa modu (ok tuşları, dokunma) veya kaydırma modu.
- **Okuma ayarları:** Tema (açık, koyu, sepya, saf siyah), 7 yazı tipi, yazı boyutu, satır, kelime ve harf aralığı, kenar boşluğu, hizalama. Ayarlar tarayıcıda saklanır.
- **Saf siyah:** Hesap > Görünüm altında; koyu modda AMOLED için tam siyah zemin.
- **Özel kapak:** Yönetici, seri sayfasından kapağı JPEG, PNG veya WebP ile değiştirebilir. Yeniden tarama özel kapağı ezmez.
- **Okuma takvimi:** Geçmiş sayfasında yıllık ısı haritası, gün süzgeci ve okuma ritmi grafikleri.
- **Kaldığın satır:** Metin kitaplarda kalınan yer ekranın en üstündeki satırın başıdır. Kitap yeniden açılınca o satır en üste gelir, solunda vurgu renginde bir çubuk durur. Çubuk oturum boyunca yerinde kalır; okuyucu kapanınca konum o anki satıra taşınır. Satır başı yazı boyutundan ve ekran genişliğinden bağımsız tutulur: web, Android ve terminal okuyucu aynı satırı bulur.
- **Yer imleri ve kitap içi arama:** Metin kitaplarda (EPUB, MOBI) yer imi ekleme ve arama.
- **Sonraki cilt:** Kitabın sonunda bir sonraki cildi açma kartı.
- **Çizgi roman:** Sağdan sola okuma yönü ve dikey (webtoon) görünüm.
- **Tahmini kalan süre:** Kitabın kendi okuma hızına göre.
- **Tam ekran** düğmesi (tarayıcı destekliyorsa) ve okuma sırasında ekranın açık kalması (tarayıcı destekliyorsa).
- **Ana ekrana ekleme (PWA):** Uygulama manifesti ve simgeleri vardır. Service worker yoktur; uygulama çevrimdışı çalışmaz. Ana ekrana ekleme için tarayıcı güvenli bağlantı (HTTPS veya `localhost`) ister.

## Terminal okuyucu (Linux)

`crates/tui`, Android uygulamasıyla aynı Rust çekirdeğini (`crates/core`) kullanan bir terminal istemcisidir ([ratatui](https://ratatui.rs)). Kütüphane, indirme, çevrimdışı okuma, ilerleme ve yer imi eşitlemesi, okuma süresi ve istatistikler web ve Android ile ortaktır; bir cihazda kalınan yer diğerinde açılırken önerilir.

**Kurulum:** [Releases](https://github.com/mustafabayramguven/ReadCrab/releases) sayfasındaki `readcrab-tui-*-x86_64-linux.tar.gz` arşivini açıp `readcrab-tui` dosyasını `PATH`'teki bir dizine (ör. `~/.local/bin`) koyun. Kaynaktan: `cargo build --release -p readcrab_tui`.

```
readcrab-tui                  # ilk açılışta sunucu adresi, kullanıcı adı ve parola sorulur
readcrab-tui --halfblocks     # grafik protokolü olmayan terminaller için
```

- **Metin kitaplar (EPUB, MOBI):** Bölüm terminal genişliğine dizilir; başlık, liste, alıntı, kalın/italik korunur, görseller `[görsel: …]` olarak gösterilir. Boşluk sonraki sayfa, `t` içindekiler, `m`/`M` yer imi, `/` kitapta arama, `+`/`-` metin genişliği. Kaldığın satırın solunda `▌` görünür.
- **Çizgi roman (CBZ, CBR):** Sayfalar terminalin desteklediği protokolle çizilir (sixel: foot, WezTerm, xterm; kitty; iTerm2), yoksa yarım blok karakterlerle.
- **Çevrimdışı:** Seride `d` kitabı, `D` serinin tamamını indirir. Bağlantı yokken yapılan ilerleme yerelde bekler, bağlantı gelince gönderilir.
- Her ekranda `?` o ekranın tuşlarını gösterir.
- Veri `~/.local/share/readcrab-tui` altında (yalnız kullanıcıya açık) tutulur; `--data-dir` ile değiştirilebilir. Parola saklanmaz, yalnız oturum belirteci saklanır.
- `http://` yalnız yerel ağ adreslerinde kabul edilir; internetteki sunucu için `https://` adresi girilmelidir.

GitHub Actions'ta `tui.yml` her değişiklikte derler ve test eder; `tui-vX.Y.Z` etiketi (`crates/tui/Cargo.toml` sürümüyle aynı olmalı) Linux ikilisini Releases'ta yayınlar.

## Ortam değişkenleri

| Değişken | Varsayılan | Anlam |
|---|---|---|
| `PORT` | `3435` | Dinleme portu, `0.0.0.0` |
| `DATA_DIR` | `/data` | `readcrab.db`, `covers/` |
| `BOOKS_DIR` | `/books` | Salt okunur kütüphane |
| `COOKIE_SECURE` | `0` | `1` ise çereze `Secure` eklenir (HTTPS arkasında) |
| `TRUSTED_PROXIES` | boş | Ters vekilin adresi/ağı, virgülle (ör. `172.16.0.0/12`). Giriş hız sınırı yalnız bu adreslerden gelen isteklerde `X-Forwarded-For`'daki gerçek istemci IP'sini kullanır; boşsa vekil arkasında tüm istemciler tek IP sayılır ve 20 hatalı giriş herkesi 15 dk kilitler |
| `RUST_LOG` | `info` | tracing filtresi |

## Notlar

- HTTPS arkasında çalıştırıyorsanız `COOKIE_SECURE=1` kullanın.
- Ters vekil (reverse proxy, ör. Nginx) arkasında çalıştırıyorsanız `proxy_set_header Host $host;` ayarını ekleyin; aksi halde durum değiştiren istekler `Origin`/`Host` uyuşmazlığı nedeniyle `403 csrf` hatası alır.
- Yönetici parolasını unutursanız kurtarma yolu yoktur; `DATA_DIR/readcrab.db` silinerek sıfırdan kurulum yapılır.
- MOBI desteği klasik MOBI içindir. KF8-only/AZW3 dosyaları desteklenmez.
- EPUB'ın kendi CSS'i uygulanmaz; SVG içindeki kapak görselleri `<img>` olarak gösterilir.
- RAR/CBR okuma `bsdtar` ile yapılır.
- Tema modu (Otomatik, Açık, Koyu), saf siyah ve kaynak renk, Hesap sayfasından (`#/hesap`) seçilir. Seçim tarayıcıda saklanır.
- Hesap sayfasında parolanızı da değiştirebilirsiniz; diğer oturumlarınız kapanır.

## Lisans ve üçüncü taraf varlıkları

- Uygulama kodu: GPL-3.0. Tam metin `LICENSE` dosyasındadır.
- İkonlar: Material Symbols, Apache License 2.0 (`web/ICONS-LICENSE.txt`).
- CI'nin e2e işi uygulamanın ekran görüntülerini `ui-screenshots` artifact'ı olarak saklar.

## Yazı tipleri ve lisansları

- Arayüz yazı tipi: Roboto Flex, SIL Open Font License 1.1 (`web/fonts/LICENSE`).
- Okuyucu yazı tipleri (OFL-1.1, `web/fonts/LICENSE-reader.txt`): Literata, Merriweather, Source Serif 4, EB Garamond, Inter, Atkinson Hyperlegible, Lexend. Dosyalar fontsource paketlerinden (`@fontsource-variable/*`, `@fontsource/atkinson-hyperlegible`) alınmış ve değiştirilmemiştir; bağlantıları `web/reader-fonts.css` tanımlar.
