#!/usr/bin/env bash
# Demo capture helper — stands up a throwaway Antra demo so the only thing left
# to do is press record. See demo-shot-list.md for the 20-second sequence.
#
# Usage:
#   bash launch/demo-capture.sh            # static demo (no Node needed)
#   DEMO=vite bash launch/demo-capture.sh  # Vite + HMR variant (needs Node)
set -euo pipefail

DOMAIN="${DOMAIN:-myapp.localhost}"
PORT="${PORT:-5173}"
DEMO_DIR="$(mktemp -d /tmp/antra-demo.XXXXXX)"

if ! command -v antra >/dev/null 2>&1; then
  echo "antra not found — install it first:"
  echo "  curl -fsSL https://antra.iifelse.com/install.sh | bash"
  exit 1
fi

cat > "$DEMO_DIR/index.html" <<'HTML'
<!doctype html>
<title>antra demo</title>
<h1>Served over HTTPS at a real URL</h1>
<p>This is myapp.localhost — no port, no /etc/hosts, no warning.</p>
HTML

echo "demo files: $DEMO_DIR"
echo
echo "Press record, then run:"
if [ "${DEMO:-static}" = "vite" ]; then
  echo "  antra run --domain $DOMAIN -- npm create vite@latest -- --template vanilla"
  echo "  # (cd into the created project, then:)"
  echo "  antra run --domain $DOMAIN -- npm run dev"
else
  echo "  antra run --domain $DOMAIN -- python3 -m http.server $PORT --directory '$DEMO_DIR'"
fi
echo
echo "Open https://$DOMAIN when the URL line appears."
