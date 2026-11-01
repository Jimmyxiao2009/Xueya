#!/usr/bin/env sh
set -eu

DIR="$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)"
cd "$DIR"

errors=0
warns=0

fail() { echo "[FAIL] $*" >&2; errors=$((errors + 1)); }
warn() { echo "[WARN] $*" >&2; warns=$((warns + 1)); }
ok()   { echo "[ OK ] $*"; }

echo "Orbit deployment preflight"
echo "Directory: $DIR"
echo ""

if [ ! -f .env ]; then
  fail "Missing deploy/.env. Copy .env.example to .env and fill it in."
else
  ok "deploy/.env exists"
  set -a
  # shellcheck disable=SC1091
  . ./.env
  set +a
fi

require_value() {
  name="$1"
  value="$(printenv "$name" 2>/dev/null || true)"
  case "$value" in
    ''|changeme|change-me|replace-*|'<'*|your-*|sk-...|https://example.com|https://orbit.example.com)
      fail "$name is missing or still a placeholder"
      ;;
    *)
      ok "$name is set"
      ;;
  esac
}

require_value ORBIT_DB_PASSWORD
require_value VERRUCKT_ACCESS_TOKEN_SECRET
require_value ORBIT_ADMIN_TOKEN
require_value ORBIT_PUBLIC_ORIGIN
require_value ORBIT_AI_BASE_URL
require_value ORBIT_AI_API_KEY
require_value ORBIT_AI_MODEL

case "${ORBIT_PUBLIC_ORIGIN:-}" in
  https://*) ok "ORBIT_PUBLIC_ORIGIN uses HTTPS" ;;
  http://127.0.0.1*|http://localhost*) warn "ORBIT_PUBLIC_ORIGIN is local HTTP; put HTTPS in front before public use" ;;
  *) warn "ORBIT_PUBLIC_ORIGIN should normally be the public HTTPS origin" ;;
esac

if ! command -v docker >/dev/null 2>&1; then
  fail "docker is not installed or not on PATH; on Ubuntu/Debian run deploy/install-docker-ubuntu.sh"
else
  if docker info >/dev/null 2>&1; then
    ok "docker daemon is reachable"
  else
    fail "docker daemon is not reachable (is the service running?)"
  fi
fi

if docker compose version >/dev/null 2>&1; then
  ok "docker compose v2 is available"
elif command -v docker-compose >/dev/null 2>&1; then
  ok "docker-compose v1 is available"
else
  fail "docker compose / docker-compose is not available"
fi

if [ -f orbit-demo-images.tar ]; then
  ok "offline image bundle orbit-demo-images.tar exists"
else
  warn "offline image bundle not found; deploy.sh will try to build images"
fi

if [ -f compose.yml ]; then
  if docker compose -f compose.yml --env-file .env config --quiet >/dev/null 2>&1; then
    ok "Compose configuration parses"
  else
    fail "Compose configuration is invalid; run docker compose config for details"
  fi
else
  fail "compose.yml is missing"
fi

HTTP_PORT="${ORBIT_HTTP_PORT:-1420}"
HTTP_BIND="${ORBIT_HTTP_BIND:-127.0.0.1}"
if command -v ss >/dev/null 2>&1; then
  if ss -ltn 2>/dev/null | awk '{print $4}' | grep -Eq "[:.]${HTTP_PORT}$"; then
    case "$HTTP_BIND" in
      0.0.0.0|'::'|'[::]') fail "port ${HTTP_PORT} is already in use" ;;
      *) warn "port ${HTTP_PORT} is in use; verify it is not another Orbit instance" ;;
    esac
  else
    ok "HTTP port ${HTTP_PORT} is free"
  fi
else
  warn "ss not available; skipped port check for ${HTTP_PORT}"
fi

if command -v df >/dev/null 2>&1; then
  free_kb="$(df -Pk /var/lib/docker 2>/dev/null | awk 'NR==2 {print $4}' || true)"
  if [ -n "${free_kb:-}" ] && [ "$free_kb" -lt 5242880 ] 2>/dev/null; then
    warn "less than 5 GB free for Docker data at /var/lib/docker"
  else
    ok "Docker data disk has sufficient free space"
  fi
fi

echo ""
if [ "$errors" -gt 0 ]; then
  echo "Preflight failed: $errors error(s), $warns warning(s)." >&2
  exit 1
fi
echo "Preflight passed with $warns warning(s)."
