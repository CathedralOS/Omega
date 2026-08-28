#!/usr/bin/env sh
# Focused fail-closed progress gate for unsupported Beta tokens. A malformed
# statement must terminate promptly instead of repeatedly emitting Alpha while
# its source cursor remains fixed.
set -eu

GATE_DIR=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd -P)
OMEGA_REPO_ROOT=$GATE_DIR
while [ ! -f "$OMEGA_REPO_ROOT/tools/lattice/paths.sh" ]; do
  PARENT=$(dirname -- "$OMEGA_REPO_ROOT")
  [ "$PARENT" != "$OMEGA_REPO_ROOT" ] || exit 2
  OMEGA_REPO_ROOT=$PARENT
done
. "$OMEGA_REPO_ROOT/tools/lattice/paths.sh"
. "$OMEGA_PATH_ALPHA/seed_env.sh"
. "$OMEGA_PATH_BETA_COMPILER/artifact_env.sh"

case "$(uname -sm)" in
  "Darwin arm64") ;;
  *) echo "Beta malformed progress: skipped (requires Darwin arm64)"; exit 0 ;;
esac
command -v python3 >/dev/null 2>&1 || {
  echo "Beta malformed progress: skipped (python3 absent)"
  exit 0
}

T=$(mktemp -d)
trap 'rm -rf "$T"' EXIT
SEED="$OMEGA_PATH_ALPHA/$ALPHA_SEED"
ASM="$OMEGA_PATH_ALPHA_ASSEMBLER/$BETA_SEED"
stamp_beta_compiler "$T/bc.persisted" >/dev/null
"$T/bc.persisted" < "$OMEGA_PATH_BETA_COMPILER/bc.beta" > "$T/bc.self.alpha"
"$ASM" < "$T/bc.self.alpha" > "$T/bc.self.tape"
stamp_seed "$T/bc.self.tape" "$SEED" "$T/bc.self" >/dev/null 2>&1

python3 - "$T/bc.persisted" "$T/bc.self" <<'PY'
import os
import selectors
import subprocess
import sys
import time

LIMIT = 4_096
TIMEOUT = 2.0
CASES = {
    "return-and-and": b"proc main() { return 1 && 2 }\n",
    "guard-and-and": (
        b"proc main() { let a = 1 state s { to done when (a && 1) return 0 } "
        b"state done { return 1 } }\n"
    ),
    "leading-ampersand": b"proc main() { & return 0 }\n",
    "unsupported-or-or": b"proc main() { return 1 || 0 }\n",
}


def observe(executable: str, label: str, source: bytes) -> int:
    process = subprocess.Popen(
        [executable], stdin=subprocess.PIPE, stdout=subprocess.PIPE,
        stderr=subprocess.DEVNULL,
    )
    assert process.stdin is not None and process.stdout is not None
    process.stdin.write(source)
    process.stdin.close()
    selector = selectors.DefaultSelector()
    selector.register(process.stdout, selectors.EVENT_READ)
    output = bytearray()
    started = time.monotonic()
    failure = None
    while True:
        elapsed = time.monotonic() - started
        if elapsed > TIMEOUT:
            failure = f"timeout after {elapsed:.3f}s"
            break
        events = selector.select(0.01)
        for key, _ in events:
            chunk = os.read(key.fd, 4096)
            if chunk:
                output.extend(chunk)
                if len(output) > LIMIT:
                    failure = f"output exceeded {LIMIT} bytes"
                    break
        if failure is not None or process.poll() is not None:
            break
    if failure is not None:
        process.kill()
    status = process.wait()
    selector.close()
    if failure is None:
        while len(output) <= LIMIT:
            chunk = process.stdout.read(4096)
            if not chunk:
                break
            output.extend(chunk)
    elapsed = time.monotonic() - started
    if failure is not None or status != 251 or len(output) > LIMIT:
        raise SystemExit(
            f"Beta malformed progress FAIL - {label}: status={status}, "
            f"bytes={len(output)}, elapsed={elapsed:.3f}s, failure={failure}"
        )
    return len(output)


largest = 0
for executable in sys.argv[1:]:
    realization = os.path.basename(executable)
    for case, source in CASES.items():
        largest = max(largest, observe(executable, f"{realization}/{case}", source))
print(
    "Beta malformed progress: persisted/self-built unsupported-token rejection "
    f"returned 251 within {TIMEOUT:.0f}s and at most {largest} bytes"
)
PY
