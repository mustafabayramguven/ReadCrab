(function () {
  'use strict';

  var KRS = (window.KRS = window.KRS || {});
  KRS.readers = KRS.readers || {};

  var FIT_MODES = [
    { key: 'width', label: 'Genişlik', ariaLabel: 'Genişliğe sığdır' },
    { key: 'height', label: 'Yükseklik', ariaLabel: 'Yüksekliğe sığdır' },
    { key: 'page', label: 'Sayfa', ariaLabel: 'Sayfaya sığdır' },
    { key: 'actual', label: 'Gerçek', ariaLabel: 'Gerçek boyut' }
  ];

  function getSavedFit() {
    try {
      var v = localStorage.getItem('krs.fit');
      for (var i = 0; i < FIT_MODES.length; i++) {
        if (FIT_MODES[i].key === v) return v;
      }
    } catch (_) {}
    return 'page';
  }

  function saveFit(v) {
    try {
      localStorage.setItem('krs.fit', v);
    } catch (_) {}
  }

  function getSaved(key, allowed, def) {
    try {
      var v = localStorage.getItem(key);
      if (allowed.indexOf(v) >= 0) return v;
    } catch (_) {}
    return def;
  }

  function save(key, v) {
    try {
      localStorage.setItem(key, v);
    } catch (_) {}
  }

  async function open(book, mount, gen) {
    var myGen = gen !== undefined ? gen : KRS.routeGen;
    var el = KRS.el;
    var icon = KRS.icon;
    var clearNode = KRS.clearNode;
    var ui = KRS.readerUi;
    var calc = window.KRS_CALC;
    clearNode(mount);

    var pagesRes = await KRS.api('GET', '/api/books/' + book.id + '/pages');
    if (myGen !== undefined && myGen !== KRS.routeGen) {
      return null;
    }
    var total = Math.max(1, Number((pagesRes && pagesRes.count) || book.page_count || 1));
    var initialPage = parseInt(book.locator, 10);
    if (!Number.isFinite(initialPage) || initialPage < 0 || initialPage >= total) {
      initialPage = 0;
    }
    var curPage = initialPage;
    var curFit = getSavedFit();
    var dirKey = 'krs.dir.' + book.id;
    var modeKey = 'krs.cmode.' + book.id;
    var curDir = getSaved(dirKey, ['ltr', 'rtl'], 'ltr');
    var curMode = getSaved(modeKey, ['page', 'vertical'], 'page');
    var disposed = false;

    var readerEl = el('div', { className: 'reader comic' });
    var black = Boolean(window.KRS_THEME && window.KRS_THEME.getBlack && window.KRS_THEME.getBlack());
    if (window.KRS_THEME) {
      var darkPal = window.KRS_THEME.generatePalette(window.KRS_THEME.getSeed(), true, black);
      var palKeys = Object.keys(darkPal);
      for (var pk = 0; pk < palKeys.length; pk++) {
        readerEl.style.setProperty('--md-sys-color-' + palKeys[pk], darkPal[palKeys[pk]]);
      }
      readerEl.style.background = black ? '#000000' : darkPal['surface-container-lowest'];
      readerEl.style.color = darkPal['on-surface'];
    }

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
          openSettingsSheet();
        }
      },
      [icon('tune')]
    );
    var bookmarks = ui.bookmarkController(book, {
      getLocator: function () {
        return String(curPage);
      },
      getLabel: function () {
        return 'Sayfa ' + (curPage + 1);
      },
      matches: function (bm) {
        return bm.locator === String(curPage);
      }
    });
    var fsBtn = ui.fullscreenButton();
    var topBar = el(
      'div',
      { className: 'reader-bar top' },
      [backBtn, titleEl, bookmarks.toggleBtn, fsBtn, settingsBtn].filter(Boolean)
    );

    // ---- Sahne -----------------------------------------------------------------
    var pageImg = el('img', {
      className: 'reader-page-img',
      alt: book.title + ' sayfa'
    });
    var stage = el('div', { className: 'reader-stage fit-' + curFit }, [pageImg]);
    var webtoon = null;

    // ---- Alt çubuk -------------------------------------------------------------
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
    var pageInd = el('span', {
      id: 'page-indicator',
      className: 'type-label-medium',
      text: curPage + 1 + ' / ' + total
    });
    var eta = ui.etaLabel(book);
    var prevBtn = el(
      'button',
      {
        type: 'button',
        className: 'icon-btn',
        'aria-label': 'Önceki sayfa',
        onClick: function () {
          step(-1);
        }
      },
      [icon('chevron_left')]
    );
    var nextBtn = el(
      'button',
      {
        type: 'button',
        className: 'icon-btn',
        'aria-label': 'Sonraki sayfa',
        onClick: function () {
          step(1);
        }
      },
      [icon('chevron_right')]
    );
    var progWrap = el('div', { className: 'reader-progress-wrap' }, [progTrack, pageInd, eta.el]);
    var bottomBar = el('div', { className: 'reader-bar bottom' }, [progWrap, prevBtn, nextBtn]);
    var nextVolume = ui.nextVolumeCard(book);

    // ---- Ayarlar alt sayfası -----------------------------------------------------
    var shell = ui.sheetShell({
      title: 'Okuma ayarları',
      label: 'Okuyucu ayarları',
      className: 'reader-settings'
    });
    var sheet = shell.dialog;

    var viewSeg = ui.segmented(
      'Görünüm',
      [
        { key: 'page', label: 'Sayfa' },
        { key: 'vertical', label: 'Dikey' }
      ],
      curMode,
      function (k) {
        setMode(k);
      }
    );
    var dirSeg = ui.segmented(
      'Okuma yönü',
      [
        { key: 'ltr', label: 'Soldan sağa' },
        { key: 'rtl', label: 'Sağdan sola' }
      ],
      curDir,
      function (k) {
        setDir(k);
      }
    );
    var fitGroup = el('div', {
      className: 'segmented',
      role: 'radiogroup',
      'aria-label': 'Sığdırma modu'
    });
    var fitSection = ui.section('Sığdırma modu', fitGroup);
    var listSheet = ui.sheetShell({ title: 'Yer imleri', label: 'Yer imleri' });
    listSheet.body.appendChild(
      bookmarks.buildList(function (bm) {
        var n = parseInt(bm.locator, 10);
        if (!Number.isFinite(n)) return;
        listSheet.dialog.close();
        if (curMode === 'vertical') {
          scrollToPage(n);
        } else {
          goToPage(n, true);
        }
      })
    );
    var marksBtn = el('button', {
      type: 'button',
      className: 'btn tonal rs-reset',
      text: 'Yer imleri',
      onClick: function () {
        sheet.close();
        ui.openSheet(listSheet.dialog);
      }
    });
    [
      ui.section('Görünüm', viewSeg.el),
      ui.section('Okuma yönü', dirSeg.el),
      fitSection,
      el('div', { className: 'rs-actions' }, [marksBtn])
    ].forEach(function (n) {
      shell.body.appendChild(n);
    });

    function applyFit(mode) {
      curFit = mode;
      saveFit(mode);
      if (curMode === 'page') stage.className = 'reader-stage fit-' + curFit;
      clearNode(fitGroup);
      for (var i = 0; i < FIT_MODES.length; i++) {
        (function (m) {
          var sel = m.key === curFit;
          var ch = sel ? [icon('check'), m.label] : [m.label];
          fitGroup.appendChild(
            el(
              'button',
              {
                type: 'button',
                className: 'seg-btn',
                role: 'radio',
                'aria-label': m.ariaLabel,
                'aria-checked': sel ? 'true' : 'false',
                onClick: function () {
                  applyFit(m.key);
                }
              },
              ch
            )
          );
        })(FIT_MODES[i]);
      }
    }
    applyFit(curFit);

    function openSettingsSheet() {
      showBars();
      ui.openSheet(sheet);
    }

    readerEl.appendChild(topBar);
    readerEl.appendChild(stage);
    readerEl.appendChild(bottomBar);
    if (nextVolume) readerEl.appendChild(nextVolume);
    readerEl.appendChild(sheet);
    readerEl.appendChild(listSheet.dialog);
    mount.appendChild(readerEl);

    // ---- Çubuklar ---------------------------------------------------------------
    var hideTimer = null;
    function anySheetOpen() {
      return sheet.open || listSheet.dialog.open;
    }
    function scheduleHideBars() {
      if (hideTimer) clearTimeout(hideTimer);
      hideTimer = setTimeout(function () {
        if (anySheetOpen()) return;
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

    // ---- Yön ve görünüm -----------------------------------------------------------
    function rtl() {
      return curDir === 'rtl' && curMode === 'page';
    }

    function layoutBottom() {
      clearNode(bottomBar);
      var wrap = progWrap;
      if (curMode === 'vertical') {
        prevBtn.hidden = true;
        nextBtn.hidden = true;
        bottomBar.appendChild(wrap);
        return;
      }
      prevBtn.hidden = false;
      nextBtn.hidden = false;
      var r = rtl();
      prevBtn.replaceChild(icon(r ? 'chevron_right' : 'chevron_left'), prevBtn.firstChild);
      nextBtn.replaceChild(icon(r ? 'chevron_left' : 'chevron_right'), nextBtn.firstChild);
      bottomBar.appendChild(wrap);
      if (r) {
        bottomBar.appendChild(nextBtn);
        bottomBar.appendChild(prevBtn);
      } else {
        bottomBar.appendChild(prevBtn);
        bottomBar.appendChild(nextBtn);
      }
    }

    function setDir(d) {
      curDir = d;
      save(dirKey, d);
      dirSeg.set(d);
      layoutBottom();
    }

    function setMode(m) {
      if (m === curMode) return;
      var keep = curPage;
      curMode = m;
      save(modeKey, m);
      viewSeg.set(m);
      dirSeg.setDisabled(null, m === 'vertical');
      fitSection.hidden = m === 'vertical';
      curPage = keep;
      if (m === 'vertical') {
        enterVertical();
      } else {
        leaveVertical();
        goToPage(keep, false, true);
      }
      layoutBottom();
    }

    // ---- Konum bilgisi ------------------------------------------------------------
    function updateUi() {
      var pct = (curPage + 1) / total;
      var pct100 = Math.round(pct * 100);
      pageInd.textContent = curPage + 1 + ' / ' + total;
      progFill.style.width = pct100 + '%';
      progTrack.setAttribute('aria-valuenow', String(pct100));
      eta.update(pct);
      bookmarks.sync();
      return pct;
    }

    function record(saveProg) {
      var pct = updateUi();
      if (KRS.tracker) {
        KRS.tracker.addPage(curPage);
        if (saveProg) {
          KRS.tracker.scheduleProgress(book.id, String(curPage), pct);
        }
      }
    }

    var isRar = book.format === 'rar' || book.format === 'cbr';
    function preloadNeighbors(idx) {
      if (isRar) {
        if (idx + 1 < total) {
          var nextRarImg = new Image();
          nextRarImg.src = '/api/books/' + book.id + '/pages/' + (idx + 1);
        }
        return;
      }
      for (var f = 1; f <= 3; f++) {
        if (idx + f < total) {
          var nextImg = new Image();
          nextImg.src = '/api/books/' + book.id + '/pages/' + (idx + f);
        }
      }
      if (idx - 1 >= 0) {
        var prevImg = new Image();
        prevImg.src = '/api/books/' + book.id + '/pages/' + (idx - 1);
      }
    }

    // ---- Sayfa modu ----------------------------------------------------------------
    var errBox = null;
    function clearErrBox() {
      if (errBox && errBox.parentNode) {
        errBox.parentNode.removeChild(errBox);
      }
      errBox = null;
      pageImg.style.display = '';
    }

    function goToPage(idx, saveProg, force) {
      if (curMode !== 'page') return;
      var target = Math.max(0, Math.min(total - 1, idx));
      if (!force && target === curPage && pageImg.getAttribute('src') && !errBox) return;
      clearErrBox();
      curPage = target;

      pageImg.style.opacity = '0';
      pageImg.onload = function () {
        clearErrBox();
        pageImg.style.opacity = '1';
      };
      pageImg.onerror = function () {
        clearErrBox();
        pageImg.style.display = 'none';
        errBox = el('div', { className: 'empty error' }, [
          icon('error'),
          el('h2', { text: 'Yüklenemedi' }),
          el('p', { text: 'Sayfa yüklenemedi.' }),
          el('button', {
            type: 'button',
            className: 'btn tonal',
            text: 'Yeniden dene',
            onClick: function (ev) {
              ev.stopPropagation();
              clearErrBox();
              pageImg.removeAttribute('src');
              goToPage(curPage, saveProg);
            }
          })
        ]);
        stage.appendChild(errBox);
      };
      pageImg.src = '/api/books/' + book.id + '/pages/' + curPage;
      stage.scrollTop = 0;
      stage.scrollLeft = 0;

      record(saveProg);
      if (nextVolume) nextVolume.setVisible(curPage === total - 1);
      preloadNeighbors(curPage);
    }

    // ileri (+1) / geri (-1): sağdan sola okumada tuş ve dokunma yönleri tersine çevrilir
    function step(delta) {
      if (curMode === 'vertical') {
        scrollWebtoon(delta);
        return;
      }
      goToPage(curPage + delta, true);
    }

    // ---- Dikey (webtoon) mod --------------------------------------------------------
    var observer = null;
    var holders = [];
    var loadQueue = [];
    var loading = 0;
    var scrollRaf = 0;
    var restoring = false;
    var restoreTimer = null;

    function maxLoads() {
      return isRar ? 2 : 6;
    }

    function pumpLoads() {
      while (loading < maxLoads() && loadQueue.length > 0) {
        var h = loadQueue.shift();
        if (h.img) continue;
        startLoad(h);
      }
    }

    function startLoad(h) {
      var idx = Number(h.node.getAttribute('data-idx'));
      var img = el('img', {
        className: 'webtoon-img',
        loading: 'lazy',
        alt: 'Sayfa ' + (idx + 1)
      });
      h.img = img;
      loading++;
      function done() {
        loading--;
        h.node.classList.add('loaded');
        pumpLoads();
      }
      img.addEventListener('load', done, { once: true });
      img.addEventListener(
        'error',
        function () {
          // sayfa çökmez: yer tutucu kalır, hata kutusu ve "Yeniden dene" gösterilir
          h.img = null;
          if (img.parentNode) h.node.removeChild(img);
          loading--;
          pumpLoads();
          var box = el('div', { className: 'empty error' }, [
            icon('error'),
            el('p', { text: 'Sayfa yüklenemedi.' }),
            el('button', {
              type: 'button',
              className: 'btn tonal',
              text: 'Yeniden dene',
              onClick: function () {
                if (box.parentNode) box.parentNode.removeChild(box);
                requestLoad(idx);
              }
            })
          ]);
          h.node.appendChild(box);
        },
        { once: true }
      );
      img.src = '/api/books/' + book.id + '/pages/' + idx;
      h.node.appendChild(img);
    }

    function requestLoad(idx) {
      var h = holders[idx];
      if (!h || h.img || loadQueue.indexOf(h) >= 0) return;
      loadQueue.push(h);
      pumpLoads();
    }

    function visibleIndex() {
      // alta ulaşıldıysa kısa da olsa son sayfa görünürdür
      if (webtoon.scrollTop + webtoon.clientHeight >= webtoon.scrollHeight - 2) {
        return holders.length - 1;
      }
      var top = webtoon.scrollTop + 0.3 * webtoon.clientHeight;
      for (var i = 0; i < holders.length; i++) {
        var n = holders[i].node;
        if (n.offsetTop + n.offsetHeight > top) return i;
      }
      return holders.length - 1;
    }

    function atBottom() {
      return webtoon.scrollTop + webtoon.clientHeight >= webtoon.scrollHeight - 4;
    }

    function onWebtoonScroll() {
      if (scrollRaf) return;
      scrollRaf = requestAnimationFrame(function () {
        scrollRaf = 0;
        if (!webtoon || disposed) return;
        var idx = visibleIndex();
        if (idx !== curPage) {
          curPage = idx;
          if (!restoring) record(true);
          else updateUi();
        }
        if (nextVolume) nextVolume.setVisible(curPage === total - 1 && atBottom());
      });
    }

    function scrollToPage(idx) {
      if (!webtoon || !holders[idx]) return;
      holders[idx].node.scrollIntoView({ block: 'start' });
      for (var d = -2; d <= 2; d++) requestLoad(idx + d);
      curPage = idx;
      record(true);
    }

    function scrollWebtoon(dir) {
      if (!webtoon) return;
      webtoon.scrollTop += dir * Math.max(40, Math.floor(webtoon.clientHeight * 0.9));
    }

    function enterVertical() {
      clearErrBox();
      stage.className = 'reader-stage webtoon-mode';
      if (pageImg.parentNode) stage.removeChild(pageImg);
      webtoon = el('div', {
        className: 'webtoon',
        role: 'region',
        'aria-label': 'Dikey okuma',
        tabindex: '0'
      });
      holders = [];
      for (var i = 0; i < total; i++) {
        var node = el('div', { className: 'webtoon-page', 'data-idx': String(i) });
        holders.push({ node: node, img: null });
        webtoon.appendChild(node);
      }
      stage.appendChild(webtoon);
      if ('IntersectionObserver' in window) {
        observer = new IntersectionObserver(
          function (entries) {
            entries.forEach(function (en) {
              if (en.isIntersecting) requestLoad(Number(en.target.getAttribute('data-idx')));
            });
          },
          { root: webtoon, rootMargin: '200% 0px' }
        );
        holders.forEach(function (h) {
          observer.observe(h.node);
        });
      } else {
        for (var j = 0; j < Math.min(total, 6); j++) requestLoad(j);
      }
      webtoon.addEventListener('scroll', onWebtoonScroll, { passive: true });
      webtoon.addEventListener('click', function (e) {
        if (e.target.closest && e.target.closest('a,button')) return;
        toggleBars();
      });
      restoring = true;
      var start = curPage;
      for (var d = -2; d <= 2; d++) requestLoad(start + d);
      holders[start].node.scrollIntoView({ block: 'start' });
      updateUi();
      restoreTimer = setTimeout(function () {
        restoreTimer = null;
        restoring = false;
        // geri yükleme sırasında kullanıcı kaydırdıysa yeni konum da kaydedilir
        record(curPage !== start);
      }, 300);
      if (nextVolume) nextVolume.setVisible(false);
    }

    function leaveVertical() {
      if (restoreTimer) {
        clearTimeout(restoreTimer);
        restoreTimer = null;
      }
      restoring = false;
      if (observer) {
        observer.disconnect();
        observer = null;
      }
      loadQueue = [];
      if (webtoon && webtoon.parentNode) webtoon.parentNode.removeChild(webtoon);
      webtoon = null;
      holders = [];
      if (!pageImg.parentNode) stage.appendChild(pageImg);
      stage.className = 'reader-stage fit-' + curFit;
    }

    dirSeg.setDisabled(null, curMode === 'vertical');
    fitSection.hidden = curMode === 'vertical';
    layoutBottom();

    if (KRS.tracker) {
      KRS.tracker.start(book.id, (curPage + 1) / total);
    }
    if (curMode === 'vertical') {
      enterVertical();
    } else {
      goToPage(curPage, false, true);
    }
    scheduleHideBars();

    function onKeyDown(e) {
      showBars();
      if (e.ctrlKey || e.metaKey || e.altKey) return;
      if (anySheetOpen()) {
        if (e.key === 'Escape') {
          sheet.close();
          listSheet.dialog.close();
        }
        return;
      }
      if (curMode === 'vertical') {
        if (e.key === 'ArrowDown') {
          e.preventDefault();
          if (webtoon) webtoon.scrollTop += 60;
        } else if (e.key === 'ArrowUp') {
          e.preventDefault();
          if (webtoon) webtoon.scrollTop -= 60;
        } else if (e.key === 'PageDown' || (e.key === ' ' && !e.shiftKey)) {
          e.preventDefault();
          scrollWebtoon(1);
        } else if (e.key === 'PageUp' || (e.key === ' ' && e.shiftKey)) {
          e.preventDefault();
          scrollWebtoon(-1);
        } else if (e.key === 'Escape') {
          e.preventDefault();
          KRS.navigate('#/seri/' + book.series_id);
        }
        return;
      }
      var left = rtl() ? 1 : -1;
      if (e.key === 'ArrowRight') {
        e.preventDefault();
        step(-left);
      } else if (e.key === 'ArrowLeft') {
        e.preventDefault();
        step(left);
      } else if (e.key === 'PageDown' || (e.key === ' ' && !e.shiftKey)) {
        e.preventDefault();
        step(1);
      } else if (e.key === 'PageUp' || (e.key === ' ' && e.shiftKey)) {
        e.preventDefault();
        step(-1);
      } else if (e.key === 'Home') {
        e.preventDefault();
        goToPage(0, true);
      } else if (e.key === 'End') {
        e.preventDefault();
        goToPage(total - 1, true);
      } else if (e.key === 'f' || e.key === 'F') {
        e.preventDefault();
        var curIdx = 0;
        for (var i = 0; i < FIT_MODES.length; i++) {
          if (FIT_MODES[i].key === curFit) curIdx = i;
        }
        applyFit(FIT_MODES[(curIdx + 1) % FIT_MODES.length].key);
      } else if (e.key === 'Escape') {
        e.preventDefault();
        KRS.navigate('#/seri/' + book.series_id);
      }
    }

    var startX = null;
    var startY = null;
    stage.addEventListener('pointerdown', function (e) {
      if (curMode !== 'page' || (e.target !== stage && e.target !== pageImg)) {
        startX = null;
        startY = null;
        return;
      }
      startX = e.clientX;
      startY = e.clientY;
    });
    stage.addEventListener('pointercancel', function () {
      startX = null;
      startY = null;
    });
    stage.addEventListener('pointerup', function (e) {
      if (startX === null || startY === null) return;
      var sx = startX;
      var sy = startY;
      startX = null;
      startY = null;
      var dx = e.clientX - sx;
      var dy = e.clientY - sy;
      // sol/sağ yönü: soldan sağa okumada sola kaydırma ileri; sağdan sola okumada tersi
      var dirSign = rtl() ? -1 : 1;
      if (Math.abs(dx) > 50 && Math.abs(dx) > Math.abs(dy)) {
        step(dx < 0 ? dirSign : -dirSign);
        return;
      }
      if (Math.abs(dx) < 10 && Math.abs(dy) < 10) {
        var rect = stage.getBoundingClientRect();
        var relX = e.clientX - rect.left;
        if (relX < rect.width / 3) {
          step(-dirSign);
        } else if (relX > (rect.width * 2) / 3) {
          step(dirSign);
        } else {
          toggleBars();
        }
      }
    });

    function onUserMove() {
      showBars();
    }

    var releaseAwake = ui.keepAwake();
    window.addEventListener('keydown', onKeyDown);
    window.addEventListener('pointermove', onUserMove, { passive: true });
    stage.addEventListener('scroll', onUserMove, { passive: true });

    return function cleanup() {
      disposed = true;
      if (restoreTimer) clearTimeout(restoreTimer);
      if (hideTimer) clearTimeout(hideTimer);
      if (observer) observer.disconnect();
      if (scrollRaf) cancelAnimationFrame(scrollRaf);
      releaseAwake();
      if (fsBtn && fsBtn.cleanup) fsBtn.cleanup();
      window.removeEventListener('keydown', onKeyDown);
      window.removeEventListener('pointermove', onUserMove);
    };
  }

  KRS.readers.comic = { open: open };
})();
