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

step "alpha — seed behavior and exact assembler construction" alpha verify.sh --edge

echo ""
echo "LATTICE FLOOR GATES PASS — canonical Beta, Gamma, Delta, omega₀, and omega edges remain open"
