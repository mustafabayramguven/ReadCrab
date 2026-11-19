import assert from 'node:assert/strict';
import { createRequire } from 'node:module';

const require = createRequire(import.meta.url);
const a = require('./anchor.js');

// Ortak örnek: crates/tui/src/wrap.rs testlerinde de aynı dizi ve sonuç (18) var.
const SAMPLE = '  Merhaba, dünya!­​ \u{1F600} a\tb　c﻿';
assert.equal(a.countText(SAMPLE), 18);

assert.equal(a.countText(''), 0);
assert.equal(a.countText(' \n\t  '), 0);
assert.equal(a.countText('a b'), 2);
assert.equal(a.countText('\u{1F600}\u{1F600}'), 2);

// unitAt: k. anlamlı kod noktasının kod birimi konumu
assert.deepEqual(a.unitAt('  ab', 0), { off: 2, len: 1 });
assert.deepEqual(a.unitAt('a \u{1F600}b', 1), { off: 2, len: 2 });
assert.deepEqual(a.unitAt('a \u{1F600}b', 2), { off: 4, len: 1 });
assert.equal(a.unitAt('ab', 2), null);

assert.equal(a.significant(0x20), false);
assert.equal(a.significant(0xad), false);
assert.equal(a.significant(0x41), true);
assert.equal(a.significant(0xb7), true);

console.log('anchor.test.mjs: tamam');
