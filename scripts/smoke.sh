#!/usr/bin/env bash
set -euo pipefail

IMG="${1:?kullanım: smoke.sh <imaj>}"
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
BASE="http://127.0.0.1:3435"
books="$(mktemp -d)"
jar="$(mktemp)"
# konteyner 10001 kullanıcısı okuyabilsin diye
chmod 755 "$books"

cleanup() {
  docker rm -f smoke >/dev/null 2>&1 || true
  docker volume rm smoke-data >/dev/null 2>&1 || true
  rm -rf "$books" "$jar"
}
trap cleanup EXIT

fail() {
  echo "SMOKE HATA: $*" >&2
  docker logs smoke >&2 || true
  exit 1
}

docker volume create smoke-data >/dev/null
cp "$ROOT/tests/fixtures/rar5_stored.rar" "$books/"
chmod 644 "$books/rar5_stored.rar"
docker run -d --name smoke -p 3435:3435 -v smoke-data:/data -v "$books":/books:ro "$IMG" >/dev/null

# 3: /health en çok 30 sn içinde "ok" dönmeli
ok=0
for _ in $(seq 1 30); do
  if [ "$(curl -fsS "$BASE/health" 2>/dev/null || true)" = "ok" ]; then ok=1; break; fi
  sleep 1
done
[ "$ok" = 1 ] || fail "/health 30 sn içinde ok dönmedi"

# 4: kök sayfa
body="$(curl -fsS "$BASE/")" || fail "GET / başarısız"
case "$body" in *kavita-rs*) ;; *) fail "GET / gövdesinde kavita-rs yok" ;; esac

# 5: kimliksiz /api/me 401
code="$(curl -s -o /dev/null -w '%{http_code}' "$BASE/api/me")"
[ "$code" = 401 ] || fail "GET /api/me çerezsiz $code (beklenen 401)"

# 6: ilk kurulum 201, aynı çerezle /api/me 200 (düz HTTP'de oturum çalışır)
code="$(curl -s -o /dev/null -w '%{http_code}' -c "$jar" -H 'Content-Type: application/json' \
  -d '{"username":"smoke","password":"smoke-pass-123"}' "$BASE/api/setup")"
[ "$code" = 201 ] || fail "POST /api/setup $code (beklenen 201)"
code="$(curl -s -o /dev/null -w '%{http_code}' -b "$jar" "$BASE/api/me")"
[ "$code" = 200 ] || fail "çerezli GET /api/me $code (beklenen 200)"

# 7: ikinci kurulum 409
code="$(curl -s -o /dev/null -w '%{http_code}' -H 'Content-Type: application/json' \
  -d '{"username":"smoke2","password":"smoke-pass-123"}' "$BASE/api/setup")"
[ "$code" = 409 ] || fail "ikinci POST /api/setup $code (beklenen 409)"

# 8: root değil, bsdtar var, RAR5 fixture'ı listelenebiliyor
uid="$(docker exec smoke id -u)"
[ "$uid" != 0 ] || fail "konteyner root olarak çalışıyor"
docker exec smoke bsdtar --version >/dev/null || fail "bsdtar çalışmıyor"
rar_list="$(docker exec smoke bsdtar -tf /books/rar5_stored.rar)" || fail "RAR5 listelenemedi"
[ -n "$rar_list" ] || fail "RAR5 listesi boş"

# 9: healthcheck en çok 60 sn içinde healthy
health=""
for _ in $(seq 1 60); do
  health="$(docker inspect --format '{{.State.Health.Status}}' smoke 2>/dev/null || true)"
  [ "$health" = healthy ] && break
  sleep 1
done
[ "$health" = healthy ] || fail "healthcheck healthy olmadı (son: $health)"

echo "smoke: tamam"
