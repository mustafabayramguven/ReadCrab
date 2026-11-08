export default {
  testDir: '.',
  testMatch: 'reader.spec.mjs',
  use: {
    baseURL: 'http://127.0.0.1:3435',
    headless: true
  },
  workers: 1,
  timeout: 90000
};
