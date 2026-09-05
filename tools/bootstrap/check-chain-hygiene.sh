#!/usr/bin/env sh
# Enforce the selected trust-minimizing bootstrap topology.
set -eu

SCRIPT_DIR=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd -P)
OMEGA_REPO_ROOT=$(CDPATH= cd -- "$SCRIPT_DIR/../.." && pwd -P)
. "$SCRIPT_DIR/paths.sh"

command -v python3 >/dev/null 2>&1 || {
  echo "bootstrap chain topology: skipped (python3 absent)"
  exit 0
}

fail() {
  echo "bootstrap paths: $*" >&2
  exit 1
}

owner_roots() {
  # Empty relocation leftovers and ignored local artifacts are not owners.
  # Include untracked source so a new alternate owner fails before staging.
  if [ -e "$OMEGA_REPO_ROOT/.git" ] &&
      git -C "$OMEGA_REPO_ROOT" rev-parse --is-inside-work-tree >/dev/null 2>&1; then
    owner_files=$(git -C "$OMEGA_REPO_ROOT" ls-files \
      --cached --others --exclude-standard -- "$1") ||
      fail "cannot inventory $1 owners"
  else
    # Source archives remain checkable without Git metadata or a Git binary.
    owner_files=$(find "$OMEGA_REPO_ROOT/$1" -mindepth 2 ! -type d -print | \
      sed "s#^$OMEGA_REPO_ROOT/##")
  fi
  printf '%s\n' "$owner_files" | awk -F/ 'NF >= 3 { print $2 }' | sort -u
}

for required in \
  "$OMEGA_PATH_ALPHA" \
  "$OMEGA_PATH_BETA" \
  "$OMEGA_PATH_BETA_COMPILER" \
  "$OMEGA_PATH_GAMMA" \
  "$OMEGA_PATH_GAMMA_EVALUATOR" \
  "$OMEGA_PATH_CONCATENATIVE_GAMMA" \
  "$OMEGA_PATH_CONCATENATIVE_GAMMA_COMPILER" \
  "$OMEGA_PATH_DELTA" \
  "$OMEGA_PATH_DELTA_COMPILER" \
  "$OMEGA_PATH_EPSILON" \
  "$OMEGA_PATH_EPSILON_COMPILER" \
  "$OMEGA_PATH_OMEGA" \
  "$OMEGA_PATH_OMEGA_D" \
  "$OMEGA_PATH_OMEGA_COMPILER" \
  "$OMEGA_REPO_ROOT/source/library" \
  "$OMEGA_REPO_ROOT/source/psi"
do
  [ -d "$required" ] || fail "required owner is absent: $required"
done

[ -f "$OMEGA_PATH_BETA_COMPILER_SOURCE" ] ||
  fail "Beta compiler source is absent"
[ -f "$OMEGA_PATH_BETA_COMPILER_TAPE" ] ||
  fail "Beta compiler tape is absent"
[ -f "$OMEGA_PATH_GAMMA_EVALUATOR_SOURCE" ] ||
  fail "Beta-written Gamma evaluator source is absent"
[ -f "$OMEGA_PATH_GAMMA_EVALUATOR_TAPE" ] ||
  fail "Gamma evaluator tape is absent"
[ -f "$OMEGA_PATH_CONCATENATIVE_GAMMA_COMPILER_SOURCE" ] ||
  fail "downgraded concatenative Gamma compiler source is absent"
[ -f "$OMEGA_PATH_CONCATENATIVE_GAMMA_COMPILER_RECEIPT" ] ||
  fail "downgraded concatenative Gamma compiler receipt is absent"
[ -f "$OMEGA_PATH_CONCATENATIVE_GAMMA_COMPILER_TAPE" ] ||
  fail "downgraded concatenative Gamma compiler tape is absent"
[ -f "$OMEGA_PATH_DELTA_COMPILER_SOURCE" ] ||
  fail "Gamma-authored staged Delta compiler entry is absent"
[ -f "$OMEGA_PATH_DELTA_COMPILER_SOURCES" ] ||
  fail "Gamma-authored staged Delta compiler source manifest is absent"
[ -f "$OMEGA_PATH_DELTA_COMPILER_COMPOSED" ] ||
  fail "staged Delta composed identity is absent"
[ -x "$OMEGA_REPO_ROOT/tools/bootstrap/check-chain-hygiene.sh" ] ||
  fail "bootstrap topology gate is not executable"
[ -x "$OMEGA_REPO_ROOT/tests/bootstrap/alpha-beta-edge.sh" ] ||
  fail "Alpha-to-Beta edge gate is not executable"
[ -x "$OMEGA_REPO_ROOT/tools/bootstrap/gamma/invoke.py" ] ||
  fail "Gamma atomic invocation plumbing is not executable"
[ -f "$OMEGA_PATH_BETA/LANGUAGE.md" ] || fail "Beta contract is absent"
[ -f "$OMEGA_PATH_GAMMA/LANGUAGE.md" ] || fail "Gamma contract is absent"
[ -f "$OMEGA_PATH_GAMMA/COMPOSED_ARTIFACT.md" ] ||
  fail "Gamma composed-artifact contract is absent"
[ -f "$OMEGA_PATH_DELTA/LANGUAGE.md" ] || fail "Delta contract is absent"
[ -f "$OMEGA_PATH_EPSILON/LANGUAGE.md" ] || fail "Epsilon contract is absent"
[ -f "$OMEGA_PATH_EPSILON_COMPILER_SOURCES" ] ||
  fail "Delta-written Epsilon evaluator source manifest is absent"
[ -f "$OMEGA_PATH_OMEGA_COMPILER_SOURCES" ] ||
  fail "Epsilon-written Omega D source manifest is absent"
[ -x "$OMEGA_REPO_ROOT/tools/bootstrap/source_closure.py" ] ||
  fail "source-closure materializer is not executable"
[ -x "$OMEGA_REPO_ROOT/tests/bootstrap/source-closure.sh" ] ||
  fail "source-closure gate is not executable"
[ -f "$OMEGA_PATH_OMEGA/build.omg" ] || fail "Omega C build root is absent"
[ -f "$OMEGA_PATH_OMEGA/main.omg" ] || fail "Omega C main root is absent"

for beta_source in \
  "$OMEGA_PATH_BETA_COMPILER_SOURCE" \
  "$OMEGA_PATH_GAMMA_EVALUATOR_SOURCE"
do
  uncommented_targets=$(grep -En \
    '^[[:space:]]*(jmp|jz|jnz|jlt|jeq|call)[[:space:]].*0x[0-9a-f]+[[:space:]]*$' \
    "$beta_source" || true)
  [ -z "$uncommented_targets" ] ||
    fail "selected Beta control target lacks a compact label comment: $uncommented_targets"
done

for retired in \
  "$OMEGA_REPO_ROOT/tools/alpha" \
  "$OMEGA_REPO_ROOT/bootstrap/alpha/checker" \
  "$OMEGA_REPO_ROOT/tests/proof-checker" \
  "$OMEGA_REPO_ROOT/tools/bootstrap/proof-checker" \
  "$OMEGA_REPO_ROOT/tools/bootstrap/epsilon/materialize_source_closure.py"
do
  [ ! -e "$retired" ] || fail "retired owner remains: $retired"
done

tracked_source_roots=$(owner_roots source)
expected_source_roots='library
omega
psi'
[ "$tracked_source_roots" = "$expected_source_roots" ] ||
  fail "tracked final-source owners differ from library, Psi, and Omega"

tracked_bootstrap_roots=$(owner_roots bootstrap)
expected_bootstrap_roots='alpha
beta
delta
epsilon
gamma
omega'
[ "$tracked_bootstrap_roots" = "$expected_bootstrap_roots" ] ||
  fail "tracked bootstrap owners differ from the selected rung set"

tracked_compiler_sources=$(find \
  "$OMEGA_PATH_BETA_COMPILER" "$OMEGA_PATH_DELTA_COMPILER" \
  "$OMEGA_PATH_EPSILON_COMPILER" \
  -type f -name '*compiler.*' -print | \
  sed "s#^$OMEGA_REPO_ROOT/##" | \
  grep -E '/[^/]*compiler\.(beta|gamma|delta|epsilon|omg)$' | sort || true)
expected_compiler_sources='bootstrap/beta/compiler/beta_compiler.beta
bootstrap/delta/compiler/delta_compiler.gamma
bootstrap/epsilon/compiler/epsilon_compiler.delta'
[ "$tracked_compiler_sources" = "$expected_compiler_sources" ] ||
  fail "compiler source exists outside selected edges"

PACKED_DIR=$(mktemp -d)
trap 'rm -rf -- "$PACKED_DIR"' EXIT HUP INT TERM
python3 "$OMEGA_REPO_ROOT/tools/bootstrap/source_closure.py" \
  "$OMEGA_PATH_DELTA_COMPILER_SOURCES" "$PACKED_DIR/compiler.gamma" \
  --prefix "$OMEGA_PATH_DELTA_COMPILER_SOURCE"
python3 "$OMEGA_REPO_ROOT/tools/bootstrap/source_closure.py" \
  "$OMEGA_PATH_EPSILON_COMPILER_SOURCES" "$PACKED_DIR/evaluator.delta"
python3 "$OMEGA_REPO_ROOT/tools/bootstrap/source_closure.py" \
  "$OMEGA_PATH_OMEGA_COMPILER_SOURCES" "$PACKED_DIR/compiler.epsilon"

tracked_compiler_tapes=$(find \
  "$OMEGA_PATH_BETA_COMPILER" "$OMEGA_PATH_DELTA_COMPILER" \
  "$OMEGA_PATH_EPSILON_COMPILER" \
  -type f -name '*compiler*.tape' -print | \
  sed "s#^$OMEGA_REPO_ROOT/##" | sort || true)
expected_compiler_tapes='bootstrap/beta/compiler/beta_compiler_bytecode.tape'
[ "$tracked_compiler_tapes" = "$expected_compiler_tapes" ] ||
  fail "compiler tapes differ from selected edges or declared experiments"

stale_paths=$(grep -RInE \
  --exclude-dir=target --exclude-dir=build \
  --exclude=decisions.md --exclude=check-chain-hygiene.sh \
  'tools/alpha(/|$)|tools/bootstrap/proof-checker(/|$)|bootstrap/alpha/checker|tests/proof-checker|compiler\.delta|\.alphaasm|alpha_tape_assembler|Alpha Tape Assembly|beta_evaluator|BETAREQ|OMEGA_PATH_ALPHA_TAPE|OMEGA_PATH_BETA_EVALUATOR|tools/bootstrap/epsilon/materialize_source_closure\.py|OMEGA_PATH_EPSILON_COMPILER_SOURCE([^S]|$)' \
  "$OMEGA_PATH_BOOTSTRAP" "$OMEGA_REPO_ROOT/source" "$OMEGA_REPO_ROOT/tests" \
  "$OMEGA_REPO_ROOT/tools" "$OMEGA_REPO_ROOT/wiki" \
  "$OMEGA_REPO_ROOT/README.md" "$OMEGA_REPO_ROOT/TASKS_BOOTSTRAP.md" || true)
[ -z "$stale_paths" ] || fail "retired live path or identity remains: $stale_paths"

for bootstrap_source in \
  "$OMEGA_PATH_BETA_COMPILER_SOURCE" \
  "$OMEGA_PATH_BETA/LANGUAGE.md" \
  "$OMEGA_PATH_GAMMA/LANGUAGE.md" \
  "$OMEGA_PATH_GAMMA/COMPOSED_ARTIFACT.md" \
  "$OMEGA_PATH_GAMMA_EVALUATOR_SOURCE" \
  "$OMEGA_PATH_CONCATENATIVE_GAMMA_COMPILER_SOURCE" \
  "$OMEGA_PATH_CONCATENATIVE_GAMMA_COMPILER_RECEIPT" \
  "$OMEGA_PATH_DELTA/LANGUAGE.md" \
  "$OMEGA_PATH_DELTA_COMPILER_SOURCE" \
  "$OMEGA_PATH_DELTA_COMPILER_COMPOSED" \
  "$OMEGA_PATH_CONCATENATIVE_DELTA_COMPILER_SOURCE"
do
  if ! od -An -tu1 -v "$bootstrap_source" | awk '
    {
      for (i = 1; i <= NF; i++) {
        b = $i + 0
        if (b != 9 && b != 10 && b != 13 && (b < 32 || b > 126)) exit 1
      }
    }
  '; then
    fail "bootstrap source contains a forbidden byte: $bootstrap_source"
  fi
done

printf '%s\n' 'bootstrap chain topology and path hygiene OK'
