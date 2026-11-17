import { test, expect } from '@playwright/test';

// Cihazlar arası konum ve çevrimdışı okuma. reader.spec.mjs'nin kurduğu e2e hesabını ve
// taranmış kütüphaneyi kullanır.
test.use({ serviceWorkers: 'allow' });
test.describe.configure({ mode: 'serial' });

const USER = { username: 'e2e', password: 'e2e-pass-123' };

async function call(page, method, url, body) {
  return page.evaluate(
    async ([m, u, b]) => {
      const r = await fetch(u, {
        method: m,
        headers: { 'Content-Type': 'application/json' },
        body: b ? JSON.stringify(b) : undefined
      });
      let j = null;
      try {
        j = await r.json();
      } catch (_) {}
      return { status: r.status, json: j };
    },
    [method, url, body]
  );
}

async function topParagraph(page) {
  return page
    .frameLocator('iframe.reader-frame')
    .locator('body')
    .evaluate((body) => {
      for (const p of body.querySelectorAll('p, h1')) {
        if (p.getBoundingClientRect().bottom > 2 && p.textContent.trim()) return p.textContent;
      }
      return '';
    });
}

const para = (s) => Number((String(s).match(/paragraf (\d+)/) || [])[1]);

async function scrollTo(page, fraction) {
  await page
    .frameLocator('iframe.reader-frame')
    .locator('body')
    .evaluate((b, f) => {
      const el = b.ownerDocument.scrollingElement;
      el.scrollTop = (el.scrollHeight - el.clientHeight) * f;
    }, fraction);
}

let bookId = null;
let seriesId = null;

test('telefon ve bilgisayar aynı paragrafı açar; eski sekme yeni konumu ezemez', async ({ browser }) => {
  const phoneCtx = await browser.newContext({ viewport: { width: 390, height: 760 } });
  const phone = await phoneCtx.newPage();
  await phone.goto('/');
  expect((await call(phone, 'POST', '/api/login', USER)).status).toBe(200);
  const list = (await call(phone, 'GET', '/api/series')).json;
  const series = (list.items || list).find((s) => s.name === 'Üçüncü Uzun Seri');
  seriesId = series.id;
  const detail = (await call(phone, 'GET', '/api/series/' + seriesId)).json;
  bookId = detail.books.find((b) => b.title === 'Cilt 2').id;

  await phone.goto(`/#/oku/${bookId}`);
  await expect(phone.frameLocator('iframe.reader-frame').locator('h1')).toBeVisible();
  await scrollTo(phone, 0.5);
  await expect
    .poll(async () => (await call(phone, 'GET', `/api/books/${bookId}/progress`)).json.locator)
    .toContain('"t":');
  const phoneSaved = (await call(phone, 'GET', `/api/books/${bookId}/progress`)).json;
  const phoneTop = await topParagraph(phone);

  const deskCtx = await browser.newContext({ viewport: { width: 1400, height: 900 } });
  await deskCtx.addCookies(await phoneCtx.cookies());
  const desk = await deskCtx.newPage();
  await desk.goto(`/#/oku/${bookId}`);
  await expect(desk.frameLocator('iframe.reader-frame').locator('h1')).toBeAttached();
  await expect.poll(async () => Math.abs(para(await topParagraph(desk)) - para(phoneTop))).toBeLessThanOrEqual(1);

  // Bilgisayarda ilerle, sonra telefondaki eski sekme kaydetmeye çalışsın.
  await scrollTo(desk, 0.9);
  await expect
    .poll(async () => (await call(desk, 'GET', `/api/books/${bookId}/progress`)).json.client_id)
    .not.toBe(phoneSaved.client_id);
  const deskSaved = (await call(desk, 'GET', `/api/books/${bookId}/progress`)).json;
  await phone.waitForTimeout(1100); // updated_at saniye çözünürlüklü
  await scrollTo(phone, 0.55);
  await expect(phone.getByRole('button', { name: 'Oraya git' })).toBeVisible();
  expect((await call(phone, 'GET', `/api/books/${bookId}/progress`)).json.locator).toBe(deskSaved.locator);

  await phone.getByRole('button', { name: 'Oraya git' }).click();
  const deskTop = await topParagraph(desk);
  await expect.poll(async () => Math.abs(para(await topParagraph(phone)) - para(deskTop))).toBeLessThanOrEqual(1);

  await deskCtx.close();
  await phoneCtx.close();
});

test('indirilen kitap internetsiz açılır ve konum bağlantı gelince eşitlenir', async ({ browser }) => {
  const ctx = await browser.newContext({ viewport: { width: 390, height: 760 } });
  const page = await ctx.newPage();
  await page.goto('/');
  expect((await call(page, 'POST', '/api/login', USER)).status).toBe(200);
  await page.goto(`/#/seri/${seriesId}`);
  await page.waitForFunction(() => navigator.serviceWorker && navigator.serviceWorker.ready);
  await page.reload();
  await page.waitForFunction(() => Boolean(navigator.serviceWorker.controller));

  const row = page.locator('li.list-item', { hasText: 'Cilt 2' });
  await row.getByRole('button', { name: /çevrimdışı okumak için indir/ }).click();
  await expect(row.locator('button.dl-btn.is-ready')).toBeVisible({ timeout: 30000 });

  await page.goto('/#/');
  await expect(page.locator('section[aria-label="İndirilenler"]')).toBeVisible();
  await ctx.setOffline(true);
  await page.reload();
  await expect(page.locator('section[aria-label="İndirilenler"] .offline-note')).toBeVisible();
  await page.locator('section[aria-label="İndirilenler"] a.shelf-card', { hasText: 'Cilt 2' }).click();
  const frame = page.frameLocator('iframe.reader-frame');
  await expect(frame.locator('h1')).toBeVisible();
  // görseller de indirilmiş olmalı
  await expect
    .poll(() => frame.locator('img').first().evaluate((img) => img.complete && img.naturalWidth > 0))
    .toBe(true);

  await page.getByRole('button', { name: 'Sonraki bölüm' }).click({ force: true });
  await expect(frame.locator('h1')).toHaveText('Bölüm 2');
  await expect.poll(() => page.evaluate(() => localStorage.getItem('krs.pendingProgress'))).not.toBe(null);

  await ctx.setOffline(false);
  await page.evaluate(() => window.dispatchEvent(new Event('online')));
  await expect
    .poll(async () => (await call(page, 'GET', `/api/books/${bookId}/progress`)).json.locator)
    .toContain('"ch":1');

  // çıkış indirilenleri siler
  await page.goto('/#/');
  await page.getByRole('button', { name: 'Çıkış yap' }).first().click();
  await expect(page).toHaveURL(/#\/giris/);
  expect(await page.evaluate(() => localStorage.getItem('krs.offline.books'))).toBe('{}');
  await ctx.close();
});
