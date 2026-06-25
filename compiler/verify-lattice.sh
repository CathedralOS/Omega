#!/usr/bin/env sh
# Verify the whole bootstrap lattice, rung by rung, in one command — from the
# hand-audited seed up to the certificate checker. Each step is the rung's own
# gate; this just runs them in dependency order and stops on the first failure.
#
#   alpha   the seed re-derives from source, conforms to SEMANTICS.md, and the
#           two seeds form a diamond (provenance + behavior + diamond)
#   beta    the assembler self-hosts (reproduces its own bytecode byte-for-byte)
#   Beta    the language compiler (Rust on-ramp) compiles + runs the corpus
#   bc      the Beta compiler WRITTEN IN BETA self-hosts (Rust leaves the lineage)
#   delta   the certificate checker accepts valid proofs, rejects invalid ones
cd "$(dirname "$0")"
fail=0
step() {  # label  dir  script
  printf '\n=== %s ===\n' "$1"
  if ( cd "$2" && sh "$3" ); then :; else echo "FAILED: $1"; fail=1; fi
}
step "alpha — seed (provenance + behavior + diamond)" alpha       verify.sh
step "beta  — assembler self-hosts"                   beta        selfhost.sh
step "Beta  — language compiler (on-ramp) + corpus"   beta-lang-rs test.sh
step "bc    — Beta compiler in Beta self-hosts"       beta-lang   selfhost.sh
step "bc    — per-feature gate"                       beta-lang   test.sh
step "delta — certificate checker"                    delta       test.sh
step "delta — soundness battery (no false proof)"     delta       soundness.sh
step "gamma — reference interpreter (ADTs + match)"   gamma       test-interp.sh
step "gamma — static type checker"                    gamma       test-typeck.sh
step "gamma — the Delta checker, written IN gamma"    gamma       test-checker.sh
step "diamond — two checkers (Beta vs Gamma) agree"   delta       checker-diamond.sh
step "diamond — definitional eq vs operational eval"  delta       semantics-diamond.sh
step "seam — inductive universals vs operational eval" delta      induction-soundness.sh
# untrusted proof elaborator (named binders -> raw certs); skipped if python3 is absent
if command -v python3 >/dev/null 2>&1; then
  step "tool — proof elaborator (named binders -> check.beta)" delta elab-test.sh
  step "tool — elaborator/de-elaborator round-trip on the corpus" delta delab-roundtrip.sh
fi

echo ""
if [ "$fail" = 0 ]; then
  echo "LATTICE VERIFIED ✓ — seed → assembler → bc → checker; + gamma interp running the checker-in-gamma"
else
  echo "LATTICE: one or more rungs FAILED"; exit 1
fi
