// Satır başı konumu (locator "c"): bölüm gövdesinde konumdan önceki anlamlı birim sayısı.
// Boşluk ve görünmez biçim karakterleri sayılmaz, her <img> 1 birimdir. Sayım satır genişliğine,
// yazı tipine ve cihaza bağlı değildir; web, Android (aynı dosya) ve terminal okuyucu
// (crates/tui/src/wrap.rs: `significant`) aynı kümeyi kullanır, biri değişirse hepsi değişmeli.
// Tarayıcıda window.KRS_ANCHOR, Node'da module.exports.
(function () {
  'use strict';

  var PIN_CLASS = 'rc-pin';
  var SKIP = { script: 1, style: 1, noscript: 1, template: 1, title: 1 };

  // u: UTF-16 kod birimi (BMP). Taşan kod noktaları (vekil çiftler) her zaman anlamlıdır.
  function significant(u) {
    return !(
      u <= 0x20 ||
      (u >= 0x7f && u <= 0x9f) ||
      u === 0xa0 ||
      u === 0xad ||
      u === 0x1680 ||
      (u >= 0x2000 && u <= 0x200f) ||
      (u >= 0x2028 && u <= 0x202f) ||
      (u >= 0x205f && u <= 0x2064) ||
      u === 0x3000 ||
      u === 0xfeff
    );
  }

  function pairLen(s, i) {
    var u = s.charCodeAt(i);
    if (u >= 0xd800 && u <= 0xdbff && i + 1 < s.length) {
      var lo = s.charCodeAt(i + 1);
      if (lo >= 0xdc00 && lo <= 0xdfff) return 2;
    }
    return 1;
  }

  function countText(s) {
    var n = 0;
    for (var i = 0; i < s.length; ) {
      var len = pairLen(s, i);
      if (len === 2 || significant(s.charCodeAt(i))) n++;
      i += len;
    }
    return n;
  }

  // s içindeki k. anlamlı kod noktasının konumu (kod birimi) ve uzunluğu
  function unitAt(s, k) {
    for (var i = 0; i < s.length; ) {
      var len = pairLen(s, i);
      if (len === 2 || significant(s.charCodeAt(i))) {
        if (k === 0) return { off: i, len: len };
        k--;
      }
      i += len;
    }
    return null;
  }

  // Bölüm gövdesinin birim dizini. Belge değişince (belirteç eklenince) yeniden alınmalı.
  function index(doc) {
    var ix = { doc: doc, items: [], starts: [], total: 0 };
    var body = doc && doc.body;
    if (!body) return ix;
    var walker = doc.createTreeWalker(body, 0x5 /* ELEMENT | TEXT */, {
      acceptNode: function (n) {
        if (n.nodeType === 3) return 1;
        var name = String(n.localName || '').toLowerCase();
        if (SKIP[name] || (n.classList && n.classList.contains(PIN_CLASS))) return 2;
        return name === 'img' || name === 'image' ? 1 : 3;
      }
    });
    for (var n = walker.nextNode(); n; n = walker.nextNode()) {
      var c = n.nodeType === 3 ? countText(n.data) : 1;
      if (c > 0) {
        ix.items.push(n);
        ix.starts.push(ix.total);
        ix.total += c;
      }
    }
    return ix;
  }

  function locate(ix, k) {
    if (!ix || !(k >= 0) || k >= ix.total) return null;
    var lo = 0;
    var hi = ix.starts.length - 1;
    while (lo < hi) {
      var mid = (lo + hi + 1) >> 1;
      if (ix.starts[mid] <= k) lo = mid;
      else hi = mid - 1;
    }
    var node = ix.items[lo];
    if (node.nodeType !== 3) return { node: node, off: -1, len: 0 };
    var u = unitAt(node.data, k - ix.starts[lo]);
    return u ? { node: node, off: u.off, len: u.len } : null;
  }

  // k biriminin görünüm koordinatındaki dikdörtgeni; görünmüyorsa null
  function rectOf(ix, k) {
    var pos = locate(ix, k);
    if (!pos) return null;
    if (pos.off < 0) return pos.node.getBoundingClientRect();
    var range = ix.doc.createRange();
    range.setStart(pos.node, pos.off);
    range.setEnd(pos.node, pos.off + pos.len);
    var rs = range.getClientRects();
    return rs.length ? rs[0] : null;
  }

  // pred'i sağlayan ilk birim (pred belge sırasında monoton); yoksa -1
  function first(ix, pred) {
    if (!ix || !ix.total) return -1;
    var lo = 0;
    var hi = ix.total;
    while (lo < hi) {
      var mid = (lo + hi) >> 1;
      var r = rectOf(ix, mid);
      if (r && pred(r)) hi = mid;
      else lo = mid + 1;
    }
    return lo < ix.total ? lo : -1;
  }

  // Kaydırma modu: üst kenardan tam görünen ilk satırın ilk birimi
  function topUnit(ix) {
    return first(ix, function (r) {
      return r.top >= -2;
    });
  }

  // Sayfa modu: belge koordinatında pageLeft'te başlayan sayfanın ilk birimi
  function pageUnit(ix, scrollLeft, pageLeft) {
    return first(ix, function (r) {
      return r.left + scrollLeft >= pageLeft - 1;
    });
  }

  function unpin(doc) {
    var old = doc.querySelectorAll('.' + PIN_CLASS);
    for (var i = 0; i < old.length; i++) old[i].parentNode.removeChild(old[i]);
  }

  // "Kaldığın yer" belirtecini k biriminin önüne koyar (eskisi kalkar). Dizin bundan sonra
  // eskir; çağıran index() ile yenisini almalı.
  function pin(ix, k) {
    if (!ix || !ix.total) return false;
    unpin(ix.doc);
    var pos = locate(ix, Math.max(0, Math.min(ix.total - 1, k)));
    if (!pos) return false;
    var el = ix.doc.createElement('span');
    el.className = PIN_CLASS;
    el.setAttribute('aria-hidden', 'true');
    var at = pos.off > 0 ? pos.node.splitText(pos.off) : pos.node;
    at.parentNode.insertBefore(el, at);
    return true;
  }

  var api = {
    significant: significant,
    countText: countText,
    unitAt: unitAt,
    index: index,
    rectOf: rectOf,
    topUnit: topUnit,
    pageUnit: pageUnit,
    pin: pin,
    unpin: unpin
  };

  if (typeof window !== 'undefined') window.KRS_ANCHOR = api;
  if (typeof module !== 'undefined' && module.exports) module.exports = api;
})();
