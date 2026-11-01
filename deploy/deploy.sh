#!/usr/bin/env sh
set -eu

DIR="$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)"
cd "$DIR"

if [ ! -f .env ]; then
  echo "Missing deploy/.env. Copy deploy/.env.example to deploy/.env and fill it in." >&2
  exit 1
fi

if [ -f preflight.sh ]; then
  echo "Running deployment preflight..."
  sh preflight.sh
fi

if ! command -v docker >/dev/null 2>&1; then
  echo "docker is required." >&2
  exit 1
fi

if docker compose version >/dev/null 2>&1; then
  COMPOSE="docker compose"
elif command -v docker-compose >/dev/null 2>&1; then
  COMPOSE="docker-compose"
else
  echo "Docker Compose v2 (docker compose) is required." >&2
  exit 1
fi

HTTP_PORT="$(sed -n 's/^ORBIT_HTTP_PORT=//p' .env | tail -n 1)"
HTTP_PORT="${HTTP_PORT:-1420}"

PULL_POLICY=""
if [ -f orbit-demo-images.tar ]; then
  echo "Loading offline images from orbit-demo-images.tar..."
  docker load -i orbit-demo-images.tar
  PULL_POLICY="--pull never"
fi

if [ "${ORBIT_BUILD:-0}" = "1" ] || [ ! -f orbit-demo-images.tar ]; then
  echo "Building application images..."
  $COMPOSE --env-file .env -f compose.yml build
  PULL_POLICY=""
fi

echo "Validating Compose configuration..."
$COMPOSE --env-file .env -f compose.yml config --quiet

echo "Starting Orbit stack..."
$COMPOSE --env-file .env -f compose.yml up -d $PULL_POLICY --wait --wait-timeout 300

echo ""
$COMPOSE --env-file .env -f compose.yml ps
echo ""
echo "Orbit should be reachable through:"
echo "  http://127.0.0.1:${HTTP_PORT}/healthz"
echo "Public URL must match ORBIT_PUBLIC_ORIGIN in deploy/.env."
echo "Put an HTTPS reverse proxy in front of the web service before exposing it."
