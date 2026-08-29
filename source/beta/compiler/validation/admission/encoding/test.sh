#!/usr/bin/env sh
# Exercise the Alpha-written, subject-bound encoding ledger.  The optional
# arguments select an explicit source and tape; the canonical edge is default.
set -eu

[ "$#" -le 2 ] || { echo "usage: $0 [SOURCE [TAPE]]" >&2; exit 2; }

HERE=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd -P)
OMEGA_REPO_ROOT=$HERE
while [ ! -f "$OMEGA_REPO_ROOT/tools/lattice/paths.sh" ]; do
  OMEGA_PATH_PARENT=$(dirname -- "$OMEGA_REPO_ROOT")
  [ "$OMEGA_PATH_PARENT" != "$OMEGA_REPO_ROOT" ] || exit 2
  OMEGA_REPO_ROOT=$OMEGA_PATH_PARENT
done
unset OMEGA_PATH_PARENT
. "$OMEGA_REPO_ROOT/tools/lattice/paths.sh"
. "$OMEGA_PATH_ALPHA/seed_env.sh"
. "$OMEGA_PATH_ALPHA_CHECKER/artifact_env.sh"

SOURCE=${1:-"$OMEGA_PATH_BETA_COMPILER_SOURCE"}
TAPE=${2:-"$OMEGA_PATH_BETA_COMPILER_TAPE"}
LEDGER_SOURCE=$HERE/beta-compiler-encoding-ledger.alpha
ASSEMBLER=$OMEGA_PATH_ALPHA_ASSEMBLER/$BETA_SEED
SEED=$OMEGA_PATH_ALPHA/$ALPHA_SEED

[ -f "$SOURCE" ] || { echo "missing source: $SOURCE" >&2; exit 2; }
[ -f "$TAPE" ] || { echo "missing tape: $TAPE" >&2; exit 2; }
[ "$(wc -c < "$SOURCE" | tr -d ' ')" = 78109 ] || {
  echo "encoding ledger subject source must be exactly 78109 bytes" >&2
  exit 2
}
[ "$(wc -c < "$TAPE" | tr -d ' ')" = 20977 ] || {
  echo "encoding ledger subject tape must be exactly 20977 bytes" >&2
  exit 2
}

T=$(mktemp -d)
trap 'trash "$T"' EXIT HUP INT TERM

# Emit an unsigned 64-bit little-endian word.  Subject extents are small enough
# for every POSIX shell arithmetic implementation used by this repository.
u64le() {
  U64LE_VALUE=$1
  U64LE_I=0
  while [ "$U64LE_I" -lt 8 ]; do
    U64LE_BYTE=$((U64LE_VALUE % 256))
    printf "\\$(printf '%03o' "$U64LE_BYTE")"
    U64LE_VALUE=$((U64LE_VALUE / 256))
    U64LE_I=$((U64LE_I + 1))
  done
}

frame() { # source source-extent tape tape-extent output
  {
    u64le "$2"
    cat "$1"
    u64le "$4"
    cat "$3"
  } > "$5"
}

checker_frame() { # source tape certificate output
  {
    printf 'OMGCHK1\n'
    u64le "$(wc -c < "$1" | tr -d ' ')"
    dd if="$1" status=none
    u64le "$(wc -c < "$2" | tr -d ' ')"
    dd if="$2" status=none
    u64le "$(printf '%s' "$3" | wc -c | tr -d ' ')"
    printf '%s' "$3"
  } > "$4"
}

"$ASSEMBLER" < "$LEDGER_SOURCE" > "$T/ledger.tape"
stamp_seed "$T/ledger.tape" "$SEED" "$T/ledger" >/dev/null
stamp_proof_checker "$T/checker" >/dev/null

PASS=0
FAIL=0
case_run() { # name expected-status frame
  set +e
  "$T/ledger" < "$3" > "$T/stdout"
  CASE_STATUS=$?
  set -e
  CASE_STDOUT=$(wc -c < "$T/stdout" | tr -d ' ')
  if [ "$CASE_STATUS" = "$2" ] && [ "$CASE_STDOUT" = 0 ]; then
    PASS=$((PASS + 1))
  else
    FAIL=$((FAIL + 1))
    echo "  FAIL $1: expected status $2/empty output, got $CASE_STATUS/$CASE_STDOUT bytes"
  fi
}

frame "$SOURCE" 78109 "$TAPE" 20977 "$T/valid.frame"
case_run "canonical source and tape" 0 "$T/valid.frame"

# Exercise the exact checker carrier at compiler scale. This is deliberately
# only a capacity/binding control; the status-only Alpha ledger below remains
# nonauthoritative until its full relation is emitted as the checked proof.
CHECKER_CERT='(& (= source source) (= tape tape)) (pair (refl source) (refl tape))'
checker_frame "$SOURCE" "$TAPE" "$CHECKER_CERT" "$T/checker.frame"
set +e
CHECKER_OUT=$("$T/checker" < "$T/checker.frame")
CHECKER_STATUS=$?
set -e
if [ "$CHECKER_STATUS" = 1 ] && [ "$CHECKER_OUT" = accept ]; then
  PASS=$((PASS + 1))
else
  FAIL=$((FAIL + 1))
  echo "  FAIL exact checker subject carrier: expected 1/accept, got $CHECKER_STATUS/$CHECKER_OUT"
fi

# Source-byte control: change the immediate in the first `imm r0, 0` to one.
# This preserves both extents and syntax, but changes one reconstructed byte.
[ "$(od -An -tu1 -j 4668 -N 1 "$SOURCE" | tr -d ' ')" = 48 ] || {
  echo "encoding ledger source-byte control offset is stale" >&2
  exit 2
}
cp "$SOURCE" "$T/source-byte.alpha"
printf '\061' | dd of="$T/source-byte.alpha" bs=1 seek=4668 conv=notrunc status=none
frame "$T/source-byte.alpha" 78109 "$TAPE" 20977 "$T/source-byte.frame"
case_run "source byte" 1 "$T/source-byte.frame"

# Closed-grammar controls retain both subject extents. They must reject as
# malformed source rather than merely reaching the byte comparison mismatch.
[ "$(od -An -tu1 -j 4684 -N 1 "$SOURCE" | tr -d ' ')" = 114 ] || {
  echo "encoding ledger register-kind control offset is stale" >&2
  exit 2
}
cp "$SOURCE" "$T/register-kind.alpha"
printf '\060' | dd of="$T/register-kind.alpha" bs=1 seek=4684 conv=notrunc status=none
frame "$T/register-kind.alpha" 78109 "$TAPE" 20977 "$T/register-kind.frame"
case_run "register operand kind" 7 "$T/register-kind.frame"

[ "$(od -An -tu1 -j 3011 -N 1 "$SOURCE" | tr -d ' ')" = 48 ] || {
  echo "encoding ledger decimal-token control offset is stale" >&2
  exit 2
}
cp "$SOURCE" "$T/decimal-token.alpha"
printf '\170' | dd of="$T/decimal-token.alpha" bs=1 seek=3011 conv=notrunc status=none
frame "$T/decimal-token.alpha" 78109 "$TAPE" 20977 "$T/decimal-token.frame"
case_run "decimal token" 7 "$T/decimal-token.frame"

[ "$(dd if="$SOURCE" bs=1 skip=5332 count=7 status=none)" = pp_loop ] || {
  echo "encoding ledger empty-label control offset is stale" >&2
  exit 2
}
cp "$SOURCE" "$T/empty-label.alpha"
printf '       ' | dd of="$T/empty-label.alpha" bs=1 seek=5332 conv=notrunc status=none
frame "$T/empty-label.alpha" 78109 "$TAPE" 20977 "$T/empty-label.frame"
case_run "empty label" 7 "$T/empty-label.frame"

[ "$(dd if="$SOURCE" bs=1 skip=72964 count=7 status=none)" = af_done ] || {
  echo "encoding ledger duplicate-label control offset is stale" >&2
  exit 2
}
cp "$SOURCE" "$T/duplicate-label.alpha"
printf 'pp_loop' | dd of="$T/duplicate-label.alpha" bs=1 seek=72964 conv=notrunc status=none
frame "$T/duplicate-label.alpha" 78109 "$TAPE" 20977 "$T/duplicate-label.frame"
case_run "duplicate label" 7 "$T/duplicate-label.frame"

[ "$(od -An -tu1 -j 77785 -N 1 "$SOURCE" | tr -d ' ')" = 114 ] || {
  echo "encoding ledger db-escape control offset is stale" >&2
  exit 2
}
cp "$SOURCE" "$T/db-escape.alpha"
printf '\134' | dd of="$T/db-escape.alpha" bs=1 seek=77785 conv=notrunc status=none
frame "$T/db-escape.alpha" 78109 "$TAPE" 20977 "$T/db-escape.frame"
case_run "unknown db escape" 9 "$T/db-escape.frame"

[ "$(od -An -tu1 -j 77783 -N 1 "$SOURCE" | tr -d ' ')" = 34 ] || {
  echo "encoding ledger db-prefix control offset is stale" >&2
  exit 2
}
cp "$SOURCE" "$T/db-prefix.alpha"
printf '\040' | dd of="$T/db-prefix.alpha" bs=1 seek=77783 conv=notrunc status=none
frame "$T/db-prefix.alpha" 78109 "$TAPE" 20977 "$T/db-prefix.frame"
case_run "nonseparator before db string" 9 "$T/db-prefix.frame"

# Ordinary tape-byte control: mutate the first opcode, outside any label fixup.
[ "$(od -An -tu1 -j 0 -N 1 "$TAPE" | tr -d ' ')" = 1 ] || {
  echo "encoding ledger tape-byte control offset is stale" >&2
  exit 2
}
cp "$TAPE" "$T/tape-byte.tape"
printf '\002' | dd of="$T/tape-byte.tape" bs=1 seek=0 conv=notrunc status=none
frame "$SOURCE" 78109 "$T/tape-byte.tape" 20977 "$T/tape-byte.frame"
case_run "tape byte" 1 "$T/tape-byte.frame"

# Label-target control: byte 25 is the low byte of the first conditional branch
# target (92).  Changing only that fixup must be detected by pass-two resolution.
[ "$(od -An -tu1 -j 25 -N 1 "$TAPE" | tr -d ' ')" = 92 ] || {
  echo "encoding ledger label-target control offset is stale" >&2
  exit 2
}
cp "$TAPE" "$T/label-target.tape"
printf '\135' | dd of="$T/label-target.tape" bs=1 seek=25 conv=notrunc status=none
frame "$SOURCE" 78109 "$T/label-target.tape" 20977 "$T/label-target.frame"
case_run "label target" 1 "$T/label-target.frame"

# Extent controls retain the exact raw payloads but lie in their frame headers.
frame "$SOURCE" 78108 "$TAPE" 20977 "$T/source-extent.frame"
case_run "source extent" 2 "$T/source-extent.frame"
frame "$SOURCE" 78109 "$TAPE" 20976 "$T/tape-extent.frame"
case_run "tape extent" 2 "$T/tape-extent.frame"

echo "Beta compiler exact encoding ledger: $PASS passed, $FAIL failed ($(wc -c < "$T/ledger.tape" | tr -d ' ')-byte Alpha ledger tape)"
[ "$FAIL" = 0 ]
