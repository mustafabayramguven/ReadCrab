import assert from 'node:assert/strict';
import { createRequire } from 'node:module';

const require = createRequire(import.meta.url);
const theme = require('./theme.js');

const {
  SEEDS,
  toneToY,
  luminance,
  contrast,
  hexToOklch,
  oklchToHex,
  colorAtTone,
  generatePalette,
  readerColors
} = theme;

function hexChannels(hex) {
  const clean = hex.replace(/^#/, '');
  return [
    parseInt(clean.slice(0, 2), 16),
    parseInt(clean.slice(2, 4), 16),
    parseInt(clean.slice(4, 6), 16)
  ];
}

// 1. roundtrip: 20 sabit örnek için oklchToHex(hexToOklch(x)) === x (±1 kanal toleransı)
const samples20 = [
  '#000000', '#ffffff', '#6750a4', '#0b57d0', '#2e7d32',
  '#c25e00', '#b3265f', '#00796b', '#b3261e', '#1d1b20',
  '#fef7ff', '#eaddff', '#ffd8e4', '#381e72', '#4f378b',
  '#141218', '#e6e0e9', '#79747e', '#f4ecd8', '#433422'
];
for (const sample of samples20) {
  const rt = oklchToHex(hexToOklch(sample));
  const [r1, g1, b1] = hexChannels(sample);
  const [r2, g2, b2] = hexChannels(rt);
  assert.ok(Math.abs(r1 - r2) <= 1, `roundtrip R mismatch for ${sample}: ${rt}`);
  assert.ok(Math.abs(g1 - g2) <= 1, `roundtrip G mismatch for ${sample}: ${rt}`);
  assert.ok(Math.abs(b1 - b2) <= 1, `roundtrip B mismatch for ${sample}: ${rt}`);
}

// 2. tone_luminance: 6 tohum × [10,40,80,90] tonu için |luminance(colorAtTone(h,Cp,t)) − toneToY(t)| < 0.01
for (const seed of SEEDS) {
  const { C, h } = hexToOklch(seed.hex);
  const Cp = Math.max(0.05, Math.min(0.17, C));
  for (const t of [10, 40, 80, 90]) {
    const col = colorAtTone(h, Cp, t);
    const diff = Math.abs(luminance(col) - toneToY(t));
    assert.ok(diff < 0.01, `tone_luminance failed for ${seed.hex} tone=${t}: diff=${diff}`);
  }
}

// 3. palette_shape: her tohum ve mod için 15.3 tablosundaki tüm roller var ve hepsi /^#[0-9a-f]{6}$/
const expectedRoles = [
  'primary', 'on-primary', 'primary-container', 'on-primary-container',
  'secondary', 'on-secondary', 'secondary-container', 'on-secondary-container',
  'tertiary', 'on-tertiary', 'tertiary-container', 'on-tertiary-container',
  'error', 'on-error', 'error-container', 'on-error-container',
  'surface', 'surface-dim', 'surface-bright',
  'surface-container-lowest', 'surface-container-low', 'surface-container',
  'surface-container-high', 'surface-container-highest',
  'on-surface', 'on-surface-variant', 'outline', 'outline-variant',
  'inverse-surface', 'inverse-on-surface', 'inverse-primary',
  'scrim'
];

// 4. contrast_pairs: 6 tohum × açık/koyu için çiftlerin hepsinde contrast ≥ 4.5
const contrastPairs45 = [
  ['on-primary', 'primary'],
  ['on-primary-container', 'primary-container'],
  ['on-secondary', 'secondary'],
  ['on-secondary-container', 'secondary-container'],
  ['on-tertiary', 'tertiary'],
  ['on-tertiary-container', 'tertiary-container'],
  ['on-error', 'error'],
  ['on-error-container', 'error-container'],
  ['on-surface', 'surface'],
  ['on-surface', 'surface-dim'],
  ['on-surface', 'surface-bright'],
  ['on-surface', 'surface-container-lowest'],
  ['on-surface', 'surface-container-low'],
  ['on-surface', 'surface-container'],
  ['on-surface', 'surface-container-high'],
  ['on-surface', 'surface-container-highest'],
  ['on-surface-variant', 'surface'],
  ['on-surface-variant', 'surface-container-highest'],
  ['inverse-on-surface', 'inverse-surface'],
  ['inverse-primary', 'inverse-surface']
];

// 5. nontext_pairs: aynı 12 kombinasyonda outline/surface ≥ 3 ve primary/surface ≥ 3
const nonTextPairs30 = [
  ['outline', 'surface'],
  ['primary', 'surface']
];

for (const seed of SEEDS) {
  for (const dark of [false, true]) {
    const pal = generatePalette(seed.hex, dark);
    for (const role of expectedRoles) {
      assert.ok(role in pal, `Missing role ${role} for ${seed.hex} dark=${dark}`);
      assert.match(pal[role], /^#[0-9a-f]{6}$/, `Invalid hex ${pal[role]} for role ${role}`);
    }
    for (const [fg, bg] of contrastPairs45) {
      const c = contrast(pal[fg], pal[bg]);
      assert.ok(c >= 4.5, `contrast < 4.5 for ${seed.hex} dark=${dark} ${fg}/${bg}: ${c}`);
    }
    for (const [fg, bg] of nonTextPairs30) {
      const c = contrast(pal[fg], pal[bg]);
      assert.ok(c >= 3.0, `nontext contrast < 3.0 for ${seed.hex} dark=${dark} ${fg}/${bg}: ${c}`);
    }
  }

  // 6. reader_colors: light, dark, sepia × 6 tohum için fg/bg ≥ 7 ve link/bg ≥ 4.5
  for (const mode of ['light', 'dark', 'sepia']) {
    const rc = readerColors(mode, seed.hex);
    const fgBg = contrast(rc.fg, rc.bg);
    const linkBg = contrast(rc.link, rc.bg);
    assert.ok(fgBg >= 7.0, `reader_colors fg/bg < 7 for ${seed.hex} ${mode}: ${fgBg}`);
    assert.ok(linkBg >= 4.5, `reader_colors link/bg < 4.5 for ${seed.hex} ${mode}: ${linkBg}`);
  }
}

console.log('theme.test.mjs: tüm testler başarılı');
