# Shared run.sh preamble for the tests/bootstrap/*/ gates that accept only an
# optional --identity flag. The caller sets GATE_DIR to its own directory,
# GATE_LABEL to the gate's display name, and GATE_ENVS to the rung
# environments; sourcing this file resolves the repository root and rung
# environments, parses --identity, skips hosts without python3, and requires
# a seed-capable host for the default run.
. "$GATE_DIR/../../gate_env.sh"

IDENTITY_ONLY=0
if [ "$#" -gt 1 ] || { [ "$#" -eq 1 ] && [ "$1" != --identity ]; }; then
    echo "usage: $0 [--identity]" >&2
    exit 2
fi
if [ "$#" -eq 1 ]; then
    IDENTITY_ONLY=1
fi

. "$GATE_DIR/../../gate_host.sh"
