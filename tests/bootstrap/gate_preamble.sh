# Shared run.sh preamble for the tests/bootstrap/*/ gates that accept only an
# optional --identity flag. The caller sets GATE_DIR to its own directory and
# GATE_LABEL to the gate's display name; sourcing this file resolves the
# repository root and rung environments, parses --identity, skips hosts
# without python3, and requires a seed-capable host for the default run.
. "$GATE_DIR/../gate_env.sh"

IDENTITY_ONLY=0
if [ "$#" -gt 1 ] || { [ "$#" -eq 1 ] && [ "$1" != --identity ]; }; then
    echo "usage: $0 [--identity]" >&2
    exit 2
fi
if [ "$#" -eq 1 ]; then
    IDENTITY_ONLY=1
fi

command -v python3 >/dev/null 2>&1 || {
    echo "$GATE_LABEL: skipped (python3 absent)"
    exit 0
}

# --identity validates every bound identity and assembles both byte streams
# without executing the seed, so any Python-3 host (including Linux, or a
# Windows host before the multi-hour run) can check the gate's whole
# non-executing surface. The default run still requires a seed host.
if [ "$IDENTITY_ONLY" = 0 ]; then
    require_seed_execution_host "$GATE_LABEL"
fi
