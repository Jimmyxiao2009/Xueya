#!/usr/bin/env sh
# One-command server bootstrap: Docker (if needed) -> .env -> preflight -> deploy.
# Run from the repository or an extracted orbit-deploy release bundle:
#   sh bootstrap.sh
set -eu

DIR="$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)"
cd "$DIR"

if ! command -v docker >/dev/null 2>&1 || ! docker compose version >/dev/null 2>&1; then
  if [ "$(id -u)" -ne 0 ]; then
    echo "Docker is missing. Re-run this script as root, or install Docker manually." >&2
    exit 1
  fi
  if [ ! -f install-docker-ubuntu.sh ]; then
    echo "install-docker-ubuntu.sh is missing; install Docker manually." >&2
    exit 1
  fi
  sh install-docker-ubuntu.sh
fi

if [ ! -f .env ]; then
  if [ ! -f init-env.sh ]; then
    echo "init-env.sh is missing and .env does not exist." >&2
    exit 1
  fi
  sh init-env.sh
fi

if [ -f preflight.sh ]; then
  sh preflight.sh
fi

sh deploy.sh
