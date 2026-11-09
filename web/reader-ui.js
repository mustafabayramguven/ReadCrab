// Okuyucu ortak bileşenleri (20.12): ayarlar modeli, alt sayfa parçaları, yer imi,
// tam ekran, ekranı açık tutma, sonraki cilt kartı, kalan süre, sekmeler ve arama.
(function () {
  'use strict';

  var KRS = (window.KRS = window.KRS || {});
  var calc = window.KRS_CALC;
  var STORAGE_KEY = 'krs.readerPrefs';
  var SEARCH_DELAY = 350;

  var SERIF = ', Georgia, "Noto Serif", serif';
  var SANS = ', system-ui, sans-serif';
  var FONTS = [
    { id: 'literata', label: 'Literata', family: '"Literata Variable"' + SERIF },
    { id: 'merriweather', label: 'Merriweather', family: '"Merriweather Variable"' + SERIF },
    { id: 'source-serif', label: 'Source Serif 4', family: '"Source Serif 4 Variable"' + SERIF },
    { id: 'eb-garamond', label: 'EB Garamond', family: '"EB Garamond Variable"' + SERIF },
    { id: 'inter', label: 'Inter', family: '"Inter Variable"' + SANS },
    { id: 'atkinson', label: 'Atkinson Hyperlegible', family: '"Atkinson Hyperlegible"' + SANS },
    { id: 'lexend', label: 'Lexend', family: '"Lexend Variable"' + SANS }
  ];

  function noop() {}

  function fontFamily(id) {
    for (var i = 0; i < FONTS.length; i++) {
      if (FONTS[i].id === id) return FONTS[i].family;
    }
    return FONTS[0].family;
  }

  // ---- Ayarlar modeli -------------------------------------------------------

  var cache = null;

  function writePrefs(p) {
    try {
      localStorage.setItem(STORAGE_KEY, JSON.stringify(p));
    } catch (_) {}
  }

  function readPrefs() {
    var raw = null;
    try {
      raw = localStorage.getItem(STORAGE_KEY);
    } catch (_) {}
    var legacy = null;
    if (raw === null) {
      // eski anahtarlar okunur ama silinmez
      try {
        var fs = parseInt(localStorage.getItem('krs.fs'), 10);
        legacy = {
          fs: Number.isFinite(fs) ? fs : undefined,
          readerTheme: localStorage.getItem('krs.readerTheme')
        };
      } catch (_) {}
    }
    var p = calc.normalizePrefs(raw, legacy);
    if (raw === null) writePrefs(p);
    return p;
  }

  KRS.readerPrefs = {
    get: function () {
      if (!cache) cache = readPrefs();
      return Object.assign({}, cache);
    },
    set: function (patch) {
      var next = calc.normalizePrefs(Object.assign({}, this.get(), patch), null);
      cache = next;
      writePrefs(next);
      return Object.assign({}, next);
    },
    reset: function () {
      cache = calc.normalizePrefs(null, null);
      writePrefs(cache);
      return Object.assign({}, cache);
    }
  };

  // Kayıtlı tema yoksa uygulama modu izlenir (koyuysa dark, saf siyah açıksa black).
  function resolveTheme(prefs) {
    if (prefs.theme) return prefs.theme;
    var api = window.KRS_THEME;
    var dark = document.documentElement.dataset.theme === 'dark';
    if (!dark) return 'light';
    return api && api.getBlack && api.getBlack() ? 'black' : 'dark';
  }

  // ---- Küçük arayüz parçaları -----------------------------------------------

  function formatNum(v) {
    return String(v).replace('.', ',');
  }

  // Alt sayfa kabuğu: tutamaç, sabit başlık (başlık + kapat), kaydırılan gövde.
  function sheetShell(o) {
    var el = KRS.el;
    var dlg = el('dialog', {
      className: 'sheet sheet-shell' + (o.className ? ' ' + o.className : ''),
      'aria-label': o.label || o.title
    });
    var closeBtn = el(
      'button',
      {
        type: 'button',
        className: 'icon-btn',
        'aria-label': 'Kapat',
        onClick: function () {
          dlg.close();
        }
      },
      [KRS.icon('close')]
    );
    var head = el('div', { className: 'sheet-head' }, [
      el('h2', { className: 'type-title-medium', text: o.title }),
      closeBtn
    ]);
    var body = el('div', { className: 'sheet-scroll' });
    dlg.appendChild(el('div', { className: 'sheet-handle' }));
    dlg.appendChild(head);
    dlg.appendChild(body);
    dlg.addEventListener('click', function (e) {
      if (e.target === dlg) dlg.close();
    });
    return { dialog: dlg, head: head, body: body };
  }

  function openSheet(dlg) {
    if (!dlg.open) dlg.showModal();
  }

  // Seçenek grubu (radiogroup). Düğmeler yerinde güncellenir, odak kaybolmaz.
  function segmented(label, options, current, onPick) {
    var el = KRS.el;
    var group = el('div', { className: 'segmented', role: 'radiogroup', 'aria-label': label });
    var buttons = [];
    function set(key) {
      for (var i = 0; i < buttons.length; i++) {
        buttons[i].btn.setAttribute('aria-checked', buttons[i].key === key ? 'true' : 'false');
      }
    }
    options.forEach(function (o, idx) {
      var btn = el(
        'button',
        {
          type: 'button',
          className: 'seg-btn',
          role: 'radio',
          'aria-checked': 'false',
          disabled: o.disabled ? true : false,
          onClick: function () {
            set(o.key);
            onPick(o.key);
          },
          onKeydown: function (e) {
            var step = e.key === 'ArrowRight' ? 1 : e.key === 'ArrowLeft' ? -1 : 0;
            if (!step) return;
            var j = idx;
            do {
              j = (j + step + buttons.length) % buttons.length;
            } while (buttons[j].btn.disabled && j !== idx);
            e.preventDefault();
            buttons[j].btn.focus();
            buttons[j].btn.click();
          }
        },
        [KRS.icon('check'), o.label]
      );
      buttons.push({ key: o.key, btn: btn });
      group.appendChild(btn);
    });
    set(current);
    return {
      el: group,
      set: set,
      setDisabled: function (key, off) {
        for (var i = 0; i < buttons.length; i++) {
          if (buttons[i].key === key || key === null) buttons[i].btn.disabled = off;
        }
      }
    };
  }

  // Kaydırıcı satırı: başlık solda, değer sağda, altında kaydırıcı.
  function sliderRow(o) {
    var el = KRS.el;
    var input = el('input', {
      type: 'range',
      className: 'slider',
      min: String(o.min),
      max: String(o.max),
      step: String(o.step),
      value: String(o.value),
      'aria-label': o.ariaLabel || o.label
    });
    var out = el('span', { className: 'rs-value type-label-large', 'aria-hidden': 'true' });
    function set(v) {
      var n = Number(v);
      input.value = String(n);
      out.textContent = o.format(n);
      input.setAttribute('aria-valuetext', o.format(n));
      input.style.setProperty('--val', ((n - o.min) / (o.max - o.min)) * 100 + '%');
    }
    input.addEventListener('input', function () {
      var n = Number(input.value);
      set(n);
      o.onInput(n);
    });
    set(o.value);
    var control = o.wrap ? o.wrap(input) : input;
    return {
      el: el('div', { className: 'rs-section' }, [
        el('div', { className: 'rs-title-row' }, [
          el('h3', { className: 'type-title-small', text: o.label }),
          out
        ]),
        control
      ]),
      set: set
    };
  }

  function section(title, node) {
    return KRS.el('div', { className: 'rs-section' }, [
      KRS.el('h3', { className: 'type-title-small', text: title }),
      node
    ]);
  }

  // Metin okuyucu ayarları. onChange(prefs) her değişiklikte çağrılır.
  function textSettingsSheet(onChange) {
    var el = KRS.el;
    var shell = sheetShell({
      title: 'Okuma ayarları',
      label: 'Okuyucu ayarları',
      className: 'reader-settings'
    });
    var prefs = KRS.readerPrefs.get();

    function apply(patch) {
      prefs = KRS.readerPrefs.set(patch);
      onChange(prefs);
    }

    var theme = segmented(
      'Okuma teması',
      [
        { key: 'light', label: 'Açık' },
        { key: 'dark', label: 'Koyu' },
        { key: 'sepia', label: 'Sepya' },
        { key: 'black', label: 'Siyah' }
      ],
      resolveTheme(prefs),
      function (k) {
        apply({ theme: k });
      }
    );

    var fontList = el('ul', { className: 'font-list', role: 'radiogroup', 'aria-label': 'Yazı tipi' });
    var fontBtns = {};
    FONTS.forEach(function (f) {
      var btn = el(
        'button',
        {
          type: 'button',
          className: 'font-option',
          role: 'radio',
          'aria-checked': 'false',
          text: f.label,
          onClick: function () {
            setFont(f.id);
            apply({ font: f.id });
          }
        }
      );
      btn.style.fontFamily = f.family;
      fontBtns[f.id] = btn;
      fontList.appendChild(el('li', { role: 'none' }, [btn]));
    });
    function setFont(id) {
      Object.keys(fontBtns).forEach(function (k) {
        fontBtns[k].setAttribute('aria-checked', k === id ? 'true' : 'false');
      });
    }

    var fs = sliderRow({
      label: 'Yazı boyutu',
      ariaLabel: 'Yazı boyutu',
      min: 14,
      max: 28,
      step: 2,
      value: prefs.fs,
      format: function (v) {
        return v + ' px';
      },
      onInput: function (v) {
        apply({ fs: v });
      },
      wrap: function (input) {
        function bump(d) {
          var v = Math.max(14, Math.min(28, prefs.fs + d));
          fs.set(v);
          apply({ fs: v });
        }
        return el('div', { className: 'rs-fs-row' }, [
          el('button', {
            type: 'button',
            className: 'icon-btn type-label-large',
            'aria-label': 'Yazıyı küçült (A-)',
            text: 'A-',
            onClick: function () {
              bump(-2);
            }
          }),
          input,
          el('button', {
            type: 'button',
            className: 'icon-btn type-label-large',
            'aria-label': 'Yazıyı büyüt (A+)',
            text: 'A+',
            onClick: function () {
              bump(2);
            }
          })
        ]);
      }
    });
    var lh = sliderRow({
      label: 'Satır aralığı',
      min: 1.2,
      max: 2.2,
      step: 0.1,
      value: prefs.lh,
      format: formatNum,
      onInput: function (v) {
        apply({ lh: v });
      }
    });
    var ws = sliderRow({
      label: 'Kelime arası',
      min: 0,
      max: 0.5,
      step: 0.05,
      value: prefs.ws,
      format: function (v) {
        return formatNum(v) + ' em';
      },
      onInput: function (v) {
        apply({ ws: v });
      }
    });
    var ls = sliderRow({
      label: 'Harf arası',
      min: 0,
      max: 0.1,
      step: 0.01,
      value: prefs.ls,
      format: function (v) {
        return formatNum(v) + ' em';
      },
      onInput: function (v) {
        apply({ ls: v });
      }
    });
    var mg = sliderRow({
      label: 'Kenar boşluğu',
      min: 0,
      max: 20,
      step: 1,
      value: prefs.mg,
      format: function (v) {
        return '%' + v;
      },
      onInput: function (v) {
        apply({ mg: v });
      }
    });
    var align = segmented(
      'Hizalama',
      [
        { key: 'left', label: 'Sol' },
        { key: 'justify', label: 'İki yana' }
      ],
      prefs.align,
      function (k) {
        apply({ align: k });
      }
    );
    var mode = segmented(
      'Okuma modu',
      [
        { key: 'scroll', label: 'Kaydırma' },
        { key: 'page', label: 'Sayfa' }
      ],
      prefs.mode,
      function (k) {
        apply({ mode: k });
      }
    );
    var resetBtn = el('button', {
      type: 'button',
      className: 'btn tonal rs-reset',
      text: 'Varsayılana dön',
      onClick: function () {
        prefs = KRS.readerPrefs.reset();
        sync();
        onChange(prefs);
      }
    });

    function sync() {
      theme.set(resolveTheme(prefs));
      setFont(prefs.font);
      fs.set(prefs.fs);
      lh.set(prefs.lh);
      ws.set(prefs.ws);
      ls.set(prefs.ls);
      mg.set(prefs.mg);
      align.set(prefs.align);
      mode.set(prefs.mode);
    }
    sync();

    [
      section('Görünüm', theme.el),
      section('Yazı tipi', fontList),
      fs.el,
      lh.el,
      ws.el,
      ls.el,
      mg.el,
      section('Hizalama', align.el),
      section('Okuma modu', mode.el),
      el('div', { className: 'rs-actions' }, [resetBtn])
    ].forEach(function (n) {
      shell.body.appendChild(n);
    });

    return { dialog: shell.dialog, sync: sync };
  }

  // ---- Tam ekran, ekranı açık tutma -----------------------------------------

  function fullscreenButton() {
    if (!document.fullscreenEnabled) return null;
    var btn = KRS.el(
      'button',
      {
        type: 'button',
        className: 'icon-btn',
        'aria-label': 'Tam ekran',
        onClick: function () {
          try {
            var p = document.fullscreenElement
              ? document.exitFullscreen()
              : document.documentElement.requestFullscreen();
            if (p && p.catch) p.catch(noop);
          } catch (_) {}
        }
      },
      [KRS.icon('fullscreen')]
    );
    function sync() {
      var on = Boolean(document.fullscreenElement);
      btn.setAttribute('aria-label', on ? 'Tam ekrandan çık' : 'Tam ekran');
      btn.replaceChild(KRS.icon(on ? 'fullscreen_exit' : 'fullscreen'), btn.firstChild);
    }
    document.addEventListener('fullscreenchange', sync);
    btn.cleanup = function () {
      document.removeEventListener('fullscreenchange', sync);
      if (document.fullscreenElement) {
        try {
          var p = document.exitFullscreen();
          if (p && p.catch) p.catch(noop);
        } catch (_) {}
      }
    };
    return btn;
  }

  function keepAwake() {
    var lock = null;
    var released = false;
    function acquire() {
      if (released || !window.isSecureContext || !navigator.wakeLock) return;
      if (document.visibilityState !== 'visible') return;
      try {
        navigator.wakeLock
          .request('screen')
          .then(function (l) {
            if (released) {
              l.release().catch(noop);
              return;
            }
            lock = l;
            l.addEventListener('release', function () {
              if (lock === l) lock = null;
            });
          })
          .catch(noop);
      } catch (_) {}
    }
    function onVisible() {
      if (document.visibilityState === 'visible' && !lock) acquire();
    }
    document.addEventListener('visibilitychange', onVisible);
    acquire();
    return function release() {
      released = true;
      document.removeEventListener('visibilitychange', onVisible);
      if (lock) {
        lock.release().catch(noop);
        lock = null;
      }
    };
  }

  // ---- Yer imleri -------------------------------------------------------------

  function bookmarkController(book, o) {
    var el = KRS.el;
    var items = [];
    var lists = [];
    var btn = el(
      'button',
      {
        type: 'button',
        className: 'icon-btn',
        'aria-label': 'Yer imi ekle',
        'aria-pressed': 'false',
        onClick: function () {
          toggle();
        }
      },
      [KRS.icon('bookmark')]
    );

    function current() {
      for (var i = 0; i < items.length; i++) {
        if (o.matches(items[i])) return items[i];
      }
      return null;
    }

    var shown = null;
    // Konum değişince yalnız düğme güncellenir.
    function sync() {
      var cur = current();
      var key = cur ? cur.id : 0;
      if (key === shown) return;
      shown = key;
      btn.setAttribute('aria-pressed', cur ? 'true' : 'false');
      btn.setAttribute('aria-label', cur ? 'Yer imini kaldır' : 'Yer imi ekle');
      btn.replaceChild(KRS.icon(cur ? 'bookmark-fill' : 'bookmark'), btn.firstChild);
    }

    // Liste değişince düğme ve listeler birlikte güncellenir.
    function refresh() {
      shown = null;
      sync();
      lists.forEach(function (l) {
        l.render();
      });
    }

    async function load() {
      try {
        var r = await KRS.api('GET', '/api/books/' + book.id + '/bookmarks');
        items = (r && r.items) || [];
      } catch (_) {
        items = [];
      }
      refresh();
    }

    async function remove(bm) {
      try {
        await KRS.api('DELETE', '/api/bookmarks/' + bm.id);
        items = items.filter(function (x) {
          return x.id !== bm.id;
        });
        refresh();
      } catch (e) {
        KRS.toast(e.message || 'Yer imi silinemedi.');
      }
    }

    async function toggle() {
      var cur = current();
      if (cur) {
        await remove(cur);
        return;
      }
      try {
        var created = await KRS.api('POST', '/api/books/' + book.id + '/bookmarks', {
          locator: o.getLocator(),
          label: o.getLabel()
        });
        items.unshift(created);
        refresh();
      } catch (e) {
        KRS.toast(e.message || 'Yer imi kaydedilemedi.');
      }
    }

    var dateFmt = new Intl.DateTimeFormat('tr-TR', { day: 'numeric', month: 'short', year: 'numeric' });

    function buildList(onGo) {
      var ul = el('ul', { className: 'list' });
      function render() {
        KRS.clearNode(ul);
        if (items.length === 0) {
          ul.appendChild(el('li', { className: 'rs-empty type-body-medium', text: 'Yer imi yok' }));
          return;
        }
        items.forEach(function (bm) {
          ul.appendChild(
            el('li', {}, [
              el('div', { className: 'list-item bm-row' }, [
                el(
                  'button',
                  {
                    type: 'button',
                    className: 'bm-go',
                    onClick: function () {
                      onGo(bm);
                    }
                  },
                  [
                    el('span', { className: 'type-body-large bm-label', text: bm.label }),
                    el('span', {
                      className: 'type-body-small bm-date',
                      text: dateFmt.format(new Date(bm.created_at * 1000))
                    })
                  ]
                ),
                el(
                  'button',
                  {
                    type: 'button',
                    className: 'icon-btn',
                    'aria-label': 'Yer imini sil: ' + bm.label,
                    onClick: function () {
                      remove(bm);
                    }
                  },
                  [KRS.icon('delete')]
                )
              ])
            ])
          );
        });
      }
      var holder = { el: ul, render: render };
      lists.push(holder);
      render();
      return ul;
    }

    load();
    return { toggleBtn: btn, buildList: buildList, refresh: refresh, sync: sync };
  }

  // ---- Sonraki cilt, kalan süre -----------------------------------------------

  function nextVolumeCard(book) {
    if (!book.next_book_id) return null;
    var card = KRS.el('div', { className: 'next-volume', role: 'status' }, [
      KRS.el('a', {
        className: 'btn filled',
        href: '#/oku/' + book.next_book_id,
        text: 'Sonraki cilt: ' + (book.next_book_title || '')
      })
    ]);
    card.hidden = true;
    card.setVisible = function (v) {
      card.hidden = !v;
    };
    return card;
  }

  function etaLabel(book) {
    var node = KRS.el('span', { id: 'eta-label', className: 'type-label-small eta-label' });
    node.hidden = true;
    return {
      el: node,
      update: function (overall) {
        var t = calc.formatEta(book.pace_seconds === undefined ? null : book.pace_seconds, overall);
        node.textContent = t;
        node.hidden = !t;
      }
    };
  }

  // ---- Sekmeler -----------------------------------------------------------------

  function tabs(prefix, defs, onSelect) {
    var el = KRS.el;
    var list = el('div', { className: 'segmented tab-list', role: 'tablist', 'aria-label': 'Bölümler' });
    var panels = {};
    var buttons = {};
    defs.forEach(function (d, idx) {
      var btn = el(
        'button',
        {
          type: 'button',
          className: 'seg-btn',
          role: 'tab',
          id: prefix + '-tab-' + d.key,
          'aria-selected': 'false',
          'aria-controls': prefix + '-panel-' + d.key,
          text: d.label,
          onClick: function () {
            select(d.key);
          },
          onKeydown: function (e) {
            var step = e.key === 'ArrowRight' ? 1 : e.key === 'ArrowLeft' ? -1 : 0;
            if (!step) return;
            var next = defs[(idx + step + defs.length) % defs.length].key;
            e.preventDefault();
            select(next);
            buttons[next].focus();
          }
        }
      );
      buttons[d.key] = btn;
      list.appendChild(btn);
      panels[d.key] = el('div', {
        className: 'tab-panel',
        role: 'tabpanel',
        id: prefix + '-panel-' + d.key,
        'aria-labelledby': prefix + '-tab-' + d.key
      });
      panels[d.key].hidden = true;
    });
    function select(key) {
      defs.forEach(function (d) {
        var on = d.key === key;
        buttons[d.key].setAttribute('aria-selected', on ? 'true' : 'false');
        buttons[d.key].tabIndex = on ? 0 : -1;
        panels[d.key].hidden = !on;
      });
      if (onSelect) onSelect(key);
    }
    return { list: list, panels: panels, select: select };
  }

  // ---- Kitapta arama ------------------------------------------------------------

  function searchPanel(book, onGo) {
    var el = KRS.el;
    var input = el('input', {
      type: 'search',
      className: 'rs-search',
      placeholder: 'Kitapta ara',
      'aria-label': 'Kitapta ara',
      autocomplete: 'off',
      enterkeyhint: 'search'
    });
    var status = el('p', { className: 'type-body-small search-status', 'aria-live': 'polite' });
    var results = el('ul', { className: 'list search-results' });
    var timer = null;
    var seq = 0;

    function setStatus(msg) {
      status.textContent = msg;
      status.hidden = !msg;
    }
    setStatus('');

    function render(res) {
      KRS.clearNode(results);
      var items = (res && res.results) || [];
      items.forEach(function (r) {
        results.appendChild(
          el('li', {}, [
            el(
              'button',
              {
                type: 'button',
                className: 'list-item search-hit',
                onClick: function () {
                  onGo(r.ch, r.p);
                }
              },
              [
                el('span', { className: 'search-text type-body-medium' }, [
                  document.createTextNode(r.before),
                  el('mark', { text: r.hit }),
                  document.createTextNode(r.after)
                ]),
                el('span', { className: 'type-label-small search-meta', text: 'Bölüm ' + (r.ch + 1) })
              ]
            )
          ])
        );
      });
      if (items.length === 0) {
        setStatus('Sonuç bulunamadı');
      } else {
        setStatus(res.truncated ? 'İlk 100 sonuç gösteriliyor' : '');
      }
    }

    async function run(q) {
      var mine = ++seq;
      setStatus('Aranıyor…');
      try {
        var res = await KRS.api(
          'GET',
          '/api/books/' + book.id + '/search?q=' + encodeURIComponent(q)
        );
        if (mine !== seq) return;
        render(res);
      } catch (e) {
        if (mine !== seq) return;
        KRS.clearNode(results);
        setStatus(
          e.status === 429
            ? 'Çok fazla arama yapıldı, birkaç saniye sonra yeniden deneyin.'
            : e.message || 'Arama yapılamadı.'
        );
      }
    }

    input.addEventListener('input', function () {
      if (timer) clearTimeout(timer);
      var q = input.value.trim();
      seq++;
      if (q.length < 2) {
        KRS.clearNode(results);
        setStatus(q.length === 0 ? '' : 'En az 2 karakter girin');
        return;
      }
      timer = setTimeout(function () {
        run(q);
      }, SEARCH_DELAY);
    });

    return {
      el: el('div', { className: 'section-stack' }, [input, status, results]),
      input: input,
      cleanup: function () {
        if (timer) clearTimeout(timer);
        seq++;
      }
    };
  }

  KRS.readerUi = {
    FONTS: FONTS,
    fontFamily: fontFamily,
    resolveTheme: resolveTheme,
    sheetShell: sheetShell,
    openSheet: openSheet,
    segmented: segmented,
    sliderRow: sliderRow,
    section: section,
    textSettingsSheet: textSettingsSheet,
    fullscreenButton: fullscreenButton,
    keepAwake: keepAwake,
    bookmarkController: bookmarkController,
    nextVolumeCard: nextVolumeCard,
    etaLabel: etaLabel,
    tabs: tabs,
    searchPanel: searchPanel
  };
})();
