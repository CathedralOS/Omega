#!/usr/bin/env sh
# Verify the current closed bootstrap lattice, rung by rung, in one command —
# from the hand-audited seed through Gamma. Each step is the rung's own
# gate; this just runs them in dependency order and stops on the first failure.
#
#   alpha   the seed re-derives from source, conforms to SEMANTICS.md, and the
#           platform realizations share provenance/conformance/reproduction gates
#   alpha-assembler  the assembler self-hosts (reproduces its own bytecode byte-for-byte)
#   Beta    the Alpha-rooted compiler artifact compiles + runs the corpus
#   bc      the Beta compiler WRITTEN IN BETA self-hosts
#   Gamma   the lower-rooted interpreter/checker executes the retained meaning gates
#   Delta   source and provisional artifacts are retained, but canonical publication
#           from the lower lattice remains explicit unfinished work
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
  while [ ! -f "$OMEGA_REPO_ROOT/tools/bootstrap/paths.sh" ]; do
    OMEGA_PATH_PARENT=$(dirname -- "$OMEGA_REPO_ROOT")
    if [ "$OMEGA_PATH_PARENT" = "$OMEGA_REPO_ROOT" ]; then
      echo "bootstrap paths: cannot find repository root from $OMEGA_GATE_DIR" >&2
      exit 2
    fi
    OMEGA_REPO_ROOT=$OMEGA_PATH_PARENT
  done
  unset OMEGA_PATH_PARENT
fi
. "$OMEGA_REPO_ROOT/tools/bootstrap/paths.sh" || exit $?
cd "$OMEGA_GATE_DIR"
fail=0
CACHE="$OMEGA_REPO_ROOT/.lattice-cache"
CACHE_PROFILE_DIR=${LATTICE_CACHE_PROFILE_DIR:-"$OMEGA_REPO_ROOT/tests/lattice/lattice-cache-deps"}
mkdir -p "$CACHE"

if [ "${LATTICE_CACHE_CHECK_ONLY:-0}" != "1" ]; then
  sh "$OMEGA_REPO_ROOT/tools/bootstrap/check-path-hygiene.sh" || exit $?
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
CORE=$(hash_inputs "$OMEGA_PATH_ALPHA" "$OMEGA_PATH_BETA" \
  "$OMEGA_PATH_GAMMA" "$OMEGA_PATH_DELTA" \
  "$OMEGA_PATH_BOOTSTRAP_TOOLS_ROOT/paths.sh" \
  "$OMEGA_PATH_BOOTSTRAP_TOOLS_ROOT/check-path-hygiene.sh" \
  "$OMEGA_PATH_BOOTSTRAP_TOOLS_ROOT/test-paths.sh")
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
# The closed lower-rooted lattice currently ends at Gamma. Delta source and
# checked-in artifacts remain, but no external producer is accepted as a
# substitute for reconstructing the canonical Delta compiler from below.
step "delta D0 storage meaning — omega2gamma.beta -> interp.beta" omega-bootstrap-gates delta-storage-meaning.sh omega-bootstrap gamma
step "omega2gamma termination canary — translator halts on every sample, supported or refused" omega-bootstrap-gates omega2gamma-termination.sh alpha-assembler beta corpus
step "omega-bootstrap source bundle — canonical deterministic multi-file input" omega-bootstrap-gates omega-bootstrap-bundle-test.sh
step "omega-bootstrap compilation envelope — canonical package/source/alias transport and malformed/resource teeth" omega-bootstrap-gates omega-bootstrap-compilation-test.sh omega-bootstrap
step "omega-bootstrap two-package fixture — pinned deterministic OMGCOMP and semantic negatives" omega-bootstrap-gates two-unit-compilation-fixture.sh omega-bootstrap-compiler

# Native/self Delta compilation, bridge production, and the downstream
# refinement gates are intentionally absent until TASKS_BOOTSTRAP.md's
# LOWER-ROOTED-DELTA-PUBLICATION work supplies the canonical compiler artifact.

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
  echo "CLOSED LATTICE VERIFIED ✓ — seed → assembler → bc → Gamma; Delta publication remains explicit unfinished work  ($RAN run, $SKIPPED cached)"
else
  echo "LATTICE: one or more rungs FAILED  ($RAN run, $SKIPPED cached)"; exit 1
fi
