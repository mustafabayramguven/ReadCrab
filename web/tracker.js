(function () {
  'use strict';

  var KRS = (window.KRS = window.KRS || {});

  var session = null;
  var secTimer = null;
  var pingTimer = null;
  var quickPingTimer = null;
  var progTimer = null;
  var pendingProgress = null;

  function randomClientId() {
    var bytes = new Uint8Array(16);
    window.crypto.getRandomValues(bytes);
    var out = '';
    for (var i = 0; i < bytes.length; i++) {
      var b = bytes[i];
      out += (b < 16 ? '0' : '') + b.toString(16);
    }
    return out;
  }

  function localDateStr() {
    if (KRS.todayLocal) return KRS.todayLocal();
    var d = new Date();
    var y = d.getFullYear();
    var m = String(d.getMonth() + 1).padStart(2, '0');
    var day = String(d.getDate()).padStart(2, '0');
    return y + '-' + m + '-' + day;
  }

  function clamp01(p) {
    var n = Number(p);
    if (!Number.isFinite(n)) return 0;
    return n < 0 ? 0 : n > 1 ? 1 : n;
  }

  function buildPingBody(s) {
    return {
      client_id: s.clientId,
      book_id: Number(s.bookId),
      active_seconds: Math.min(86400, Math.max(0, Math.floor(s.activeSeconds))),
      pages_read: Math.min(100000, s.pages.size),
      start_percent: clamp01(s.startPercent),
      end_percent: clamp01(s.endPercent),
      local_date: s.localDate
    };
  }

  function sendPingFetch(s, keepalive) {
    if (!s) return;
    var body = buildPingBody(s);
    fetch('/api/reading/ping', {
      method: 'POST',
      credentials: 'same-origin',
      keepalive: Boolean(keepalive),
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify(body)
    }).catch(function () {});
  }

  function sendPingBeacon(s) {
    if (!s) return;
    var body = buildPingBody(s);
    if (navigator.sendBeacon) {
      var blob = new Blob([JSON.stringify(body)], { type: 'application/json' });
      navigator.sendBeacon('/api/reading/ping', blob);
    } else {
      sendPingFetch(s, true);
    }
  }

  function flushProgress(keepalive) {
    if (progTimer) {
      clearTimeout(progTimer);
      progTimer = null;
    }
    if (!pendingProgress) return;
    var p = pendingProgress;
    pendingProgress = null;
    fetch('/api/books/' + encodeURIComponent(p.bookId) + '/progress', {
      method: 'PUT',
      credentials: 'same-origin',
      keepalive: Boolean(keepalive),
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify({
        locator: String(p.locator),
        percent: clamp01(p.percent),
        local_date: p.localDate
      })
    }).catch(function () {});
  }

  function scheduleProgress(bookId, locator, percent) {
    var pct = clamp01(percent);
    if (session && Number(session.bookId) === Number(bookId)) {
      session.endPercent = pct;
    }
    pendingProgress = {
      bookId: bookId,
      locator: String(locator),
      percent: pct,
      localDate: session ? session.localDate : localDateStr()
    };
    if (progTimer) clearTimeout(progTimer);
    progTimer = setTimeout(function () {
      flushProgress(false);
    }, 1000);
  }

  function touch() {
    if (!session) return;
    session.lastInputAt = Date.now();
  }

  function scheduleQuickPing() {
    if (!session || quickPingTimer) return;
    var s = session;
    quickPingTimer = setTimeout(function () {
      quickPingTimer = null;
      if (session === s) {
        sendPingFetch(s, false);
      }
    }, 250);
  }

  function addPage(key) {
    if (!session) return;
    session.lastInputAt = Date.now();
    var prevSize = session.pages.size;
    session.pages.add(String(key));
    if (session.pages.size !== prevSize) {
      scheduleQuickPing();
    }
  }

  function setPercent(p) {
    if (!session) return;
    session.lastInputAt = Date.now();
    session.endPercent = clamp01(p);
    scheduleQuickPing();
  }

  function clearTimers() {
    if (secTimer) {
      clearInterval(secTimer);
      secTimer = null;
    }
    if (pingTimer) {
      clearInterval(pingTimer);
      pingTimer = null;
    }
    if (quickPingTimer) {
      clearTimeout(quickPingTimer);
      quickPingTimer = null;
    }
  }

  function start(bookId, startPercent) {
    stop();
    var sp = clamp01(startPercent);
    session = {
      clientId: randomClientId(),
      bookId: Number(bookId),
      activeSeconds: 1,
      pages: new Set(),
      startPercent: sp,
      endPercent: sp,
      localDate: localDateStr(),
      lastInputAt: Date.now()
    };

    secTimer = setInterval(function () {
      if (!session) return;
      if (
        document.visibilityState === 'visible' &&
        Date.now() - session.lastInputAt <= 60000
      ) {
        session.activeSeconds += 1;
      }
    }, 1000);

    pingTimer = setInterval(function () {
      if (!session) return;
      sendPingFetch(session, false);
    }, 15000);
  }

  function stop() {
    clearTimers();
    flushProgress(true);
    if (session) {
      var s = session;
      session = null;
      sendPingFetch(s, true);
    }
  }

  var activityEvents = ['pointerdown', 'keydown', 'touchstart', 'wheel', 'scroll'];
  for (var i = 0; i < activityEvents.length; i++) {
    window.addEventListener(activityEvents[i], touch, { passive: true });
  }

  document.addEventListener('visibilitychange', function () {
    if (document.visibilityState === 'hidden' && session) {
      sendPingBeacon(session);
    }
  });

  window.addEventListener('pagehide', function () {
    flushProgress(true);
    if (session) {
      sendPingBeacon(session);
    }
  });

  KRS.tracker = {
    start: start,
    touch: touch,
    addPage: addPage,
    setPercent: setPercent,
    stop: stop,
    scheduleProgress: scheduleProgress,
    flushProgress: flushProgress
  };
})();
