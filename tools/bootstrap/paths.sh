#!/usr/bin/env sh
# Canonical repository paths for the bootstrap sequence.
#
# This file is replaceable invocation plumbing. It names source owners only;
# it does not perform a translation or contribute semantics to a compiler edge.

if [ -z "${OMEGA_REPO_ROOT:-}" ] || [ ! -f "$OMEGA_REPO_ROOT/TASKS_BOOTSTRAP.md" ]; then
  echo "bootstrap paths: OMEGA_REPO_ROOT is not an Omega repository root" >&2
  return 2
fi

: "${OMEGA_PATH_SOURCE_ROOT:=$OMEGA_REPO_ROOT/source}"
: "${OMEGA_PATH_LATTICE_TEST_ROOT:=$OMEGA_REPO_ROOT/tests/lattice}"
: "${OMEGA_PATH_BOOTSTRAP_TOOLS_ROOT:=$OMEGA_REPO_ROOT/tools/bootstrap}"

: "${OMEGA_PATH_ALPHA:=$OMEGA_PATH_SOURCE_ROOT/alpha}"
: "${OMEGA_PATH_ALPHA_ASSEMBLER:=$OMEGA_PATH_ALPHA/assembler}"
: "${OMEGA_PATH_BETA:=$OMEGA_PATH_SOURCE_ROOT/beta}"
: "${OMEGA_PATH_BETA_COMPILER:=$OMEGA_PATH_BETA/compiler}"
: "${OMEGA_PATH_BETA_REFERENCE:=$OMEGA_PATH_BETA/reference}"
: "${OMEGA_PATH_BETA_VALIDATION:=$OMEGA_PATH_BETA_COMPILER/validation}"
: "${OMEGA_PATH_GAMMA:=$OMEGA_PATH_SOURCE_ROOT/gamma}"
: "${OMEGA_PATH_DELTA:=$OMEGA_PATH_SOURCE_ROOT/delta}"
: "${OMEGA_PATH_DELTA_MEANING:=$OMEGA_PATH_DELTA/meaning}"
: "${OMEGA_PATH_PROOF_KERNEL:=$OMEGA_PATH_ALPHA/checker}"
: "${OMEGA_PATH_PROOF_KERNEL_GATES:=$OMEGA_PATH_PROOF_KERNEL/gates}"
: "${OMEGA_PATH_PROOF_KERNEL_BETA:=$OMEGA_PATH_PROOF_KERNEL/implementations/beta}"
: "${OMEGA_PATH_PROOF_KERNEL_REFERENCE:=$OMEGA_PATH_PROOF_KERNEL/implementations/reference}"
: "${OMEGA_PATH_PROOF_KERNEL_GAMMA:=$OMEGA_PATH_PROOF_KERNEL/implementations/gamma}"
: "${OMEGA_PATH_CORPUS:=$OMEGA_PATH_LATTICE_TEST_ROOT/corpus}"
: "${OMEGA_PATH_OMEGA_RUST:=$OMEGA_PATH_SOURCE_ROOT/omega-rust}"
: "${OMEGA_PATH_PSI_RUST:=$OMEGA_PATH_OMEGA_RUST/psi}"
: "${OMEGA_PATH_OMEGA_RUST_LOWERING:=$OMEGA_PATH_OMEGA_RUST/omega}"
: "${OMEGA_PATH_PSI_PRODUCT:=$OMEGA_PATH_SOURCE_ROOT/psi}"
: "${OMEGA_PATH_OMEGA_PRODUCT:=$OMEGA_PATH_SOURCE_ROOT/omega}"

export OMEGA_REPO_ROOT OMEGA_PATH_SOURCE_ROOT
export OMEGA_PATH_LATTICE_TEST_ROOT OMEGA_PATH_BOOTSTRAP_TOOLS_ROOT
export OMEGA_PATH_ALPHA OMEGA_PATH_ALPHA_ASSEMBLER OMEGA_PATH_BETA
export OMEGA_PATH_BETA_COMPILER OMEGA_PATH_BETA_REFERENCE
export OMEGA_PATH_BETA_VALIDATION OMEGA_PATH_GAMMA
export OMEGA_PATH_DELTA OMEGA_PATH_DELTA_MEANING
export OMEGA_PATH_PROOF_KERNEL OMEGA_PATH_PROOF_KERNEL_GATES
export OMEGA_PATH_PROOF_KERNEL_BETA OMEGA_PATH_PROOF_KERNEL_REFERENCE
export OMEGA_PATH_PROOF_KERNEL_GAMMA OMEGA_PATH_CORPUS
export OMEGA_PATH_OMEGA_RUST OMEGA_PATH_PSI_RUST OMEGA_PATH_OMEGA_RUST_LOWERING
export OMEGA_PATH_PSI_PRODUCT OMEGA_PATH_OMEGA_PRODUCT

# bootstrap_path ROLE
# Print one canonical role path. The function name belongs to the runner; it
# does not name a compiler artifact.
bootstrap_path() {
  case "$1" in
    source) printf '%s\n' "$OMEGA_PATH_SOURCE_ROOT" ;;
    lattice-tests) printf '%s\n' "$OMEGA_PATH_LATTICE_TEST_ROOT" ;;
    bootstrap-tools) printf '%s\n' "$OMEGA_PATH_BOOTSTRAP_TOOLS_ROOT" ;;
    alpha) printf '%s\n' "$OMEGA_PATH_ALPHA" ;;
    alpha-assembler) printf '%s\n' "$OMEGA_PATH_ALPHA_ASSEMBLER" ;;
    beta) printf '%s\n' "$OMEGA_PATH_BETA" ;;
    beta-compiler) printf '%s\n' "$OMEGA_PATH_BETA_COMPILER" ;;
    beta-reference) printf '%s\n' "$OMEGA_PATH_BETA_REFERENCE" ;;
    beta-validation) printf '%s\n' "$OMEGA_PATH_BETA_VALIDATION" ;;
    gamma) printf '%s\n' "$OMEGA_PATH_GAMMA" ;;
    delta) printf '%s\n' "$OMEGA_PATH_DELTA" ;;
    delta-meaning) printf '%s\n' "$OMEGA_PATH_DELTA_MEANING" ;;
    proof-kernel) printf '%s\n' "$OMEGA_PATH_PROOF_KERNEL" ;;
    proof-kernel-gates) printf '%s\n' "$OMEGA_PATH_PROOF_KERNEL_GATES" ;;
    proof-kernel-beta) printf '%s\n' "$OMEGA_PATH_PROOF_KERNEL_BETA" ;;
    proof-kernel-reference) printf '%s\n' "$OMEGA_PATH_PROOF_KERNEL_REFERENCE" ;;
    proof-kernel-gamma) printf '%s\n' "$OMEGA_PATH_PROOF_KERNEL_GAMMA" ;;
    corpus) printf '%s\n' "$OMEGA_PATH_CORPUS" ;;
    omega-rust) printf '%s\n' "$OMEGA_PATH_OMEGA_RUST" ;;
    omega-rust/*) printf '%s/%s\n' "$OMEGA_PATH_OMEGA_RUST" "${1#omega-rust/}" ;;
    psi-rust) printf '%s\n' "$OMEGA_PATH_PSI_RUST" ;;
    psi-rust/*) printf '%s/%s\n' "$OMEGA_PATH_PSI_RUST" "${1#psi-rust/}" ;;
    omega-rust-lowering) printf '%s\n' "$OMEGA_PATH_OMEGA_RUST_LOWERING" ;;
    psi) printf '%s\n' "$OMEGA_PATH_PSI_PRODUCT" ;;
    psi/*) printf '%s/%s\n' "$OMEGA_PATH_PSI_PRODUCT" "${1#psi/}" ;;
    omega) printf '%s\n' "$OMEGA_PATH_OMEGA_PRODUCT" ;;
    omega/*) printf '%s/%s\n' "$OMEGA_PATH_OMEGA_PRODUCT" "${1#omega/}" ;;
    /*) printf '%s\n' "$1" ;;
    *)
      echo "bootstrap paths: unknown repository role: $1" >&2
      return 2
      ;;
  esac
}
