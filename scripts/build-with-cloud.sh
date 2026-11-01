#!/usr/bin/env bash
set -euo pipefail

usage() {
  echo "usage: $0 <cloud-url> [--profile Release|Debug] [--bundles nsis|msi] [--skip-tests] [--no-bundle] [--allow-local-http]" >&2
  exit 1
}

if [ "$#" -lt 1 ]; then
  usage
fi

cloud_url="${1%/}"
shift

profile="Release"
skip_tests=0
no_bundle=0
bundles="nsis"
allow_local_http=0

while [ "$#" -gt 0 ]; do
  case "$1" in
    --profile)
      [ "$#" -ge 2 ] || usage
      profile="$2"
      shift 2
      ;;
    --skip-tests)
      skip_tests=1
      shift
      ;;
    --no-bundle)
      no_bundle=1
      shift
      ;;
    --bundles)
      [ "$#" -ge 2 ] || usage
      bundles="$2"
      shift 2
      ;;
    --allow-local-http)
      allow_local_http=1
      shift
      ;;
    *)
      usage
      ;;
  esac
done

fail() {
  echo "[FAIL] $1" >&2
  exit 1
}

[ -n "$cloud_url" ] || fail "Cloud URL is required."
if [[ "$cloud_url" != https://* ]]; then
  if [[ "$allow_local_http" -eq 1 && "$cloud_url" =~ ^http://(localhost|127\.0\.0\.1)(:[0-9]+)?$ ]]; then
    :
  else
    fail "Production Cloud URL must use https://. Use --allow-local-http only for local development endpoints."
  fi
fi

export VERRUCKT_DEFAULT_CLOUD_URL="$cloud_url"

echo
echo "Orbit / 学涯 production build"
echo "Cloud endpoint: $cloud_url"
echo "Mode: $profile"
echo "Tests: $([ "$skip_tests" -eq 0 ] && echo true || echo false)"
echo "Bundle: $([ "$no_bundle" -eq 0 ] && echo true || echo false)"
echo

if [ "$skip_tests" -eq 0 ]; then
  echo "==> npm run typecheck"
  npm run typecheck
  echo "==> npm run lint"
  npm run lint
fi

echo "==> npm run build"
npm run build

tauri_args=("tauri" "build")
if [ "$profile" = "Debug" ]; then
  tauri_args+=("--debug")
fi
if [ "$no_bundle" -eq 1 ]; then
  tauri_args+=("--no-bundle")
elif [ -n "$bundles" ]; then
  tauri_args+=("--bundles" "$bundles")
fi

echo "==> npx ${tauri_args[*]}"
npx "${tauri_args[@]}"

profile_dir="release"
if [ "$profile" = "Debug" ]; then
  profile_dir="debug"
fi

bundle_dir="src-tauri/target/$profile_dir/bundle"
echo
echo "Build artifacts:"
installer=""
if [ -d "$bundle_dir" ]; then
  installer="$(find "$bundle_dir" -type f \( -name '*.exe' -o -name '*.msi' \) -print | head -n 1 || true)"
fi
if [ -n "$installer" ]; then
  echo "Installer: $installer"
else
  echo "Installer: not produced (use without --no-bundle)"
fi

executable="$(find "src-tauri/target/$profile_dir" -maxdepth 1 -type f -name '*.exe' ! -name '*setup*' -print | head -n 1 || true)"
if [ -n "$executable" ]; then
  echo "Executable: $executable"
else
  echo "Executable: not found"
fi
echo "Cloud endpoint embedded: $cloud_url"
echo "BUILD PASS"
