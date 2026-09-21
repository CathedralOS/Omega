#!/usr/bin/env sh
# Enforce the selected trust-minimizing bootstrap topology.
set -eu

SCRIPT_DIR=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd -P)
OMEGA_REPO_ROOT=$(CDPATH= cd -- "$SCRIPT_DIR/../.." && pwd -P)
. "$SCRIPT_DIR/paths.sh"
. "$SCRIPT_DIR/delta/compiler_env.sh"
. "$SCRIPT_DIR/epsilon/evaluator_env.sh"
. "$SCRIPT_DIR/omega/compiler_env.sh"

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
  "$OMEGA_PATH_DELTA" \
  "$OMEGA_PATH_DELTA_COMPILER" \
  "$OMEGA_PATH_EPSILON" \
  "$OMEGA_PATH_EPSILON_COMPILER" \
  "$OMEGA_PATH_OMEGA" \
  "$OMEGA_PATH_OMEGA_D" \
  "$OMEGA_PATH_OMEGA_COMPILER" \
  "$OMEGA_PATH_PROOFS" \
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
[ -f "$OMEGA_PATH_DELTA_COMPILER_SOURCE" ] ||
  fail "Gamma-authored staged Delta compiler entry is absent"
[ -f "$OMEGA_PATH_DELTA_COMPILER_SOURCES" ] ||
  fail "Gamma-authored staged Delta compiler source manifest is absent"
[ -f "$OMEGA_PATH_DELTA_COMPILER_COMPOSED" ] ||
  fail "staged Delta composed identity is absent"
[ -f "$OMEGA_PATH_DELTA_COMPILER_SUPPORT_SOURCES" ] ||
  fail "Delta support-member source manifest is absent"
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
[ -f "$OMEGA_PATH_DERIVATION_CHECKER_SOURCES" ] ||
  fail "Gamma derivation checker source manifest is absent"
[ -f "$OMEGA_PATH_BETA_ENCODING_SOURCES" ] ||
  fail "Beta encoding theory source manifest is absent"

for flat_rung in "$OMEGA_PATH_ALPHA" "$OMEGA_PATH_BETA" "$OMEGA_PATH_GAMMA"
do
  nested_directories=$(find "$flat_rung" -mindepth 1 -type d -print)
  [ -z "$nested_directories" ] || fail "flat rung contains subdirectories: $nested_directories"
done
for compiler_rung in "$OMEGA_PATH_DELTA" "$OMEGA_PATH_EPSILON" "$OMEGA_PATH_OMEGA_D"
do
  [ ! -e "$compiler_rung/compiler" ] || fail "redundant compiler directory remains: $compiler_rung/compiler"
done
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
  "$OMEGA_REPO_ROOT/bootstrap/0_alpha/checker" \
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
expected_bootstrap_roots='0_alpha
1_beta
2_gamma
3_delta
4_epsilon
5_omega
proofs'
[ "$tracked_bootstrap_roots" = "$expected_bootstrap_roots" ] ||
  fail "tracked bootstrap owners differ from the selected rungs and proof work"

tracked_compiler_sources=$(find \
  "$OMEGA_PATH_BETA_COMPILER" "$OMEGA_PATH_DELTA_COMPILER" \
  "$OMEGA_PATH_EPSILON_COMPILER" \
  -type f -name '*compiler.*' -print | \
  sed "s#^$OMEGA_REPO_ROOT/##" | \
  grep -E '/[^/]*compiler\.(beta|gamma|delta|epsilon|omg)$' | sort || true)
expected_compiler_sources='bootstrap/1_beta/beta_compiler.beta
bootstrap/3_delta/delta_compiler.gamma
bootstrap/4_epsilon/epsilon_compiler.delta'
[ "$tracked_compiler_sources" = "$expected_compiler_sources" ] ||
  fail "compiler source exists outside selected edges"

# The enumerated compiler sources must be exactly the bound chain artifacts:
# each require_* check binds the canonical entry, manifest, every member, the
# packed closure, and the composed record to the audited edge records, and
# refuses before any consumer could pack a substituted manifest.
require_delta_compiler_identity
require_epsilon_evaluator_identity
require_omega_compiler_identity

# Every gate-local subject bound in the manifest env — each omega-* gate's
# customer entry and the request fixture — packs on top of the bound compiler
# prefix and is a separate bound input, never part of the manifested members.
# These checks bind the local subjects to the same size and digest records
# each gate's run.sh verifies before packing, so a substituted or drifted
# gate-local subject refuses here exactly as a substituted manifest does.
require_omega_parser_entry_identity
require_omega_outcome_entry_identity
require_omega_request_entry_identity
require_omega_request_fixture_identity
require_omega_executable_entries_identity

# The manifest's gate-local digests are one subject recorded twice: the bound
# pin above and the gate's own record of the same subject. The executable
# gate repins every entry in its README; the single-entry gates repin entry
# and fixture digests in both README.md and gate.py. Refuse when a record
# that claims the subject lacks its bound digest — the records are the same
# pin, not independent identities.
for bound_record in \
  "$OMEGA_PARSER_ENTRY_SHA256 tests/bootstrap/omega-parser/README.md" \
  "$OMEGA_PARSER_ENTRY_SHA256 tests/bootstrap/omega-parser/gate.py" \
  "$OMEGA_OUTCOME_ENTRY_SHA256 tests/bootstrap/omega-outcome/README.md" \
  "$OMEGA_OUTCOME_ENTRY_SHA256 tests/bootstrap/omega-outcome/gate.py" \
  "$OMEGA_REQUEST_ENTRY_SHA256 tests/bootstrap/omega-request/README.md" \
  "$OMEGA_REQUEST_ENTRY_SHA256 tests/bootstrap/omega-request/gate.py" \
  "$OMEGA_REQUEST_FIXTURE_SHA256 tests/bootstrap/omega-request/README.md" \
  "$OMEGA_REQUEST_FIXTURE_SHA256 tests/bootstrap/omega-request/gate.py" \
  "$OMEGA_EXECUTABLE_MAIN_ENTRY_SHA256 tests/bootstrap/omega-executable/README.md" \
  "$OMEGA_EXECUTABLE_OCREQ_ENTRY_SHA256 tests/bootstrap/omega-executable/README.md" \
  "$OMEGA_EXECUTABLE_CONTROLS_ENTRY_SHA256 tests/bootstrap/omega-executable/README.md" \
  "$OMEGA_EXECUTABLE_CONTROLS_B_ENTRY_SHA256 tests/bootstrap/omega-executable/README.md" \
  "$OMEGA_EXECUTABLE_CONTROLS_C_ENTRY_SHA256 tests/bootstrap/omega-executable/README.md" \
  "$OMEGA_EXECUTABLE_CONTROLS_D_ENTRY_SHA256 tests/bootstrap/omega-executable/README.md" \
  "$OMEGA_EXECUTABLE_CONTROLS_E_ENTRY_SHA256 tests/bootstrap/omega-executable/README.md" \
  "$OMEGA_EXECUTABLE_CONTROLS_F_ENTRY_SHA256 tests/bootstrap/omega-executable/README.md" \
  "$OMEGA_EXECUTABLE_CONTROLS_G_ENTRY_SHA256 tests/bootstrap/omega-executable/README.md" \
  "$OMEGA_EXECUTABLE_CONTROLS_H_ENTRY_SHA256 tests/bootstrap/omega-executable/README.md"
do
  set -- $bound_record
  grep -Fq "$1" "$OMEGA_REPO_ROOT/$2" ||
    fail "bound gate-local digest is absent from the gate's own record: $2"
done

tracked_compiler_tapes=$(find \
  "$OMEGA_PATH_BETA_COMPILER" "$OMEGA_PATH_DELTA_COMPILER" \
  "$OMEGA_PATH_EPSILON_COMPILER" \
  -type f -name '*compiler*.tape' -print | \
  sed "s#^$OMEGA_REPO_ROOT/##" | sort || true)
expected_compiler_tapes='bootstrap/1_beta/beta_compiler_bytecode.tape'
[ "$tracked_compiler_tapes" = "$expected_compiler_tapes" ] ||
  fail "compiler tapes differ from selected edges or declared experiments"

stale_paths=$(grep -RInE \
  --exclude-dir=target --exclude-dir=build \
  --exclude=check-chain-hygiene.sh \
  'tools/alpha(/|$)|tools/bootstrap/proof-checker(/|$)|bootstrap/0_alpha/checker|tests/proof-checker|omega_compiler\.delta|\.alphaasm|alpha_tape_assembler|Alpha Tape Assembly|beta_evaluator|BETAREQ|OMEGA_PATH_ALPHA_TAPE|OMEGA_PATH_BETA_EVALUATOR|tools/bootstrap/epsilon/materialize_source_closure\.py|OMEGA_PATH_EPSILON_COMPILER_SOURCE([^S]|$)' \
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
  "$OMEGA_PATH_DELTA/LANGUAGE.md" \
  "$OMEGA_PATH_DELTA_COMPILER_SOURCE" \
  "$OMEGA_PATH_DELTA_COMPILER_COMPOSED" \
  "$OMEGA_PATH_DELTA_COMPILER_SUPPORT_SOURCES" \
  "$OMEGA_PATH_DELTA_COMPILER_SUPPORT/bytes.gamma" \
  "$OMEGA_PATH_DELTA_COMPILER_SUPPORT/conformance.gamma" \
  "$OMEGA_PATH_DELTA_COMPILER_SUPPORT/adapter.gamma"
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
