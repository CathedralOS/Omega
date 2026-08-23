#!/usr/bin/env sh
# Verify the whole bootstrap lattice, rung by rung, in one command — from the
# hand-audited seed up to the certificate checker. Each step is the rung's own
# gate; this just runs them in dependency order and stops on the first failure.
#
#   alpha   the seed re-derives from source, conforms to SEMANTICS.md, and the
#           platform realizations share provenance/conformance/reproduction gates
#   alpha-assembler  the assembler self-hosts (reproduces its own bytecode byte-for-byte)
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
OMEGA_GATE_DIR=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd -P)
if [ -z "${OMEGA_REPO_ROOT:-}" ]; then
  OMEGA_REPO_ROOT=$OMEGA_GATE_DIR
  while [ ! -f "$OMEGA_REPO_ROOT/bootstrap/paths.sh" ]; do
    OMEGA_PATH_PARENT=$(dirname -- "$OMEGA_REPO_ROOT")
    if [ "$OMEGA_PATH_PARENT" = "$OMEGA_REPO_ROOT" ]; then
      echo "bootstrap paths: cannot find repository root from $OMEGA_GATE_DIR" >&2
      exit 2
    fi
    OMEGA_REPO_ROOT=$OMEGA_PATH_PARENT
  done
  unset OMEGA_PATH_PARENT
fi
. "$OMEGA_REPO_ROOT/bootstrap/paths.sh" || exit $?
cd "$OMEGA_GATE_DIR"
fail=0
CACHE="$OMEGA_REPO_ROOT/.lattice-cache"
mkdir -p "$CACHE"

sh "$OMEGA_REPO_ROOT/bootstrap/check-path-hygiene.sh" || exit $?

# content hash of the given dirs/files (source + scripts only; build outputs excluded)
hash_inputs() {
  { for d in "$@"; do
      d=$(omega_bootstrap_path "$d") || exit $?
      find "$d" -type f \
        -not -path '*/target/*' -not -path '*/build/*' -not -path '*/.git/*' \
        \( -name '*.beta' -o -name '*.alpha' -o -name '*.gamma' -o -name '*.alp' \
           -o -name '*.omg' -o -name '*.sh' -o -name '*.py' -o -name '*.rs' \
           -o -name '*.s' -o -name '*.toml' -o -name '*.md5' -o -name '*.elab' \
           -o -name '*.hex' \) -print 2>/dev/null
    done; } | sort | xargs shasum 2>/dev/null | shasum | cut -d' ' -f1
}

# The language spine and its shared path plumbing sit under every step. Assurance
# services are deliberately excluded here: steps that consume the proof kernel
# declare the proof-kernel role and hash it independently.
CORE=$(hash_inputs "$OMEGA_PATH_RUNGS_ROOT" \
  "$OMEGA_PATH_BOOTSTRAP_ROOT/paths.sh" \
  "$OMEGA_PATH_BOOTSTRAP_ROOT/check-path-hygiene.sh" \
  "$OMEGA_PATH_BOOTSTRAP_ROOT/test-paths.sh" \
  beta-rust)
RAN=0; SKIPPED=0

step() {  # label dir script [extra dep dirs...]
  s_label="$1"; s_role="$2"; s_script="$3"; shift 3
  s_dir=$(omega_bootstrap_path "$s_role") || exit $?
  s_key=$(printf '%s_%s' "$s_role" "$s_script" | tr '/ .' '___')
  s_hash="$CORE:$(hash_inputs "$s_role" "$@")"
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
step "alpha — assembler self-hosts"                   alpha-assembler selfhost.sh
step "alpha — REFERENCE: asm_ref.py agrees with the lattice assembler over the corpus" alpha-assembler asm-diamond.sh beta-rust beta proof-kernel
step "alpha — disposable Rust assembler producer agrees with the lattice assembler" alpha-assembler-rust test.sh alpha-assembler
step "Beta  — language compiler (on-ramp) + corpus"   beta-rust test.sh
step "bc    — Beta compiler in Beta self-hosts"       beta   selfhost.sh
step "bc    — per-feature gate"                       beta   test.sh
step "bc    — checked source-arena exhaustion"        beta   source-exhaustion.sh beta-rust alpha-assembler
step "bc    — Alpha-written cold-start compiler Slice A" beta cold-start/test.sh alpha alpha-assembler
step "bc    — CORRECTNESS: reference interpreter (beta_interp.py) == compile+run, random programs" beta-reference beta-correctness-fuzz.sh beta-rust alpha-assembler
step "bc    — EXHAUSTIVE I/O: interpret == compile+run over ALL 256 input bytes per program" beta-reference beta-io-exhaust.sh beta-rust alpha-assembler
step "proof kernel — certificate checker"                    proof-kernel-gates test.sh
step "proof kernel — soundness battery (no false proof)"     proof-kernel-gates soundness.sh
step "proof kernel — CROSS-CHECK: check_ref.py agrees on logic + equality + TV certs" proof-kernel-gates check-ref-diamond.sh beta-rust alpha-assembler
step "gamma — reference interpreter (ADTs + match)"   gamma       test-interp.sh
step "gamma — MEANING CROSS-CHECK: gamma_ref.py agrees with interp.beta (fuzz)" gamma gamma-diamond-py.sh beta-rust alpha-assembler
step "gamma — static type checker"                    gamma       test-typeck.sh
step "gamma — shared typed canonical-byte decoder" gamma test-canonical-bytes.sh
step "gamma — the proof kernel, written IN gamma"    proof-kernel-gates gamma-checker.sh gamma
step "cross-check — checkers agree (Beta, Gamma, type-erased typed)" proof-kernel-gates checker-diamond.sh gamma
step "seam — definitional eq vs operational eval"  proof-kernel-gates semantics-diamond.sh gamma
step "seam — inductive universals vs operational eval" proof-kernel-gates induction-soundness.sh gamma
step "seam — inductive predicates vs operational decision" proof-kernel-gates predicate-soundness.sh gamma
step "seam — propositional logic vs classical truth-table"  proof-kernel-gates logic-soundness.sh gamma
step "seam — corpus theorems: proved AND operationally true" proof-kernel-gates soundness-sweep.sh gamma
step "seam — FUZZ: random +/* defeq vs operational eval" proof-kernel-gates seam-fuzz.sh gamma
step "seam — recx accumulator recursion vs independent evaluation (check.beta + check_ref + checker.gamma agree)" proof-kernel-gates recx-soundness.sh gamma alpha-assembler beta beta-rust
step "seam — prodrec product eliminator cross-check: check.beta + check_ref + checker.gamma decide identically (guard + soundness controls rejected by all three)" proof-kernel-gates prodrec-seam.sh gamma alpha-assembler beta beta-rust
step "contract discharge (omega source) — math_proofs requires/ensures translated to kernel propositions and proven by check.beta + check_ref + checker.gamma (perturbation rejected)" proof-kernel-gates math-contracts.sh gamma alpha-assembler beta beta-rust corpus
step "termination discharge (omega source) — 'terminates by s -> Slice::Length' tail-recursion tied to a 3-checker measure-decrease lemma (reversed measure rejected)" proof-kernel-gates termination-obligations.sh gamma alpha-assembler beta beta-rust corpus
step "forall-input theorem — count(xs,n)=len(xs)+n proven for ALL inputs by induction (check.beta + check_ref + checker.gamma; perturbation rejected)" proof-kernel-gates forall-input.sh gamma alpha-assembler beta beta-rust
step "forall-input SAMPLE connection — a real sample's count loop tied to the ∀-input theorem: proven = len(s)+acc for EVERY input (not just documented vectors)" proof-kernel-gates forall-sample.sh gamma alpha-assembler beta beta-rust corpus
step "checker cross-check — FUZZ: random props, check.beta vs checker.gamma" proof-kernel-gates checker-diamond-fuzz.sh gamma
step "logic cross-check — FUZZ: random propositional proofs, all 3 checkers" proof-kernel-gates logic-diamond-fuzz.sh gamma
step "predicate cross-check — FUZZ: random Mem/ProdIs/Perm proofs, all 3 checkers" proof-kernel-gates predicate-diamond-fuzz.sh gamma
step "predicate soundness — FUZZ: random predicates, kernel vs operational decision" proof-kernel-gates predicate-soundness-fuzz.sh gamma
step "delta — on-ramp compiles + RUNS its corpus"   delta-rs  test_aarch64.sh
step "delta meaning — native exec vs gamma reference interpreter" delta-rs delta-meaning-diamond.sh gamma
step "delta D0 storage meaning (RUST-FREE) — omega2gamma.beta -> interp.beta" delta-rs delta-storage-meaning.sh omega0 gamma
step "omega0 Delta O1 frontend — variable straight-line console profile through lexer/parser/checker and Delta-written recompilation" delta-rs omega0-frontend-test.sh omega0 corpus
step "omega0 Delta O1 frontend meaning (RUST-FREE) — retained operands + dual-channel output + semantic rejection through Gamma" delta-rs omega0-frontend-meaning.sh omega0 gamma corpus
step "omega kernel cross-check (RUST-FREE) — native vs omega2gamma.beta->interp.beta" omega0-gates kernel-diamond.sh delta-rs gamma
step "convergence — Delta emits a proof; the proof kernel checks it" delta-rs convergence.sh proof-kernel
step "convergence (self-hosted) — the self-hosted compiler's certifiers, checked by the proof kernel" delta-rs convergence-selfhost.sh proof-kernel
step "convergence (reference route) — certifier RUN on interp.beta; cert checked by check.beta" delta-rs convergence-reference.sh proof-kernel gamma
step "convergence (RUST-FREE) — omega2gamma.beta->interp.beta; cert checked by check.beta" omega0-gates convergence-reference.sh delta-rs proof-kernel gamma
step "omega2gamma termination canary — translator halts on every sample, supported or refused (no silent scan-forever)" omega0-gates omega2gamma-termination.sh alpha-assembler beta beta-rust corpus
step "omega0 source bundle — canonical deterministic multi-file input" omega0-gates omega0-bundle-test.sh
step "omega0 Delta O1 artifact — variable terminal-Psi to byte-identical x86-64 ELF" omega0-gates delta-terminal-to-elf.sh delta-rs omega psi/semantics/psi-terminal-codec
step "omega0 Delta O1 artifact meaning (RUST-FREE) — exact native vs omega2gamma.beta->interp.beta images" omega0-gates delta-terminal-to-elf-meaning.sh delta-rs gamma omega psi/semantics/psi-terminal-codec
step "omega meaning — real Omega samples run Rust-free; exits match documented intent" omega0-gates omega-meaning.sh gamma corpus
step "omega meaning-TV — the kernel re-computes each covered sample's arithmetic (proof, not comparison)" omega0-refinement meaning-tv.sh omega0-meaning gamma proof-kernel alpha-assembler beta beta-rust corpus
step "input-grid meaning TV — input-taking samples proven per documented input vector (substitution closes the program; the whole proof pipe applies per vector)" omega0-refinement input-tv.sh omega0-meaning gamma proof-kernel alpha-assembler beta beta-rust corpus
step "meaning-cert cross-check — meaning-TV certs replayed through check.beta AND check_ref.py" omega0-refinement meaning-cert-diamond.sh omega0-meaning proof-kernel alpha-assembler beta beta-rust corpus
step "translation validation — the proof kernel re-evaluates each compilation's result (+ - * < == / %, loops, gcd, cross-machine)" omega0-refinement translation-validation.sh omega0-meaning delta-rs proof-kernel gamma
step "symbolic loops — beta_symbolic's data-dependent loop summaries (symbolic trip count -> closed form) pinned to the interpreter across an input grid" beta-refinement symbolic-loops.sh
step "refinement — bc's machine code proved to compute its Beta source meaning (instruction-level TV: both meanings auto-derived, equivalence kernel-checked, never run)" beta-refinement refinement.sh alpha proof-kernel alpha-assembler beta beta-rust
step "refinement-cert cross-check — every refl cert replayed through check.beta AND check_ref.py" beta-refinement refinement-cert-diamond.sh alpha proof-kernel alpha-assembler beta beta-rust
step "contracts — compiler discharges ensures; the proof kernel checks at build" delta-rs contracts.sh proof-kernel
step "contracts — static discharge and runtime asserts agree (soundness)" delta-rs discharge-soundness.sh proof-kernel
# untrusted proof elaborator (named binders -> raw certs); skipped if python3 is absent
if command -v python3 >/dev/null 2>&1; then
  step "tool — proof elaborator (named binders -> check.beta)" proof-kernel-gates elab-test.sh gamma
  step "tool — proof-library cross-check (WHOLE corpus decided identically by check.beta AND check_ref.py; perturbations rejected)" proof-kernel-gates proofs-crosscheck.sh gamma alpha-assembler beta beta-rust
  step "tool — elaborator/de-elaborator round-trip on the corpus" proof-kernel-gates delab-roundtrip.sh gamma
  step "tool — proof-search front line (prover discharges; check.beta validates)" proof-kernel-gates prover-test.sh gamma
  step "tool — prover certificate cross-check (accepted by check.beta AND checker.gamma)" proof-kernel-gates prover-diamond.sh gamma
fi

echo ""
if [ "$fail" = 0 ]; then
  echo "LATTICE VERIFIED ✓ — seed → assembler → bc → Delta; proof kernel verified; + gamma interp running the checker-in-gamma  ($RAN run, $SKIPPED cached)"
else
  echo "LATTICE: one or more rungs FAILED  ($RAN run, $SKIPPED cached)"; exit 1
fi
