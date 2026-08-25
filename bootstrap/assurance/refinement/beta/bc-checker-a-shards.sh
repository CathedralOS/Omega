#!/usr/bin/env sh
# Ordered, content-keyed orchestration for Checker A's historical mutation
# matrix. The build and rejection phases deliberately retain their historical
# global order; each module publishes green only after its own rejection phase.

checker_a_prepare_mapper_phased() { # name inventory module
  checker_a_mapper_name=$1
  checker_a_mapper_inventory=$2
  checker_a_mapper_module=$3
  shift 3
  bc_prepare_phased_teeth "$checker_a_mapper_name" \
    "$checker_a_mapper_inventory" "$GATE_DIR/$checker_a_mapper_module" \
    "$T/control-check.alpha" "$T/control.bundle" \
    "$ARTIFACT" "$ASM" "$SEED" \
    "$OMEGA_PATH_BETA/artifact_env.sh" "$OMEGA_PATH_ALPHA/seed_env.sh" \
    "$GATE_DIR/bc_block_control_map.py" "$@"
}

checker_a_prepare_call_bounds_phased() { # name inventory module
  checker_a_bounds_name=$1
  checker_a_bounds_inventory=$2
  checker_a_bounds_module=$3
  shift 3
  bc_prepare_phased_teeth "$checker_a_bounds_name" \
    "$checker_a_bounds_inventory" "$GATE_DIR/$checker_a_bounds_module" \
    "$T/control-check.alpha" "$T/control.bundle" \
    "$ARTIFACT" "$ASM" "$SEED" \
    "$OMEGA_PATH_BETA/artifact_env.sh" "$OMEGA_PATH_ALPHA/seed_env.sh" \
    "$GATE_DIR/bc_call_bounds.py" "$@"
}

checker_a_prepare_shards() {
  bc_prepare_standard_phased raw_load_family '1 case' bc-raw-load-family-teeth.sh
  bc_prepare_standard_phased slurp_summary '4 cases' bc-slurp-summary-teeth.sh
  bc_prepare_standard_phased main_slurp '4 cases' bc-main-slurp-teeth.sh
  bc_prepare_standard_phased write_str '5 cases' bc-write-str-teeth.sh
  bc_prepare_standard_phased fixed_emitter '4 cases' bc-fixed-emitter-teeth.sh
  bc_prepare_standard_phased cursor_leaf '6 cases' bc-cursor-leaf-teeth.sh
  bc_prepare_standard_phased skip_ws '16 cases' bc-skip-ws-teeth.sh
  bc_prepare_standard_phased main_ready '7 cases' bc-main-ready-teeth.sh
  bc_prepare_standard_phased main_loop '12 cases' bc-main-loop-teeth.sh
  bc_prepare_standard_phased byte_classifier '10 cases' bc-byte-classifier-teeth.sh
  bc_prepare_standard_phased read_ident '12 cases' bc-read-ident-teeth.sh
  bc_prepare_standard_phased expect '11 cases' bc-expect-teeth.sh
  bc_prepare_standard_phased declare '12 cases' bc-declare-teeth.sh
  bc_prepare_standard_phased let_keyword '12 cases' bc-let-keyword-teeth.sh
  bc_prepare_standard_phased literal_skip '14 cases' bc-literal-skip-teeth.sh

  bc_prepare_standard_phased count_lets '24 cases' bc-count-lets-teeth.sh
  bc_prepare_standard_phased parse_parameter '18 cases' bc-parse-parameter-teeth.sh
  bc_prepare_standard_phased parse_capacity '17 cases' bc-parse-capacity-teeth.sh
  bc_prepare_standard_phased emit_ident '17 cases' bc-emit-ident-teeth.sh
  bc_prepare_standard_phased emit_dec '23 cases' bc-emit-dec-teeth.sh
  bc_prepare_standard_phased fixed_decimal_emitters '19 cases' bc-fixed-decimal-emitters-teeth.sh
  bc_prepare_standard_phased parse_output_prefix '18 cases' bc-parse-output-prefix-teeth.sh
  bc_prepare_standard_phased gen_stmts_boundary '24 cases' bc-gen-stmts-boundary-teeth.sh
  bc_prepare_standard_phased parse_number '24 cases' bc-parse-number-teeth.sh
  bc_prepare_standard_phased parse_char '38 cases' bc-parse-char-teeth.sh
  bc_prepare_standard_phased operator_classifier '24 cases' bc-operator-classifier-teeth.sh
  bc_prepare_standard_phased cmp_op '41 cases' bc-cmp-op-teeth.sh
  bc_prepare_standard_phased fixed_keyword '42 cases' bc-fixed-keyword-teeth.sh

  bc_prepare_standard_phased stack_owner '1 case' bc-stack-owner-teeth.sh
  bc_prepare_standard_phased ranged_static '2 cases' bc-ranged-static-teeth.sh
  bc_prepare_standard_phased ranged_transfer '3 cases' bc-ranged-transfer-teeth.sh
  bc_prepare_standard_phased frame_summary '2 cases' bc-frame-summary-teeth.sh
  bc_prepare_standard_phased counter_potential '3 cases' bc-counter-potential-teeth.sh
  checker_a_prepare_mapper_phased flat_composition '3 cases' \
    bc-flat-composition-teeth.sh \
    "$T/composition-order.bundle" "$T/composition-argument-order.bundle" \
    "$T/composition-store-order.bundle"
  checker_a_prepare_mapper_phased coherent_ranged '3 cases' bc-coherent-ranged-teeth.sh
  checker_a_prepare_call_bounds_phased call_bounds '2 cases' \
    bc-call-bounds-teeth.sh \
    "$T/call-bounds-probe.bundle" "$T/call-bounds-root.bundle"

  checker_a_prepare_mapper_phased artifact_control_flow '5 cases' \
    bc-artifact-control-flow-teeth.sh \
    "$T/retarget.bundle" "$T/operand.bundle" "$T/duplicate.bundle" \
    "$T/missing.bundle" "$T/noncanonical.bundle"
  checker_a_prepare_mapper_phased artifact_effect_emitter '11 cases' \
    bc-artifact-effect-emitter-teeth.sh \
    "$T/call-retarget.bundle" "$T/read-register.bundle" \
    "$T/write-register.bundle" "$T/helper-write.bundle" \
    "$T/emit-byte.bundle" "$T/emit-length.bundle" \
    "$T/emit-pointer.bundle" "$T/emit-helper.bundle" \
    "$T/orphan-io.bundle" "$T/duplicate-event.bundle" \
    "$T/noncanonical-event.bundle"
  checker_a_prepare_mapper_phased artifact_frame_call '7 cases' \
    bc-artifact-frame-call-teeth.sh \
    "$T/frame-size.bundle" "$T/saved-fp.bundle" "$T/frame-base.bundle" \
    "$T/param-offset.bundle" "$T/param-register.bundle" \
    "$T/call-pop-order.bundle" "$T/call-pop-step.bundle"
  checker_a_prepare_mapper_phased artifact_local_access '7 cases' \
    bc-artifact-local-access-teeth.sh \
    "$T/local-load-slot.bundle" "$T/local-store-slot.bundle" \
    "$T/local-base.bundle" "$T/local-load-opcode.bundle" \
    "$T/local-store-opcode.bundle" "$T/duplicate-local.bundle" \
    "$T/noncanonical-local.bundle"
  checker_a_prepare_mapper_phased artifact_raw_memory '7 cases' \
    bc-artifact-raw-memory-teeth.sh \
    "$T/memory-load-width.bundle" "$T/memory-store-width.bundle" \
    "$T/memory-load-register.bundle" "$T/memory-store-register.bundle" \
    "$T/memory-pop-step.bundle" "$T/duplicate-memory.bundle" \
    "$T/noncanonical-memory.bundle"
  checker_a_prepare_mapper_phased artifact_primitive_composition '11 cases' \
    bc-artifact-primitive-composition-teeth.sh \
    "$T/literal-value.bundle" "$T/literal-register.bundle" \
    "$T/arithmetic-opcode.bundle" "$T/arithmetic-pop-step.bundle" \
    "$T/arithmetic-register.bundle" "$T/duplicate-primitive.bundle" \
    "$T/noncanonical-primitive.bundle" "$T/synthetic-literal.bundle" \
    "$T/composition-order.bundle" "$T/composition-argument-order.bundle" \
    "$T/composition-store-order.bundle"
  checker_a_prepare_mapper_phased artifact_comparison '5 cases' \
    bc-artifact-comparison-teeth.sh \
    "$T/comparison-opcode.bundle" "$T/comparison-operand.bundle" \
    "$T/comparison-branch-target.bundle" "$T/comparison-result.bundle" \
    "$T/comparison-pop-step.bundle"
  checker_a_prepare_mapper_phased artifact_stack_push '6 cases' \
    bc-artifact-stack-push-teeth.sh \
    "$T/push-step.bundle" "$T/push-stack-register.bundle" \
    "$T/push-value-register.bundle" "$T/push-opcode.bundle" \
    "$T/duplicate-push.bundle" "$T/cross-block-push.bundle"
  bc_prepare_phased_teeth artifact_structural_survival '41 cases' \
    "$GATE_DIR/bc-artifact-structural-survival-teeth.sh" \
    "$T/control-check.alpha" "$T/control.bundle" \
    "$ARTIFACT" "$ASM" "$SEED" \
    "$OMEGA_PATH_BETA/artifact_env.sh" "$OMEGA_PATH_ALPHA/seed_env.sh" \
    "$GATE_DIR/bc_block_control_map.py" \
    "$GATE_DIR/bc-artifact-structure.alpha" \
    "$T/retarget.tape" "$T/call-retarget.tape" \
    "$T/read-register.tape" "$T/write-register.tape" \
    "$T/helper-write.tape" "$T/emit-byte.tape" \
    "$T/emit-length.tape" "$T/emit-pointer.tape" \
    "$T/emit-helper.tape" "$T/orphan-io.tape" \
    "$T/frame-size.tape" "$T/saved-fp.tape" "$T/frame-base.tape" \
    "$T/param-offset.tape" "$T/param-register.tape" \
    "$T/call-pop-order.tape" "$T/call-pop-step.tape" \
    "$T/local-load-slot.tape" "$T/local-store-slot.tape" \
    "$T/local-base.tape" "$T/local-load-opcode.tape" \
    "$T/local-store-opcode.tape" "$T/memory-load-width.tape" \
    "$T/memory-store-width.tape" "$T/memory-load-register.tape" \
    "$T/memory-store-register.tape" "$T/memory-pop-step.tape" \
    "$T/literal-value.tape" "$T/literal-register.tape" \
    "$T/arithmetic-opcode.tape" "$T/arithmetic-pop-step.tape" \
    "$T/arithmetic-register.tape" "$T/comparison-opcode.tape" \
    "$T/comparison-operand.tape" "$T/comparison-branch-target.tape" \
    "$T/comparison-result.tape" "$T/comparison-pop-step.tape" \
    "$T/push-step.tape" "$T/push-stack-register.tape" \
    "$T/push-value-register.tape" "$T/push-opcode.tape"
}

checker_a_build_shards() {

  bc_phased_build raw_load_family
  bc_phased_build slurp_summary
  bc_phased_build main_slurp
  bc_phased_build write_str
  bc_phased_build fixed_emitter
  bc_phased_build cursor_leaf
  bc_phased_build skip_ws
  bc_phased_build main_ready
  bc_phased_build main_loop
  bc_phased_build byte_classifier
  bc_phased_build read_ident
  bc_phased_build expect
  bc_phased_build declare
  bc_phased_build let_keyword
  bc_phased_build literal_skip

  bc_phased_build count_lets
  bc_phased_build parse_parameter
  bc_phased_build parse_capacity
  bc_phased_build emit_ident
  bc_phased_build emit_dec
  bc_phased_build fixed_decimal_emitters
  bc_phased_build parse_output_prefix
  bc_phased_build gen_stmts_boundary
  bc_phased_build parse_number
  bc_phased_build parse_char
  bc_phased_build operator_classifier
  bc_phased_build cmp_op
  bc_phased_build fixed_keyword

  bc_phased_build stack_owner
  bc_phased_build ranged_static
  bc_phased_build ranged_transfer
  bc_phased_build frame_summary
  bc_phased_build counter_potential
  bc_phased_build flat_composition
  bc_phased_build coherent_ranged
  bc_phased_build call_bounds

  bc_phased_build artifact_control_flow
  bc_phased_build artifact_effect_emitter
  bc_phased_build artifact_frame_call
  bc_phased_build artifact_local_access
  bc_phased_build artifact_raw_memory
  bc_phased_build artifact_primitive_composition
  bc_phased_build artifact_comparison
  bc_phased_build artifact_stack_push
  bc_phased_build artifact_structural_survival
}

checker_a_reject_shards() {

  bc_phased_reject_commit raw_load_family
  bc_phased_reject_commit slurp_summary
  bc_phased_reject_commit main_slurp
  bc_phased_reject_commit write_str
  bc_phased_reject_commit fixed_emitter
  bc_phased_reject_commit cursor_leaf
  bc_phased_reject_commit skip_ws
  bc_phased_reject_commit main_ready
  bc_phased_reject_commit main_loop
  bc_phased_reject_commit byte_classifier
  bc_phased_reject_commit read_ident
  bc_phased_reject_commit expect
  bc_phased_reject_commit declare
  bc_phased_reject_commit let_keyword
  bc_phased_reject_commit literal_skip

  bc_phased_reject_commit count_lets
  bc_phased_reject_commit parse_parameter
  bc_phased_reject_commit parse_capacity
  bc_phased_reject_commit emit_ident
  bc_phased_reject_commit emit_dec
  bc_phased_reject_commit fixed_decimal_emitters
  bc_phased_reject_commit parse_output_prefix
  bc_phased_reject_commit gen_stmts_boundary
  bc_phased_reject_commit parse_number
  bc_phased_reject_commit parse_char
  bc_phased_reject_commit operator_classifier
  bc_phased_reject_commit cmp_op
  bc_phased_reject_commit fixed_keyword

  bc_phased_reject_commit coherent_ranged
  bc_phased_reject_commit call_bounds
  bc_phased_reject_commit stack_owner
  bc_phased_reject_commit ranged_static
  bc_phased_reject_commit ranged_transfer
  bc_phased_reject_commit frame_summary
  bc_phased_reject_commit counter_potential
  bc_phased_reject_commit flat_composition

  bc_phased_reject_commit artifact_control_flow
  bc_phased_reject_commit artifact_effect_emitter
  bc_phased_reject_commit artifact_frame_call
  bc_phased_reject_commit artifact_local_access
  bc_phased_reject_commit artifact_raw_memory
  bc_phased_reject_commit artifact_primitive_composition
  bc_phased_reject_commit artifact_comparison
  bc_phased_reject_commit artifact_stack_push
  bc_phased_reject_commit artifact_structural_survival
}
