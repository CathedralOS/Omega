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

step "alpha — seed (provenance + behavior + reproduction)" alpha verify.sh
step "alpha — reference VM agrees with the host realization" alpha diamond-py.sh
step "alpha — VM fuzz" alpha vm-fuzz.sh
step "alpha — assembler self-hosts" alpha-assembler selfhost.sh
step "alpha — reference assembler agreement" alpha-assembler asm-diamond.sh beta proof-kernel
step "alpha — register and label regression" alpha-assembler register-label-regression.sh alpha beta

step "beta — Alpha-written cold-start compiler surface" beta cold-start/test.sh alpha alpha-assembler
step "beta — Alpha-rooted full source and artifact fixed point" beta cold-start/full-source.sh alpha alpha-assembler
step "beta — lower-rooted artifact obligations" beta-refinement bc-artifact-structure.sh alpha beta alpha-assembler
step "beta — self-host" beta selfhost.sh
step "beta — per-feature gate" beta test.sh
step "beta — checked compiler resource profile" beta source-exhaustion.sh alpha-assembler
step "beta — reference correctness fuzz" beta-reference beta-correctness-fuzz.sh beta alpha-assembler
step "beta — exhaustive input agreement" beta-reference beta-io-exhaust.sh beta alpha-assembler

step "proof kernel — certificate checker" proof-kernel-gates test.sh
step "proof kernel — soundness battery" proof-kernel-gates soundness.sh
step "proof kernel — reference agreement" proof-kernel-gates check-ref-diamond.sh beta alpha-assembler

step "gamma — reference interpreter" gamma test-interp.sh
step "gamma — meaning cross-check" gamma gamma-diamond-py.sh beta alpha-assembler
step "gamma — static type checker" gamma test-typeck.sh
step "gamma — canonical-byte decoder" gamma test-canonical-bytes.sh

step "proof kernel — Gamma implementation" proof-kernel-gates gamma-checker.sh gamma
step "proof kernel — checker agreement" proof-kernel-gates checker-diamond.sh gamma
step "proof kernel — definitional equality seam" proof-kernel-gates semantics-diamond.sh gamma
step "proof kernel — induction seam" proof-kernel-gates induction-soundness.sh gamma
step "proof kernel — predicate seam" proof-kernel-gates predicate-soundness.sh gamma
step "proof kernel — logic seam" proof-kernel-gates logic-soundness.sh gamma
step "proof kernel — corpus soundness" proof-kernel-gates soundness-sweep.sh gamma
step "proof kernel — evaluator fuzz" proof-kernel-gates seam-fuzz.sh gamma
step "proof kernel — recursive accumulator seam" proof-kernel-gates recx-soundness.sh gamma alpha-assembler beta
step "proof kernel — product eliminator seam" proof-kernel-gates prodrec-seam.sh gamma alpha-assembler beta
step "proof kernel — Omega contract discharge" proof-kernel-gates math-contracts.sh gamma alpha-assembler beta corpus
step "proof kernel — Omega termination discharge" proof-kernel-gates termination-obligations.sh gamma alpha-assembler beta corpus
step "proof kernel — universal theorem" proof-kernel-gates forall-input.sh gamma alpha-assembler beta
step "proof kernel — universal sample connection" proof-kernel-gates forall-sample.sh gamma alpha-assembler beta corpus
step "proof kernel — checker fuzz" proof-kernel-gates checker-diamond-fuzz.sh gamma
step "proof kernel — logic fuzz" proof-kernel-gates logic-diamond-fuzz.sh gamma
step "proof kernel — predicate agreement fuzz" proof-kernel-gates predicate-diamond-fuzz.sh gamma
step "proof kernel — predicate soundness fuzz" proof-kernel-gates predicate-soundness-fuzz.sh gamma

step "delta — exact path-independent compiler source closure" delta source-closure-snapshot-v1.sh
step "delta — lower-rooted publication receipt model" delta lower-rooted-assembly-publication-v1-test.sh alpha-assembler beta gamma

if command -v python3 >/dev/null 2>&1; then
  step "tool — proof elaborator" proof-kernel-gates elab-test.sh gamma
  step "tool — proof-library cross-check" proof-kernel-gates proofs-crosscheck.sh gamma alpha-assembler beta
  step "tool — elaborator round-trip" proof-kernel-gates delab-roundtrip.sh gamma
  step "tool — proof search" proof-kernel-gates prover-test.sh gamma
  step "tool — prover certificate agreement" proof-kernel-gates prover-diamond.sh gamma
fi

echo ""
echo "BOOTSTRAP GATES PASS — Alpha → Beta → Gamma; Delta publication and direct Ωself build remain open"
