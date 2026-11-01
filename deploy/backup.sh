#!/usr/bin/env sh
set -eu

DIR="$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)"
cd "$DIR"

if [ ! -f .env ]; then
  echo "Missing deploy/.env." >&2
  exit 1
fi

if docker compose version >/dev/null 2>&1; then
  COMPOSE="docker compose"
elif command -v docker-compose >/dev/null 2>&1; then
  COMPOSE="docker-compose"
else
  echo "Docker Compose v2 is required." >&2
  exit 1
fi

STAMP="$(date -u +%Y%m%d-%H%M%S)"
DEST="backups/$STAMP"
mkdir -p "$DEST"

echo "Backing up PostgreSQL..."
$COMPOSE --env-file .env -f compose.yml exec -T postgres pg_dump -Fc -U orbit -d orbit > "$DEST/postgres.dump"

WEB_ID="$($COMPOSE --env-file .env -f compose.yml ps -q web)"
if [ -z "$WEB_ID" ]; then
  echo "web container is not running; cannot locate learning volume." >&2
  exit 1
fi

LEARNING_VOLUME="$(docker inspect -f '{{ range .Mounts }}{{ if eq .Destination "/data" }}{{ .Name }}{{ end }}{{ end }}' "$WEB_ID")"
if [ -z "$LEARNING_VOLUME" ]; then
  echo "learning volume not found on web container." >&2
  exit 1
fi

echo "Backing up learning volume $LEARNING_VOLUME..."
docker run --rm -v "$LEARNING_VOLUME:/data:ro" -v "$DIR/$DEST:/backup" postgres:16-alpine \
  tar czf /backup/learning.tgz -C /data .

printf 'learning_volume=%s\ncreated_at=%s\n' "$LEARNING_VOLUME" "$STAMP" > "$DEST/metadata.txt"

echo ""
echo "Backup written to $DIR/$DEST"
echo "Copy this directory to durable storage. It contains account/learning data and may contain personal data."
