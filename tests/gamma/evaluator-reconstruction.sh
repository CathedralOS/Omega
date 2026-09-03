#!/usr/bin/env sh
set -eu

GATE_DIR=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd -P)
OMEGA_REPO_ROOT=$(CDPATH= cd -- "$GATE_DIR/../.." && pwd -P)
. "$OMEGA_REPO_ROOT/tools/bootstrap/paths.sh"
. "$OMEGA_REPO_ROOT/tools/bootstrap/beta/artifact_env.sh"

command -v python3 >/dev/null 2>&1 || {
    echo "Gamma evaluator reconstruction: skipped (python3 absent)"
    exit 0
}

TMP=$(mktemp -d)
trap 'rm -rf -- "$TMP"' EXIT HUP INT TERM
RECONSTRUCTOR="$GATE_DIR/evaluator_reconstructor.gamma"

materialize_beta_compiler "$TMP/beta-compiler" >/dev/null
"$TMP/beta-compiler" < "$OMEGA_PATH_CONCATENATIVE_GAMMA_EVALUATOR_SOURCE" > "$TMP/expected.tape"
stamp_seed "$TMP/expected.tape" "$OMEGA_PATH_ALPHA/$ALPHA_SEED" "$TMP/evaluator" >/dev/null

RECONSTRUCTOR=$RECONSTRUCTOR SUBJECT=$OMEGA_PATH_CONCATENATIVE_GAMMA_EVALUATOR_SOURCE \
    python3 -c '
import os, struct, sys
from pathlib import Path
program = Path(os.environ["RECONSTRUCTOR"]).read_bytes()
subject = Path(os.environ["SUBJECT"]).read_bytes()
sys.stdout.buffer.write(struct.pack("<I", len(program)) + program + subject)
' > "$TMP/request"

"$TMP/evaluator" < "$TMP/request" > "$TMP/actual.tape"
cmp "$TMP/expected.tape" "$TMP/actual.tape"

RECONSTRUCTOR=$RECONSTRUCTOR SUBJECT=$OMEGA_PATH_CONCATENATIVE_GAMMA_EVALUATOR_SOURCE \
    python3 -c '
import os, re, struct, sys
from pathlib import Path
program = Path(os.environ["RECONSTRUCTOR"]).read_bytes()
subject, count = re.subn(
    rb"(?m)^0x([0-9a-f]+):",
    lambda match: f"0x{int(match.group(1), 16) + 1:x}:".encode(),
    Path(os.environ["SUBJECT"]).read_bytes(),
    count=1,
)
if count != 1:
    raise SystemExit("missing address assertion")
sys.stdout.buffer.write(struct.pack("<I", len(program)) + program + subject)
' > "$TMP/bad-request"
set +e
"$TMP/evaluator" < "$TMP/bad-request" > "$TMP/rejected-prefix"
STATUS=$?
set -e
[ "$STATUS" -eq 2 ] || {
    echo "Gamma evaluator reconstruction: bad address exited $STATUS, expected 2" >&2
    exit 1
}

LINES=$(RECONSTRUCTOR=$RECONSTRUCTOR python3 -c '
import os
from pathlib import Path
print(len(Path(os.environ["RECONSTRUCTOR"]).read_text().splitlines()))
')
echo "Gamma evaluator reconstruction: $LINES-line Gamma program reproduced exact $(wc -c < "$TMP/actual.tape" | tr -d ' ')-byte evaluator tape"