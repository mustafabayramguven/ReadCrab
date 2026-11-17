(function () {
  'use strict';

  // Çevrimdışı okuma (Play Books'taki "İndir" gibi): kitabın bölümleri/sayfaları ve görselleri
  // tarayıcı önbelleğine kaydedilir, service worker internet yokken bunları sunar.
  var KRS = (window.KRS = window.KRS || {});

  var META_KEY = 'krs.offline.books';
  var API = 'readcrab-api-v1';
  var BOOKS = 'readcrab-books-v1';
  var PARALLEL = 4;

  var supported =
    'serviceWorker' in navigator && 'caches' in window && window.isSecureContext === true;
  var jobs = {};
  var listeners = [];

  if (supported) {
    window.addEventListener('load', function () {
      navigator.serviceWorker.register('/static/sw.js', { scope: '/' }).catch(function () {});
    });
  }

  function readMeta() {
    try {
      var m = JSON.parse(localStorage.getItem(META_KEY) || '{}');
      return m && typeof m === 'object' ? m : {};
    } catch (_) {
      return {};
    }
  }

  function writeMeta(m) {
    try {
      localStorage.setItem(META_KEY, JSON.stringify(m));
    } catch (_) {}
  }

  function emit(id) {
    listeners.slice().forEach(function (fn) {
      try {
        fn(Number(id));
      } catch (_) {}
    });
  }

  function onChange(fn) {
    listeners.push(fn);
    return function () {
      listeners = listeners.filter(function (x) {
        return x !== fn;
      });
    };
  }

  function state(id) {
    var job = jobs[id];
    if (job) return { state: 'downloading', done: job.done, total: job.total };
    var m = readMeta()[id];
    if (m) return { state: 'ready', bytes: m.bytes || 0 };
    return { state: 'none' };
  }

  function list() {
    var m = readMeta();
    return Object.keys(m)
      .map(function (k) {
        return m[k];
      })
      .sort(function (a, b) {
        return (b.at || 0) - (a.at || 0);
      });
  }

  function isComic(fmt) {
    return ['cbz', 'cbr', 'zip', 'rar'].indexOf(String(fmt || '').toLowerCase()) !== -1;
  }

  async function fetchOk(url) {
    var res = await fetch(url, { credentials: 'same-origin', cache: 'no-store' });
    if (res.status === 401) throw new Error('Oturum kapanmış. Yeniden giriş yapın.');
    if (!res.ok) throw new Error('İndirilemedi (' + res.status + ').');
    return res;
  }

  function cancelled(job) {
    if (job.cancelled) throw new Error('İptal edildi.');
  }

  function assetUrls(html, bookId) {
    var re = new RegExp('/api/books/' + bookId + '/asset\\?p=[^"\'\\s<>)]+', 'g');
    var found = html.match(re) || [];
    var out = {};
    found.forEach(function (u) {
      out[u.replace(/&amp;/g, '&')] = true;
    });
    return Object.keys(out);
  }

  async function removeCached(id) {
    var prefix = '/api/books/' + id + '/';
    var cache = await caches.open(BOOKS);
    var keys = await cache.keys();
    await Promise.all(
      keys
        .filter(function (req) {
          return new URL(req.url).pathname.indexOf(prefix) === 0;
        })
        .map(function (req) {
          return cache.delete(req);
        })
    );
  }

  // Tüm URL'ler sırayla değil, PARALLEL kadar eşzamanlı indirilir.
  async function storeAll(urls, cache, job, onHtml) {
    var next = 0;
    async function worker() {
      while (next < urls.length) {
        var url = urls[next++];
        cancelled(job);
        var res = await fetchOk(url);
        if (onHtml) {
          var text = await res.clone().text();
          onHtml(text);
        }
        job.bytes += Number(res.headers.get('content-length')) || 0;
        await cache.put(url, res);
        job.done++;
        emit(job.id);
      }
    }
    var workers = [];
    for (var i = 0; i < Math.min(PARALLEL, urls.length); i++) workers.push(worker());
    await Promise.all(workers);
  }

  async function download(book) {
    if (!supported) {
      throw new Error('Çevrimdışı okuma için HTTPS gerekir (veya bu tarayıcı desteklemiyor).');
    }
    var id = Number(book.id);
    if (jobs[id]) return;
    var job = { id: id, done: 0, total: 1, bytes: 0, cancelled: false };
    jobs[id] = job;
    emit(id);
    try {
      if (navigator.storage && navigator.storage.persist) {
        navigator.storage.persist().catch(function () {});
      }
      var base = '/api/books/' + id;
      var api = await caches.open(API);
      var books = await caches.open(BOOKS);

      var detailRes = await fetchOk(base);
      var detail = await detailRes.clone().json();
      await api.put(base, detailRes);
      var coverUrl = base + '/cover' + (detail.cover_version !== undefined ? '?v=' + detail.cover_version : '');
      try {
        await api.put(coverUrl, await fetchOk(coverUrl));
      } catch (_) {
        // kapaksız kitap da okunur
      }

      if (isComic(detail.format)) {
        var pagesRes = await fetchOk(base + '/pages');
        var count = Number((await pagesRes.clone().json()).count) || 0;
        await books.put(base + '/pages', pagesRes);
        var pages = [];
        for (var p = 0; p < count; p++) pages.push(base + '/pages/' + p);
        job.total = pages.length || 1;
        emit(id);
        await storeAll(pages, books, job);
      } else {
        var tocRes = await fetchOk(base + '/toc');
        var toc = await tocRes.clone().json();
        await books.put(base + '/toc', tocRes);
        var n = Math.max(Number(detail.page_count) || 0, (toc.chapters || []).length, 1);
        var chapters = [];
        for (var c = 0; c < n; c++) chapters.push(base + '/chapters/' + c);
        var assets = {};
        job.total = chapters.length;
        emit(id);
        await storeAll(chapters, books, job, function (html) {
          assetUrls(html, id).forEach(function (u) {
            assets[u] = true;
          });
        });
        var assetList = Object.keys(assets);
        job.total += assetList.length;
        emit(id);
        await storeAll(assetList, books, job);
      }
      cancelled(job);
      var meta = readMeta();
      meta[id] = {
        id: id,
        title: detail.title,
        series_id: detail.series_id,
        series_name: detail.series_name,
        format: detail.format,
        cover_version: detail.cover_version,
        bytes: job.bytes,
        at: Date.now()
      };
      writeMeta(meta);
    } catch (err) {
      await removeCached(id).catch(function () {});
      throw err;
    } finally {
      delete jobs[id];
      emit(id);
    }
  }

  function cancel(id) {
    if (jobs[id]) jobs[id].cancelled = true;
  }

  async function remove(id) {
    cancel(id);
    await removeCached(id);
    var meta = readMeta();
    delete meta[id];
    writeMeta(meta);
    emit(id);
  }

  // Çıkışta başka bir kullanıcının görmemesi için kullanıcı verisi silinir.
  async function clearUserData() {
    Object.keys(jobs).forEach(cancel);
    writeMeta({});
    try {
      localStorage.removeItem('krs.pendingProgress');
    } catch (_) {}
    if ('caches' in window) {
      await Promise.all([caches.delete(API), caches.delete(BOOKS)]).catch(function () {});
    }
    listeners.forEach(function (fn) {
      try {
        fn(0);
      } catch (_) {}
    });
  }

  KRS.offline = {
    supported: supported,
    state: state,
    list: list,
    download: download,
    cancel: cancel,
    remove: remove,
    onChange: onChange,
    clearUserData: clearUserData
  };
})();
