#!/usr/bin/env sh
# Verify the current bootstrap lattice, rung by rung, in one command — from the
# hand-audited seed through the Delta/omega-bootstrap vertical slices. Each step is the rung's own
# gate; this just runs them in dependency order and stops on the first failure.
#
#   alpha   the seed re-derives from source, conforms to SEMANTICS.md, and the
#           platform realizations share provenance/conformance/reproduction gates
#   alpha-assembler  the assembler self-hosts (reproduces its own bytecode byte-for-byte)
#   Beta    the Alpha-rooted compiler artifact compiles + runs the corpus
#   bc      the Beta compiler WRITTEN IN BETA self-hosts
#   Beta/Rust  the disposable producer remains an explicit diagnostic comparison
#   delta   the compiler-host language: programs run natively and through the
#           Gamma meaning path (macOS arm64 native legs skip cleanly elsewhere)
#   proof   the cross-cutting proof kernel and its semantic seams
#
# INCREMENTAL: each step declares its input dirs; a step whose inputs are
# unchanged since its last GREEN run is skipped (content-hash cache in
# .lattice-cache/). So an omega-slice edit re-verifies only the gates it can
# reach, not the full prover battery. LATTICE_FULL=1 forces everything.
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
           -o -name '*.hex' -o -name '*.json' \) -print 2>/dev/null
    done; } | sort | xargs shasum 2>/dev/null | shasum | cut -d' ' -f1
}

# The language spine and its shared path plumbing sit under every step. Assurance
# services are deliberately excluded here: steps that consume the proof kernel
# declare the proof-kernel role and hash it independently.
CORE=$(hash_inputs "$OMEGA_PATH_RUNGS_ROOT" \
  "$OMEGA_PATH_BOOTSTRAP_ROOT/paths.sh" \
  "$OMEGA_PATH_BOOTSTRAP_ROOT/check-path-hygiene.sh" \
  "$OMEGA_PATH_BOOTSTRAP_ROOT/test-paths.sh")
RAN=0; SKIPPED=0

step() {  # label dir script [extra dep dirs...]
  s_label="$1"; s_role="$2"; s_script="$3"; shift 3
  s_dir=$(omega_bootstrap_path "$s_role") || exit $?
  s_variant=${LATTICE_STEP_VARIANT:-default}
  if [ "$s_variant" = default ]; then
    # Preserve existing cache keys and hashes for every unvariant step.
    s_key=$(printf '%s_%s' "$s_role" "$s_script" | tr '/ .' '___')
    s_hash="$CORE:$(hash_inputs "$s_role" "$@")"
  else
    s_key=$(printf '%s_%s_%s' "$s_role" "$s_script" "$s_variant" | tr '/ .' '___')
    s_hash="$CORE:$s_variant:$(hash_inputs "$s_role" "$@")"
  fi
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
step "alpha — REFERENCE: asm_ref.py agrees with the lattice assembler over the corpus" alpha-assembler asm-diamond.sh beta proof-kernel
step "alpha — disposable Rust assembler producer agrees with the lattice assembler" alpha-assembler-rust test.sh alpha-assembler
step "bc    — Alpha-written cold-start compiler surface" beta cold-start/test.sh alpha alpha-assembler
step "bc    — Alpha-rooted full source, artifact fixed point, corpus" beta cold-start/full-source.sh alpha alpha-assembler
step "bc    — lower-rooted artifact framing + direct-target + call-region obligations" beta-refinement bc-artifact-structure.sh alpha beta alpha-assembler
if [ "${LATTICE_FULL:-0}" = "1" ]; then
  step "bc    — source control/effect/frame/data sites plus exhaustive historical mutations" beta-refinement bc-block-control.sh alpha beta alpha-assembler
else
  BC_BLOCK_FOCUS=root-observation LATTICE_STEP_VARIANT=root-observation \
    step "bc    — source/artifact control composed to the maximal root observation" beta-refinement bc-block-control.sh alpha beta alpha-assembler
fi
step "bc    — Beta compiler in Beta self-hosts"       beta   selfhost.sh
step "bc    — per-feature gate"                       beta   test.sh
step "bc    — checked compiler resource profile"      beta   source-exhaustion.sh alpha-assembler
step "bc    — CORRECTNESS: reference interpreter (beta_interp.py) == compile+run, random programs" beta-reference beta-correctness-fuzz.sh beta alpha-assembler
step "bc    — EXHAUSTIVE I/O: interpret == compile+run over ALL 256 input bytes per program" beta-reference beta-io-exhaust.sh beta alpha-assembler
step "Beta/Rust — DIAGNOSTIC on-ramp + corpus"         beta-rust test.sh beta
step "proof kernel — certificate checker"                    proof-kernel-gates test.sh
step "proof kernel — soundness battery (no false proof)"     proof-kernel-gates soundness.sh
step "proof kernel — CROSS-CHECK: check_ref.py agrees on logic + equality + TV certs" proof-kernel-gates check-ref-diamond.sh beta alpha-assembler
step "gamma — reference interpreter (ADTs + match)"   gamma       test-interp.sh
step "gamma — MEANING CROSS-CHECK: gamma_ref.py agrees with interp.beta (fuzz)" gamma gamma-diamond-py.sh beta alpha-assembler
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
step "seam — recx accumulator recursion vs independent evaluation (check.beta + check_ref + checker.gamma agree)" proof-kernel-gates recx-soundness.sh gamma alpha-assembler beta
step "seam — prodrec product eliminator cross-check: check.beta + check_ref + checker.gamma decide identically (guard + soundness controls rejected by all three)" proof-kernel-gates prodrec-seam.sh gamma alpha-assembler beta
step "contract discharge (omega source) — math_proofs requires/ensures translated to kernel propositions and proven by check.beta + check_ref + checker.gamma (perturbation rejected)" proof-kernel-gates math-contracts.sh gamma alpha-assembler beta corpus
step "termination discharge (omega source) — 'terminates by s -> Slice::Length' tail-recursion tied to a 3-checker measure-decrease lemma (reversed measure rejected)" proof-kernel-gates termination-obligations.sh gamma alpha-assembler beta corpus
step "forall-input theorem — count(xs,n)=len(xs)+n proven for ALL inputs by induction (check.beta + check_ref + checker.gamma; perturbation rejected)" proof-kernel-gates forall-input.sh gamma alpha-assembler beta
step "forall-input SAMPLE connection — a real sample's count loop tied to the ∀-input theorem: proven = len(s)+acc for EVERY input (not just documented vectors)" proof-kernel-gates forall-sample.sh gamma alpha-assembler beta corpus
step "checker cross-check — FUZZ: random props, check.beta vs checker.gamma" proof-kernel-gates checker-diamond-fuzz.sh gamma
step "logic cross-check — FUZZ: random propositional proofs, all 3 checkers" proof-kernel-gates logic-diamond-fuzz.sh gamma
step "predicate cross-check — FUZZ: random Mem/ProdIs/Perm proofs, all 3 checkers" proof-kernel-gates predicate-diamond-fuzz.sh gamma
step "predicate soundness — FUZZ: random predicates, kernel vs operational decision" proof-kernel-gates predicate-soundness-fuzz.sh gamma
step "delta — on-ramp compiles + RUNS its corpus"   delta-rust  test_aarch64.sh
step "delta compiler scale — aggregate parameter tables remain disjoint beyond machine 64 and signature bounds fail closed" delta-rust lowermachine-scale-test.sh
step "delta meaning — native exec vs gamma reference interpreter" delta-rust delta-meaning-diamond.sh gamma
step "delta D0 storage meaning (RUST-FREE) — omega2gamma.beta -> interp.beta" delta-rust delta-storage-meaning.sh omega-bootstrap gamma
step "omega-bootstrap Delta frontend — O1 regression plus bounded source-unit transport through lexer/parser/checker and Delta-written recompilation" delta-rust omega-bootstrap-frontend-test.sh omega-bootstrap corpus
step "omega-bootstrap Delta frontend meaning (RUST-FREE) — source-unit identity, retained operands, dual-channel rejection, and exhaustion through Gamma" delta-rust omega-bootstrap-frontend-meaning.sh omega-bootstrap gamma corpus
step "omega kernel cross-check (RUST-FREE) — native vs omega2gamma.beta->interp.beta" omega-bootstrap-gates kernel-diamond.sh delta-rust gamma
step "convergence — Delta emits a proof; the proof kernel checks it" delta-rust convergence.sh proof-kernel
step "convergence (self-hosted) — the self-hosted compiler's certifiers, checked by the proof kernel" delta-rust convergence-selfhost.sh proof-kernel
step "convergence (reference route) — certifier RUN on interp.beta; cert checked by check.beta" delta-rust convergence-reference.sh proof-kernel gamma
step "convergence (RUST-FREE) — omega2gamma.beta->interp.beta; cert checked by check.beta" omega-bootstrap-gates convergence-reference.sh delta-rust proof-kernel gamma
step "omega2gamma termination canary — translator halts on every sample, supported or refused (no silent scan-forever)" omega-bootstrap-gates omega2gamma-termination.sh alpha-assembler beta corpus
step "lowermachine meaning — real compiler executes through Gamma; exact state/tree/source ceilings fail closed" omega-bootstrap-gates lowermachine-meaning.sh delta-rust gamma
step "omega-bootstrap source bundle — canonical deterministic multi-file input" omega-bootstrap-gates omega-bootstrap-bundle-test.sh
step "omega-bootstrap compilation envelope — canonical package/source/alias transport and malformed/resource teeth" omega-bootstrap-gates omega-bootstrap-compilation-test.sh omega-bootstrap
step "omega-bootstrap Delta compilation-envelope checker — structural native/self-built relations and resource boundaries" omega-bootstrap-gates delta-compilation-envelope.sh delta-rust omega-bootstrap
step "omega-bootstrap Delta compilation-envelope meaning (RUST-FREE) — structural 0/251/252 through Gamma" omega-bootstrap-gates delta-compilation-envelope-meaning.sh omega-bootstrap-meaning gamma
step "omega-bootstrap two-package fixture — pinned deterministic OMGCOMP and semantic negatives" omega-bootstrap-gates two-unit-compilation-fixture.sh omega-bootstrap-compiler
step "omega-bootstrap Delta resolution handoff — exact OMGCOMP to canonical OMGRSW1, native/self/resource agreement" omega-bootstrap-gates delta-resolution-handoff.sh omega-bootstrap-compiler delta-rust
step "omega-bootstrap resolution meaning (RUST-FREE) — canonical 0/251/252 through Gamma" omega-bootstrap-gates delta-resolution-handoff-meaning.sh omega-bootstrap-compiler omega-bootstrap-meaning delta-rust gamma
step "omega-bootstrap resolved-source lowerer — exact OMGLOW1 to CKIR1, native/self relation and resource agreement" omega-bootstrap-gates delta-resolved-to-ckir.sh omega-bootstrap-compiler delta-rust
step "omega-bootstrap resolved-source lowering meaning (RUST-FREE) — canonical CKIR plus 251/252 through Gamma" omega-bootstrap-gates delta-resolved-to-ckir-meaning.sh omega-bootstrap-compiler omega-bootstrap-meaning delta-rust gamma
step "omega-bootstrap two-package producer composite — exact witness, CKIR, ELF, result, and native/self cross-builds" omega-bootstrap-gates delta-two-package-composite.sh omega-bootstrap-compiler delta-rust
step "omega-bootstrap two-package producer meaning (RUST-FREE) — Gamma lowerer CKIR feeds Gamma backend ELF" omega-bootstrap-gates delta-two-package-composite-meaning.sh omega-bootstrap-compiler omega-bootstrap-meaning delta-rust gamma
step "omega-bootstrap role-3 resolution handoff — exact same-module cross-source attached-machine bindings" omega-bootstrap-gates delta-role3-resolution-handoff.sh omega-bootstrap-compiler delta-rust
step "omega-bootstrap CKIR2 resolved-source lowerer — explicit root and typed finite calls, native/self relation" omega-bootstrap-gates delta-resolved-to-ckir2.sh omega-bootstrap-compiler delta-rust
step "omega-bootstrap CKIR2 lowering meaning (RUST-FREE) — explicit root/calls and 251/252 through Gamma" omega-bootstrap-gates delta-resolved-to-ckir2-meaning.sh omega-bootstrap-compiler omega-bootstrap-meaning delta-rust gamma
step "omega-bootstrap CKIR2 backend — reachable call closure, ABI staging, rel32, ELF, and result" omega-bootstrap-gates delta-checked-ir-v2-backend.sh omega-bootstrap-compiler delta-rust
step "omega-bootstrap CKIR2 backend meaning (RUST-FREE) — exact ELF/result and 251/252 through Gamma" omega-bootstrap-gates delta-checked-ir-v2-backend-meaning.sh omega-bootstrap-compiler omega-bootstrap-meaning delta-rust gamma
step "omega-bootstrap CKIR2 role-3 producer composite — resolver, lowerer, backend, mixed builds, and exact result" omega-bootstrap-gates delta-role3-ckir2-composite.sh omega-bootstrap-compiler delta-rust
step "omega-bootstrap CKIR3 resolved-source lowerer — constant DAG, <=, guardless and cyclic control, native/self relation" omega-bootstrap-gates delta-resolved-to-ckir3.sh omega-bootstrap-compiler delta-rust psi
step "omega-bootstrap CKIR3 lowering meaning (RUST-FREE) — representative constant DAG, aggregate copy, <=, and 251/252 through Gamma" omega-bootstrap-gates delta-resolved-to-ckir3-meaning.sh omega-bootstrap-compiler omega-bootstrap-meaning delta-rust gamma
step "omega-bootstrap CKIR3 backend — derived read-only image, conditional ELF segments, and exact reconstruction" omega-bootstrap-gates delta-checked-ir-v3-backend.sh omega-bootstrap-compiler delta-rust
step "omega-bootstrap CKIR3 backend meaning (RUST-FREE) — exact three-segment ELF/result and 251/252 through Gamma" omega-bootstrap-gates delta-checked-ir-v3-backend-meaning.sh omega-bootstrap-compiler omega-bootstrap-meaning delta-rust gamma
step "omega-bootstrap CKIR3 producer composite — native/self/mixed producer-backend pairs and independent result/ELF" omega-bootstrap-gates delta-ckir3-composite.sh omega-bootstrap-compiler delta-rust psi
step "omega-bootstrap OMGRFN2 layer 1 — exact frame, OMGCOMP graph, nested bundle, and source custody below Delta" omega-bootstrap-refinement omgrfn2-frame-omgcomp-custody.sh alpha alpha-assembler beta omega-bootstrap omega-bootstrap-gates
step "omega-bootstrap OMGRFN2 layer 2 — independent first-artifact source-to-witness resolution below Delta" omega-bootstrap-refinement omgrfn2-source-witness-independent.sh alpha alpha-assembler beta delta-rust omega-bootstrap omega-bootstrap-gates
step "omega-bootstrap OMGRFN2 layer 3 — independent witness-to-CKIR declarations, layout, and root below Delta" omega-bootstrap-refinement omgrfn2-witness-ckir-tables.sh alpha alpha-assembler beta delta-rust omega-bootstrap omega-bootstrap-gates
step "omega-bootstrap OMGRFN2 layer 4 — independent resolved bodies, CKIR rows, and source-only result below Delta" omega-bootstrap-refinement omgrfn2-resolved-body-result.sh alpha alpha-assembler beta delta-rust omega-bootstrap omega-bootstrap-gates
step "omega-bootstrap OMGRFN2 layer 5 — complete CKIR/result and CKIR-to-ELF relations at version-2 frame offsets below Delta" omega-bootstrap-refinement omgrfn2-ckir-elf-refinement.sh alpha alpha-assembler beta delta-rust omega-bootstrap omega-bootstrap-gates
step "omega-bootstrap OMGRFN3 layer 1 — exact version-3 frame, OMGCOMP graph, bundle, and source custody below Delta" omega-bootstrap-refinement omgrfn3-frame-omgcomp-custody.sh alpha alpha-assembler beta omega-bootstrap omega-bootstrap-gates
step "omega-bootstrap OMGRFN3 layer 2 — independent source-to-role-3 witness resolution below Delta" omega-bootstrap-refinement omgrfn3-source-witness-independent.sh alpha alpha-assembler beta delta-rust omega-bootstrap omega-bootstrap-gates
step "omega-bootstrap OMGRFN3 layer 3 — independent witness-to-CKIR2 tables, layout, types, and root below Delta" omega-bootstrap-refinement omgrfn3-witness-ckir2-tables.sh alpha alpha-assembler beta delta-rust omega-bootstrap omega-bootstrap-gates
step "omega-bootstrap OMGRFN3 layer 4 — independent bodies/calls, CKIR2 rows, and artifact-free source result below Delta" omega-bootstrap-refinement omgrfn3-resolved-body-result.sh alpha alpha-assembler beta delta-rust omega-bootstrap omega-bootstrap-gates
step "omega-bootstrap OMGRFN3 layer 5a — complete CKIR2/result validation below Delta" omega-bootstrap-refinement omgrfn3-ckir2-artifact-result.sh alpha alpha-assembler beta delta-rust omega-bootstrap omega-bootstrap-gates
step "omega-bootstrap OMGRFN3 layer 5b — exact reachable-call CKIR2-to-ELF reconstruction below Delta" omega-bootstrap-refinement omgrfn3-ckir2-refinement-elf.sh alpha alpha-assembler beta delta-rust omega-bootstrap omega-bootstrap-gates
step "omega-bootstrap OMGRFN3 composite — all five independent responsibilities consume one exact role-3 frame" omega-bootstrap-refinement omgrfn3-same-frame-composite.sh alpha alpha-assembler beta delta-rust omega-bootstrap omega-bootstrap-gates
step "omega-bootstrap OMGRFN4 layer 1 — exact version-4 frame, OMGCOMP graph, bundle, and source custody below Delta" omega-bootstrap-refinement omgrfn4-frame-omgcomp-custody.sh alpha alpha-assembler beta omega-bootstrap omega-bootstrap-gates
step "product compiler checkpoint — exact resolver closure plus provisional Ωself admission" source-checkpoints verify.sh omega-rust psi
step "omega-bootstrap source-custody frontend probe — exhaustive native plus representative Delta-self-built checking" omega-bootstrap-gates delta-source-custody-frontend.sh delta-rust psi source-checkpoints
step "omega-bootstrap source-custody meaning (RUST-FREE) — exact product unit plus semantic rejection and exhaustion through Gamma" omega-bootstrap-gates delta-source-custody-meaning.sh omega-bootstrap-meaning gamma psi source-checkpoints
step "omega-bootstrap CKIR1 artifact — exhaustive native/self producer and backend relations" omega-bootstrap-gates delta-source-custody-artifact.sh delta-rust omega-bootstrap-compiler psi omega-rust psi-rust
step "omega-bootstrap CKIR1 artifact meaning (RUST-FREE) — producer/backend 0/251/252 and exact bytes through Gamma" omega-bootstrap-gates delta-source-custody-artifact-meaning.sh delta-rust omega-bootstrap-compiler omega-bootstrap-meaning gamma
step "omega-bootstrap refinement envelope — exact source/CKIR/ELF custody and untrusted claims below Delta" omega-bootstrap-refinement checked-ir-refinement-envelope.sh alpha alpha-assembler beta
step "omega-bootstrap refinement source input — exact one-unit bundle and lexical custody below Delta" omega-bootstrap-refinement checked-ir-refinement-source-input.sh alpha alpha-assembler beta omega-bootstrap
step "omega-bootstrap refinement CKIR — exact relations and selected result across all schema negatives below Delta" omega-bootstrap-refinement checked-ir-refinement-artifact.sh alpha alpha-assembler beta delta-rust omega-bootstrap omega-bootstrap-gates psi
step "omega-bootstrap refinement source tables — independent declarations, types, layout, and CKIR signatures below Delta" omega-bootstrap-refinement checked-ir-refinement-source-tables.sh alpha alpha-assembler beta delta-rust omega-bootstrap omega-bootstrap-gates psi
step "omega-bootstrap refinement source lowering — independent bodies, facts, operations, and terminators below Delta" omega-bootstrap-refinement checked-ir-refinement-source-lowering.sh alpha alpha-assembler beta delta-rust omega-bootstrap omega-bootstrap-gates psi
step "omega-bootstrap refinement ELF — exact CKIR-to-limited-ELF relation and selected observation below Delta" omega-bootstrap-refinement checked-ir-refinement-elf.sh alpha alpha-assembler beta delta-rust omega-bootstrap omega-bootstrap-gates psi
step "omega-bootstrap refinement source result — composed source, CKIR, and limited-ELF observations below Delta" omega-bootstrap-refinement checked-ir-refinement-source-result.sh alpha alpha-assembler beta delta-rust omega-bootstrap omega-bootstrap-gates psi
step "omega-bootstrap scalar Call reference (DIFFERENTIAL ONLY) — exact vocabulary-28 fixture, meaning, lowering, and mutation teeth" omega-bootstrap-gates scalar-call-reference.sh omega-rust psi-rust/semantics/psi-terminal-codec
step "omega-bootstrap Delta scalar-call frontend — table-driven source, native/self-host identity, product validation, and boundaries" omega-bootstrap-gates delta-scalar-call-frontend.sh delta-rust omega-rust psi-rust/semantics/psi-terminal-codec
step "omega-bootstrap Delta artifact — O1/scalar terminal-Psi to deterministic x86-64 ELF" omega-bootstrap-gates delta-terminal-to-elf.sh delta-rust omega-rust psi-rust/semantics/psi-terminal-codec
step "omega-bootstrap Delta self-host composite — lowermachine-built frontend/backend compose O1/scalar through terminal vocabulary 28 to ELF" omega-bootstrap-gates delta-o1-selfhost-composite.sh delta-rust omega-rust psi-rust/semantics/psi-terminal-codec
step "omega-bootstrap Delta artifact meaning (RUST-FREE) — native vs omega2gamma.beta->interp.beta O1/scalar images" omega-bootstrap-gates delta-terminal-to-elf-meaning.sh delta-rust gamma omega-rust psi-rust/semantics/psi-terminal-codec
step "omega meaning — real Omega samples run Rust-free; exits match documented intent" omega-bootstrap-gates omega-meaning.sh gamma corpus
step "omega meaning-TV — the kernel re-computes each covered sample's arithmetic (proof, not comparison)" omega-bootstrap-refinement meaning-tv.sh omega-bootstrap-meaning gamma proof-kernel alpha-assembler beta corpus
step "input-grid meaning TV — input-taking samples proven per documented input vector (substitution closes the program; the whole proof pipe applies per vector)" omega-bootstrap-refinement input-tv.sh omega-bootstrap-meaning gamma proof-kernel alpha-assembler beta corpus
step "meaning-cert cross-check — meaning-TV certs replayed through check.beta AND check_ref.py" omega-bootstrap-refinement meaning-cert-diamond.sh omega-bootstrap-meaning gamma proof-kernel alpha-assembler beta corpus
step "translation validation — the proof kernel re-evaluates each compilation's result (+ - * < == / %, loops, gcd, cross-machine)" omega-bootstrap-refinement translation-validation.sh omega-bootstrap-meaning delta-rust proof-kernel gamma
step "symbolic loops — beta_symbolic's data-dependent loop summaries (symbolic trip count -> closed form) pinned to the interpreter across an input grid" beta-refinement symbolic-loops.sh
step "refinement — bc's machine code proved to compute its Beta source meaning (instruction-level TV: both meanings auto-derived, equivalence kernel-checked, never run)" beta-refinement refinement.sh alpha proof-kernel alpha-assembler beta
step "refinement-cert cross-check — every refl cert replayed through check.beta AND check_ref.py" beta-refinement refinement-cert-diamond.sh alpha proof-kernel alpha-assembler beta
step "contracts — compiler discharges ensures; the proof kernel checks at build" delta-rust contracts.sh proof-kernel
step "contracts — static discharge and runtime asserts agree (soundness)" delta-rust discharge-soundness.sh proof-kernel
# untrusted proof elaborator (named binders -> raw certs); skipped if python3 is absent
if command -v python3 >/dev/null 2>&1; then
  step "tool — proof elaborator (named binders -> check.beta)" proof-kernel-gates elab-test.sh gamma
  step "tool — proof-library cross-check (WHOLE corpus decided identically by check.beta AND check_ref.py; perturbations rejected)" proof-kernel-gates proofs-crosscheck.sh gamma alpha-assembler beta
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
