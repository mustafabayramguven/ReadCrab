// Saf hesaplar: DOM'a dokunmaz. Tarayıcıda window.KRS_CALC, Node'da module.exports.
(function () {
  'use strict';

  var MONTHS = ['Oca', 'Şub', 'Mar', 'Nis', 'May', 'Haz', 'Tem', 'Ağu', 'Eyl', 'Eki', 'Kas', 'Ara'];
  var FONTS = ['literata', 'merriweather', 'source-serif', 'eb-garamond', 'inter', 'atkinson', 'lexend'];
  var THEMES = ['light', 'dark', 'sepia', 'black'];
  var DAY_MS = 86400000;

  function pad(n) {
    return (n < 10 ? '0' : '') + n;
  }

  function heatThresholds(values) {
    var sorted = [];
    for (var i = 0; i < values.length; i++) {
      if (values[i] > 0) sorted.push(values[i]);
    }
    sorted.sort(function (a, b) { return a - b; });
    var n = sorted.length;
    if (n === 0) return [0, 0, 0];
    return [0.25, 0.5, 0.75].map(function (p) {
      return sorted[Math.ceil(p * n) - 1];
    });
  }

  function heatLevel(v, th) {
    if (!(v > 0)) return 0;
    if (v <= th[0]) return 1;
    if (v <= th[1]) return 2;
    if (v <= th[2]) return 3;
    return 4;
  }

  function yearGrid(year) {
    var jan1 = Date.UTC(year, 0, 1);
    var offset = (new Date(jan1).getUTCDay() + 6) % 7;
    var cells = [];
    var months = [];
    var seenMonth = -1;
    var total = Math.round((Date.UTC(year + 1, 0, 1) - jan1) / DAY_MS);
    var maxCol = 0;
    for (var i = 0; i < total; i++) {
      var d = new Date(jan1 + i * DAY_MS);
      var col = Math.floor((i + offset) / 7);
      var row = (i + offset) % 7;
      cells.push({
        date: d.getUTCFullYear() + '-' + pad(d.getUTCMonth() + 1) + '-' + pad(d.getUTCDate()),
        col: col,
        row: row
      });
      if (d.getUTCMonth() !== seenMonth) {
        seenMonth = d.getUTCMonth();
        months.push({ label: MONTHS[seenMonth], col: col });
      }
      maxCol = col;
    }
    return { cols: maxCol + 1, cells: cells, months: months };
  }

  function parseDate(s) {
    var m = /^(\d{4})-(\d{2})-(\d{2})$/.exec(String(s));
    if (!m) return null;
    return { y: +m[1], m: +m[2] - 1, d: +m[3] };
  }

  function monthTotals(days) {
    var out = [0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0];
    (days || []).forEach(function (x) {
      var p = parseDate(x.date);
      if (p) out[p.m] += x.seconds || 0;
    });
    return out;
  }

  function weekdayTotals(days) {
    var out = [0, 0, 0, 0, 0, 0, 0];
    (days || []).forEach(function (x) {
      var p = parseDate(x.date);
      if (p) out[(new Date(Date.UTC(p.y, p.m, p.d)).getUTCDay() + 6) % 7] += x.seconds || 0;
    });
    return out;
  }

  function yearSummary(days) {
    var months = monthTotals(days);
    var bestMonth = null;
    for (var i = 0; i < 12; i++) {
      if (months[i] > 0 && (bestMonth === null || months[i] > months[bestMonth])) bestMonth = i;
    }
    var bestDay = null;
    var bestDaySeconds = 0;
    var daysRead = 0;
    var total = 0;
    (days || []).forEach(function (x) {
      var s = x.seconds || 0;
      if (s > 0) {
        daysRead++;
        total += s;
        if (s > bestDaySeconds) {
          bestDaySeconds = s;
          bestDay = x.date;
        }
      }
    });
    return {
      bestMonth: bestMonth,
      bestMonthSeconds: bestMonth === null ? 0 : months[bestMonth],
      bestDay: bestDay,
      bestDaySeconds: bestDaySeconds,
      daysRead: daysRead,
      avgPerDay: daysRead === 0 ? 0 : total / daysRead
    };
  }

  var DEFAULT_PREFS = {
    v: 1, theme: null, font: 'literata', fs: 18, lh: 1.7, ws: 0, ls: 0, mg: 0, align: 'left', mode: 'scroll'
  };

  function num(v, lo, hi, step, def) {
    if (typeof v === 'string' && v.trim() !== '') v = Number(v);
    if (typeof v !== 'number' || !isFinite(v)) return def;
    var c = Math.min(hi, Math.max(lo, v));
    var snapped = lo + Math.round((c - lo) / step) * step;
    return Math.round(Math.min(hi, snapped) * 1000) / 1000;
  }

  function normalizePrefs(raw, legacy) {
    var obj = null;
    var absent = raw === null || raw === undefined || raw === '';
    if (typeof raw === 'string' && !absent) {
      try { obj = JSON.parse(raw); } catch (_) { obj = null; }
    } else if (!absent) {
      obj = raw;
    }
    if (obj === null || typeof obj !== 'object' || Array.isArray(obj)) obj = {};
    if (absent && legacy && typeof legacy === 'object') {
      if (legacy.fs !== undefined && legacy.fs !== null) obj.fs = legacy.fs;
      if (THEMES.indexOf(legacy.readerTheme) >= 0) obj.theme = legacy.readerTheme;
    }
    var d = DEFAULT_PREFS;
    return {
      v: 1,
      theme: THEMES.indexOf(obj.theme) >= 0 ? obj.theme : null,
      font: FONTS.indexOf(obj.font) >= 0 ? obj.font : d.font,
      // fs 14-28 adım 2: tek sayı çifte yuvarlanır
      fs: num(obj.fs, 14, 28, 2, d.fs),
      lh: num(obj.lh, 1.2, 2.2, 0.1, d.lh),
      ws: num(obj.ws, 0, 0.5, 0.05, d.ws),
      ls: num(obj.ls, 0, 0.1, 0.01, d.ls),
      mg: num(obj.mg, 0, 20, 1, d.mg),
      align: obj.align === 'justify' ? 'justify' : 'left',
      mode: obj.mode === 'page' ? 'page' : 'scroll'
    };
  }

  function pageCount(contentRight, pw) {
    return Math.max(1, Math.ceil((contentRight - 1) / pw));
  }

  function pageFromRatio(p, n) {
    return n <= 1 ? 0 : Math.round(p * (n - 1));
  }

  function ratioFromPage(i, n) {
    return n <= 1 ? 0 : i / (n - 1);
  }

  function formatEta(pace, overall) {
    if (pace === null || pace === undefined || !isFinite(pace) || overall >= 0.99) return '';
    var rem = pace * (1 - overall);
    var t;
    if (rem < 60) {
      t = "1 dk'dan az";
    } else {
      var m = rem < 600 ? Math.round(rem / 60) : Math.round(rem / 300) * 5;
      if (m >= 60) {
        var h = Math.floor(m / 60);
        var mm = m % 60;
        t = h + ' sa' + (mm ? ' ' + mm + ' dk' : '');
      } else {
        t = m + ' dk';
      }
    }
    return 'Bu kitapta ~' + t + ' kaldı';
  }

  // Ardışık iki gün de bağlıysa (nitelikli veya donmuş) aralarına bağlantı. cells = yearGrid(y).cells (günlük, sıralı).
  function chainLinks(cells, byDate) {
    var out = [];
    function linked(d) { return !!d && (d.qualified === true || d.frozen === true); }
    for (var i = 0; i + 1 < cells.length; i++) {
      var da = byDate[cells[i].date], db = byDate[cells[i + 1].date];
      if (linked(da) && linked(db)) {
        out.push({ a: i, b: i + 1, wrap: cells[i + 1].col !== cells[i].col, current: !!(da.in_current && db.in_current) });
      }
    }
    return out;
  }

  function streakMessage(s) {
    var min = s.streak_min_seconds || 300;
    var mins = Math.max(1, Math.ceil(Math.max(0, min - (s.today_seconds || 0)) / 60));
    var freezeText = s.freeze_available ? 'Bu haftanın dondurma hakkı duruyor' : 'Bu haftanın dondurma hakkı kullanıldı';
    if (s.streak_at_risk) return { kind: 'risk', text: 'Seriyi korumak için bugün ' + mins + ' dk daha oku', freezeText: freezeText };
    if ((s.streak_current || 0) > 0) return { kind: 'ok', text: 'Bugünkü hedef tamam', freezeText: freezeText };
    return { kind: 'none', text: 'Seri başlatmak için bugün ' + mins + ' dk oku', freezeText: freezeText };
  }

  var api = {
    chainLinks: chainLinks,
    streakMessage: streakMessage,
    DEFAULT_PREFS: DEFAULT_PREFS,
    FONTS: FONTS,
    heatThresholds: heatThresholds,
    heatLevel: heatLevel,
    yearGrid: yearGrid,
    monthTotals: monthTotals,
    weekdayTotals: weekdayTotals,
    yearSummary: yearSummary,
    normalizePrefs: normalizePrefs,
    pageCount: pageCount,
    pageFromRatio: pageFromRatio,
    ratioFromPage: ratioFromPage,
    formatEta: formatEta
  };

  if (typeof window !== 'undefined') window.KRS_CALC = api;
  if (typeof module !== 'undefined' && module.exports) module.exports = api;
})();
