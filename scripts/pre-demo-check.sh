#!/usr/bin/env bash
set -euo pipefail

usage() {
  echo "usage: $0 <cloud-url> [--allow-local-http]" >&2
  exit 1
}

[ "$#" -ge 1 ] || usage
cloud_url="${1%/}"
shift
allow_local_http=0
if [ "${1:-}" = "--allow-local-http" ]; then
  allow_local_http=1
fi

email="${VERRUCKT_DEMO_EMAIL:-}"
password="${VERRUCKT_DEMO_PASSWORD:-}"
if [ -z "$email" ] || [ -z "$password" ]; then
  echo "Set VERRUCKT_DEMO_EMAIL and VERRUCKT_DEMO_PASSWORD." >&2
  exit 1
fi

if [[ "$cloud_url" != https://* ]]; then
  if [[ "$allow_local_http" -eq 1 && "$cloud_url" =~ ^http://(localhost|127\.0\.0\.1)(:[0-9]+)?$ ]]; then
    :
  else
    echo "[FAIL] Cloud reachable" >&2
    echo "Reason: Cloud URL must use https:// (or pass --allow-local-http)." >&2
    exit 1
  fi
fi

json_field() {
  python3 -c "import sys,json; print(json.load(sys.stdin).get('$1',''))"
}

echo "VERRUCKT PRE-DEMO CHECK"
echo "Cloud: $cloud_url"
echo

if ! health="$(curl -fsS "$cloud_url/healthz")"; then
  echo "[FAIL] Cloud reachable"; echo "Reason: health request failed"; exit 1
fi
echo "[PASS] Cloud reachable"
if [ "$(printf '%s' "$health" | json_field status)" != "ok" ]; then
  echo "[FAIL] Cloud health"; exit 1
fi
echo "[PASS] Cloud health"

login_body="$(python3 - "$email" "$password" <<'PY'
import json,sys
print(json.dumps({"email":sys.argv[1],"password":sys.argv[2],"device":{"name":"Pre-demo check","platform":"linux","app_version":"0.1.0"}}))
PY
)"
if ! login="$(curl -fsS -X POST "$cloud_url/api/v1/auth/login" -H 'Content-Type: application/json' -d "$login_body")"; then
  echo "[FAIL] Account login"; exit 1
fi
token="$(printf '%s' "$login" | python3 -c 'import sys,json; print(json.load(sys.stdin).get("access_token",""))')"
if [ -z "$token" ]; then
  echo "[FAIL] Account login"; echo "Reason: no access token"; exit 1
fi
echo "[PASS] Account login"

auth=(-H "Authorization: Bearer $token")
if ! entitlements="$(curl -fsS "${auth[@]}" "$cloud_url/api/v1/me/entitlements")"; then
  echo "[FAIL] Entitlement"; exit 1
fi
if [ "$(printf '%s' "$entitlements" | python3 -c 'import sys,json; print(str(json.load(sys.stdin)["features"]["official_ai"]).lower())')" != "true" ]; then
  echo "[FAIL] Official AI entitlement"; exit 1
fi
echo "[PASS] Official AI entitlement"
if [ "$(printf '%s' "$entitlements" | python3 -c 'import sys,json; print(str(json.load(sys.stdin)["features"]["cloud_sync"]).lower())')" != "true" ]; then
  echo "[FAIL] Cloud Sync entitlement"; exit 1
fi
echo "[PASS] Cloud Sync entitlement"

if ! usage="$(curl -fsS "${auth[@]}" "$cloud_url/api/v1/ai/usage")"; then
  echo "[FAIL] AI quota"; exit 1
fi
remaining="$(printf '%s' "$usage" | python3 -c 'import sys,json; print(json.load(sys.stdin).get("credits_remaining",0))')"
limit="$(printf '%s' "$usage" | python3 -c 'import sys,json; print(json.load(sys.stdin).get("credits_limit",0))')"
if [ "$limit" -le 0 ] || [ "$remaining" -le 0 ]; then
  echo "[FAIL] AI quota"; echo "Reason: limit=$limit remaining=$remaining"; exit 1
fi
echo "[PASS] AI quota"

chat_body='{"model":"verruckt-fast","messages":[{"role":"system","content":"You are 学涯, a Socratic tutor. Return ONLY JSON with message and suggestedAction."},{"role":"user","content":"用一个问题开始讲解：为什么天空是蓝色的？"}],"max_tokens":256,"stream":false}'
if ! chat="$(curl -fsS -X POST "${auth[@]}" -H 'Content-Type: application/json' -d "$chat_body" "$cloud_url/v1/chat/completions")"; then
  echo "[FAIL] AI Gateway"; exit 1
fi
content="$(printf '%s' "$chat" | python3 -c 'import sys,json; print(json.load(sys.stdin)["choices"][0]["message"]["content"])')"
if [ -z "$content" ]; then
  echo "[FAIL] Upstream model"; exit 1
fi
echo "[PASS] AI Gateway"
echo "[PASS] Upstream model"

if printf '%s' "$content" | python3 -c 'import sys,json; d=json.load(sys.stdin); assert d.get("message") and d.get("suggestedAction")' >/dev/null 2>&1; then
  echo "[PASS] Structured teaching response"
else
  echo "[WARN] Structured teaching response: upstream returned plain text"
fi

if curl -fsS "${auth[@]}" "$cloud_url/api/v1/sync/pull?limit=1" >/dev/null; then
  echo "[PASS] Cloud Sync protocol"
else
  echo "[FAIL] Cloud Sync protocol"; exit 1
fi

echo
echo "READY FOR DEMO"
