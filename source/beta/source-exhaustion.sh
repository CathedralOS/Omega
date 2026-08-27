#!/usr/bin/env sh
# Pin the supported bc compiler resource profile: source bytes, per-procedure
# names, register arguments, and recursive expression depth. Source exhaustion
# occurs before publication; later structural exhaustion returns a deterministic
# maximal assembly prefix without writing outside compiler-owned extents.
set -e
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
cd "$OMEGA_GATE_DIR"
T=$(mktemp -d); trap 'rm -rf "$T"' EXIT
. "$OMEGA_PATH_BETA/artifact_env.sh"
BC="$T/bc.exe"
stamp_beta_compiler "$BC" >/dev/null || { echo "bc source exhaustion FAIL — artifact stamp"; exit 1; }
LIMIT=1048576

run_status() { # input output
  set +e
  "$BC" < "$1" > "$2" 2>/dev/null
  RUN_STATUS=$?
  set -e
}

accepted() { # name input
  run_status "$2" "$T/out"
  if [ "$RUN_STATUS" != 0 ] || [ ! -s "$T/out" ]; then
    echo "bc resource exhaustion FAIL — $1 exited $RUN_STATUS or emitted no assembly"
    exit 1
  fi
}

exhausted() { # name input expected-status empty|prefix
  run_status "$2" "$T/first"
  first_status=$RUN_STATUS
  run_status "$2" "$T/second"
  second_status=$RUN_STATUS
  if [ "$first_status" != "$3" ] || [ "$second_status" != "$3" ] || ! cmp -s "$T/first" "$T/second"; then
    echo "bc resource exhaustion FAIL — $1 status/prefix is not deterministic ($first_status/$second_status)"
    exit 1
  fi
  if [ "$4" = empty ] && [ -s "$T/first" ]; then
    echo "bc resource exhaustion FAIL — $1 published a partial artifact"
    exit 1
  fi
  if [ "$4" = prefix ] && [ ! -s "$T/first" ]; then
    echo "bc resource exhaustion FAIL — $1 did not retain its maximal emitted prefix"
    exit 1
  fi
}

printf 'proc main() { return 0 }' > "$T/exact.beta"
PREFIX=$(wc -c < "$T/exact.beta" | tr -d ' ')
dd if=/dev/zero bs=$((LIMIT - PREFIX)) count=1 2>/dev/null | tr '\000' ' ' >> "$T/exact.beta"

accepted "exact 1 MiB source" "$T/exact.beta"

# A full source ending in a single '=' used to let cmp_op read NAMEOFF[0] one
# byte past the source arena. Vary only the first local's offset so its low byte
# is '=' (61) versus '>' (62); bounded lookahead must make outputs identical.
cmp_boundary_program() { # prepad output
  cmp_prepad=$1
  cmp_dst=$2
  printf 'proc p() {' > "$cmp_dst"
  dd if=/dev/zero bs="$cmp_prepad" count=1 2>/dev/null | tr '\000' ' ' >> "$cmp_dst"
  printf 'let a = 0 return 0 }' >> "$cmp_dst"
  CMP_TAIL='proc z() { return 0='
  cmp_used=$(wc -c < "$cmp_dst" | tr -d ' ')
  cmp_fill=$((LIMIT - cmp_used - ${#CMP_TAIL}))
  dd if=/dev/zero bs="$cmp_fill" count=1 2>/dev/null | tr '\000' ' ' >> "$cmp_dst"
  printf '%s' "$CMP_TAIL" >> "$cmp_dst"
}
cmp_boundary_program 47 "$T/cmp-boundary-a.beta"
cmp_boundary_program 48 "$T/cmp-boundary-b.beta"
[ "$(wc -c < "$T/cmp-boundary-a.beta" | tr -d ' ')" = "$LIMIT" ]
[ "$(wc -c < "$T/cmp-boundary-b.beta" | tr -d ' ')" = "$LIMIT" ]
[ "$(tail -c "${#CMP_TAIL}" "$T/cmp-boundary-a.beta")" = "$CMP_TAIL" ]
[ "$(tail -c "${#CMP_TAIL}" "$T/cmp-boundary-b.beta")" = "$CMP_TAIL" ]
[ "$(dd if="$T/cmp-boundary-a.beta" bs=1 skip=61 count=1 2>/dev/null)" = a ]
[ "$(dd if="$T/cmp-boundary-b.beta" bs=1 skip=62 count=1 2>/dev/null)" = a ]
run_status "$T/cmp-boundary-a.beta" "$T/cmp-boundary-a.alpha"
cmp_boundary_a_status=$RUN_STATUS
run_status "$T/cmp-boundary-b.beta" "$T/cmp-boundary-b.alpha"
cmp_boundary_b_status=$RUN_STATUS
if [ "$cmp_boundary_a_status" != 0 ] || [ "$cmp_boundary_b_status" != 0 ] ||
   ! cmp -s "$T/cmp-boundary-a.alpha" "$T/cmp-boundary-b.alpha" ||
   [ "$(wc -c < "$T/cmp-boundary-a.alpha" | tr -d ' ')" != 621 ] ||
   grep -q '^jeq r0,r1,' "$T/cmp-boundary-a.alpha"; then
  echo "bc resource exhaustion FAIL — full-source cmp lookahead crossed into NAMEOFF"
  exit 1
fi

cp "$T/exact.beta" "$T/oversized.beta"
printf x >> "$T/oversized.beta"
exhausted "1 MiB + 1 source" "$T/oversized.beta" 253 empty

# NAMEOFF/NAMELEN contain exactly 1,024 slots. The 1,025th declaration must
# reject before either table overlaps the other.
names_program() { # count output
  n=$1
  dst=$2
  printf 'proc main() {\n' > "$dst"
  i=0
  while [ "$i" -lt "$n" ]; do
    printf 'let n%s = 0\n' "$i" >> "$dst"
    i=$((i + 1))
  done
  printf 'return 0\n}\n' >> "$dst"
}
names_program 1024 "$T/names-exact.beta"
names_program 1025 "$T/names-over.beta"
accepted "1,024 name slots" "$T/names-exact.beta"
exhausted "1,025th name slot" "$T/names-over.beta" 252 prefix

# Beta's ABI has four live argument registers. Refuse a fifth parameter or
# argument rather than staging an unbounded list on the compiler data stack.
printf '%s\n' 'proc f(a,b,c,d) { return a+b+c+d }' 'proc main() { return f(1,2,3,4) }' > "$T/args-exact.beta"
printf '%s\n' 'proc f(a,b,c,d,e) { return a }' 'proc main() { return 0 }' > "$T/params-over.beta"
printf '%s\n' 'proc f(a,b,c,d) { return a }' 'proc main() { return f(1,2,3,4,5) }' > "$T/args-over.beta"
accepted "four parameters and arguments" "$T/args-exact.beta"
exhausted "fifth parameter" "$T/params-over.beta" 252 prefix
exhausted "fifth argument" "$T/args-over.beta" 252 prefix

# The outer gen_expr invocation consumes one level, so 63 parenthesized factors
# reach exactly depth 64 and the next level is checked exhaustion.
expr_program() { # parenthesis count output
  n=$1
  dst=$2
  printf 'proc main() { return ' > "$dst"
  i=0
  while [ "$i" -lt "$n" ]; do printf '(' >> "$dst"; i=$((i + 1)); done
  printf '0' >> "$dst"
  i=0
  while [ "$i" -lt "$n" ]; do printf ')' >> "$dst"; i=$((i + 1)); done
  printf ' }\n' >> "$dst"
}
expr_program 63 "$T/depth-exact.beta"
expr_program 64 "$T/depth-over.beta"
accepted "expression depth 64" "$T/depth-exact.beta"
exhausted "expression depth 65" "$T/depth-over.beta" 252 prefix

# Nested state blocks recurse through gen_stmts independently of expression
# recursion. The outer procedure body is level one.
block_program() { # nested-state count output
  n=$1
  dst=$2
  printf 'proc main() {\n' > "$dst"
  i=0
  while [ "$i" -lt "$n" ]; do printf 'state s%s {\n' "$i" >> "$dst"; i=$((i + 1)); done
  printf 'return 0\n' >> "$dst"
  i=0
  while [ "$i" -lt "$n" ]; do printf '}\n' >> "$dst"; i=$((i + 1)); done
  printf '}\n' >> "$dst"
}
block_program 63 "$T/blocks-exact.beta"
block_program 64 "$T/blocks-over.beta"
accepted "block depth 64" "$T/blocks-exact.beta"
exhausted "block depth 65" "$T/blocks-over.beta" 252 prefix

echo "bc resource profile: source 1048576/+1, names 1024/+1, args 4/+1, expression/block depth 64/+1"
