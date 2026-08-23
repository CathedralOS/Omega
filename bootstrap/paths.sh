#!/usr/bin/env sh
# Canonical repository paths for bootstrap gates.
#
# Callers must set OMEGA_REPO_ROOT to the repository root before sourcing this
# file.  Gate scripts discover that root by walking upward from their own
# location, so the gates remain callable from any working directory and retain
# that behavior when their owning directory moves.

if [ -z "${OMEGA_REPO_ROOT:-}" ] || [ ! -f "$OMEGA_REPO_ROOT/TASKS_BOOTSTRAP.md" ]; then
  echo "bootstrap paths: OMEGA_REPO_ROOT is not an Omega repository root" >&2
  return 2
fi

: "${OMEGA_PATH_COMPILER_ROOT:=$OMEGA_REPO_ROOT/compiler}"
: "${OMEGA_PATH_BOOTSTRAP_ROOT:=$OMEGA_REPO_ROOT/bootstrap}"
: "${OMEGA_PATH_RUNGS_ROOT:=$OMEGA_PATH_BOOTSTRAP_ROOT/rungs}"
: "${OMEGA_PATH_ASSURANCE_ROOT:=$OMEGA_PATH_BOOTSTRAP_ROOT/assurance}"

# Current locations, named by architectural role.  Ownership migrations update
# this manifest; gates do not encode the repository topology themselves.
: "${OMEGA_PATH_ALPHA:=$OMEGA_PATH_RUNGS_ROOT/alpha}"
: "${OMEGA_PATH_ALPHA_ASSEMBLER:=$OMEGA_PATH_ALPHA/assembler}"
# Historical compatibility name.  The assembler is an Alpha-tier tool; new
# plumbing should use OMEGA_PATH_ALPHA_ASSEMBLER / the alpha-assembler role.
: "${OMEGA_PATH_BETA_ASSEMBLER:=$OMEGA_PATH_ALPHA_ASSEMBLER}"
: "${OMEGA_PATH_BETA:=${OMEGA_PATH_BETA_LANGUAGE:-$OMEGA_PATH_RUNGS_ROOT/beta}}"
# Historical compatibility name.  New plumbing should use OMEGA_PATH_BETA /
# the beta role for the language and its self-hosting compiler.
: "${OMEGA_PATH_BETA_LANGUAGE:=$OMEGA_PATH_BETA}"
: "${OMEGA_PATH_BETA_RUST:=$OMEGA_PATH_COMPILER_ROOT/beta-lang-rs}"
: "${OMEGA_PATH_BETA_REFERENCE:=$OMEGA_PATH_BETA/reference}"
: "${OMEGA_PATH_BETA_REFINEMENT:=$OMEGA_PATH_ASSURANCE_ROOT/refinement/beta}"
: "${OMEGA_PATH_GAMMA:=$OMEGA_PATH_COMPILER_ROOT/gamma}"
: "${OMEGA_PATH_DELTA:=$OMEGA_PATH_COMPILER_ROOT/delta-rs}"
: "${OMEGA_PATH_DELTA_RUST:=$OMEGA_PATH_COMPILER_ROOT/delta-rs}"
: "${OMEGA_PATH_PROOF_KERNEL:=$OMEGA_PATH_ASSURANCE_ROOT/proof-kernel}"
: "${OMEGA_PATH_PROOF_KERNEL_GATES:=$OMEGA_PATH_PROOF_KERNEL/gates}"
: "${OMEGA_PATH_PROOF_KERNEL_BETA:=$OMEGA_PATH_PROOF_KERNEL/implementations/beta}"
: "${OMEGA_PATH_PROOF_KERNEL_REFERENCE:=$OMEGA_PATH_PROOF_KERNEL/implementations/reference}"
: "${OMEGA_PATH_PROOF_KERNEL_GAMMA:=$OMEGA_PATH_PROOF_KERNEL/implementations/gamma}"
: "${OMEGA_PATH_OMEGA0:=$OMEGA_PATH_BOOTSTRAP_ROOT/omega0}"
: "${OMEGA_PATH_OMEGA0_MEANING:=$OMEGA_PATH_OMEGA0/meaning}"
: "${OMEGA_PATH_OMEGA0_COMPILER:=$OMEGA_PATH_OMEGA0/compiler}"
: "${OMEGA_PATH_OMEGA0_GATES:=$OMEGA_PATH_OMEGA0/gates}"
: "${OMEGA_PATH_CORPUS:=$OMEGA_PATH_COMPILER_ROOT/lattice-corpus}"
: "${OMEGA_PATH_PSI_PRODUCT:=$OMEGA_PATH_COMPILER_ROOT/psi-rs}"
: "${OMEGA_PATH_OMEGA_PRODUCT:=$OMEGA_PATH_COMPILER_ROOT/omega-rs}"

export OMEGA_REPO_ROOT OMEGA_PATH_COMPILER_ROOT OMEGA_PATH_BOOTSTRAP_ROOT
export OMEGA_PATH_RUNGS_ROOT OMEGA_PATH_ASSURANCE_ROOT
export OMEGA_PATH_ALPHA OMEGA_PATH_ALPHA_ASSEMBLER
export OMEGA_PATH_BETA_ASSEMBLER OMEGA_PATH_BETA OMEGA_PATH_BETA_LANGUAGE
export OMEGA_PATH_BETA_RUST OMEGA_PATH_BETA_REFERENCE OMEGA_PATH_BETA_REFINEMENT OMEGA_PATH_GAMMA
export OMEGA_PATH_DELTA OMEGA_PATH_DELTA_RUST OMEGA_PATH_PROOF_KERNEL OMEGA_PATH_PROOF_KERNEL_GATES
export OMEGA_PATH_PROOF_KERNEL_BETA OMEGA_PATH_PROOF_KERNEL_REFERENCE OMEGA_PATH_PROOF_KERNEL_GAMMA
export OMEGA_PATH_OMEGA0 OMEGA_PATH_CORPUS
export OMEGA_PATH_OMEGA0_MEANING OMEGA_PATH_OMEGA0_COMPILER OMEGA_PATH_OMEGA0_GATES
export OMEGA_PATH_PSI_PRODUCT OMEGA_PATH_OMEGA_PRODUCT

# omega_bootstrap_path ROLE
# Print a canonical role path.  Subpaths on product roots are accepted for the
# top-level lattice driver's dependency hashing.
omega_bootstrap_path() {
  case "$1" in
    compiler) printf '%s\n' "$OMEGA_PATH_COMPILER_ROOT" ;;
    alpha) printf '%s\n' "$OMEGA_PATH_ALPHA" ;;
    alpha-assembler) printf '%s\n' "$OMEGA_PATH_ALPHA_ASSEMBLER" ;;
    beta-assembler) printf '%s\n' "$OMEGA_PATH_BETA_ASSEMBLER" ;;
    beta) printf '%s\n' "$OMEGA_PATH_BETA" ;;
    beta-lang) printf '%s\n' "$OMEGA_PATH_BETA_LANGUAGE" ;;
    beta-lang-rs) printf '%s\n' "$OMEGA_PATH_BETA_RUST" ;;
    beta-reference|beta-lang-py) printf '%s\n' "$OMEGA_PATH_BETA_REFERENCE" ;;
    beta-refinement) printf '%s\n' "$OMEGA_PATH_BETA_REFINEMENT" ;;
    gamma) printf '%s\n' "$OMEGA_PATH_GAMMA" ;;
    delta) printf '%s\n' "$OMEGA_PATH_DELTA" ;;
    delta-rs) printf '%s\n' "$OMEGA_PATH_DELTA_RUST" ;;
    proof-kernel) printf '%s\n' "$OMEGA_PATH_PROOF_KERNEL" ;;
    proof-kernel-gates) printf '%s\n' "$OMEGA_PATH_PROOF_KERNEL_GATES" ;;
    proof-kernel-beta) printf '%s\n' "$OMEGA_PATH_PROOF_KERNEL_BETA" ;;
    proof-kernel-reference) printf '%s\n' "$OMEGA_PATH_PROOF_KERNEL_REFERENCE" ;;
    proof-kernel-gamma) printf '%s\n' "$OMEGA_PATH_PROOF_KERNEL_GAMMA" ;;
    omega0|omega) printf '%s\n' "$OMEGA_PATH_OMEGA0" ;;
    omega0-meaning) printf '%s\n' "$OMEGA_PATH_OMEGA0_MEANING" ;;
    omega0-compiler) printf '%s\n' "$OMEGA_PATH_OMEGA0_COMPILER" ;;
    omega0-gates) printf '%s\n' "$OMEGA_PATH_OMEGA0_GATES" ;;
    corpus|lattice-corpus) printf '%s\n' "$OMEGA_PATH_CORPUS" ;;
    psi) printf '%s\n' "$OMEGA_PATH_PSI_PRODUCT" ;;
    psi/*) printf '%s/%s\n' "$OMEGA_PATH_PSI_PRODUCT" "${1#psi/}" ;;
    omega-product) printf '%s\n' "$OMEGA_PATH_OMEGA_PRODUCT" ;;
    omega-product/*) printf '%s/%s\n' "$OMEGA_PATH_OMEGA_PRODUCT" "${1#omega-product/}" ;;
    /*) printf '%s\n' "$1" ;;
    *)
      echo "bootstrap paths: unknown repository role: $1" >&2
      return 2
      ;;
  esac
}
