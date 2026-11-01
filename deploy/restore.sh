#!/usr/bin/env sh
set -eu

CALLER_DIR="$(pwd)"
DIR="$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)"
cd "$DIR"

if [ ! -f .env ]; then
  echo "Missing deploy/.env." >&2
  exit 1
fi

BACKUP="${1:-}"
if [ -z "$BACKUP" ]; then
  BACKUP="$(find "$DIR/backups" -mindepth 1 -maxdepth 1 -type d 2>/dev/null | sort | tail -n 1)"
elif [ "${BACKUP#/}" = "$BACKUP" ]; then
  BACKUP="$CALLER_DIR/$BACKUP"
fi
if [ -z "$BACKUP" ] || [ ! -d "$BACKUP" ]; then
  echo "Usage: sh deploy/restore.sh deploy/backups/<timestamp>" >&2
  exit 1
fi
if [ ! -f "$BACKUP/postgres.dump" ] || [ ! -f "$BACKUP/learning.tgz" ]; then
  echo "Backup is missing postgres.dump or learning.tgz: $BACKUP" >&2
  exit 1
fi

LEARNING_VOLUME="$(sed -n 's/^learning_volume=//p' "$BACKUP/metadata.txt" 2>/dev/null | tail -n 1)"
if [ -z "$LEARNING_VOLUME" ]; then
  echo "Backup metadata does not contain learning_volume. Restore aborted." >&2
  exit 1
fi

if [ "${ORBIT_RESTORE_YES:-0}" != "1" ]; then
  echo "This will overwrite the current PostgreSQL data and learning volume:"
  echo "  backup:          $BACKUP"
  echo "  learning volume: $LEARNING_VOLUME"
  echo "Set ORBIT_RESTORE_YES=1 to continue."
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

echo "Starting PostgreSQL..."
$COMPOSE --env-file .env -f compose.yml up -d --wait --wait-timeout 180 postgres

echo "Stopping web and cloud to prevent writes during restore..."
$COMPOSE --env-file .env -f compose.yml stop web cloud >/dev/null 2>&1 || true

echo "Restoring PostgreSQL..."
cat "$BACKUP/postgres.dump" | \
  $COMPOSE --env-file .env -f compose.yml exec -T postgres pg_restore --clean --if-exists --no-owner -U orbit -d orbit

echo "Restoring learning volume $LEARNING_VOLUME..."
docker run --rm \
  -v "$LEARNING_VOLUME:/data" \
  -v "$BACKUP:/backup:ro" \
  postgres:16-alpine \
  sh -c 'find /data -mindepth 1 -maxdepth 1 -exec rm -rf {} \; && tar xzf /backup/learning.tgz -C /data'

echo "Starting full stack..."
$COMPOSE --env-file .env -f compose.yml up -d --wait --wait-timeout 300

echo "Restore completed."
