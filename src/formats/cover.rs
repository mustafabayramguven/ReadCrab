use std::{
    io::Cursor,
    path::{Path, PathBuf},
};

use image::{
    codecs::jpeg::JpegEncoder, metadata::Orientation, DynamicImage, ImageDecoder, ImageFormat,
    ImageReader, Limits, Rgb, RgbImage,
};
use tokio::sync::Semaphore;

use crate::{
    formats::{
        comic::{comic_pages, comic_read},
        epub::{epub_asset, epub_open},
        mobi::mobi_open,
        Format, MAX_ENTRY_BYTES, MAX_PIXELS,
    },
    safepath::resolve_book_path,
};

/// Özel kapak kodlama aynı anda tek istek çalışır, diğerleri sıraya girer.
pub static COVER_ENCODE: Semaphore = Semaphore::const_new(1);

/// `{id}.{rastgele}.tmp`: eşzamanlı yazımlar birbirinin dosyasını ezmez.
pub fn unique_tmp(dir: &Path, id: i64) -> PathBuf {
    dir.join(format!("{id}.{:016x}.tmp", rand::random::<u64>()))
}

/// Geçici dosyaya yazar; hata olursa dosyayı siler.
pub fn write_tmp(dir: &Path, id: i64, bytes: &[u8]) -> std::io::Result<PathBuf> {
    std::fs::create_dir_all(dir)?;
    let tmp = unique_tmp(dir, id);
    if let Err(e) = std::fs::write(&tmp, bytes) {
        let _ = std::fs::remove_file(&tmp);
        return Err(e);
    }
    Ok(tmp)
}

/// Geçici dosyayı yayımlar (rename); hata olursa geçici dosyayı siler.
pub fn publish_tmp(tmp: &Path, dst: &Path) -> std::io::Result<()> {
    std::fs::rename(tmp, dst).inspect_err(|_| {
        let _ = std::fs::remove_file(tmp);
    })
}

/// Başlangıçta yarım kalmış `*.tmp` dosyalarını temizler.
pub fn remove_stale_tmp(dir: &Path) {
    let Ok(rd) = std::fs::read_dir(dir) else {
        return;
    };
    for e in rd.flatten() {
        if e.path().extension().is_some_and(|x| x == "tmp") {
            let _ = std::fs::remove_file(e.path());
        }
    }
}

#[derive(Clone, Debug)]
pub struct BookRow {
    pub id: i64,
    pub rel_path: String,
    pub format: Format,
}

/// Kapağı üretip geçici dosyaya yazar; yayımlamak çağıranın işidir (`cover_lock` altında).
pub async fn render_cover(root: &Path, book: &BookRow, covers_dir: &Path) -> Option<PathBuf> {
    let Ok(full_path) = resolve_book_path(root, &book.rel_path) else {
        return None;
    };

    let raw_bytes = match book.format {
        Format::Cbz | Format::Zip | Format::Rar | Format::Cbr => {
            let Ok(pages) = comic_pages(&full_path, book.format).await else {
                return None;
            };
            let first = pages.first()?;
            let Ok(bytes) = comic_read(&full_path, book.format, first, MAX_ENTRY_BYTES).await
            else {
                return None;
            };
            bytes
        }
        Format::Epub => {
            let p = full_path.clone();
            let Ok(Some(bytes)) = tokio::task::spawn_blocking(move || {
                let info = epub_open(&p).ok()?;
                let cover_href = info.cover.as_deref()?;
                let (bytes, _) = epub_asset(&p, &info, cover_href).ok()?;
                Some(bytes)
            })
            .await
            else {
                return None;
            };
            bytes
        }
        Format::Mobi => {
            let p = full_path.clone();
            let Ok(Some(bytes)) = tokio::task::spawn_blocking(move || {
                let m = mobi_open(&p).ok()?;
                m.images.into_iter().next().map(Vec::from)
            })
            .await
            else {
                return None;
            };
            bytes
        }
    };

    let out_path = unique_tmp(covers_dir, book.id);
    let tmp = out_path.clone();
    let ok = tokio::task::spawn_blocking(move || resize_and_save_jpeg(&raw_bytes, &out_path))
        .await
        .unwrap_or(false);
    if !ok {
        let _ = std::fs::remove_file(&tmp);
        return None;
    }
    Some(tmp)
}

/// Üret ve doğrudan yayımla (kilitsiz; yarış olmayan bağlamlar ve testler için).
pub async fn extract_cover(root: &Path, book: &BookRow, covers_dir: &Path) -> bool {
    let Some(tmp) = render_cover(root, book, covers_dir).await else {
        return false;
    };
    publish_tmp(&tmp, &covers_dir.join(format!("{}.jpg", book.id))).is_ok()
}

fn resize_and_save_jpeg(raw_bytes: &[u8], out_path: &Path) -> bool {
    let Ok(dim_reader) = ImageReader::new(Cursor::new(raw_bytes)).with_guessed_format() else {
        return false;
    };
    let Ok((w, h)) = dim_reader.into_dimensions() else {
        return false;
    };
    if w == 0 || h == 0 || (w as u64) * (h as u64) > MAX_PIXELS {
        return false;
    }

    let Ok(mut reader) = ImageReader::new(Cursor::new(raw_bytes)).with_guessed_format() else {
        return false;
    };
    let mut limits = Limits::default();
    limits.max_alloc = Some(256 * 1024 * 1024);
    limits.max_image_width = Some(16384);
    limits.max_image_height = Some(16384);
    reader.limits(limits);

    let Ok(img) = reader.decode() else {
        return false;
    };

    let new_h = ((h as u64 * 300) / (w as u64)).max(1) as u32;
    let thumb = img.thumbnail_exact(300, new_h);
    let rgb = thumb.to_rgb8();

    let mut jpeg_buf = Vec::new();
    let mut encoder = JpegEncoder::new_with_quality(&mut jpeg_buf, 80);
    if encoder.encode_image(&rgb).is_err() {
        return false;
    }

    if let Some(parent) = out_path.parent() {
        if std::fs::create_dir_all(parent).is_err() {
            return false;
        }
    }
    std::fs::write(out_path, jpeg_buf).is_ok()
}

/// Yüklenen kapağı çözer, alfayı beyaza birleştirir, 600 pikselden geniş ise küçültür ve JPEG (q85) üretir.
#[allow(clippy::result_unit_err)]
pub fn encode_custom_cover(raw: &[u8]) -> Result<Vec<u8>, ()> {
    let reader = ImageReader::new(Cursor::new(raw))
        .with_guessed_format()
        .map_err(|_| ())?;
    if !matches!(
        reader.format(),
        Some(ImageFormat::Jpeg | ImageFormat::Png | ImageFormat::WebP)
    ) {
        return Err(());
    }
    let mut reader = reader;
    let mut limits = Limits::default();
    limits.max_alloc = Some(256 * 1024 * 1024);
    limits.max_image_width = Some(16384);
    limits.max_image_height = Some(16384);
    reader.limits(limits);
    let (w, h) = ImageReader::new(Cursor::new(raw))
        .with_guessed_format()
        .map_err(|_| ())?
        .into_dimensions()
        .map_err(|_| ())?;
    if w == 0 || h == 0 || u64::from(w) * u64::from(h) > MAX_PIXELS {
        return Err(());
    }
    let mut decoder = reader.into_decoder().map_err(|_| ())?;
    // EXIF yönü: etiket okunamazsa dönüştürme yapılmaz
    let orientation = decoder.orientation().unwrap_or(Orientation::NoTransforms);
    let mut img = DynamicImage::from_decoder(decoder).map_err(|_| ())?;
    img.apply_orientation(orientation);
    let (w, h) = (img.width(), img.height());

    let rgba = img.to_rgba8();
    let mut rgb = RgbImage::new(w, h);
    for (x, y, px) in rgba.enumerate_pixels() {
        let a = u32::from(px[3]);
        let mix = |c: u8| ((u32::from(c) * a + 255 * (255 - a) + 127) / 255) as u8;
        rgb.put_pixel(x, y, Rgb([mix(px[0]), mix(px[1]), mix(px[2])]));
    }
    let rgb = if w > 600 {
        let new_h = ((u64::from(h) * 600) / u64::from(w)).max(1) as u32;
        image::imageops::thumbnail(&rgb, 600, new_h)
    } else {
        rgb
    };

    let mut out = Vec::new();
    JpegEncoder::new_with_quality(&mut out, 85)
        .encode_image(&rgb)
        .map_err(|_| ())?;
    Ok(out)
}

#[cfg(test)]
mod tests {
    use std::{fs::File, io::Write};

    use image::{ImageFormat, Rgb, RgbImage};
    use zip::{write::SimpleFileOptions, ZipWriter};

    use super::*;

    fn test_png(w: u32, h: u32) -> Vec<u8> {
        let img = RgbImage::from_fn(w, h, |_, _| Rgb([10, 20, 30]));
        let mut buf = Vec::new();
        img.write_to(&mut Cursor::new(&mut buf), ImageFormat::Png)
            .unwrap();
        buf
    }

    // EXIF yön etiketi (Orientation=6) olan JPEG: APP1 bölümü SOI'den hemen sonra eklenir.
    fn jpeg_with_orientation(w: u32, h: u32, orientation: u8) -> Vec<u8> {
        let img = RgbImage::from_fn(w, h, |_, _| Rgb([200, 30, 30]));
        let mut plain = Vec::new();
        JpegEncoder::new_with_quality(&mut plain, 90)
            .encode_image(&img)
            .unwrap();
        let mut tiff = vec![0x49, 0x49, 0x2A, 0x00, 0x08, 0x00, 0x00, 0x00, 0x01, 0x00];
        tiff.extend_from_slice(&[0x12, 0x01, 0x03, 0x00, 0x01, 0x00, 0x00, 0x00]);
        tiff.extend_from_slice(&[orientation, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00]);
        let mut seg = b"Exif\0\0".to_vec();
        seg.extend_from_slice(&tiff);
        let len = (seg.len() + 2) as u16;
        let mut out = vec![0xFF, 0xD8, 0xFF, 0xE1];
        out.extend_from_slice(&len.to_be_bytes());
        out.extend_from_slice(&seg);
        out.extend_from_slice(&plain[2..]);
        out
    }

    #[test]
    fn custom_cover_applies_exif_orientation() {
        let raw = jpeg_with_orientation(40, 20, 6);
        let out = encode_custom_cover(&raw).unwrap();
        let img = image::load_from_memory(&out).unwrap();
        assert_eq!((img.width(), img.height()), (20, 40));
        // etiketsiz kaynak döndürülmez
        let plain = encode_custom_cover(&jpeg_with_orientation(40, 20, 1)).unwrap();
        let img = image::load_from_memory(&plain).unwrap();
        assert_eq!((img.width(), img.height()), (40, 20));
    }

    #[tokio::test]
    async fn extract_cover_resizes_to_300px_jpeg() {
        let root = tempfile::tempdir().unwrap();
        let covers = tempfile::tempdir().unwrap();
        let cbz_path = root.path().join("book.cbz");
        let f = File::create(&cbz_path).unwrap();
        let mut zip = ZipWriter::new(f);
        zip.start_file("01.png", SimpleFileOptions::default())
            .unwrap();
        zip.write_all(&test_png(600, 900)).unwrap();
        zip.finish().unwrap();

        let book = BookRow {
            id: 7,
            rel_path: "book.cbz".to_string(),
            format: Format::Cbz,
        };
        assert!(extract_cover(root.path(), &book, covers.path()).await);

        let cover_file = covers.path().join("7.jpg");
        assert!(cover_file.exists());
        let bytes = std::fs::read(&cover_file).unwrap();
        assert_eq!(&bytes[..2], &[0xFF, 0xD8]);
        let decoded = image::load_from_memory(&bytes).unwrap();
        assert_eq!(decoded.width(), 300);
        assert_eq!(decoded.height(), 450);
    }
}
