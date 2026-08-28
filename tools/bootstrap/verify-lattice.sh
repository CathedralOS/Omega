#!/usr/bin/env sh
# Convenience runner for the independently invocable bootstrap gates.
#
# This script contributes no compiler semantics. Every row below is an ordinary
# command that can be run directly; this file merely orders them and stops at
# the first failure.
set -eu

OMEGA_GATE_DIR=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd -P)
OMEGA_REPO_ROOT=$(CDPATH= cd -- "$OMEGA_GATE_DIR/../.." && pwd -P)
export OMEGA_REPO_ROOT
. "$OMEGA_GATE_DIR/paths.sh"

step() { # label owner-role script [documentary dependency roles...]
  label=$1
  role=$2
  script=$3
  directory=$(bootstrap_path "$role")
  printf '\n=== %s ===\n' "$label"
  printf 'command: (cd "%s" && sh "./%s")\n' "$directory" "$script"
  if (cd "$directory" && sh "./$script"); then
    return 0
  else
    status=$?
  fi
  printf 'FAILED (%s): (cd "%s" && sh "./%s")\n' \
    "$status" "$directory" "$script" >&2
  return "$status"
}

step "bootstrap — canonical path hygiene" bootstrap-tools check-path-hygiene.sh

step "alpha — accepted seed and assembler reproduction" alpha verify.sh
step "alpha — assembler fixed point" alpha-assembler selfhost.sh
step "alpha — below-Beta checker construction" alpha-checker reconstruct-artifact.sh alpha alpha-assembler

step "beta — Alpha-rooted compiler construction" beta-compiler cold-start/full-source.sh alpha alpha-assembler
step "beta — compiler artifact framing" beta-validation bc-artifact-structure.sh alpha beta-compiler alpha-assembler
step "beta — maximal-observation reconstruction" beta-validation bc-block-control.sh alpha beta-compiler alpha-assembler
step "beta — proof-carrying instruction refinement" beta-validation refinement.sh alpha alpha-checker beta-compiler

step "gamma — interpreter" gamma test-interp.sh beta beta-compiler
step "gamma — type checker" gamma test-typeck.sh beta beta-compiler
step "gamma — canonical bytes" gamma test-canonical-bytes.sh beta beta-compiler

step "delta — exact compiler source closure" delta source-closure-snapshot-v1.sh
step "delta — lower-rooted publication model" delta lower-rooted-assembly-publication-v1-test.sh alpha-assembler beta-compiler gamma

echo ""
echo "DIRECT LATTICE GATES PASS — Alpha → Beta → Gamma; Delta publication and direct Omega build remain open"
