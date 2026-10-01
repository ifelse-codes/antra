#!/usr/bin/env bash
# install.sh must not print raw colour escapes at the user.
#
# Reported 2026-09-07 (tests/user-test-2026-09-07-1430.md) and left unfixed
# until 2026-10-01: install.sh defines its colours as '\033[1m' literals, and
# ten lines printed them with plain `echo`, which does not interpret
# backslash escapes. The user saw the text "\033[1mTrusting the CA\033[0m
# means HTTPS works with zero browser warnings" on first contact with the
# product — the installer's own quick-start block, the one thing every new
# user reads.
#
# This sources the real install.sh and calls the real functions, so it fails if
# the escapes ever leak again. It asserts the escape is *interpreted* (a real
# ESC byte reaches the terminal), not merely that the literal text is gone:
# stripping the escapes outright would also pass a weaker check and would
# leave the output unstyled.
#
# The user-facing paths are exercised directly rather than by running a full
# install, which would need a network fetch of a release tarball. ask_trust is
# pure output plus a prompt, and the prompt read is fed from /dev/null so it
# takes its non-interactive path. The quick-start block is not a function — it
# is inline at the tail of main(), after the download — so those lines are
# checked by extracting them from main()'s own source and evaluating just
# those. That is why the source is kept: `sed '$ d'` gives us both.

set -uo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
INSTALLER="$REPO_ROOT/install.sh"

pass=0
fail=0
skip=0

# These are prefixed because install.sh is sourced below and defines its own
# `ok`, `info` and `warn` — an unprefixed `ok()` here is silently replaced by
# the installer's, which prints a checkmark and never touches a counter. That
# bug made this file report "0 pass" while printing seven green ticks.
t_ok()  { printf '  \033[0;32mok\033[0m   %s\n' "$1"; pass=$((pass + 1)); }
t_bad() { printf '  \033[0;31mFAIL\033[0m %s\n' "$1"; fail=$((fail + 1)); }
t_skip(){ printf '  \033[0;33mskip\033[0m %s\n' "$1"; skip=$((skip + 1)); }

# Sourcing install.sh must not run main: it ends with `main "$@"`. Strip that
# one line so the functions can be called directly.
tmp_src="$(mktemp "${TMPDIR:-/tmp}/antra-inst.XXXXXX")"
sed '$ d' "$INSTALLER" > "$tmp_src"

# shellcheck source=/dev/null
. "$tmp_src" >/dev/null 2>&1 || { echo "cannot source install.sh"; exit 1; }

# The tail of main(): the quick-start block and NEXT STEPS, from the
# "Quick start:" line to the Docs line. Those lines are inline in main(), not a
# function of their own, so they are lifted out of main()'s source and
# evaluated as a function body — which runs them for real against the sourced
# helpers, rather than grepping the text and trusting it.
quick_start_fn() {
    # main() sets these before the block it contains; under `set -u` an unset
    # one aborts the eval, which silently truncated this function's output.
    local REPO="ifelse-codes/antra" installed_path="/bin/true"
    # eval, not just sed: printing the lifted lines would make every check
    # below pass on the *source text* rather than on what the user sees, which
    # is the exact "green while proving nothing" trap AGENT.md warns about.
    # ${BOLD} in the source is not \033[ in the output, so a grep of the
    # unevaluated lines reports "no raw escape" no matter what say() does.
    eval "$(sed -n '/${BOLD}Quick start/,/Docs: https/p' "$tmp_src" | sed '$ d')"
}

# ── 1. ask_trust prints no raw escape ────────────────────────────────────────
# The heading and the three explanation lines are styled. Darwin vs Linux only
# changes one line's wording, so both are checked.
for uname_s in Darwin Linux; do
    out="$( uname() { echo "$uname_s"; }; ask_trust /bin/true </dev/null 2>&1 )"
    if printf '%s' "$out" | grep -q '\\033\['; then
        t_bad "ask_trust ($uname_s) prints a literal \\033[ escape"
        printf '%s' "$out" | grep -o '\\033\[[0-9;]*m' | sort -u | sed 's/^/         saw: /'
    else
        t_ok "ask_trust ($uname_s) prints no literal \\033[ escape"
    fi
done

# ── 2. The styled lines are actually styled ──────────────────────────────────
# Guards against "fix" being satisfied by deleting the colour variables. A real
# ESC byte (0x1b) must reach stdout.
esc_count="$( uname() { echo Darwin; }; ask_trust /bin/true </dev/null 2>&1 | tr -cd '\033' | wc -c | tr -d ' ')"
if [ "${esc_count:-0}" -gt 0 ]; then
    t_ok "ask_trust emits $esc_count real ESC bytes (styling intact)"
else
    t_bad "ask_trust emitted no ESC bytes — styling was stripped, not interpreted"
fi

# ── 3. The quick-start block prints no raw escape, and still has content ─────
out="$( quick_start_fn </dev/null 2>&1 )"
if printf '%s' "$out" | grep -q '\\033\['; then
    t_bad "quick-start block prints a literal \\033[ escape"
else
    t_ok "quick-start block prints no literal \\033[ escape"
fi
if printf '%s' "$out" | grep -q 'Quick start'; then
    t_ok "quick-start block still prints its content"
else
    t_bad "quick-start block lost its content"
fi

# ── 4. No unexpanded shell variable leaks ────────────────────────────────────
# A second failure mode of the same bug class: a line written as
# `echo "  ${BOLD}..."` inside single quotes, or a `say` called with the
# variable name rather than its value, prints "$BOLD" to the user.
for fn in ask_trust quick_start_fn; do
    case "$fn" in
        ask_trust)      o="$( uname() { echo Darwin; }; ask_trust /bin/true </dev/null 2>&1 )" ;;
        quick_start_fn) o="$( quick_start_fn </dev/null 2>&1 )" ;;
    esac
    if printf '%s' "$o" | grep -qE '\$\{?(BOLD|DIM|GREEN|YELLOW|CYAN|RED|RESET)\}?'; then
        t_bad "$fn leaks an unexpanded colour variable"
    else
        t_ok "$fn leaks no unexpanded colour variable"
    fi
done

rm -f "$tmp_src"

# ── 5. landing/install.sh stays byte-identical ───────────────────────────────
# Merging deploys landing/ to production, so a divergence here means the live
# site serves an installer that is not the one in this repo.
if diff -q "$REPO_ROOT/install.sh" "$REPO_ROOT/landing/install.sh" >/dev/null 2>&1; then
    t_ok "landing/install.sh is byte-identical to install.sh"
elif [ -f "$REPO_ROOT/landing/install.sh" ]; then
    t_bad "landing/install.sh has drifted from install.sh"
else
    t_skip "landing/install.sh not present"
fi

echo
printf 'installer_output: %d pass / %d fail / %d skip\n' "$pass" "$fail" "$skip"
[ "$fail" -eq 0 ]
