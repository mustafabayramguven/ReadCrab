// shortcut: HCT yerine OKLCH, renkler resmi M3 üreticisinden birebir değil
(function () {
  'use strict';

  var SEEDS = [
    { name: 'Mor', hex: '#6750A4' },
    { name: 'Mavi', hex: '#0B57D0' },
    { name: 'Yeşil', hex: '#2E7D32' },
    { name: 'Turuncu', hex: '#C25E00' },
    { name: 'Pembe', hex: '#B3265F' },
    { name: 'Turkuaz', hex: '#00796B' }
  ];

  function clamp(v, lo, hi) {
    return v < lo ? lo : v > hi ? hi : v;
  }

  function srgbToLinear(c) {
    return c <= 0.04045 ? c / 12.92 : Math.pow((c + 0.055) / 1.055, 2.4);
  }

  function linearToSrgb(c) {
    var v = clamp(c, 0, 1);
    return v <= 0.0031308 ? 12.92 * v : 1.055 * Math.pow(v, 1 / 2.4) - 0.055;
  }

  function parseHex(hex) {
    var clean = String(hex || '').trim().replace(/^#/, '');
    if (!/^[0-9a-fA-F]{6}$/.test(clean)) {
      clean = '6750A4';
    }
    var n = parseInt(clean, 16);
    return [
      ((n >> 16) & 255) / 255,
      ((n >> 8) & 255) / 255,
      (n & 255) / 255
    ];
  }

  function toHexByte(v) {
    var b = Math.round(clamp(v, 0, 1) * 255);
    return (b < 16 ? '0' : '') + b.toString(16);
  }

  function rgbToHex(r, g, b) {
    return '#' + toHexByte(r) + toHexByte(g) + toHexByte(b);
  }

  function toneToY(tone) {
    var t = clamp(tone, 0, 100);
    return t > 8 ? Math.pow((t + 16) / 116, 3) : t / 903.3;
  }

  function luminance(hex) {
    var rgb = parseHex(hex);
    var r = srgbToLinear(rgb[0]);
    var g = srgbToLinear(rgb[1]);
    var b = srgbToLinear(rgb[2]);
    return 0.2126 * r + 0.7152 * g + 0.0722 * b;
  }

  function contrast(hexA, hexB) {
    var ya = luminance(hexA);
    var yb = luminance(hexB);
    var ymax = Math.max(ya, yb);
    var ymin = Math.min(ya, yb);
    return (ymax + 0.05) / (ymin + 0.05);
  }

  function hexToOklch(hex) {
    var rgb = parseHex(hex);
    var r = srgbToLinear(rgb[0]);
    var g = srgbToLinear(rgb[1]);
    var b = srgbToLinear(rgb[2]);

    var l = 0.4122214708 * r + 0.5363325363 * g + 0.0514459929 * b;
    var m = 0.2119034982 * r + 0.6806995451 * g + 0.1073969566 * b;
    var s = 0.0883024619 * r + 0.2817188376 * g + 0.6299787005 * b;

    var l_ = Math.cbrt(l);
    var m_ = Math.cbrt(m);
    var s_ = Math.cbrt(s);

    var L = 0.2104542553 * l_ + 0.7936177850 * m_ - 0.0040720468 * s_;
    var a = 1.9779984951 * l_ - 2.4285922050 * m_ + 0.4505937099 * s_;
    var bLab = 0.0259040371 * l_ + 0.7827717662 * m_ - 0.8086757660 * s_;

    var C = Math.hypot(a, bLab);
    var h = (Math.atan2(bLab, a) * 180 / Math.PI) % 360;
    if (h < 0) h += 360;
    return { L: L, C: C, h: h };
  }

  function oklchToLinRgb(L, C, h) {
    var rad = h * Math.PI / 180;
    var a = C * Math.cos(rad);
    var b = C * Math.sin(rad);

    var l_ = L + 0.3963377774 * a + 0.2158037573 * b;
    var m_ = L - 0.1055613458 * a - 0.0638541728 * b;
    var s_ = L - 0.0894841775 * a - 1.2914855480 * b;

    var l = l_ * l_ * l_;
    var m = m_ * m_ * m_;
    var s = s_ * s_ * s_;

    var r = 4.0767416621 * l - 3.3077115913 * m + 0.2309699292 * s;
    var g = -1.2684380046 * l + 2.6097574011 * m - 0.3413193965 * s;
    var bLin = -0.0041960863 * l - 0.7034186147 * m + 1.7076147010 * s;
    return [r, g, bLin];
  }

  function oklchToHex(oklch) {
    var lin = oklchToLinRgb(oklch.L, oklch.C, oklch.h);
    return rgbToHex(
      linearToSrgb(lin[0]),
      linearToSrgb(lin[1]),
      linearToSrgb(lin[2])
    );
  }

  function inGamut(rgb) {
    return rgb[0] >= 0 && rgb[0] <= 1 &&
           rgb[1] >= 0 && rgb[1] <= 1 &&
           rgb[2] >= 0 && rgb[2] <= 1;
  }

  function maxChroma(L, h, C) {
    if (inGamut(oklchToLinRgb(L, C, h))) {
      return C;
    }
    var lo = 0;
    var hi = C;
    for (var i = 0; i < 24; i++) {
      var mid = 0.5 * (lo + hi);
      if (inGamut(oklchToLinRgb(L, mid, h))) {
        lo = mid;
      } else {
        hi = mid;
      }
    }
    return lo;
  }

  function colorAtTone(h, C, tone) {
    var Yt = toneToY(tone);
    if (Yt <= 0.0001) return '#000000';
    if (Yt >= 0.9999) return '#ffffff';

    var L = Math.cbrt(Yt);
    for (var i = 0; i < 12; i++) {
      var c = maxChroma(L, h, C);
      var lin = oklchToLinRgb(L, c, h);
      var r = clamp(lin[0], 0, 1);
      var g = clamp(lin[1], 0, 1);
      var b = clamp(lin[2], 0, 1);
      var Y = 0.2126 * r + 0.7152 * g + 0.0722 * b;
      if (Math.abs(Y - Yt) < 1e-4) {
        break;
      }
      L = clamp(L * Math.cbrt(Yt / Math.max(Y, 1e-6)), 0, 1);
    }

    var cf = maxChroma(L, h, C);
    var linf = oklchToLinRgb(L, cf, h);
    return rgbToHex(
      linearToSrgb(clamp(linf[0], 0, 1)),
      linearToSrgb(clamp(linf[1], 0, 1)),
      linearToSrgb(clamp(linf[2], 0, 1))
    );
  }

  function generatePalette(seedHex, dark) {
    var o = hexToOklch(seedHex);
    var h0 = o.h;
    var Cp = clamp(o.C, 0.05, 0.17);

    var P = [h0, Cp];
    var S = [h0, Math.max(0.02, Cp * 0.33)];
    var T = [(h0 + 60) % 360, Math.max(0.04, Cp * 0.55)];
    var N = [h0, 0.006];
    var NV = [h0, 0.012];
    var E = [29.2, 0.19];

    function tn(pal, t) {
      return colorAtTone(pal[0], pal[1], t);
    }

    if (!dark) {
      return {
        'primary': tn(P, 40),
        'on-primary': tn(P, 100),
        'primary-container': tn(P, 90),
        'on-primary-container': tn(P, 10),
        'secondary': tn(S, 40),
        'on-secondary': tn(S, 100),
        'secondary-container': tn(S, 90),
        'on-secondary-container': tn(S, 10),
        'tertiary': tn(T, 40),
        'on-tertiary': tn(T, 100),
        'tertiary-container': tn(T, 90),
        'on-tertiary-container': tn(T, 10),
        'error': tn(E, 40),
        'on-error': tn(E, 100),
        'error-container': tn(E, 90),
        'on-error-container': tn(E, 10),
        'surface': tn(N, 98),
        'surface-dim': tn(N, 87),
        'surface-bright': tn(N, 98),
        'surface-container-lowest': tn(N, 100),
        'surface-container-low': tn(N, 96),
        'surface-container': tn(N, 94),
        'surface-container-high': tn(N, 92),
        'surface-container-highest': tn(N, 90),
        'on-surface': tn(N, 10),
        'on-surface-variant': tn(NV, 30),
        'outline': tn(NV, 50),
        'outline-variant': tn(NV, 80),
        'inverse-surface': tn(N, 20),
        'inverse-on-surface': tn(N, 95),
        'inverse-primary': tn(P, 80),
        'scrim': '#000000'
      };
    }

    return {
      'primary': tn(P, 80),
      'on-primary': tn(P, 20),
      'primary-container': tn(P, 30),
      'on-primary-container': tn(P, 90),
      'secondary': tn(S, 80),
      'on-secondary': tn(S, 20),
      'secondary-container': tn(S, 30),
      'on-secondary-container': tn(S, 90),
      'tertiary': tn(T, 80),
      'on-tertiary': tn(T, 20),
      'tertiary-container': tn(T, 30),
      'on-tertiary-container': tn(T, 90),
      'error': tn(E, 80),
      'on-error': tn(E, 20),
      'error-container': tn(E, 30),
      'on-error-container': tn(E, 90),
      'surface': tn(N, 6),
      'surface-dim': tn(N, 6),
      'surface-bright': tn(N, 24),
      'surface-container-lowest': tn(N, 4),
      'surface-container-low': tn(N, 10),
      'surface-container': tn(N, 12),
      'surface-container-high': tn(N, 17),
      'surface-container-highest': tn(N, 22),
      'on-surface': tn(N, 90),
      'on-surface-variant': tn(NV, 80),
      'outline': tn(NV, 60),
      'outline-variant': tn(NV, 30),
      'inverse-surface': tn(N, 90),
      'inverse-on-surface': tn(N, 20),
      'inverse-primary': tn(P, 40),
      'scrim': '#000000'
    };
  }

  function readerColors(name, seedHex) {
    if (name === 'sepia') {
      return { bg: '#F4ECD8', fg: '#433422', link: '#7A4A12' };
    }
    var dark = name === 'dark';
    var p = generatePalette(seedHex || getSeed(), dark);
    return {
      bg: p['surface'],
      fg: p['on-surface'],
      link: p['primary']
    };
  }

  function validSeed(s) {
    return typeof s === 'string' && /^#[0-9a-fA-F]{6}$/.test(s) ? s.toUpperCase() : '#6750A4';
  }

  function validMode(m) {
    return m === 'light' || m === 'dark' || m === 'auto' ? m : 'auto';
  }

  function getSeed() {
    try {
      return validSeed(localStorage.getItem('krs.seed'));
    } catch (_) {
      return '#6750A4';
    }
  }

  function getMode() {
    try {
      return validMode(localStorage.getItem('krs.mode'));
    } catch (_) {
      return 'auto';
    }
  }

  function getMotion() {
    try {
      return localStorage.getItem('krs.motion') === 'reduced';
    } catch (_) {
      return false;
    }
  }

  function apply(seedHex, mode) {
    if (typeof document === 'undefined') return;
    var s = validSeed(seedHex);
    var m = validMode(mode);
    var prefersDark = typeof window !== 'undefined' &&
      window.matchMedia &&
      window.matchMedia('(prefers-color-scheme: dark)').matches;
    var dark = m === 'dark' || (m === 'auto' && prefersDark);
    var pal = generatePalette(s, dark);
    var root = document.documentElement;
    var keys = Object.keys(pal);
    for (var i = 0; i < keys.length; i++) {
      root.style.setProperty('--md-sys-color-' + keys[i], pal[keys[i]]);
    }
    root.dataset.theme = dark ? 'dark' : 'light';
    root.style.colorScheme = dark ? 'dark' : 'light';
    var meta = document.querySelector('meta[name="theme-color"]');
    if (meta) {
      meta.setAttribute('content', pal['surface']);
    }
  }

  function setSeed(hex) {
    var s = validSeed(hex);
    try {
      localStorage.setItem('krs.seed', s);
    } catch (_) {}
    apply(s, getMode());
  }

  function setMode(m) {
    var mode = validMode(m);
    try {
      localStorage.setItem('krs.mode', mode);
    } catch (_) {}
    apply(getSeed(), mode);
  }

  function setMotion(reduced) {
    var isRed = Boolean(reduced);
    try {
      localStorage.setItem('krs.motion', isRed ? 'reduced' : 'normal');
    } catch (_) {}
    if (typeof document !== 'undefined') {
      if (isRed) {
        document.documentElement.dataset.motion = 'reduced';
      } else {
        delete document.documentElement.dataset.motion;
      }
    }
  }

  var mediaBound = false;
  function init() {
    if (typeof document === 'undefined') return;
    if (getMotion()) {
      document.documentElement.dataset.motion = 'reduced';
    } else {
      delete document.documentElement.dataset.motion;
    }
    apply(getSeed(), getMode());
    if (!mediaBound && typeof window !== 'undefined' && window.matchMedia) {
      mediaBound = true;
      var mq = window.matchMedia('(prefers-color-scheme: dark)');
      if (mq && mq.addEventListener) {
        mq.addEventListener('change', function () {
          if (getMode() === 'auto') {
            apply(getSeed(), 'auto');
          }
        });
      }
    }
  }

  var api = {
    SEEDS: SEEDS,
    toneToY: toneToY,
    luminance: luminance,
    contrast: contrast,
    hexToOklch: hexToOklch,
    oklchToHex: oklchToHex,
    maxChroma: maxChroma,
    colorAtTone: colorAtTone,
    generatePalette: generatePalette,
    readerColors: readerColors,
    apply: apply,
    init: init,
    setSeed: setSeed,
    setMode: setMode,
    setMotion: setMotion,
    getSeed: getSeed,
    getMode: getMode,
    getMotion: getMotion
  };

  if (typeof window !== 'undefined') {
    window.KRS_THEME = api;
    init();
  }
  if (typeof module !== 'undefined' && module.exports) {
    module.exports = api;
  }
})();
