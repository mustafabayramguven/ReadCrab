(function () {
  'use strict';
  var FONTS = {
    literata: '"Literata Variable", Georgia, "Noto Serif", serif',
    merriweather: '"Merriweather Variable", Georgia, "Noto Serif", serif',
    'source-serif': '"Source Serif 4 Variable", Georgia, "Noto Serif", serif',
    'eb-garamond': '"EB Garamond Variable", Georgia, "Noto Serif", serif',
    inter: '"Inter Variable", system-ui, sans-serif',
    atkinson: '"Atkinson Hyperlegible", system-ui, sans-serif',
    lexend: '"Lexend Variable", system-ui, sans-serif'
  };
  var BLOCK_SEL = 'p,h1,h2,h3,h4,h5,h6,li,pre,figure,tr,dt,dd';
  var MIN_M = 24, VPAD = 32, SWIPE = 40;
  var root = document.documentElement;
  var P = { mode: 'scroll', mg: 0, fs: 18, lh: 1.7, ws: 0, ls: 0, zones: true,
            bg: '#FFFFFF', fg: '#000000', link: '#0000EE', font: 'literata', align: 'left' };
  var ch = 0, ratio = 0, anchor = -1, pageIdx = 0, pageTotal = 1, blocks = [];
  var ready = false, restoring = false, deferred = false, lastScrollAt = 0;
  var lastBlock = -2, sentEnd = false, relT = null, down = null;
  var pg = document.createElement('div');
  pg.id = 'pg';
  root.appendChild(pg);

  function post(o) { try { Android.post(JSON.stringify(o)); } catch (e) {} }
  function clamp01(v) { v = Number(v); return !isFinite(v) ? 0 : v < 0 ? 0 : v > 1 ? 1 : v; }
  function se() { return document.scrollingElement || root; }
  function paged() { return P.mode === 'page'; }

  function collectBlocks() {
    var all = Array.prototype.slice.call(document.body.querySelectorAll(BLOCK_SEL));
    blocks = all.filter(function (e) {
      return !e.querySelector(BLOCK_SEL) && ((e.textContent || '').trim() !== '' || !!e.querySelector('img'));
    });
    if (blocks.length === 0) blocks = Array.prototype.slice.call(document.body.children);
  }
  function firstRect(el) { var r = el.getClientRects(); return r.length ? r[0] : null; }
  function pageOfBlock(i) {
    var el = blocks[i], r = el && firstRect(el);
    if (!r) return 0;
    return Math.max(0, Math.min(pageTotal - 1, Math.floor((r.left + se().scrollLeft + 1) / window.innerWidth)));
  }
  function scrollTopOf(i) {
    var el = blocks[i];
    if (!el || i === 0) return 0;
    return Math.max(0, Math.round(el.getBoundingClientRect().top + se().scrollTop - 8));
  }
  function blockAtPage(i) {          // sayfada başlayan ilk blok; yoksa sayfayı kaplayan blok
    var pw = window.innerWidth, lo = i * pw, hi = lo + pw, sl = se().scrollLeft, cover = -1;
    for (var k = 0; k < blocks.length; k++) {
      var rs = blocks[k].getClientRects();
      if (!rs.length) continue;
      var l = rs[0].left + sl;
      if (l >= hi - 1) break;
      if (l >= lo - 1) return k;
      if (rs[rs.length - 1].right + sl > lo + 1) cover = k;
    }
    return cover;
  }
  function blockAtScroll() {         // üstten görünür ilk blok (ikili arama)
    if (!blocks.length) return -1;
    var lo = 0, hi = blocks.length - 1;
    while (lo < hi) {
      var mid = (lo + hi) >> 1;
      if (blocks[mid].getBoundingClientRect().bottom > 8) hi = mid; else lo = mid + 1;
    }
    return lo;
  }
  function footer() { pg.textContent = paged() ? (pageIdx + 1) + ' / ' + pageTotal : ''; }
  // u: kullanıcı eylemi (kaydırma/çevirme) — yalnız u=1 olanlar kaydedilir
  function report(u) {
    footer();
    post({ t: 'pos', ch: ch, p: Number(ratio.toFixed(6)), b: anchor, nb: blocks.length,
           page: pageIdx, pages: pageTotal, u: u ? 1 : 0 });
  }

  function applyPrefs() {
    var s = root.style;
    s.setProperty('--bg', P.bg); s.setProperty('--fg', P.fg); s.setProperty('--link', P.link);
    s.setProperty('--fs', P.fs + 'px'); s.setProperty('--lh', String(P.lh));
    s.setProperty('--ws', P.ws + 'em'); s.setProperty('--ls', P.ls + 'em');
    s.setProperty('--font', FONTS[P.font] || FONTS.literata);
    s.setProperty('--align', P.align); s.setProperty('--hyph', P.align === 'justify' ? 'auto' : 'manual');
    root.classList.toggle('paged', paged());
  }

  function relayout() {
    if (!ready) return;
    var pw = window.innerWidth, ph = window.innerHeight, e = se();
    var m = Math.max(MIN_M + (pw * P.mg) / 100, (pw - 38 * P.fs) / 2);
    var st = root.style;
    st.setProperty('--pw', pw + 'px'); st.setProperty('--ph', ph + 'px'); st.setProperty('--m', m + 'px');
    st.setProperty('--mt', VPAD + 'px'); st.setProperty('--mb', VPAD + 'px');
    st.setProperty('--bh', Math.max(0, ph - 2 * VPAD) + 'px');
    st.setProperty('--mx', Math.round((pw * P.mg) / 100) + 'px');
    restoring = true;
    var promoted = false;
    if (paged()) {
      var range = document.createRange();
      range.selectNodeContents(document.body);
      var right = range.getBoundingClientRect().right + e.scrollLeft;
      pageTotal = Math.max(1, Math.ceil((right - 1) / pw));
      pageIdx = anchor >= 0 ? pageOfBlock(anchor) : (pageTotal <= 1 ? 0 : Math.round(ratio * (pageTotal - 1)));
      window.scrollTo(pageIdx * pw, 0);
      ratio = pageTotal <= 1 ? 0 : pageIdx / (pageTotal - 1);
      if (pageTotal <= 1) { ratio = 1; promoted = true; }  // tek sayfalık bölüm
    } else {
      pageTotal = 1; pageIdx = 0;
      var max = Math.max(0, e.scrollHeight - e.clientHeight);
      e.scrollTop = anchor >= 0 ? scrollTopOf(anchor) : Math.round(ratio * max);
      if (max <= 2) { ratio = 1; promoted = true; }          // tek ekranlık bölüm
      else ratio = clamp01(e.scrollTop / max);
    }
    if (anchor < 0) anchor = paged() ? blockAtPage(pageIdx) : blockAtScroll();
    lastBlock = anchor;
    requestAnimationFrame(function () {
      requestAnimationFrame(function () {
        restoring = false;
        if (deferred) { deferred = false; onScroll(); }
      });
    });
    report(promoted);   // tek ekranlık bölüm okunmuş sayılır (web ile aynı); diğer yerleşimler kaydedilmez
  }
  function relayoutSoon() {
    clearTimeout(relT);
    relT = setTimeout(function () { relT = null; relayout(); }, 100);
  }

  function onScroll() {                    // yalnız kaydırma modu
    lastScrollAt = Date.now();
    if (restoring) { deferred = true; return; }
    if (paged()) return;
    var e = se(), max = Math.max(0, e.scrollHeight - e.clientHeight);
    var r = max > 0 ? clamp01(e.scrollTop / max) : 0;
    if (e.scrollTop + e.clientHeight >= e.scrollHeight - 2) r = 1;
    ratio = r;
    var b = blockAtScroll();
    var end = r >= 1 && !sentEnd;
    if (b !== lastBlock || end) {
      anchor = b; lastBlock = b;
      if (r >= 1) sentEnd = true;
      report(true);
    }
  }
  window.addEventListener('scroll', onScroll, { passive: true });
  window.addEventListener('resize', relayoutSoon);
  document.addEventListener('load', relayoutSoon, true);
  if (document.fonts && document.fonts.addEventListener) document.fonts.addEventListener('loadingdone', relayoutSoon);

  function turn(d) {                       // sayfa modu: ayrık sayfa; edge: bölüm sınırı
    var i = pageIdx + d;
    if (i >= pageTotal) { post({ t: 'edge', d: 1 }); return; }
    if (i < 0) { post({ t: 'edge', d: -1 }); return; }
    pageIdx = i;
    ratio = pageTotal <= 1 ? 0 : i / (pageTotal - 1);
    window.scrollTo(i * window.innerWidth, 0);
    anchor = blockAtPage(i);
    report(true);
  }

  document.addEventListener('pointerdown', function (e) { down = { x: e.clientX, y: e.clientY }; }, { passive: true });
  document.addEventListener('pointerup', function (e) {
    if (!down) return;
    var dx = e.clientX - down.x, dy = e.clientY - down.y;
    down = null;
    if (paged() && Math.abs(dx) >= SWIPE && Math.abs(dx) > 1.5 * Math.abs(dy)) { turn(dx < 0 ? 1 : -1); return; }
    if (Math.abs(dx) > 10 || Math.abs(dy) > 10) return;
    if (!paged() && Date.now() - lastScrollAt < 250) return;
    if (paged() && P.zones) {
      var z = e.clientX / window.innerWidth;
      if (z < 1 / 3) { turn(-1); return; }
      if (z > 2 / 3) { turn(1); return; }
    }
    post({ t: 'tap' });
  });
  document.addEventListener('click', function (e) { e.preventDefault(); });  // dış bağlantı yok

  var RC = (window.RC = {});
  RC.setPrefs = function (p) { P = p; applyPrefs(); relayoutSoon(); };
  RC.load = function (path, c, r, b) {
    ready = false; ch = c; ratio = clamp01(r); anchor = (typeof b === 'number' && b >= 0) ? b : -1; sentEnd = false;
    fetch(path).then(function (res) {
      if (!res.ok) throw new Error('HTTP ' + res.status);
      return res.text();
    }).then(function (html) {
      document.body.innerHTML = html;
      collectBlocks();
      if (anchor >= blocks.length) anchor = -1;
      ready = true; applyPrefs(); relayout();
      post({ t: 'loaded', ch: ch, pages: pageTotal });
    }).catch(function (err) { post({ t: 'error', msg: String(err) }); });
  };
  RC.page = function (d) {                 // Kotlin: ses tuşları / düğmeler
    if (!ready) return;
    if (paged()) { turn(d); return; }
    var e = se();
    if (d > 0 && e.scrollTop + e.clientHeight >= e.scrollHeight - 2) { post({ t: 'edge', d: 1 }); return; }
    if (d < 0 && e.scrollTop <= 0) { post({ t: 'edge', d: -1 }); return; }
    e.scrollTop += d * Math.max(40, Math.floor(e.clientHeight * 0.9));
  };
  post({ t: 'ready' });
})();
