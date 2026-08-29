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
OMEGA_PATH_LATTICE_TOOLS_ROOT=$OMEGA_REPO_ROOT/tools/lattice

OMEGA_PATH_ALPHA=$OMEGA_PATH_SOURCE_ROOT/alpha
OMEGA_PATH_ALPHA_ASSEMBLER=$OMEGA_PATH_ALPHA/assembler
OMEGA_PATH_BETA=$OMEGA_PATH_SOURCE_ROOT/beta
OMEGA_PATH_BETA_COMPILER=$OMEGA_PATH_BETA/compiler
OMEGA_PATH_BETA_REFERENCE=$OMEGA_PATH_BETA/reference
OMEGA_PATH_BETA_VALIDATION=$OMEGA_PATH_BETA_COMPILER/validation
OMEGA_PATH_BETA_COMPILER_SOURCE=$OMEGA_PATH_BETA_COMPILER/beta_compiler.alpha
OMEGA_PATH_BETA_COMPILER_TAPE=$OMEGA_PATH_BETA_COMPILER/beta_compiler_bytecode.tape
OMEGA_PATH_GAMMA=$OMEGA_PATH_SOURCE_ROOT/gamma
OMEGA_PATH_GAMMA_COMPILER=$OMEGA_PATH_GAMMA/compiler
OMEGA_PATH_GAMMA_COMPILER_SOURCE=$OMEGA_PATH_GAMMA_COMPILER/gamma_compiler.beta
OMEGA_PATH_GAMMA_COMPILER_TAPE=$OMEGA_PATH_GAMMA_COMPILER/gamma_compiler_bytecode.tape
OMEGA_PATH_DELTA=$OMEGA_PATH_SOURCE_ROOT/delta
OMEGA_PATH_DELTA_COMPILER=$OMEGA_PATH_DELTA/compiler
OMEGA_PATH_DELTA_COMPILER_SOURCE=$OMEGA_PATH_DELTA_COMPILER/delta_compiler.gamma
OMEGA_PATH_DELTA_COMPILER_TAPE=$OMEGA_PATH_DELTA_COMPILER/delta_compiler_bytecode.tape
OMEGA_PATH_OMEGA=$OMEGA_PATH_SOURCE_ROOT/omega
OMEGA_PATH_OMEGA_D_SOURCE=$OMEGA_PATH_OMEGA/omega_compiler.delta
OMEGA_PATH_OMEGA0_COMPILER_TAPE=$OMEGA_PATH_OMEGA/omega0_compiler_bytecode.tape
OMEGA_PATH_OMEGA_C_BUILD=$OMEGA_PATH_OMEGA/build.omg
OMEGA_PATH_OMEGA_C_MAIN=$OMEGA_PATH_OMEGA/main.omg
OMEGA_PATH_OMEGA_COMPILER_TAPE=$OMEGA_PATH_OMEGA/omega_compiler_bytecode.tape
OMEGA_PATH_ALPHA_CHECKER=$OMEGA_PATH_ALPHA/checker

export OMEGA_REPO_ROOT OMEGA_PATH_SOURCE_ROOT
export OMEGA_PATH_LATTICE_TOOLS_ROOT
export OMEGA_PATH_ALPHA OMEGA_PATH_ALPHA_ASSEMBLER OMEGA_PATH_BETA
export OMEGA_PATH_BETA_COMPILER OMEGA_PATH_BETA_REFERENCE
export OMEGA_PATH_BETA_VALIDATION OMEGA_PATH_BETA_COMPILER_SOURCE
export OMEGA_PATH_BETA_COMPILER_TAPE OMEGA_PATH_GAMMA OMEGA_PATH_GAMMA_COMPILER
export OMEGA_PATH_GAMMA_COMPILER_SOURCE OMEGA_PATH_GAMMA_COMPILER_TAPE
export OMEGA_PATH_DELTA OMEGA_PATH_DELTA_COMPILER OMEGA_PATH_DELTA_COMPILER_SOURCE
export OMEGA_PATH_DELTA_COMPILER_TAPE OMEGA_PATH_OMEGA OMEGA_PATH_OMEGA_D_SOURCE
export OMEGA_PATH_OMEGA0_COMPILER_TAPE OMEGA_PATH_OMEGA_C_BUILD
export OMEGA_PATH_OMEGA_C_MAIN OMEGA_PATH_OMEGA_COMPILER_TAPE
export OMEGA_PATH_ALPHA_CHECKER

# lattice_path ROLE
# Print one canonical chain-owner path. Alpha services, language references,
# validation directories, Psi, and this runner have shared location variables
# above but are deliberately not compiler-chain roles.
lattice_path() {
  case "$1" in
    alpha) printf '%s\n' "$OMEGA_PATH_ALPHA" ;;
    beta-compiler) printf '%s\n' "$OMEGA_PATH_BETA_COMPILER" ;;
    gamma-compiler) printf '%s\n' "$OMEGA_PATH_GAMMA_COMPILER" ;;
    delta-compiler) printf '%s\n' "$OMEGA_PATH_DELTA_COMPILER" ;;
    omega) printf '%s\n' "$OMEGA_PATH_OMEGA" ;;
    *)
      echo "lattice paths: unknown repository role: $1" >&2
      return 2
      ;;
  esac
}
