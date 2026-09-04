#!/usr/bin/env sh
set -eu

TEST_DIR=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd -P)
OMEGA_REPO_ROOT=$(CDPATH= cd -- "$TEST_DIR/../.." && pwd -P)
export OMEGA_REPO_ROOT
. "$OMEGA_REPO_ROOT/tools/bootstrap/paths.sh"
. "$OMEGA_REPO_ROOT/tools/bootstrap/gamma/evaluator_env.sh"

TMP=$(mktemp -d)
trap 'rm -rf -- "$TMP"' EXIT HUP INT TERM
INVOKE="$OMEGA_REPO_ROOT/tools/bootstrap/gamma/invoke.py"
SOURCE="$OMEGA_PATH_DELTA_COMPILER_SOURCE"
MANIFEST="$OMEGA_PATH_DELTA_COMPILER/delta_compiler.composed"
CUSTOMER="$OMEGA_REPO_ROOT/tests/delta/staged-compiler/nullary_match.delta"
EXPECTED="$OMEGA_REPO_ROOT/tests/delta/staged-compiler/nullary_match.gamma"

materialize_gamma_evaluator "$TMP/evaluator" >/dev/null

SOURCE="$SOURCE" EVALUATOR_TAPE="$OMEGA_PATH_GAMMA_EVALUATOR_TAPE" \
    MANIFEST="$MANIFEST" python3 - <<'PY'
import hashlib
import os
from pathlib import Path

source = Path(os.environ["SOURCE"]).read_bytes()
evaluator = Path(os.environ["EVALUATOR_TAPE"]).read_bytes()
expected = (
    "GammaComposedV1\n"
    f"evaluator-sha256 {hashlib.sha256(evaluator).hexdigest()}\n"
    f"source-sha256 {hashlib.sha256(source).hexdigest()}\n"
    f"source-length {len(source)}\n"
).encode("ascii")
if Path(os.environ["MANIFEST"]).read_bytes() != expected:
    raise SystemExit("Delta compiler composed identity changed")
PY

python3 "$INVOKE" --evaluator "$TMP/evaluator" --source "$SOURCE" \
    --input "$CUSTOMER" --output "$TMP/receipt.gamma" --timeout 60
cmp "$TMP/receipt.gamma" "$EXPECTED"

cat > "$TMP/late-trap.gamma" <<'EOF'
(def $application () Int 1)
(def main () Int
  (let emitted Int (write 65)
    (read (input))))
EOF
printf 'unchanged' > "$TMP/not-published"
set +e
python3 "$INVOKE" --evaluator "$TMP/evaluator" --source "$TMP/late-trap.gamma" \
    --output "$TMP/not-published" --timeout 10
STATUS=$?
set -e
[ "$STATUS" -eq 249 ]
[ "$(cat "$TMP/not-published")" = unchanged ]

cat > "$TMP/empty.gamma" <<'EOF'
(def $application () Int 1)
(def main () Int (pair 0 1))
EOF
printf 'unchanged' > "$TMP/empty-published"
python3 "$INVOKE" --evaluator "$TMP/evaluator" --source "$TMP/empty.gamma" \
        --output "$TMP/empty-published" --timeout 10
[ ! -s "$TMP/empty-published" ]

cat > "$TMP/published-failure.gamma" <<'EOF'
(def $application () Int 1)
(def main () Int
    (let emitted Int (write 65)
        (pair 2 1)))
EOF
printf 'unchanged' > "$TMP/failure-published"
set +e
python3 "$INVOKE" --evaluator "$TMP/evaluator" \
        --source "$TMP/published-failure.gamma" \
        --output "$TMP/failure-published" --timeout 10
STATUS=$?
set -e
[ "$STATUS" -eq 2 ]
[ "$(cat "$TMP/failure-published")" = A ]

cat > "$TMP/discarded.gamma" <<'EOF'
(def $application () Int 1)
(def main () Int
    (let emitted Int (write 65)
        (pair 249 0)))
EOF
printf 'unchanged' > "$TMP/discarded-output"
set +e
python3 "$INVOKE" --evaluator "$TMP/evaluator" \
        --source "$TMP/discarded.gamma" --output "$TMP/discarded-output" \
        --timeout 10
STATUS=$?
set -e
[ "$STATUS" -eq 249 ]
[ "$(cat "$TMP/discarded-output")" = unchanged ]

echo "Gamma composed artifact: exact identity and generic atomic publication passed"
