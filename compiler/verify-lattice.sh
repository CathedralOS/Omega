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
#   omega   the summit surface: kernel-subset programs run Rust-free, certify
#           their results and safety obligations (macOS arm64 for the native
#           legs; those skip cleanly off-platform)
#
# INCREMENTAL: each step declares its input dirs; a step whose inputs are
# unchanged since its last GREEN run is skipped (content-hash cache in
# .lattice-cache/). So an omega-slice edit re-verifies only the gates it can
# reach, not the 843-case prover battery. LATTICE_FULL=1 forces everything.
# The cache holds only *hashes of inputs of passing runs* — deleting it is
# always safe and merely makes the next run full.
cd "$(dirname "$0")"
fail=0
CACHE=.lattice-cache
mkdir -p "$CACHE"

# content hash of the given dirs/files (source + scripts only; build outputs excluded)
hash_inputs() {
  { for d in "$@"; do
      find "$d" -type f \
        -not -path '*/target/*' -not -path '*/build/*' -not -path '*/.git/*' \
        \( -name '*.beta' -o -name '*.alpha' -o -name '*.gamma' -o -name '*.alp' \
           -o -name '*.omg' -o -name '*.sh' -o -name '*.py' -o -name '*.rs' \
           -o -name '*.s' -o -name '*.toml' -o -name '*.md5' \) -print 2>/dev/null
    done; } | sort | xargs shasum 2>/dev/null | shasum | cut -d' ' -f1
}

# the build lineage everything sits on: any change here re-runs every step
CORE=$(hash_inputs alpha beta beta-lang beta-lang-rs)
RAN=0; SKIPPED=0

step() {  # label dir script [extra dep dirs...]
  s_label="$1"; s_dir="$2"; s_script="$3"; shift 3
  s_key=$(printf '%s_%s' "$s_dir" "$s_script" | tr '/ .' '___')
  s_hash="$CORE:$(hash_inputs "$s_dir" "$@")"
  if [ "${LATTICE_FULL:-0}" != "1" ] && [ -f "$CACHE/$s_key" ] \
     && [ "$(cat "$CACHE/$s_key")" = "$s_hash" ]; then
    printf '\n=== %s === (cached: inputs unchanged since last green run)\n' "$s_label"
    SKIPPED=$((SKIPPED+1))
    return
  fi
  printf '\n=== %s ===\n' "$s_label"
  if ( cd "$s_dir" && sh "$s_script" ); then
    RAN=$((RAN+1))
    printf '%s' "$s_hash" > "$CACHE/$s_key"
  else
    echo "FAILED: $s_label"; fail=1; rm -f "$CACHE/$s_key"
  fi
}

step "alpha — seed (provenance + behavior + diamond)" alpha       verify.sh
step "alpha — 3rd VM: independent Python reference agrees (deepens the Thompson root)" alpha diamond-py.sh
step "alpha — VM FUZZ: seed vs reference over random arithmetic tapes (signedness/wraparound/traps)" alpha vm-fuzz.sh
step "beta  — assembler self-hosts"                   beta        selfhost.sh
step "beta  — DIVERSITY: independent reference assembler (asm_ref.py) agrees byte-for-byte" beta asm-diamond.sh beta-lang-rs beta-lang delta
step "Beta  — language compiler (on-ramp) + corpus"   beta-lang-rs test.sh
step "bc    — Beta compiler in Beta self-hosts"       beta-lang   selfhost.sh
step "bc    — per-feature gate"                       beta-lang   test.sh
step "bc    — DIVERSITY: independent 2nd front-end (bc2.py) DDCs the trust surface (Thompson, D5)" beta-lang-py diverse-double-compilation.sh beta-lang-rs beta-lang delta gamma omega
step "bc    — CORRECTNESS: reference interpreter (beta_interp.py) == compile+run, random programs" beta-lang-py beta-correctness-fuzz.sh beta-lang-rs beta
step "delta — certificate checker"                    delta       test.sh
step "delta — soundness battery (no false proof)"     delta       soundness.sh
step "gamma — reference interpreter (ADTs + match)"   gamma       test-interp.sh
step "gamma — static type checker"                    gamma       test-typeck.sh
step "gamma — the Delta checker, written IN gamma"    gamma       test-checker.sh
step "diamond — checkers agree (Beta, Gamma, type-erased typed)" delta  checker-diamond.sh gamma
step "diamond — definitional eq vs operational eval"  delta       semantics-diamond.sh gamma
step "seam — inductive universals vs operational eval" delta      induction-soundness.sh gamma
step "seam — inductive predicates vs operational decision" delta   predicate-soundness.sh gamma
step "seam — propositional logic vs classical truth-table"  delta   logic-soundness.sh gamma
step "seam — corpus theorems: proved AND operationally true" delta soundness-sweep.sh gamma
step "seam — FUZZ: random +/* defeq vs operational eval" delta     seam-fuzz.sh gamma
step "checker diamond — FUZZ: random props, check.beta vs checker.gamma" delta checker-diamond-fuzz.sh gamma
step "logic diamond — FUZZ: random propositional proofs, all 3 checkers" delta logic-diamond-fuzz.sh gamma
step "predicate diamond — FUZZ: random Mem/ProdIs/Perm proofs, all 3 checkers" delta predicate-diamond-fuzz.sh gamma
step "predicate soundness — FUZZ: random predicates, kernel vs operational decision" delta predicate-soundness-fuzz.sh gamma
step "epsilon — on-ramp compiles + RUNS its corpus"   epsilon-rs  test_aarch64.sh
step "epsilon meaning — native exec vs gamma reference interpreter (diamond)" epsilon-rs epsilon-meaning-diamond.sh gamma
step "omega kernel diamond (RUST-FREE) — native vs omega2gamma.beta->interp.beta" omega kernel-diamond.sh epsilon-rs gamma
step "convergence — epsilon emits a proof; delta checks it" epsilon-rs convergence.sh delta
step "convergence (self-hosted) — the self-hosted compiler's certifiers, checked by delta" epsilon-rs convergence-selfhost.sh delta
step "convergence (reference route) — certifier RUN on interp.beta; cert checked by check.beta" epsilon-rs convergence-reference.sh delta gamma
step "convergence (RUST-FREE) — omega2gamma.beta->interp.beta; cert checked by check.beta" omega convergence-reference.sh epsilon-rs delta gamma
step "omega meaning — real Omega samples run Rust-free; exits match documented intent" omega omega-meaning.sh gamma ../samples
step "translation validation — delta re-evaluates each compilation's result (+ - * < == / %, loops, gcd, cross-machine)" omega translation-validation.sh epsilon-rs delta gamma
step "contracts — compiler discharges ensures; delta checks at build" epsilon-rs contracts.sh delta
step "contracts — static discharge and runtime asserts agree (soundness)" epsilon-rs discharge-soundness.sh delta
# untrusted proof elaborator (named binders -> raw certs); skipped if python3 is absent
if command -v python3 >/dev/null 2>&1; then
  step "tool — proof elaborator (named binders -> check.beta)" delta elab-test.sh gamma
  step "tool — elaborator/de-elaborator round-trip on the corpus" delta delab-roundtrip.sh gamma
  step "tool — proof-search front line (prover discharges; check.beta validates)" delta prover-test.sh gamma
  step "tool — prover diamond (prover certs accepted by check.beta AND checker.gamma)" delta prover-diamond.sh gamma
fi

echo ""
if [ "$fail" = 0 ]; then
  echo "LATTICE VERIFIED ✓ — seed → assembler → bc → checker; + gamma interp running the checker-in-gamma  ($RAN run, $SKIPPED cached)"
else
  echo "LATTICE: one or more rungs FAILED  ($RAN run, $SKIPPED cached)"; exit 1
fi
