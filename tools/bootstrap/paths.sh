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

: "${OMEGA_PATH_SOURCE_ROOT:=$OMEGA_REPO_ROOT/source}"
: "${OMEGA_PATH_ON_RAMP_ROOT:=$OMEGA_PATH_SOURCE_ROOT/on-ramp}"
: "${OMEGA_PATH_REFINEMENT_ROOT:=$OMEGA_PATH_SOURCE_ROOT/refinement}"
: "${OMEGA_PATH_LATTICE_TEST_ROOT:=$OMEGA_REPO_ROOT/tests/lattice}"
: "${OMEGA_PATH_BOOTSTRAP_TOOLS_ROOT:=$OMEGA_REPO_ROOT/tools/bootstrap}"

# Current locations, named by architectural role.  Ownership migrations update
# this manifest; gates do not encode the repository topology themselves.
: "${OMEGA_PATH_ALPHA:=$OMEGA_PATH_SOURCE_ROOT/alpha}"
: "${OMEGA_PATH_ALPHA_ASSEMBLER:=$OMEGA_PATH_ALPHA/assembler}"
: "${OMEGA_PATH_BETA:=$OMEGA_PATH_SOURCE_ROOT/beta}"
: "${OMEGA_PATH_BETA_REFERENCE:=$OMEGA_PATH_BETA/reference}"
: "${OMEGA_PATH_BETA_REFINEMENT:=$OMEGA_PATH_REFINEMENT_ROOT/alpha-beta}"
: "${OMEGA_PATH_GAMMA:=$OMEGA_PATH_SOURCE_ROOT/gamma}"
: "${OMEGA_PATH_DELTA:=$OMEGA_PATH_SOURCE_ROOT/delta}"
: "${OMEGA_PATH_PROOF_KERNEL:=$OMEGA_PATH_SOURCE_ROOT/proof-kernel}"
: "${OMEGA_PATH_PROOF_KERNEL_GATES:=$OMEGA_PATH_PROOF_KERNEL/gates}"
: "${OMEGA_PATH_PROOF_KERNEL_BETA:=$OMEGA_PATH_PROOF_KERNEL/implementations/beta}"
: "${OMEGA_PATH_PROOF_KERNEL_REFERENCE:=$OMEGA_PATH_PROOF_KERNEL/implementations/reference}"
: "${OMEGA_PATH_PROOF_KERNEL_GAMMA:=$OMEGA_PATH_PROOF_KERNEL/implementations/gamma}"
# Canonical Delta-built bridge owner. The historical Omega0 variables remain
# accepted aliases for callers that source this manifest directly.
: "${OMEGA_PATH_OMEGA_BOOTSTRAP:=${OMEGA_PATH_OMEGA0:-$OMEGA_PATH_ON_RAMP_ROOT/omega-bootstrap}}"
: "${OMEGA_PATH_OMEGA_BOOTSTRAP_MEANING:=${OMEGA_PATH_OMEGA0_MEANING:-$OMEGA_PATH_OMEGA_BOOTSTRAP/meaning}}"
: "${OMEGA_PATH_OMEGA_BOOTSTRAP_COMPILER:=${OMEGA_PATH_OMEGA0_COMPILER:-$OMEGA_PATH_OMEGA_BOOTSTRAP/compiler}}"
: "${OMEGA_PATH_OMEGA_BOOTSTRAP_GATES:=${OMEGA_PATH_OMEGA0_GATES:-$OMEGA_PATH_OMEGA_BOOTSTRAP/gates}}"
: "${OMEGA_PATH_OMEGA_BOOTSTRAP_REFINEMENT:=${OMEGA_PATH_OMEGA0_REFINEMENT:-$OMEGA_PATH_REFINEMENT_ROOT/delta-omega-bootstrap}}"
: "${OMEGA_PATH_OMEGA0:=$OMEGA_PATH_OMEGA_BOOTSTRAP}"
: "${OMEGA_PATH_OMEGA0_MEANING:=$OMEGA_PATH_OMEGA_BOOTSTRAP_MEANING}"
: "${OMEGA_PATH_OMEGA0_COMPILER:=$OMEGA_PATH_OMEGA_BOOTSTRAP_COMPILER}"
: "${OMEGA_PATH_OMEGA0_GATES:=$OMEGA_PATH_OMEGA_BOOTSTRAP_GATES}"
: "${OMEGA_PATH_OMEGA0_REFINEMENT:=$OMEGA_PATH_OMEGA_BOOTSTRAP_REFINEMENT}"
: "${OMEGA_PATH_CORPUS:=$OMEGA_PATH_LATTICE_TEST_ROOT/corpus}"
: "${OMEGA_PATH_OMEGA_RUST_ONRAMP_ROOT:=$OMEGA_PATH_ON_RAMP_ROOT/rust}"
: "${OMEGA_PATH_PSI_RUST:=$OMEGA_PATH_OMEGA_RUST_ONRAMP_ROOT/psi}"
: "${OMEGA_PATH_OMEGA_RUST:=$OMEGA_PATH_OMEGA_RUST_ONRAMP_ROOT/omega}"
: "${OMEGA_PATH_PSI_PRODUCT:=$OMEGA_PATH_SOURCE_ROOT/psi}"
: "${OMEGA_PATH_OMEGA_PRODUCT:=$OMEGA_PATH_SOURCE_ROOT/omega}"
: "${OMEGA_PATH_PRODUCT_SOURCE_CHECKPOINTS:=$OMEGA_PATH_OMEGA_PRODUCT/source-checkpoints}"

export OMEGA_REPO_ROOT OMEGA_PATH_SOURCE_ROOT OMEGA_PATH_ON_RAMP_ROOT
export OMEGA_PATH_REFINEMENT_ROOT OMEGA_PATH_LATTICE_TEST_ROOT OMEGA_PATH_BOOTSTRAP_TOOLS_ROOT
export OMEGA_PATH_ALPHA OMEGA_PATH_ALPHA_ASSEMBLER
export OMEGA_PATH_BETA
export OMEGA_PATH_BETA_REFERENCE OMEGA_PATH_BETA_REFINEMENT OMEGA_PATH_GAMMA
export OMEGA_PATH_DELTA OMEGA_PATH_PROOF_KERNEL OMEGA_PATH_PROOF_KERNEL_GATES
export OMEGA_PATH_PROOF_KERNEL_BETA OMEGA_PATH_PROOF_KERNEL_REFERENCE OMEGA_PATH_PROOF_KERNEL_GAMMA
export OMEGA_PATH_OMEGA_BOOTSTRAP OMEGA_PATH_CORPUS
export OMEGA_PATH_OMEGA_BOOTSTRAP_MEANING OMEGA_PATH_OMEGA_BOOTSTRAP_COMPILER
export OMEGA_PATH_OMEGA_BOOTSTRAP_GATES OMEGA_PATH_OMEGA_BOOTSTRAP_REFINEMENT
export OMEGA_PATH_OMEGA0 OMEGA_PATH_OMEGA0_MEANING OMEGA_PATH_OMEGA0_COMPILER
export OMEGA_PATH_OMEGA0_GATES OMEGA_PATH_OMEGA0_REFINEMENT
export OMEGA_PATH_OMEGA_RUST_ONRAMP_ROOT OMEGA_PATH_PSI_RUST OMEGA_PATH_OMEGA_RUST
export OMEGA_PATH_PSI_PRODUCT OMEGA_PATH_OMEGA_PRODUCT OMEGA_PATH_PRODUCT_SOURCE_CHECKPOINTS

# omega_bootstrap_path ROLE
# Print a canonical role path.  Subpaths on product roots are accepted for the
# top-level lattice driver's dependency hashing.
omega_bootstrap_path() {
  case "$1" in
    source) printf '%s\n' "$OMEGA_PATH_SOURCE_ROOT" ;;
    on-ramp) printf '%s\n' "$OMEGA_PATH_ON_RAMP_ROOT" ;;
    lattice-tests) printf '%s\n' "$OMEGA_PATH_LATTICE_TEST_ROOT" ;;
    bootstrap-tools) printf '%s\n' "$OMEGA_PATH_BOOTSTRAP_TOOLS_ROOT" ;;
    alpha) printf '%s\n' "$OMEGA_PATH_ALPHA" ;;
    alpha-assembler) printf '%s\n' "$OMEGA_PATH_ALPHA_ASSEMBLER" ;;
    beta) printf '%s\n' "$OMEGA_PATH_BETA" ;;
    beta-reference) printf '%s\n' "$OMEGA_PATH_BETA_REFERENCE" ;;
    refinement) printf '%s\n' "$OMEGA_PATH_REFINEMENT_ROOT" ;;
    beta-refinement) printf '%s\n' "$OMEGA_PATH_BETA_REFINEMENT" ;;
    omega-bootstrap-refinement|omega0-refinement) printf '%s\n' "$OMEGA_PATH_OMEGA_BOOTSTRAP_REFINEMENT" ;;
    gamma) printf '%s\n' "$OMEGA_PATH_GAMMA" ;;
    delta) printf '%s\n' "$OMEGA_PATH_DELTA" ;;
    proof-kernel) printf '%s\n' "$OMEGA_PATH_PROOF_KERNEL" ;;
    proof-kernel-gates) printf '%s\n' "$OMEGA_PATH_PROOF_KERNEL_GATES" ;;
    proof-kernel-beta) printf '%s\n' "$OMEGA_PATH_PROOF_KERNEL_BETA" ;;
    proof-kernel-reference) printf '%s\n' "$OMEGA_PATH_PROOF_KERNEL_REFERENCE" ;;
    proof-kernel-gamma) printf '%s\n' "$OMEGA_PATH_PROOF_KERNEL_GAMMA" ;;
    omega-bootstrap|omega0) printf '%s\n' "$OMEGA_PATH_OMEGA_BOOTSTRAP" ;;
    omega-bootstrap-meaning|omega0-meaning) printf '%s\n' "$OMEGA_PATH_OMEGA_BOOTSTRAP_MEANING" ;;
    omega-bootstrap-compiler|omega0-compiler) printf '%s\n' "$OMEGA_PATH_OMEGA_BOOTSTRAP_COMPILER" ;;
    omega-bootstrap-gates|omega0-gates) printf '%s\n' "$OMEGA_PATH_OMEGA_BOOTSTRAP_GATES" ;;
    corpus) printf '%s\n' "$OMEGA_PATH_CORPUS" ;;
    omega-rust-onramp) printf '%s\n' "$OMEGA_PATH_OMEGA_RUST_ONRAMP_ROOT" ;;
    psi-rust) printf '%s\n' "$OMEGA_PATH_PSI_RUST" ;;
    psi-rust/*) printf '%s/%s\n' "$OMEGA_PATH_PSI_RUST" "${1#psi-rust/}" ;;
    omega-rust) printf '%s\n' "$OMEGA_PATH_OMEGA_RUST" ;;
    omega-rust/*) printf '%s/%s\n' "$OMEGA_PATH_OMEGA_RUST" "${1#omega-rust/}" ;;
    psi) printf '%s\n' "$OMEGA_PATH_PSI_PRODUCT" ;;
    psi/*) printf '%s/%s\n' "$OMEGA_PATH_PSI_PRODUCT" "${1#psi/}" ;;
    omega) printf '%s\n' "$OMEGA_PATH_OMEGA_PRODUCT" ;;
    omega/*) printf '%s/%s\n' "$OMEGA_PATH_OMEGA_PRODUCT" "${1#omega/}" ;;
    source-checkpoints) printf '%s\n' "$OMEGA_PATH_PRODUCT_SOURCE_CHECKPOINTS" ;;
    source-checkpoints/*) printf '%s/%s\n' "$OMEGA_PATH_PRODUCT_SOURCE_CHECKPOINTS" "${1#source-checkpoints/}" ;;
    /*) printf '%s\n' "$1" ;;
    *)
      echo "bootstrap paths: unknown repository role: $1" >&2
      return 2
      ;;
  esac
}
