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

test('okuyucu uçtan uca', async ({ page }) => {
  const consoleMessages = [];
  const pageErrors = [];
  const pingStatuses = [];

  attachConsoleGuards(page, consoleMessages, pageErrors);
  page.on('response', (res) => {
    if (res.url().includes('/api/reading/ping')) {
      pingStatuses.push(res.status());
    }
  });

  // 1. #/kurulum formu ile e2e / e2e-pass-123 kurulumu
  await page.goto('/#/kurulum');
  await page.locator('#setup-username').fill('e2e');
  await page.locator('#setup-password').fill('e2e-pass-123');
  await page.getByRole('button', { name: 'Kurulumu tamamla' }).click();

  await expect(page).toHaveURL(/#\/$/);

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

  // 7. #/gecmis sayfasında en az bir satır deneme içerir
  await page.goto('/#/gecmis');
  const historyTable = page.locator('#history-table');
  await expect(historyTable).toBeVisible();
  await expect(historyTable.locator('li', { hasText: 'deneme' }).first()).toBeVisible();

  // 6. /api/reading/ping yanıtlarından en az biri 204 olmalı
  expect(pingStatuses).toContain(204);

  // 8. #/istatistik sayfasında svg ve içinde en az bir rect vardır
  await page.goto('/#/istatistik');
  const chartRect = page.locator('.chart-card svg rect').first();
  await expect(chartRect).toBeVisible();

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
      name: 'okuyucu-cbz',
      hash: `/#/oku/${sharedCbzId}`,
      waitSelector: '#page-indicator',
      isReader: true
    },
    {
      name: 'okuyucu-epub',
      hash: `/#/oku/${sharedEpubId}`,
      waitSelector: 'iframe.reader-frame',
      isReader: true
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
    }
  ];

  for (const vp of viewports) {
    for (const colorScheme of schemes) {
      const context = await browser.newContext({ viewport: vp, colorScheme });
      const loginRes = await context.request.post('/api/login', {
        data: { username: 'e2e', password: 'e2e-pass-123' },
        headers: { 'Content-Type': 'application/json' }
      });
      expect(loginRes.ok()).toBeTruthy();

      const page = await context.newPage();
      await page.emulateMedia({ colorScheme });

      const consoleMessages = [];
      const pageErrors = [];
      attachConsoleGuards(page, consoleMessages, pageErrors);

      for (const scr of screens) {
        await page.goto(scr.hash);
        await page.waitForLoadState('networkidle');
        await expect(page.locator(scr.waitSelector).first()).toBeVisible();

        if (scr.name === 'okuyucu-epub') {
          await expect(
            page.frameLocator('iframe.reader-frame').locator('h1')
          ).toBeVisible();
        }

        if (scr.isReader) {
          await page.mouse.move(120, 120);
          await expect(page.locator('.reader-bar.top')).not.toHaveClass(/hidden/);
        }

        const shotPath = `screenshots/${scr.name}-${vp.width}-${colorScheme}.png`;
        await page.screenshot({ path: shotPath, fullPage: false });

        const results = await new AxeBuilder({ page }).analyze();
        const seriousOrCritical = results.violations.filter(
          (v) => v.impact === 'serious' || v.impact === 'critical'
        );
        expect(
          seriousOrCritical,
          `Axe ihlali (${scr.name} ${vp.width} ${colorScheme}): ` +
            seriousOrCritical.map((v) => `${v.id}: ${v.help}`).join('; ')
        ).toEqual([]);
      }

      assertNoCspOrPageErrors(consoleMessages, pageErrors);
      await context.close();
    }
  }
});
