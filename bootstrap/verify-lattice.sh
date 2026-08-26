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
# Active successor gates may use checked manifests under lattice-cache-deps/
# instead of hashing an entire owner directory. Unmigrated gates retain the
# coarse behavior, and LATTICE_FULL=1 always bypasses both cache forms.
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
CACHE_PROFILE_DIR=${LATTICE_CACHE_PROFILE_DIR:-"$OMEGA_REPO_ROOT/bootstrap/lattice-cache-deps"}
mkdir -p "$CACHE"

if [ "${LATTICE_CACHE_CHECK_ONLY:-0}" != "1" ]; then
  sh "$OMEGA_REPO_ROOT/bootstrap/check-path-hygiene.sh" || exit $?
fi

# content hash of the given dirs/files (source + scripts only; build outputs excluded)
hash_inputs() {
  { for d in "$@"; do
      d=$(omega_bootstrap_path "$d") || exit $?
      find "$d" -type f \
        -not -path '*/target/*' -not -path '*/build/*' -not -path '*/.git/*' \
        \( -name '*.beta' -o -name '*.alpha' -o -name '*.gamma' -o -name '*.alp' \
           -o -name '*.omg' -o -name '*.sh' -o -name '*.py' -o -name '*.rs' \
           -o -name '*.s' -o -name '*.toml' -o -name '*.md5' -o -name '*.elab' \
           -o -name '*.hex' -o -name '*.json' -o -name '*.lock' \) -print 2>/dev/null
    done; } | sort | xargs shasum 2>/dev/null | shasum | cut -d' ' -f1
}

# A precise cache profile is an intentionally conservative union of exact
# transitive inputs for a related family of expensive gates. Each non-comment
# row is `script REPOSITORY_PATH` or `input REPOSITORY_PATH`. Directories are
# permitted when the whole subtree is a real input (for example a Rust crate),
# and are filtered by hash_inputs exactly like coarse role directories.
validate_cache_profile() {
  v_profile=$1
  [ -f "$v_profile" ] || {
    echo "lattice cache profile missing: $v_profile" >&2
    return 1
  }
  awk '
    /^[[:space:]]*(#|$)/ { next }
    NF != 2 { print FILENAME ":" FNR ": expected KIND PATH" > "/dev/stderr"; bad=1; next }
    $1 != "script" && $1 != "input" {
      print FILENAME ":" FNR ": unknown kind " $1 > "/dev/stderr"; bad=1; next
    }
    seen[$2]++ {
      print FILENAME ":" FNR ": duplicate path " $2 > "/dev/stderr"; bad=1
    }
    $1 == "script" { scripts++ }
    END {
      if (scripts == 0) { print FILENAME ": no script rows" > "/dev/stderr"; bad=1 }
      exit bad
    }
  ' "$v_profile" || return 1
  while read -r v_kind v_path v_extra; do
    case "$v_kind" in ''|'#'*) continue ;; esac
    case "$v_path" in
      /*|..|../*|*/..|*/../*)
        echo "$v_profile: unsafe repository path: $v_path" >&2
        return 1
        ;;
    esac
    [ -z "$v_extra" ] || {
      echo "$v_profile: path contains whitespace: $v_path $v_extra" >&2
      return 1
    }
    [ -e "$OMEGA_REPO_ROOT/$v_path" ] || {
      echo "$v_profile: missing input: $v_path" >&2
      return 1
    }
  done < "$v_profile"
}

validate_cache_profiles() {
  [ -d "$CACHE_PROFILE_DIR" ] || {
    echo "lattice cache profile directory missing: $CACHE_PROFILE_DIR" >&2
    return 1
  }
  v_count=0
  for v_profile in "$CACHE_PROFILE_DIR"/*.deps; do
    [ -e "$v_profile" ] || {
      echo "lattice cache profile directory has no .deps files" >&2
      return 1
    }
    validate_cache_profile "$v_profile" || return 1
    v_count=$((v_count+1))
  done
  [ "$v_count" -gt 0 ]
}

authorize_cache_profile_script() { # profile-file exact-invoked-script
  a_profile=$1
  a_script=$2
  awk -v script="$a_script" '
    $1 == "script" && $2 == script { found=1 }
    END { exit !found }
  ' "$a_profile" || {
    echo "$a_profile: precise step does not authorize script $a_script" >&2
    return 1
  }
}

hash_cache_profile() { # profile-file
  h_profile=$1
  h_entries=$(awk -v root="$OMEGA_REPO_ROOT" \
    '!/^[[:space:]]*(#|$)/ { print root "/" $2 }' "$h_profile")
  # Profile integrity rejects whitespace, so intentional field splitting here
  # produces one repository-relative dependency per positional parameter.
  # shellcheck disable=SC2086
  set -- $h_entries
  h_manifest=$(shasum "$h_profile" | cut -d' ' -f1)
  h_inputs=$(hash_inputs "$@") || return 1
  printf '%s:%s' "$h_manifest" "$h_inputs"
}

validate_cache_profiles || exit $?
if [ "${LATTICE_CACHE_CHECK_ONLY:-0}" = "profiles" ]; then
  echo "lattice cache profiles: manifests validated"
  exit 0
fi

# The language spine and its shared path plumbing sit under every step. Assurance
# services are deliberately excluded here: steps that consume the proof kernel
# declare the proof-kernel role and hash it independently.
CORE=$(hash_inputs "$OMEGA_PATH_RUNGS_ROOT" \
  "$OMEGA_PATH_BOOTSTRAP_ROOT/paths.sh" \
  "$OMEGA_PATH_BOOTSTRAP_ROOT/check-path-hygiene.sh" \
  "$OMEGA_PATH_BOOTSTRAP_ROOT/test-paths.sh")
RAN=0; SKIPPED=0; PRECISE_CHECKED=0

run_hashed_step() {
  if [ "${LATTICE_CACHE_CHECK_ONLY:-0}" = "1" ]; then
    return
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

step() {  # label dir script [extra dep dirs...]
  s_label="$1"; s_role="$2"; s_script="$3"; shift 3
  if [ "${LATTICE_CACHE_CHECK_ONLY:-0}" = "1" ]; then
    return
  fi
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
  run_hashed_step
}

precise_step() { # label dir script cache-profile
  s_label="$1"; s_role="$2"; s_script="$3"; s_profile_name="$4"
  s_dir=$(omega_bootstrap_path "$s_role") || exit $?
  s_script_path="$s_dir/$s_script"
  case "$s_script_path" in
    "$OMEGA_REPO_ROOT"/*) s_script_rel=${s_script_path#"$OMEGA_REPO_ROOT/"} ;;
    *) echo "precise lattice script is outside repository: $s_script_path" >&2; exit 2 ;;
  esac
  [ -f "$s_script_path" ] || {
    echo "precise lattice script missing: $s_script_path" >&2
    exit 2
  }
  s_profile="$CACHE_PROFILE_DIR/$s_profile_name.deps"
  authorize_cache_profile_script "$s_profile" "$s_script_rel" || exit $?
  case "$s_profile_name" in
    omega-bootstrap-ckir4-7)
      if [ -z "${CACHE_HASH_CKIR4_7+x}" ]; then
        CACHE_HASH_CKIR4_7=$(hash_cache_profile "$s_profile") || exit $?
      fi
      s_profile_hash=$CACHE_HASH_CKIR4_7
      ;;
    omega-bootstrap-omgrfn7-9)
      if [ -z "${CACHE_HASH_OMGRFN7_9+x}" ]; then
        CACHE_HASH_OMGRFN7_9=$(hash_cache_profile "$s_profile") || exit $?
      fi
      s_profile_hash=$CACHE_HASH_OMGRFN7_9
      ;;
    omega-bootstrap-ckir8)
      if [ -z "${CACHE_HASH_CKIR8+x}" ]; then
        CACHE_HASH_CKIR8=$(hash_cache_profile "$s_profile") || exit $?
      fi
      s_profile_hash=$CACHE_HASH_CKIR8
      ;;
    omega-bootstrap-ckir9)
      if [ -z "${CACHE_HASH_CKIR9+x}" ]; then
        CACHE_HASH_CKIR9=$(hash_cache_profile "$s_profile") || exit $?
      fi
      s_profile_hash=$CACHE_HASH_CKIR9
      ;;
    omega-bootstrap-ckir10)
      if [ -z "${CACHE_HASH_CKIR10+x}" ]; then
        CACHE_HASH_CKIR10=$(hash_cache_profile "$s_profile") || exit $?
      fi
      s_profile_hash=$CACHE_HASH_CKIR10
      ;;
    omega-bootstrap-ckir11)
      if [ -z "${CACHE_HASH_CKIR11+x}" ]; then
        CACHE_HASH_CKIR11=$(hash_cache_profile "$s_profile") || exit $?
      fi
      s_profile_hash=$CACHE_HASH_CKIR11
      ;;
    omega-bootstrap-ckir12)
      if [ -z "${CACHE_HASH_CKIR12+x}" ]; then
        CACHE_HASH_CKIR12=$(hash_cache_profile "$s_profile") || exit $?
      fi
      s_profile_hash=$CACHE_HASH_CKIR12
      ;;
    omega-bootstrap-omgrfn10)
      if [ -z "${CACHE_HASH_OMGRFN10+x}" ]; then
        CACHE_HASH_OMGRFN10=$(hash_cache_profile "$s_profile") || exit $?
      fi
      s_profile_hash=$CACHE_HASH_OMGRFN10
      ;;
    omega-bootstrap-omgrfn11)
      if [ -z "${CACHE_HASH_OMGRFN11+x}" ]; then
        CACHE_HASH_OMGRFN11=$(hash_cache_profile "$s_profile") || exit $?
      fi
      s_profile_hash=$CACHE_HASH_OMGRFN11
      ;;
    omega-bootstrap-omgrfn12)
      if [ -z "${CACHE_HASH_OMGRFN12+x}" ]; then
        CACHE_HASH_OMGRFN12=$(hash_cache_profile "$s_profile") || exit $?
      fi
      s_profile_hash=$CACHE_HASH_OMGRFN12
      ;;
    omega-bootstrap-omgrfn13)
      if [ -z "${CACHE_HASH_OMGRFN13+x}" ]; then
        CACHE_HASH_OMGRFN13=$(hash_cache_profile "$s_profile") || exit $?
      fi
      s_profile_hash=$CACHE_HASH_OMGRFN13
      ;;
    omega-bootstrap-omgrfn14)
      if [ -z "${CACHE_HASH_OMGRFN14+x}" ]; then
        CACHE_HASH_OMGRFN14=$(hash_cache_profile "$s_profile") || exit $?
      fi
      s_profile_hash=$CACHE_HASH_OMGRFN14
      ;;
    *)
      s_profile_hash=$(hash_cache_profile "$s_profile") || exit $?
      ;;
  esac
  s_variant=${LATTICE_STEP_VARIANT:-default}
  s_key=$(printf '%s_%s' "$s_role" "$s_script" | tr '/ .' '___')
  s_hash="$CORE:precise-v1:$s_variant:$s_profile_name:$s_profile_hash"
  PRECISE_CHECKED=$((PRECISE_CHECKED+1))
  run_hashed_step
}

step "alpha — seed (provenance + behavior + reproduction)" alpha       verify.sh
step "alpha — reference VM agrees with the host realization" alpha diamond-py.sh
step "alpha — VM FUZZ: seed vs reference over random arithmetic tapes (signedness/wraparound/traps)" alpha vm-fuzz.sh
step "alpha — assembler self-hosts"                   alpha-assembler selfhost.sh
step "alpha — REFERENCE: asm_ref.py agrees with the lattice assembler over the corpus" alpha-assembler asm-diamond.sh beta proof-kernel
step "alpha — whole-token registers and r+digit labels agree with the reference" alpha-assembler register-label-regression.sh alpha beta
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
step "proof kernel — Gamma implementation"           proof-kernel-gates gamma-checker.sh gamma
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
precise_step "omega-bootstrap OMGCOMP2 custody — exact Linux-x86-64/native-provider target configuration with opaque source semantics" omega-bootstrap-gates delta-compilation-envelope-v2.sh omega-bootstrap-omgcomp2
step "omega-bootstrap two-package fixture — pinned deterministic OMGCOMP and semantic negatives" omega-bootstrap-gates two-unit-compilation-fixture.sh omega-bootstrap-compiler
precise_step "omega-bootstrap bounded SHA-256 — exact raw-envelope digest native/self and resource boundary" omega-bootstrap-gates delta-sha256.sh omega-bootstrap-sha256
precise_step "omega-bootstrap bounded SHA-256 meaning (RUST-FREE) — exact abc digest through Gamma" omega-bootstrap-gates delta-sha256-meaning.sh omega-bootstrap-sha256
step "omega-bootstrap Delta resolution handoff — exact OMGCOMP to canonical OMGRSW1, native/self/resource agreement" omega-bootstrap-gates delta-resolution-handoff.sh omega-bootstrap-compiler delta-rust
step "omega-bootstrap resolution meaning (RUST-FREE) — canonical 0/251/252 through Gamma" omega-bootstrap-gates delta-resolution-handoff-meaning.sh omega-bootstrap-compiler omega-bootstrap-meaning delta-rust gamma
precise_step "omega-bootstrap OMGRSW6 independent resolution — exact boundary requirement/candidate/call tables with no selection" omega-bootstrap-gates delta-provider-resolution-v6-reference.sh omega-bootstrap-omgrsw6
precise_step "omega-bootstrap OMGRSW6 handoff — exact OMGCOMP2 native/self resolution-only provider graph" omega-bootstrap-gates delta-provider-resolution-v6-handoff.sh omega-bootstrap-omgrsw6
precise_step "omega-bootstrap OMGRSW6 meaning (RUST-FREE) — exact publication plus semantic/resource refusal through Gamma" omega-bootstrap-gates delta-provider-resolution-v6-meaning.sh omega-bootstrap-omgrsw6-meaning
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
precise_step "omega-bootstrap generated-source custody — sealed reproduction recipe, exact OMGCOMP1 extent, and no-partial-publication teeth" omega-bootstrap-gates generated-source-custody.sh omega-bootstrap-generated-source
step "omega-bootstrap CKIR3 resolved-source lowerer — constant DAG, <=, guardless and cyclic control, native/self relation" omega-bootstrap-gates delta-resolved-to-ckir3.sh omega-bootstrap-compiler delta-rust psi
step "omega-bootstrap CKIR3 greatest source frame — exact canonical 728680-byte input and adjacent exhaustion" omega-bootstrap-gates delta-resolved-to-ckir3-greatest-frame.sh omega-bootstrap-compiler delta-rust
step "omega-bootstrap CKIR3 lowering meaning (RUST-FREE) — representative constant DAG, aggregate copy, <=, and 251/252 through Gamma" omega-bootstrap-gates delta-resolved-to-ckir3-meaning.sh omega-bootstrap-compiler omega-bootstrap-meaning delta-rust gamma
step "omega-bootstrap CKIR3 backend — derived read-only image, conditional ELF segments, and exact reconstruction" omega-bootstrap-gates delta-checked-ir-v3-backend.sh omega-bootstrap-compiler delta-rust
step "omega-bootstrap CKIR3 resources — canonical graph, wire, image, frame, text, and ELF boundaries" omega-bootstrap-gates delta-checked-ir-v3-resources.sh omega-bootstrap-compiler delta-rust
step "omega-bootstrap CKIR3 backend meaning (RUST-FREE) — exact three-segment ELF/result and 251/252 through Gamma" omega-bootstrap-gates delta-checked-ir-v3-backend-meaning.sh omega-bootstrap-compiler omega-bootstrap-meaning delta-rust gamma
step "omega-bootstrap CKIR3 producer composite — native/self/mixed producer-backend pairs and independent result/ELF" omega-bootstrap-gates delta-ckir3-composite.sh omega-bootstrap-compiler delta-rust psi \
  "$OMEGA_REPO_ROOT/bootstrap/omega-bootstrap/compiler/OMEGA_BOOTSTRAP_GENERATED_SOURCE_CUSTODY.md" \
  "$OMEGA_REPO_ROOT/bootstrap/omega-bootstrap/gates/fixtures/generated-source-custody/unicode-tables.recipe.json" \
  "$OMEGA_REPO_ROOT/bootstrap/omega-bootstrap/gates/generated_source_custody.py" \
  "$OMEGA_REPO_ROOT/bootstrap/onramps/omega-rust/psi/pipeline/psi-source-files-to-tokens/src/bin/generate_omega_unicode.rs" \
  "$OMEGA_REPO_ROOT/bootstrap/onramps/omega-rust/psi/pipeline/psi-source-files-to-tokens/Cargo.toml" \
  "$OMEGA_REPO_ROOT/Cargo.toml" "$OMEGA_REPO_ROOT/Cargo.lock" \
  "$OMEGA_REPO_ROOT/compiler/psi/generated/unicode_tables.omg"
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
step "omega-bootstrap OMGRFN4 layer 2 — independent Unicode source-to-role-3 witness resolution below Delta" omega-bootstrap-refinement omgrfn4-source-witness-independent.sh alpha alpha-assembler beta delta-rust omega-bootstrap omega-bootstrap-gates
step "omega-bootstrap OMGRFN4 layer 3 — independent witness-to-CKIR3 declarations, layout, selected entry, and intrinsic constant DAG" omega-bootstrap-refinement omgrfn4-witness-ckir3-tables.sh alpha alpha-assembler beta delta-rust omega-bootstrap omega-bootstrap-gates
step "omega-bootstrap OMGRFN4 layer 4 boundary evidence — source-only active frames and dynamic block entries" omega-bootstrap-refinement omgrfn4-source-only-boundaries.sh alpha alpha-assembler beta delta-rust omega-bootstrap omega-bootstrap-gates
step "omega-bootstrap OMGRFN4 layer 5 boundary evidence — CKIR-only active frames and dynamic block entries" omega-bootstrap-refinement omgrfn4-ckir3-evaluator-resources.sh alpha alpha-assembler beta delta-rust omega-bootstrap omega-bootstrap-gates
step "omega-bootstrap OMGRFN4 layer 4 lowering — source bodies, operations, constant roots, and cyclic intervals below Delta" omega-bootstrap-refinement omgrfn4-resolved-body-result.sh alpha alpha-assembler beta delta-rust omega-bootstrap omega-bootstrap-gates
step "omega-bootstrap OMGRFN4 layer 4 meaning — physically artifact-free source result below Delta" omega-bootstrap-refinement omgrfn4-source-only-result.sh alpha alpha-assembler beta delta-rust omega-bootstrap omega-bootstrap-gates
step "omega-bootstrap OMGRFN4 layer 5a — complete CKIR3/result validation below Delta" omega-bootstrap-refinement omgrfn4-ckir3-artifact-result.sh alpha alpha-assembler beta delta-rust omega-bootstrap omega-bootstrap-gates
step "omega-bootstrap OMGRFN4 layer 5b — exact CKIR3-to-ELF reconstruction below Delta" omega-bootstrap-refinement omgrfn4-ckir3-refinement-elf.sh alpha alpha-assembler beta delta-rust omega-bootstrap omega-bootstrap-gates
step "omega-bootstrap OMGRFN4 composite — all five independent responsibilities consume one exact constant-aggregate frame" omega-bootstrap-refinement omgrfn4-same-frame-composite.sh alpha alpha-assembler beta delta-rust omega-bootstrap omega-bootstrap-gates \
  "$OMEGA_REPO_ROOT/bootstrap/omega-bootstrap/compiler/OMEGA_BOOTSTRAP_GENERATED_SOURCE_CUSTODY.md" \
  "$OMEGA_REPO_ROOT/bootstrap/omega-bootstrap/gates/fixtures/generated-source-custody/unicode-tables.recipe.json" \
  "$OMEGA_REPO_ROOT/bootstrap/omega-bootstrap/gates/generated_source_custody.py" \
  "$OMEGA_REPO_ROOT/bootstrap/onramps/omega-rust/psi/pipeline/psi-source-files-to-tokens/src/bin/generate_omega_unicode.rs" \
  "$OMEGA_REPO_ROOT/bootstrap/onramps/omega-rust/psi/pipeline/psi-source-files-to-tokens/Cargo.toml" \
  "$OMEGA_REPO_ROOT/Cargo.toml" "$OMEGA_REPO_ROOT/Cargo.lock" \
  "$OMEGA_REPO_ROOT/compiler/psi/generated/unicode_tables.omg"
precise_step "omega-bootstrap CKIR4 resolved-source lowerer — runtime records plus versioned direct field receivers" omega-bootstrap-gates delta-resolved-to-ckir4.sh omega-bootstrap-ckir4-7
precise_step "omega-bootstrap CKIR4 lowering meaning (RUST-FREE) — constructor/field-receiver/Call 0/251/252 through Gamma" omega-bootstrap-gates delta-resolved-to-ckir4-meaning.sh omega-bootstrap-ckir4-7
precise_step "omega-bootstrap CKIR4 independent reference — constructor objects, result, exact ELF, and mutation sweep" omega-bootstrap-gates delta-checked-ir-v4-reference.sh omega-bootstrap-ckir4-7
precise_step "omega-bootstrap CKIR4 backend — immutable object extents and exact constructor ELF templates" omega-bootstrap-gates delta-checked-ir-v4-backend.sh omega-bootstrap-ckir4-7
precise_step "omega-bootstrap CKIR4 backend meaning (RUST-FREE) — exact constructor ELF/result and 251/252 through Gamma" omega-bootstrap-gates delta-checked-ir-v4-backend-meaning.sh omega-bootstrap-ckir4-7
precise_step "omega-bootstrap CKIR5 independent reference — pure sums, constructors, dispatch, payload bindings, and resource teeth" omega-bootstrap-gates delta-checked-ir-v5-reference.sh omega-bootstrap-ckir4-7
precise_step "omega-bootstrap CKIR5 resolved-source lowerer — OMGLOW6 construction/Copy/Call/dispatch, native/self exact result 70" omega-bootstrap-gates delta-resolved-to-ckir5.sh omega-bootstrap-ckir4-7
precise_step "omega-bootstrap CKIR5 backend — private sum layout, selected payload snapshots, exact ELF, and frozen CKIR4 parity" omega-bootstrap-gates delta-checked-ir-v5-backend.sh omega-bootstrap-ckir4-7
precise_step "omega-bootstrap CKIR6 independent reference — bool-only LogicalNot meaning, identity, and resource teeth" omega-bootstrap-gates delta-checked-ir-v6-reference.sh omega-bootstrap-ckir4-7
precise_step "omega-bootstrap CKIR6 resolved-source lowerer — OMGLOW7 with least OMGRSW1/2/3 and native/self result 70" omega-bootstrap-gates delta-resolved-to-ckir6.sh omega-bootstrap-ckir4-7
precise_step "omega-bootstrap CKIR6 lowering meaning (RUST-FREE) — least OMGRSW1 LogicalNot 0/251/252 through Gamma" omega-bootstrap-gates delta-resolved-to-ckir6-meaning.sh omega-bootstrap-ckir4-7
precise_step "omega-bootstrap CKIR6 backend — exact LogicalNot load/xor-one/store template and native/self artifact identity" omega-bootstrap-gates delta-checked-ir-v6-backend.sh omega-bootstrap-ckir4-7
precise_step "omega-bootstrap CKIR7 independent reference — pure Boolean AND/OR truth functions, identity, and resource teeth" omega-bootstrap-gates delta-checked-ir-v7-reference.sh omega-bootstrap-ckir4-7
precise_step "omega-bootstrap CKIR7 resolved-source lowerer — OMGLOW8 pure/nontrapping &&/|| with least OMGRSW1/2/3" omega-bootstrap-gates delta-resolved-to-ckir7.sh omega-bootstrap-ckir4-7
precise_step "omega-bootstrap CKIR7 lowering meaning (RUST-FREE) — short-circuit-equivalent pure Boolean 0/251/252 through Gamma" omega-bootstrap-gates delta-resolved-to-ckir7-meaning.sh omega-bootstrap-ckir4-7
precise_step "omega-bootstrap CKIR7 backend — exact LogicalAnd/LogicalOr templates and native/self artifact identity" omega-bootstrap-gates delta-checked-ir-v7-backend.sh omega-bootstrap-ckir4-7
precise_step "omega-bootstrap CKIR8 independent reference — primitive bool/u8/u32 ScalarEqual meaning, identity, and resource teeth" omega-bootstrap-gates delta-checked-ir-v8-reference.sh omega-bootstrap-ckir8
precise_step "omega-bootstrap CKIR8 resolved-source lowerer — OMGLOW9 pure/nontrapping same-carrier equality" omega-bootstrap-gates delta-resolved-to-ckir8.sh omega-bootstrap-ckir8
precise_step "omega-bootstrap CKIR8 lowering meaning (RUST-FREE) — scalar equality 0/251/252 through Gamma" omega-bootstrap-gates delta-resolved-to-ckir8-meaning.sh omega-bootstrap-ckir8
precise_step "omega-bootstrap CKIR8 backend — exact CMP/SETE/MOVZX template and native/self artifact identity" omega-bootstrap-gates delta-checked-ir-v8-backend.sh omega-bootstrap-ckir8
precise_step "omega-bootstrap CKIR9 independent reference — same-carrier u8/u32 Greater/GreaterEqual meaning, identity, and resource teeth" omega-bootstrap-gates delta-checked-ir-v9-reference.sh omega-bootstrap-ckir9
precise_step "omega-bootstrap CKIR9 resolved-source lowerer — OMGLOWA pure/nontrapping same-carrier >/>=" omega-bootstrap-gates delta-resolved-to-ckir9.sh omega-bootstrap-ckir9
precise_step "omega-bootstrap CKIR9 lowering meaning (RUST-FREE) — ordered scalar comparison 0/251/252 through Gamma" omega-bootstrap-gates delta-resolved-to-ckir9-meaning.sh omega-bootstrap-ckir9
precise_step "omega-bootstrap CKIR9 backend — exact CMP/SETA/SETAE/MOVZX templates and native/self artifact identity" omega-bootstrap-gates delta-checked-ir-v9-backend.sh omega-bootstrap-ckir9
precise_step "omega-bootstrap CKIR10 independent reference — u8 as u32 in Trapping IntegerWiden meaning, identity, and resource teeth" omega-bootstrap-gates delta-checked-ir-v10-reference.sh omega-bootstrap-ckir10
precise_step "omega-bootstrap CKIR10 resolved-source lowerer — OMGLOWB exact unsigned u8-to-u32 widening" omega-bootstrap-gates delta-resolved-to-ckir10.sh omega-bootstrap-ckir10
precise_step "omega-bootstrap CKIR10 lowering meaning (RUST-FREE) — IntegerWiden 0/251/252 through Gamma" omega-bootstrap-gates delta-resolved-to-ckir10-meaning.sh omega-bootstrap-ckir10
precise_step "omega-bootstrap CKIR10 backend — exact opcode-21 IntegerWiden load/store template and native/self artifact identity" omega-bootstrap-gates delta-checked-ir-v10-backend.sh omega-bootstrap-ckir10
precise_step "omega-bootstrap CKIR11 independent reference — canonical trapping-u32 leaf-plus-literal Add meaning and resource teeth" omega-bootstrap-gates delta-checked-ir-v11-reference.sh omega-bootstrap-ckir11
precise_step "omega-bootstrap CKIR11 resolved-source lowerer — OMGLOWC selected trapping addition in admitted contexts" omega-bootstrap-gates delta-resolved-to-ckir11.sh omega-bootstrap-ckir11
precise_step "omega-bootstrap CKIR11 lowering meaning (RUST-FREE) — successful and runtime-overflow Add through Gamma" omega-bootstrap-gates delta-resolved-to-ckir11-meaning.sh omega-bootstrap-ckir11
precise_step "omega-bootstrap CKIR11 backend — exact Add/carry/range/store template and native/self artifact identity" omega-bootstrap-gates delta-checked-ir-v11-backend.sh omega-bootstrap-ckir11
precise_step "omega-bootstrap OMGRSW4 resolution — bounded shared-byte views and plain-ASCII literals" omega-bootstrap-gates delta-shared-byte-view-resolution-handoff.sh omega-bootstrap-ckir12
precise_step "omega-bootstrap CKIR12 independent reference — program-static shared-byte-view meaning and resource teeth" omega-bootstrap-gates delta-checked-ir-v12-reference.sh omega-bootstrap-ckir12
precise_step "omega-bootstrap CKIR12 resolved-source lowerer — OMGLOWD guarded head and one-byte tail" omega-bootstrap-gates delta-resolved-to-ckir12.sh omega-bootstrap-ckir12
precise_step "omega-bootstrap CKIR12 backend — exact descriptor, guarded head/tail, and native/self artifact identity" omega-bootstrap-gates delta-checked-ir-v12-backend.sh omega-bootstrap-ckir12
precise_step "omega-bootstrap CKIR13 resolved-source lowerer — OMGRSW5/OMGLOWE direct full-u32 trapping subtraction" omega-bootstrap-gates delta-resolved-to-ckir13.sh omega-bootstrap-ckir13
precise_step "omega-bootstrap CKIR13 lowering meaning (RUST-FREE) — full-u32 subtraction success/underflow and 251/252 through Gamma" omega-bootstrap-gates delta-resolved-to-ckir13-meaning.sh omega-bootstrap-ckir13-meaning
precise_step "omega-bootstrap CKIR13 backend — exact full-u32 SUB/borrow/range/store native/self artifact identity" omega-bootstrap-gates delta-checked-ir-v13-backend.sh omega-bootstrap-ckir13
step "omega-bootstrap OMGRFN5/6/7 layer 1 — exact successor frame, OMGCOMP graph, and source custody" omega-bootstrap-refinement omgrfn5-frame-omgcomp-custody.sh alpha alpha-assembler beta omega-bootstrap omega-bootstrap-gates
step "omega-bootstrap OMGRFN5 layer 2 — independent runtime-record source resolution below Delta" omega-bootstrap-refinement omgrfn5-source-witness-independent.sh alpha alpha-assembler beta delta-rust omega-bootstrap omega-bootstrap-gates
step "omega-bootstrap OMGRFN5 layer 3 — independent witness-to-CKIR4 declarations, layout, root, and intrinsic envelope" omega-bootstrap-refinement omgrfn5-witness-ckir4-tables.sh alpha alpha-assembler beta delta-rust omega-bootstrap omega-bootstrap-gates
step "omega-bootstrap OMGRFN5 layer 4 — constructor source lowering and artifact-free source meaning below Delta" omega-bootstrap-refinement omgrfn5-source-lowering-meaning.sh alpha alpha-assembler beta delta-rust omega-bootstrap omega-bootstrap-gates
step "omega-bootstrap OMGRFN5 layer 5a — complete CKIR4/result validation below Delta" omega-bootstrap-refinement omgrfn5-ckir4-artifact-result.sh alpha alpha-assembler beta delta-rust omega-bootstrap omega-bootstrap-gates
step "omega-bootstrap OMGRFN5 layer 5b — exact CKIR4-to-ELF reconstruction below Delta" omega-bootstrap-refinement omgrfn5-ckir4-refinement-elf.sh alpha alpha-assembler beta delta-rust omega-bootstrap omega-bootstrap-gates
step "omega-bootstrap OMGRFN5 composite — all five independent responsibilities consume two exact runtime-record carriers" omega-bootstrap-refinement omgrfn5-same-frame-composite.sh alpha alpha-assembler beta delta-rust omega-bootstrap omega-bootstrap-gates
precise_step "omega-bootstrap OMGRFN7 layer 2 — independent pure-sum source-to-OMGRSW3 reconstruction below Delta" omega-bootstrap-refinement omgrfn7-source-witness-independent.sh omega-bootstrap-omgrfn7-9
precise_step "omega-bootstrap OMGRFN7 layer 3 — independent OMGRSW3-to-CKIR5 sums, layout, constructors, dispatch arms, and payload identities" omega-bootstrap-refinement omgrfn7-witness-ckir5-tables.sh omega-bootstrap-omgrfn7-9
precise_step "omega-bootstrap OMGRFN7 layer 4a — exact pure-sum source-to-CKIR5 lowering below Delta" omega-bootstrap-refinement omgrfn7-source-ckir5-lowering.sh omega-bootstrap-omgrfn7-9
precise_step "omega-bootstrap OMGRFN7 layer 4b — artifact-free pure-sum source meaning below Delta" omega-bootstrap-refinement omgrfn7-source-lowering-meaning.sh omega-bootstrap-omgrfn7-9
precise_step "omega-bootstrap OMGRFN7 layer 5a — independent complete CKIR5 structure below Delta" omega-bootstrap-refinement omgrfn7-ckir5-structure.sh omega-bootstrap-omgrfn7-9
precise_step "omega-bootstrap OMGRFN7 layer 5b — independent CKIR5 result below Delta" omega-bootstrap-refinement omgrfn7-ckir5-result.sh omega-bootstrap-omgrfn7-9
precise_step "omega-bootstrap OMGRFN7 layer 5c — exact CKIR5-to-ELF reconstruction below Delta" omega-bootstrap-refinement omgrfn7-ckir5-elf.sh omega-bootstrap-omgrfn7-9
precise_step "omega-bootstrap OMGRFN7 composite — all five independent responsibilities consume one exact pure-sum frame" omega-bootstrap-refinement omgrfn7-same-frame-composite.sh omega-bootstrap-omgrfn7-9
precise_step "omega-bootstrap OMGRFN8 composite — all five independent responsibilities consume one exact logical-negation frame" omega-bootstrap-refinement omgrfn8-same-frame-composite.sh omega-bootstrap-omgrfn7-9
precise_step "omega-bootstrap OMGRFN9 composite — all five independent responsibilities consume one exact logical-binary frame" omega-bootstrap-refinement omgrfn9-same-frame-composite.sh omega-bootstrap-omgrfn7-9
precise_step "omega-bootstrap OMGRFN10 composite — all five independent responsibilities consume one exact primitive-equality frame" omega-bootstrap-refinement omgrfn10-same-frame-composite.sh omega-bootstrap-omgrfn10
precise_step "omega-bootstrap OMGRFN11 composite — all five independent responsibilities consume one exact primitive-ordered-comparison frame" omega-bootstrap-refinement omgrfn11-same-frame-composite.sh omega-bootstrap-omgrfn11
precise_step "omega-bootstrap OMGRFN12 composite — all five independent responsibilities consume one exact primitive-integer-widen frame" omega-bootstrap-refinement omgrfn12-same-frame-composite.sh omega-bootstrap-omgrfn12
precise_step "omega-bootstrap OMGRFN13 composite — all five independent responsibilities consume one exact canonical trapping-add frame" omega-bootstrap-refinement omgrfn13-same-frame-composite.sh omega-bootstrap-omgrfn13
precise_step "omega-bootstrap OMGRFN14 composite — all five independent responsibilities consume exact static shared-byte-view frames" omega-bootstrap-refinement omgrfn14-same-frame-composite.sh omega-bootstrap-omgrfn14
precise_step "omega-bootstrap OMGRFN15 composite — all five independent responsibilities consume one exact full-u32 subtraction frame" omega-bootstrap-refinement omgrfn15-same-frame-composite.sh omega-bootstrap-omgrfn15
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

if [ "${LATTICE_CACHE_CHECK_ONLY:-0}" = "1" ]; then
  echo "lattice cache profiles: $PRECISE_CHECKED precise call sites and all manifests validated"
  exit 0
fi

echo ""
if [ "$fail" = 0 ]; then
  echo "LATTICE VERIFIED ✓ — seed → assembler → bc → Delta; proof kernel verified; + gamma interp running the checker-in-gamma  ($RAN run, $SKIPPED cached)"
else
  echo "LATTICE: one or more rungs FAILED  ($RAN run, $SKIPPED cached)"; exit 1
fi
