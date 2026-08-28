#!/usr/bin/env sh
# Canonical repository paths for the compiler lattice.
#
# This file is replaceable invocation plumbing. It names source owners only;
# it does not perform a translation or contribute semantics to a compiler edge.

if [ -z "${OMEGA_REPO_ROOT:-}" ] || [ ! -f "$OMEGA_REPO_ROOT/TASKS_BOOTSTRAP.md" ]; then
  echo "lattice paths: OMEGA_REPO_ROOT is not an Omega repository root" >&2
  return 2
fi

OMEGA_PATH_SOURCE_ROOT=$OMEGA_REPO_ROOT/source
OMEGA_PATH_LATTICE_TEST_ROOT=$OMEGA_REPO_ROOT/tests/lattice
OMEGA_PATH_LATTICE_TOOLS_ROOT=$OMEGA_REPO_ROOT/tools/lattice

OMEGA_PATH_ALPHA=$OMEGA_PATH_SOURCE_ROOT/alpha
OMEGA_PATH_ALPHA_ASSEMBLER=$OMEGA_PATH_ALPHA/assembler
OMEGA_PATH_BETA=$OMEGA_PATH_SOURCE_ROOT/beta
OMEGA_PATH_BETA_COMPILER=$OMEGA_PATH_BETA/compiler
OMEGA_PATH_BETA_REFERENCE=$OMEGA_PATH_BETA/reference
OMEGA_PATH_BETA_VALIDATION=$OMEGA_PATH_BETA_COMPILER/validation
OMEGA_PATH_GAMMA=$OMEGA_PATH_SOURCE_ROOT/gamma
OMEGA_PATH_DELTA=$OMEGA_PATH_SOURCE_ROOT/delta
OMEGA_PATH_DELTA_MEANING=$OMEGA_PATH_DELTA/meaning
OMEGA_PATH_OMEGA=$OMEGA_PATH_SOURCE_ROOT/omega
OMEGA_PATH_OMEGA_PSI=$OMEGA_PATH_OMEGA/psi
OMEGA_PATH_ALPHA_CHECKER=$OMEGA_PATH_ALPHA/checker
OMEGA_PATH_CORPUS=$OMEGA_PATH_LATTICE_TEST_ROOT/corpus

export OMEGA_REPO_ROOT OMEGA_PATH_SOURCE_ROOT
export OMEGA_PATH_LATTICE_TEST_ROOT OMEGA_PATH_LATTICE_TOOLS_ROOT
export OMEGA_PATH_ALPHA OMEGA_PATH_ALPHA_ASSEMBLER OMEGA_PATH_BETA
export OMEGA_PATH_BETA_COMPILER OMEGA_PATH_BETA_REFERENCE
export OMEGA_PATH_BETA_VALIDATION OMEGA_PATH_GAMMA
export OMEGA_PATH_DELTA OMEGA_PATH_DELTA_MEANING
export OMEGA_PATH_OMEGA OMEGA_PATH_OMEGA_PSI
export OMEGA_PATH_ALPHA_CHECKER OMEGA_PATH_CORPUS

# lattice_path ROLE
# Print one canonical role path. The function name belongs to the runner; it
# does not name a compiler artifact.
lattice_path() {
  case "$1" in
    lattice-tools) printf '%s\n' "$OMEGA_PATH_LATTICE_TOOLS_ROOT" ;;
    alpha) printf '%s\n' "$OMEGA_PATH_ALPHA" ;;
    alpha-assembler) printf '%s\n' "$OMEGA_PATH_ALPHA_ASSEMBLER" ;;
    alpha-checker) printf '%s\n' "$OMEGA_PATH_ALPHA_CHECKER" ;;
    beta-compiler) printf '%s\n' "$OMEGA_PATH_BETA_COMPILER" ;;
    beta-validation) printf '%s\n' "$OMEGA_PATH_BETA_VALIDATION" ;;
    gamma) printf '%s\n' "$OMEGA_PATH_GAMMA" ;;
    delta) printf '%s\n' "$OMEGA_PATH_DELTA" ;;
    omega) printf '%s\n' "$OMEGA_PATH_OMEGA" ;;
    omega-psi) printf '%s\n' "$OMEGA_PATH_OMEGA_PSI" ;;
    *)
      echo "lattice paths: unknown repository role: $1" >&2
      return 2
      ;;
  esac
}
