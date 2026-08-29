#!/usr/bin/env sh
# Convenience runner for the independently invocable compiler-lattice gates.
#
# This script contributes no compiler semantics. Every row below is an ordinary
# command that can be run directly; this file merely orders them and stops at
# the first failure.
set -eu

OMEGA_GATE_DIR=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd -P)
OMEGA_REPO_ROOT=$(CDPATH= cd -- "$OMEGA_GATE_DIR/../.." && pwd -P)
export OMEGA_REPO_ROOT
. "$OMEGA_GATE_DIR/paths.sh"

step() { # label owner-role script [arguments...]
  label=$1
  role=$2
  script=$3
  shift 3
  directory=$(lattice_path "$role")
  printf '\n=== %s ===\n' "$label"
  printf 'command: (cd "%s" && sh "./%s"' "$directory" "$script"
  for argument in "$@"; do
    printf ' "%s"' "$argument"
  done
  printf ')\n'
  if (cd "$directory" && sh "./$script" "$@"); then
    return 0
  else
    status=$?
  fi
  printf 'FAILED (%s): %s/%s\n' "$status" "$directory" "$script" >&2
  return "$status"
}

step "alpha — accepted seed and assembler reproduction" alpha verify.sh
step "alpha — below-Beta checker construction" alpha-checker reconstruct-artifact.sh

step "beta — Alpha-rooted compiler construction" beta-compiler cold-start/rebuild-artifact.sh --check
step "beta — maximal-observation reconstruction" beta-validation admission/bc-block-control.sh

echo ""
echo "DIRECT LATTICE GATES PASS — Alpha → bc; Gamma/Delta production and both Omega builds remain open"
