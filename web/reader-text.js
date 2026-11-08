(function () {
  'use strict';

  var KRS = (window.KRS = window.KRS || {});
  KRS.readers = KRS.readers || {};

  var THEMES = [
    { key: 'light', label: 'Açık' },
    { key: 'dark', label: 'Koyu' },
    { key: 'sepia', label: 'Sepya' }
  ];

  function getSavedTheme() {
    try {
      var v = localStorage.getItem('krs.readerTheme');
      if (v === 'light' || v === 'dark' || v === 'sepia') return v;
    } catch (_) {}
    if (typeof document !== 'undefined' && document.documentElement) {
      return document.documentElement.dataset.theme === 'dark' ? 'dark' : 'light';
    }
    return 'light';
  }

  function saveTheme(v) {
    try {
      localStorage.setItem('krs.readerTheme', v);
    } catch (_) {}
  }

  function getSavedFs() {
    try {
      var n = parseInt(localStorage.getItem('krs.fs'), 10);
      if (Number.isFinite(n) && n >= 14 && n <= 28) {
        return n % 2 === 0 ? n : n - 1;
      }
    } catch (_) {}
    return 18;
  }

  function saveFs(n) {
    try {
      localStorage.setItem('krs.fs', String(n));
    } catch (_) {}
  }

  function clamp01(v) {
    var n = Number(v);
    if (!Number.isFinite(n)) return 0;
    return n < 0 ? 0 : n > 1 ? 1 : n;
  }

  async function open(book, mount, gen) {
    var myGen = gen !== undefined ? gen : KRS.routeGen;
    var el = KRS.el;
    var icon = KRS.icon;
    var clearNode = KRS.clearNode;
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

    var curChapter = 0;
    var curRatio = 0;
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

    var curTheme = getSavedTheme();
    var curFs = getSavedFs();
    var restoringScroll = false;

    var readerEl = el('div', { className: 'reader' });

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
          if (!settingsSheet.open) settingsSheet.showModal();
        }
      },
      [icon('tune')]
    );
    var topBar = el('div', { className: 'reader-bar top' }, [
      backBtn,
      titleEl,
      settingsBtn
    ]);

    var iframe = el('iframe', {
      className: 'reader-frame',
      referrerpolicy: 'no-referrer',
      title: book.title + ' içeriği'
    });
    var stage = el('div', { className: 'reader-stage' }, [iframe]);

    var tocBtn = el(
      'button',
      {
        type: 'button',
        className: 'icon-btn',
        'aria-label': 'İçindekiler',
        onClick: function () {
          showBars();
          renderTocList();
          if (!tocSheet.open) tocSheet.showModal();
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
    var pctLabel = el('span', { className: 'type-label-medium', text: '%0' });
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
      el('div', { className: 'reader-progress-wrap' }, [progTrack, pctLabel]),
      prevChBtn,
      nextChBtn
    ]);

    // İçindekiler alt sayfası
    var tocSheet = el('dialog', { className: 'sheet', 'aria-label': 'İçindekiler' });
    var tocList = el('ul', { className: 'list' });
    tocSheet.appendChild(el('div', { className: 'sheet-handle' }));
    tocSheet.appendChild(
      el('div', { className: 'section-stack' }, [
        el('div', { className: 'setting-row' }, [
          el('h2', { className: 'type-title-medium', text: 'İçindekiler' }),
          el(
            'button',
            {
              type: 'button',
              className: 'icon-btn',
              'aria-label': 'Kapat',
              onClick: function () {
                tocSheet.close();
              }
            },
            [icon('close')]
          )
        ]),
        tocList
      ])
    );
    tocSheet.addEventListener('click', function (e) {
      if (e.target === tocSheet) tocSheet.close();
    });

    function renderTocList() {
      clearNode(tocList);
      for (var i = 0; i < chapters.length; i++) {
        (function (ch) {
          var isCur = ch.index === curChapter;
          var btn = el(
            'button',
            {
              type: 'button',
              className: 'list-item',
              'aria-current': isCur ? 'true' : false,
              onClick: function () {
                tocSheet.close();
                loadChapter(ch.index, 0, true);
              }
            },
            [
              el('span', { className: 'list-title', text: ch.title }),
              isCur ? icon('check') : null
            ]
          );
          if (isCur && window.KRS_THEME) {
            var pal = window.KRS_THEME.generatePalette(
              window.KRS_THEME.getSeed(),
              document.documentElement.dataset.theme === 'dark'
            );
            btn.style.background = pal['secondary-container'];
            btn.style.color = pal['on-secondary-container'];
          }
          var li = el('li', {}, [btn]);
          tocList.appendChild(li);
        })(chapters[i]);
      }
    }

    // Ayarlar alt sayfası
    var settingsSheet = el('dialog', {
      className: 'sheet',
      'aria-label': 'Okuyucu ayarları'
    });
    var themeGroup = el('div', {
      className: 'segmented',
      role: 'radiogroup',
      'aria-label': 'Okuma teması'
    });
    var fsSlider = el('input', {
      type: 'range',
      className: 'slider',
      min: '14',
      max: '28',
      step: '2',
      value: String(curFs),
      'aria-label': 'Yazı boyutu',
      'aria-valuetext': curFs + ' px'
    });
    var fsMinusBtn = el(
      'button',
      {
        type: 'button',
        className: 'icon-btn type-label-large',
        'aria-label': 'Yazıyı küçült (A-)',
        text: 'A-',
        onClick: function () {
          setFontSize(curFs - 2);
        }
      }
    );
    var fsPlusBtn = el(
      'button',
      {
        type: 'button',
        className: 'icon-btn type-label-large',
        'aria-label': 'Yazıyı büyüt (A+)',
        text: 'A+',
        onClick: function () {
          setFontSize(curFs + 2);
        }
      }
    );

    fsSlider.addEventListener('input', function () {
      setFontSize(parseInt(fsSlider.value, 10));
    });

    settingsSheet.appendChild(el('div', { className: 'sheet-handle' }));
    settingsSheet.appendChild(
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
                settingsSheet.close();
              }
            },
            [icon('close')]
          )
        ]),
        el('div', { className: 'setting-row' }, [
          el('span', { className: 'type-body-large', text: 'Tema' }),
          themeGroup
        ]),
        el('div', { className: 'section-stack' }, [
          el('span', {
            className: 'type-body-large',
            text: 'Yazı boyutu'
          }),
          el('div', { className: 'setting-row slider-row' }, [
            fsMinusBtn,
            fsSlider,
            fsPlusBtn
          ])
        ])
      ])
    );
    settingsSheet.addEventListener('click', function (e) {
      if (e.target === settingsSheet) settingsSheet.close();
    });

    function updateSliderVisual() {
      var pct = ((curFs - 14) / (28 - 14)) * 100;
      fsSlider.value = String(curFs);
      fsSlider.setAttribute('aria-valuetext', curFs + ' px');
      fsSlider.style.setProperty('--val', pct + '%');
    }

    function buildThemeButtons() {
      clearNode(themeGroup);
      for (var i = 0; i < THEMES.length; i++) {
        (function (t) {
          var sel = t.key === curTheme;
          var ch = sel ? [icon('check'), t.label] : [t.label];
          themeGroup.appendChild(
            el(
              'button',
              {
                type: 'button',
                className: 'seg-btn',
                role: 'radio',
                'aria-checked': sel ? 'true' : 'false',
                onClick: function () {
                  curTheme = t.key;
                  saveTheme(curTheme);
                  buildThemeButtons();
                  applyFrameStyles();
                }
              },
              ch
            )
          );
        })(THEMES[i]);
      }
    }

    function setFontSize(val) {
      var clamped = Math.max(14, Math.min(28, val));
      if (clamped % 2 !== 0) clamped -= 1;
      curFs = clamped;
      saveFs(curFs);
      updateSliderVisual();
      applyFrameStyles();
    }

    function applyFrameStyles() {
      var seed = window.KRS_THEME ? window.KRS_THEME.getSeed() : '#6750A4';
      var rc = window.KRS_THEME
        ? window.KRS_THEME.readerColors(curTheme, seed)
        : { bg: '#FEF7FF', fg: '#1D1B20', link: '#6750A4' };
      readerEl.style.background = rc.bg;
      stage.style.background = rc.bg;
      iframe.style.background = rc.bg;
      var doc = iframe.contentDocument;
      if (doc && doc.documentElement) {
        var st = doc.documentElement.style;
        st.setProperty('--bg', rc.bg);
        st.setProperty('--fg', rc.fg);
        st.setProperty('--link', rc.link);
        st.setProperty('--fs', curFs + 'px');
        st.setProperty('--font', 'Georgia, "Noto Serif", "DejaVu Serif", serif');
      }
    }

    buildThemeButtons();
    updateSliderVisual();

    readerEl.appendChild(topBar);
    readerEl.appendChild(stage);
    readerEl.appendChild(bottomBar);
    readerEl.appendChild(tocSheet);
    readerEl.appendChild(settingsSheet);
    mount.appendChild(readerEl);

    var hideTimer = null;
    function scheduleHideBars() {
      if (hideTimer) clearTimeout(hideTimer);
      hideTimer = setTimeout(function () {
        if (tocSheet.open || settingsSheet.open) return;
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

    function updateProgressUi(ratio) {
      var overall = (curChapter + clamp01(ratio)) / totalChapters;
      var pct100 = Math.round(overall * 100);
      progFill.style.width = pct100 + '%';
      progTrack.setAttribute('aria-valuenow', String(pct100));
      pctLabel.textContent = '%' + pct100;
      return overall;
    }

    function bindFrameEvents() {
      var doc = iframe.contentDocument;
      var win = iframe.contentWindow;
      if (!doc || !win) return;

      doc.addEventListener('click', function (e) {
        showBars();
        if (KRS.tracker) KRS.tracker.touch();
        var a = e.target.closest ? e.target.closest('a') : null;
        if (a) {
          e.preventDefault();
          var href = a.getAttribute('href') || '';
          if (href.indexOf('http://') === 0 || href.indexOf('https://') === 0) {
            window.open(href, '_blank', 'noopener,noreferrer');
          }
        }
      });

      doc.addEventListener(
        'pointermove',
        function () {
          showBars();
          if (KRS.tracker) KRS.tracker.touch();
        },
        { passive: true }
      );

      doc.addEventListener('keydown', function (e) {
        handleKey(e);
      });

      win.addEventListener(
        'scroll',
        function () {
          showBars();
          if (restoringScroll) return;
          var se = doc.scrollingElement || doc.documentElement || doc.body;
          if (!se) return;
          var maxScroll = Math.max(0, se.scrollHeight - se.clientHeight);
          var ratio = maxScroll > 0 ? clamp01(se.scrollTop / maxScroll) : 0;
          curRatio = ratio;
          var overall = updateProgressUi(ratio);
          var screenIdx = Math.floor(
            se.scrollTop / Math.max(1, se.clientHeight)
          );
          if (KRS.tracker) {
            KRS.tracker.addPage(curChapter + ':' + screenIdx);
            KRS.tracker.scheduleProgress(
              book.id,
              JSON.stringify({
                ch: curChapter,
                p: Number(ratio.toFixed(6))
              }),
              overall
            );
          }
        },
        { passive: true }
      );
    }

    var errBox = null;
    function clearErrBox() {
      if (errBox && errBox.parentNode) {
        errBox.parentNode.removeChild(errBox);
      }
      errBox = null;
      iframe.style.display = '';
    }

    function updateNavButtons() {
      prevChBtn.disabled = curChapter <= 0;
      nextChBtn.disabled = curChapter >= totalChapters - 1;
    }

    function loadChapter(idx, targetRatio, saveProg) {
      if (idx < 0 || idx >= totalChapters) return;
      clearErrBox();
      curChapter = idx;
      curRatio = clamp01(targetRatio);
      updateNavButtons();
      applyFrameStyles();

      iframe.style.opacity = '0';
      iframe.onload = function () {
        if (myGen !== undefined && myGen !== KRS.routeGen) return;
        var doc = iframe.contentDocument;
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
        applyFrameStyles();
        bindFrameEvents();
        var se = doc && (doc.scrollingElement || doc.documentElement || doc.body);
        if (se) {
          restoringScroll = true;
          var maxScroll = Math.max(0, se.scrollHeight - se.clientHeight);
          se.scrollTop = Math.round(curRatio * maxScroll);
          restoringScroll = false;
          var screenIdx = Math.floor(
            se.scrollTop / Math.max(1, se.clientHeight)
          );
          if (KRS.tracker) {
            KRS.tracker.addPage(curChapter + ':' + screenIdx);
          }
        }
        var overall = updateProgressUi(curRatio);
        if (saveProg && KRS.tracker) {
          KRS.tracker.scheduleProgress(
            book.id,
            JSON.stringify({
              ch: curChapter,
              p: Number(curRatio.toFixed(6))
            }),
            overall
          );
        }
        iframe.style.opacity = '1';
      };
      iframe.src = '/api/books/' + book.id + '/chapters/' + curChapter;
    }

    function handleKey(e) {
      showBars();
      if (e.ctrlKey || e.metaKey || e.altKey) return;
      if (tocSheet.open) {
        if (e.key === 'Escape') tocSheet.close();
        return;
      }
      if (settingsSheet.open) {
        if (e.key === 'Escape') settingsSheet.close();
        return;
      }
      var doc = iframe.contentDocument;
      var se = doc && (doc.scrollingElement || doc.documentElement || doc.body);
      if (e.key === '[' || e.key === 'ArrowLeft') {
        e.preventDefault();
        loadChapter(curChapter - 1, 0, true);
      } else if (e.key === ']' || e.key === 'ArrowRight') {
        e.preventDefault();
        loadChapter(curChapter + 1, 0, true);
      } else if (e.key === 'ArrowDown') {
        e.preventDefault();
        if (se) {
          se.scrollTop += 60;
        }
      } else if (e.key === 'ArrowUp') {
        e.preventDefault();
        if (se) {
          se.scrollTop -= 60;
        }
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

    if (KRS.tracker) {
      KRS.tracker.start(book.id, (curChapter + curRatio) / totalChapters);
    }
    applyFrameStyles();
    loadChapter(curChapter, curRatio, false);
    scheduleHideBars();

    window.addEventListener('keydown', handleKey);
    window.addEventListener('pointermove', onUserMove, { passive: true });

    return function cleanup() {
      if (hideTimer) clearTimeout(hideTimer);
      window.removeEventListener('keydown', handleKey);
      window.removeEventListener('pointermove', onUserMove);
    };
  }

  KRS.readers.text = { open: open };
})();
