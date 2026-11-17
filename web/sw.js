'use strict';

// ReadCrab service worker: uygulama kabuğu ve "İndir" ile kaydedilen kitaplar çevrimdışı açılır.
// Kitap içeriği önce önbellekten (Play Books gibi yerel okuma), diğer her şey önce ağdan gelir.

var VERSION = 'v1';
var SHELL = 'readcrab-shell-' + VERSION;
var API = 'readcrab-api-' + VERSION;
var BOOKS = 'readcrab-books-' + VERSION;
var KEEP = [SHELL, API, BOOKS];

var SHELL_URLS = [
  '/',
  '/static/style.css',
  '/static/reader-fonts.css',
  '/static/frame.css',
  '/static/theme.js',
  '/static/tracker.js',
  '/static/calc.js',
  '/static/offline.js',
  '/static/reader-ui.js',
  '/static/reader-comic.js',
  '/static/reader-text.js',
  '/static/stats.js',
  '/static/app.js',
  '/static/icons.svg',
  '/static/favicon.svg',
  '/static/manifest.webmanifest',
  '/static/icon-192.png',
  '/static/fonts/roboto-flex-latin.woff2',
  '/static/fonts/roboto-flex-latin-ext.woff2'
];

// İndirilen kitabın değişmeyen içeriği.
var BOOK_CONTENT = /^\/api\/books\/\d+\/(toc|pages|pages\/\d+|chapters\/\d+|asset)$/;
// Önbelleğe alınmayan, her zaman canlı olması gereken uçlar.
var LIVE_ONLY = /^\/api\/(login|logout|setup$|scan-status|admin\/|reading\/|books\/\d+\/(search|progress|status|bookmarks))/;

self.addEventListener('install', function (event) {
  event.waitUntil(
    caches
      .open(SHELL)
      .then(function (cache) {
        return cache.addAll(SHELL_URLS);
      })
      .then(function () {
        return self.skipWaiting();
      })
  );
});

self.addEventListener('activate', function (event) {
  event.waitUntil(
    caches
      .keys()
      .then(function (names) {
        return Promise.all(
          names
            .filter(function (n) {
              return n.indexOf('readcrab-') === 0 && KEEP.indexOf(n) === -1;
            })
            .map(function (n) {
              return caches.delete(n);
            })
        );
      })
      .then(function () {
        return self.clients.claim();
      })
  );
});

function timeout(ms) {
  return new Promise(function (_, reject) {
    setTimeout(function () {
      reject(new Error('timeout'));
    }, ms);
  });
}

// Ağ önce; ağ yoksa (veya önbellekte kopya varken çok yavaşsa) son başarılı yanıt.
async function networkFirst(request, cacheName) {
  var cache = await caches.open(cacheName);
  var cached = await cache.match(request);
  var network = fetch(request).then(function (res) {
    if (res.ok && res.type === 'basic') cache.put(request, res.clone());
    return res;
  });
  try {
    return await (cached ? Promise.race([network, timeout(5000)]) : network);
  } catch (err) {
    if (cached) return cached;
    if (request.mode === 'navigate') {
      var shell = await caches.match('/');
      if (shell) return shell;
    }
    return Response.error();
  }
}

async function bookContent(request) {
  var hit = await caches.match(request, { cacheName: BOOKS });
  if (hit) return hit;
  return fetch(request);
}

self.addEventListener('fetch', function (event) {
  var request = event.request;
  if (request.method !== 'GET') return;
  var url = new URL(request.url);
  if (url.origin !== self.location.origin) return;
  var path = url.pathname;
  if (BOOK_CONTENT.test(path)) {
    event.respondWith(bookContent(request));
  } else if (path.indexOf('/api/') === 0) {
    if (!LIVE_ONLY.test(path)) event.respondWith(networkFirst(request, API));
  } else if (path === '/' || path.indexOf('/static/') === 0) {
    event.respondWith(networkFirst(request, SHELL));
  }
});
