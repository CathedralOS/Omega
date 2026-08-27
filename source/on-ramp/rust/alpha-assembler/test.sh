#!/usr/bin/env sh
# Focused conformance gate for the disposable Rust Alpha-assembler producer.
set -eu

OMEGA_GATE_DIR=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd -P)
if [ -z "${OMEGA_REPO_ROOT:-}" ]; then
  OMEGA_REPO_ROOT=$OMEGA_GATE_DIR
  while [ ! -f "$OMEGA_REPO_ROOT/tools/bootstrap/paths.sh" ]; do
    OMEGA_PATH_PARENT=$(dirname -- "$OMEGA_REPO_ROOT")
    if [ "$OMEGA_PATH_PARENT" = "$OMEGA_REPO_ROOT" ]; then
      echo "bootstrap paths: cannot find repository root from $OMEGA_GATE_DIR" >&2
      exit 2
    fi
    OMEGA_REPO_ROOT=$OMEGA_PATH_PARENT
  done
  unset OMEGA_PATH_PARENT
fi
. "$OMEGA_REPO_ROOT/tools/bootstrap/paths.sh" || exit $?
. "$OMEGA_PATH_ALPHA/seed_env.sh"
cd "$OMEGA_GATE_DIR"

if ! command -v cargo >/dev/null 2>&1; then
  echo "alpha-assembler-rust SKIP — cargo unavailable"
  exit 0
fi

LATTICE_ASSEMBLER="$OMEGA_PATH_ALPHA_ASSEMBLER/$BETA_SEED"
[ -x "$LATTICE_ASSEMBLER" ] || {
  echo "alpha-assembler-rust FAIL — host lattice assembler is unavailable" >&2
  exit 1
}

cargo build --quiet
RUST_ASSEMBLER="$OMEGA_GATE_DIR/target/debug/assembler"
T=$(mktemp -d)
trap 'rm -rf "$T"' EXIT
PASS=0
FAIL=0

compare_source() {
  name=$1
  source=$2
  if "$RUST_ASSEMBLER" < "$source" > "$T/rust.tape" \
      && "$LATTICE_ASSEMBLER" < "$source" > "$T/lattice.tape" \
      && cmp -s "$T/rust.tape" "$T/lattice.tape"; then
    PASS=$((PASS + 1))
  else
    FAIL=$((FAIL + 1))
    echo "  FAIL $name: Rust and lattice-built assembler tapes differ" >&2
  fi
}

compare_source "assembler.alpha self source" \
  "$OMEGA_PATH_ALPHA_ASSEMBLER/assembler.alpha"
for source in "$OMEGA_PATH_ALPHA_ASSEMBLER"/examples/*.alpha; do
  [ -f "$source" ] && compare_source "example $(basename "$source")" "$source"
done

# File arguments must publish exactly the same tape as stdin/stdout.
source="$OMEGA_PATH_ALPHA_ASSEMBLER/examples/multiply.alpha"
if "$RUST_ASSEMBLER" "$source" "$T/file.tape" \
    && "$RUST_ASSEMBLER" < "$source" > "$T/stream.tape" \
    && cmp -s "$T/file.tape" "$T/stream.tape"; then
  PASS=$((PASS + 1))
else
  FAIL=$((FAIL + 1))
  echo "  FAIL file arguments changed the emitted tape" >&2
fi

# The historical Rust-only numeric transport must preserve the tape. The
# lattice-built assembler's accepted mnemonic surface is independently gated;
# it is not required to accept this cold-start transport syntax.
if "$RUST_ASSEMBLER" --num "$source" > "$T/numeric.alpha" \
    && "$RUST_ASSEMBLER" < "$T/numeric.alpha" > "$T/numeric-rust.tape" \
    && cmp -s "$T/stream.tape" "$T/numeric-rust.tape"; then
  PASS=$((PASS + 1))
else
  FAIL=$((FAIL + 1))
  echo "  FAIL numeric transport changed the emitted tape" >&2
fi

reject() {
  name=$1
  source=$2
  set +e
  printf '%s\n' "$source" | "$RUST_ASSEMBLER" > "$T/reject.tape" 2> "$T/reject.err"
  status=$?
  set -e
  if [ "$status" -ne 0 ] && [ ! -s "$T/reject.tape" ]; then
    PASS=$((PASS + 1))
  else
    FAIL=$((FAIL + 1))
    echo "  FAIL $name: malformed source status=$status output=$(wc -c < "$T/reject.tape" | tr -d ' ') bytes" >&2
  fi
}

reject "unknown mnemonic" "unknown r0"
reject "undefined label" "jmp missing"
reject "register out of range" "halt r16"
reject "wrong arity" "ret r0"
reject "numeric opcode out of range" "21 r0"

echo "alpha-assembler-rust: $PASS passed, $FAIL failed"
[ "$FAIL" = 0 ]
