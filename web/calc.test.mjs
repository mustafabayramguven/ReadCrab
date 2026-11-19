import assert from 'node:assert/strict';
import { createRequire } from 'node:module';

const require = createRequire(import.meta.url);
const c = require('./calc.js');

// heatThresholds
assert.deepEqual(c.heatThresholds([]), [0, 0, 0]);
assert.deepEqual(c.heatThresholds([0, -1]), [0, 0, 0]);
assert.deepEqual(c.heatThresholds([5]), [5, 5, 5]);
assert.deepEqual(c.heatThresholds([10, 9, 8, 7, 6, 5, 4, 3, 2, 1, 0]), [3, 5, 8]);

// heatLevel
const th = [3, 5, 8];
assert.deepEqual([0, -2, 1, 3, 4, 5, 6, 8, 9].map((v) => c.heatLevel(v, th)), [0, 0, 1, 1, 2, 2, 3, 3, 4]);

// yearGrid
const g26 = c.yearGrid(2026);
assert.equal(g26.cells.length, 365);
assert.deepEqual(g26.cells[0], { date: '2026-01-01', col: 0, row: 3 });
assert.equal(g26.cells[364].date, '2026-12-31');
assert.ok(g26.cols >= 53 && g26.cols <= 54);
assert.equal(g26.months.length, 12);
assert.deepEqual(g26.months[0], { label: 'Oca', col: 0 });
assert.equal(c.yearGrid(2023).cells[0].row, 6);
const g28 = c.yearGrid(2028);
assert.equal(g28.cells.length, 366);
assert.ok(g28.cols <= 54);
assert.equal(new Set(g28.cells.map((x) => x.date)).size, 366);

// totals ve özet
const days = [
  { date: '2026-01-05', seconds: 600 },
  { date: '2026-01-06', seconds: 1200 },
  { date: '2026-03-02', seconds: 300 },
  { date: '2026-03-03', seconds: 0 }
];
const mt = c.monthTotals(days);
assert.equal(mt.length, 12);
assert.equal(mt[0], 1800);
assert.equal(mt[2], 300);
const wt = c.weekdayTotals(days);
assert.equal(wt.length, 7);
assert.equal(wt[0], 900); // 5 Oca ve 2 Mar 2026 Pazartesi
assert.equal(wt[1], 1200);
const sum = c.yearSummary(days);
assert.equal(sum.bestMonth, 0);
assert.equal(sum.bestDay, '2026-01-06');
assert.equal(sum.daysRead, 3);
assert.equal(sum.avgPerDay, 700);
const empty = c.yearSummary([]);
assert.equal(empty.bestMonth, null);
assert.equal(empty.bestDay, null);
assert.equal(empty.avgPerDay, 0);

// normalizePrefs
const def = c.normalizePrefs(null, null);
assert.deepEqual(def, { v: 1, theme: null, font: 'literata', fs: 18, lh: 1.7, ws: 0, ls: 0, mg: 0, align: 'left', mode: 'scroll' });
const mig = c.normalizePrefs(null, { fs: 20, readerTheme: 'sepia' });
assert.equal(mig.fs, 20);
assert.equal(mig.theme, 'sepia');
assert.deepEqual(c.normalizePrefs('{bozuk', { fs: 22, readerTheme: 'dark' }), def);
assert.equal(c.normalizePrefs(JSON.stringify({ fs: 20 }), { fs: 26 }).fs, 20);
const clamped = c.normalizePrefs({ fs: 99, lh: 0.1, ws: 9, ls: -1, mg: 50, font: 'comic', theme: 'pembe', align: 'x', mode: 'y', evil: 1 });
assert.deepEqual(clamped, { v: 1, theme: null, font: 'literata', fs: 28, lh: 1.2, ws: 0.5, ls: 0, mg: 20, align: 'left', mode: 'scroll' });
assert.equal(c.normalizePrefs({ fs: 19 }).fs, 20);
assert.equal(c.normalizePrefs({ fs: 21 }).fs, 22);
assert.equal(c.normalizePrefs({ fs: 'abc' }).fs, 18);
assert.equal(c.normalizePrefs({ lh: 1.74 }).lh, 1.7);
assert.equal(c.normalizePrefs({ ws: 0.12 }).ws, 0.1);
assert.equal(c.normalizePrefs({ ls: 0.037 }).ls, 0.04);
assert.ok(!('evil' in clamped));
assert.deepEqual(c.normalizePrefs(c.normalizePrefs({ fs: 24, theme: 'black', font: 'lexend', align: 'justify', mode: 'page' })), c.normalizePrefs({ fs: 24, theme: 'black', font: 'lexend', align: 'justify', mode: 'page' }));

// sayfa hesapları
assert.equal(c.pageCount(0, 400), 1);
assert.equal(c.pageCount(401, 400), 1);
assert.equal(c.pageCount(402, 400), 2);
assert.equal(c.pageCount(800, 400), 2);
assert.equal(c.pageCount(801, 400), 2);
assert.equal(c.pageCount(802, 400), 3);
assert.equal(c.pageFromRatio(0.5, 1), 0);
assert.equal(c.ratioFromPage(3, 1), 0);
for (const n of [2, 3, 7, 40]) {
  for (let i = 0; i < n; i++) {
    assert.equal(c.pageFromRatio(c.ratioFromPage(i, n), n), i);
  }
}

// formatEta
assert.equal(c.formatEta(null, 0.1), '');
assert.equal(c.formatEta(7200, 0.99), '');
assert.equal(c.formatEta(7200, 0.5), 'Bu kitapta ~1 sa kaldı');
assert.equal(c.formatEta(100, 0.5), "Bu kitapta ~1 dk'dan az kaldı");
assert.equal(c.formatEta(300, 0), 'Bu kitapta ~5 dk kaldı');
assert.equal(c.formatEta(540, 0), 'Bu kitapta ~9 dk kaldı');
assert.equal(c.formatEta(1500, 0), 'Bu kitapta ~25 dk kaldı');
assert.equal(c.formatEta(7800, 0), 'Bu kitapta ~2 sa 10 dk kaldı');

// chainLinks
const cells = c.yearGrid(2026).cells;
const idx = (d) => cells.findIndex((x) => x.date === d);
const byDate = {
  '2026-11-03': { qualified: true, frozen: false, in_current: true },
  '2026-11-04': { qualified: false, frozen: true, in_current: true },
  '2026-11-05': { qualified: true, frozen: false, in_current: true },
  '2026-11-06': { qualified: false, frozen: false, in_current: false },
  '2026-11-07': { qualified: true, frozen: false, in_current: false },
  '2026-11-08': { qualified: true, frozen: false, in_current: false }
};
const links = c.chainLinks(cells, byDate);
assert.deepEqual(links.map((l) => [l.a, l.b]), [[idx('2026-11-03'), idx('2026-11-04')], [idx('2026-11-04'), idx('2026-11-05')], [idx('2026-11-07'), idx('2026-11-08')]]);
assert.deepEqual(links.map((l) => l.current), [true, true, false]);
assert.equal(links[2].wrap, false); // 11-07 Cumartesi ve 11-08 Pazar aynı sütunda
byDate['2026-11-09'] = { qualified: true, frozen: false, in_current: false };
const wrapLink = c.chainLinks(cells, byDate).find((l) => l.a === idx('2026-11-08'));
assert.equal(wrapLink.wrap, true); // Pazar -> sonraki Pazartesi: yeni sütun

// streakMessage
assert.deepEqual(c.streakMessage({ streak_at_risk: true, today_seconds: 0, streak_min_seconds: 300, freeze_available: true, streak_current: 4 }),
  { kind: 'risk', text: 'Seriyi korumak için bugün 5 dk daha oku', freezeText: 'Bu haftanın dondurma hakkı duruyor' });
assert.equal(c.streakMessage({ streak_at_risk: true, today_seconds: 241, streak_min_seconds: 300, freeze_available: false }).text, 'Seriyi korumak için bugün 1 dk daha oku');
assert.equal(c.streakMessage({ streak_at_risk: false, streak_current: 3, today_seconds: 300, streak_min_seconds: 300, freeze_available: false }).kind, 'ok');
assert.equal(c.streakMessage({ streak_current: 0, today_seconds: 120, streak_min_seconds: 300, freeze_available: true }).text, 'Seri başlatmak için bugün 3 dk oku');
assert.equal(c.streakMessage({ streak_current: 0, freeze_available: false }).freezeText, 'Bu haftanın dondurma hakkı kullanıldı');

console.log('calc.test.mjs: tüm testler başarılı');
