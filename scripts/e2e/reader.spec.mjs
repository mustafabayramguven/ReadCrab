import fs from 'node:fs';
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

  await page.locator('#setup-username').fill('e2e');
  await page.locator('#setup-password').fill('e2e-pass-123');
  await page.getByRole('button', { name: 'Kurulumu tamamla' }).click();

  await expect(page).toHaveURL(/#\/$/);

  // Tarama öncesi boş kütüphane ekran görüntüsü (1280 açık)
  const scanBtn = page.getByRole('button', { name: 'Kütüphaneyi tara' }).first();
  await expect(scanBtn).toBeVisible();
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

  // 2. Arayüzde "Kütüphaneyi tara", /api/scan-status running:false olana kadar yokla
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
        if (!page.url().endsWith(scr.hash)) {
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

        const shotPath = `screenshots/${scr.name}-${vp.width}-${colorScheme}.png`;
        await page.screenshot({ path: shotPath, fullPage: false });
        await runAxeCheck(page, `${scr.name} ${vp.width} ${colorScheme}`);

        if (scr.openAction) {
          await page.keyboard.press('Escape');
          await expect(page.locator('dialog[open]')).toHaveCount(0);
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
