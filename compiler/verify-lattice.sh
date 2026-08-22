#!/usr/bin/env sh
# Verify the whole bootstrap lattice, rung by rung, in one command — from the
# hand-audited seed up to the certificate checker. Each step is the rung's own
# gate; this just runs them in dependency order and stops on the first failure.
#
#   alpha   the seed re-derives from source, conforms to SEMANTICS.md, and the
#           platform realizations share provenance/conformance/reproduction gates
#   beta    the assembler self-hosts (reproduces its own bytecode byte-for-byte)
#   Beta    the language compiler (Rust on-ramp) compiles + runs the corpus
#   bc      the Beta compiler WRITTEN IN BETA self-hosts (Rust leaves the lineage)
#   delta   the compiler-host language: programs run natively and through the
#           Gamma meaning path (macOS arm64 native legs skip cleanly elsewhere)
#   proof   the cross-cutting proof kernel and its semantic seams
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
           -o -name '*.s' -o -name '*.toml' -o -name '*.md5' -o -name '*.elab' \
           -o -name '*.hex' \) -print 2>/dev/null
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

step "alpha — seed (provenance + behavior + reproduction)" alpha       verify.sh
step "alpha — reference VM agrees with the host realization" alpha diamond-py.sh
step "alpha — VM FUZZ: seed vs reference over random arithmetic tapes (signedness/wraparound/traps)" alpha vm-fuzz.sh
step "beta  — assembler self-hosts"                   beta        selfhost.sh
step "beta  — REFERENCE: asm_ref.py agrees with the lattice assembler over the corpus" beta asm-diamond.sh beta-lang-rs beta-lang proof-kernel
step "Beta  — language compiler (on-ramp) + corpus"   beta-lang-rs test.sh
step "bc    — Beta compiler in Beta self-hosts"       beta-lang   selfhost.sh
step "bc    — per-feature gate"                       beta-lang   test.sh
step "bc    — CORRECTNESS: reference interpreter (beta_interp.py) == compile+run, random programs" beta-lang-py beta-correctness-fuzz.sh beta-lang-rs beta
step "bc    — EXHAUSTIVE I/O: interpret == compile+run over ALL 256 input bytes per program" beta-lang-py beta-io-exhaust.sh beta-lang-rs beta
step "proof kernel — certificate checker"                    proof-kernel       test.sh
step "proof kernel — soundness battery (no false proof)"     proof-kernel       soundness.sh
step "proof kernel — CROSS-CHECK: check_ref.py agrees on logic + equality + TV certs" proof-kernel check-ref-diamond.sh beta-lang-rs beta
step "gamma — reference interpreter (ADTs + match)"   gamma       test-interp.sh
step "gamma — MEANING CROSS-CHECK: gamma_ref.py agrees with interp.beta (fuzz)" gamma gamma-diamond-py.sh beta-lang-rs beta
step "gamma — static type checker"                    gamma       test-typeck.sh
step "gamma — shared typed canonical-byte decoder" gamma test-canonical-bytes.sh
step "gamma — canonical terminal ledger + closed leaf/call schemas" gamma test-terminal-ledger-spike.sh psi-rs/semantics/psi-terminal-codec
step "gamma — the proof kernel, written IN gamma"    gamma       test-checker.sh
step "cross-check — checkers agree (Beta, Gamma, type-erased typed)" proof-kernel  checker-diamond.sh gamma
step "seam — definitional eq vs operational eval"  proof-kernel       semantics-diamond.sh gamma
step "seam — inductive universals vs operational eval" proof-kernel      induction-soundness.sh gamma
step "seam — inductive predicates vs operational decision" proof-kernel   predicate-soundness.sh gamma
step "seam — propositional logic vs classical truth-table"  proof-kernel   logic-soundness.sh gamma
step "seam — corpus theorems: proved AND operationally true" proof-kernel soundness-sweep.sh gamma
step "seam — FUZZ: random +/* defeq vs operational eval" proof-kernel     seam-fuzz.sh gamma
step "seam — recx accumulator recursion vs independent evaluation (check.beta + check_ref + checker.gamma agree)" proof-kernel recx-soundness.sh gamma beta beta-lang beta-lang-rs
step "seam — prodrec product eliminator cross-check: check.beta + check_ref + checker.gamma decide identically (guard + soundness controls rejected by all three)" proof-kernel prodrec-seam.sh gamma beta beta-lang beta-lang-rs
step "contract discharge (omega source) — math_proofs requires/ensures translated to kernel propositions and proven by check.beta + check_ref + checker.gamma (perturbation rejected)" proof-kernel math-contracts.sh gamma beta beta-lang beta-lang-rs ../lattice-corpus
step "termination discharge (omega source) — 'terminates by s -> Slice::Length' tail-recursion tied to a 3-checker measure-decrease lemma (reversed measure rejected)" proof-kernel termination-obligations.sh gamma beta beta-lang beta-lang-rs ../lattice-corpus
step "forall-input theorem — count(xs,n)=len(xs)+n proven for ALL inputs by induction (check.beta + check_ref + checker.gamma; perturbation rejected)" proof-kernel forall-input.sh gamma beta beta-lang beta-lang-rs
step "forall-input SAMPLE connection — a real sample's count loop tied to the ∀-input theorem: proven = len(s)+acc for EVERY input (not just documented vectors)" proof-kernel forall-sample.sh gamma beta beta-lang beta-lang-rs ../lattice-corpus
step "checker cross-check — FUZZ: random props, check.beta vs checker.gamma" proof-kernel checker-diamond-fuzz.sh gamma
step "logic cross-check — FUZZ: random propositional proofs, all 3 checkers" proof-kernel logic-diamond-fuzz.sh gamma
step "predicate cross-check — FUZZ: random Mem/ProdIs/Perm proofs, all 3 checkers" proof-kernel predicate-diamond-fuzz.sh gamma
step "predicate soundness — FUZZ: random predicates, kernel vs operational decision" proof-kernel predicate-soundness-fuzz.sh gamma
step "delta — on-ramp compiles + RUNS its corpus"   delta-rs  test_aarch64.sh
step "delta meaning — native exec vs gamma reference interpreter" delta-rs delta-meaning-diamond.sh gamma
step "delta D0 storage meaning (RUST-FREE) — omega2gamma.beta -> interp.beta" delta-rs delta-storage-meaning.sh ../omega ../gamma
step "omega kernel cross-check (RUST-FREE) — native vs omega2gamma.beta->interp.beta" omega kernel-diamond.sh delta-rs gamma
step "convergence — Delta emits a proof; the proof kernel checks it" delta-rs convergence.sh proof-kernel
step "convergence (self-hosted) — the self-hosted compiler's certifiers, checked by the proof kernel" delta-rs convergence-selfhost.sh proof-kernel
step "convergence (reference route) — certifier RUN on interp.beta; cert checked by check.beta" delta-rs convergence-reference.sh proof-kernel gamma
step "convergence (RUST-FREE) — omega2gamma.beta->interp.beta; cert checked by check.beta" omega convergence-reference.sh delta-rs proof-kernel gamma
step "omega2gamma termination canary — translator halts on every sample, supported or refused (no silent scan-forever)" omega omega2gamma-termination.sh beta beta-lang beta-lang-rs ../lattice-corpus
step "omega0 source bundle — canonical deterministic multi-file input" omega omega0-bundle-test.sh
step "omega meaning — real Omega samples run Rust-free; exits match documented intent" omega omega-meaning.sh gamma ../lattice-corpus
step "omega meaning-TV — the kernel re-computes each covered sample's arithmetic (proof, not comparison)" omega meaning-tv.sh gamma proof-kernel beta beta-lang beta-lang-rs ../lattice-corpus
step "input-grid meaning TV — input-taking samples proven per documented input vector (substitution closes the program; the whole proof pipe applies per vector)" omega input-tv.sh gamma proof-kernel beta beta-lang beta-lang-rs ../lattice-corpus
step "meaning-cert cross-check — meaning-TV certs replayed through check.beta AND check_ref.py" omega meaning-cert-diamond.sh proof-kernel beta beta-lang beta-lang-rs ../lattice-corpus
step "translation validation — the proof kernel re-evaluates each compilation's result (+ - * < == / %, loops, gcd, cross-machine)" omega translation-validation.sh delta-rs proof-kernel gamma
step "symbolic loops — beta_symbolic's data-dependent loop summaries (symbolic trip count -> closed form) pinned to the interpreter across an input grid" beta-lang-py symbolic-loops.sh
step "refinement — bc's machine code proved to compute its Beta source meaning (instruction-level TV: both meanings auto-derived, equivalence kernel-checked, never run)" alpha refinement.sh proof-kernel beta beta-lang beta-lang-rs beta-lang-py
step "refinement-cert cross-check — every refl cert replayed through check.beta AND check_ref.py" alpha refinement-cert-diamond.sh proof-kernel beta beta-lang beta-lang-rs beta-lang-py
step "contracts — compiler discharges ensures; the proof kernel checks at build" delta-rs contracts.sh proof-kernel
step "contracts — static discharge and runtime asserts agree (soundness)" delta-rs discharge-soundness.sh proof-kernel
# untrusted proof elaborator (named binders -> raw certs); skipped if python3 is absent
if command -v python3 >/dev/null 2>&1; then
  step "tool — proof elaborator (named binders -> check.beta)" proof-kernel elab-test.sh gamma
  step "tool — proof-library cross-check (WHOLE corpus decided identically by check.beta AND check_ref.py; perturbations rejected)" proof-kernel proofs-crosscheck.sh gamma beta beta-lang beta-lang-rs
  step "tool — elaborator/de-elaborator round-trip on the corpus" proof-kernel delab-roundtrip.sh gamma
  step "tool — proof-search front line (prover discharges; check.beta validates)" proof-kernel prover-test.sh gamma
  step "tool — prover certificate cross-check (accepted by check.beta AND checker.gamma)" proof-kernel prover-diamond.sh gamma
fi

echo ""
if [ "$fail" = 0 ]; then
  echo "LATTICE VERIFIED ✓ — seed → assembler → bc → Delta; proof kernel verified; + gamma interp running the checker-in-gamma  ($RAN run, $SKIPPED cached)"
else
  echo "LATTICE: one or more rungs FAILED  ($RAN run, $SKIPPED cached)"; exit 1
fi
