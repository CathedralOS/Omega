#!/usr/bin/env sh
# Coherent source/artifact ranged-store induction teeth.

coherent_ranged_build_teeth() {
  # Build the projection immediately before ranged-store induction. Coherent
  # mutations must retain every earlier custody and stack-bound fact.
  sed 's/jeq r2, r3, ranged_bounds_init/jeq r2, r3, scan_owned_effects_init/' \
    "$GATE_DIR/bc-stack-register-custody.alpha" > "$T/bc-stack-register-pre-ranged.alpha"
  cat "$GATE_DIR/bc-block-control.alpha" \
    "$GATE_DIR/bc-effect-sites.alpha" \
    "$GATE_DIR/bc-frame-shape.alpha" \
    "$GATE_DIR/bc-local-access.alpha" \
    "$GATE_DIR/bc-memory-sites.alpha" \
    "$GATE_DIR/bc-expr-primitives.alpha" \
    "$GATE_DIR/bc-stack-pushes.alpha" \
    "$GATE_DIR/bc-expr-composition.alpha" \
    "$GATE_DIR/bc-raw-load-families.alpha" \
    "$GATE_DIR/bc-call-bounds.alpha" \
    "$T/bc-stack-register-pre-ranged.alpha" > "$T/pre-ranged-check.alpha"
  "$ASM" < "$T/pre-ranged-check.alpha" > "$T/pre-ranged-check.tape"
  stamp_seed "$T/pre-ranged-check.tape" "$SEED" "$T/pre-ranged-check" >/dev/null
}

coherent_ranged_mutant() { # label
  ranged_label=$1
  case "$ranged_label" in
    slurp-cap)
      sed 's/to full when (n == 1048576)/to full when (n == 1048577)/' \
        "$SOURCE" > "$T/$ranged_label.beta"
      ;;
    declare-cap)
      sed 's/to write when (s < 1024)/to write when (s <= 1024)/' \
        "$SOURCE" > "$T/$ranged_label.beta"
      ;;
    nloc-step)
      sed 's/word\[2097128\] = s + 1/word[2097128] = s - 1/' \
        "$SOURCE" > "$T/$ranged_label.beta"
      ;;
    *)
      exit 2
      ;;
  esac
  "$T/bc" < "$T/$ranged_label.beta" > "$T/$ranged_label.alpha"
  "$ASM" < "$T/$ranged_label.alpha" > "$T/$ranged_label.tape"
  python3 "$GATE_DIR/bc_block_control_map.py" \
    --repo "$OMEGA_REPO_ROOT" \
    --source "$T/$ranged_label.beta" \
    --assembly "$T/$ranged_label.alpha" \
    --tape "$T/$ranged_label.tape" \
    --output "$T/$ranged_label.witness"
  ranged_source_len=$(wc -c < "$T/$ranged_label.beta" | tr -d ' ')
  ranged_tape_len=$(wc -c < "$T/$ranged_label.tape" | tr -d ' ')
  u32_file "$ranged_source_len" "$T/$ranged_label-source.len"
  u32_file "$ranged_tape_len" "$T/$ranged_label-tape.len"
  cat "$T/$ranged_label-source.len" "$T/$ranged_label.beta" \
    "$T/$ranged_label-tape.len" "$T/$ranged_label.tape" \
    "$T/$ranged_label.witness" "$T/call-bounds.witness" \
    > "$T/$ranged_label.bundle"

  set +e
  "$T/pre-ranged-check" < "$T/$ranged_label.bundle" > "$T/stdout"
  ranged_pre_status=$?
  set -e
  if [ "$ranged_pre_status" != 0 ] || [ -s "$T/stdout" ]; then
    echo "bc block control FAIL — $ranged_label did not preserve the pre-induction projection" >&2
    exit 1
  fi
  case_run "unsafe ranged-store induction: $ranged_label" 1 "$T/$ranged_label.bundle"
}

coherent_ranged_reject_teeth() {
  # Preserve the historical per-mutant build, projection check, and final
  # rejection order rather than constructing all three mutants up front.
  coherent_ranged_mutant slurp-cap
  coherent_ranged_mutant declare-cap
  coherent_ranged_mutant nloc-step
}
