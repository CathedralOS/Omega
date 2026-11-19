# Shared host preamble for the tests/<rung>/<gate>/ run.sh gates: with
# GATE_LABEL set to the gate's display name, skip hosts without python3 and
# require a seed-capable host. Sourced after gate_env.sh (and after any
# argument parsing the gate does before checking its host).
command -v python3 >/dev/null 2>&1 || {
    echo "$GATE_LABEL: skipped (python3 absent)"
    exit 0
}
require_seed_execution_host "$GATE_LABEL"
