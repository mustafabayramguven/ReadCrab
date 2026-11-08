(function () {
  'use strict';

  var KRS = (window.KRS = window.KRS || {});
  KRS.readers = KRS.readers || {};

  var FIT_MODES = [
    { key: 'width', label: 'Genişliğe sığdır' },
    { key: 'height', label: 'Yüksekliğe sığdır' },
    { key: 'page', label: 'Sayfaya sığdır' },
    { key: 'actual', label: 'Gerçek boyut' }
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

  async function open(book, mount, gen) {
    var myGen = gen !== undefined ? gen : KRS.routeGen;
    var el = KRS.el;
    var icon = KRS.icon;
    var clearNode = KRS.clearNode;
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

    var readerEl = el('div', { className: 'reader comic' });
    if (window.KRS_THEME) {
      var darkPal = window.KRS_THEME.generatePalette(window.KRS_THEME.getSeed(), true);
      var palKeys = Object.keys(darkPal);
      for (var pk = 0; pk < palKeys.length; pk++) {
        readerEl.style.setProperty('--md-sys-color-' + palKeys[pk], darkPal[palKeys[pk]]);
      }
      readerEl.style.background = darkPal['surface-container-lowest'];
      readerEl.style.color = darkPal['on-surface'];
    }

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
    var topBar = el('div', { className: 'reader-bar top' }, [
      backBtn,
      titleEl,
      settingsBtn
    ]);

    var pageImg = el('img', {
      className: 'reader-page-img',
      alt: book.title + ' sayfa'
    });
    var stage = el('div', { className: 'reader-stage fit-' + curFit }, [pageImg]);

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
    var prevBtn = el(
      'button',
      {
        type: 'button',
        className: 'icon-btn',
        'aria-label': 'Önceki sayfa',
        onClick: function () {
          goToPage(curPage - 1, true);
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
          goToPage(curPage + 1, true);
        }
      },
      [icon('chevron_right')]
    );
    var bottomBar = el('div', { className: 'reader-bar bottom' }, [
      el('div', { className: 'reader-progress-wrap' }, [progTrack, pageInd]),
      prevBtn,
      nextBtn
    ]);

    var sheet = el('dialog', { className: 'sheet', 'aria-label': 'Okuyucu ayarları' });
    var fitGroup = el('div', {
      className: 'segmented',
      role: 'radiogroup',
      'aria-label': 'Sığdırma modu'
    });
    sheet.appendChild(el('div', { className: 'sheet-handle' }));
    sheet.appendChild(
      el('div', { className: 'section-stack' }, [
        el('div', { className: 'setting-row' }, [
          el('h2', { className: 'type-title-medium', text: 'Ayarlar' }),
          el(
            'button',
            {
              type: 'button',
              className: 'icon-btn',
              'aria-label': 'Kapat',
              onClick: function () {
                sheet.close();
              }
            },
            [icon('close')]
          )
        ]),
        el('div', { className: 'section-stack' }, [
          el('span', { className: 'type-body-large', text: 'Sığdırma modu' }),
          fitGroup
        ])
      ])
    );
    sheet.addEventListener('click', function (e) {
      if (e.target === sheet) {
        sheet.close();
      }
    });

    function applyFit(mode) {
      curFit = mode;
      saveFit(mode);
      stage.className = 'reader-stage fit-' + curFit;
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
      if (!sheet.open) {
        sheet.showModal();
      }
    }

    readerEl.appendChild(topBar);
    readerEl.appendChild(stage);
    readerEl.appendChild(bottomBar);
    readerEl.appendChild(sheet);
    mount.appendChild(readerEl);

    var hideTimer = null;
    function scheduleHideBars() {
      if (hideTimer) clearTimeout(hideTimer);
      hideTimer = setTimeout(function () {
        if (sheet.open) return;
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

    var errBox = null;
    function clearErrBox() {
      if (errBox && errBox.parentNode) {
        errBox.parentNode.removeChild(errBox);
      }
      errBox = null;
      pageImg.style.display = '';
    }

    function goToPage(idx, saveProg) {
      var target = Math.max(0, Math.min(total - 1, idx));
      if (target === curPage && pageImg.getAttribute('src') && !errBox) return;
      clearErrBox();
      curPage = target;
      var pct = (curPage + 1) / total;
      var pct100 = Math.round(pct * 100);

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

      pageInd.textContent = curPage + 1 + ' / ' + total;
      progFill.style.width = pct100 + '%';
      progTrack.setAttribute('aria-valuenow', String(pct100));

      if (KRS.tracker) {
        KRS.tracker.addPage(curPage);
        if (saveProg) {
          KRS.tracker.scheduleProgress(book.id, String(curPage), pct);
        }
      }
      preloadNeighbors(curPage);
    }

    if (KRS.tracker) {
      KRS.tracker.start(book.id, (curPage + 1) / total);
    }
    goToPage(curPage, false);
    scheduleHideBars();

    function onKeyDown(e) {
      showBars();
      if (e.ctrlKey || e.metaKey || e.altKey) return;
      if (sheet.open) {
        if (e.key === 'Escape') {
          sheet.close();
        }
        return;
      }
      if (e.key === 'ArrowRight' || e.key === 'PageDown') {
        e.preventDefault();
        goToPage(curPage + 1, true);
      } else if (e.key === 'ArrowLeft' || e.key === 'PageUp') {
        e.preventDefault();
        goToPage(curPage - 1, true);
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
      if (e.target !== stage && e.target !== pageImg) {
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
      if (Math.abs(dx) > 50 && Math.abs(dx) > Math.abs(dy)) {
        if (dx < 0) {
          goToPage(curPage + 1, true);
        } else {
          goToPage(curPage - 1, true);
        }
        return;
      }
      if (Math.abs(dx) < 10 && Math.abs(dy) < 10) {
        var rect = stage.getBoundingClientRect();
        var relX = e.clientX - rect.left;
        if (relX < rect.width / 3) {
          goToPage(curPage - 1, true);
        } else if (relX > (rect.width * 2) / 3) {
          goToPage(curPage + 1, true);
        } else {
          toggleBars();
        }
      }
    });

    function onUserMove() {
      showBars();
    }

    window.addEventListener('keydown', onKeyDown);
    window.addEventListener('pointermove', onUserMove, { passive: true });
    stage.addEventListener('scroll', onUserMove, { passive: true });

    return function cleanup() {
      if (hideTimer) clearTimeout(hideTimer);
      window.removeEventListener('keydown', onKeyDown);
      window.removeEventListener('pointermove', onUserMove);
    };
  }

  KRS.readers.comic = { open: open };
})();
