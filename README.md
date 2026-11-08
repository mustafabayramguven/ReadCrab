# kavita-rs

Kendi sunucunuzda çalışan çizgi roman ve e-kitap kütüphanesi. Kavita ile bağlantısı yoktur.

## Çalıştırma

```
docker compose up -d
```

`http://localhost:3435` adresini açın. İlk açılışta kurulum ekranı gelir: ilk oluşturulan hesap yöneticidir, kayıt bundan sonra kapanır.

Kitaplarınızı `./books` altına koyun; her seri için bir klasör oluşturun. Tarama yönetici tarafından Kütüphane sayfasından başlatılır.

## Ortam değişkenleri

| Değişken | Varsayılan | Anlam |
|---|---|---|
| `PORT` | `3435` | Dinleme portu, `0.0.0.0` |
| `DATA_DIR` | `/data` | `kavita.db`, `covers/` |
| `BOOKS_DIR` | `/books` | Salt okunur kütüphane |
| `COOKIE_SECURE` | `0` | `1` ise çereze `Secure` eklenir (HTTPS arkasında) |
| `RUST_LOG` | `info` | tracing filtresi |

## Notlar

- HTTPS arkasında çalıştırıyorsanız `COOKIE_SECURE=1` kullanın.
- Yönetici parolasını unutursanız kurtarma yolu yoktur; `DATA_DIR/kavita.db` silinerek sıfırdan kurulum yapılır.
- MOBI desteği klasik MOBI içindir. KF8-only/AZW3 dosyaları desteklenmez.
- EPUB'ın kendi CSS'i uygulanmaz; SVG içindeki kapak görselleri `<img>` olarak gösterilir.
- RAR/CBR okuma `bsdtar` ile yapılır.

## Lisans

GPL-3.0. Tam metin `LICENSE` dosyasındadır.
