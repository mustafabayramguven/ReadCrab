export default {
  testDir: '.',
  // dosyalar alfabetik sırayla koşar: reader.spec kurulumu yapar, sync.spec onu kullanır
  testMatch: ['reader.spec.mjs', 'sync.spec.mjs'],
  use: {
    baseURL: 'http://127.0.0.1:3435',
    headless: true,
    // reader.spec ağ isteklerini page.route ile taklit eder; service worker bunu atlatmasın
    serviceWorkers: 'block'
  },
  workers: 1,
  timeout: 90000
};
