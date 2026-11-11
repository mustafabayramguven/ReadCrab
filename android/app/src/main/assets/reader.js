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
  var BAR_V = 176;                       // frame.css: üst 80 + alt 96 dolgu
  var root = document.documentElement;
  var P = { mode: 'scroll', mg: 0, fs: 18, zones: true };
  var ch = 0, ratio = 0, pageIdx = 0, pageTotal = 1;
  var ready = false, restoring = false, deferred = false, lastScrollAt = 0;
  var snapT = null, relT = null, down = null;

  function post(o) { try { Android.post(JSON.stringify(o)); } catch (e) {} }
  function clamp01(v) { v = Number(v); return !isFinite(v) ? 0 : v < 0 ? 0 : v > 1 ? 1 : v; }
  function se() { return document.scrollingElement || root; }
  function paged() { return P.mode === 'page'; }
  function report() { post({ t: 'pos', ch: ch, p: Number(ratio.toFixed(6)), page: pageIdx, pages: pageTotal }); }

  function applyPrefs() {
    var s = root.style;
    s.setProperty('--bg', P.bg); s.setProperty('--fg', P.fg); s.setProperty('--link', P.link);
    s.setProperty('--fs', P.fs + 'px'); s.setProperty('--lh', String(P.lh));
    s.setProperty('--ws', '0em'); s.setProperty('--ls', '0em');
    s.setProperty('--font', FONTS[P.font] || FONTS.literata);
    s.setProperty('--align', P.align); s.setProperty('--hyph', P.align === 'justify' ? 'auto' : 'manual');
    root.classList.toggle('paged', paged());
  }

  function relayout() {
    if (!ready) return;
    var pw = window.innerWidth, ph = window.innerHeight;
    var m = Math.max(16 + (pw * P.mg) / 100, (pw - 38 * P.fs) / 2);
    var st = root.style;
    st.setProperty('--pw', pw + 'px'); st.setProperty('--ph', ph + 'px'); st.setProperty('--m', m + 'px');
    st.setProperty('--bh', Math.max(0, ph - BAR_V) + 'px');
    st.setProperty('--mx', Math.round((pw * P.mg) / 100) + 'px');
    var e = se();
    restoring = true;
    if (paged()) {
      var range = document.createRange();
      range.selectNodeContents(document.body);
      var right = range.getBoundingClientRect().right + e.scrollLeft;
      pageTotal = Math.max(1, Math.ceil((right - 1) / pw));
      pageIdx = pageTotal <= 1 ? 0 : Math.round(ratio * (pageTotal - 1));
      window.scrollTo({ left: pageIdx * pw, top: 0, behavior: 'auto' });
    } else {
      pageTotal = 1; pageIdx = 0;
      e.scrollTop = Math.round(ratio * Math.max(0, e.scrollHeight - e.clientHeight));
    }
    requestAnimationFrame(function () {
      requestAnimationFrame(function () {
        restoring = false;
        if (deferred) { deferred = false; onScroll(); }
      });
    });
    var single = paged() ? pageTotal <= 1 : e.scrollHeight - e.clientHeight <= 2;
    if (single && ratio < 1) ratio = 1;      // tek ekranlık bölüm okunmuş sayılır
    report();
  }
  function relayoutSoon() {
    clearTimeout(relT); clearTimeout(snapT);
    relT = setTimeout(function () { relT = null; relayout(); }, 100);
  }

  function snapSoon() {
    clearTimeout(snapT);
    snapT = setTimeout(function () {
      if (relT || !paged()) return;
      var pw = window.innerWidth, e = se();
      var i = Math.max(0, Math.min(pageTotal - 1, Math.round(e.scrollLeft / pw)));
      if (Math.abs(e.scrollLeft - i * pw) > 1) window.scrollTo({ left: i * pw, top: 0, behavior: 'auto' });
      if (i !== pageIdx) { pageIdx = i; ratio = pageTotal <= 1 ? 0 : i / (pageTotal - 1); report(); }
    }, 120);
  }

  function onScroll() {
    lastScrollAt = Date.now();
    if (restoring) { deferred = true; return; }
    if (paged()) { snapSoon(); return; }
    var e = se(), max = Math.max(0, e.scrollHeight - e.clientHeight);
    var r = max > 0 ? clamp01(e.scrollTop / max) : 0;
    if (e.scrollTop + e.clientHeight >= e.scrollHeight - 2) r = 1;
    ratio = r; report();
  }
  window.addEventListener('scroll', onScroll, { passive: true });
  window.addEventListener('resize', relayoutSoon);
  document.addEventListener('load', relayoutSoon, true);            // görseller yüklendikçe düzen değişir
  if (document.fonts && document.fonts.addEventListener) document.fonts.addEventListener('loadingdone', relayoutSoon);

  document.addEventListener('pointerdown', function (e) { down = { x: e.clientX, y: e.clientY }; }, { passive: true });
  document.addEventListener('click', function (e) {
    var a = e.target.closest && e.target.closest('a');
    if (a) e.preventDefault();                                       // dış bağlantı yok
    var moved = down && (Math.abs(e.clientX - down.x) > 10 || Math.abs(e.clientY - down.y) > 10);
    down = null;
    if (moved || Date.now() - lastScrollAt < 250) return;
    if (paged() && P.zones) {
      var z = e.clientX / window.innerWidth;
      if (z < 1 / 3) { RC.page(-1); return; }
      if (z > 2 / 3) { RC.page(1); return; }
    }
    post({ t: 'tap' });
  });

  var RC = (window.RC = {});
  RC.setPrefs = function (p) { P = p; applyPrefs(); relayoutSoon(); };
  RC.load = function (path, c, r) {
    ready = false; ch = c; ratio = clamp01(r);
    fetch(path).then(function (res) {
      if (!res.ok) throw new Error('HTTP ' + res.status);
      return res.text();
    }).then(function (html) {
      document.body.innerHTML = html;
      ready = true; applyPrefs(); relayout();
      post({ t: 'loaded', ch: ch, pages: pageTotal });
    }).catch(function (err) { post({ t: 'error', msg: String(err) }); });
  };
  RC.page = function (d) {
    if (!ready) return;
    var e = se();
    if (paged()) {
      var i = pageIdx + d;
      if (i >= pageTotal) { post({ t: 'edge', d: 1 }); return; }
      if (i < 0) { post({ t: 'edge', d: -1 }); return; }
      pageIdx = i; ratio = pageTotal <= 1 ? 0 : i / (pageTotal - 1);
      window.scrollTo({ left: i * window.innerWidth, top: 0, behavior: 'auto' });
      report();
    } else {
      if (d > 0 && e.scrollTop + e.clientHeight >= e.scrollHeight - 2) { post({ t: 'edge', d: 1 }); return; }
      if (d < 0 && e.scrollTop <= 0) { post({ t: 'edge', d: -1 }); return; }
      e.scrollTop += d * Math.max(40, Math.floor(e.clientHeight * 0.9));
    }
  };
  post({ t: 'ready' });
})();
