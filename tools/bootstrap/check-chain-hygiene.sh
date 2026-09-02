#!/usr/bin/env sh
# Enforce the selected trust-minimizing bootstrap topology.
set -eu

SCRIPT_DIR=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd -P)
OMEGA_REPO_ROOT=$(CDPATH= cd -- "$SCRIPT_DIR/../.." && pwd -P)
. "$SCRIPT_DIR/paths.sh"

fail() {
  echo "bootstrap paths: $*" >&2
  exit 1
}

for required in \
  "$OMEGA_PATH_ALPHA" \
  "$OMEGA_PATH_BETA" \
  "$OMEGA_PATH_BETA_COMPILER" \
  "$OMEGA_PATH_GAMMA" \
  "$OMEGA_PATH_GAMMA_EVALUATOR" \
  "$OMEGA_PATH_DELTA" \
  "$OMEGA_PATH_EPSILON" \
  "$OMEGA_PATH_EPSILON_COMPILER" \
  "$OMEGA_PATH_OMEGA"
do
  [ -d "$required" ] || fail "required owner is absent: $required"
done

[ -f "$OMEGA_PATH_BETA_COMPILER_SOURCE" ] ||
  fail "Beta compiler source is absent"
[ -f "$OMEGA_PATH_BETA_COMPILER_TAPE" ] ||
  fail "Beta compiler tape is absent"
[ -f "$OMEGA_PATH_GAMMA_EVALUATOR_SOURCE" ] ||
  fail "Beta-written Gamma evaluator source is absent"
[ -x "$OMEGA_REPO_ROOT/tools/bootstrap/check-chain-hygiene.sh" ] ||
  fail "bootstrap topology gate is not executable"
[ -x "$OMEGA_REPO_ROOT/tests/bootstrap/alpha-beta-edge.sh" ] ||
  fail "Alpha-to-Beta edge gate is not executable"
[ -f "$OMEGA_PATH_BETA/LANGUAGE.md" ] || fail "Beta contract is absent"
[ -f "$OMEGA_PATH_GAMMA/LANGUAGE.md" ] || fail "Gamma contract is absent"
[ -f "$OMEGA_PATH_DELTA/LANGUAGE.md" ] || fail "Delta contract is absent"
[ -f "$OMEGA_PATH_EPSILON/LANGUAGE.md" ] || fail "Epsilon contract is absent"
[ -f "$OMEGA_PATH_EPSILON_COMPILER_SOURCE" ] ||
  fail "Delta-written Epsilon compiler source is absent"
[ -f "$OMEGA_PATH_OMEGA_COMPILER_SOURCE" ] ||
  fail "Epsilon-written Omega D source is absent"
[ -f "$OMEGA_PATH_OMEGA/build.omg" ] || fail "Omega C build root is absent"
[ -f "$OMEGA_PATH_OMEGA/main.omg" ] || fail "Omega C main root is absent"

for retired in \
  "$OMEGA_REPO_ROOT/tools/alpha" \
  "$OMEGA_REPO_ROOT/source/alpha/checker" \
  "$OMEGA_REPO_ROOT/tests/proof-checker" \
  "$OMEGA_REPO_ROOT/tools/bootstrap/proof-checker"
do
  [ ! -e "$retired" ] || fail "retired owner remains: $retired"
done

tracked_source_roots=$(find "$OMEGA_REPO_ROOT/source" -mindepth 1 -maxdepth 1 \
  -type d -exec basename {} \; | sort)
expected_source_roots='alpha
beta
delta
epsilon
gamma
library
omega
omega-rust
psi'
[ "$tracked_source_roots" = "$expected_source_roots" ] ||
  fail "tracked source owners differ from the selected source-root set"

tracked_compiler_sources=$(find \
  "$OMEGA_PATH_BETA" "$OMEGA_PATH_GAMMA" "$OMEGA_PATH_DELTA" \
  "$OMEGA_PATH_EPSILON" "$OMEGA_PATH_OMEGA" \
  -type f -name '*compiler.*' | \
  sed "s#^$OMEGA_REPO_ROOT/##" | \
  grep -E '/[^/]*compiler\.(beta|gamma|delta|epsilon|omg)$' | sort || true)
expected_compiler_sources='source/beta/compiler/beta_compiler.beta
source/delta/compiler/experiments/state_machine/delta_compiler.gamma
source/epsilon/compiler/epsilon_compiler.delta
source/gamma/compiler/gamma_compiler.gamma
source/omega/omega_compiler.epsilon'
[ "$tracked_compiler_sources" = "$expected_compiler_sources" ] ||
  fail "compiler source exists outside selected edges or declared experiments"

tracked_compiler_tapes=$(find \
  "$OMEGA_PATH_BETA" "$OMEGA_PATH_GAMMA" "$OMEGA_PATH_DELTA" \
  "$OMEGA_PATH_EPSILON" "$OMEGA_PATH_OMEGA" \
  -type f -name '*compiler*.tape' | sed "s#^$OMEGA_REPO_ROOT/##" | sort || true)
expected_compiler_tapes='source/beta/compiler/beta_compiler_bytecode.tape
source/gamma/compiler/gamma_compiler_bytecode.tape'
[ "$tracked_compiler_tapes" = "$expected_compiler_tapes" ] ||
  fail "compiler tapes differ from selected edges or declared experiments"

stale_paths=$(grep -RInE \
  --exclude-dir=target --exclude-dir=build \
  --exclude=decisions.md --exclude=check-chain-hygiene.sh \
  'tools/alpha(/|$)|tools/bootstrap/proof-checker(/|$)|source/alpha/checker|tests/proof-checker|omega_compiler\.delta|\.alphaasm|alpha_tape_assembler|Alpha Tape Assembly|beta_evaluator|BETAREQ|gamma_compiler\.beta|OMEGA_PATH_ALPHA_TAPE|OMEGA_PATH_BETA_EVALUATOR|OMEGA_PATH_GAMMA_COMPILER' \
  "$OMEGA_REPO_ROOT/source" "$OMEGA_REPO_ROOT/tests" \
  "$OMEGA_REPO_ROOT/tools" "$OMEGA_REPO_ROOT/wiki" \
  "$OMEGA_REPO_ROOT/README.md" "$OMEGA_REPO_ROOT/TASKS_BOOTSTRAP.md" || true)
[ -z "$stale_paths" ] || fail "retired live path or identity remains: $stale_paths"

for bootstrap_source in \
  "$OMEGA_PATH_BETA_COMPILER_SOURCE" \
  "$OMEGA_PATH_BETA/LANGUAGE.md" \
  "$OMEGA_PATH_GAMMA/LANGUAGE.md" \
  "$OMEGA_PATH_GAMMA_EVALUATOR_SOURCE" \
  "$OMEGA_PATH_GAMMA/compiler/gamma_compiler.gamma" \
  "$OMEGA_PATH_GAMMA/reconstruction/gamma_evaluator_reconstructor.gamma" \
  "$OMEGA_PATH_DELTA/LANGUAGE.md" \
  "$OMEGA_PATH_DELTA/compiler/experiments/state_machine/delta_compiler.gamma" \
  "$OMEGA_PATH_EPSILON_COMPILER_SOURCE" \
  "$OMEGA_PATH_OMEGA_COMPILER_SOURCE"
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
