(function () {
  'use strict';

  var KRS = (window.KRS = window.KRS || {});
  KRS.readers = KRS.readers || {};

  var RELAYOUT_DELAY = 100;
  var SNAP_DELAY = 120;
  var TAP_GUARD_MS = 250;
  var BAR_V = 176;

  function clamp01(v) {
    var n = Number(v);
    if (!Number.isFinite(n)) return 0;
    return n < 0 ? 0 : n > 1 ? 1 : n;
  }

  function prefersReducedMotion() {
    if (document.documentElement.dataset.motion === 'reduced') return true;
    return Boolean(
      window.matchMedia && window.matchMedia('(prefers-reduced-motion: reduce)').matches
    );
  }

  async function open(book, mount, gen) {
    var myGen = gen !== undefined ? gen : KRS.routeGen;
    var el = KRS.el;
    var icon = KRS.icon;
    var clearNode = KRS.clearNode;
    var ui = KRS.readerUi;
    var calc = window.KRS_CALC;
    clearNode(mount);

    var tocRes = await KRS.api('GET', '/api/books/' + book.id + '/toc');
    if (myGen !== undefined && myGen !== KRS.routeGen) {
      return null;
    }
    var chapters = (tocRes && tocRes.chapters) || [];
    var totalChapters = Math.max(1, Number(book.page_count || chapters.length || 1));
    if (chapters.length === 0) {
      for (var cIdx = 0; cIdx < totalChapters; cIdx++) {
        chapters.push({ index: cIdx, title: 'Bölüm ' + (cIdx + 1) });
      }
    }

    var prefs = KRS.readerPrefs.get();
    var curChapter = 0;
    var curRatio = 0;
    var pageIdx = 0;
    var pageTotal = 1;
    var frameReady = false;
    var frameGen = 0;
    var unbinders = [];
    var restoring = false;
    var deferredScroll = false;
    var lastScrollAt = 0;
    var relayoutTimer = null;
    var snapTimer = null;
    var disposed = false;

    if (book.locator) {
      try {
        var parsed = JSON.parse(book.locator);
        if (parsed && typeof parsed === 'object') {
          var ch = parseInt(parsed.ch, 10);
          if (Number.isFinite(ch) && ch >= 0 && ch < totalChapters) {
            curChapter = ch;
          }
          curRatio = clamp01(parsed.p);
        }
      } catch (_) {}
    }

    function isPaged() {
      return prefs.mode === 'page';
    }

    var readerEl = el('div', { className: 'reader' });

    // ---- Üst çubuk -------------------------------------------------------------
    var backBtn = el(
      'button',
      {
        type: 'button',
        className: 'icon-btn',
        'aria-label': 'Seriye dön',
        onClick: function () {
          KRS.navigate('#/seri/' + book.series_id);
        }
      },
      [icon('arrow_back')]
    );
    var titleEl = el('h1', { className: 'reader-title', text: book.title });
    var settingsBtn = el(
      'button',
      {
        type: 'button',
        className: 'icon-btn',
        'aria-label': 'Ayarlar',
        onClick: function () {
          showBars();
          ui.openSheet(settings.dialog);
        }
      },
      [icon('tune')]
    );

    var bookmarks = ui.bookmarkController(book, {
      getLocator: function () {
        return JSON.stringify({ ch: curChapter, p: Number(curRatio.toFixed(6)) });
      },
      getLabel: function () {
        return 'Bölüm ' + (curChapter + 1) + ' · %' + Math.round(overallRatio() * 100);
      },
      matches: function (bm) {
        try {
          var loc = JSON.parse(bm.locator);
          if (loc.ch !== curChapter) return false;
          // sayfa modunda aynı sayfa, kaydırma modunda 0,005 içinde
          if (isPaged()) {
            return calc.pageFromRatio(clamp01(loc.p), pageTotal) === pageIdx;
          }
          return Math.abs(Number(loc.p) - curRatio) < 0.005;
        } catch (_) {
          return false;
        }
      }
    });
    var fsBtn = ui.fullscreenButton();
    var topBar = el(
      'div',
      { className: 'reader-bar top' },
      [backBtn, titleEl, bookmarks.toggleBtn, fsBtn, settingsBtn].filter(Boolean)
    );

    // ---- Sahne -----------------------------------------------------------------
    var iframe = el('iframe', {
      className: 'reader-frame',
      referrerpolicy: 'no-referrer',
      title: book.title + ' içeriği'
    });
    var stage = el('div', { className: 'reader-stage' }, [iframe]);

    // ---- Alt çubuk -------------------------------------------------------------
    var tocBtn = el(
      'button',
      {
        type: 'button',
        className: 'icon-btn',
        'aria-label': 'İçindekiler',
        onClick: function () {
          showBars();
          renderTocList();
          navTabs.select('toc');
          ui.openSheet(navSheet.dialog);
        }
      },
      [icon('format_list_bulleted')]
    );
    var progFill = el('div', { className: 'progress-linear-bar' });
    var progTrack = el(
      'div',
      {
        className: 'progress-linear',
        role: 'progressbar',
        'aria-label': 'Okuma ilerlemesi',
        'aria-valuemin': '0',
        'aria-valuemax': '100',
        'aria-valuenow': '0'
      },
      [progFill]
    );
    var indicator = el('span', { id: 'text-indicator', className: 'type-label-medium', text: '' });
    var eta = ui.etaLabel(book);
    var prevChBtn = el(
      'button',
      {
        type: 'button',
        className: 'icon-btn',
        'aria-label': 'Önceki bölüm',
        onClick: function () {
          loadChapter(curChapter - 1, 0, true);
        }
      },
      [icon('chevron_left')]
    );
    var nextChBtn = el(
      'button',
      {
        type: 'button',
        className: 'icon-btn',
        'aria-label': 'Sonraki bölüm',
        onClick: function () {
          loadChapter(curChapter + 1, 0, true);
        }
      },
      [icon('chevron_right')]
    );
    var bottomBar = el('div', { className: 'reader-bar bottom' }, [
      tocBtn,
      prevChBtn,
      el('div', { className: 'reader-progress-wrap' }, [progTrack, indicator, eta.el]),
      nextChBtn
    ]);

    var nextVolume = ui.nextVolumeCard(book);

    // ---- İçindekiler / Yer imleri / Ara alt sayfası ------------------------------
    var navSheet = ui.sheetShell({ title: 'İçindekiler', label: 'İçindekiler' });
    var navTitle = navSheet.head.firstChild;
    var tocList = el('ul', { className: 'list' });
    var searchUi = ui.searchPanel(book, function (chIdx, p) {
      navSheet.dialog.close();
      gotoPosition(chIdx, p);
    });
    var bmList = bookmarks.buildList(function (bm) {
      try {
        var loc = JSON.parse(bm.locator);
        navSheet.dialog.close();
        gotoPosition(Number(loc.ch) || 0, clamp01(loc.p));
      } catch (_) {}
    });
    var TAB_TITLES = { toc: 'İçindekiler', marks: 'Yer imleri', search: 'Ara' };
    var navTabs = ui.tabs(
      'nav',
      [
        { key: 'toc', label: 'İçindekiler' },
        { key: 'marks', label: 'Yer imleri' },
        { key: 'search', label: 'Ara' }
      ],
      function (key) {
        navTitle.textContent = TAB_TITLES[key];
      }
    );
    navTabs.panels.toc.appendChild(tocList);
    navTabs.panels.marks.appendChild(bmList);
    navTabs.panels.search.appendChild(searchUi.el);
    navSheet.body.appendChild(navTabs.list);
    ['toc', 'marks', 'search'].forEach(function (k) {
      navSheet.body.appendChild(navTabs.panels[k]);
    });
    navTabs.select('toc');

    function renderTocList() {
      clearNode(tocList);
      for (var i = 0; i < chapters.length; i++) {
        (function (chap) {
          var isCur = chap.index === curChapter;
          var btn = el(
            'button',
            {
              type: 'button',
              className: 'list-item' + (isCur ? ' is-current' : ''),
              'aria-current': isCur ? 'true' : false,
              onClick: function () {
                navSheet.dialog.close();
                loadChapter(chap.index, 0, true);
              }
            },
            [el('span', { className: 'list-title', text: chap.title }), isCur ? icon('check') : null]
          );
          tocList.appendChild(el('li', {}, [btn]));
        })(chapters[i]);
      }
    }

    // ---- Ayarlar alt sayfası -----------------------------------------------------
    var settings = ui.textSettingsSheet(function (next) {
      prefs = next;
      applyFrameStyles();
      relayout();
      updateProgressUi();
    });

    readerEl.appendChild(topBar);
    readerEl.appendChild(stage);
    readerEl.appendChild(bottomBar);
    if (nextVolume) readerEl.appendChild(nextVolume);
    readerEl.appendChild(navSheet.dialog);
    readerEl.appendChild(settings.dialog);
    mount.appendChild(readerEl);

    // ---- Çubukları gizleme ---------------------------------------------------------
    var hideTimer = null;
    function sheetOpen() {
      return navSheet.dialog.open || settings.dialog.open;
    }
    function scheduleHideBars() {
      if (hideTimer) clearTimeout(hideTimer);
      hideTimer = setTimeout(function () {
        if (sheetOpen()) return;
        var active = document.activeElement;
        if (active && (topBar.contains(active) || bottomBar.contains(active))) {
          return;
        }
        topBar.classList.add('hidden');
        bottomBar.classList.add('hidden');
      }, 3000);
    }

    function showBars() {
      topBar.classList.remove('hidden');
      bottomBar.classList.remove('hidden');
      scheduleHideBars();
    }

    function toggleBars() {
      if (topBar.classList.contains('hidden')) {
        showBars();
      } else {
        topBar.classList.add('hidden');
        bottomBar.classList.add('hidden');
      }
    }

    // ---- İlerleme ------------------------------------------------------------------
    function overallRatio() {
      return (curChapter + clamp01(curRatio)) / totalChapters;
    }

    function updateNextVolume() {
      if (!nextVolume) return;
      var atEnd = isPaged() ? pageIdx >= pageTotal - 1 : curRatio >= 0.98;
      nextVolume.setVisible(frameReady && curChapter === totalChapters - 1 && atEnd);
    }

    function updateProgressUi() {
      var overall = overallRatio();
      var pct = Math.round(overall * 100);
      progFill.style.width = pct + '%';
      progTrack.setAttribute('aria-valuenow', String(pct));
      var label = 'Bölüm ' + (curChapter + 1);
      if (isPaged()) label += ' · sayfa ' + (pageIdx + 1) + '/' + pageTotal;
      indicator.textContent = label + ' · %' + pct;
      eta.update(overall);
      bookmarks.sync();
      updateNextVolume();
      return overall;
    }

    function persist(overall) {
      // yeni çerçeve hazır olana kadar eski konum kaydedilmez
      if (!KRS.tracker || !frameReady) return;
      KRS.tracker.scheduleProgress(
        book.id,
        JSON.stringify({ ch: curChapter, p: Number(curRatio.toFixed(6)) }),
        overall
      );
    }

    function updateNavButtons() {
      prevChBtn.disabled = curChapter <= 0;
      nextChBtn.disabled = curChapter >= totalChapters - 1;
    }

    // ---- Çerçeve biçimleri ---------------------------------------------------------
    function frameDoc() {
      return iframe.contentDocument;
    }

    function scrollEl(doc) {
      return doc && (doc.scrollingElement || doc.documentElement || doc.body);
    }

    function applyFrameStyles() {
      var seed = window.KRS_THEME ? window.KRS_THEME.getSeed() : '#6750A4';
      var rc = window.KRS_THEME
        ? window.KRS_THEME.readerColors(ui.resolveTheme(prefs), seed)
        : { bg: '#FEF7FF', fg: '#1D1B20', link: '#6750A4' };
      readerEl.style.background = rc.bg;
      stage.style.background = rc.bg;
      iframe.style.background = rc.bg;
      var doc = frameDoc();
      if (!doc || !doc.documentElement) return;
      var root = doc.documentElement;
      var st = root.style;
      st.setProperty('--bg', rc.bg);
      st.setProperty('--fg', rc.fg);
      st.setProperty('--link', rc.link);
      st.setProperty('--fs', prefs.fs + 'px');
      st.setProperty('--lh', String(prefs.lh));
      st.setProperty('--ws', prefs.ws + 'em');
      st.setProperty('--ls', prefs.ls + 'em');
      st.setProperty('--font', ui.fontFamily(prefs.font));
      st.setProperty('--align', prefs.align);
      st.setProperty('--hyph', prefs.align === 'justify' ? 'auto' : 'manual');
      root.classList.toggle('paged', isPaged());
      // yatay kaydırılan belge klavye erişimi için odaklanabilir olmalı
      if (isPaged()) {
        root.setAttribute('tabindex', '0');
      } else {
        root.removeAttribute('tabindex');
      }
    }

    // Sayfa/kaydırma düzenini yeniden hesaplar ve konumu curRatio ile korur.
    // shortcut: oran korunur, yeniden akışta en çok ±1 sayfa kayar; gerekirse caret anchor
    function relayout() {
      if (disposed) return;
      var doc = frameDoc();
      var win = iframe.contentWindow;
      if (!frameReady || !doc || !doc.body || !win) return;
      var pw = iframe.clientWidth;
      var ph = iframe.clientHeight;
      var m = Math.max(16 + (pw * prefs.mg) / 100, (pw - 38 * prefs.fs) / 2);
      var st = doc.documentElement.style;
      st.setProperty('--pw', pw + 'px');
      st.setProperty('--ph', ph + 'px');
      st.setProperty('--m', m + 'px');
      st.setProperty('--bh', Math.max(0, ph - BAR_V) + 'px');
      st.setProperty('--mx', Math.round((pw * prefs.mg) / 100) + 'px');
      var se = scrollEl(doc);
      restoring = true;
      if (isPaged()) {
        var range = doc.createRange();
        range.selectNodeContents(doc.body);
        var contentRight = range.getBoundingClientRect().right + (se ? se.scrollLeft : 0);
        pageTotal = calc.pageCount(contentRight, pw);
        pageIdx = calc.pageFromRatio(curRatio, pageTotal);
        win.scrollTo({ left: pageIdx * pw, top: 0, behavior: 'auto' });
      } else {
        pageTotal = 1;
        pageIdx = 0;
        if (se) {
          var maxScroll = Math.max(0, se.scrollHeight - se.clientHeight);
          se.scrollTop = Math.round(curRatio * maxScroll);
        }
      }
      // kaydırma olayları eşzamansız gelir; kilit çift requestAnimationFrame sonrası açılır
      var gen = frameGen;
      requestAnimationFrame(function () {
        requestAnimationFrame(function () {
          if (gen !== frameGen) return;
          restoring = false;
          // kilit sırasında gelen kaydırma kaybolmaz: kilit açılınca işlenir
          if (deferredScroll) {
            deferredScroll = false;
            if (isPaged()) {
              scheduleSnap();
            } else if (frameReady) {
              onScrollMode(frameDoc());
            }
          }
        });
      });
      // tek sayfa/ekranlık bölümde oran 1 sayılır ve kaydedilir (son bölümse kitap %100)
      var single = isPaged()
        ? pageTotal <= 1
        : se
          ? se.scrollHeight - se.clientHeight <= 2
          : false;
      var promoted = single && curRatio < 1;
      if (promoted) curRatio = 1;
      var overall = updateProgressUi();
      if (promoted) {
        if (KRS.tracker) KRS.tracker.addPage(curChapter + ':' + pageIdx);
        persist(overall);
      }
    }

    // Yeniden düzen beklerken eski ölçüye göre yapışma yapılmaz (konumu bozar).
    function scheduleRelayout() {
      if (relayoutTimer) clearTimeout(relayoutTimer);
      if (snapTimer) clearTimeout(snapTimer);
      relayoutTimer = setTimeout(function () {
        relayoutTimer = null;
        relayout();
      }, RELAYOUT_DELAY);
    }

    // ---- Sayfa modu --------------------------------------------------------------
    function commitPage(i) {
      pageIdx = i;
      curRatio = calc.ratioFromPage(i, pageTotal);
      var overall = updateProgressUi();
      if (KRS.tracker) KRS.tracker.addPage(curChapter + ':' + i);
      persist(overall);
    }

    function goPage(i) {
      if (!isPaged() || !frameReady) return;
      if (i >= pageTotal) {
        if (curChapter + 1 < totalChapters) {
          loadChapter(curChapter + 1, 0, true);
        } else {
          updateNextVolume();
        }
        return;
      }
      if (i < 0) {
        if (curChapter > 0) loadChapter(curChapter - 1, 1, true);
        return;
      }
      if (i === pageIdx) return;
      var win = iframe.contentWindow;
      var pw = iframe.clientWidth;
      commitPage(i);
      if (win) {
        win.scrollTo({ left: i * pw, top: 0, behavior: prefersReducedMotion() ? 'auto' : 'smooth' });
      }
    }

    // Kullanıcı kaydırması bitince en yakın sayfaya yapışır ve sayfa sayılır.
    function scheduleSnap() {
      if (snapTimer) clearTimeout(snapTimer);
      snapTimer = setTimeout(function () {
        if (relayoutTimer) return;
        var doc = frameDoc();
        var win = iframe.contentWindow;
        var se = scrollEl(doc);
        if (!se || !win || !isPaged()) return;
        var pw = iframe.clientWidth;
        var idx = Math.max(0, Math.min(pageTotal - 1, Math.round(se.scrollLeft / pw)));
        if (Math.abs(se.scrollLeft - idx * pw) > 1) {
          win.scrollTo({ left: idx * pw, top: 0, behavior: 'auto' });
        }
        if (idx !== pageIdx) commitPage(idx);
      }, SNAP_DELAY);
    }

    // ---- Kaydırma modu ---------------------------------------------------------------
    function onScrollMode(doc) {
      var se = scrollEl(doc);
      if (!se) return;
      var maxScroll = Math.max(0, se.scrollHeight - se.clientHeight);
      var ratio = maxScroll > 0 ? clamp01(se.scrollTop / maxScroll) : 0;
      // kesirli kaydırma/yakınlaştırmada 0.997'de kalmasın: alt uca varınca tam 1
      if (se.scrollTop + se.clientHeight >= se.scrollHeight - 2) ratio = 1;
      curRatio = ratio;
      var overall = updateProgressUi();
      var screenIdx = Math.floor(se.scrollTop / Math.max(1, se.clientHeight));
      if (KRS.tracker) {
        KRS.tracker.addPage(curChapter + ':' + screenIdx);
      }
      persist(overall);
    }

    // ---- Çerçeve olayları ------------------------------------------------------------
    // Eski belgenin dinleyicileri çerçeve değişirken kaldırılır.
    function unbindFrame() {
      var list = unbinders;
      unbinders = [];
      list.forEach(function (fn) {
        try {
          fn();
        } catch (_) {}
      });
    }

    function bindFrameEvents() {
      var doc = frameDoc();
      var win = iframe.contentWindow;
      if (!doc || !win) return;
      var myFrame = frameGen;
      function on(target, type, fn, opts) {
        var wrapped = function (e) {
          if (myFrame !== frameGen) return;
          fn(e);
        };
        target.addEventListener(type, wrapped, opts);
        unbinders.push(function () {
          target.removeEventListener(type, wrapped, opts);
        });
      }

      var down = null;
      on(
        doc,
        'pointerdown',
        function (e) {
          down = { x: e.clientX, y: e.clientY };
        },
        { passive: true }
      );

      on(doc, 'click', function (e) {
        if (KRS.tracker) KRS.tracker.touch();
        var a = e.target.closest ? e.target.closest('a') : null;
        if (a) {
          e.preventDefault();
          showBars();
          var href = a.getAttribute('href') || '';
          if (href.indexOf('http://') === 0 || href.indexOf('https://') === 0) {
            window.open(href, '_blank', 'noopener,noreferrer');
          }
          return;
        }
        if (!isPaged()) {
          showBars();
          return;
        }
        // sürükleme veya yeni biten kaydırma dokunma sayılmaz (çift sayım olmaz)
        var moved = down && (Math.abs(e.clientX - down.x) > 10 || Math.abs(e.clientY - down.y) > 10);
        down = null;
        if (moved || Date.now() - lastScrollAt < TAP_GUARD_MS) return;
        var pw = iframe.clientWidth || 1;
        var zone = e.clientX / pw;
        if (zone < 1 / 3) {
          goPage(pageIdx - 1);
        } else if (zone > 2 / 3) {
          goPage(pageIdx + 1);
        } else {
          toggleBars();
        }
      });

      on(
        doc,
        'pointermove',
        function () {
          showBars();
          if (KRS.tracker) KRS.tracker.touch();
        },
        { passive: true }
      );

      on(doc, 'keydown', function (e) {
        handleKey(e);
      });

      // görseller ve yazı tipleri yüklendikçe düzen değişir
      on(doc, 'load', scheduleRelayout, true);
      if (doc.fonts && doc.fonts.addEventListener) {
        on(doc.fonts, 'loadingdone', scheduleRelayout);
      }

      on(
        win,
        'scroll',
        function () {
          lastScrollAt = Date.now();
          showBars();
          if (restoring) {
            deferredScroll = true;
            return;
          }
          if (isPaged()) {
            scheduleSnap();
          } else {
            onScrollMode(doc);
          }
        },
        { passive: true }
      );
    }

    // ---- Bölüm yükleme ---------------------------------------------------------------
    var errBox = null;
    function clearErrBox() {
      if (errBox && errBox.parentNode) {
        errBox.parentNode.removeChild(errBox);
      }
      errBox = null;
      iframe.style.display = '';
    }

    function loadChapter(idx, targetRatio, saveProg) {
      if (idx < 0 || idx >= totalChapters) return;
      clearErrBox();
      frameReady = false;
      frameGen++;
      restoring = true;
      unbindFrame();
      if (snapTimer) clearTimeout(snapTimer);
      curChapter = idx;
      curRatio = clamp01(targetRatio);
      updateNavButtons();
      applyFrameStyles();
      updateProgressUi();

      iframe.style.opacity = '0';
      iframe.onload = function () {
        if (disposed || (myGen !== undefined && myGen !== KRS.routeGen)) return;
        var doc = frameDoc();
        if (!doc || doc.contentType !== 'text/html') {
          var errMsg = 'Bölüm yüklenemedi.';
          if (doc && doc.body) {
            try {
              var errJson = JSON.parse(doc.body.textContent || '');
              if (errJson && errJson.error === 'unauthorized') {
                if (KRS.navigate) KRS.navigate('#/giris');
                return;
              }
              if (errJson && errJson.message) {
                errMsg = errJson.message;
              }
            } catch (_) {}
          }
          clearErrBox();
          iframe.style.display = 'none';
          errBox = el('div', { className: 'empty error' }, [
            icon('error'),
            el('h2', { text: 'Yüklenemedi' }),
            el('p', { text: errMsg }),
            el('button', {
              type: 'button',
              className: 'btn tonal',
              text: 'Yeniden dene',
              onClick: function () {
                clearErrBox();
                loadChapter(curChapter, curRatio, saveProg);
              }
            })
          ]);
          stage.appendChild(errBox);
          return;
        }
        frameReady = true;
        applyFrameStyles();
        bindFrameEvents();
        relayout();
        if (KRS.tracker) {
          if (isPaged()) {
            KRS.tracker.addPage(curChapter + ':' + pageIdx);
          } else {
            var se = scrollEl(doc);
            if (se) {
              KRS.tracker.addPage(
                curChapter + ':' + Math.floor(se.scrollTop / Math.max(1, se.clientHeight))
              );
            }
          }
        }
        if (saveProg) persist(overallRatio());
        iframe.style.opacity = '1';
      };
      iframe.src = '/api/books/' + book.id + '/chapters/' + curChapter;
    }

    // Aynı bölümdeyse çerçeve yeniden yüklenmeden konuma gider.
    function gotoPosition(chIdx, p) {
      var target = Math.max(0, Math.min(totalChapters - 1, chIdx));
      if (target === curChapter && frameReady) {
        curRatio = clamp01(p);
        relayout();
        persist(overallRatio());
      } else {
        loadChapter(target, p, true);
      }
    }

    // ---- Tuşlar ------------------------------------------------------------------------
    function handleKey(e) {
      showBars();
      if (e.ctrlKey || e.metaKey || e.altKey) return;
      if (navSheet.dialog.open) {
        if (e.key === 'Escape') navSheet.dialog.close();
        return;
      }
      if (settings.dialog.open) {
        if (e.key === 'Escape') settings.dialog.close();
        return;
      }
      // odaklı kontrol (düğme, bağlantı, alan) Boşluk/Enter'ı kendisi işler
      if (
        (e.key === ' ' || e.key === 'Enter') &&
        e.target &&
        e.target.closest &&
        e.target.closest('button, a, input, select, textarea, [role="radio"], [role="tab"]')
      ) {
        return;
      }
      var doc = frameDoc();
      var se = scrollEl(doc);
      var paged = isPaged();
      if (e.key === '[') {
        e.preventDefault();
        loadChapter(curChapter - 1, 0, true);
      } else if (e.key === ']') {
        e.preventDefault();
        loadChapter(curChapter + 1, 0, true);
      } else if (paged && (e.key === 'ArrowLeft' || e.key === 'PageUp' || (e.key === ' ' && e.shiftKey))) {
        e.preventDefault();
        goPage(pageIdx - 1);
      } else if (paged && (e.key === 'ArrowRight' || e.key === 'PageDown' || e.key === ' ')) {
        e.preventDefault();
        goPage(pageIdx + 1);
      } else if (paged && (e.key === 'ArrowUp' || e.key === 'ArrowDown')) {
        // sayfa modunda yok sayılır
      } else if (e.key === 'ArrowLeft') {
        e.preventDefault();
        loadChapter(curChapter - 1, 0, true);
      } else if (e.key === 'ArrowRight') {
        e.preventDefault();
        loadChapter(curChapter + 1, 0, true);
      } else if (e.key === 'ArrowDown') {
        e.preventDefault();
        if (se) se.scrollTop += 60;
      } else if (e.key === 'ArrowUp') {
        e.preventDefault();
        if (se) se.scrollTop -= 60;
      } else if (e.key === 'PageDown' || (e.key === ' ' && !e.shiftKey)) {
        e.preventDefault();
        if (se) {
          if (se.scrollTop + se.clientHeight >= se.scrollHeight - 2) {
            if (curChapter + 1 < totalChapters) {
              loadChapter(curChapter + 1, 0, true);
            }
          } else {
            se.scrollTop += Math.max(40, Math.floor(se.clientHeight * 0.9));
          }
        }
      } else if (e.key === 'PageUp' || (e.key === ' ' && e.shiftKey)) {
        e.preventDefault();
        if (se) {
          se.scrollTop -= Math.max(40, Math.floor(se.clientHeight * 0.9));
        }
      } else if (e.key === 'Escape') {
        e.preventDefault();
        KRS.navigate('#/seri/' + book.series_id);
      }
    }

    function onUserMove() {
      showBars();
    }

    var resizeObs = null;
    if (window.ResizeObserver) {
      resizeObs = new ResizeObserver(scheduleRelayout);
      resizeObs.observe(stage);
    }

    var releaseAwake = ui.keepAwake();

    if (KRS.tracker) {
      KRS.tracker.start(book.id, (curChapter + curRatio) / totalChapters);
    }
    applyFrameStyles();
    loadChapter(curChapter, curRatio, false);
    scheduleHideBars();

    window.addEventListener('keydown', handleKey);
    window.addEventListener('pointermove', onUserMove, { passive: true });

    return function cleanup() {
      disposed = true;
      if (hideTimer) clearTimeout(hideTimer);
      if (relayoutTimer) clearTimeout(relayoutTimer);
      if (snapTimer) clearTimeout(snapTimer);
      if (resizeObs) resizeObs.disconnect();
      unbindFrame();
      searchUi.cleanup();
      releaseAwake();
      if (fsBtn && fsBtn.cleanup) fsBtn.cleanup();
      window.removeEventListener('keydown', handleKey);
      window.removeEventListener('pointermove', onUserMove);
    };
  }

  KRS.readers.text = { open: open };
})();
