(function () {
  'use strict';

  var SVG_NS = 'http://www.w3.org/2000/svg';
  var KRS = (window.KRS = window.KRS || {});
  KRS.readers = KRS.readers || {};
  KRS.me = null;
  KRS.routeGen = 0;

  var activeCleanup = null;
  var scanPollTimer = null;
  var toastTimer = null;

  var shell = null;
  var navRail = null;
  var navBar = null;
  var topBar = null;
  var topBarBack = null;
  var topBarTitle = null;
  var topBarActions = null;
  var mainPage = null;
  var snackbar = null;

  function setCleanup(fn) {
    if (typeof activeCleanup === 'function') {
      try {
        activeCleanup();
      } catch (_) {}
    }
    activeCleanup = fn || null;
  }

  function todayLocal() {
    var d = new Date();
    var y = d.getFullYear();
    var m = String(d.getMonth() + 1).padStart(2, '0');
    var day = String(d.getDate()).padStart(2, '0');
    return y + '-' + m + '-' + day;
  }

  function icon(name) {
    var svg = document.createElementNS(SVG_NS, 'svg');
    svg.setAttribute('class', 'icon');
    svg.setAttribute('aria-hidden', 'true');
    var use = document.createElementNS(SVG_NS, 'use');
    use.setAttribute('href', '/static/icons.svg#' + name);
    svg.appendChild(use);
    return svg;
  }

  function el(tag, attrs, children) {
    var node = document.createElement(tag);
    if (attrs) {
      var keys = Object.keys(attrs);
      for (var i = 0; i < keys.length; i++) {
        var k = keys[i];
        var v = attrs[k];
        if (v === undefined || v === null || v === false) continue;
        if (k === 'className') {
          node.className = v;
        } else if (k === 'text') {
          node.textContent = String(v);
        } else if (k.slice(0, 2) === 'on' && typeof v === 'function') {
          node.addEventListener(k.slice(2).toLowerCase(), v);
        } else {
          node.setAttribute(k, v === true ? '' : String(v));
        }
      }
    }
    if (Array.isArray(children)) {
      for (var j = 0; j < children.length; j++) {
        var c = children[j];
        if (c === null || c === undefined || c === false) continue;
        if (typeof c === 'string' || typeof c === 'number') {
          node.appendChild(document.createTextNode(String(c)));
        } else {
          node.appendChild(c);
        }
      }
    }
    return node;
  }

  function clearNode(node) {
    while (node.firstChild) {
      node.removeChild(node.firstChild);
    }
  }

  function createCircularProgress(percent, large) {
    var r = 18;
    var c = 2 * Math.PI * r;
    var svg = document.createElementNS(SVG_NS, 'svg');
    svg.setAttribute('viewBox', '0 0 48 48');
    svg.setAttribute(
      'class',
      'progress-circular' +
        (large ? ' lg' : '') +
        (percent === null || percent === undefined ? ' indeterminate' : '')
    );
    svg.setAttribute('role', 'progressbar');
    svg.setAttribute('aria-label', 'İlerleme durumu');
    svg.setAttribute('aria-valuemin', '0');
    svg.setAttribute('aria-valuemax', '100');

    var track = document.createElementNS(SVG_NS, 'circle');
    track.setAttribute('class', 'track');
    track.setAttribute('cx', '24');
    track.setAttribute('cy', '24');
    track.setAttribute('r', String(r));

    var arc = document.createElementNS(SVG_NS, 'circle');
    arc.setAttribute('class', 'arc');
    arc.setAttribute('cx', '24');
    arc.setAttribute('cy', '24');
    arc.setAttribute('r', String(r));
    arc.setAttribute('stroke-dasharray', String(c.toFixed(2)));

    if (percent === null || percent === undefined) {
      arc.setAttribute('stroke-dashoffset', String((c * 0.7).toFixed(2)));
      svg.setAttribute('aria-valuenow', '0');
    } else {
      var p = Math.max(0, Math.min(1, percent));
      var offset = c * (1 - p);
      arc.setAttribute('stroke-dashoffset', String(offset.toFixed(2)));
      svg.setAttribute('aria-valuenow', String(Math.round(p * 100)));
    }

    svg.appendChild(track);
    svg.appendChild(arc);
    return svg;
  }

  function createLinearProgress(percent, label) {
    var pct = Math.max(0, Math.min(100, Math.round((percent || 0) * 100)));
    var bar = el('div', { className: 'progress-linear-bar' });
    bar.style.width = pct + '%';
    return el(
      'div',
      {
        className: 'progress-linear',
        role: 'progressbar',
        'aria-label': label || 'Okuma ilerlemesi',
        'aria-valuemin': '0',
        'aria-valuemax': '100',
        'aria-valuenow': String(pct)
      },
      [bar]
    );
  }

  function createField(opts) {
    var input = el('input', {
      id: opts.id,
      type: opts.type || 'text',
      name: opts.name || opts.id,
      placeholder: ' ',
      value: opts.value || '',
      required: Boolean(opts.required),
      autocomplete: opts.autocomplete || 'off'
    });
    if (opts.min !== undefined) input.setAttribute('min', String(opts.min));
    if (opts.max !== undefined) input.setAttribute('max', String(opts.max));
    var label = el('label', { for: opts.id, text: opts.label });
    var help = el('div', { className: 'field-help', text: opts.help || '' });
    var wrap = el('div', { className: 'field' }, [input, label, help]);
    return {
      wrap: wrap,
      input: input,
      setError: function (msg) {
        if (msg) {
          input.setAttribute('aria-invalid', 'true');
          help.textContent = msg;
        } else {
          input.removeAttribute('aria-invalid');
          help.textContent = opts.help || '';
        }
      }
    };
  }

  function toast(msg, opts) {
    if (!snackbar) return;
    if (toastTimer) {
      clearTimeout(toastTimer);
      toastTimer = null;
    }
    clearNode(snackbar);
    snackbar.appendChild(el('span', { text: String(msg || '') }));
    if (opts && opts.action && typeof opts.onAction === 'function') {
      var btn = el('button', {
        type: 'button',
        className: 'btn text',
        text: opts.action,
        onClick: function () {
          snackbar.classList.remove('visible');
          opts.onAction();
        }
      });
      snackbar.appendChild(btn);
    }
    snackbar.classList.add('visible');
    toastTimer = setTimeout(function () {
      snackbar.classList.remove('visible');
    }, 4000);
  }

  function confirmDialog(title, text, okLabel) {
    return new Promise(function (resolve) {
      var dlg = el('dialog', { className: 'md-dialog' });
      var resolved = false;
      function finish(val) {
        if (resolved) return;
        resolved = true;
        if (dlg.open) dlg.close();
        dlg.remove();
        resolve(val);
      }
      var titleEl = el('h2', { className: 'dialog-title', text: title });
      var bodyEl = el('div', { className: 'dialog-body' }, [
        el('p', { text: text })
      ]);
      var cancelBtn = el('button', {
        type: 'button',
        className: 'btn text',
        text: 'İptal',
        onClick: function () {
          finish(false);
        }
      });
      var okBtn = el('button', {
        type: 'button',
        className: 'btn filled',
        text: okLabel || 'Onayla',
        onClick: function () {
          finish(true);
        }
      });
      var actions = el('div', { className: 'dialog-actions' }, [cancelBtn, okBtn]);
      dlg.appendChild(titleEl);
      dlg.appendChild(bodyEl);
      dlg.appendChild(actions);
      dlg.addEventListener('close', function () {
        finish(false);
      });
      document.body.appendChild(dlg);
      dlg.showModal();
    });
  }

  async function api(method, url, body) {
    var m = String(method || 'GET').toUpperCase();
    var opts = {
      method: m,
      credentials: 'same-origin',
      headers: {}
    };
    if (m === 'POST' || m === 'PUT' || m === 'DELETE') {
      opts.headers['Content-Type'] = 'application/json';
      opts.body = JSON.stringify(body !== undefined ? body : {});
    }
    var res = await fetch(url, opts);
    if (res.status === 204) {
      return null;
    }
    var ct = res.headers.get('content-type') || '';
    var data = null;
    if (ct.indexOf('application/json') !== -1) {
      try {
        data = await res.json();
      } catch (_) {}
    }
    if (!res.ok) {
      if (
        res.status === 401 &&
        url !== '/api/login' &&
        url !== '/api/me/password' &&
        url !== '/api/me'
      ) {
        KRS.me = null;
        if (location.hash !== '#/giris' && location.hash !== '#/kurulum') {
          navigate('#/giris');
        }
      }
      var err = new Error((data && data.message) || 'İstek başarısız oldu.');
      err.status = res.status;
      err.code = data && data.error;
      throw err;
    }
    return data;
  }

  function navigate(hash) {
    if (location.hash !== hash) {
      location.hash = hash;
    }
  }

  function initRipple() {
    document.addEventListener('pointerdown', function (e) {
      if (document.documentElement.dataset.motion === 'reduced') return;
      if (
        window.matchMedia &&
        window.matchMedia('(prefers-reduced-motion: reduce)').matches
      ) {
        return;
      }
      var target = e.target.closest(
        '.btn, .icon-btn, .fab, .chip, .list-item, .nav-item, .card, .seg-btn'
      );
      if (!target || target.disabled || target.getAttribute('aria-busy') === 'true') {
        return;
      }
      var rect = target.getBoundingClientRect();
      var size = Math.max(rect.width, rect.height) * 2;
      var x = e.clientX - rect.left - size / 2;
      var y = e.clientY - rect.top - size / 2;
      var rip = el('span', { className: 'ripple' });
      rip.style.width = size + 'px';
      rip.style.height = size + 'px';
      rip.style.left = x + 'px';
      rip.style.top = y + 'px';
      rip.addEventListener('animationend', function () {
        rip.remove();
      });
      target.appendChild(rip);
    });
  }

  function buildShell() {
    shell = document.getElementById('app');
    if (!shell) {
      shell = el('div', { id: 'app', className: 'app-shell' });
      document.body.insertBefore(shell, document.body.firstChild);
    }
    clearNode(shell);

    navRail = el('nav', { className: 'nav-rail', 'aria-label': 'Kenar gezinti' });
    topBarBack = el('div');
    topBarTitle = el('h1', { className: 'top-app-bar-title', text: 'ReadCrab' });
    topBarActions = el('div', { className: 'chip-group' });
    topBar = el('header', { className: 'top-app-bar' }, [
      topBarBack,
      topBarTitle,
      topBarActions
    ]);
    mainPage = el('main', { className: 'page', id: 'main-content' });
    navBar = el('nav', { className: 'nav-bar', 'aria-label': 'Alt gezinti' });
    snackbar = el('div', {
      className: 'snackbar',
      role: 'status',
      'aria-live': 'polite'
    });

    shell.appendChild(navRail);
    shell.appendChild(topBar);
    shell.appendChild(mainPage);
    shell.appendChild(navBar);
    shell.appendChild(snackbar);

    window.addEventListener(
      'scroll',
      function () {
        if (!topBar) return;
        if (window.scrollY > 4) {
          topBar.classList.add('scrolled');
        } else {
          topBar.classList.remove('scrolled');
        }
      },
      { passive: true }
    );
  }

  function renderNav(activeHash) {
    if (!navRail || !navBar) return;
    clearNode(navRail);
    clearNode(navBar);

    var items = [
      { hash: '#/', label: 'Kütüphane', icon: 'menu_book' },
      { hash: '#/gecmis', label: 'Geçmiş', icon: 'history' },
      { hash: '#/istatistik', label: 'İstatistik', icon: 'bar_chart' },
      { hash: '#/hesap', label: 'Hesap', icon: 'person' }
    ];
    if (KRS.me && KRS.me.is_admin) {
      items.push({
        hash: '#/yonetim',
        label: 'Yönetim',
        icon: 'admin_panel_settings'
      });
    }

    function makeItem(it) {
      var isAct =
        activeHash === it.hash ||
        (it.hash === '#/' && activeHash.indexOf('#/seri/') === 0);
      var icName = isAct ? it.icon + '-fill' : it.icon;
      var pill = el('span', { className: 'nav-pill' }, [icon(icName)]);
      var lbl = el('span', { text: it.label });
      var a = el(
        'a',
        {
          href: it.hash,
          className: 'nav-item',
          'aria-current': isAct ? 'page' : false
        },
        [pill, lbl]
      );
      return a;
    }

    for (var i = 0; i < items.length; i++) {
      navRail.appendChild(makeItem(items[i]));
      navBar.appendChild(makeItem(items[i]));
    }
  }

  function setTopBar(opts) {
    clearNode(topBarBack);
    clearNode(topBarActions);
    topBarTitle.textContent = (opts && opts.title) || 'ReadCrab';
    if (opts && opts.backHash) {
      topBarBack.appendChild(
        el(
          'a',
          {
            href: opts.backHash,
            className: 'icon-btn',
            'aria-label': 'Geri'
          },
          [icon('arrow_back')]
        )
      );
    }
    if (KRS.me) {
      var userBtn = el(
        'button',
        {
          type: 'button',
          className: 'btn text',
          text: KRS.me.username,
          onClick: function () {
            navigate('#/hesap');
          }
        },
        [icon('person')]
      );
      topBarActions.appendChild(userBtn);
      var outBtn = el(
        'button',
        {
          type: 'button',
          className: 'icon-btn',
          'aria-label': 'Çıkış yap',
          onClick: async function () {
            try {
              await api('POST', '/api/logout', {});
            } catch (_) {}
            KRS.me = null;
            navigate('#/giris');
          }
        },
        [icon('logout')]
      );
      topBarActions.appendChild(outBtn);
    }
  }

  function makeCoverNode(bookId, titleText) {
    var firstChar = (String(titleText || '?').trim().charAt(0) || '?').toUpperCase();
    var fallback = el('div', { className: 'cover-fallback', text: firstChar });
    if (!bookId) {
      return fallback;
    }
    var img = el('img', {
      loading: 'lazy',
      alt: '',
      src: '/api/books/' + bookId + '/cover'
    });
    img.addEventListener('error', function () {
      if (img.parentNode) {
        img.parentNode.replaceChild(fallback, img);
      }
    });
    return img;
  }

  // Kurulum sayfası (#/kurulum)
  async function renderSetup() {
    shell.classList.add('auth-only');
    clearNode(mainPage);

    var uField = createField({
      id: 'setup-username',
      label: 'Kullanıcı adı',
      required: true,
      autocomplete: 'username'
    });
    var pField = createField({
      id: 'setup-password',
      label: 'Parola',
      type: 'password',
      required: true,
      autocomplete: 'new-password'
    });
    var submitBtn = el('button', {
      type: 'submit',
      className: 'btn filled',
      text: 'Kurulumu tamamla'
    });

    var form = el(
      'form',
      {
        className: 'section-stack',
        onSubmit: async function (e) {
          e.preventDefault();
          uField.setError('');
          pField.setError('');
          var username = uField.input.value.trim();
          var password = pField.input.value;
          if (username.length < 3) {
            uField.setError('En az 3 karakter girin.');
            return;
          }
          if (password.length < 8) {
            pField.setError('En az 8 karakter girin.');
            return;
          }
          try {
            KRS.me = await api('POST', '/api/setup', {
              username: username,
              password: password
            });
            navigate('#/');
          } catch (err) {
            toast(err.message);
          }
        }
      },
      [uField.wrap, pField.wrap, submitBtn]
    );

    var card = el('div', { className: 'auth-card' }, [
      el('h1', { className: 'type-headline-small', text: 'ReadCrab' }),
      el('p', {
        className: 'type-body-medium',
        text: 'İlk yönetici hesabını oluşturun.'
      }),
      form
    ]);
    mainPage.appendChild(el('div', { className: 'auth-wrap' }, [card]));
  }

  // Giriş sayfası (#/giris)
  async function renderLogin(gen) {
    var myGen = gen !== undefined ? gen : KRS.routeGen;
    try {
      var st = await api('GET', '/api/setup-status');
      if (myGen !== KRS.routeGen) return;
      if (st && st.needs_setup) {
        navigate('#/kurulum');
        return;
      }
    } catch (_) {
      if (myGen !== KRS.routeGen) return;
    }

    shell.classList.add('auth-only');
    clearNode(mainPage);

    var uField = createField({
      id: 'login-username',
      label: 'Kullanıcı adı',
      required: true,
      autocomplete: 'username'
    });
    var pField = createField({
      id: 'login-password',
      label: 'Parola',
      type: 'password',
      required: true,
      autocomplete: 'current-password'
    });
    var submitBtn = el('button', {
      type: 'submit',
      className: 'btn filled',
      text: 'Giriş yap'
    });

    var form = el(
      'form',
      {
        className: 'section-stack',
        onSubmit: async function (e) {
          e.preventDefault();
          try {
            KRS.me = await api('POST', '/api/login', {
              username: uField.input.value.trim(),
              password: pField.input.value
            });
            navigate('#/');
          } catch (err) {
            toast(err.message);
          }
        }
      },
      [uField.wrap, pField.wrap, submitBtn]
    );

    var card = el('div', { className: 'auth-card' }, [
      el('h1', { className: 'type-headline-small', text: 'ReadCrab' }),
      el('p', { className: 'type-body-medium', text: 'Oturum açın.' }),
      form
    ]);
    mainPage.appendChild(el('div', { className: 'auth-wrap' }, [card]));
  }

  // Kütüphane (#/)
  function renderLibrary(gen) {
    var myGen = gen !== undefined ? gen : KRS.routeGen;
    var active = true;
    setTopBar({ title: 'ReadCrab' });
    clearNode(mainPage);

    var state = {
      q: '',
      sort: 'name',
      status: 'all'
    };
    var debounceId = null;
    var limit = 60;
    var offset = 0;
    var total = 0;
    var seq = 0;
    var loading = false;
    var grid = null;
    var moreWrap = el('div', { className: 'setting-row' });
    var moreBtn = null;
    var observer = null;

    function disconnectObserver() {
      if (observer) {
        observer.disconnect();
        observer = null;
      }
    }

    var searchInput = el('input', {
      type: 'search',
      'aria-label': 'Kütüphanede ara',
      placeholder: 'Seri veya kitap ara...'
    });
    var clearBtn = el(
      'button',
      {
        type: 'button',
        className: 'icon-btn',
        'aria-label': 'Aramayı temizle',
        onClick: function () {
          searchInput.value = '';
          state.q = '';
          updateClearBtn();
          loadSeries();
        }
      },
      [icon('close')]
    );

    function updateClearBtn() {
      clearBtn.style.display = searchInput.value ? 'inline-flex' : 'none';
    }
    updateClearBtn();

    searchInput.addEventListener('input', function () {
      updateClearBtn();
      if (debounceId) clearTimeout(debounceId);
      debounceId = setTimeout(function () {
        state.q = searchInput.value.trim();
        loadSeries();
      }, 250);
    });

    var searchBar = el('div', { className: 'search-bar' }, [
      icon('search'),
      searchInput,
      clearBtn
    ]);

    var sortGroup = el('div', {
      className: 'chip-group',
      role: 'radiogroup',
      'aria-label': 'Sıralama'
    });
    var statusGroup = el('div', {
      className: 'chip-group',
      role: 'radiogroup',
      'aria-label': 'Durum'
    });

    var sorts = [
      { key: 'name', label: 'Ad' },
      { key: 'added', label: 'Eklenme' },
      { key: 'recent', label: 'Son okunan' }
    ];
    var statuses = [
      { key: 'all', label: 'Tümü' },
      { key: 'reading', label: 'Okunuyor' },
      { key: 'finished', label: 'Bitti' },
      { key: 'unread', label: 'Okunmadı' }
    ];

    function buildChips() {
      clearNode(sortGroup);
      for (var i = 0; i < sorts.length; i++) {
        (function (s) {
          var sel = state.sort === s.key;
          var children = sel ? [icon('check'), s.label] : [s.label];
          sortGroup.appendChild(
            el(
              'button',
              {
                type: 'button',
                className: 'chip',
                role: 'radio',
                'aria-checked': sel ? 'true' : 'false',
                onClick: function () {
                  if (state.sort === s.key) return;
                  state.sort = s.key;
                  buildChips();
                  loadSeries();
                }
              },
              children
            )
          );
        })(sorts[i]);
      }

      clearNode(statusGroup);
      for (var j = 0; j < statuses.length; j++) {
        (function (st) {
          var sel = state.status === st.key;
          var children = sel ? [icon('check'), st.label] : [st.label];
          statusGroup.appendChild(
            el(
              'button',
              {
                type: 'button',
                className: 'chip',
                role: 'radio',
                'aria-checked': sel ? 'true' : 'false',
                onClick: function () {
                  if (state.status === st.key) return;
                  state.status = st.key;
                  buildChips();
                  loadSeries();
                }
              },
              children
            )
          );
        })(statuses[j]);
      }
    }
    buildChips();

    var controls = el('div', { className: 'library-controls' }, [
      searchBar,
      el('div', { className: 'chip-row' }, [sortGroup, statusGroup])
    ]);
    var contentArea = el('div');
    mainPage.appendChild(controls);
    mainPage.appendChild(contentArea);

    var fab = null;
    if (KRS.me && KRS.me.is_admin) {
      fab = el(
        'button',
        {
          type: 'button',
          className: 'fab',
          'aria-label': 'Kütüphaneyi tara',
          title: 'Kütüphaneyi tara',
          onClick: function () {
            triggerScan();
          }
        },
        [icon('sync')]
      );
      mainPage.appendChild(fab);
    }

    function updateFabStatus(st) {
      if (!fab) return;
      clearNode(fab);
      if (st && st.running) {
        fab.setAttribute('aria-busy', 'true');
        fab.disabled = true;
        var ratio = st.total > 0 ? st.done / st.total : null;
        fab.appendChild(createCircularProgress(ratio, false));
      } else {
        fab.removeAttribute('aria-busy');
        fab.disabled = false;
        fab.appendChild(icon('sync'));
      }
    }

    async function triggerScan() {
      try {
        await api('POST', '/api/admin/scan', {});
      } catch (err) {
        if (err.status !== 409) {
          toast(err.message);
          return;
        }
      }
      pollScan(true);
    }

    function pollScan(notifyWhenDone) {
      if (scanPollTimer) {
        clearTimeout(scanPollTimer);
        scanPollTimer = null;
      }
      if (!active || myGen !== KRS.routeGen) return;
      api('GET', '/api/scan-status')
        .then(function (st) {
          if (!active || myGen !== KRS.routeGen) return;
          updateFabStatus(st);
          if (st && st.running) {
            scanPollTimer = setTimeout(function () {
              pollScan(true);
            }, 400);
          } else if (notifyWhenDone) {
            toast('Tarama tamamlandı');
            loadSeries(true);
          }
        })
        .catch(function () {});
    }

    function appendSeriesCards(items) {
      for (var i = 0; i < items.length; i++) {
        var it = items[i];
        var coverChildren = [
          makeCoverNode(it.cover_book_id, it.name),
          createLinearProgress(it.percent || 0, it.name + ' okuma ilerlemesi')
        ];
        if (it.book_count > 0 && it.read_count === it.book_count) {
          coverChildren.push(el('span', { className: 'badge', text: 'Bitti' }));
        } else if (
          it.reading_count > 0 ||
          (it.read_count > 0 && it.read_count < it.book_count)
        ) {
          coverChildren.push(el('span', { className: 'badge', text: 'Okunuyor' }));
        }
        var card = el(
          'a',
          {
            href: '#/seri/' + it.id,
            className: 'card',
            'data-testid': 'series-card'
          },
          [
            el('div', { className: 'cover' }, coverChildren),
            el('div', { className: 'card-body' }, [
              el('h2', { className: 'card-title', text: it.name }),
              el('p', { className: 'card-meta', text: it.book_count + ' cilt' })
            ])
          ]
        );
        grid.appendChild(card);
      }
    }

    async function loadSeries(reset) {
      var isReset = reset !== false;
      if (!isReset) {
        if (loading || offset >= total) return;
        if (moreBtn) moreBtn.disabled = true;
      } else {
        disconnectObserver();
        offset = 0;
        total = 0;
        grid = null;
        moreBtn = null;
        clearNode(moreWrap);
        clearNode(contentArea);
        var skelGrid = el('div', { className: 'grid' });
        for (var s = 0; s < 6; s++) {
          skelGrid.appendChild(el('div', { className: 'skeleton skeleton-card' }));
        }
        contentArea.appendChild(skelGrid);
      }

      loading = true;
      var mySeq = ++seq;
      var reqOffset = isReset ? 0 : offset;

      var qs =
        '?q=' +
        encodeURIComponent(state.q) +
        '&sort=' +
        encodeURIComponent(state.sort) +
        '&status=' +
        encodeURIComponent(state.status) +
        '&limit=' +
        limit +
        '&offset=' +
        reqOffset;

      try {
        var res = await api('GET', '/api/series' + qs);
        if (!active || myGen !== KRS.routeGen || mySeq !== seq) return;
        loading = false;
        var items = (res && res.items) || [];
        total = Number((res && res.total) || 0);

        if (isReset) {
          clearNode(contentArea);
          if (items.length === 0) {
            if (state.q || state.status !== 'all') {
              contentArea.appendChild(
                el('div', { className: 'empty' }, [
                  icon('search'),
                  el('h2', { text: 'Sonuç bulunamadı' }),
                  el('p', { text: 'Başka bir arama deneyin.' })
                ])
              );
            } else {
              var emptyChildren = [
                icon('menu_book'),
                el('h2', { text: 'Kütüphane boş' }),
                el('p', {
                  text: '/books klasörüne seri klasörleri ekleyin ve ardından tarayın.'
                })
              ];
              if (KRS.me && KRS.me.is_admin) {
                emptyChildren.push(
                  el(
                    'button',
                    {
                      type: 'button',
                      className: 'btn tonal',
                      text: 'Kütüphaneyi tara',
                      onClick: function () {
                        triggerScan();
                      }
                    }
                  )
                );
              }
              contentArea.appendChild(el('div', { className: 'empty' }, emptyChildren));
            }
            return;
          }
          grid = el('div', { className: 'grid' });
          contentArea.appendChild(grid);
          contentArea.appendChild(moreWrap);
        }

        appendSeriesCards(items);
        offset = reqOffset + items.length;
        disconnectObserver();
        clearNode(moreWrap);
        moreBtn = null;

        if (offset < total) {
          moreBtn = el('button', {
            type: 'button',
            className: 'btn tonal',
            text: 'Daha fazla',
            onClick: function () {
              loadSeries(false);
            }
          });
          moreWrap.appendChild(moreBtn);
          if (typeof IntersectionObserver !== 'undefined') {
            observer = new IntersectionObserver(function (entries) {
              for (var eIdx = 0; eIdx < entries.length; eIdx++) {
                if (entries[eIdx].isIntersecting) {
                  loadSeries(false);
                  break;
                }
              }
            });
            observer.observe(moreBtn);
          }
        }
      } catch (err) {
        if (!active || myGen !== KRS.routeGen || mySeq !== seq) return;
        loading = false;
        if (!isReset) {
          if (moreBtn) moreBtn.disabled = false;
          toast(err.message);
          return;
        }
        clearNode(contentArea);
        contentArea.appendChild(
          el('div', { className: 'empty error' }, [
            icon('error'),
            el('h2', { text: 'Yüklenemedi' }),
            el('p', { text: err.message }),
            el('button', {
              type: 'button',
              className: 'btn tonal',
              text: 'Yeniden dene',
              onClick: function () {
                loadSeries(true);
              }
            })
          ])
        );
      }
    }

    loadSeries(true);
    pollScan(false);

    setCleanup(function () {
      active = false;
      if (debounceId) clearTimeout(debounceId);
      if (scanPollTimer) {
        clearTimeout(scanPollTimer);
        scanPollTimer = null;
      }
      disconnectObserver();
    });
  }

  function statusLabel(st) {
    if (st === 'reading') return 'Okunuyor';
    if (st === 'finished') return 'Bitti';
    if (st === 'dropped') return 'Bırakıldı';
    return 'Okunmadı';
  }

  function isComicFormat(fmt) {
    var f = String(fmt || '').toLowerCase();
    return f === 'cbz' || f === 'zip' || f === 'rar' || f === 'cbr';
  }

  function openBookStatusDialog(book, onSaved) {
    var dlg = el('dialog', { className: 'md-dialog' });
    var current = book.status || 'unread';
    var options = [
      { key: 'unread', label: 'Okunmadı' },
      { key: 'reading', label: 'Okunuyor' },
      { key: 'finished', label: 'Bitti' },
      { key: 'dropped', label: 'Bırakıldı' }
    ];
    var body = el('div', { className: 'dialog-body' });
    for (var i = 0; i < options.length; i++) {
      (function (opt) {
        var r = el('input', {
          type: 'radio',
          name: 'book-status',
          value: opt.key,
          checked: current === opt.key
        });
        r.addEventListener('change', function () {
          if (r.checked) current = opt.key;
        });
        body.appendChild(
          el('label', { className: 'radio-option' }, [
            r,
            el('span', { text: opt.label })
          ])
        );
      })(options[i]);
    }

    var cancelBtn = el('button', {
      type: 'button',
      className: 'btn text',
      text: 'İptal',
      onClick: function () {
        dlg.close();
        dlg.remove();
      }
    });
    var saveBtn = el('button', {
      type: 'button',
      className: 'btn filled',
      text: 'Kaydet',
      onClick: async function () {
        try {
          await api('PUT', '/api/books/' + book.id + '/status', {
            status: current,
            local_date: todayLocal()
          });
          dlg.close();
          dlg.remove();
          toast('Durum güncellendi');
          if (onSaved) onSaved();
        } catch (err) {
          toast(err.message);
        }
      }
    });

    dlg.appendChild(el('h2', { className: 'dialog-title', text: 'Okuma durumu' }));
    dlg.appendChild(body);
    dlg.appendChild(el('div', { className: 'dialog-actions' }, [cancelBtn, saveBtn]));
    dlg.addEventListener('close', function () {
      dlg.remove();
    });
    document.body.appendChild(dlg);
    dlg.showModal();
  }

  // Seri sayfası (#/seri/{id})
  async function renderSeries(seriesId, gen) {
    var myGen = gen !== undefined ? gen : KRS.routeGen;
    setTopBar({ title: 'Seri', backHash: '#/' });
    clearNode(mainPage);

    var skelList = el('div', { className: 'list' });
    for (var s = 0; s < 4; s++) {
      skelList.appendChild(el('div', { className: 'skeleton skeleton-row' }));
    }
    mainPage.appendChild(skelList);

    try {
      var data = await api('GET', '/api/series/' + encodeURIComponent(seriesId));
      if (myGen !== KRS.routeGen) return;
      setTopBar({ title: data.name, backHash: '#/' });
      clearNode(mainPage);

      var list = el('ul', { className: 'list' });
      var books = data.books || [];
      for (var i = 0; i < books.length; i++) {
        (function (b) {
          var hasErr = Boolean(b.error);
          var unit = isComicFormat(b.format) ? 'sayfa' : 'bölüm';
          var metaText = b.format.toUpperCase() + ' · ' + b.page_count + ' ' + unit;
          if (hasErr) {
            metaText +=
              ' · ' + (b.error === 'drm' ? 'DRM korumalı' : 'Bozuk dosya');
          }

          var thumb = el('div', { className: 'list-thumb' }, [
            makeCoverNode(b.id, b.title)
          ]);
          var content = el('div', { className: 'list-content' }, [
            el('h2', { className: 'list-title', text: b.title }),
            el('p', { className: 'list-sub', text: metaText }),
            createLinearProgress(b.percent || 0, b.title + ' ilerlemesi')
          ]);

          var mainLink = hasErr
            ? el('div', { className: 'list-item-main', 'aria-disabled': 'true' }, [
                thumb,
                content
              ])
            : el(
                'a',
                { href: '#/oku/' + b.id, className: 'list-item-main' },
                [thumb, content]
              );

          var stBadge = el('span', {
            className: 'badge',
            text: statusLabel(b.status)
          });
          var moreBtn = el(
            'button',
            {
              type: 'button',
              className: 'icon-btn',
              'aria-label': 'Durumu değiştir',
              onClick: function (e) {
                e.stopPropagation();
                openBookStatusDialog(b, function () {
                  renderSeries(seriesId, myGen);
                });
              }
            },
            [icon('more_vert')]
          );

          var li = el(
            'li',
            { className: 'list-item' + (hasErr ? ' is-error' : '') },
            [mainLink, stBadge, moreBtn]
          );
          list.appendChild(li);
        })(books[i]);
      }
      mainPage.appendChild(list);
    } catch (err) {
      if (myGen !== KRS.routeGen) return;
      clearNode(mainPage);
      mainPage.appendChild(
        el('div', { className: 'empty error' }, [
          icon('error'),
          el('h2', { text: 'Yüklenemedi' }),
          el('p', { text: err.message }),
          el('button', {
            type: 'button',
            className: 'btn tonal',
            text: 'Yeniden dene',
            onClick: function () {
              renderSeries(seriesId, myGen);
            }
          })
        ])
      );
    }
  }

  // Okuyucu (#/oku/{bookId})
  async function renderReader(bookId, gen) {
    var myGen = gen !== undefined ? gen : KRS.routeGen;
    document.body.classList.add('reading');
    clearNode(mainPage);

    try {
      var book = await api('GET', '/api/books/' + encodeURIComponent(bookId));
      if (myGen !== KRS.routeGen) {
        if ((location.hash || '').indexOf('#/oku/') !== 0) {
          document.body.classList.remove('reading');
        }
        return;
      }
      if (book.error) {
        document.body.classList.remove('reading');
        setTopBar({ title: book.title, backHash: '#/seri/' + book.series_id });
        mainPage.appendChild(
          el('div', { className: 'empty error' }, [
            icon('error'),
            el('h2', { text: 'Kitap açılamıyor' }),
            el('p', {
              text:
                book.error === 'drm'
                  ? 'Bu kitap DRM korumalı olduğu için açılamıyor.'
                  : 'Bu dosya bozuk veya okunamıyor.'
            }),
            el('a', {
              href: '#/seri/' + book.series_id,
              className: 'btn tonal',
              text: 'Seriye dön'
            })
          ])
        );
        return;
      }

      if (isComicFormat(book.format)) {
        if (KRS.readers.comic && typeof KRS.readers.comic.open === 'function') {
          var cleanupComic = await KRS.readers.comic.open(book, mainPage, myGen);
          if (myGen !== KRS.routeGen) {
            if (typeof cleanupComic === 'function') cleanupComic();
            if ((location.hash || '').indexOf('#/oku/') !== 0) {
              document.body.classList.remove('reading');
            }
            return;
          }
          setCleanup(cleanupComic);
        }
      } else {
        if (KRS.readers.text && typeof KRS.readers.text.open === 'function') {
          var cleanupText = await KRS.readers.text.open(book, mainPage, myGen);
          if (myGen !== KRS.routeGen) {
            if (typeof cleanupText === 'function') cleanupText();
            if ((location.hash || '').indexOf('#/oku/') !== 0) {
              document.body.classList.remove('reading');
            }
            return;
          }
          setCleanup(cleanupText);
        }
      }
    } catch (err) {
      if (myGen !== KRS.routeGen) {
        if ((location.hash || '').indexOf('#/oku/') !== 0) {
          document.body.classList.remove('reading');
        }
        return;
      }
      document.body.classList.remove('reading');
      setTopBar({ title: 'Okuyucu', backHash: '#/' });
      mainPage.appendChild(
        el('div', { className: 'empty error' }, [
          icon('error'),
          el('h2', { text: 'Yüklenemedi' }),
          el('p', { text: err.message }),
          el('a', { href: '#/', className: 'btn tonal', text: 'Kütüphaneye dön' })
        ])
      );
    }
  }

  // Hesap sayfası (#/hesap)
  function renderAccount() {
    setTopBar({ title: 'Hesap' });
    clearNode(mainPage);

    var themeApi = window.KRS_THEME;
    var modeGroup = el('div', {
      className: 'segmented',
      role: 'radiogroup',
      'aria-label': 'Tema modu'
    });
    var modes = [
      { key: 'auto', label: 'Otomatik' },
      { key: 'light', label: 'Açık' },
      { key: 'dark', label: 'Koyu' }
    ];

    function buildModes() {
      clearNode(modeGroup);
      var cur = themeApi ? themeApi.getMode() : 'auto';
      for (var i = 0; i < modes.length; i++) {
        (function (m) {
          var sel = cur === m.key;
          var ch = sel ? [icon('check'), m.label] : [m.label];
          modeGroup.appendChild(
            el(
              'button',
              {
                type: 'button',
                className: 'seg-btn',
                role: 'radio',
                'aria-checked': sel ? 'true' : 'false',
                onClick: function () {
                  if (themeApi) themeApi.setMode(m.key);
                  buildModes();
                }
              },
              ch
            )
          );
        })(modes[i]);
      }
    }
    buildModes();

    var swatchGroup = el('div', {
      className: 'swatch-group',
      role: 'radiogroup',
      'aria-label': 'Kaynak renk'
    });
    var customColor = el('input', {
      type: 'color',
      className: 'color-picker-input',
      'aria-label': 'Özel renk',
      value: themeApi ? themeApi.getSeed() : '#6750A4'
    });

    function buildSwatches() {
      clearNode(swatchGroup);
      var curSeed = (themeApi ? themeApi.getSeed() : '#6750A4').toUpperCase();
      var seeds = (themeApi && themeApi.SEEDS) || [];
      for (var i = 0; i < seeds.length; i++) {
        (function (s) {
          var sel = curSeed === s.hex.toUpperCase();
          var btn = el(
            'button',
            {
              type: 'button',
              className: 'swatch',
              role: 'radio',
              'aria-label': s.name,
              'aria-checked': sel ? 'true' : 'false',
              onClick: function () {
                if (themeApi) themeApi.setSeed(s.hex);
                customColor.value = s.hex;
                buildSwatches();
              }
            },
            sel ? [icon('check')] : []
          );
          btn.style.background = s.hex;
          btn.style.color = '#ffffff';
          swatchGroup.appendChild(btn);
        })(seeds[i]);
      }
    }
    buildSwatches();

    customColor.addEventListener('input', function () {
      if (themeApi) themeApi.setSeed(customColor.value);
      buildSwatches();
    });

    var motionSwitch = el('input', {
      type: 'checkbox',
      role: 'switch',
      className: 'switch',
      id: 'motion-switch',
      checked: themeApi ? themeApi.getMotion() : false
    });
    motionSwitch.addEventListener('change', function () {
      if (themeApi) themeApi.setMotion(motionSwitch.checked);
    });

    var blackSwitch = el('input', {
      type: 'checkbox',
      role: 'switch',
      className: 'switch',
      id: 'black-switch',
      'aria-describedby': 'black-switch-help',
      checked: themeApi && themeApi.getBlack ? themeApi.getBlack() : false
    });
    blackSwitch.addEventListener('change', function () {
      if (themeApi && themeApi.setBlack) themeApi.setBlack(blackSwitch.checked);
    });

    var appearanceCard = el('section', { className: 'section-card' }, [
      el('h2', { className: 'type-title-medium', text: 'Görünüm' }),
      el('div', { className: 'setting-row' }, [
        el('span', { className: 'type-body-large', text: 'Tema modu' }),
        modeGroup
      ]),
      el('div', { className: 'setting-row' }, [
        el('span', { className: 'type-body-large', text: 'Kaynak renk' }),
        el('div', { className: 'swatch-row' }, [swatchGroup, customColor])
      ]),
      el('div', { className: 'setting-row' }, [
        el('label', {
          for: 'motion-switch',
          className: 'type-body-large',
          text: 'Hareketi azalt'
        }),
        motionSwitch
      ]),
      el('div', { className: 'setting-row' }, [
        el('div', { className: 'setting-text' }, [
          el('label', {
            for: 'black-switch',
            className: 'type-body-large',
            text: 'Saf siyah'
          }),
          el('span', {
            id: 'black-switch-help',
            className: 'type-body-small setting-help',
            text: 'Koyu modda AMOLED siyahı. Açık temada etkisizdir.'
          })
        ]),
        blackSwitch
      ])
    ]);

    var curPw = createField({
      id: 'acc-current-password',
      label: 'Mevcut parola',
      type: 'password',
      required: true,
      autocomplete: 'current-password'
    });
    var newPw = createField({
      id: 'acc-new-password',
      label: 'Yeni parola',
      type: 'password',
      required: true,
      autocomplete: 'new-password'
    });
    var pwForm = el(
      'form',
      {
        className: 'section-stack',
        onSubmit: async function (e) {
          e.preventDefault();
          curPw.setError('');
          newPw.setError('');
          if (newPw.input.value.length < 8) {
            newPw.setError('En az 8 karakter girin.');
            return;
          }
          try {
            await api('PUT', '/api/me/password', {
              current_password: curPw.input.value,
              new_password: newPw.input.value
            });
            curPw.input.value = '';
            newPw.input.value = '';
            toast('Parola değiştirildi');
          } catch (err) {
            toast(err.message);
          }
        }
      },
      [
        curPw.wrap,
        newPw.wrap,
        el('div', {}, [
          el('button', {
            type: 'submit',
            className: 'btn filled',
            text: 'Parolayı değiştir'
          })
        ])
      ]
    );

    var pwCard = el('section', { className: 'section-card' }, [
      el('h2', { className: 'type-title-medium', text: 'Parola değiştir' }),
      pwForm
    ]);

    var logoutCard = el('section', { className: 'section-card' }, [
      el('div', { className: 'setting-row' }, [
        el('span', {
          className: 'type-body-large',
          text: 'Oturum: ' + (KRS.me ? KRS.me.username : '')
        }),
        el(
          'button',
          {
            type: 'button',
            className: 'btn outlined',
            text: 'Çıkış',
            onClick: async function () {
              try {
                await api('POST', '/api/logout', {});
              } catch (_) {}
              KRS.me = null;
              navigate('#/giris');
            }
          },
          [icon('logout')]
        )
      ])
    ]);

    mainPage.appendChild(
      el('div', { className: 'section-stack' }, [
        appearanceCard,
        pwCard,
        logoutCard
      ])
    );
  }

  // Yönetim sayfası (#/yonetim)
  async function renderAdmin(gen) {
    var myGen = gen !== undefined ? gen : KRS.routeGen;
    if (!KRS.me || !KRS.me.is_admin) {
      navigate('#/');
      return;
    }
    setTopBar({ title: 'Yönetim' });
    clearNode(mainPage);

    var headerRow = el('div', { className: 'setting-row' }, [
      el('h2', { className: 'type-title-medium', text: 'Kullanıcılar' }),
      el('div', { className: 'chip-group' }, [
        el(
          'button',
          {
            type: 'button',
            className: 'btn tonal',
            text: 'Kütüphaneyi tara',
            onClick: async function () {
              try {
                await api('POST', '/api/admin/scan', {});
                toast('Tarama başlatıldı');
              } catch (err) {
                toast(err.message);
              }
            }
          },
          [icon('sync')]
        ),
        el(
          'button',
          {
            type: 'button',
            className: 'btn filled',
            text: 'Kullanıcı ekle',
            onClick: function () {
              openAddUserDialog();
            }
          },
          [icon('add')]
        )
      ])
    ]);

    var listWrap = el('div');
    mainPage.appendChild(
      el('div', { className: 'section-stack' }, [headerRow, listWrap])
    );

    function openAddUserDialog() {
      var dlg = el('dialog', { className: 'md-dialog' });
      var uF = createField({
        id: 'add-user-name',
        label: 'Kullanıcı adı',
        required: true
      });
      var pF = createField({
        id: 'add-user-pw',
        label: 'Parola',
        type: 'password',
        required: true
      });
      var cancelBtn = el('button', {
        type: 'button',
        className: 'btn text',
        text: 'İptal',
        onClick: function () {
          dlg.close();
          dlg.remove();
        }
      });
      var saveBtn = el('button', {
        type: 'submit',
        className: 'btn filled',
        text: 'Ekle'
      });
      var form = el(
        'form',
        {
          className: 'dialog-body',
          onSubmit: async function (e) {
            e.preventDefault();
            try {
              await api('POST', '/api/admin/users', {
                username: uF.input.value.trim(),
                password: pF.input.value
              });
              dlg.close();
              dlg.remove();
              toast('Kullanıcı eklendi');
              loadUsers();
            } catch (err) {
              toast(err.message);
            }
          }
        },
        [
          uF.wrap,
          pF.wrap,
          el('div', { className: 'dialog-actions' }, [cancelBtn, saveBtn])
        ]
      );
      dlg.appendChild(
        el('h2', { className: 'dialog-title', text: 'Kullanıcı ekle' })
      );
      dlg.appendChild(form);
      dlg.addEventListener('close', function () {
        dlg.remove();
      });
      document.body.appendChild(dlg);
      dlg.showModal();
    }

    function openResetPwDialog(user) {
      var dlg = el('dialog', { className: 'md-dialog' });
      var pF = createField({
        id: 'reset-user-pw',
        label: 'Yeni parola',
        type: 'password',
        required: true
      });
      var cancelBtn = el('button', {
        type: 'button',
        className: 'btn text',
        text: 'İptal',
        onClick: function () {
          dlg.close();
          dlg.remove();
        }
      });
      var saveBtn = el('button', {
        type: 'submit',
        className: 'btn filled',
        text: 'Sıfırla'
      });
      var form = el(
        'form',
        {
          className: 'dialog-body',
          onSubmit: async function (e) {
            e.preventDefault();
            try {
              await api('PUT', '/api/admin/users/' + user.id + '/password', {
                password: pF.input.value
              });
              dlg.close();
              dlg.remove();
              toast('Parola sıfırlandı');
            } catch (err) {
              toast(err.message);
            }
          }
        },
        [pF.wrap, el('div', { className: 'dialog-actions' }, [cancelBtn, saveBtn])]
      );
      dlg.appendChild(
        el('h2', {
          className: 'dialog-title',
          text: user.username + ' parolasını sıfırla'
        })
      );
      dlg.appendChild(form);
      dlg.addEventListener('close', function () {
        dlg.remove();
      });
      document.body.appendChild(dlg);
      dlg.showModal();
    }

    async function loadUsers() {
      clearNode(listWrap);
      try {
        var users = await api('GET', '/api/admin/users');
        if (myGen !== KRS.routeGen) return;
        var ul = el('ul', { className: 'list' });
        for (var i = 0; i < users.length; i++) {
          (function (u) {
            var letter = (u.username.charAt(0) || '?').toUpperCase();
            var actions = [
              el(
                'button',
                {
                  type: 'button',
                  className: 'icon-btn',
                  'aria-label': 'Parolayı sıfırla',
                  title: 'Parolayı sıfırla',
                  onClick: function () {
                    openResetPwDialog(u);
                  }
                },
                [icon('lock')]
              )
            ];
            if (!KRS.me || u.id !== KRS.me.id) {
              actions.push(
                el(
                  'button',
                  {
                    type: 'button',
                    className: 'icon-btn',
                    'aria-label': 'Sil',
                    title: 'Sil',
                    onClick: async function () {
                      var ok = await confirmDialog(
                        'Kullanıcıyı sil',
                        u.username + ' kullanıcısı silinsin mi?',
                        'Sil'
                      );
                      if (!ok) return;
                      try {
                        await api('DELETE', '/api/admin/users/' + u.id, {});
                        toast('Kullanıcı silindi');
                        loadUsers();
                      } catch (err) {
                        toast(err.message);
                      }
                    }
                  },
                  [icon('delete')]
                )
              );
            }
            ul.appendChild(
              el('li', { className: 'list-item' }, [
                el('span', { className: 'avatar', text: letter }),
                el('div', { className: 'list-content' }, [
                  el('h3', { className: 'list-title', text: u.username })
                ]),
                el('span', {
                  className: 'badge',
                  text: u.is_admin ? 'Yönetici' : 'Kullanıcı'
                }),
                el('div', { className: 'chip-group' }, actions)
              ])
            );
          })(users[i]);
        }
        listWrap.appendChild(ul);
      } catch (err) {
        if (myGen !== KRS.routeGen) return;
        toast(err.message);
      }
    }

    loadUsers();
  }

  async function handleRoute() {
    var gen = ++KRS.routeGen;
    setCleanup(null);
    if (KRS.tracker && typeof KRS.tracker.stop === 'function') {
      KRS.tracker.stop();
    }
    document.body.classList.remove('reading');

    var hash = location.hash || '#/';
    if (hash === '#/kurulum') {
      await renderSetup();
      return;
    }
    if (hash === '#/giris') {
      await renderLogin(gen);
      return;
    }

    if (!KRS.me) {
      try {
        var st = await api('GET', '/api/setup-status');
        if (gen !== KRS.routeGen) return;
        if (st && st.needs_setup) {
          navigate('#/kurulum');
          return;
        }
        KRS.me = await api('GET', '/api/me');
        if (gen !== KRS.routeGen) return;
      } catch (_) {
        if (gen !== KRS.routeGen) return;
        navigate('#/giris');
        return;
      }
    }

    shell.classList.remove('auth-only');
    renderNav(hash);

    if (hash === '#/' || hash === '#') {
      renderLibrary(gen);
    } else if (hash.indexOf('#/seri/') === 0) {
      renderSeries(hash.slice('#/seri/'.length), gen);
    } else if (hash.indexOf('#/oku/') === 0) {
      await renderReader(hash.slice('#/oku/'.length), gen);
    } else if (hash === '#/gecmis') {
      setTopBar({ title: 'Geçmiş' });
      if (KRS.stats && typeof KRS.stats.renderHistory === 'function') {
        KRS.stats.renderHistory(mainPage, gen);
      }
    } else if (hash === '#/istatistik') {
      setTopBar({ title: 'İstatistik' });
      if (KRS.stats && typeof KRS.stats.renderStats === 'function') {
        var cleanupStats = await KRS.stats.renderStats(mainPage, gen);
        if (gen !== KRS.routeGen) {
          if (typeof cleanupStats === 'function') cleanupStats();
          return;
        }
        setCleanup(cleanupStats);
      }
    } else if (hash === '#/hesap') {
      renderAccount();
    } else if (hash === '#/yonetim') {
      await renderAdmin(gen);
    } else {
      navigate('#/');
    }
  }

  KRS.api = api;
  KRS.toast = toast;
  KRS.confirm = confirmDialog;
  KRS.navigate = navigate;
  KRS.setCleanup = setCleanup;
  KRS.todayLocal = todayLocal;
  KRS.icon = icon;
  KRS.el = el;
  KRS.clearNode = clearNode;
  KRS.createCircularProgress = createCircularProgress;
  KRS.createLinearProgress = createLinearProgress;
  KRS.createField = createField;

  document.addEventListener('DOMContentLoaded', function () {
    buildShell();
    initRipple();
    window.addEventListener('hashchange', handleRoute);
    handleRoute();
  });
})();
