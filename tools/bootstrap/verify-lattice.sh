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

sh "$OMEGA_GATE_DIR/check-path-hygiene.sh"

step() { # label owner-role script [documentary dependency roles...]
  label=$1
  role=$2
  script=$3
  directory=$(bootstrap_path "$role")
  printf '\n=== %s ===\n' "$label"
  (cd "$directory" && sh "$script")
}

step "alpha — accepted seed and assembler reproduction" alpha verify.sh
step "alpha — assembler fixed point" alpha-assembler selfhost.sh

step "beta — Alpha-rooted compiler construction" beta-compiler cold-start/full-source.sh alpha alpha-assembler
step "beta — compiler artifact framing" beta-validation bc-artifact-structure.sh alpha beta-compiler alpha-assembler

step "gamma — interpreter" gamma test-interp.sh beta beta-compiler
step "gamma — type checker" gamma test-typeck.sh beta beta-compiler
step "gamma — canonical bytes" gamma test-canonical-bytes.sh beta beta-compiler

step "delta — exact compiler source closure" delta source-closure-snapshot-v1.sh
step "delta — lower-rooted publication model" delta lower-rooted-assembly-publication-v1-test.sh alpha-assembler beta-compiler gamma

# Large differential, mutation, fuzz, and proof-corpus campaigns are useful
# diagnostics, not hidden compiler stages. Keep them off the direct path unless
# explicitly requested.
if [ "${LATTICE_STRESS:-0}" = 1 ]; then
  step "stress — Alpha reference VM" alpha diamond-py.sh
  step "stress — Alpha VM fuzz" alpha vm-fuzz.sh
  step "stress — assembler reference agreement" alpha-assembler asm-diamond.sh beta proof-kernel
  step "stress — assembler register/label regression" alpha-assembler register-label-regression.sh alpha beta
  step "stress — Beta self-host" beta selfhost.sh
  step "stress — Beta resource profile" beta source-exhaustion.sh alpha-assembler
  step "stress — Beta reference fuzz" beta-reference beta-correctness-fuzz.sh beta-compiler alpha-assembler
  step "stress — Beta exhaustive input agreement" beta-reference beta-io-exhaust.sh beta-compiler alpha-assembler
  step "stress — Alpha checker regression" proof-kernel-gates test.sh
  step "stress — Alpha checker negative battery" proof-kernel-gates soundness.sh
  step "stress — checker reference agreement" proof-kernel-gates check-ref-diamond.sh beta-compiler alpha-assembler
  step "stress — Gamma meaning cross-check" gamma gamma-diamond-py.sh beta-compiler alpha-assembler
  step "stress — Gamma checker implementation" proof-kernel-gates gamma-checker.sh gamma
  step "stress — checker agreement" proof-kernel-gates checker-diamond.sh gamma
  step "stress — definitional equality seam" proof-kernel-gates semantics-diamond.sh gamma
  step "stress — induction seam" proof-kernel-gates induction-soundness.sh gamma
  step "stress — predicate seam" proof-kernel-gates predicate-soundness.sh gamma
  step "stress — logic seam" proof-kernel-gates logic-soundness.sh gamma
  step "stress — proof corpus" proof-kernel-gates soundness-sweep.sh gamma
  step "stress — evaluator fuzz" proof-kernel-gates seam-fuzz.sh gamma
  step "stress — recursive accumulator seam" proof-kernel-gates recx-soundness.sh gamma alpha-assembler beta-compiler
  step "stress — product eliminator seam" proof-kernel-gates prodrec-seam.sh gamma alpha-assembler beta-compiler
  step "stress — Omega contracts" proof-kernel-gates math-contracts.sh gamma alpha-assembler beta-compiler corpus
  step "stress — Omega termination" proof-kernel-gates termination-obligations.sh gamma alpha-assembler beta-compiler corpus
  step "stress — universal theorem" proof-kernel-gates forall-input.sh gamma alpha-assembler beta-compiler
  step "stress — universal sample" proof-kernel-gates forall-sample.sh gamma alpha-assembler beta-compiler corpus
  step "stress — checker fuzz" proof-kernel-gates checker-diamond-fuzz.sh gamma
  step "stress — logic fuzz" proof-kernel-gates logic-diamond-fuzz.sh gamma
  step "stress — predicate agreement fuzz" proof-kernel-gates predicate-diamond-fuzz.sh gamma
  step "stress — predicate soundness fuzz" proof-kernel-gates predicate-soundness-fuzz.sh gamma

  if command -v python3 >/dev/null 2>&1; then
    step "stress — proof elaborator" proof-kernel-gates elab-test.sh gamma
    step "stress — proof-library cross-check" proof-kernel-gates proofs-crosscheck.sh gamma alpha-assembler beta-compiler
    step "stress — elaborator round-trip" proof-kernel-gates delab-roundtrip.sh gamma
    step "stress — proof search" proof-kernel-gates prover-test.sh gamma
    step "stress — prover agreement" proof-kernel-gates prover-diamond.sh gamma
  fi
fi

echo ""
echo "DIRECT LATTICE GATES PASS — Alpha → Beta → Gamma; Delta publication and direct Omega build remain open"
