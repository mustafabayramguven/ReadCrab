import fs from 'node:fs';
import zlib from 'node:zlib';
import { test, expect } from '@playwright/test';
import AxeBuilder from '@axe-core/playwright';

test.describe.configure({ mode: 'serial' });

let sharedSeriesId = null;
let sharedCbzId = null;
let sharedEpubId = null;

function attachConsoleGuards(page, consoleMessages, pageErrors) {
  page.on('console', (msg) => {
    consoleMessages.push(msg.text());
  });
  page.on('pageerror', (err) => {
    pageErrors.push(String(err));
  });
}

function assertNoCspOrPageErrors(consoleMessages, pageErrors) {
  expect(pageErrors).toEqual([]);
  for (const text of consoleMessages) {
    expect(text).not.toContain('Content Security Policy');
    expect(text).not.toContain('Blocked script');
    expect(text).not.toContain('Refused to');
  }
}

async function runAxeCheck(page, label) {
  const results = await new AxeBuilder({ page }).analyze();
  const seriousOrCritical = results.violations.filter(
    (v) => v.impact === 'serious' || v.impact === 'critical'
  );
  expect(
    seriousOrCritical,
    `Axe ihlali (${label}): ` +
      seriousOrCritical.map((v) => `${v.id}: ${v.help}`).join('; ')
  ).toEqual([]);
}

function rgbLuminance(cssColor) {
  const m = String(cssColor || '').match(
    /rgba?\(\s*(\d+)\s*,\s*(\d+)\s*,\s*(\d+)/i
  );
  if (!m) return 1;
  const toLin = (v) => {
    const c = Number(v) / 255;
    return c <= 0.04045 ? c / 12.92 : Math.pow((c + 0.055) / 1.055, 2.4);
  };
  return 0.2126 * toLin(m[1]) + 0.7152 * toLin(m[2]) + 0.0722 * toLin(m[3]);
}


function crc32(buf) {
  let crc = 0xffffffff;
  for (const b of buf) {
    crc ^= b;
    for (let k = 0; k < 8; k++) crc = (crc >>> 1) ^ (0xedb88320 & -(crc & 1));
  }
  return (crc ^ 0xffffffff) >>> 0;
}

function makePng(width, height, rgb) {
  const chunk = (type, data) => {
    const len = Buffer.alloc(4);
    len.writeUInt32BE(data.length);
    const body = Buffer.concat([Buffer.from(type, 'ascii'), data]);
    const crc = Buffer.alloc(4);
    crc.writeUInt32BE(crc32(body));
    return Buffer.concat([len, body, crc]);
  };
  const ihdr = Buffer.alloc(13);
  ihdr.writeUInt32BE(width, 0);
  ihdr.writeUInt32BE(height, 4);
  ihdr[8] = 8;
  ihdr[9] = 2;
  const row = Buffer.concat([Buffer.from([0]), Buffer.from(Array(width).fill(rgb).flat())]);
  const raw = Buffer.concat(Array(height).fill(row));
  return Buffer.concat([
    Buffer.from([0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a]),
    chunk('IHDR', ihdr),
    chunk('IDAT', zlib.deflateSync(raw)),
    chunk('IEND', Buffer.alloc(0))
  ]);
}

const JSON_HEADERS = { 'Content-Type': 'application/json' };

async function loginContext(browser, viewport, colorScheme = 'light') {
  const context = await browser.newContext({ viewport, colorScheme });
  const res = await context.request.post('/api/login', {
    data: { username: 'e2e', password: 'e2e-pass-123' },
    headers: JSON_HEADERS
  });
  expect(res.ok()).toBeTruthy();
  const page = await context.newPage();
  await page.emulateMedia({ colorScheme });
  const consoleMessages = [];
  const pageErrors = [];
  attachConsoleGuards(page, consoleMessages, pageErrors);
  return { context, page, consoleMessages, pageErrors };
}

// Kitapları başlıkla bulur ('deneme' iki biçimde var: deneme.cbz / deneme.epub).
async function lookupBooks(request) {
  const list = await (await request.get('/api/series?limit=200')).json();
  const out = {};
  for (const s of list.items) {
    const d = await (await request.get(`/api/series/${s.id}`)).json();
    for (const b of d.books) {
      const key = b.title === 'deneme' ? `deneme.${b.format}` : b.title;
      out[key] = { id: b.id, seriesId: s.id, format: b.format };
    }
  }
  return out;
}

async function resetProgress(request, id, locator, percent) {
  const res = await request.put(`/api/books/${id}/progress`, {
    data: { locator, percent, local_date: new Date().toISOString().slice(0, 10) },
    headers: JSON_HEADERS
  });
  expect(res.ok()).toBeTruthy();
}

async function openReaderSettings(page) {
  await page.mouse.move(120, 120);
  await page.mouse.move(140, 130);
  await page.getByRole('button', { name: 'Ayarlar' }).click();
  await expect(page.locator('dialog.reader-settings[open]')).toBeVisible();
}

async function closeSheet(page) {
  await page.keyboard.press('Escape');
  await expect(page.locator('dialog[open]')).toHaveCount(0);
}

async function pickRadio(page, name) {
  await page.getByRole('radio', { name, exact: true }).click();
}

async function pageInfo(page) {
  const text = (await page.locator('#text-indicator').textContent()) || '';
  const m = /sayfa (\d+)\/(\d+) · %(\d+)/.exec(text);
  return m ? { idx: +m[1], n: +m[2], pct: +m[3], text } : { text };
}

// Yazı tipleri ve yeniden akış oturana kadar bekler.
async function settleFrame(page) {
  await page
    .frameLocator('iframe.reader-frame')
    .locator('html')
    .evaluate(() => document.fonts.ready.then(() => true));
  await page.waitForTimeout(450);
}

test('okuyucu uçtan uca', async ({ page }) => {
  fs.mkdirSync('screenshots', { recursive: true });
  await page.setViewportSize({ width: 1280, height: 800 });
  await page.emulateMedia({ colorScheme: 'light' });

  const consoleMessages = [];
  const pageErrors = [];
  const pingStatuses = [];

  attachConsoleGuards(page, consoleMessages, pageErrors);
  page.on('response', (res) => {
    if (res.url().includes('/api/reading/ping')) {
      pingStatuses.push(res.status());
    }
  });

  // 1. #/kurulum formu ile e2e / e2e-pass-123 kurulumu + kurulum ekran görüntüsü
  await page.goto('/#/kurulum');
  await page.waitForLoadState('networkidle');
  await expect(page.locator('#setup-username')).toBeVisible();
  await page.screenshot({
    path: 'screenshots/kurulum-1280-light.png',
    fullPage: false
  });
  fs.copyFileSync('screenshots/kurulum-1280-light.png', 'screenshots/kurulum.png');
  await runAxeCheck(page, 'kurulum 1280 light');

  await page.route('**/api/series*', (r) =>
    r.fulfill({ json: { total: 0, items: [] } })
  );

  await page.locator('#setup-username').fill('e2e');
  await page.locator('#setup-password').fill('e2e-pass-123');
  await page.getByRole('button', { name: 'Kurulumu tamamla' }).click();

  await expect(page).toHaveURL(/#\/$/);

  // Tarama öncesi boş kütüphane ekran görüntüsü (1280 açık)
  await expect(page.locator('.empty')).toBeVisible();
  await expect(page.locator('.empty h2')).toHaveText('Kütüphane boş');
  await page.waitForLoadState('networkidle');
  await page.screenshot({
    path: 'screenshots/kutuphane-bos-1280-light.png',
    fullPage: false
  });
  fs.copyFileSync(
    'screenshots/kutuphane-bos-1280-light.png',
    'screenshots/kutuphane-bos.png'
  );
  await runAxeCheck(page, 'kutuphane-bos 1280 light');

  // Arama sonuçsuz durumu ekran görüntüsü (kutuphane-sonuc-yok)
  await page
    .getByRole('searchbox', { name: 'Kütüphanede ara' })
    .fill('olmayan');
  await expect(page.locator('.empty h2')).toHaveText('Sonuç bulunamadı');
  await expect(page.locator('.empty')).toBeVisible();
  await page.waitForLoadState('networkidle');
  await page.screenshot({
    path: 'screenshots/kutuphane-sonuc-yok-1280-light.png',
    fullPage: false
  });
  fs.copyFileSync(
    'screenshots/kutuphane-sonuc-yok-1280-light.png',
    'screenshots/kutuphane-sonuc-yok.png'
  );
  await runAxeCheck(page, 'kutuphane-sonuc-yok 1280 light');

  await page.unroute('**/api/series*');
  await page.getByRole('button', { name: 'Aramayı temizle' }).click();

  // 2. Arayüzde "Kütüphaneyi tara", /api/scan-status running:false olana kadar yokla
  const scanBtn = page.getByRole('button', { name: 'Kütüphaneyi tara' }).first();
  await expect(scanBtn).toBeVisible();
  await scanBtn.click();

  await expect
    .poll(
      async () => {
        const res = await page.request.get('/api/scan-status');
        if (!res.ok()) return true;
        const data = await res.json();
        return data.running;
      },
      { timeout: 20000 }
    )
    .toBe(false);

  await page.goto('/#/');
  const seriesCard = page.locator('[data-testid="series-card"]', {
    hasText: 'Test Serisi'
  });
  await expect(seriesCard).toBeVisible();

  const href = await seriesCard.getAttribute('href');
  sharedSeriesId = href.replace('#/seri/', '');

  await seriesCard.click();
  await expect(page.locator('.list-item')).toHaveCount(2);

  const seriesRes = await page.request.get(`/api/series/${sharedSeriesId}`);
  const seriesData = await seriesRes.json();
  expect(seriesData.books.length).toBe(2);
  for (const b of seriesData.books) {
    expect(b.title).toContain('deneme');
    if (b.format === 'cbz') sharedCbzId = b.id;
    if (b.format === 'epub') sharedEpubId = b.id;
  }
  expect(sharedCbzId).toBeTruthy();
  expect(sharedEpubId).toBeTruthy();

  // 3. CBZ aç (#/oku/{id}), ArrowRight ile ikinci sayfaya geç, progress locator "1" bekle, yenile, "2 / 3" doğrula
  await page.goto(`/#/oku/${sharedCbzId}`);
  const indicator = page.locator('#page-indicator');
  await expect(indicator).toHaveText('1 / 3');

  await page.waitForTimeout(1100);
  await page.keyboard.press('ArrowRight');
  await expect(indicator).toHaveText('2 / 3');

  await expect
    .poll(
      async () => {
        const r = await page.request.get(`/api/books/${sharedCbzId}/progress`);
        const j = await r.json();
        return j.locator;
      },
      { timeout: 15000 }
    )
    .toBe('1');

  await page.reload();
  await expect(page.locator('#page-indicator')).toHaveText('2 / 3');

  // 4. EPUB aç, iframe içinde scrollingElement.scrollTop yaz, PUT progress bekle, yenile, ±5 px doğrula
  await page.goto(`/#/oku/${sharedEpubId}`);
  const frameEl = page.locator('iframe.reader-frame');
  await expect(frameEl).toBeVisible();

  const frame = page.frameLocator('iframe.reader-frame');
  await expect(frame.locator('h1')).toContainText('Birinci Bölüm');

  const prevChBtn = page.getByRole('button', { name: 'Önceki bölüm' });
  const nextChBtn = page.getByRole('button', { name: 'Sonraki bölüm' });
  await expect(prevChBtn).toBeDisabled();
  await expect(nextChBtn).toBeEnabled();

  const targetScroll = await frame.locator('html').evaluate(() => {
    const se = document.scrollingElement || document.documentElement || document.body;
    const maxS = Math.max(0, se.scrollHeight - se.clientHeight);
    const desired = Math.min(300, Math.floor(maxS * 0.4));
    se.scrollTop = desired;
    window.dispatchEvent(new Event('scroll'));
    return se.scrollTop;
  });
  expect(targetScroll).toBeGreaterThan(20);

  await expect
    .poll(
      async () => {
        const r = await page.request.get(`/api/books/${sharedEpubId}/progress`);
        const j = await r.json();
        if (!j.locator) return 0;
        try {
          return JSON.parse(j.locator).p || 0;
        } catch {
          return 0;
        }
      },
      { timeout: 15000 }
    )
    .toBeGreaterThan(0);

  await page.reload();
  await expect(page.frameLocator('iframe.reader-frame').locator('h1')).toContainText(
    'Birinci Bölüm'
  );

  await expect
    .poll(
      async () => {
        const cur = await page
          .frameLocator('iframe.reader-frame')
          .locator('html')
          .evaluate(() => {
            const se =
              document.scrollingElement || document.documentElement || document.body;
            return se.scrollTop;
          });
        return Math.abs(cur - targetScroll) <= 5;
      },
      { timeout: 10000 }
    )
    .toBe(true);

  // Son bölümde "Sonraki bölüm" disabled olmalı (Madde 18)
  await page.mouse.move(120, 120);
  await page.getByRole('button', { name: 'Sonraki bölüm' }).click();
  await expect(page.frameLocator('iframe.reader-frame').locator('h1')).toContainText(
    'İkinci Bölüm'
  );
  await expect(page.getByRole('button', { name: 'Sonraki bölüm' })).toBeDisabled();
  await expect(page.getByRole('button', { name: 'Önceki bölüm' })).toBeEnabled();

  // İlk bölüme geri al (sonraki görsel testler için)
  await page.getByRole('button', { name: 'Önceki bölüm' }).click();
  await expect(page.frameLocator('iframe.reader-frame').locator('h1')).toContainText(
    'Birinci Bölüm'
  );

  // Okuyucudan hemen çıkıldığında body.reading kalmadığı (Madde 16)
  await page.evaluate((cbzId) => {
    window.location.hash = '#/oku/' + cbzId;
    window.location.hash = '#/gecmis';
  }, sharedCbzId);
  await expect(page.locator('body')).not.toHaveClass(/reading/);
  await page.waitForTimeout(300);
  await expect(page.locator('body')).not.toHaveClass(/reading/);

  // 7. #/gecmis sayfasında en az bir satır deneme ve süre metni içerir
  await page.goto('/#/gecmis');
  const historyTable = page.locator('#history-table');
  await expect(historyTable).toBeVisible();
  const firstHistoryRow = historyTable.locator('li', { hasText: 'deneme' }).first();
  await expect(firstHistoryRow).toBeVisible();
  const historyRowText = await firstHistoryRow.textContent();
  expect(historyRowText).toMatch(/\b\d+\s*(sn|dk|sa)\b/);

  // 6. /api/reading/ping yanıtlarından en az biri 204 olmalı
  expect(pingStatuses).toContain(204);

  // 8. #/istatistik sayfasında sıfır çubuk yokluğu ve en az bir veri rect'i (rect:not(.hit), sıfır yüksekliksiz)
  await page.goto('/#/istatistik');
  const dataRects = page.locator('.chart-card svg rect:not(.hit)');
  await expect(dataRects.first()).toBeVisible();
  const rectCount = await dataRects.count();
  expect(rectCount).toBeGreaterThanOrEqual(1);
  // 2 grafik x 30 gün = 60 gün dilimi içinde yalnız bugünün verisi var, sıfır değerli günler için rect çizilmemiş olmalı
  expect(rectCount).toBeLessThan(10);
  for (let i = 0; i < rectCount; i++) {
    const r = dataRects.nth(i);
    const h = Number(await r.getAttribute('height'));
    expect(h).toBeGreaterThan(0);
    const titleText = await r.locator('title').textContent();
    expect(titleText).not.toMatch(/:\s*0\s+/);
  }

  // 5. Konsol mesajlarında CSP / Blocked script / Refused to geçmez; pageerror 0
  assertNoCspOrPageErrors(consoleMessages, pageErrors);
});

test('görünüm ve erişilebilirlik', async ({ browser }) => {
  fs.mkdirSync('screenshots', { recursive: true });

  const viewports = [
    { width: 360, height: 800 },
    { width: 1280, height: 800 }
  ];
  const schemes = ['light', 'dark'];

  const bookCtx = await browser.newContext();
  const bookLogin = await bookCtx.request.post('/api/login', {
    data: { username: 'e2e', password: 'e2e-pass-123' },
    headers: JSON_HEADERS
  });
  expect(bookLogin.ok()).toBeTruthy();
  const bookMap = await lookupBooks(bookCtx.request);
  await bookCtx.close();
  const cilt1 = bookMap['Cilt 1'].id;
  const dikey = bookMap['dikey'].id;

  const screens = [
    {
      name: 'kutuphane',
      hash: '/#/',
      waitSelector: '[data-testid="series-card"]',
      isReader: false
    },
    {
      name: 'seri',
      hash: `/#/seri/${sharedSeriesId}`,
      waitSelector: '.list-item',
      isReader: false
    },
    {
      name: 'seri-durum-dialog',
      hash: `/#/seri/${sharedSeriesId}`,
      waitSelector: '.list-item',
      isReader: false,
      openAction: async (page) => {
        await page
          .getByRole('button', { name: 'Durumu değiştir' })
          .first()
          .click();
        await expect(page.locator('dialog.md-dialog[open]')).toBeVisible();
      }
    },
    {
      name: 'okuyucu-cbz',
      hash: `/#/oku/${sharedCbzId}`,
      waitSelector: '#page-indicator',
      isReader: true
    },
    {
      name: 'okuyucu-comic-ayarlar',
      hash: `/#/oku/${sharedCbzId}`,
      waitSelector: '#page-indicator',
      isReader: true,
      openAction: async (page) => {
        await page.getByRole('button', { name: 'Ayarlar' }).click();
        await expect(page.locator('dialog.sheet[open]')).toBeVisible();
      }
    },
    {
      name: 'okuyucu-epub',
      hash: `/#/oku/${sharedEpubId}`,
      waitSelector: 'iframe.reader-frame',
      isReader: true
    },
    {
      name: 'okuyucu-epub-ayarlar',
      hash: `/#/oku/${sharedEpubId}`,
      waitSelector: 'iframe.reader-frame',
      isReader: true,
      openAction: async (page) => {
        await page.getByRole('button', { name: 'Ayarlar' }).click();
        await expect(page.locator('dialog.sheet[open]')).toBeVisible();
      }
    },
    {
      name: 'gecmis',
      hash: '/#/gecmis',
      waitSelector: '#history-table li',
      isReader: false
    },
    {
      name: 'kutuphane-raf',
      hash: '/#/',
      waitSelector: '.shelf-card',
      isReader: false
    },
    {
      name: 'okuyucu-sayfa-modu',
      hash: `/#/oku/${cilt1}`,
      waitSelector: 'iframe.reader-frame',
      isReader: true,
      before: async (page) => {
        await page.evaluate(() => {
          localStorage.setItem('krs.readerPrefs', JSON.stringify({ v: 1, mode: 'page' }));
        });
      },
      after: async (page) => {
        await page.evaluate(() => localStorage.removeItem('krs.readerPrefs'));
      }
    },
    {
      name: 'okuyucu-ayarlar-yeni',
      hash: `/#/oku/${cilt1}`,
      waitSelector: 'iframe.reader-frame',
      isReader: true,
      before: async (page) => {
        await page.evaluate(() => localStorage.removeItem('krs.readerPrefs'));
      },
      openAction: async (page) => {
        await page.getByRole('button', { name: 'Ayarlar' }).click();
        await expect(page.locator('dialog.reader-settings[open]')).toBeVisible();
      }
    },
    {
      name: 'gecmis-heatmap',
      hash: '/#/gecmis',
      waitSelector: '.heatmap-scroll svg',
      isReader: false
    },
    {
      name: 'kutuphane-saf-siyah',
      hash: '/#/',
      waitSelector: '[data-testid="series-card"]',
      isReader: false,
      before: async (page) => {
        await page.evaluate(() => window.KRS_THEME.setBlack(true));
      },
      after: async (page) => {
        await page.evaluate(() => window.KRS_THEME.setBlack(false));
      }
    },
    {
      name: 'okuyucu-webtoon',
      hash: `/#/oku/${dikey}`,
      waitSelector: '.webtoon-page',
      isReader: true,
      before: async (page) => {
        await page.evaluate((id) => localStorage.setItem(`krs.cmode.${id}`, 'vertical'), dikey);
      },
      after: async (page) => {
        await page.evaluate((id) => localStorage.removeItem(`krs.cmode.${id}`), dikey);
      }
    },
    {
      name: 'istatistik',
      hash: '/#/istatistik',
      waitSelector: '.chart-card svg',
      isReader: false
    },
    {
      name: 'hesap',
      hash: '/#/hesap',
      waitSelector: '.swatch',
      isReader: false
    },
    {
      name: 'yonetim',
      hash: '/#/yonetim',
      waitSelector: '.list-item',
      isReader: false
    }
  ];

  for (const vp of viewports) {
    for (const colorScheme of schemes) {
      const context = await browser.newContext({ viewport: vp, colorScheme });
      const page = await context.newPage();
      await page.emulateMedia({ colorScheme });

      const consoleMessages = [];
      const pageErrors = [];
      attachConsoleGuards(page, consoleMessages, pageErrors);

      // Oturumsuz giriş ekranı (giris)
      await page.goto('/#/giris');
      await page.waitForLoadState('networkidle');
      await expect(page.locator('#login-username')).toBeVisible();
      await page.screenshot({
        path: `screenshots/giris-${vp.width}-${colorScheme}.png`,
        fullPage: false
      });
      await runAxeCheck(page, `giris ${vp.width} ${colorScheme}`);

      // Giriş yap
      const loginRes = await context.request.post('/api/login', {
        data: { username: 'e2e', password: 'e2e-pass-123' },
        headers: { 'Content-Type': 'application/json' }
      });
      expect(loginRes.ok()).toBeTruthy();

      for (const scr of screens) {
        if (scr.before) {
          await page.goto(scr.hash);
          await scr.before(page);
          await page.reload();
          await page.waitForLoadState('networkidle');
        } else if (!page.url().endsWith(scr.hash)) {
          await page.goto(scr.hash);
          await page.waitForLoadState('networkidle');
        }
        await expect(page.locator(scr.waitSelector).first()).toBeVisible();

        if (
          scr.name === 'okuyucu-epub' ||
          scr.name === 'okuyucu-epub-ayarlar'
        ) {
          await expect(
            page.frameLocator('iframe.reader-frame').locator('h1')
          ).toBeVisible();
        }

        if (scr.isReader) {
          await page.mouse.move(120, 120);
          await expect(page.locator('.reader-bar.top')).not.toHaveClass(/hidden/);
        }

        if (scr.name === 'okuyucu-epub' && colorScheme === 'dark') {
          const frameBg = await page
            .frameLocator('iframe.reader-frame')
            .locator('html')
            .evaluate((el) => getComputedStyle(el).backgroundColor);
          expect(rgbLuminance(frameBg)).toBeLessThan(0.3);
        }

        if (scr.openAction) {
          await scr.openAction(page);
        }

        if (scr.name === 'okuyucu-webtoon') {
          await page.waitForTimeout(600);
        }
        if (scr.name === 'okuyucu-sayfa-modu') {
          await settleFrame(page);
          await expect(page.locator('#text-indicator')).toHaveText(/sayfa 1\/\d+/);
        }

        const shotPath = `screenshots/${scr.name}-${vp.width}-${colorScheme}.png`;
        await page.screenshot({ path: shotPath, fullPage: false });
        await runAxeCheck(page, `${scr.name} ${vp.width} ${colorScheme}`);

        if (scr.openAction) {
          await page.keyboard.press('Escape');
          await expect(page.locator('dialog[open]')).toHaveCount(0);
        }
        if (scr.after) {
          await scr.after(page);
        }
      }

      if (vp.width === 1280 && colorScheme === 'dark') {
        await page.goto('/#/');
        await page.waitForLoadState('networkidle');
        await expect(
          page.locator('[data-testid="series-card"]').first()
        ).toBeVisible();
        await page.evaluate(() => {
          window.KRS_THEME.setSeed('#2E7D32');
        });
        await page.screenshot({
          path: 'screenshots/kutuphane-yesil-1280-dark.png',
          fullPage: false
        });
        fs.copyFileSync(
          'screenshots/kutuphane-yesil-1280-dark.png',
          'screenshots/kutuphane-yesil.png'
        );
        await runAxeCheck(page, 'kutuphane-yesil 1280 dark');
      }

      assertNoCspOrPageErrors(consoleMessages, pageErrors);
      await context.close();
    }
  }
});

test('oturum düşünce 401 döngüsü yok', async ({ browser }) => {
  const context = await browser.newContext({ viewport: { width: 1280, height: 800 } });
  const page = await context.newPage();
  let setupStatusCalls = 0;
  page.on('request', (req) => {
    if (req.url().includes('/api/setup-status')) setupStatusCalls++;
  });

  const loginRes = await context.request.post('/api/login', {
    data: { username: 'e2e', password: 'e2e-pass-123' },
    headers: { 'Content-Type': 'application/json' }
  });
  expect(loginRes.ok()).toBeTruthy();

  await page.goto(`/#/oku/${sharedCbzId}`);
  await expect(page.locator('#page-indicator')).toBeVisible();

  await context.clearCookies();
  setupStatusCalls = 0;
  await page.keyboard.press('ArrowRight');

  await expect(page).toHaveURL(/#\/giris$/);
  await expect(page.locator('#login-username')).toBeVisible();
  await page.waitForTimeout(3000);
  expect(setupStatusCalls).toBeLessThanOrEqual(2);
  await expect(page.locator('#login-username')).toBeVisible();
  await context.close();
});

test('sayfa modu', async ({ browser }) => {
  const { context, page, consoleMessages, pageErrors } = await loginContext(browser, {
    width: 1280,
    height: 800
  });
  const books = await lookupBooks(context.request);
  const id = books['Cilt 1'].id;
  await resetProgress(context.request, id, JSON.stringify({ ch: 0, p: 0 }), 0);

  await page.goto(`/#/oku/${id}`);
  const frame = page.frameLocator('iframe.reader-frame');
  await expect(frame.locator('h1')).toContainText('Bölüm 1');
  await openReaderSettings(page);
  await pickRadio(page, 'Sayfa');
  await closeSheet(page);

  const ind = page.locator('#text-indicator');
  await expect(ind).toHaveText(/^Bölüm 1 · sayfa 1\/\d+ · %\d+$/);
  await settleFrame(page);
  const info0 = await pageInfo(page);
  expect(info0.n).toBeGreaterThan(5);

  // klavye: sonraki, önceki
  await page.keyboard.press('ArrowRight');
  await expect(ind).toHaveText(/^Bölüm 1 · sayfa 2\//);
  await page.keyboard.press('ArrowLeft');
  await expect(ind).toHaveText(/^Bölüm 1 · sayfa 1\//);

  // dokunma bölgeleri: sağ üçte bir sonraki, sol üçte bir önceki sayfa
  await page.waitForTimeout(400);
  const html = frame.locator('html');
  await html.click({ position: { x: 1100, y: 400 } });
  await expect(ind).toHaveText(/^Bölüm 1 · sayfa 2\//);
  await page.waitForTimeout(400);
  await html.click({ position: { x: 100, y: 400 } });
  await expect(ind).toHaveText(/^Bölüm 1 · sayfa 1\//);

  // konum koruma: yazı boyutu 18 -> 24
  for (let i = 0; i < 4; i++) await page.keyboard.press('ArrowRight');
  await expect(ind).toHaveText(/^Bölüm 1 · sayfa 5\//);
  const before = await pageInfo(page);
  const r1 = (before.idx - 1) / (before.n - 1);
  await openReaderSettings(page);
  for (let i = 0; i < 3; i++) {
    await page.getByRole('button', { name: 'Yazıyı büyüt (A+)' }).click();
  }
  await closeSheet(page);
  await settleFrame(page);
  const bigger = await pageInfo(page);
  expect(bigger.n).toBeGreaterThan(before.n);
  const r2 = (bigger.idx - 1) / (bigger.n - 1);
  expect(Math.abs(r2 - r1)).toBeLessThanOrEqual(1 / (bigger.n - 1) + 0.02);

  // pencere 1280 -> 360: oran korunur
  await page.setViewportSize({ width: 360, height: 800 });
  await settleFrame(page);
  const narrow = await pageInfo(page);
  expect(narrow.n).toBeGreaterThan(bigger.n);
  const r3 = (narrow.idx - 1) / (narrow.n - 1);
  expect(Math.abs(r3 - r2)).toBeLessThanOrEqual(1 / (narrow.n - 1) + 0.02);
  await page.setViewportSize({ width: 1280, height: 800 });
  await settleFrame(page);

  // sayfa -> kaydırma: bölüm içi oran korunur (±0.1)
  const beforeMode = await pageInfo(page);
  const rMode = (beforeMode.idx - 1) / (beforeMode.n - 1);
  await openReaderSettings(page);
  await pickRadio(page, 'Kaydırma');
  await closeSheet(page);
  await settleFrame(page);
  const scrollRatio = await html.evaluate(() => {
    const se = document.scrollingElement;
    return se.scrollTop / Math.max(1, se.scrollHeight - se.clientHeight);
  });
  expect(Math.abs(scrollRatio - rMode)).toBeLessThanOrEqual(0.1);
  await expect(ind).toHaveText(/^Bölüm 1 · %\d+$/);

  // kaydırma -> sayfa, ardından sondaki sayfa kontrolleri
  await openReaderSettings(page);
  await pickRadio(page, 'Sayfa');
  await closeSheet(page);
  await settleFrame(page);
  const paged = await pageInfo(page);
  const n = paged.n;
  for (let i = paged.idx; i < n; i++) await page.keyboard.press('ArrowRight');
  await expect(ind).toHaveText(new RegExp(`^Bölüm 1 · sayfa ${n}/${n} · `));
  const geom = await html.evaluate(() => {
    const se = document.scrollingElement;
    const range = document.createRange();
    range.selectNodeContents(document.body);
    const bh = parseFloat(getComputedStyle(document.documentElement).getPropertyValue('--bh'));
    const imgs = [...document.images].map((i) => i.getBoundingClientRect().height);
    return {
      right: range.getBoundingClientRect().right + se.scrollLeft,
      pw: window.innerWidth,
      bh,
      maxImg: Math.max(0, ...imgs)
    };
  });
  // son sütunda metin var: boş son sayfa yok
  expect(geom.right).toBeGreaterThan((n - 1) * geom.pw);
  expect(geom.maxImg).toBeLessThanOrEqual(geom.bh + 0.5);

  // son sayfada sonraki -> sonraki bölümün ilk sayfası; ilk sayfada önceki -> önceki bölümün son sayfası
  await page.keyboard.press('ArrowRight');
  await expect(ind).toHaveText(/^Bölüm 2 · sayfa 1\//);
  await settleFrame(page);
  await page.keyboard.press('ArrowLeft');
  await expect(ind).toHaveText(new RegExp(`^Bölüm 1 · sayfa ${n}/${n} · `));
  await settleFrame(page);

  // azaltılmış hareket: kaydırma davranışı smooth değil, sayfa hemen değişir
  await page.emulateMedia({ reducedMotion: 'reduce' });
  const pre = await pageInfo(page);
  await page.keyboard.press('ArrowLeft');
  const scrollLeft = await html.evaluate(() => document.scrollingElement.scrollLeft);
  expect(scrollLeft).toBe((pre.idx - 2) * 1280);
  const behavior = await html.evaluate(() => getComputedStyle(document.documentElement).scrollBehavior);
  expect(behavior).not.toBe('smooth');

  assertNoCspOrPageErrors(consoleMessages, pageErrors);
  await context.close();
});

test('ayarlar çerçeveye uygulanır', async ({ browser }) => {
  const { context, page, consoleMessages, pageErrors } = await loginContext(browser, {
    width: 1280,
    height: 800
  });
  const books = await lookupBooks(context.request);
  const id = books['Cilt 1'].id;
  await resetProgress(context.request, id, JSON.stringify({ ch: 0, p: 0 }), 0);
  await page.goto(`/#/oku/${id}`);
  const frame = page.frameLocator('iframe.reader-frame');
  await expect(frame.locator('h1')).toContainText('Bölüm 1');
  const html = frame.locator('html');

  await openReaderSettings(page);
  await runAxeCheck(page, 'okuyucu ayarlar sheet');
  await pickRadio(page, 'Lexend');
  expect(await html.evaluate(() => getComputedStyle(document.documentElement).fontFamily)).toContain(
    'Lexend Variable'
  );
  await page.locator('input[aria-label="Satır aralığı"]').fill('2');
  expect(await html.evaluate(() => getComputedStyle(document.documentElement).getPropertyValue('--lh').trim())).toBe('2');
  await pickRadio(page, 'Siyah');
  expect(await html.evaluate(() => getComputedStyle(document.documentElement).backgroundColor)).toBe('rgb(0, 0, 0)');
  await pickRadio(page, 'İki yana');
  expect(await frame.locator('body').evaluate(() => getComputedStyle(document.body).textAlign)).toBe('justify');
  await closeSheet(page);

  const stored = await page.evaluate(() => JSON.parse(localStorage.getItem('krs.readerPrefs')));
  expect(stored).toMatchObject({ v: 1, font: 'lexend', theme: 'black', align: 'justify', lh: 2 });

  // kalıcı: yeniden yüklemede aynı
  await page.reload();
  await expect(page.frameLocator('iframe.reader-frame').locator('h1')).toContainText('Bölüm 1');
  const again = page.frameLocator('iframe.reader-frame').locator('html');
  await expect
    .poll(() => again.evaluate(() => getComputedStyle(document.documentElement).fontFamily))
    .toContain('Lexend Variable');
  expect(await again.evaluate(() => getComputedStyle(document.documentElement).backgroundColor)).toBe('rgb(0, 0, 0)');

  // varsayılana dön
  await openReaderSettings(page);
  await page.getByRole('button', { name: 'Varsayılana dön' }).click();
  const reset = await page.evaluate(() => JSON.parse(localStorage.getItem('krs.readerPrefs')));
  expect(reset).toMatchObject({ font: 'literata', theme: null, align: 'left', lh: 1.7, fs: 18 });
  await closeSheet(page);
  assertNoCspOrPageErrors(consoleMessages, pageErrors);
  await context.close();

  // eski anahtarlardan göç (krs.readerPrefs yok)
  const mig = await loginContext(browser, { width: 1280, height: 800 });
  await mig.page.addInitScript(() => {
    if (!localStorage.getItem('krs.readerPrefs')) {
      localStorage.setItem('krs.fs', '22');
      localStorage.setItem('krs.readerTheme', 'sepia');
    }
  });
  await mig.page.goto(`/#/oku/${id}`);
  await expect(mig.page.frameLocator('iframe.reader-frame').locator('h1')).toContainText('Bölüm 1');
  const migrated = await mig.page.evaluate(() => JSON.parse(localStorage.getItem('krs.readerPrefs')));
  expect(migrated).toMatchObject({ fs: 22, theme: 'sepia' });
  expect(await mig.page.evaluate(() => localStorage.getItem('krs.fs'))).toBe('22');
  expect(
    await mig.page
      .frameLocator('iframe.reader-frame')
      .locator('html')
      .evaluate(() => getComputedStyle(document.documentElement).backgroundColor)
  ).toBe('rgb(244, 236, 216)');
  assertNoCspOrPageErrors(mig.consoleMessages, mig.pageErrors);
  await mig.context.close();
});

test('yazı tipleri yüklenir ve CSP bozulmaz', async ({ browser }) => {
  const { context, page, consoleMessages, pageErrors } = await loginContext(browser, {
    width: 1280,
    height: 800
  });
  const books = await lookupBooks(context.request);
  const id = books['Cilt 1'].id;
  const fontReqs = [];
  page.on('response', (res) => {
    if (res.url().includes('/static/fonts/') && res.url().endsWith('.woff2')) {
      fontReqs.push({ url: res.url(), status: res.status() });
    }
  });
  await page.goto(`/#/oku/${id}`);
  const frame = page.frameLocator('iframe.reader-frame');
  await expect(frame.locator('h1')).toContainText('Bölüm 1');
  await openReaderSettings(page);

  const fonts = [
    ['Literata', 'Literata Variable', 'literata-'],
    ['Merriweather', 'Merriweather Variable', 'merriweather-'],
    ['Source Serif 4', 'Source Serif 4 Variable', 'source-serif-4-'],
    ['EB Garamond', 'EB Garamond Variable', 'eb-garamond-'],
    ['Inter', 'Inter Variable', 'inter-'],
    ['Atkinson Hyperlegible', 'Atkinson Hyperlegible', 'atkinson-hyperlegible-'],
    ['Lexend', 'Lexend Variable', 'lexend-']
  ];
  for (const [label, family, prefix] of fonts) {
    await pickRadio(page, label);
    await frame.locator('html').evaluate(() => document.fonts.ready.then(() => true));
    const ok = await frame
      .locator('html')
      .evaluate((fam) => document.fonts.check(`16px "${fam}"`, 'şğıİ'), family);
    expect(ok, `${family} yüklenmedi`).toBe(true);
    const used = await frame
      .locator('html')
      .evaluate(() => getComputedStyle(document.documentElement).fontFamily);
    expect(used).toContain(family);
    expect(fontReqs.some((r) => r.url.includes(`/static/fonts/${prefix}`)), `${prefix} istenmedi`).toBe(true);
  }
  for (const r of fontReqs) expect(r.status, r.url).toBe(200);
  await closeSheet(page);
  assertNoCspOrPageErrors(consoleMessages, pageErrors);
  await context.close();
});

test('webtoon ve sağdan sola', async ({ browser }) => {
  const { context, page, consoleMessages, pageErrors } = await loginContext(browser, {
    width: 1280,
    height: 800
  });
  const books = await lookupBooks(context.request);
  const id = books['dikey'].id;
  await resetProgress(context.request, id, '0', 0);
  await page.goto(`/#/oku/${id}`);
  await expect(page.locator('#page-indicator')).toHaveText('1 / 8');

  await openReaderSettings(page);
  await pickRadio(page, 'Dikey');
  await closeSheet(page);
  await expect(page.locator('.webtoon-page')).toHaveCount(8);
  expect(await page.evaluate((b) => localStorage.getItem(`krs.cmode.${b}`), id)).toBe('vertical');

  await page.locator('.webtoon').evaluate((el) => {
    el.scrollTop = el.scrollHeight / 2;
  });
  await expect(page.locator('#page-indicator')).toHaveText(/^[2-8] \/ 8$/);
  await expect
    .poll(
      async () => {
        const r = await page.request.get(`/api/books/${id}/progress`);
        return Number((await r.json()).locator);
      },
      { timeout: 15000 }
    )
    .toBeGreaterThan(0);

  // sayfa moduna dön, sağdan sola oku: sol ok sonraki sayfaya götürür
  await openReaderSettings(page);
  await pickRadio(page, 'Sayfa');
  await pickRadio(page, 'Sağdan sola');
  await closeSheet(page);
  expect(await page.evaluate((b) => localStorage.getItem(`krs.dir.${b}`), id)).toBe('rtl');
  const text = (await page.locator('#page-indicator').textContent()) || '';
  const cur = Number(text.split(' / ')[0]);
  expect(cur).toBeLessThan(8);
  await page.keyboard.press('ArrowLeft');
  await expect(page.locator('#page-indicator')).toHaveText(`${cur + 1} / 8`);

  // temizlik
  await page.evaluate((b) => {
    localStorage.removeItem(`krs.cmode.${b}`);
    localStorage.removeItem(`krs.dir.${b}`);
  }, id);
  assertNoCspOrPageErrors(consoleMessages, pageErrors);
  await context.close();
});

test('yer imi, arama ve sonraki cilt', async ({ browser }) => {
  const { context, page, consoleMessages, pageErrors } = await loginContext(browser, {
    width: 1280,
    height: 800
  });
  const books = await lookupBooks(context.request);
  const cilt1 = books['Cilt 1'].id;
  const cilt2 = books['Cilt 2'].id;
  await resetProgress(context.request, cilt1, JSON.stringify({ ch: 0, p: 0 }), 0);

  await page.goto(`/#/oku/${cilt1}`);
  const frame = page.frameLocator('iframe.reader-frame');
  await expect(frame.locator('h1')).toContainText('Bölüm 1');
  await page.mouse.move(120, 120);
  await page.mouse.move(140, 130);

  // yer imi ekle / sheet'te göster / sil
  const toggle = page.locator('.reader-bar.top button[aria-pressed]');
  await expect(toggle).toHaveAttribute('aria-pressed', 'false');
  await toggle.click();
  await expect(toggle).toHaveAttribute('aria-pressed', 'true');
  await page.getByRole('button', { name: 'İçindekiler' }).click();
  await page.getByRole('tab', { name: 'Yer imleri' }).click();
  await expect(page.locator('.bm-row')).toHaveCount(1);
  await page.getByRole('button', { name: /^Yer imini sil/ }).click();
  await expect(page.locator('.bm-row')).toHaveCount(0);
  await expect(page.locator('.rs-empty')).toHaveText('Yer imi yok');
  await expect(toggle).toHaveAttribute('aria-pressed', 'false');

  // arama: "şeker" için işaretli sonuç, tıklayınca sheet kapanır ve konum değişir
  await page.getByRole('tab', { name: 'Ara' }).click();
  await page.getByRole('searchbox', { name: 'Kitapta ara' }).fill('şeker');
  const hit = page.locator('.search-hit').first();
  await expect(hit).toBeVisible();
  await expect(hit.locator('mark')).toHaveText(/şeker/i);
  const beforeText = await page.locator('#text-indicator').textContent();
  await hit.click();
  await expect(page.locator('dialog[open]')).toHaveCount(0);
  await expect(page.locator('#text-indicator')).not.toHaveText(beforeText || '');

  // sonraki cilt: son bölümün sonunda kart
  await page.keyboard.press(']');
  await expect(frame.locator('h1')).toContainText('Bölüm 2');
  await frame.locator('html').evaluate(() => {
    const se = document.scrollingElement;
    se.scrollTop = se.scrollHeight;
  });
  const nextCard = page.locator('a', { hasText: 'Sonraki cilt: Cilt 2' });
  await expect(nextCard).toBeVisible();
  await nextCard.click();
  await expect(page).toHaveURL(new RegExp(`#/oku/${cilt2}$`));

  // görsel okuyucuda yer imi ekle / kaldır
  const cbz = books['deneme.cbz'].id;
  await page.goto(`/#/oku/${cbz}`);
  await expect(page.locator('#page-indicator')).toBeVisible();
  await page.mouse.move(120, 120);
  await page.mouse.move(140, 130);
  const cToggle = page.locator('.reader-bar.top button[aria-pressed]');
  await cToggle.click();
  await expect(cToggle).toHaveAttribute('aria-pressed', 'true');
  await cToggle.click();
  await expect(cToggle).toHaveAttribute('aria-pressed', 'false');

  assertNoCspOrPageErrors(consoleMessages, pageErrors);
  await context.close();
});

test('tam ekran düğmesi ve manifest', async ({ browser }) => {
  const { context, page, consoleMessages, pageErrors } = await loginContext(browser, {
    width: 1280,
    height: 800
  });
  const books = await lookupBooks(context.request);
  await page.goto(`/#/oku/${books['deneme.cbz'].id}`);
  await expect(page.locator('#page-indicator')).toBeVisible();
  await page.mouse.move(120, 120);
  await page.mouse.move(140, 130);
  const supported = await page.evaluate(() => document.fullscreenEnabled);
  if (supported) {
    const fsBtn = page.getByRole('button', { name: 'Tam ekran', exact: true });
    await expect(fsBtn).toBeVisible();
    await fsBtn.click();
    await page.evaluate(() =>
      document.fullscreenElement ? document.exitFullscreen().catch(() => {}) : null
    );
  }
  const manifest = await page.request.get('/static/manifest.webmanifest');
  expect(manifest.status()).toBe(200);
  expect((await manifest.json()).icons.length).toBe(2);
  await expect(page.locator('link[rel="manifest"]')).toHaveCount(1);
  assertNoCspOrPageErrors(consoleMessages, pageErrors);
  await context.close();
});

test('saf siyah anahtarı', async ({ browser }) => {
  const { context, page, consoleMessages, pageErrors } = await loginContext(
    browser,
    { width: 1280, height: 800 },
    'dark'
  );
  await page.goto('/#/hesap');
  const sw = page.locator('#black-switch');
  await expect(sw).toBeVisible();
  const surface = () =>
    page.evaluate(() =>
      getComputedStyle(document.documentElement).getPropertyValue('--md-sys-color-surface').trim()
    );
  const themeColor = () =>
    page.evaluate(() => document.querySelector('meta[name="theme-color"]').getAttribute('content'));
  await sw.check();
  expect(await surface()).toBe('#000000');
  expect(await themeColor()).toBe('#000000');
  expect(await page.evaluate(() => localStorage.getItem('krs.black'))).toBe('1');
  await page.reload();
  await expect(page.locator('#black-switch')).toBeChecked();
  expect(await surface()).toBe('#000000');
  await page.locator('#black-switch').uncheck();
  expect(await surface()).not.toBe('#000000');
  expect(await themeColor()).not.toBe('#000000');
  expect(await page.evaluate(() => localStorage.getItem('krs.black'))).toBe('0');
  assertNoCspOrPageErrors(consoleMessages, pageErrors);
  await context.close();
});

test('özel kapak', async ({ browser }) => {
  const { context, page, consoleMessages, pageErrors } = await loginContext(browser, {
    width: 1280,
    height: 800
  });
  const books = await lookupBooks(context.request);
  const seriesId = books['deneme.cbz'].seriesId;
  const coverImg = () => page.locator('.list-item').first().locator('.list-thumb img');
  const openCoverDialog = async () => {
    await page.getByRole('button', { name: 'Durumu değiştir' }).first().click();
    await page.getByRole('button', { name: 'Kapağı değiştir' }).click();
    await expect(page.locator('#cover-file')).toBeVisible();
  };

  await page.goto(`/#/seri/${seriesId}`);
  await expect(page.locator('.list-item')).toHaveCount(2);
  await expect
    .poll(() => coverImg().evaluate((img) => img.complete && img.naturalWidth))
    .toBeGreaterThan(0);

  await openCoverDialog();
  await page.setInputFiles('#cover-file', {
    name: 'kapak.png',
    mimeType: 'image/png',
    buffer: makePng(800, 1200, [200, 40, 40])
  });
  await expect(page.locator('img.cover-preview')).toBeVisible();
  await page.getByRole('button', { name: 'Yükle' }).click();
  await expect(coverImg()).toHaveAttribute('src', /\?v=[1-9]\d*/);
  await expect
    .poll(() => coverImg().evaluate((img) => img.complete && img.naturalWidth))
    .toBe(600);
  const uploadedSrc = await coverImg().getAttribute('src');

  // kütüphane kartı aynı sürümü gösterir
  await page.goto('/#/');
  const card = page.locator('[data-testid="series-card"]', { hasText: 'Test Serisi' });
  await expect(card).toBeVisible();
  await expect(card.locator('img')).toHaveAttribute('src', /\?v=[1-9]\d*/);

  // özel kapağı kaldır: çıkarılmış kapağa (300 piksel) dönülür, sürüm artar
  await page.goto(`/#/seri/${seriesId}`);
  await expect(page.locator('.list-item')).toHaveCount(2);
  await openCoverDialog();
  await page.getByRole('button', { name: 'Özel kapağı kaldır' }).click();
  await expect(coverImg()).not.toHaveAttribute('src', uploadedSrc || '');
  await expect
    .poll(() => coverImg().evaluate((img) => img.complete && img.naturalWidth))
    .toBe(300);
  assertNoCspOrPageErrors(consoleMessages, pageErrors);
  await context.close();
});

test('okumaya devam et rafı', async ({ browser }) => {
  const { context, page, consoleMessages, pageErrors } = await loginContext(browser, {
    width: 1280,
    height: 800
  });
  const books = await lookupBooks(context.request);
  const today = new Date().toISOString().slice(0, 10);
  for (const b of Object.values(books)) {
    const res = await context.request.put(`/api/books/${b.id}/status`, {
      data: { status: 'reading', local_date: today },
      headers: JSON_HEADERS
    });
    expect(res.ok()).toBeTruthy();
  }

  await page.goto('/#/');
  const shelf = page.locator('.shelf');
  await expect(shelf).toBeVisible();
  expect(await page.locator('.shelf-card').count()).toBeGreaterThanOrEqual(4);

  // arama yazılınca ve durum süzgeci etkinken gizlenir
  const search = page.getByRole('searchbox', { name: 'Kütüphanede ara' });
  await search.fill('Test');
  await expect(shelf).toBeHidden();
  await page.getByRole('button', { name: 'Aramayı temizle' }).click();
  await expect(shelf).toBeVisible();
  await page.getByRole('radio', { name: 'Okunuyor', exact: true }).click();
  await expect(shelf).toBeHidden();
  await page.getByRole('radio', { name: 'Tümü', exact: true }).click();
  await expect(shelf).toBeVisible();

  // 360 px: yatay kaydırılır, yapışma değeri x proximity, kart klavye ile odaklanır
  await page.setViewportSize({ width: 360, height: 800 });
  const track = page.locator('.shelf-track');
  const dims = await track.evaluate((el) => ({
    sw: el.scrollWidth,
    cw: el.clientWidth,
    snap: getComputedStyle(el).scrollSnapType,
    bodyOverflow: document.documentElement.scrollWidth > document.documentElement.clientWidth
  }));
  expect(dims.sw).toBeGreaterThan(dims.cw);
  // Chromium varsayılan 'proximity' sözcüğünü hesaplı değerde yazmaz ('x')
  expect(dims.snap).toMatch(/^x( proximity)?$/);
  expect(dims.bodyOverflow).toBe(false);
  let focused = false;
  for (let i = 0; i < 40 && !focused; i++) {
    await page.keyboard.press('Tab');
    focused = await page.evaluate(() => document.activeElement.classList.contains('shelf-card'));
  }
  expect(focused).toBe(true);
  await page.keyboard.press('Enter');
  await expect(page).toHaveURL(/#\/oku\/\d+$/);
  assertNoCspOrPageErrors(consoleMessages, pageErrors);
  await context.close();
});

test('okuma takvimi', async ({ browser }) => {
  const { context, page, consoleMessages, pageErrors } = await loginContext(browser, {
    width: 1280,
    height: 800
  });
  // seçilebilir ikinci yıl için yanıta 2024 eklenir
  await page.route('**/api/stats/heatmap**', async (route) => {
    const res = await route.fetch();
    const json = await res.json();
    json.years = Array.from(new Set([2024, ...json.years]));
    await route.fulfill({ response: res, json });
  });
  await page.goto('/#/gecmis');
  const scroll = page.locator('.heatmap-scroll');
  await expect(scroll).toBeVisible();
  const today = await page.evaluate(() => window.KRS.todayLocal());
  const cell = page.locator(`rect.hm-cell[data-date="${today}"]`);
  await expect(cell).toHaveCount(1);
  expect(await cell.getAttribute('class')).toMatch(/hm-[1-4]/);
  expect(await cell.locator('title').textContent()).toMatch(/\d+ dk|\d+ sn/);

  const allRows = await page.locator('#history-table li').count();
  expect(allRows).toBeGreaterThan(0);
  const todayText = await page.evaluate(() =>
    new Intl.DateTimeFormat('tr-TR', { day: 'numeric', month: 'short', year: 'numeric' }).format(new Date())
  );
  await cell.dispatchEvent('click');
  const chip = page.getByRole('button', { name: 'Gün süzgecini kaldır' });
  await expect(chip).toBeVisible();
  const rows = page.locator('#history-table li');
  await expect(rows.first()).toBeVisible();
  for (const text of await rows.allTextContents()) expect(text).toContain(todayText);
  await chip.click();
  await expect(chip).toHaveCount(0);
  await expect(page.locator('#history-table li')).toHaveCount(allRows);

  // klavye: odak, ArrowLeft bir hafta geri, Enter seçer
  await cell.focus();
  await page.keyboard.press('ArrowLeft');
  const movedDate = await page.evaluate(() => document.activeElement.getAttribute('data-date'));
  expect(movedDate).not.toBe(today);
  await page.keyboard.press('Enter');
  await expect(chip).toBeVisible();
  await expect(page.locator('.history-list .empty h2')).toHaveText('Bu günde okuma kaydı yok');
  await chip.click();

  // yılın ritmi: iki grafik, dört özet
  await expect(page.locator('.rhythm .chart-card')).toHaveCount(2);
  await expect(page.locator('.rhythm .summary-item')).toHaveCount(4);

  // veri olmayan yıl: boş durum, hata yok
  await page.locator('#heatmap-year').selectOption('2024');
  await expect(page.locator('.rhythm-empty')).toHaveText('Bu yıl için veri yok');
  await expect(page.locator('.heatmap-body .empty.error')).toHaveCount(0);

  // 360 px: en yeni taraf görünür
  await page.setViewportSize({ width: 360, height: 800 });
  await page.locator('#heatmap-year').selectOption(String(new Date().getFullYear()));
  await expect(cell).toHaveCount(1);
  const edge = await scroll.evaluate((el) => ({
    gap: Math.abs(el.scrollLeft + el.clientWidth - el.scrollWidth),
    overflow: document.documentElement.scrollWidth > document.documentElement.clientWidth
  }));
  expect(edge.gap).toBeLessThanOrEqual(2);
  expect(edge.overflow).toBe(false);
  assertNoCspOrPageErrors(consoleMessages, pageErrors);
  await context.close();
});
