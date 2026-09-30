#!/usr/bin/env bash
# Check the live landing site against this checkout.
#
#   .github/scripts/check-landing.sh [VERSION]
#
# Always checks the live domain, never a deployment URL: a deploy from the
# wrong branch creates a preview URL that looks fine while production still
# serves the old files (see "Releasing" in NEXT-SESSION.md). With VERSION,
# also checks the pin example on the homepage names it.
set -u
SITE="${ANTRA_SITE:-https://antra.iifelse.com}"  # override only to test this script
WANT="${1:-}"
fail=0
bad() { echo "::error::$1"; fail=1; }
ok()  { echo "ok  $1"; }

# The served installer must be byte-identical to this commit's copy. Retry
# for a couple of minutes: a fresh deploy takes a moment to reach the edge.
served="$(mktemp)"
for i in $(seq 1 12); do
  if curl -fsS "$SITE/install.sh" -o "$served" && cmp -s "$served" landing/install.sh; then
    break
  fi
  [ "$i" -lt 12 ] && sleep 10
done
if cmp -s "$served" landing/install.sh; then
  ok "/install.sh matches landing/install.sh"
else
  bad "/install.sh on $SITE differs from landing/install.sh — the site is not deployed from this commit"
fi

ctype="$(curl -fsSI "$SITE/install.sh" | tr -d '\r' | awk -F': ' 'tolower($1)=="content-type"{print $2}')"
case "$ctype" in
  text/plain*) ok "/install.sh Content-Type: $ctype" ;;
  *) bad "/install.sh Content-Type is '$ctype', expected text/plain" ;;
esac

if [ -n "$WANT" ]; then
  if curl -fsS "$SITE/" | grep -q "ANTRA_VERSION=v${WANT#v} bash"; then
    ok "homepage pin example names v${WANT#v}"
  else
    bad "homepage pin example does not name v${WANT#v}"
  fi
fi

code="$(curl -s -o /dev/null -w '%{http_code}' "$SITE/this-page-does-not-exist")"
[ "$code" = 404 ] && ok "unknown path returns 404" || bad "unknown path returned $code, expected 404"

headers="$(curl -fsSI "$SITE/" | tr -d '\r' | tr 'A-Z' 'a-z')"
for h in strict-transport-security content-security-policy x-frame-options x-content-type-options referrer-policy permissions-policy; do
  grep -q "^$h:" <<<"$headers" && ok "header $h" || bad "missing header $h"
done

exit "$fail"
