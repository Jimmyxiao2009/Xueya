#!/usr/bin/env sh
# Install Docker Engine + Compose plugin on a fresh Ubuntu/Debian server.
# Run as root:  sh install-docker-ubuntu.sh
set -eu

if [ "$(id -u)" -ne 0 ]; then
  echo "This script must run as root (use sudo)." >&2
  exit 1
fi

if command -v docker >/dev/null 2>&1 && docker compose version >/dev/null 2>&1; then
  echo "Docker Engine and Compose plugin are already installed."
  docker --version
  docker compose version
  exit 0
fi

if [ ! -f /etc/os-release ]; then
  echo "Cannot detect OS: /etc/os-release is missing." >&2
  exit 1
fi
# shellcheck disable=SC1091
. /etc/os-release

case "${ID:-}" in
  ubuntu|debian) ;;
  *)
    echo "Unsupported distribution: ${ID:-unknown}. Install Docker manually and rerun deploy/preflight.sh." >&2
    exit 1
    ;;
esac

if ! command -v curl >/dev/null 2>&1 || ! command -v apt-get >/dev/null 2>&1; then
  echo "This script expects apt-get and curl. Install them first." >&2
  exit 1
fi

echo "Installing Docker for $ID $VERSION_CODENAME..."
apt-get update
apt-get install -y ca-certificates curl gnupg

install -m 0755 -d /etc/apt/keyrings
curl -fsSL "https://download.docker.com/linux/${ID}/gpg" -o /etc/apt/keyrings/docker.asc
chmod a+r /etc/apt/keyrings/docker.asc

echo "deb [arch=$(dpkg --print-architecture) signed-by=/etc/apt/keyrings/docker.asc] https://download.docker.com/linux/${ID} ${VERSION_CODENAME} stable" \
  > /etc/apt/sources.list.d/docker.list

apt-get update
apt-get install -y docker-ce docker-ce-cli containerd.io docker-buildx-plugin docker-compose-plugin

systemctl enable --now docker

echo ""
echo "Docker installed:"
docker --version
docker compose version
echo "You can now run deploy/preflight.sh and deploy/deploy.sh."
