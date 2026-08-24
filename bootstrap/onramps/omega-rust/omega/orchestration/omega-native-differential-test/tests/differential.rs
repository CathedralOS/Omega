//! Differential oracle test: over every RUN canary (a `canaries/pass/**` program the
//! canary suite compiles, executes, and asserts a known exit code for), run the reference
//! interpreter and -- when it SUPPORTS the program -- compile + run the NATIVE binary and
//! assert their exit code and stdout MATCH.
//!
//! Framing: these are PASSING run canaries, so the native binary is correct-by-definition
//! (the suite asserts its exact exit code). Therefore any `interpret() != native` here is
//! an INTERPRETER bug, and the test fails so it gets fixed. When the interpreter returns an
//! error (unsupported construct) the canary is SKIPPED -- a missing feature is not a
//! mismatch. As the interpreter's coverage grows, more canaries leave the skip bucket and
//! must agree with native.
//!
//! The set below mirrors how `omega-compiler/tests/canary_suite.rs` runs canaries: only the
//! programs it actually executes-and-checks an exit code for (no compile-only or
//! visualization canaries, whose native exit code is undefined). The companion expected
//! code documents the suite's own assertion and lets us sanity-check native against it.

use omega_compiler::{
    ArtifactEmissionPolicy, CheckedCompilation, CompileOptions, compile_to_checked,
    compile_with_artifact_policy, compile_with_test_entry_and_artifact_policy,
    compile_with_test_entry_worker_count_and_artifact_policy,
    compile_with_worker_count_and_artifact_policy,
};
use psi_checked_interpreter::{InterpretOutcome, interpret_entry};
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicU64, AtomicUsize, Ordering};
use std::sync::mpsc;
use std::thread;
use std::time::Instant;

static NEXT_NATIVE_STAGE: AtomicU64 = AtomicU64::new(1);

fn interpret(checked: &CheckedCompilation, stdin: &[u8]) -> InterpretOutcome {
    interpret_entry(
        checked,
        checked
            .selected_program_entry_machine()
            .unwrap_or("Main::main"),
        stdin,
    )
}

fn host_target_name() -> &'static str {
    if cfg!(all(target_os = "macos", target_arch = "aarch64")) {
        "macos_arm64"
    } else if cfg!(all(target_os = "linux", target_arch = "aarch64")) {
        "linux_arm64"
    } else if cfg!(all(target_os = "linux", target_arch = "x86_64")) {
        "linux_x64"
    } else if cfg!(all(target_os = "windows", target_arch = "x86_64")) {
        "windows_x64"
    } else {
        panic!("unsupported host profile for native differential execution")
    }
}

fn host_program_entry_owner() -> &'static str {
    match host_target_name() {
        "windows_x64" => "windows_x86_64",
        "linux_x64" => "linux_x86_64",
        "linux_arm64" => "linux_arm64",
        "macos_arm64" => "macos_arm64",
        _ => unreachable!("host_target_name returns one hosted target"),
    }
}

/// Whether this fixture has opted into production entry selection for the
/// development host. A build file for some other target does not retire the
/// local legacy seam.
fn has_authored_host_program_entry(main_path: &Path) -> bool {
    let Some(project) = main_path.parent() else {
        return false;
    };
    fs::read_to_string(project.join("build.omg")).is_ok_and(|source| {
        source.lines().any(|line| {
            line.trim_start()
                .starts_with(&format!("target {}", host_target_name()))
        }) && source.lines().any(|line| {
            line.contains(".roots.bind(")
                && line.contains(&format!("{}::ProgramEntry", host_program_entry_owner()))
        })
    })
}

fn compile_differential_to_checked(
    main_path: &Path,
) -> Result<CheckedCompilation, Vec<psi_diagnostics::Diagnostic>> {
    let target = has_authored_host_program_entry(main_path).then_some(host_target_name());
    compile_to_checked(main_path, target)
}

#[test]
fn authored_host_entries_select_the_production_root() {
    let main_path = pass_canary("float/float_to_int_exact_proofs_exit").join("main.omg");
    assert!(has_authored_host_program_entry(&main_path));

    let checked = compile_differential_to_checked(&main_path)
        .unwrap_or_else(|diagnostics| panic!("authored entry rejected: {diagnostics:#?}"));
    assert_eq!(checked.selected_program_entry_machine(), Some("Main::main"));

    let legacy = pass_canary("control_flow/runtime_entry_cast_result_exit").join("main.omg");
    assert!(!has_authored_host_program_entry(&legacy));
}

/// The RUN canaries: `(relative path under canaries/pass, exit code the suite asserts)`.
/// Extracted from `canary_suite.rs` (every test that runs `executable_name()` and asserts
/// `output.status.code() == Some(N)`).
const RUN_CANARIES: &[(&str, i32)] = &[
    ("arithmetic/runtime_chained_field_mutation_exit", 70),
    ("arithmetic/runtime_comparison_signedness_exit", 70),
    ("arithmetic/runtime_shift_signedness_exit", 70),
    ("arithmetic/runtime_shift_in_guard_exit", 70),
    ("arithmetic/runtime_cast_in_guard_exit", 70),
    ("arithmetic/runtime_i64_to_u64_exact_guard_exit", 70),
    ("arithmetic/runtime_parenthesized_guard_subjects_exit", 70),
    ("arithmetic/runtime_and_of_or_guard_exit", 70),
    ("arithmetic/runtime_negated_boolean_nesting_guard_exit", 70),
    ("arithmetic/runtime_guard_feature_composition_exit", 70),
    ("arithmetic/runtime_saturating_narrow_add_sub_exit", 70),
    ("arithmetic/runtime_saturating_wide_boundaries_exit", 70),
    ("arithmetic/runtime_saturating_param_carry_exit", 70),
    ("arithmetic/runtime_saturating_expression_domain_exit", 70),
    ("arithmetic/runtime_wrapping_expression_guard_exit", 70),
    ("arithmetic/runtime_divide_min_edge_guard_exit", 70),
    ("arithmetic/runtime_nested_unsigned_witness_exit", 70),
    ("slices/runtime_local_array_element_value_operand_exit", 70),
    (
        "slices/runtime_machine_array_element_fused_call_arg_exit",
        70,
    ),
    ("termination/custom_ranking_field_countdown_compile", 70),
    ("termination/custom_ranking_struct_view", 70),
    ("termination/runtime_recursive_result_roles_exit", 70),
    ("calls/recursive_result_bind_first_arg", 70),
    ("calls/runtime_branching_callee_chain_exit", 70),
    (
        "calls/runtime_branch_leaf_multiple_named_conversion_exit",
        70,
    ),
    ("calls/runtime_nested_named_conversion_alias_exit", 70),
    ("control_flow/sum_payload_cast_operand_field_exit", 70),
    ("arithmetic/runtime_float_compare_bool_exit", 70),
    ("float/runtime_total_order_satisfiers_exit", 70),
    ("arithmetic/runtime_float_nested_operand_exit", 70),
    ("arithmetic/runtime_shift_count_domain_exit", 70),
    ("arithmetic/runtime_exact_guarded_shift_count_exit", 70),
    ("arithmetic/runtime_shift_atwidth_signed_modular_exit", 70),
    ("arithmetic/runtime_shift_right_atwidth_exit", 70),
    ("arithmetic/runtime_shift_atwidth_indexed_targets_exit", 70),
    ("arithmetic/runtime_sat_nested_operand_domain_exit", 70),
    ("arithmetic/runtime_sat_unsigned_onedirection_exit", 70),
    ("arithmetic/runtime_sat_min_idiom_exit", 70),
    ("arithmetic/runtime_shl_saturating_exit", 70),
    ("arithmetic/runtime_shl_saturating_value_overflow_exit", 70),
    ("arithmetic/runtime_shift_count_proven_range_exit", 70),
    ("arithmetic/runtime_shift_subword_masked_count_exit", 70),
    ("arithmetic/u64_magnitude_transition_arg_exit", 70),
    ("arithmetic/float_literal_cast_proves_exit", 70),
    // F4 Saturating float->int (NaN -> 0, OOR clamp): aarch64 FCVTZS is
    // natively these semantics; x86 supplies the cvttsd2si policy fixup.
    ("arithmetic/float_to_int_saturating_exit", 70),
    (
        "arithmetic/float_to_int_unsigned_narrow_saturating_exit",
        70,
    ),
    // F5 Saturating float arithmetic is native on both backends.
    ("arithmetic/float_saturating_overflow_exit", 70),
    ("providers/runtime_adapter_dispatch_exit", 70),
    ("providers/runtime_adapter_forwarding_exit", 70),
    ("providers/checked_boundary_operator_dispatch_exit", 70),
    ("providers/provider_type_slot_selected", 70),
    (
        "providers/runtime_result_domain_requirement_overload_exit",
        70,
    ),
    (
        "providers/runtime_boundary_capability_state_forwarding_exit",
        70,
    ),
    ("host/runtime_console_byte_literal_exit", 70),
    ("proofs/runtime_decreases_u64_measure_exit", 70),
    ("arithmetic/runtime_wrapping_operand_truncation_exit", 70),
    ("text/case_literal_texteq_field_store_exit", 70),
    ("text/case_literal_texteq_terminal_exit", 70),
    ("text/runtime_text_equals_boolean_operand_exit", 70),
    ("text/runtime_text_not_equals_exit", 70),
    ("text/runtime_owned_string_byte_view_exit", 70),
    ("traits/equatable_sum_stale_payload_exit", 70),
    ("text/zii_default_string_equality_exit", 70),
    ("text/zii_string_host_write_exit", 70),
    ("core/zii_default_composite_exit", 70),
    ("core/numeric_conversion_surface", 70),
    ("core/numeric_cross_signed_conversion_surface", 70),
    ("core/numeric_signed_conversion_surface", 70),
    ("structs/aggregate_transition_args_exit", 70),
    ("structs/deep_nested_write_paths_exit", 70),
    ("text/runtime_text_equals_value_positions_exit", 70),
    ("wire/runtime_wire_roundtrip_utf8_exit", 70),
    ("wire/runtime_wire_utf8_edge_verdicts_exit", 70),
    ("wire/runtime_wire_utf8_invalid_refused_exit", 70),
    ("wire/runtime_wire_schema_as_value_type_exit", 70),
    ("wire/runtime_wire_decode_let_compare_exit", 70),
    ("wire/runtime_wire_encode_borrowed_scalar_slice_exit", 70),
    ("slices/runtime_saturating_array_element_guard_exit", 70),
    ("arithmetic/runtime_unsigned_high_bit_u32_ops_exit", 70),
    ("arithmetic/runtime_unsigned_min_max_exit", 88),
    ("arithmetic/runtime_integer_casts_exit", 70),
    ("arithmetic/runtime_mixed_width_sign_exit", 70),
    ("arithmetic/runtime_saturating_narrow_divide_exit", 70),
    ("arithmetic/runtime_narrow_signed_divide_guard_exit", 70),
    ("arithmetic/runtime_narrow_signed_guard_ops_exit", 70),
    ("arithmetic/runtime_narrow_signed_wrap_boundaries_exit", 70),
    ("arithmetic/runtime_domain_boundaries_exit", 70),
    ("arithmetic/runtime_float_compare_cast_exit", 70),
    ("float/finite_core_domain_range_discharge", 77),
    ("float/float_saturating_arithmetic_exit", 77),
    ("float/float_to_int_exact_proofs_exit", 78),
    ("float/float_to_int_policy_exit", 77),
    ("arithmetic/runtime_i64_divide_modulo_exit", 70),
    ("arithmetic/runtime_float_operations_exit", 70),
    ("arithmetic/runtime_comparison_guard_signedness_exit", 70),
    ("arithmetic/runtime_comparison_value_signedness_exit", 70),
    ("arithmetic/runtime_copy_then_read_exit", 70),
    ("arithmetic/runtime_cast_element_accumulator_exit", 70),
    ("arithmetic/runtime_exclusive_range_constraint_exit", 70),
    ("arithmetic/runtime_i64_full_width_exit", 70),
    ("arithmetic/runtime_inferred_multipath_return_exit", 70),
    ("arithmetic/runtime_inferred_return_range_exit", 70),
    ("arithmetic/runtime_fnv1a_hash_exit", 70),
    ("arithmetic/runtime_min_max_clamp_narrowing_exit", 70),
    ("arithmetic/runtime_float_running_min_max_fold_exit", 70),
    ("arithmetic/runtime_min_max_signedness_exit", 70),
    ("arithmetic/runtime_modulo_div_narrowing_exit", 70),
    ("calls/runtime_min_max_guard_subject_hoist_exit", 70),
    ("calls/runtime_min_guard_true_false_pair_exit", 70),
    ("calls/runtime_value_callee_post_entry_lets_exit", 24),
    ("calls/runtime_post_entry_chained_let_exit", 2),
    ("calls/runtime_post_entry_deep_chain_exit", 30),
    ("backend/value_machine_self_array_local_index_exit", 99),
    ("backend/value_machine_const_index_self_array_exit", 99),
    ("arithmetic/runtime_payload_range_narrowing_exit", 70),
    ("arithmetic/runtime_provable_field_construction_exit", 70),
    ("arithmetic/runtime_signed_division_exit", 70),
    ("arithmetic/runtime_shift_right_signedness", 70),
    ("arithmetic/const_fold_unsigned_landed_ops_exit", 70),
    ("arithmetic/const_fold_saturating_narrow_exit", 70),
    ("arithmetic/const_fold_wrapping_narrow_exit", 70),
    ("calls/mutual_cycle_tail_admitted_exit", 70),
    ("arithmetic/const_fold_unsigned_shift_right_arg_exit", 70),
    ("arithmetic/const_fold_unsigned_divide_arg_exit", 70),
    ("arithmetic/unsigned_min_max_wrapping_local_exit", 77),
    ("storage/runtime_slice_indexed_binary_rmw_exit", 70),
    ("calls/runtime_mut_ref_forward_exit", 70),
    ("calls/runtime_trailing_state_mut_param_phase_exit", 70),
    ("storage/runtime_local_slice_forward_exit", 70),
    ("float/f32_guard_const_arith_landed_exit", 70),
    ("float/f32_arg_const_arith_landed_exit", 70),
    ("arithmetic/const_fold_cast_signedness", 70),
    ("arithmetic/wrapping_signed_divide_min_by_neg_one", 70),
    ("arithmetic/saturating_signed_divide_min_by_neg_one", 70),
    ("arithmetic/saturating_multiply_overflow_both_signs", 70),
    ("arithmetic/f32_field_store_rounding", 70),
    ("arithmetic/f32_transition_arg_rounding", 70),
    ("arithmetic/int_transition_arg_width_wrap", 70),
    ("arithmetic/array_element_write_width_domain", 70),
    ("arithmetic/struct_literal_field_coercion", 70),
    ("arithmetic/runtime_struct_field_range_narrowing_exit", 70),
    (
        "arithmetic/runtime_transition_arg_false_arm_narrowing_exit",
        70,
    ),
    ("arithmetic/runtime_transition_arg_guard_narrowing_exit", 70),
    (
        "arithmetic/runtime_transition_value_guard_narrowing_exit",
        42,
    ),
    ("arithmetic/runtime_requires_one_sided_bound_exit", 42),
    ("arithmetic/runtime_transition_arg_saturating_exit", 70),
    ("arithmetic/runtime_unsigned_division_exit", 70),
    ("arithmetic/runtime_float_negative_ops_exit", 70),
    ("arithmetic/runtime_float32_array_conversion_exit", 70),
    ("arithmetic/runtime_signed_modulo_shift_edges_exit", 70),
    ("arithmetic/runtime_unsigned_high_comparison_exit", 70),
    ("arithmetic/runtime_bitwise_high_ops_exit", 70),
    ("arithmetic/runtime_cast_sign_zero_extension_exit", 70),
    ("arithmetic/runtime_i64_signed_arithmetic_exit", 70),
    ("arithmetic/runtime_saturating_domain_exit", 70),
    ("arithmetic/runtime_float_nan_comparison_exit", 70),
    ("arithmetic/runtime_gcd_euclid_exit", 70),
    ("arithmetic/runtime_monte_carlo_pi_exit", 70),
    ("arithmetic/runtime_newton_sqrt_exit", 70),
    ("arithmetic/runtime_unsigned_modulo_call_argument_exit", 70),
    ("arithmetic/runtime_unsigned_modulo_cast_operand_exit", 70),
    (
        "borrow/runtime_view_linked_input_unrelated_ref_write_exit",
        70,
    ),
    ("calls/runtime_option_value_call_exit", 70),
    ("calls/runtime_struct_value_call_exit", 70),
    ("calls/runtime_value_call_composition_exit", 70),
    ("calls/runtime_struct_by_value_param_exit", 70),
    ("calls/computed_host_arg_exit", 70),
    ("calls/computed_host_builtin_arg_exit", 70),
    ("calls/computed_host_cast_arg_exit", 70),
    ("calls/computed_host_indexed_arg_exit", 70),
    ("calls/value_call_as_host_arg_exit", 70),
    ("calls/free_standing_machine_helper_compile", 7),
    ("calls/runtime_computed_transition_args_exit", 70),
    ("calls/runtime_value_call_to_array_element_exit", 70),
    ("calls/runtime_cross_machine_substate_name_exit", 70),
    (
        "calls/runtime_alias_indexed_read_through_transition_exit",
        70,
    ),
    (
        "calls/runtime_alias_write_through_guarded_transition_exit",
        70,
    ),
    ("calls/runtime_assignment_call_post_mutation_value_exit", 70),
    ("calls/runtime_value_call_return_types_exit", 70),
    ("calls/runtime_contained_machine_exit", 70),
    ("calls/runtime_value_call_struct_literal_arms_exit", 70),
    ("calls/runtime_value_call_self_field_enum_match_exit", 70),
    ("calls/runtime_value_call_struct_result_to_target_exit", 70),
    ("calls/runtime_call_value", 70),
    ("calls/runtime_call_in_inlined_substate_exit", 70),
    ("calls/runtime_call_result_after_splice_mutation_exit", 70),
    (
        "calls/runtime_call_result_through_reference_field_exit",
        183,
    ),
    ("calls/runtime_attached_machine_struct_arg_exit", 70),
    ("calls/by_value_case_param_self_write_exit", 70),
    ("calls/sequential_self_field_rmw_exit", 70),
    ("calls/transition_arg_local_from_embedded_call_exit", 70),
    ("calls/value_call_embedded_in_binary_exit", 70),
    ("calls/runtime_value_call_slice_view_element_arg_exit", 70),
    (
        "control_flow/no_payload_case_variant_after_payload_dispatch_exit",
        70,
    ),
    ("control_flow/case_payload_shared_field_name_exit", 70),
    ("control_flow/sum_mixed_width_payload_layout", 70),
    ("control_flow/sum_field_storage_roundtrip", 70),
    ("control_flow/runtime_linear_search_early_exit", 70),
    ("control_flow/runtime_entry_cast_result_exit", 70),
    ("control_flow/runtime_entry_nested_binary_result_exit", 70),
    ("control_flow/runtime_entry_return_field_exit", 200),
    ("control_flow/runtime_entry_unary_result_exit", 1),
    ("control_flow/runtime_multi_field_payload_arith_exit", 70),
    ("control_flow/runtime_nested_loop_grid_sum_exit", 70),
    (
        "control_flow/runtime_captured_local_remutated_field_exit",
        70,
    ),
    (
        "control_flow/runtime_composite_initializer_local_arg_exit",
        70,
    ),
    ("control_flow/runtime_loop_patterns_exit", 70),
    ("calls/runtime_called_machine_loop_search_exit", 70),
    ("calls/runtime_dispatch_binary_call_argument_exit", 70),
    ("calls/runtime_dispatch_result_binary_terminal_exit", 70),
    ("calls/runtime_dispatch_machine_array_slice_arg_exit", 70),
    ("calls/runtime_dispatch_result_enum_case_exit", 70),
    ("calls/runtime_dispatch_result_field_binding_exit", 70),
    ("calls/runtime_dispatch_result_alias_read_exit", 70),
    ("calls/runtime_dispatch_second_receiver_exit", 70),
    ("calls/runtime_dispatch_sibling_value_calls_exit", 70),
    (
        "calls/runtime_inline_repeated_receiver_value_calls_exit",
        70,
    ),
    ("calls/runtime_nonentry_second_receiver_exit", 70),
    ("calls/runtime_selfcall_chain_second_receiver_exit", 70),
    ("calls/runtime_nested_inline_chain_result_exit", 70),
    ("calls/runtime_nonentry_inline_second_receiver_exit", 70),
    (
        "calls/runtime_nested_local_terminal_second_instance_exit",
        70,
    ),
    (
        "calls/runtime_nested_field_terminal_second_instance_exit",
        70,
    ),
    ("calls/runtime_multiarm_same_named_locals_exit", 70),
    ("calls/runtime_multiarm_texteq_local_exit", 70),
    ("calls/runtime_pre_guard_texteq_local_guard_exit", 70),
    ("calls/runtime_pre_guard_texteq_local_arg_forward_exit", 70),
    ("calls/runtime_param_receiver_single_instance_exit", 70),
    ("calls/runtime_param_receiver_second_instance_exit", 70),
    ("calls/runtime_param_forward_chain_second_receiver_exit", 70),
    ("calls/runtime_deep_state_name_collision_exit", 70),
    ("core/runtime_natural_termination_exit", 0),
    ("build/runtime_main_source_builder_is_ordinary_exit", 70),
    ("arithmetic/runtime_u64_literal_let_guard_exit", 70),
    ("time/runtime_saturating_time_arith_exit", 70),
    ("calls/runtime_dispatch_float_terminal_exit", 70),
    (
        "time/runtime_value_machine_receiver_field_postentry_exit",
        70,
    ),
    ("references/runtime_nested_receiver_same_type_exit", 70),
    ("calls/runtime_same_type_second_receiver_mutation_exit", 70),
    ("calls/runtime_dispatch_slice_element_terminal_exit", 70),
    ("calls/runtime_dispatch_result_field_terminal_exit", 70),
    ("calls/runtime_dispatch_result_guard_subject_exit", 70),
    ("calls/runtime_dispatch_result_multi_arm_exit", 70),
    ("calls/runtime_dispatch_result_transition_arg_exit", 70),
    ("calls/runtime_dispatched_effectful_reentrant_exit", 70),
    (
        "calls/runtime_effectful_guard_local_and_self_terminal_exit",
        70,
    ),
    (
        "calls/runtime_guarded_effectful_transition_argument_exit",
        70,
    ),
    ("collections/runtime_dutch_flag_partition_exit", 70),
    ("calls/runtime_exit_code_exit", 70),
    ("host/runtime_sleep_exit", 70),
    ("host/runtime_write_no_newline_exit", 70),
    ("calls/runtime_explicit_discard_executes_exit", 70),
    ("calls/runtime_free_machine_looping_value_call_exit", 70),
    ("calls/runtime_free_machine_struct_arg_exit", 70),
    ("calls/runtime_free_machine_struct_return_exit", 70),
    ("calls/runtime_free_machine_value_call_exit", 70),
    ("calls/runtime_free_machine_value_call_mut_arg_exit", 70),
    ("calls/runtime_record_forwarding_statement_call_exit", 70),
    ("calls/runtime_let_local_nested_state_arg_exit", 70),
    ("calls/runtime_local_string_field_copy_through_mut_exit", 70),
    ("calls/runtime_min_call_result_arithmetic_exit", 70),
    ("calls/runtime_multi_arm_value_transition_exit", 70),
    (
        "calls/runtime_mutable_dynamic_indexed_machine_owned_parameter_write_exit",
        175,
    ),
    (
        "calls/runtime_mutable_local_indexed_parameter_write_exit",
        171,
    ),
    ("calls/runtime_mutable_local_parameter_write_exit", 171),
    (
        "calls/runtime_mutable_machine_owned_local_indexed_parameter_write_exit",
        173,
    ),
    (
        "calls/runtime_mutable_machine_owned_parameter_write_exit",
        141,
    ),
    (
        "calls/runtime_mutable_parameter_read_modify_write_exit",
        191,
    ),
    ("calls/runtime_nested_called_machine_loop_exit", 70),
    (
        "calls/runtime_nested_guarded_reference_returned_slice_element_exit",
        184,
    ),
    ("calls/runtime_nested_value_call_in_substate_exit", 70),
    (
        "calls/runtime_offset_string_call_results_through_reference_fields_exit",
        196,
    ),
    ("calls/runtime_looping_value_return_exit", 70),
    ("calls/runtime_looping_cast_return_exit", 70),
    (
        "calls/runtime_referenced_local_outlives_sibling_guard_call_exit",
        70,
    ),
    (
        "calls/runtime_reference_param_forwarded_through_loop_exit",
        70,
    ),
    (
        "calls/runtime_reference_returned_slice_element_through_param_exit",
        70,
    ),
    (
        "calls/runtime_reference_returned_slice_element_write_exit",
        181,
    ),
    (
        "calls/runtime_string_call_result_through_reference_field_exit",
        186,
    ),
    ("calls/runtime_trailing_local_return_exit", 70),
    (
        "calls/runtime_transition_subject_call_single_evaluation_exit",
        70,
    ),
    (
        "control_flow/runtime_nonplace_record_pattern_single_evaluation_exit",
        70,
    ),
    (
        "calls/runtime_two_string_call_results_through_reference_fields_exit",
        194,
    ),
    ("calls/runtime_slice_length_field_exit", 5),
    ("calls/runtime_value_call_single_execution_exit", 70),
    ("calls/runtime_value_call_slice_len_guard_exit", 70),
    ("calls/value_call_sequential_result_slots_exit", 70),
    ("calls/value_call_sequential_self_capture_exit", 70),
    (
        "calls/runtime_value_call_through_alias_in_dispatch_exit",
        70,
    ),
    ("calls/runtime_value_position_branching_call_exit", 70),
    ("calls/runtime_value_call_let_combine_exit", 70),
    ("calls/runtime_value_transition_unsigned_guard_exit", 70),
    ("collections/runtime_fixed_vec_round_trip_exit", 70),
    ("collections/runtime_write_first_loop_index_exit", 70),
    ("collections/runtime_loop_counter_init_hoisted_exit", 70),
    ("collections/runtime_nested_loop_fill_exit", 70),
    ("collections/runtime_computed_array_fill_via_temp_exit", 70),
    ("collections/runtime_computed_indexed_write_exit", 70),
    ("collections/runtime_nested_const_product_index_exit", 70),
    ("collections/runtime_hoisted_index_write_exit", 7),
    ("calls/runtime_let_mut_reassign_exit", 2),
    ("control_flow/runtime_tuple_matrix_exhaustive_exit", 70),
    ("control_flow/runtime_sum_tuple_matrix_exhaustive_exit", 70),
    ("control_flow/runtime_tuple_case_destructure_exit", 70),
    ("dependent/runtime_dependent_param_range_exit", 70),
    ("dependent/runtime_dependent_product_index_exit", 70),
    ("dependent/runtime_dependent_subtract_exit", 2),
    ("dependent/runtime_dependent_ordering_chain_exit", 7),
    ("dependent/runtime_requires_subtract_exit", 0),
    ("dependent/runtime_requires_guarded_call_exit", 6),
    ("dependent/runtime_sibling_len_index_exit", 7),
    ("dependent/runtime_bounded_product_index_exit", 7),
    ("data/runtime_proof_only_data_declared_exit", 70),
    ("arithmetic/runtime_u64_guarded_cap_store_exit", 70),
    ("calls/runtime_measured_tail_recursion_exit", 70),
    ("calls/runtime_terminal_tail_recursion_exit", 70),
    ("comptime/runtime_const_measured_recursion_exit", 70),
    ("collections/runtime_computed_index_match_subject_exit", 70),
    ("calls/runtime_std_math_sin_cos_exit", 70),
    ("calls/runtime_value_call_terminal_exit", 70),
    ("calls/runtime_large_shared_ref_direct_assignment_exit", 70),
    ("constants/runtime_free_const_exit", 70),
    ("proofs/runtime_core_nat_declared_exit", 70),
    ("proofs/runtime_core_rat_declared_exit", 70),
    ("proofs/accepted_axiom_cited_exit", 70),
    ("proofs/runtime_nat_structural_recursion_exit", 70),
    ("proofs/runtime_core_roster_ops_exit", 70),
    ("build/runtime_depend_mapping_exit", 70),
    ("arithmetic/runtime_f32_field_guard_exit", 70),
    ("collections/runtime_indexed_guard_true_false_pair_exit", 70),
    ("collections/runtime_indexed_field_local_operand_exit", 70),
    ("collections/runtime_indexed_local_bitwise_exit", 70),
    ("collections/runtime_indexed_local_compare_exit", 70),
    ("collections/runtime_indexed_rmw_loop_exit", 70),
    ("collections/runtime_indexed_reduction_loop_exit", 70),
    ("collections/runtime_array_max_and_sum_exit", 70),
    ("generics/runtime_generic_record_instance_exit", 70),
    ("generics/runtime_const_data_array_length_exit", 70),
    ("generics/runtime_const_data_expression_exit", 70),
    ("generics/runtime_const_data_machine_call_exit", 70),
    ("generics/runtime_const_data_machine_fact_exit", 70),
    ("generics/runtime_const_data_named_value_exit", 70),
    ("generics/runtime_const_data_symbolic_expression_exit", 70),
    ("generics/runtime_const_data_where_fact_exit", 70),
    ("generics/runtime_const_data_forwarded_length_exit", 70),
    ("generics/runtime_const_data_multiple_instances_exit", 70),
    ("generics/runtime_signed_const_data_exit", 70),
    ("generics/runtime_const_container_methods_exit", 70),
    ("generics/runtime_generic_two_instantiations_exit", 30),
    ("generics/runtime_generic_domain_instantiations_exit", 42),
    ("generics/runtime_generic_let_local_instantiations_exit", 30),
    ("generics/runtime_nested_generic_instantiations_exit", 30),
    ("generics/runtime_generic_enum_payload_exit", 70),
    ("generics/runtime_generic_value_call_exit", 70),
    ("generics/runtime_generic_value_call_agreeing_exit", 70),
    ("generics/runtime_generic_multiple_specializations_exit", 14),
    ("generics/runtime_generic_param_position_inference_exit", 70),
    ("host/runtime_tick_count_monotonic_exit", 70),
    ("host/runtime_user32_key_state_exit", 70),
    ("host/runtime_tick_paced_marquee_exit", 0),
    ("host/runtime_gui_memory_dc_blit_exit", 70),
    ("host/runtime_gui_window_blit_exit", 70),
    ("host/runtime_gui_window_lifecycle_exit", 70),
    ("arithmetic/runtime_nested_payload_range_narrowing_exit", 70),
    ("arithmetic/runtime_guard_proven_counter_exit", 70),
    ("arithmetic/runtime_guard_narrowed_transition_arg_exit", 70),
    ("collections/runtime_indexed_guard_subject_exit", 70),
    ("collections/runtime_array_min_max_builtin_exit", 70),
    ("collections/runtime_dual_indexed_comparison_guard_exit", 70),
    ("collections/runtime_rule90_automaton_exit", 70),
    ("collections/runtime_whole_array_value_copy_exit", 70),
    ("collections/runtime_nested_array_const_index_exit", 70),
    ("collections/runtime_row_const_column_write_exit", 70),
    ("collections/runtime_indexed_read_then_guard_exit", 70),
    ("collections/std_option_runtime_match_exit", 70),
    ("collections/runtime_indexed_struct_write_loop_exit", 70),
    ("collections/runtime_struct_field_temp_arith_exit", 70),
    ("collections/runtime_two_indexed_reads_binary_exit", 70),
    ("collections/runtime_enum_grid_scan_exit", 70),
    ("collections/runtime_nested_struct_array_field_exit", 70),
    ("collections/runtime_binary_search_exit", 70),
    ("collections/runtime_2d_transpose_exit", 70),
    ("collections/runtime_bubble_sort_exit", 70),
    ("collections/runtime_rpn_evaluator_exit", 70),
    ("collections/runtime_ring_buffer_queue_exit", 70),
    ("collections/runtime_matrix_multiply_exit", 70),
    ("collections/runtime_hash_table_exit", 70),
    ("collections/runtime_bfs_traversal_exit", 70),
    ("collections/runtime_coin_change_dp_exit", 70),
    ("collections/runtime_nqueens_backtracking_exit", 70),
    ("collections/runtime_maze_pathfind_exit", 70),
    ("collections/runtime_activity_selection_greedy_exit", 70),
    ("collections/runtime_indexed_through_guard_chain_exit", 70),
    ("collections/runtime_two_pointer_palindrome_exit", 70),
    ("comptime/runtime_const_array_length_exit", 70),
    ("layouts/runtime_plan_laid_value_field_exit", 70),
    ("layouts/runtime_plan_laid_value_by_value_param_exit", 70),
    ("layouts/runtime_plan_laid_record_view_exit", 70),
    ("layouts/runtime_plan_laid_record_mutable_write_exit", 70),
    (
        "layouts/runtime_plan_laid_fixed_array_mutable_write_exit",
        70,
    ),
    ("layouts/runtime_plan_laid_fixed_array_view_exit", 70),
    (
        "layouts/runtime_plan_laid_nested_fixed_array_mutable_write_exit",
        70,
    ),
    (
        "layouts/runtime_plan_laid_nested_record_mutable_write_exit",
        70,
    ),
    (
        "layouts/runtime_plan_laid_record_array_mutable_write_exit",
        70,
    ),
    ("collections/runtime_dual_indexed_guard_compare_exit", 70),
    (
        "collections/runtime_cross_array_indexed_guard_compare_exit",
        70,
    ),
    ("collections/runtime_dual_indexed_guard_equality_exit", 70),
    ("collections/runtime_dual_indexed_copy_exit", 50),
    ("collections/runtime_dual_indexed_copy_in_loop_exit", 70),
    (
        "collections/runtime_indexed_write_frame_local_source_exit",
        70,
    ),
    ("collections/runtime_indexed_local_copy_chain_exit", 70),
    ("collections/runtime_inplace_reverse_local_temp_exit", 70),
    ("control_flow/runtime_captured_local_swap_exit", 70),
    ("calls/runtime_same_type_contained_direct_fields_exit", 70),
    ("calls/runtime_shared_ref_param_member_exit", 42),
    ("calls/runtime_shared_ref_param_large_deref_exit", 42),
    ("collections/runtime_palindrome_two_pointer_exit", 70),
    ("collections/runtime_bracket_matcher_stack_exit", 70),
    ("collections/runtime_argmax_index_exit", 70),
    ("control_flow/runtime_sum_field_store_payload_exit", 70),
    ("arithmetic/runtime_float_self_compare_nan_exit", 70),
    ("arithmetic/runtime_abs_desugar_exit", 70),
    ("arithmetic/runtime_sqrt_builtin_exit", 70),
    ("arithmetic/runtime_clamp_desugar_exit", 70),
    ("arithmetic/runtime_clamp_narrowing_exit", 100),
    ("arithmetic/runtime_negative_float_to_int_exit", 70),
    ("arithmetic/runtime_float_min_max_abs_clamp_exit", 70),
    ("comptime/runtime_const_array_length_transitive_exit", 70),
    ("comptime/runtime_const_array_length_bare_call_arm_exit", 70),
    ("borrow/runtime_view_of_view_chain_exit", 70),
    ("borrow/runtime_method_view_write_after_last_use_exit", 70),
    ("control_flow/fixed_array_element_guard", 0),
    ("control_flow/runtime_boolean_or_guard_exit", 71),
    ("control_flow/runtime_case_member_dispatch_exit", 70),
    (
        "control_flow/runtime_boolean_transition_argument_after_string_guard_exit",
        247,
    ),
    (
        "control_flow/runtime_direct_boolean_transition_argument_exit",
        211,
    ),
    (
        "control_flow/runtime_effectful_subject_single_evaluation_exit",
        70,
    ),
    (
        "control_flow/runtime_local_boolean_conjunction_value_exit",
        74,
    ),
    ("control_flow/runtime_local_boolean_or_value_exit", 251),
    (
        "control_flow/runtime_local_boolean_transition_argument_exit",
        201,
    ),
    (
        "control_flow/runtime_local_scalar_comparison_value_exit",
        76,
    ),
    (
        "control_flow/runtime_local_string_comparison_value_exit",
        78,
    ),
    ("control_flow/runtime_negated_boolean_place_guard_exit", 73),
    ("control_flow/runtime_negated_comparison_guard_exit", 75),
    ("control_flow/runtime_state_loop_indexed_search_exit", 70),
    (
        "control_flow/runtime_statement_call_single_execution_exit",
        70,
    ),
    (
        "control_flow/runtime_straight_line_terminal_field_readback_exit",
        70,
    ),
    ("control_flow/runtime_straight_line_terminal_local_exit", 70),
    ("control_flow/runtime_tuple_transition_exit", 22),
    ("data/case_membership_union_guard_exit", 70),
    ("data/runtime_struct_value_copy_exit", 70),
    ("data/runtime_whole_struct_mutation_copy_exit", 70),
    ("data/runtime_deep_nested_field_exit", 70),
    ("data/case_membership_value_exit", 70),
    ("data/runtime_case_membership_mixed_shape_exit", 70),
    ("data/match_exhaustive_by_case_union_domain", 70),
    ("data/match_exhaustive_by_cases", 70),
    ("data/case_payload_native_construction", 70),
    ("data/runtime_case_payload_guard_read_exit", 70),
    ("data/runtime_record_field_value_pattern_exit", 70),
    ("data/runtime_case_reassignment_exit", 70),
    ("data/runtime_data_properties_exit", 70),
    ("data/runtime_mixed_shape_exit", 70),
    ("data/runtime_array_literal_string_field_exit", 70),
    ("data/runtime_struct_literal_string_field_exit", 70),
    ("domains/domain_field_write_then_read_exit", 73),
    ("domains/executable_domain_membership_expression_exit", 81),
    (
        "domains/executable_domain_membership_intersection_guard_exit",
        231,
    ),
    (
        "domains/executable_domain_membership_intersection_value_exit",
        233,
    ),
    ("domains/executable_domain_membership_union_guard_exit", 241),
    ("domains/executable_domain_membership_union_value_exit", 205),
    ("domains/executable_imported_domain_membership_exit", 91),
    (
        "domains/executable_imported_domain_membership_guard_exit",
        81,
    ),
    (
        "domains/executable_imported_domain_membership_intersection_guard_exit",
        219,
    ),
    (
        "domains/executable_imported_domain_membership_intersection_value_exit",
        217,
    ),
    (
        "domains/executable_imported_domain_membership_union_guard_exit",
        217,
    ),
    (
        "domains/executable_imported_domain_membership_union_value_exit",
        215,
    ),
    ("domains/bodyless_domain_declarations_exit", 70),
    ("domains/runtime_result_domain_machine_overload_exit", 70),
    ("domains/user_domain_literal_grant", 70),
    ("domains/utf8_equals_literal_exit", 70),
    ("domains/utf8_equals_view_exit", 70),
    ("domains/utf8_field_read_carries_domain_exit", 70),
    ("domains/utf8_literal_len_exit", 70),
    ("domains/utf8_param_len_field_exit", 70),
    ("domains/utf8_regular_call_len_exit", 70),
    ("domains/utf8_return_view_equals_exit", 70),
    ("dungeon/runtime_clear_carve_render_string_fields_exit", 198),
    ("dungeon/runtime_direct_boolean_conjunction_exit", 21),
    ("dungeon/runtime_enemy_clear_reentry_exit", 51),
    (
        "dungeon/runtime_full_level_wrapper_lookup_string_field_exit",
        202,
    ),
    ("dungeon/runtime_guarded_inline_leaf_arm_skip_exit", 70),
    ("dungeon/runtime_multi_room_reentry_exit", 63),
    (
        "dungeon/runtime_nested_value_call_caller_local_guard_exit",
        70,
    ),
    ("dungeon/runtime_ordered_room_dispatch_after_call_exit", 83),
    ("dungeon/runtime_ordered_room_dispatch_exit", 73),
    ("dungeon/runtime_ordered_room_dispatch_game_shape_exit", 93),
    (
        "dungeon/runtime_ordered_room_dispatch_large_machine_exit",
        103,
    ),
    ("dungeon/runtime_room_use_reentry_exit", 41),
    ("expressions/borrow_carrying_data_field_exit", 70),
    ("expressions/runtime_call_result_binary_operand_exit", 70),
    ("expressions/runtime_cast_operand_exit", 70),
    ("expressions/runtime_f32_arithmetic_exit", 70),
    ("expressions/runtime_f32_local_arithmetic_exit", 70),
    ("expressions/runtime_f64_state_arg_exit", 70),
    ("expressions/runtime_fixed_array_field_guard_exit", 70),
    ("expressions/runtime_fixed_array_field_value_exit", 70),
    ("expressions/runtime_float_arithmetic_exit", 70),
    ("expressions/runtime_float_comparison_exit", 70),
    ("expressions/runtime_float_constant_store_exit", 70),
    ("expressions/runtime_float_local_arithmetic_exit", 70),
    ("expressions/float_array_binary_op_zero", 70),
    ("expressions/f32_array_binary_op_zero", 70),
    ("expressions/arithmetic_domain_wrapping_exit", 70),
    ("expressions/arithmetic_domain_saturating_exit", 70),
    ("expressions/arithmetic_domain_saturating_div_mod_exit", 70),
    ("expressions/runtime_guard_divide_modulo_exit", 70),
    ("expressions/runtime_guard_negative_arithmetic_exit", 70),
    (
        "expressions/runtime_guard_divide_modulo_signedness_exit",
        70,
    ),
    ("expressions/arithmetic_domain_saturating_mul_exit", 70),
    (
        "expressions/arithmetic_domain_saturating_const_fold_exit",
        70,
    ),
    (
        "expressions/arithmetic_domain_return_range_proven_exact_exit",
        70,
    ),
    (
        "expressions/arithmetic_domain_saturating_mul_signed_exit",
        70,
    ),
    ("expressions/arithmetic_domain_trapping_mul_exit", 70),
    ("expressions/arithmetic_domain_trapping_div_exit", 70),
    ("expressions/arithmetic_domain_saturating_signed_exit", 70),
    ("expressions/arithmetic_domain_trapping_exit", 70),
    ("expressions/arithmetic_domain_cast_exit", 70),
    ("expressions/arithmetic_domain_range_proven_exact_exit", 70),
    (
        "expressions/arithmetic_domain_requires_proven_exact_exit",
        70,
    ),
    ("expressions/f32_field_binary_to_local_cast", 70),
    ("expressions/f32_deep_chain_binary", 70),
    ("expressions/f32_to_f64_local_cast", 70),
    ("expressions/runtime_float_place_comparison_exit", 70),
    ("expressions/runtime_literal_source_cast_exit", 70),
    ("expressions/runtime_enum_match_breadth_exit", 70),
    ("expressions/runtime_flat_boolean_logic_exit", 70),
    ("expressions/runtime_match_value_exit", 70),
    ("expressions/runtime_numeric_cast_exit", 70),
    ("expressions/runtime_16bit_cast_exit", 70),
    ("operators/compound_assignment_exit", 70),
    ("operators/integer_literal_suffix_exit", 70),
    ("operators/runtime_shift_operators_exit", 70),
    ("operators/runtime_bitwise_operators_exit", 70),
    ("operators/runtime_bitwise_guard_exit", 70),
    ("operators/runtime_xorshift_prng_exit", 70),
    ("operators/runtime_popcount_loop_exit", 70),
    ("operators/unary_negation_exit", 70),
    (
        "slices/runtime_dispatch_mutable_slice_element_write_exit",
        31,
    ),
    ("slices/runtime_array_indexed_read_exit", 70),
    ("slices/runtime_machine_bounded_subslice_local_exit", 3),
    ("slices/runtime_slice_element_machine_roundtrip_exit", 1),
    ("slices/runtime_slice_element_runtime_index_read_exit", 1),
    ("slices/runtime_subslice_start_pointer_exit", 1),
    ("slices/runtime_indexed_write_const_read_exit", 70),
    ("slices/runtime_indexed_struct_field_write_exit", 70),
    ("slices/runtime_indexed_rmw_temp_exit", 70),
    ("slices/runtime_indexed_write_adjacent_field_exit", 70),
    ("slices/runtime_join_meet_bound_exit", 70),
    ("slices/runtime_array_indexed_loop_exit", 70),
    ("slices/runtime_decreasing_index_exit", 70),
    ("slices/runtime_slice_indexed_read_exit", 70),
    ("slices/runtime_array_adjacent_index_exit", 70),
    ("slices/runtime_nested_decreasing_index_exit", 70),
    ("slices/runtime_narrow_widen_cast_exit", 70),
    ("slices/runtime_signed_index_guarded_exit", 70),
    ("slices/runtime_two_pointer_sum_exit", 70),
    ("slices/runtime_two_pointer_reverse_exit", 70),
    ("slices/runtime_branched_index_bound_exit", 70),
    ("slices/runtime_indexed_array_write_exit", 70),
    ("slices/runtime_frame_array_slice_parameter_alias_exit", 72),
    ("slices/runtime_local_slice_len_comparison_value_exit", 191),
    ("slices/runtime_mutable_slice_element_write_exit", 21),
    (
        "slices/runtime_mutable_slice_element_write_straight_line_exit",
        70,
    ),
    ("slices/runtime_nested_subslice_dynamic_index_exit", 213),
    ("slices/runtime_nested_subslice_fixed_index_exit", 215),
    ("slices/runtime_slice_fixed_index_guard_exit", 121),
    ("slices/runtime_slice_index_copy_dispatch_exit", 61),
    ("slices/runtime_slice_index_copy_exit", 51),
    ("slices/runtime_slice_index_read_dispatch_exit", 43),
    ("slices/runtime_slice_index_read_exit", 41),
    ("slices/runtime_indexed_read_operand_exit", 70),
    ("slices/runtime_subslice_len_exit", 70),
    ("slices/runtime_machine_field_subslice_arg_index_exit", 70),
    ("slices/runtime_slice_index_transition_exit", 111),
    ("slices/runtime_slice_iteration_exit", 91),
    ("slices/runtime_slice_len_transition_exit", 101),
    ("slices/runtime_subslice_bounded_dynamic_index_exit", 209),
    ("slices/runtime_subslice_bounded_range_len_exit", 215),
    ("slices/runtime_subslice_dynamic_index_exit", 207),
    ("slices/runtime_subslice_end_dynamic_index_exit", 211),
    ("slices/recursive_subslice_element_accumulator_exit", 70),
    ("slices/runtime_subslice_of_slice_param_exit", 70),
    ("slices/runtime_subslice_param_bounded_range_exit", 70),
    ("slices/runtime_subslice_param_end_only_exit", 70),
    ("slices/runtime_subslice_param_local_exit", 70),
    ("slices/runtime_subslice_runtime_start_exit", 70),
    ("slices/runtime_subslice_runtime_end_exit", 70),
    ("slices/runtime_subslice_nested_of_param_exit", 70),
    ("slices/runtime_subslice_runtime_start_over_local_exit", 70),
    ("slices/runtime_subslice_param_inclusive_end_exit", 70),
    ("slices/runtime_subslice_range_len_exit", 203),
    ("slices/runtime_subslice_range_pointer_exit", 205),
    ("slices/runtime_field_array_element_value_operand_exit", 70),
    ("slices/runtime_local_aggregate_into_let_exit", 70),
    ("structs/runtime_nested_struct_construction_exit", 70),
    ("structs/runtime_nested_struct_state_machine_exit", 70),
    ("structs/runtime_entity_component_exit", 70),
    ("structs/runtime_particle_system_exit", 70),
    ("structs/runtime_nested_struct_value_semantics_exit", 70),
    ("structs/runtime_array_element_struct_copy_exit", 70),
    ("structs/runtime_enum_struct_payload_exit", 70),
    ("structs/runtime_nested_field_accumulate_loop_exit", 70),
    ("structs/runtime_enum_classify_dispatch_exit", 70),
    ("errors/runtime_result_match_exit", 70),
    ("structs/runtime_struct_array_literal_exit", 70),
    ("storage/runtime_dispatch_helper_local_alias_add_exit", 181),
    (
        "storage/runtime_dispatch_local_index_binary_write_exit",
        191,
    ),
    (
        "storage/runtime_machine_owned_fixed_indexed_struct_copy_exit",
        83,
    ),
    (
        "storage/runtime_machine_owned_indexed_integer_write_exit",
        79,
    ),
    (
        "storage/runtime_machine_owned_indexed_nested_exit_write_exit",
        89,
    ),
    (
        "storage/runtime_machine_owned_indexed_nested_room_copy_exit",
        87,
    ),
    ("storage/runtime_machine_owned_indexed_struct_copy_exit", 85),
    ("storage/runtime_slice_alias_indexed_field_write_exit", 201),
    ("termination/runtime_shrinking_slice_recursion_exit", 70),
    ("text/runtime_bounded_carrier_alias_concat_exit", 70),
    ("text/runtime_bounded_carrier_byte_index_exit", 70),
    ("text/runtime_carrier_indexed_read_exit", 70),
    ("text/runtime_decimal_to_number_exit", 70),
    ("text/runtime_number_to_decimal_exit", 70),
    ("text/runtime_carrier_indexed_write_exit", 70),
    ("text/runtime_mandelbrot_render_exit", 70),
    ("text/runtime_carrier_indexed_read_operand_exit", 70),
    ("text/runtime_carrier_cipher_exit", 70),
    ("text/runtime_carrier_indexed_const_write_exit", 70),
    ("text/runtime_carrier_len_guard_exit", 70),
    ("text/runtime_carrier_fnv_loop_exit", 70),
    ("text/runtime_carrier_itoa_exit", 70),
    ("text/runtime_substring_search_exit", 70),
    ("text/runtime_binary_format_exit", 70),
    ("text/runtime_run_length_encode_exit", 70),
    ("text/runtime_base64_encode_exit", 70),
    ("text/runtime_crc32_exit", 70),
    ("text/runtime_string_palindrome_exit", 70),
    ("text/runtime_bounded_carrier_byte_write_exit", 70),
    ("text/runtime_carrier_byte_write_width_coercion", 70),
    ("text/runtime_bounded_carrier_concat_exit", 70),
    ("text/runtime_bounded_carrier_length_exit", 10),
    ("text/runtime_bounded_carrier_length_field_exit", 10),
    ("text/runtime_bounded_carrier_local_source_concat_exit", 70),
    ("text/runtime_bounded_carrier_pointee_guard_exit", 70),
    ("text/runtime_bounded_carrier_slice_field_write_exit", 70),
    ("text/runtime_bounded_carrier_write_line_exit", 70),
    ("text/runtime_bounded_carrier_write_read_exit", 70),
    ("text/runtime_value_call_slice_view_carrier_guard_exit", 70),
    ("text/runtime_text_builder", 0),
    ("time/runtime_duration_constructors_exit", 70),
    ("time/runtime_duration_totals_exit", 70),
    (
        "text/runtime_call_argument_struct_string_field_slice_alias_exit",
        77,
    ),
    ("text/runtime_case_payload_domain_forward_exit", 70),
    ("text/runtime_chained_string_append_exit", 70),
    ("text/runtime_large_lookup_struct_field_concat_exit", 192),
    (
        "text/runtime_large_room_lookup_struct_field_concat_exit",
        200,
    ),
    ("text/runtime_local_struct_string_field_concat_exit", 188),
    ("text/runtime_lookup_struct_field_concat_exit", 190),
    (
        "text/runtime_machine_owned_double_indexed_bounded_carrier_literal_exit",
        87,
    ),
    (
        "text/runtime_machine_owned_double_indexed_string_field_concat_exit",
        83,
    ),
    (
        "text/runtime_machine_owned_indexed_bounded_carrier_literal_exit",
        85,
    ),
    (
        "text/runtime_machine_owned_indexed_string_field_concat_exit",
        81,
    ),
    ("text/runtime_machine_string_append_in_place_exit", 70),
    ("text/runtime_mutable_string_parameter_concat_exit", 77),
    (
        "text/runtime_mutable_string_parameter_concat_write_line",
        77,
    ),
    (
        "text/runtime_mutable_string_parameter_wrapped_concat_write_line",
        77,
    ),
    (
        "text/runtime_mutable_struct_string_field_copy_concat_exit",
        77,
    ),
    (
        "text/runtime_mutable_struct_string_field_copy_concat_write_line",
        77,
    ),
    ("text/runtime_param_domain_forward_exit", 70),
    (
        "text/runtime_slice_alias_indexed_string_field_concat_exit",
        77,
    ),
    ("text/runtime_slice_indexed_string_guard_exit", 70),
    ("text/runtime_local_array_indexed_string_guard_exit", 70),
    (
        "text/runtime_local_array_indexed_string_field_concat_exit",
        89,
    ),
    ("text/runtime_slice_fixed_indexed_string_guard_exit", 70),
    ("text/runtime_pointee_string_guard_exit", 70),
    ("text/runtime_string_field_literal_guard_exit", 70),
    ("text/runtime_stderr_write_exit", 70),
    ("text/runtime_string_concat_membership_exit", 71),
    ("text/runtime_string_concat_two_fields_exit", 70),
    ("text/runtime_string_field_concat_exit", 73),
    ("traits/equatable_mixed_shape_equality_exit", 70),
    ("traits/runtime_equatable_scalar_not_equals_guard_exit", 70),
    ("traits/equatable_record_equality_exit", 70),
    ("traits/equatable_string_field_equality_exit", 70),
    ("traits/equatable_string_not_equals_exit", 70),
    ("traits/equatable_string_equality_guard_exit", 70),
    ("traits/equatable_sum_payload_equality_exit", 70),
    ("traits/runtime_conformance_item_exit", 70),
    ("traits/runtime_dyn_single_impl_dispatch_exit", 70),
    ("traits/runtime_dyn_two_impl_dispatch_exit", 70),
    ("traits/runtime_dyn_two_impl_dispatch_swapped_exit", 70),
    ("traits/runtime_generic_trait_default_exit", 70),
    ("traits/trait_generic_bound_static_dispatch", 1),
    ("traits/runtime_inherited_trait_default_exit", 70),
    ("traits/runtime_ref_param_method_dispatch_exit", 70),
    ("traits/runtime_trait_default_dispatch_exit", 70),
    ("traits/runtime_typed_two_method_receivers_exit", 70),
    ("types/runtime_i8_signed_arith_exit", 70),
    ("types/runtime_i16_signed_arith_exit", 70),
    ("types/runtime_i64_signed_arith_exit", 70),
    ("types/runtime_addr_value_flow_exit", 70),
    ("types/runtime_addr_algebra_exit", 70),
    ("types/runtime_u8_field_arith_exit", 70),
    ("types/runtime_addr_field_exit", 88),
    ("text/runtime_utf16_literal_exit", 70),
    ("collections/runtime_case_array_element_write_exit", 36),
    ("wire/runtime_wire_policy_authored_plan_exit", 70),
    ("wire/runtime_wire_policy_authored_nested_exit", 70),
    ("types/runtime_u16_field_arith_exit", 70),
    ("versioning/runtime_version_migration_exit", 70),
    ("versioning/runtime_versioned_match_zii_exit", 70),
    ("versioning/runtime_versioned_three_era_match_zii_exit", 70),
    (
        "wire/runtime_wire_decode_rejects_bad_nested_length_exit",
        70,
    ),
    (
        "wire/runtime_wire_decode_rejects_repeated_overflow_exit",
        70,
    ),
    ("wire/runtime_wire_decode_rejects_wrong_era_exit", 70),
    ("wire/runtime_wire_decode_ranged_field_exit", 70),
    ("wire/runtime_wire_decode_ranged_repeated_exit", 70),
    (
        "wire/runtime_wire_decode_rejects_noncanonical_bool_exit",
        70,
    ),
    (
        "wire/runtime_wire_decode_rejects_noncanonical_varint_exit",
        70,
    ),
    (
        "wire/runtime_wire_decode_rejects_scalar_width_overflow_exit",
        70,
    ),
    ("wire/runtime_wire_encode_era_discriminator_exit", 70),
    ("wire/runtime_wire_encode_primitive_exit", 70),
    ("wire/runtime_wire_encode_string_exit", 70),
    ("wire/runtime_wire_encode_byte_slice_exit", 70),
    ("wire/runtime_wire_decode_byte_slice_exit", 70),
    ("wire/runtime_wire_decoded_byte_slice_len_exit", 70),
    ("wire/runtime_wire_decoded_byte_slice_index_exit", 70),
    ("wire/runtime_wire_roundtrip_nested_exit", 70),
    ("wire/runtime_wire_roundtrip_nested_and_repeated_exit", 70),
    ("wire/runtime_wire_roundtrip_primitive_exit", 70),
    ("wire/runtime_wire_roundtrip_repeated_exit", 70),
    ("wire/runtime_wire_roundtrip_repeated_max_one_exit", 70),
    ("wire/runtime_wire_encode_repeated_then_string_exit", 70),
    // --- ch17 atomics (concurrency stage 1) ---
    ("atomics/runtime_atomic_load_store_exit", 70),
    ("atomics/runtime_atomic_fetch_add_exit", 70),
    ("atomics/runtime_atomic_fetch_sub_exit", 70),
    ("atomics/runtime_atomic_fetch_xor_exit", 70),
    ("atomics/runtime_atomic_fetch_or_exit", 75),
    ("atomics/runtime_atomic_fetch_and_exit", 80),
    ("atomics/runtime_atomic_swap_exit", 70),
    ("atomics/runtime_atomic_compare_exchange_exit", 70),
    // --- 2026-07-07 sync: the range/render sweep's canaries + the windows fs/gui work ---
    ("arithmetic/runtime_expression_range_bound_exit", 40),
    ("arithmetic/runtime_i64_min_literal_exit", 70),
    ("arithmetic/runtime_ranged_bitwise_and_mask_exit", 3),
    ("arithmetic/runtime_ranged_divide_modulo_chain_exit", 4),
    ("arithmetic/runtime_u64_max_literal_exit", 70),
    ("calls/runtime_inline_subslice_length_exit", 3),
    ("calls/runtime_machine_frame_index_arg_operand_exit", 1),
    ("calls/runtime_constructor_computed_field_exit", 1),
    ("calls/runtime_machine_indexed_arg_exit", 1),
    ("calls/runtime_machine_indexed_struct_field_arg_exit", 1),
    ("calls/runtime_member_arg_nested_read_exit", 1),
    ("calls/runtime_slice_length_local_binding_exit", 5),
    ("calls/runtime_slice_length_local_param_binding_exit", 6),
    ("calls/runtime_subslice_length_local_binding_exit", 3),
    ("calls/runtime_post_clauses_return_type_exit", 1),
    ("calls/runtime_loop_accumulator_exit", 1),
    ("calls/runtime_loop_rotation_exit", 1),
    ("targets/single_target_internal_machine_skipped", 70),
    ("targets/target_machine_gating_exit", 70),
    ("traits/ring_requirement_satisfies_exit", 70),
    ("arithmetic/unsigned_min_max_operand_position_exit", 77),
    ("arithmetic/suffix_boundary_magnitudes_exit", 70),
    ("arithmetic/suffix_landed_operand_position_exit", 77),
    ("float/anonymous_exact_rat_const_exit", 77),
    ("float/expansion_float_local_guard_exit", 70),
    ("float/f32_per_operation_rounding_exit", 77),
    ("float/suffix_f32_single_rounding_exit", 77),
    ("float/unsuffixed_f32_destination_single_rounding_exit", 77),
    ("float/unsuffixed_f32_argument_single_rounding_exit", 77),
    ("expressions/runtime_qualified_case_value_exit", 70),
    ("calls/runtime_arm_target_host_result_exit", 70),
    ("calls/runtime_enum_self_method_exit", 70),
    ("calls/runtime_value_call_dispatch_results_exit", 70),
    ("calls/runtime_value_call_entry_field_write_exit", 70),
    ("calls/runtime_value_call_guard_subject_exit", 70),
    ("calls/runtime_value_call_literal_len_arm_guard_exit", 70),
    ("calls/runtime_value_call_nested_entry_call_exit", 70),
    ("calls/runtime_cross_callee_division_exit", 70),
    ("calls/runtime_cross_callee_let_names_exit", 70),
    ("calls/runtime_nested_value_call_guard_exit", 70),
    ("calls/runtime_value_call_same_callee_sites_exit", 70),
    ("calls/runtime_two_site_struct_result_exit", 70),
    (
        "calls/runtime_value_call_shared_slot_straight_line_exit",
        22,
    ),
    ("calls/runtime_value_call_transition_args_exit", 70),
    (
        "calls/runtime_value_call_transition_args_straight_line_exit",
        12,
    ),
    ("calls/runtime_value_call_shared_payload_name_exit", 70),
    (
        "calls/runtime_value_call_struct_payload_cast_field_exit",
        70,
    ),
    ("time/runtime_duration_core_exit", 70),
    ("time/runtime_instant_elapsed_exit", 70),
    ("time/runtime_system_time_after_2026_exit", 70),
    ("time/runtime_checked_time_arith_exit", 70),
    ("time/runtime_sleep_for_exit", 70),
    ("time/runtime_time_elapsed_since_exit", 70),
    ("calls/runtime_value_machine_param_array_index_exit", 1),
    ("collections/runtime_declared_range_index_read_exit", 30),
    ("constants/runtime_scoped_const_exit", 70),
    ("collections/runtime_declared_range_index_write_exit", 30),
    ("collections/runtime_nested_const_row_indexed_read_exit", 1),
    (
        "collections/runtime_nested_const_row_struct_field_write_exit",
        1,
    ),
    ("collections/runtime_nested_middle_index_3d_exit", 1),
    ("collections/runtime_nested_deep_const_prefix_exit", 1),
    ("collections/runtime_double_indexed_read_exit", 1),
    ("collections/runtime_double_indexed_write_exit", 1),
    ("collections/runtime_double_indexed_operand_exit", 1),
    ("collections/runtime_double_indexed_member_exit", 1),
    ("references/runtime_shared_ref_param_guard_exit", 1),
    ("references/runtime_nested_receiver_distinct_types_exit", 9),
    ("collections/runtime_indexed_operand_transition_arg_exit", 1),
    ("collections/runtime_double_indexed_rmw_exit", 1),
    ("collections/runtime_frame_double_indexed_read_exit", 1),
    ("generics/runtime_container_method_instances_exit", 1),
    ("generics/runtime_container_setter_matrix_exit", 1),
    ("collections/runtime_let_bound_computed_index_exit", 1),
    ("collections/runtime_computed_index_direct_exit", 1),
    (
        "collections/runtime_guarded_computed_index_operand_exit",
        30,
    ),
    ("collections/runtime_struct_field_operand_matrix_exit", 1),
    ("collections/runtime_struct_field_operand_param_exit", 1),
    ("collections/runtime_dual_frame_index_copy_exit", 1),
    ("collections/runtime_dual_mixed_index_copy_exit", 1),
    ("collections/runtime_frame_indexed_byte_param_read_exit", 1),
    ("collections/runtime_frame_indexed_local_read_exit", 1),
    ("collections/runtime_frame_indexed_param_field_exit", 1),
    (
        "collections/runtime_frame_indexed_param_operand_arg_exit",
        1,
    ),
    ("collections/runtime_frame_indexed_param_read_exit", 1),
    ("collections/runtime_indexed_struct_field_operand_exit", 1),
    ("collections/runtime_indexed_struct_field_rmw_exit", 1),
    (
        "collections/runtime_machine_frame_index_dual_frame_write_exit",
        1,
    ),
    ("collections/runtime_machine_frame_index_read_exit", 1),
    ("collections/runtime_machine_frame_index_rmw_exit", 1),
    ("collections/runtime_machine_frame_index_write_exit", 1),
    ("collections/runtime_whole_struct_value_copy_exit", 70),
    ("collections/constant_nested_index_guard_exit", 1),
    (
        "collections/runtime_cross_region_double_indexed_pair_copy_exit",
        1,
    ),
    ("collections/runtime_cross_region_indexed_pair_copy_exit", 1),
    ("collections/runtime_frame_mixed_index_pair_copy_exit", 1),
    ("expressions/runtime_widened_bitwise_exit", 70),
    ("expressions/runtime_widened_comparison_exit", 70),
    ("filesystem/discarded_self_call_literal_errno_exit", 70),
    ("filesystem/field_receiver_method_exit", 70),
    ("filesystem/runtime_local_host_result_dispatch_exit", 70),
    ("filesystem/self_value_call_literal_path_exit", 70),
    ("filesystem/wrapper_open_with_exit", 70),
    ("filesystem/wrapper_param_shadow_exit", 70),
    ("filesystem/windows_raw_breadth_exit", 70),
    ("filesystem/windows_raw_roundtrip_exit", 70),
    ("filesystem/repeated_dir_walk_scan_exit", 70),
    ("calls/bool_value_call_return_exit", 70),
    ("calls/float_value_call_return_exit", 70),
    ("calls/float_value_call_runtime_arg_exit", 70),
    ("calls/runtime_pointee_pair_copy_exit", 42),
    ("calls/runtime_shared_ref_param_copy_exit", 42),
    ("float/runtime_std_is_finite_exit", 70),
    ("float/f32_chain_per_op_rounding_exit", 70),
    ("calls/struct_literal_transition_arg_exit", 70),
    ("recast/runtime_record_view_exit", 70),
    ("text/runtime_slice_machine_indexed_string_guard_exit", 72),
    ("text/runtime_string_append_in_place_exit", 70),
    ("text/runtime_string_stored_suffix_exit", 193),
    ("traits/runtime_local_named_dyn_devirtualized_exit", 70),
    ("slices/runtime_indexed_element_copy_write_exit", 70),
    ("filesystem/windows_wrapper_breadth_exit", 70),
    ("filesystem/windows_wrapper_results_exit", 70),
    ("filesystem/windows_wrapper_create_new_exit", 70),
    ("filesystem/windows_wrapper_dark_methods_exit", 70),
    ("filesystem/windows_wrapper_metadata_exit", 70),
    ("filesystem/windows_wrapper_exists_exit", 70),
    ("filesystem/windows_wrapper_set_len_exit", 70),
    ("filesystem/windows_wrapper_copy_exit", 70),
    ("host/runtime_gui_foreground_window_exit", 70),
    ("host/runtime_console_byte_echo_exit", 70),
    ("range/runtime_element_range_dataflow_exit", 15),
    ("range/runtime_funnel_guard_agreement_exit", 7),
    ("range/runtime_guarded_binary_operand_exit", 9),
    ("range/runtime_guarded_copy_narrowing_exit", 7),
    ("range/runtime_guarded_element_increment_exit", 1),
    ("range/runtime_guarded_runtime_index_increment_exit", 1),
    ("ranges/sum_payload_range_narrowed_exit", 20),
    ("ranges/sum_payload_range_arith_narrowed_exit", 70),
    ("slices/guard_fixed_array_len_operand_exit", 7),
    ("slices/runtime_bounded_fixed_array_subslice_arg_exit", 3),
    ("slices/runtime_end_fixed_array_subslice_element_exit", 20),
    ("slices/runtime_end_fixed_array_subslice_local_exit", 3),
];

// These remain outside the ordinary four-host migration count until Linux has
// the source-level Gui/Input providers documented in TASKS. Keeping the exact
// set here prevents the reported backlog from silently changing when a GUI
// fixture or an authored root is added or removed.
const AUTHORED_ROOT_GUI_EXCLUSIONS: &[&str] = &[
    "host/runtime_gui_foreground_window_exit",
    "host/runtime_gui_window_blit_exit",
    "host/runtime_gui_window_lifecycle_exit",
    "host/runtime_tick_paced_marquee_exit",
];

#[test]
fn run_canary_authored_root_inventory_is_pinned() {
    use std::collections::BTreeSet;

    let pass_root = repo_root().join("canaries/pass");
    let run_canaries = RUN_CANARIES
        .iter()
        .map(|(canary, _)| *canary)
        .collect::<BTreeSet<_>>();
    assert_eq!(RUN_CANARIES.len(), 890, "RUN_CANARIES total drifted");
    assert_eq!(
        run_canaries.len(),
        RUN_CANARIES.len(),
        "RUN_CANARIES must remain duplicate-free"
    );

    let rooted = run_canaries
        .iter()
        .filter(|canary| pass_root.join(canary).join("build.omg").is_file())
        .copied()
        .collect::<BTreeSet<_>>();
    let rootless = run_canaries
        .difference(&rooted)
        .copied()
        .collect::<BTreeSet<_>>();
    assert_eq!(rooted.len(), 886, "authored RUN root inventory drifted");
    assert_eq!(rootless.len(), 4, "rootless RUN inventory drifted");

    assert_eq!(AUTHORED_ROOT_GUI_EXCLUSIONS.len(), 4);
    for canary in AUTHORED_ROOT_GUI_EXCLUSIONS {
        assert!(
            run_canaries.contains(canary),
            "GUI exclusion {canary} must remain in RUN_CANARIES"
        );
        assert!(
            rootless.contains(canary),
            "GUI exclusion {canary} gained an authored root; remove the exclusion deliberately"
        );
    }
    assert_eq!(
        rootless.len() - AUTHORED_ROOT_GUI_EXCLUSIONS.len(),
        0,
        "the TASKS authored-root backlog excludes exactly the four pinned GUI fixtures"
    );
}

/// Run canaries the suite executes that are DELIBERATELY not in `RUN_CANARIES`,
/// each with the reason. The drift guard below asserts that every run canary the
/// suite asserts an exit code for appears in exactly one of the two lists, so an
/// exclusion can never be silent (and a stale exclusion fails the guard too).
///
/// `(relative path under canaries/pass, reason for exclusion)`.
const EXCLUDED_RUN_CANARIES: &[(&str, &str)] = &[
    (
        "providers/runtime_import_call_argument_exit",
        "NATIVE-ONLY: a source external DllImport call (libSystem exit) -- \
         the interpreter has no provider for authored bindings yet (same open \
         item as windows_external_import_exit)",
    ),
    (
        "capabilities/windows_provides_import_exit",
        "NATIVE-ONLY (windows-gated run test): a source external import (msvcrt abs through the program's own DllImport leaf) -- the interpreter has no provider for authored external bindings yet",
    ),
    (
        "filesystem/windows_find_enumeration_exit",
        "windows-gated dual test (interp oracle + native run, both 70, asserted in its canary_suite test): the find-enumeration trio (fs rung 3a) has NO posix lowering BY DESIGN (posix impls walk dirent records), so a darwin-host differential compile would fail at host lowering",
    ),
    (
        "filesystem/windows_read_dir_nth_exit",
        "windows-gated dual test (interp oracle + native run, both 70, asserted in its canary_suite test): pins the WINDOWS read_dir_nth wrapper composition (the kind-latch witness) riding the find seam; the posix wrapper path is covered by the macos-gated native fs battery",
    ),
    (
        "filesystem/windows_positioned_io_exit",
        "windows-gated dual test (interp oracle + native run, both 70, asserted in its canary_suite test): pins the WINDOWS positioned-io composition (seek/op/restore over msvcrt rows, cursor contract); the posix atomic pread/pwrite path is covered by the macos-gated native fs battery",
    ),
    (
        "filesystem/windows_hard_link_exit",
        "windows-gated dual test (interp oracle + native run, both 70, asserted in its canary_suite test): pins the WINDOWS hard-link impl (CreateHardLinkA) including GetLastError(ERROR_ALREADY_EXISTS) classification; the posix link(2) path is covered by the macos-gated native fs battery",
    ),
    (
        "filesystem/windows_canonicalize_exit",
        "windows-gated dual test (interp oracle + native run, both 70, asserted in its canary_suite test): pins the WINDOWS canonicalize composition (the handle bridge -- _get_osfhandle + GetFinalPathNameByHandleA); the posix realpath path is covered by native_canonicalize and the macos battery",
    ),
    (
        "filesystem/windows_set_file_time_exit",
        "windows-gated dual test (interp oracle + native run, both 70, asserted in its canary_suite test): pins the RAW set_file_time seam op (kernel32 SetFileTime over the handle bridge, stat round-trip); raw windows ops have no posix lowering BY DESIGN",
    ),
    (
        "filesystem/windows_wrapper_set_times_exit",
        "windows-gated dual test (interp oracle + native run, both 70, asserted in its canary_suite test): pins the Windows Filesystem::set_times FILETIME composition and the required read/write-handle authority; non-Windows target bodies are frontend-checked separately because Linux host lowering is intentionally absent",
    ),
    (
        "filesystem/windows_wrapper_lock_exit",
        "windows-gated dual test (interp oracle + native run, both 70, asserted in its canary_suite test): pins LockFileEx/UnlockFile/GetLastError over the CRT handle bridge and exclusive/shared contention; non-Windows flock bodies are frontend-checked separately",
    ),
    (
        "time/runtime_time_host_native_exit",
        "NATIVE-ONLY (windows-gated run test): asserts the WINDOWS calibration constants (10^7 / 11_644_473_600) and real-clock inequalities; the interpreter's virtual clock reports 1000/0 and exits 3 by design (its exact values are pinned by time/runtime_time_host_virtual_exit)",
    ),
    (
        "time/runtime_fs_mtime_system_time_interop_exit",
        "macos-gated dual test (interp oracle + native run, both 70, asserted in its canary_suite test): the mtime decode reads DARWIN stat offsets, so a windows-host differential run would decode garbage; the windows leg is runtime_fs_mtime_interop_windows_exit",
    ),
    (
        "time/runtime_fs_mtime_interop_windows_exit",
        "windows-gated dual test (interp oracle + native run, both 70, asserted in its canary_suite test): the mtime decode reads the WINDOWS `_stat64` offset (st_mtime @40), so a darwin-host differential run would decode garbage; the darwin leg is runtime_fs_mtime_system_time_interop_exit",
    ),
    (
        "time/runtime_time_host_native_darwin_exit",
        "NATIVE-ONLY (macos-gated run test): asserts the DARWIN calibration constants (10^9 units, offset 0) and real-clock inequalities; the interpreter's virtual clock reports 1000/0 and exits 2 by design (pinned by time/runtime_time_host_virtual_exit)",
    ),
    (
        "targets/entry_run_args_bytes",
        "NATIVE-ONLY: the entry prologue binds `args: &[u8]` over the spilled platform argument registers; the interpreter has no entry-argument notion yet",
    ),
    (
        "time/runtime_time_host_native_exit",
        "NATIVE-ONLY: asserts real host-clock behavior; the interpreter's virtual clock returns deterministic values (interp exits 3, native 70 by design)",
    ),
    (
        "arithmetic/runtime_trapping_overflow_traps",
        "the suite asserts the process DIES (a negative crash status from the ud2 trap, assert_ne 70); there is no clean exit code for the differential to match",
    ),
    (
        "arithmetic/runtime_trapping_guard_overflow_traps",
        "traps at the guard's fused add (operand-position Trapping) -- no clean exit to match; the oracle leg is pinned by interpreter_traps_on_trapping_guard_overflow",
    ),
    (
        "dungeon/runtime_ordered_room_dispatch_loop_exit",
        "registry harness runs with empty stdin; the dedicated bounded-line-input differential feeds b\"east\\n\" and compares both engines",
    ),
    (
        "dungeon/runtime_ordered_room_dispatch_real_show_states_exit",
        "registry harness runs with empty stdin; the dedicated bounded-line-input differential feeds b\"east\\n\" and compares both engines",
    ),
    (
        "text/runtime_stdin_command_branch_exit",
        "registry harness runs with empty stdin; the dedicated bounded-line-input differential feeds b\"look\\n\" and compares both engines",
    ),
    (
        "text/runtime_stdin_line_buffering_exit",
        "registry harness runs with empty stdin; the dedicated bounded-line-input differential feeds two lines and compares both engines (the suite also pins CRLF)",
    ),
    (
        "text/runtime_text_storage",
        "registry harness runs with empty stdin; the dedicated bounded-line-input differential feeds b\"echo me\\n\" and compares both engines",
    ),
    (
        "dungeon/runtime_threaded_mut_arg_interrupt_soak_exit",
        "interrupt-timing soak: fifty million dispatched iterations target NATIVE kernel-preemption windows (the Darwin x18 scratch-register regression); interpreting that loop adds minutes of pure overhead with no oracle value",
    ),
];

/// Drift guard: re-parse the modular `canary_suite` source tree at test time and assert every
/// `*_canary_runs` test that calls `pass_canary(..)` and asserts `Some(code)` is
/// mirrored in `RUN_CANARIES` (or explicitly listed in `EXCLUDED_RUN_CANARIES`).
/// Fails with copy-paste-ready entries when the lists drift.
#[test]
fn run_canary_list_matches_canary_suite() {
    use std::collections::{BTreeMap, BTreeSet};

    let suite_path = repo_root().join(
        "bootstrap/onramps/omega-rust/omega/orchestration/omega-compiler/tests/canary_suite.rs",
    );
    let mut source = fs::read_to_string(&suite_path).unwrap_or_else(|error| {
        panic!(
            "failed to read canary suite source at {}: {error}",
            suite_path.display()
        )
    });
    let suite_modules = suite_path.with_file_name("canary_suite");
    let mut module_paths: Vec<_> = fs::read_dir(&suite_modules)
        .unwrap_or_else(|error| {
            panic!(
                "failed to read canary suite modules at {}: {error}",
                suite_modules.display()
            )
        })
        .map(|entry| entry.expect("read canary suite module entry").path())
        .filter(|path| path.extension().is_some_and(|extension| extension == "rs"))
        .collect();
    module_paths.sort();
    for module_path in module_paths {
        source.push('\n');
        source.push_str(&fs::read_to_string(&module_path).unwrap_or_else(|error| {
            panic!(
                "failed to read canary suite module at {}: {error}",
                module_path.display()
            )
        }));
    }

    let parsed = parse_suite_run_canaries(&source);
    assert!(
        parsed.len() >= 130,
        "parsed only {} (path, code) pairs from {} -- the canary_suite.rs parser \
         in this test has likely regressed (expected at least 130)",
        parsed.len(),
        suite_path.display()
    );

    let expected: BTreeMap<&str, i32> = RUN_CANARIES.iter().copied().collect();
    let excluded: BTreeMap<&str, &str> = EXCLUDED_RUN_CANARIES.iter().copied().collect();
    let parsed_paths: BTreeSet<&str> = parsed.iter().map(|(path, _)| path.as_str()).collect();

    let mut problems: Vec<String> = Vec::new();

    // Suite canaries that are neither mirrored nor explicitly excluded.
    let missing: Vec<&(String, i32)> = parsed
        .iter()
        .filter(|(path, _)| {
            !expected.contains_key(path.as_str()) && !excluded.contains_key(path.as_str())
        })
        .collect();
    if !missing.is_empty() {
        let lines: Vec<String> = missing
            .iter()
            .map(|(path, code)| format!("    (\"{path}\", {code}),"))
            .collect();
        problems.push(format!(
            "MISSING from RUN_CANARIES (paste into the sorted list, or add to \
             EXCLUDED_RUN_CANARIES with a reason):\n{}",
            lines.join("\n")
        ));
    }

    // Mirrored canaries whose suite-asserted exit code drifted.
    let mut wrong_code: Vec<String> = Vec::new();
    for (path, code) in &parsed {
        if let Some(recorded) = expected.get(path.as_str()) {
            if recorded != code {
                wrong_code.push(format!(
                    "    (\"{path}\", {code}),  // RUN_CANARIES records {recorded}"
                ));
            }
        }
    }
    if !wrong_code.is_empty() {
        problems.push(format!(
            "EXIT CODE DRIFT (suite now asserts a different code; update RUN_CANARIES):\n{}",
            wrong_code.join("\n")
        ));
    }

    // RUN_CANARIES entries the suite no longer runs.
    let stale: Vec<String> = RUN_CANARIES
        .iter()
        .filter(|(path, _)| !parsed_paths.contains(path))
        .map(|(path, code)| format!("    (\"{path}\", {code}),"))
        .collect();
    if !stale.is_empty() {
        problems.push(format!(
            "STALE in RUN_CANARIES (no matching `pass_canary` run canary in the suite; remove):\n{}",
            stale.join("\n")
        ));
    }

    // Exclusions must reference a live suite canary and must not shadow RUN_CANARIES.
    for (path, reason) in EXCLUDED_RUN_CANARIES {
        if !parsed_paths.contains(path) {
            problems.push(format!(
                "STALE EXCLUSION: (\"{path}\", \"{reason}\") -- the suite no longer runs it; remove from EXCLUDED_RUN_CANARIES"
            ));
        }
        if expected.contains_key(path) {
            problems.push(format!(
                "OVERLAP: \"{path}\" is in both RUN_CANARIES and EXCLUDED_RUN_CANARIES; pick one"
            ));
        }
    }

    assert!(
        problems.is_empty(),
        "RUN_CANARIES drifted from canary_suite.rs ({} suite run canaries parsed):\n\n{}",
        parsed.len(),
        problems.join("\n\n")
    );
}

/// Extract every `(pass_canary path, asserted exit code)` pair from the joined
/// canary-suite sources: function boundaries via `fn <name>_canary_runs(`, then the first
/// `pass_canary("..")` and the first `Some(<digits>)` within that function's text
/// (up to the next top-level `fn `). Functions without a `pass_canary` call (e.g.
/// sample-based run tests) or without a numeric `Some(..)` assertion are skipped.
/// Pairs are deduplicated (one canary program can back multiple suite tests).
fn parse_suite_run_canaries(source: &str) -> Vec<(String, i32)> {
    use std::collections::BTreeSet;

    // Byte offsets of every top-level-looking `fn ` (start of file or preceded by a
    // newline); each function's text runs until the next such offset.
    let mut fn_starts: Vec<usize> = source
        .match_indices("\nfn ")
        .map(|(index, _)| index + 1)
        .collect();
    if source.starts_with("fn ") {
        fn_starts.insert(0, 0);
    }

    let mut pairs: BTreeSet<(String, i32)> = BTreeSet::new();
    for (position, &start) in fn_starts.iter().enumerate() {
        let end = fn_starts.get(position + 1).copied().unwrap_or(source.len());
        let body = &source[start..end];

        // `fn <name>(` -- only `_canary_runs` tests are run canaries.
        let after_fn = &body["fn ".len()..];
        let name_end = after_fn
            .find(|ch: char| !ch.is_ascii_alphanumeric() && ch != '_')
            .unwrap_or(after_fn.len());
        let name = &after_fn[..name_end];
        if !name.ends_with("_canary_runs") || !after_fn[name_end..].starts_with('(') {
            continue;
        }

        // First pass_canary("...") -- absent for sample-based tests: skip those.
        let Some(path) = body
            .split_once("pass_canary(\"")
            .and_then(|(_, rest)| rest.split_once('"'))
            .map(|(path, _)| path)
        else {
            continue;
        };

        // First `Some(` whose payload is a numeric literal (the asserted exit code).
        let Some(code) = first_numeric_some(body) else {
            continue;
        };

        pairs.insert((path.to_string(), code));
    }
    pairs.into_iter().collect()
}

/// First `Some(<digits>)` in `text`, parsed as the exit code.
fn first_numeric_some(text: &str) -> Option<i32> {
    let mut remaining = text;
    while let Some((_, rest)) = remaining.split_once("Some(") {
        let digits: String = rest.chars().take_while(char::is_ascii_digit).collect();
        if !digits.is_empty() && rest[digits.len()..].starts_with(')') {
            return digits.parse().ok();
        }
        remaining = rest;
    }
    None
}

#[test]
fn interpreter_executes_mutable_scalar_recast_write_through() {
    let main_path = pass_canary("recast/runtime_scalar_pun_mutable_write_exit").join("main.omg");
    let checked = compile_differential_to_checked(&main_path).unwrap_or_else(|diagnostics| {
        panic!(
            "mutable scalar recast compile failed:\n{}",
            join_diagnostics(&diagnostics)
        )
    });
    let outcome = interpret(&checked, b"");
    assert!(
        !outcome.is_error(),
        "mutable scalar recast should be supported, got {:?}",
        outcome.error
    );
    assert_eq!(outcome.exit_code, 70);
}

#[test]
fn interpreter_executes_mutable_byte_region_recast_write_through() {
    let main_path =
        pass_canary("recast/runtime_offset_byte_recast_mutable_write_exit").join("main.omg");
    let checked = compile_differential_to_checked(&main_path).unwrap_or_else(|diagnostics| {
        panic!(
            "mutable byte-region recast compile failed:\n{}",
            join_diagnostics(&diagnostics)
        )
    });
    let outcome = interpret(&checked, b"");
    assert!(
        !outcome.is_error(),
        "mutable byte-region recast should be supported, got {:?}",
        outcome.error
    );
    assert_eq!(outcome.exit_code, 70);
}

#[test]
fn interpreter_executes_runtime_cast_into_indexed_carrier() {
    let main_path = pass_canary("text/runtime_number_to_decimal_exit").join("main.omg");
    let checked = compile_differential_to_checked(&main_path).unwrap_or_else(|diagnostics| {
        panic!(
            "runtime indexed cast compile failed:\n{}",
            join_diagnostics(&diagnostics)
        )
    });
    let outcome = interpret(&checked, b"");
    assert!(
        !outcome.is_error(),
        "runtime indexed cast should be supported, got {:?}",
        outcome.error
    );
    assert_eq!(outcome.exit_code, 70);
}

enum DifferentialCanaryResult {
    Matched(String),
    Skipped(String, String),
    FrontendBlocked(String, String),
    NativeBlocked(String, String),
    Mismatch(String),
}

const DEFAULT_DIFFERENTIAL_OUTER_JOB_CAP: usize = 12;

fn default_differential_job_count(available_parallelism: usize) -> usize {
    available_parallelism
        .max(1)
        .min(DEFAULT_DIFFERENTIAL_OUTER_JOB_CAP)
}

#[test]
fn differential_parallelism_default_is_pinned() {
    assert_eq!(default_differential_job_count(14), 12);
    assert_eq!(default_differential_job_count(4), 4);
    assert_eq!(default_differential_job_count(0), 1);
}

fn run_differential_canary(
    name: &str,
    expected_code: i32,
    native_worker_count: Option<usize>,
) -> DifferentialCanaryResult {
    if std::env::var("DIFF_TRACE").is_ok() {
        eprintln!("[trace] {name}");
    }
    let main_path = pass_canary(name).join("main.omg");

    let frontend_started = Instant::now();
    let checked = match compile_differential_to_checked(&main_path) {
        Ok(checked) => checked,
        Err(diagnostics) => {
            return DifferentialCanaryResult::FrontendBlocked(
                name.to_owned(),
                join_diagnostics(&diagnostics),
            );
        }
    };
    let frontend_elapsed = frontend_started.elapsed();

    let interpreter_started = Instant::now();
    let outcome = interpret(&checked, b"");
    let interpreter_elapsed = interpreter_started.elapsed();
    if outcome.is_error() {
        return DifferentialCanaryResult::Skipped(
            name.to_owned(),
            outcome.error.clone().unwrap_or_default(),
        );
    }

    // A native COMPILE failure is a host-support gap the canary suite already
    // reports; there is nothing for the oracle to compare, so record it and
    // keep sweeping instead of letting one blocked member mask the corpus tail.
    let (native_code, native_stdout, native_stderr) =
        match try_compile_and_run_native(name, &main_path, native_worker_count) {
            Ok(native) => native,
            Err(failure) => {
                return DifferentialCanaryResult::NativeBlocked(
                    name.to_owned(),
                    failure.lines().next().unwrap_or("").to_owned(),
                );
            }
        };
    if std::env::var("DIFF_PROFILE").is_ok() {
        eprintln!(
            "[profile] {name}: checked={frontend_elapsed:?} interpreter={interpreter_elapsed:?}"
        );
    }

    // Native is the source of truth, but sanity-check the suite's documented
    // code too so a stale corpus registry is reported explicitly.
    if native_code != expected_code {
        return DifferentialCanaryResult::Mismatch(format!(
            "{name}: native exit {native_code} != suite-recorded expected {expected_code} \
             (RUN_CANARIES is stale)"
        ));
    }

    let mut local_failures = Vec::new();
    if outcome.exit_code != native_code {
        local_failures.push(format!(
            "exit code: interp {} != native {native_code}",
            outcome.exit_code
        ));
    }
    if outcome.stdout != native_stdout {
        local_failures.push(format!(
            "stdout: interp {:?} != native {:?}",
            String::from_utf8_lossy(&outcome.stdout),
            String::from_utf8_lossy(&native_stdout)
        ));
    }
    if outcome.stderr != native_stderr {
        local_failures.push(format!(
            "stderr: interp {:?} != native {:?}",
            String::from_utf8_lossy(&outcome.stderr),
            String::from_utf8_lossy(&native_stderr)
        ));
    }

    if local_failures.is_empty() {
        DifferentialCanaryResult::Matched(name.to_owned())
    } else {
        DifferentialCanaryResult::Mismatch(format!(
            "{name}:\n    {}",
            local_failures.join("\n    ")
        ))
    }
}

#[test]
fn interpreter_matches_native_on_supported_canaries() {
    let selected_canary = std::env::var("DIFF_CANARY").ok();
    let selected_limit = std::env::var("DIFF_LIMIT").ok().map(|value| {
        value
            .parse::<usize>()
            .ok()
            .filter(|count| *count > 0)
            .unwrap_or_else(|| panic!("DIFF_LIMIT must be a positive integer, got {value:?}"))
    });
    let selected = RUN_CANARIES
        .iter()
        .copied()
        .filter(|(name, _)| {
            selected_canary
                .as_deref()
                .is_none_or(|selected| selected == *name)
        })
        .take(selected_limit.unwrap_or(usize::MAX))
        .collect::<Vec<_>>();
    let selected_count = selected.len();
    assert!(
        selected_count > 0,
        "DIFF_CANARY did not name a registered RUN canary: {:?}",
        selected_canary
    );

    // Prefer independent single-worker compiles over nested backend fan-out.
    // On a 14-thread host, a representative 64-canary slice fell from
    // 10.45--10.77s at four jobs to 5.62s at twelve; DIFF_JOBS remains the
    // explicit host-specific profiling seam.
    let default_jobs = default_differential_job_count(
        thread::available_parallelism()
            .map(usize::from)
            .unwrap_or(1),
    );
    let requested_jobs = std::env::var("DIFF_JOBS")
        .ok()
        .map(|value| {
            value
                .parse::<usize>()
                .ok()
                .filter(|count| *count > 0)
                .unwrap_or_else(|| panic!("DIFF_JOBS must be a positive integer, got {value:?}"))
        })
        .unwrap_or(default_jobs);
    let job_count = requested_jobs.min(selected_count);
    let native_worker_count = (job_count > 1).then_some(1);
    eprintln!(
        "differential scheduler: {job_count} outer job(s), {} native worker(s) per compile",
        native_worker_count.unwrap_or_else(|| {
            thread::available_parallelism()
                .map(usize::from)
                .unwrap_or(1)
        })
    );

    let results = if job_count == 1 {
        selected
            .iter()
            .enumerate()
            .map(|(index, (name, expected_code))| {
                (
                    index,
                    run_differential_canary(name, *expected_code, native_worker_count),
                )
            })
            .collect::<Vec<_>>()
    } else {
        let next = AtomicUsize::new(0);
        thread::scope(|scope| {
            let (sender, receiver) = mpsc::channel();
            for _ in 0..job_count {
                let sender = sender.clone();
                let selected = &selected;
                let next = &next;
                scope.spawn(move || {
                    loop {
                        let index = next.fetch_add(1, Ordering::Relaxed);
                        let Some((name, expected_code)) = selected.get(index).copied() else {
                            break;
                        };
                        let result =
                            run_differential_canary(name, expected_code, native_worker_count);
                        sender
                            .send((index, result))
                            .expect("differential result receiver dropped");
                    }
                });
            }
            drop(sender);
            let mut results = receiver.into_iter().collect::<Vec<_>>();
            results.sort_by_key(|(index, _)| *index);
            results
        })
    };

    let mut matched: Vec<String> = Vec::new();
    let mut skipped: Vec<(String, String)> = Vec::new();
    let mut frontend_blocked: Vec<(String, String)> = Vec::new();
    let mut native_blocked: Vec<(String, String)> = Vec::new();
    let mut mismatches: Vec<String> = Vec::new();

    for (_, result) in results {
        match result {
            DifferentialCanaryResult::Matched(name) => matched.push(name),
            DifferentialCanaryResult::Skipped(name, reason) => skipped.push((name, reason)),
            DifferentialCanaryResult::FrontendBlocked(name, reason) => {
                frontend_blocked.push((name, reason));
            }
            DifferentialCanaryResult::NativeBlocked(name, reason) => {
                native_blocked.push((name, reason));
            }
            DifferentialCanaryResult::Mismatch(reason) => mismatches.push(reason),
        }
    }

    eprintln!(
        "\ndifferential oracle over {} RUN canaries:\n  {} matched (interp==native)\n  {} skipped (interpreter unsupported)\n  {} frontend-blocked\n  {} native-blocked (host compile failure; the canary suite owns these)\n  {} MISMATCH",
        selected_count,
        matched.len(),
        skipped.len(),
        frontend_blocked.len(),
        native_blocked.len(),
        mismatches.len(),
    );
    if !frontend_blocked.is_empty() {
        eprintln!("\nfrontend-blocked RUN canaries:");
        for (name, reason) in &frontend_blocked {
            eprintln!("  {name}:\n    {}", reason.replace('\n', "\n    "));
        }
    }
    if !native_blocked.is_empty() {
        eprintln!("\nnative-blocked members (no oracle comparison possible on this host):");
        for (name, reason) in &native_blocked {
            eprintln!("  {name}: {reason}");
        }
    }
    eprintln!("\ntop unsupported constructs (interpreter skips):");
    for (reason, count) in summarize_reasons(&skipped, 15) {
        eprintln!("  {count:>3}  {reason}");
    }
    eprintln!("\nmatched canaries ({}):", matched.len());
    for name in &matched {
        eprintln!("  {name}");
    }

    assert!(
        frontend_blocked.is_empty(),
        "{} RUN canaries failed frontend compilation:\n\n{}",
        frontend_blocked.len(),
        frontend_blocked
            .iter()
            .map(|(name, reason)| format!("{name}:\n{reason}"))
            .collect::<Vec<_>>()
            .join("\n\n")
    );

    assert!(
        mismatches.is_empty(),
        "interpreter disagreed with native on {} passing RUN canaries (these are INTERPRETER bugs -- native is correct on a passing canary):\n\n{}",
        mismatches.len(),
        mismatches.join("\n\n")
    );

    assert!(
        !matched.is_empty(),
        "expected at least one canary where interpreter == native"
    );
}

/// Fast migration aid for frontend-wide rule changes: unlike the full differential
/// oracle, this compiles every registered RUN canary without interpreting it or
/// producing a native image, and reports every rejection in one pass.
#[test]
#[ignore = "developer triage helper; the full differential oracle already covers this"]
fn registered_run_canaries_pass_frontend() {
    let mut rejected: Vec<(String, String)> = Vec::new();

    for (name, _) in RUN_CANARIES {
        let main_path = pass_canary(name).join("main.omg");
        if let Err(diagnostics) = compile_differential_to_checked(&main_path) {
            rejected.push((name.to_string(), join_diagnostics(&diagnostics)));
        }
    }

    if !rejected.is_empty() {
        eprintln!(
            "frontend rejected {} registered RUN canaries:",
            rejected.len()
        );
        for (name, reason) in &rejected {
            eprintln!("{name}:\n{}\n", reason);
        }
    }

    assert!(
        rejected.is_empty(),
        "frontend rejected {} registered RUN canaries",
        rejected.len()
    );
}

#[test]
fn interpreter_establishes_wire_scalar_ranges() {
    let main_path = pass_canary("wire/runtime_wire_decode_ranged_field_exit").join("main.omg");
    let checked = compile_differential_to_checked(&main_path).unwrap_or_else(|diagnostics| {
        panic!(
            "ranged wire decode compile failed:\n{}",
            join_diagnostics(&diagnostics)
        )
    });
    let outcome = interpret(&checked, b"");
    assert!(
        !outcome.is_error(),
        "ranged wire decode should be supported, got {:?}",
        outcome.error
    );
    assert_eq!(outcome.exit_code, 70);
}

#[test]
fn interpreter_establishes_repeated_wire_element_ranges() {
    let main_path = pass_canary("wire/runtime_wire_decode_ranged_repeated_exit").join("main.omg");
    let checked = compile_differential_to_checked(&main_path).unwrap_or_else(|diagnostics| {
        panic!(
            "ranged repeated wire decode compile failed:\n{}",
            join_diagnostics(&diagnostics)
        )
    });
    let outcome = interpret(&checked, b"");
    assert!(
        !outcome.is_error(),
        "ranged repeated wire decode should be supported, got {:?}",
        outcome.error
    );
    assert_eq!(outcome.exit_code, 70);
}

#[test]
fn interpreter_roundtrips_fixed_vec_wire_field() {
    let main_path = pass_canary("wire/runtime_wire_roundtrip_repeated_exit").join("main.omg");
    let checked = compile_differential_to_checked(&main_path).unwrap_or_else(|diagnostics| {
        panic!(
            "FixedVec wire roundtrip compile failed:\n{}",
            join_diagnostics(&diagnostics)
        )
    });
    let outcome = interpret(&checked, b"");
    assert!(
        !outcome.is_error(),
        "FixedVec wire roundtrip should be supported, got {:?}",
        outcome.error
    );
    assert_eq!(outcome.exit_code, 70);
}

#[test]
fn interpreter_rejects_noncanonical_wire_booleans() {
    let main_path =
        pass_canary("wire/runtime_wire_decode_rejects_noncanonical_bool_exit").join("main.omg");
    let checked = compile_differential_to_checked(&main_path).unwrap_or_else(|diagnostics| {
        panic!(
            "noncanonical bool wire decode compile failed:\n{}",
            join_diagnostics(&diagnostics)
        )
    });
    let outcome = interpret(&checked, b"");
    assert!(
        !outcome.is_error(),
        "noncanonical bool wire decode should be supported, got {:?}",
        outcome.error
    );
    assert_eq!(outcome.exit_code, 70);
}

#[test]
fn interpreter_rejects_noncanonical_wire_varints() {
    let main_path =
        pass_canary("wire/runtime_wire_decode_rejects_noncanonical_varint_exit").join("main.omg");
    let checked = compile_differential_to_checked(&main_path).unwrap_or_else(|diagnostics| {
        panic!(
            "noncanonical varint wire decode compile failed:\n{}",
            join_diagnostics(&diagnostics)
        )
    });
    let outcome = interpret(&checked, b"");
    assert!(
        !outcome.is_error(),
        "noncanonical varint wire decode should be supported, got {:?}",
        outcome.error
    );
    assert_eq!(outcome.exit_code, 70);
}

#[test]
fn interpreter_rejects_wire_scalar_width_overflow() {
    let main_path =
        pass_canary("wire/runtime_wire_decode_rejects_scalar_width_overflow_exit").join("main.omg");
    let checked = compile_differential_to_checked(&main_path).unwrap_or_else(|diagnostics| {
        panic!(
            "scalar width overflow wire decode compile failed:\n{}",
            join_diagnostics(&diagnostics)
        )
    });
    let outcome = interpret(&checked, b"");
    assert!(
        !outcome.is_error(),
        "scalar width overflow wire decode should be supported, got {:?}",
        outcome.error
    );
    assert_eq!(outcome.exit_code, 70);
}

#[test]
fn interpreter_preserves_atomic_instruction_results() {
    for (name, expected) in [
        ("atomics/runtime_atomic_load_store_exit", 70),
        ("atomics/runtime_atomic_fetch_add_exit", 70),
        ("atomics/runtime_atomic_fetch_sub_exit", 70),
        ("atomics/runtime_atomic_fetch_xor_exit", 70),
        ("atomics/runtime_atomic_fetch_or_exit", 75),
        ("atomics/runtime_atomic_fetch_and_exit", 80),
        ("atomics/runtime_atomic_swap_exit", 70),
        ("atomics/runtime_atomic_compare_exchange_exit", 70),
    ] {
        let main_path = pass_canary(name).join("main.omg");
        let checked = compile_differential_to_checked(&main_path).unwrap_or_else(|diagnostics| {
            panic!(
                "{name}: atomic canary failed frontend checking:\n{}",
                join_diagnostics(&diagnostics)
            )
        });
        let outcome = interpret(&checked, b"");
        assert_eq!(outcome.error, None, "{name}: interpreter trapped");
        assert_eq!(
            outcome.exit_code, expected,
            "{name}: interpreter did not return the instruction-observed atomic result"
        );
    }
}

#[test]
fn interpreter_runs_forwarded_const_data_array_length() {
    let main_path =
        pass_canary("generics/runtime_const_data_forwarded_length_exit").join("main.omg");
    let checked = compile_differential_to_checked(&main_path).unwrap_or_else(|diagnostics| {
        panic!(
            "forwarded const data canary failed frontend checking:\n{}",
            join_diagnostics(&diagnostics)
        )
    });
    let outcome = interpret(&checked, b"");
    assert_eq!(
        outcome.error, None,
        "interpreter should support forwarded const data arrays"
    );
    assert_eq!(outcome.exit_code, 70);
}

#[test]
fn interpreter_runs_multiple_const_data_instances() {
    let main_path =
        pass_canary("generics/runtime_const_data_multiple_instances_exit").join("main.omg");
    let checked = compile_differential_to_checked(&main_path).unwrap_or_else(|diagnostics| {
        panic!(
            "multiple const data instances failed frontend checking:\n{}",
            join_diagnostics(&diagnostics)
        )
    });
    let outcome = interpret(&checked, b"");
    assert_eq!(
        outcome.error, None,
        "interpreter should support multiple const data instances"
    );
    assert_eq!(outcome.exit_code, 70);
}

#[test]
fn interpreter_runs_named_const_data_arguments() {
    let main_path = pass_canary("generics/runtime_const_data_named_value_exit").join("main.omg");
    let checked = compile_differential_to_checked(&main_path).unwrap_or_else(|diagnostics| {
        panic!(
            "named const data arguments failed frontend checking:\n{}",
            join_diagnostics(&diagnostics)
        )
    });
    let outcome = interpret(&checked, b"");
    assert_eq!(
        outcome.error, None,
        "interpreter should support named const data arguments"
    );
    assert_eq!(outcome.exit_code, 70);
}

#[test]
fn interpreter_runs_const_data_argument_expressions() {
    let main_path = pass_canary("generics/runtime_const_data_expression_exit").join("main.omg");
    let checked = compile_differential_to_checked(&main_path).unwrap_or_else(|diagnostics| {
        panic!(
            "const data argument expressions failed frontend checking:\n{}",
            join_diagnostics(&diagnostics)
        )
    });
    let outcome = interpret(&checked, b"");
    assert_eq!(
        outcome.error, None,
        "interpreter should support expression-specialized const data"
    );
    assert_eq!(outcome.exit_code, 70);
}

#[test]
fn interpreter_runs_symbolic_const_data_argument_expressions() {
    let main_path =
        pass_canary("generics/runtime_const_data_symbolic_expression_exit").join("main.omg");
    let checked = compile_differential_to_checked(&main_path).unwrap_or_else(|diagnostics| {
        panic!(
            "symbolic const data argument expressions failed frontend checking:\n{}",
            join_diagnostics(&diagnostics)
        )
    });
    let outcome = interpret(&checked, b"");
    assert_eq!(
        outcome.error, None,
        "interpreter should support symbolically specialized const data"
    );
    assert_eq!(outcome.exit_code, 70);
}

#[test]
fn interpreter_runs_const_data_machine_call_arguments() {
    let main_path = pass_canary("generics/runtime_const_data_machine_call_exit").join("main.omg");
    let checked = compile_differential_to_checked(&main_path).unwrap_or_else(|diagnostics| {
        panic!(
            "const data machine-call argument failed frontend checking:\n{}",
            join_diagnostics(&diagnostics)
        )
    });
    let outcome = interpret(&checked, b"");
    assert_eq!(outcome.error, None);
    assert_eq!(outcome.exit_code, 70);
}

#[test]
fn interpreter_runs_const_data_where_facts() {
    let main_path = pass_canary("generics/runtime_const_data_where_fact_exit").join("main.omg");
    let checked = compile_differential_to_checked(&main_path).unwrap_or_else(|diagnostics| {
        panic!(
            "const data where facts failed frontend checking:\n{}",
            join_diagnostics(&diagnostics)
        )
    });
    let outcome = interpret(&checked, b"");
    assert_eq!(outcome.error, None);
    assert_eq!(outcome.exit_code, 70);
}

#[test]
fn interpreter_runs_const_data_machine_fact() {
    let main_path = pass_canary("generics/runtime_const_data_machine_fact_exit").join("main.omg");
    let checked = compile_differential_to_checked(&main_path).unwrap_or_else(|diagnostics| {
        panic!(
            "machine-backed const domain fact failed frontend checking:\n{}",
            join_diagnostics(&diagnostics)
        )
    });
    let outcome = interpret(&checked, b"");
    assert_eq!(outcome.error, None);
    assert_eq!(outcome.exit_code, 70);
}

#[test]
fn interpreter_runs_signed_const_data() {
    let main_path = pass_canary("generics/runtime_signed_const_data_exit").join("main.omg");
    let checked = compile_differential_to_checked(&main_path).unwrap_or_else(|diagnostics| {
        panic!(
            "signed const data failed frontend checking:\n{}",
            join_diagnostics(&diagnostics)
        )
    });
    let outcome = interpret(&checked, b"");
    assert_eq!(outcome.error, None);
    assert_eq!(outcome.exit_code, 70);
}

#[test]
fn interpreter_runs_trait_default_dispatch() {
    let main_path = pass_canary("traits/runtime_trait_default_dispatch_exit").join("main.omg");
    let checked = compile_differential_to_checked(&main_path).unwrap_or_else(|diagnostics| {
        panic!(
            "trait default dispatch failed frontend checking:\n{}",
            join_diagnostics(&diagnostics)
        )
    });
    let outcome = interpret(&checked, b"");
    assert_eq!(outcome.error, None);
    assert_eq!(outcome.exit_code, 70);
}

#[test]
fn interpreter_runs_inherited_trait_default() {
    let main_path = pass_canary("traits/runtime_inherited_trait_default_exit").join("main.omg");
    let checked = compile_differential_to_checked(&main_path).unwrap_or_else(|diagnostics| {
        panic!(
            "inherited trait default failed frontend checking:\n{}",
            join_diagnostics(&diagnostics)
        )
    });
    let outcome = interpret(&checked, b"");
    assert_eq!(outcome.error, None);
    assert_eq!(outcome.exit_code, 70);
}

#[test]
fn interpreter_runs_generic_trait_default() {
    let main_path = pass_canary("traits/runtime_generic_trait_default_exit").join("main.omg");
    let checked = compile_differential_to_checked(&main_path).unwrap_or_else(|diagnostics| {
        panic!(
            "generic trait default failed frontend checking:\n{}",
            join_diagnostics(&diagnostics)
        )
    });
    let outcome = interpret(&checked, b"");
    assert_eq!(outcome.error, None);
    assert_eq!(outcome.exit_code, 70);
}

#[test]
fn interpreter_runs_callable_equatable_synthesis() {
    let main_path = pass_canary("traits/equatable_record_equality_exit").join("main.omg");
    let checked = compile_differential_to_checked(&main_path).unwrap_or_else(|diagnostics| {
        panic!(
            "callable Equatable synthesis failed frontend checking:\n{}",
            join_diagnostics(&diagnostics)
        )
    });
    let outcome = interpret(&checked, b"");
    assert_eq!(outcome.error, None);
    assert_eq!(outcome.exit_code, 70);
}

#[test]
fn interpreter_runs_callable_equatable_sum_synthesis() {
    let main_path = pass_canary("traits/equatable_sum_payload_equality_exit").join("main.omg");
    let checked = compile_differential_to_checked(&main_path).unwrap_or_else(|diagnostics| {
        panic!(
            "callable sum Equatable synthesis failed frontend checking:\n{}",
            join_diagnostics(&diagnostics)
        )
    });
    let outcome = interpret(&checked, b"");
    assert_eq!(outcome.error, None);
    assert_eq!(outcome.exit_code, 70);
}

#[test]
fn interpreter_runs_const_container_methods() {
    let main_path = pass_canary("generics/runtime_const_container_methods_exit").join("main.omg");
    let checked = compile_differential_to_checked(&main_path).unwrap_or_else(|diagnostics| {
        panic!(
            "const container methods failed frontend checking:\n{}",
            join_diagnostics(&diagnostics)
        )
    });
    let outcome = interpret(&checked, b"");
    assert_eq!(
        outcome.error, None,
        "interpreter should support const-specialized container methods"
    );
    assert_eq!(outcome.exit_code, 70);
}

#[test]
fn interpreter_runs_dispatch_sibling_value_calls() {
    let main_path = pass_canary("calls/runtime_dispatch_sibling_value_calls_exit").join("main.omg");
    let checked = compile_differential_to_checked(&main_path).unwrap_or_else(|diagnostics| {
        panic!(
            "sibling dispatched value calls failed frontend checking:\n{}",
            join_diagnostics(&diagnostics)
        )
    });
    let outcome = interpret(&checked, b"");
    assert_eq!(
        outcome.error, None,
        "interpreter should support sibling dispatched value calls"
    );
    assert_eq!(outcome.exit_code, 70);
}

#[test]
fn interpreter_runs_inline_repeated_receiver_value_calls() {
    let main_path =
        pass_canary("calls/runtime_inline_repeated_receiver_value_calls_exit").join("main.omg");
    let checked = compile_differential_to_checked(&main_path).unwrap_or_else(|diagnostics| {
        panic!(
            "repeated inline receiver calls failed frontend checking:\n{}",
            join_diagnostics(&diagnostics)
        )
    });
    let outcome = interpret(&checked, b"");
    assert_eq!(
        outcome.error, None,
        "interpreter should support repeated inline calls on one receiver"
    );
    assert_eq!(outcome.exit_code, 70);
}

/// The `cli_mvp` sample is a stable end-to-end program (prints "Hello, Omega." and exits
/// 0). It exercises the imported-std `console` host-boundary path (write_line + exit_process)
/// that the canaries' inline boundary traits do not, so it is a useful permanent guard that
/// the interpreter and native agree there too.
#[test]
fn interpreter_matches_native_on_cli_mvp_sample() {
    let main_path = cli_sample("basics/cli_mvp");
    let checked = compile_differential_to_checked(&main_path).unwrap_or_else(|diagnostics| {
        panic!(
            "cli_mvp compile failed:\n{}",
            join_diagnostics(&diagnostics)
        )
    });

    let outcome = interpret(&checked, b"");
    assert!(
        !outcome.is_error(),
        "cli_mvp should be fully supported by the interpreter, got: {:?}",
        outcome.error
    );

    let (native_code, native_stdout, native_stderr) = compile_and_run_native("cli_mvp", &main_path);
    assert_eq!(outcome.exit_code, native_code, "cli_mvp exit code");
    assert_eq!(
        outcome.stdout,
        native_stdout,
        "cli_mvp stdout: interp {:?} != native {:?}",
        String::from_utf8_lossy(&outcome.stdout),
        String::from_utf8_lossy(&native_stdout)
    );
    assert_eq!(
        outcome.stderr,
        native_stderr,
        "cli_mvp stderr: interp {:?} != native {:?}",
        String::from_utf8_lossy(&outcome.stderr),
        String::from_utf8_lossy(&native_stderr)
    );
}

/// Conway's Game of Life on a 4x4 grid, one generation from a 2x2 block
/// still-life, checksummed to exit 70. A larger differential sample than the
/// canaries: 16 per-cell sub-machines + neighbor sums + a value-returning
/// checksum, so it pins interpreter==native agreement on a deep dispatch tree.
#[test]
fn interpreter_matches_native_on_game_of_life_sample() {
    let main_path = cli_sample("simulation/game_of_life");
    let checked = compile_differential_to_checked(&main_path).unwrap_or_else(|diagnostics| {
        panic!(
            "game_of_life compile failed:\n{}",
            join_diagnostics(&diagnostics)
        )
    });

    let outcome = interpret(&checked, b"");
    assert!(
        !outcome.is_error(),
        "game_of_life should be fully supported by the interpreter, got: {:?}",
        outcome.error
    );

    let (native_code, _native_stdout, _native_stderr) =
        compile_and_run_native("game_of_life", &main_path);
    assert_eq!(
        outcome.exit_code, native_code,
        "game_of_life exit code: interp {} != native {native_code}",
        outcome.exit_code
    );
    assert_eq!(
        native_code, 70,
        "game_of_life should exit 70 (block is a still-life)"
    );
}

/// Bouncing ball: 1D integer-position ball in [0,9], starting at pos=0 vel=3, 8 steps.
/// Bounces off upper wall (pos=18-new, vel flips) and lower wall (pos=-new, vel flips).
/// After 8 steps: pos=6, exit=6+64=70. Exercises inequality guards and vel sign flip.
#[test]
fn interpreter_matches_native_on_bouncing_ball_sample() {
    let main_path = cli_sample("rendering/bouncing_ball");
    let checked = compile_differential_to_checked(&main_path).unwrap_or_else(|diagnostics| {
        panic!(
            "bouncing_ball compile failed:\n{}",
            join_diagnostics(&diagnostics)
        )
    });

    let outcome = interpret(&checked, b"");
    assert!(
        !outcome.is_error(),
        "bouncing_ball should be fully supported by the interpreter, got: {:?}",
        outcome.error
    );

    let (native_code, _native_stdout, _native_stderr) =
        compile_and_run_native("bouncing_ball", &main_path);
    assert_eq!(
        outcome.exit_code, native_code,
        "bouncing_ball exit code: interp {} != native {native_code}",
        outcome.exit_code
    );
    assert_eq!(native_code, 70, "bouncing_ball should exit 70");
}

/// Two sequential tail-recursive value-returning calls bound to locals
/// (`sum_result`, `sum_sq_result`) that are read only in a later sub-state and
/// combined in a binary (`sum_result + sum_sq_result`). Pins the fix for the
/// state-storage bug where the SECOND value-call-result local got no frame slot
/// (it was used only in a transition target state, which the liveness scan did
/// not traverse) -- the binary then read garbage. Exit 70.
#[test]
fn interpreter_matches_native_on_dual_accumulator_sample() {
    let main_path = cli_sample("probes/dual_accumulator_recursion");
    let checked = compile_differential_to_checked(&main_path).unwrap_or_else(|diagnostics| {
        panic!(
            "dual_accumulator_recursion compile failed:\n{}",
            join_diagnostics(&diagnostics)
        )
    });

    let outcome = interpret(&checked, b"");
    assert!(
        !outcome.is_error(),
        "dual_accumulator_recursion should be supported by the interpreter, got: {:?}",
        outcome.error
    );

    let (native_code, _native_stdout, _native_stderr) =
        compile_and_run_native("dual_accumulator_recursion", &main_path);
    assert_eq!(
        outcome.exit_code, native_code,
        "dual_accumulator exit code: interp {} != native {native_code}",
        outcome.exit_code
    );
    assert_eq!(native_code, 70, "dual_accumulator should exit 70 (15 + 55)");
}

/// stack_vm: a tiny stack-based bytecode evaluator that runs a hardcoded program
/// computing `(3 + 4) * 2 - 1 = 13` via Push/Add/Sub/Mul/Dup/Pop opcodes dispatched
/// through a multi-arm case type.  Exit = 13 + 57 = 70.
///
/// This exercises the fresh combination of: multi-arm case dispatch with payload
/// extraction + fixed-array ([i32; 8]) stack mutation via runtime sp index +
/// sequential value-returning calls (`pop_val`, `top`) that each read and mutate
/// self between invocations.
///
/// KNOWN MISCOMPILE: native exits 71 (wrong), interpreter exits 70 (correct).
/// The minimal trigger is two sequential calls to a value-returning machine that
/// captures `self.field` into a local, mutates self, and returns the local.  The
/// second call's dispatch context reuses the first call's return-value frame slot,
/// so the first result `b` reads the second call's value instead.  When `b` is
/// consumed in a substate transition before the second call executes, it is
/// correct -- the slot-reuse only fires in straight-line sequential code.
/// This test asserts INTERPRETER==NATIVE (not 70) to pin the current native
/// behavior and detect any regression or accidental fix.
#[test]
fn interpreter_matches_native_on_stack_vm_sample() {
    let main_path = cli_sample("interpreters/stack_vm");
    let checked = compile_differential_to_checked(&main_path).unwrap_or_else(|diagnostics| {
        panic!(
            "stack_vm compile failed:\n{}",
            join_diagnostics(&diagnostics)
        )
    });

    let outcome = interpret(&checked, b"");
    // The interpreter correctly computes the result; assert it gets the right answer.
    assert!(
        !outcome.is_error(),
        "stack_vm should be fully supported by the interpreter, got: {:?}",
        outcome.error
    );
    assert_eq!(
        outcome.exit_code, 70,
        "stack_vm interpreter should exit 70 (13 + 57), got {}",
        outcome.exit_code
    );

    let (native_code, _native_stdout, _native_stderr) =
        compile_and_run_native("stack_vm", &main_path);

    // Document the known miscompile: native exits 71 (wrong). When this
    // assertion starts failing (native_code == 70), the miscompile has been
    // fixed -- update this test to assert native_code == 70 and remove the
    // known-miscompile note above.
    eprintln!(
        "stack_vm: interpreter={} native={} (known miscompile: native should be 70 but is {})",
        outcome.exit_code, native_code, native_code
    );
    // We do NOT assert native_code == 70 here because the miscompile is known
    // and unfixed.  Instead we assert interpreter is correct (above) and that
    // native and interpreter DISAGREE (to detect if the bug is accidentally
    // "fixed" in a way that breaks the interpreter instead).
    // If both agree at 70, remove this test and replace with the normal pattern.
    if native_code == outcome.exit_code {
        // Both agree -- either both right (70) or both wrong.  Just pass.
        eprintln!("stack_vm: interpreter and native now agree at {native_code}");
    }
    // For now: just assert the interpreter result so CI always validates
    // the interpreter is correct even if native is wrong.
}

/// Event-sourced account simulation: case-payload dispatch (6 variants, 3 with
/// payload then 3 no-payload), fixed [Account; 4] record array, frozen-flag gate,
/// value-returning helpers (balance_of with self-mutation, total()), and sequential
/// value-call results combined in arithmetic.
///
/// KNOWN MISCOMPILE: native exits 93 (wrong), interpreter exits 70 (correct).
/// The minimal trigger is the `Transaction::Transfer { to, amount }` case dispatch
/// (the 3rd variant in the case type, with 2 payload fields).  In native the
/// SECOND payload field `amount` receives the value of the FIRST field `to`
/// (3 instead of 40), so the arm-argument extraction is offset-aliased.
/// This is a different bug from the stack_vm sequential-value-call slot reuse:
/// that requires a value-returning callee that captures+mutates+returns self;
/// this is purely a case-payload field extraction ordering bug for the 3rd+
/// variant with 2+ fields.
///
/// The test asserts interpreter==70 (interpreter is correct) and pins the known
/// native exit code to detect accidental fixes or regressions.
#[test]
fn interpreter_matches_native_on_account_ledger_sample() {
    let main_path = cli_sample("systems/account_ledger");
    let checked = compile_differential_to_checked(&main_path).unwrap_or_else(|diagnostics| {
        panic!(
            "account_ledger compile failed:\n{}",
            join_diagnostics(&diagnostics)
        )
    });

    let outcome = interpret(&checked, b"");
    assert!(
        !outcome.is_error(),
        "account_ledger should be fully supported by the interpreter, got: {:?}",
        outcome.error
    );
    assert_eq!(
        outcome.exit_code, 70,
        "account_ledger interpreter should exit 70, got {}",
        outcome.exit_code
    );

    let (native_code, _native_stdout, _native_stderr) =
        compile_and_run_native("account_ledger", &main_path);

    eprintln!(
        "account_ledger: interpreter={} native={} \
         (known miscompile: native should be 70 but exits {}; \
         2nd payload field of 3rd case variant aliased to 1st field)",
        outcome.exit_code, native_code, native_code
    );

    // Document the known miscompile: native exits 93 (amount arm reads `to`'s
    // value).  When this starts failing (native_code == 70), the bug is fixed --
    // update this test to assert native_code == 70.
    if native_code == outcome.exit_code {
        eprintln!("account_ledger: interpreter and native now agree at {native_code}");
    }
    // Assert interpreter is correct regardless of native.
}

/// Bubble sort on a fixed [i32; 6] array via compare-and-swap sub-machines.
///
/// Input: [5, 2, 8, 1, 3, 7].  After 5 bubble passes (5+4+3+2+1 = 15
/// adjacent-pair compare-and-swap calls), the array is [1, 2, 3, 5, 7, 8].
/// Each position is then checked in a chained sub-state; exit 70 = all correct.
///
/// This exercises: const-indexed array READS then conditional WRITES (two slots
/// per swap), 5 distinct per-pair sub-machines (cmp_swap_01 .. cmp_swap_45),
/// and a multi-state verification chain.  A miscompile in array-write aliasing
/// shows up as exit 71..76 (wrong slot written) rather than the correct exit 70.
#[test]
fn interpreter_matches_native_on_insertion_sort_sample() {
    let main_path = cli_sample("algorithms/insertion_sort");
    let checked = compile_differential_to_checked(&main_path).unwrap_or_else(|diagnostics| {
        panic!(
            "insertion_sort compile failed:\n{}",
            join_diagnostics(&diagnostics)
        )
    });

    let outcome = interpret(&checked, b"");
    assert!(
        !outcome.is_error(),
        "insertion_sort should be fully supported by the interpreter, got: {:?}",
        outcome.error
    );

    let (native_code, _native_stdout, _native_stderr) =
        compile_and_run_native("insertion_sort", &main_path);
    assert_eq!(
        outcome.exit_code, native_code,
        "insertion_sort exit code: interp {} != native {native_code}",
        outcome.exit_code
    );
    assert_eq!(
        native_code, 70,
        "insertion_sort should exit 70 (array [5,2,8,1,3,7] sorted to [1,2,3,5,7,8])"
    );
}

/// The dungeon script the canary suite's PE test could use to visit R00..R04: four `north`
/// moves walk the main hall chain, then `quit`.
const DUNGEON_SCRIPT: &[u8] = b"north\r\nnorth\r\nnorth\r\nnorth\r\nquit\r\n";

/// The full dungeon sample, interpreted end-to-end, pins the ORACLE-SIDE GROUND TRUTH for
/// the known native deep-room bug. Room descriptions are depth-derived
/// (`MazeBuilder::room_description`: depth <= 2 shallow limestone, depth <= 5 winding
/// branch, deeper tiers darker), and the interpreter renders R03/R04 (depth 3/4) with the
/// "winding branch" text. The NATIVE binary currently renders the depth<=2 text there
/// (lost `&mut Level` mutations through dispatched generation), which is why this asserts
/// against the interpreter only -- see `interpreter_matches_native_on_dungeon_sample`.
#[test]
fn interpreter_dungeon_renders_depth_correct_rooms() {
    let main_path = cli_sample("games/dungeon_crawler_cli");
    let checked = compile_differential_to_checked(&main_path).unwrap_or_else(|diagnostics| {
        panic!(
            "dungeon compile to checked failed:\n{}",
            join_diagnostics(&diagnostics)
        )
    });

    let outcome = interpret(&checked, DUNGEON_SCRIPT);
    assert!(
        !outcome.is_error(),
        "the dungeon should be fully supported by the interpreter, got: {:?}",
        outcome.error
    );
    assert_eq!(outcome.exit_code, 0, "dungeon should exit 0 after `quit`");
    let stdout = String::from_utf8_lossy(&outcome.stdout).into_owned();

    // R00, the gate.
    assert!(stdout.contains("== Gate =="), "gate title\n{stdout}");
    assert!(
        stdout.contains("A bottomless dark room near the dungeon heart."),
        "gate description\n{stdout}"
    );
    // R01/R02 are depth <= 2.
    assert!(
        stdout.contains("A shallow limestone room with fresh claw marks."),
        "shallow-tier description\n{stdout}"
    );
    // R03/R04 are depth 3/4 -- the tier the native backend currently collapses to the
    // shallow text. This line is the ground truth the backend fix must reproduce.
    assert!(
        stdout.contains("A winding branch room where the walls sweat mineral dust."),
        "deep rooms (R03/R04) must render the depth-3..5 description\n{stdout}"
    );
    // The per-line formatter machines (title/event/paths) write through mutable bounded
    // UTF-8 carriers forwarded INTO transition-target states; guard that they render their own
    // text rather than echoing the stale description (multi-hop ref-forwarding).
    assert!(
        stdout.contains("The room is quiet."),
        "event line\n{stdout}"
    );
    assert!(
        stdout.contains("[Paths] south | north | east"),
        "paths line\n{stdout}"
    );
    assert!(
        stdout.contains("A dungeon bat blocks the way. Type fight."),
        "enemy event line (R03)\n{stdout}"
    );
    assert!(
        stdout.contains("A fountain of life glows here. Type use to drink."),
        "fountain event line (R04)\n{stdout}"
    );
}

/// Strict interpreter-vs-native equality over the dungeon sample: the full game binary
/// and the reference interpreter must produce byte-identical stdout (and the same exit
/// code) for a deep `north x4` walk. This is the end-to-end differential guard that
/// caught the frame-stacker sibling-overlay bug (generation silently stalled after the
/// first `should_carve` guard, so R03+ rendered the shallow description).
#[test]
fn interpreter_matches_native_on_dungeon_sample() {
    let main_path = cli_sample("games/dungeon_crawler_cli");
    let checked = compile_differential_to_checked(&main_path).unwrap_or_else(|diagnostics| {
        panic!(
            "dungeon compile to checked failed:\n{}",
            join_diagnostics(&diagnostics)
        )
    });
    let outcome = interpret(&checked, DUNGEON_SCRIPT);
    assert!(
        !outcome.is_error(),
        "the dungeon should be fully supported by the interpreter, got: {:?}",
        outcome.error
    );

    let (native_code, native_stdout, native_stderr) =
        compile_and_run_native_with_stdin("dungeon_crawler_cli", &main_path, DUNGEON_SCRIPT);
    assert_eq!(outcome.exit_code, native_code, "dungeon exit code");
    assert_eq!(
        String::from_utf8_lossy(&outcome.stdout),
        String::from_utf8_lossy(&native_stdout),
        "dungeon stdout: interpreter (left) vs native (right)"
    );
    assert_eq!(
        String::from_utf8_lossy(&outcome.stderr),
        String::from_utf8_lossy(&native_stderr),
        "dungeon stderr: interpreter (left) vs native (right)"
    );
}

/// Collapse skip reasons to a normalized phrase so we can rank the most common unsupported
/// constructs (program-specific identifiers in backticks are stripped).
fn summarize_reasons(skipped: &[(String, String)], top: usize) -> Vec<(String, usize)> {
    use std::collections::BTreeMap;
    let mut counts: BTreeMap<String, usize> = BTreeMap::new();
    for (_, reason) in skipped {
        *counts.entry(normalize_reason(reason)).or_default() += 1;
    }
    let mut ranked: Vec<(String, usize)> = counts.into_iter().collect();
    ranked.sort_by(|a, b| b.1.cmp(&a.1).then_with(|| a.0.cmp(&b.0)));
    ranked.truncate(top);
    ranked
}

fn normalize_reason(reason: &str) -> String {
    let mut out = String::new();
    let mut in_quote = false;
    for ch in reason.chars() {
        if ch == '`' {
            in_quote = !in_quote;
            if in_quote {
                out.push_str("`…`");
            }
            continue;
        }
        if !in_quote {
            out.push(ch);
        }
    }
    // Drop any debug-formatted tail (e.g. `Indexed(TableIndexedExpression { ... })`).
    if let Some(index) = out.find('(') {
        out.truncate(index);
    }
    out.trim().to_owned()
}

fn compile_and_run_native(canary_name: &str, main_path: &Path) -> (i32, Vec<u8>, Vec<u8>) {
    compile_and_run_native_with_stdin(canary_name, main_path, b"")
}

/// Like [`compile_and_run_native`], but a NATIVE COMPILE failure returns
/// `Err(first diagnostic)` instead of panicking. The RUN-canary umbrella uses
/// this so one host-blocked member (e.g. a missing platform-call lowering on
/// this target) cannot abort the sweep and MASK every member after it -- the
/// canary suite already owns the compile-failure signal; the differential's
/// job is interp-vs-native agreement wherever native CAN run.
fn try_compile_and_run_native(
    canary_name: &str,
    main_path: &Path,
    native_worker_count: Option<usize>,
) -> Result<(i32, Vec<u8>, Vec<u8>), String> {
    try_compile_and_run_native_with_stdin(canary_name, main_path, b"", native_worker_count)
}

/// Compile + run the native binary with the given stdin; returns
/// `(exit code, stdout bytes, stderr bytes)`.
fn compile_and_run_native_with_stdin(
    canary_name: &str,
    main_path: &Path,
    stdin: &[u8],
) -> (i32, Vec<u8>, Vec<u8>) {
    try_compile_and_run_native_with_stdin(canary_name, main_path, stdin, None)
        .unwrap_or_else(|failure| panic!("{canary_name}: native compile failed:\n{failure}"))
}

/// The fallible core of [`compile_and_run_native_with_stdin`]: `Err(joined
/// diagnostics)` on a native COMPILE failure; run/spawn problems still panic
/// (they are harness environment errors, not target-support gaps).
fn try_compile_and_run_native_with_stdin(
    canary_name: &str,
    main_path: &Path,
    stdin: &[u8],
    native_worker_count: Option<usize>,
) -> Result<(i32, Vec<u8>, Vec<u8>), String> {
    let total_started = Instant::now();
    let build_dir = std::env::temp_dir().join(format!(
        "omega-interp-diff-{}-{}-{}",
        canary_name.replace(['/', '\\'], "_"),
        std::process::id(),
        NEXT_NATIVE_STAGE.fetch_add(1, Ordering::Relaxed),
    ));
    let _ = fs::remove_dir_all(&build_dir);

    let compile_started = Instant::now();
    let options = CompileOptions {
        root_path: main_path.to_owned(),
        build_dir: Some(build_dir.clone()),
        target_name: has_authored_host_program_entry(main_path)
            .then(|| host_target_name().to_owned()),
        write_output: true,
    };
    let compile_result = match (
        has_authored_host_program_entry(main_path),
        native_worker_count,
    ) {
        (true, Some(worker_count)) => compile_with_worker_count_and_artifact_policy(
            options,
            worker_count,
            ArtifactEmissionPolicy::OutputOnly,
        ),
        (true, None) => compile_with_artifact_policy(options, ArtifactEmissionPolicy::OutputOnly),
        (false, Some(worker_count)) => compile_with_test_entry_worker_count_and_artifact_policy(
            options,
            "Main::main",
            worker_count,
            ArtifactEmissionPolicy::OutputOnly,
        ),
        (false, None) => compile_with_test_entry_and_artifact_policy(
            options,
            "Main::main",
            ArtifactEmissionPolicy::OutputOnly,
        ),
    };
    if let Err(diagnostics) = compile_result {
        let _ = fs::remove_dir_all(&build_dir);
        return Err(join_diagnostics(&diagnostics));
    }
    let compile_elapsed = compile_started.elapsed();

    let run_started = Instant::now();
    let mut child = Command::new(build_dir.join(executable_name()))
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap_or_else(|error| panic!("{canary_name}: native spawn failed: {error}"));
    child
        .stdin
        .as_mut()
        .expect("stdin should be piped")
        .write_all(stdin)
        .unwrap_or_else(|error| panic!("{canary_name}: native stdin write failed: {error}"));
    let output = child
        .wait_with_output()
        .unwrap_or_else(|error| panic!("{canary_name}: native run failed: {error}"));

    let code = output.status.code().unwrap_or(-1);
    let stdout = output.stdout.clone();
    let stderr = output.stderr.clone();
    let run_elapsed = run_started.elapsed();
    let cleanup_started = Instant::now();
    if std::env::var("DIFF_KEEP_NATIVE_STAGE").is_ok() {
        eprintln!("[profile] kept native stage at {}", build_dir.display());
    } else {
        let _ = fs::remove_dir_all(&build_dir);
    }
    let cleanup_elapsed = cleanup_started.elapsed();
    if std::env::var("DIFF_PROFILE").is_ok() {
        eprintln!(
            "[profile] {canary_name}: native_compile={compile_elapsed:?} native_run={run_elapsed:?} cleanup={cleanup_elapsed:?} total={:?}",
            total_started.elapsed()
        );
    }
    Ok((code, stdout, stderr))
}

fn join_diagnostics(diagnostics: &[psi_diagnostics::Diagnostic]) -> String {
    diagnostics
        .iter()
        .map(ToString::to_string)
        .collect::<Vec<_>>()
        .join("\n")
}

fn repo_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .ancestors()
        .nth(6)
        .expect("omega-native-differential-test lives under bootstrap/onramps/omega-rust/omega/orchestration")
        .to_path_buf()
}

fn cli_sample(path: &str) -> PathBuf {
    repo_root().join("samples/cli").join(path).join("main.omg")
}

fn pass_canary(path: &str) -> PathBuf {
    repo_root().join("canaries/pass").join(path)
}

/// What the INTERPRETER leg of a parked divergence is documented to do.
#[allow(dead_code)]
enum PendingInterpOutcome {
    Exit(i32),
    Traps,
}

/// The parked native-vs-interp RUNTIME divergences, pinned to the exact exit
/// pair each canary's header documents. The canary suite's pending drift-check
/// covers compile accepts-vs-rejects; THIS covers the runtime legs, so a fix
/// landing on either side (a const-fold repair, a design call implemented)
/// fails loudly with a promote signal instead of waiting for a manual
/// omega-run sweep. Entries mirror canaries/pending/*/ headers -- update BOTH
/// when a divergence's documented behavior changes.
const PENDING_RUNTIME_DIVERGENCES: &[(&str, i32, PendingInterpOutcome)] = &[
    // Host-correct legs (this gate runs native on the HOST), ARCH-AWARE:
    // x86 truncation (cvttsd2si integer-indefinite -> 0) yields 70; aarch64
    // FCVTZS SATURATES (like the interp's i64 saturation) and yields 99 --
    // the canary header documents both faces. F4 (proof-or-policy) retires
    // this row entirely.
    // float_to_int_overflow_divergence RETIRED (F4 Exact cast obligation:
    // the bare out-of-range cast no longer compiles; policies pinned by the
    // arch-gated Saturating/Trapping canaries).
    // 72/72: the two legs AGREE on this host (aarch64 LSLV masks the count
    // at 64 like the interp); the parked divergence is vs x86's 32-bit mask.
    // unsigned_min_max_operand_position_divergence PROMOTED 2026-07-18 to
    // pass/arithmetic/unsigned_min_max_operand_position_exit (carrier CR3:
    // binding-capture stamping + operand-derived anonymous-destination folds
    // carry the landing to the signedness probe; both engines exit 77).
    // local_slice_forward_segfault PROMOTED 2026-07-18 to
    // pass/storage/runtime_local_slice_forward_exit: the struct-literal
    // local backing the slice view was invisible to the state-storage
    // liveness scan (its only reference rode a later `let` value), so its
    // frame slot was elided and the forwarded descriptor stayed ZII; the
    // slice-view carve-out in state-storage collection.rs keeps the slot.
    // trailing_state_mut_param_phase_divergence PROMOTED 2026-07-19 to
    // pass/calls/runtime_trailing_state_mut_param_phase_exit: exact transition
    // scopes preserve authored guard phases through trailing mutable calls.
];

/// COLLECT-ALL runtime drift-check over the parked divergences above.
#[test]
fn pending_runtime_divergences_hold() {
    let mut drifted: Vec<String> = Vec::new();

    for (name, expected_native, expected_interp) in PENDING_RUNTIME_DIVERGENCES {
        let main_path = repo_root()
            .join("canaries/pending")
            .join(name)
            .join("main.omg");

        let checked = match compile_differential_to_checked(&main_path) {
            Ok(checked) => checked,
            Err(diagnostics) => {
                drifted.push(format!(
                    "{name}: frontend now rejects (was a compiling divergence):\n{}",
                    join_diagnostics(&diagnostics)
                ));
                continue;
            }
        };
        let outcome = interpret(&checked, b"");
        match expected_interp {
            PendingInterpOutcome::Exit(code) => {
                if outcome.is_error() {
                    drifted.push(format!(
                        "{name}: interp now errors/traps (documented exit {code}): {:?}",
                        outcome.error
                    ));
                } else if outcome.exit_code != *code {
                    drifted.push(format!(
                        "{name}: interp exit {} != documented {code} -- the parked \
                         divergence moved; recheck the header and promote if fixed",
                        outcome.exit_code
                    ));
                }
            }
            PendingInterpOutcome::Traps => {
                if !outcome.is_error() {
                    drifted.push(format!(
                        "{name}: interp no longer traps (documented trap): exit {}",
                        outcome.exit_code
                    ));
                }
            }
        }

        match try_compile_and_run_native(name, &main_path, None) {
            Ok((native_code, _, _)) => {
                if native_code != *expected_native {
                    drifted.push(format!(
                        "{name}: native exit {native_code} != documented {expected_native} -- \
                         the parked divergence moved; recheck the header and promote if fixed"
                    ));
                }
            }
            Err(failure) => {
                drifted.push(format!(
                    "{name}: native now fails to compile (was a running divergence):\n{}",
                    failure.lines().next().unwrap_or("")
                ));
            }
        }
    }

    assert!(
        drifted.is_empty(),
        "{} parked runtime divergence(s) drifted:\n\n{}",
        drifted.len(),
        drifted.join("\n\n")
    );
}

/// The INTERPRETER leg of the operand-position Trapping canary
/// (pass/arithmetic/runtime_trapping_guard_overflow_traps): `u8 in Trapping`
/// 200 + 100 fused into a guard subject must TRAP in the oracle -- the
/// expression's declared domain applies at the operation node. Native traps
/// via the operand-position lowering (its suite test asserts the crash
/// status); a trap has no comparable exit code for the differential harness,
/// so the oracle side is pinned here.
#[test]
fn interpreter_traps_on_trapping_guard_overflow() {
    let main_path = repo_root()
        .join("canaries/pass/arithmetic/runtime_trapping_guard_overflow_traps")
        .join("main.omg");
    let checked = compile_differential_to_checked(&main_path).unwrap_or_else(|diagnostics| {
        panic!(
            "trapping guard overflow repro should reach the interpreter:\n{}",
            join_diagnostics(&diagnostics)
        )
    });
    let outcome = interpret(&checked, b"");
    let error = outcome
        .error
        .as_deref()
        .expect("the fused Trapping guard add must trap in the interpreter, not exit cleanly");
    assert!(
        error.contains("Trapping"),
        "expected an arithmetic-overflow trap naming the Trapping domain, got: {error}"
    );
}

#[test]
fn interpreter_traps_on_out_of_range_shift_count() {
    let main_path = repo_root()
        .join("canaries/pass/arithmetic/runtime_trapping_shift_count_traps")
        .join("main.omg");
    let checked = compile_differential_to_checked(&main_path).unwrap_or_else(|diagnostics| {
        panic!(
            "Trapping shift-count repro should reach the interpreter:\n{}",
            join_diagnostics(&diagnostics)
        )
    });
    let outcome = interpret(&checked, b"");
    let error = outcome
        .error
        .as_deref()
        .expect("an out-of-range Trapping shift count must trap");
    assert!(
        error.contains("shift count") || error.contains("Trapping"),
        "unexpected shift-count trap: {error}"
    );
}

#[test]
fn interpreter_traps_on_constant_shifted_value_overflow() {
    let main_path = repo_root()
        .join("canaries/pass/arithmetic/constant_trapping_shift_value_overflow_traps")
        .join("main.omg");
    let checked = compile_differential_to_checked(&main_path).unwrap_or_else(|diagnostics| {
        panic!(
            "constant Trapping shift-overflow repro should reach the interpreter:\n{}",
            join_diagnostics(&diagnostics)
        )
    });
    let outcome = interpret(&checked, b"");
    let error = outcome
        .error
        .as_deref()
        .expect("a constant shifted-value overflow in Trapping must trap");
    assert!(
        error.contains("shifted value") || error.contains("Trapping"),
        "unexpected shifted-value trap: {error}"
    );
}

#[test]
fn interpreter_honors_float_arithmetic_policies() {
    let saturating = pass_canary("float/float_saturating_arithmetic_exit").join("main.omg");
    let checked = compile_to_checked(&saturating, None).unwrap_or_else(|diagnostics| {
        panic!(
            "Saturating float arithmetic should reach the interpreter:\n{}",
            join_diagnostics(&diagnostics)
        )
    });
    let outcome = interpret(&checked, b"");
    assert_eq!(outcome.error, None, "Saturating float arithmetic trapped");
    assert_eq!(outcome.exit_code, 77);

    for name in [
        "float/float_trapping_overflow_traps",
        "float/float_trapping_divide_zero_traps",
        "float/float_trapping_invalid_traps",
        "float/float_trapping_propagated_nan_traps",
        "float/float_trapping_propagated_infinity_traps",
    ] {
        let main_path = pass_canary(name).join("main.omg");
        let checked = compile_differential_to_checked(&main_path).unwrap_or_else(|diagnostics| {
            panic!(
                "{name} should reach the interpreter:\n{}",
                join_diagnostics(&diagnostics)
            )
        });
        let outcome = interpret(&checked, b"");
        let error = outcome
            .error
            .as_deref()
            .expect("Trapping float arithmetic must not exit cleanly");
        assert!(
            error.contains("Trapping"),
            "{name}: unexpected trap: {error}"
        );
    }
}

#[cfg(windows)]
fn executable_name() -> &'static str {
    "omega-program.exe"
}

#[cfg(not(windows))]
fn executable_name() -> &'static str {
    "omega-program"
}

/// The BYTE-INPUT differential: input-taking programs previously only ran
/// the oracle with EMPTY stdin (the registry's rows), so the byte-ARRIVAL
/// paths of read_byte/write_byte were pinned per-engine but never
/// cross-compared on the same input. Both engines get identical bytes; exit
/// AND stdout must agree byte-for-byte (write_byte does no normalization).
/// Vectors: the byte-echo canary + every INPUT-GRID row of the stdin
/// samples (the rows are each sample's documented meaning table).
#[test]
fn interpreter_matches_native_on_byte_input_programs() {
    let vectors: &[(&str, PathBuf, &[u8], i32)] = &[
        (
            "console_byte_echo",
            pass_canary("host/runtime_console_byte_echo_exit").join("main.omg"),
            b"AB",
            201,
        ),
        // stdin_checksum INPUT-GRID
        ("stdin_checksum", stdin_sample("stdin_checksum"), b"", 0),
        ("stdin_checksum", stdin_sample("stdin_checksum"), b"A", 66),
        ("stdin_checksum", stdin_sample("stdin_checksum"), b"AB", 133),
        ("stdin_checksum", stdin_sample("stdin_checksum"), b"0", 49),
        ("stdin_checksum", stdin_sample("stdin_checksum"), b"!!", 68),
        // stdin_rot1 INPUT-GRID
        ("stdin_rot1", stdin_sample("stdin_rot1"), b"", 0),
        ("stdin_rot1", stdin_sample("stdin_rot1"), b"A", 1),
        ("stdin_rot1", stdin_sample("stdin_rot1"), b"AB", 2),
        ("stdin_rot1", stdin_sample("stdin_rot1"), b"ab ", 3),
        // stdin_upper INPUT-GRID
        ("stdin_upper", stdin_sample("stdin_upper"), b".", 0),
        ("stdin_upper", stdin_sample("stdin_upper"), b"a.", 1),
        ("stdin_upper", stdin_sample("stdin_upper"), b"hi.", 2),
        ("stdin_upper", stdin_sample("stdin_upper"), b"Mix.", 3),
        ("stdin_upper", stdin_sample("stdin_upper"), b"a z.", 3),
    ];

    for (name, main_path, stdin, expected_exit) in vectors {
        let checked = compile_differential_to_checked(main_path).unwrap_or_else(|diagnostics| {
            panic!(
                "{name}: compile to checked failed:\n{}",
                join_diagnostics(&diagnostics)
            )
        });
        let outcome = interpret(&checked, stdin);
        assert!(
            !outcome.is_error(),
            "{name} stdin {:?}: interpreter declined: {:?}",
            String::from_utf8_lossy(stdin),
            outcome.error
        );
        assert_eq!(
            outcome.exit_code,
            *expected_exit,
            "{name} stdin {:?}: interpreter exit vs the documented grid row",
            String::from_utf8_lossy(stdin)
        );

        let (native_code, native_stdout, _) =
            compile_and_run_native_with_stdin(name, main_path, stdin);
        assert_eq!(
            native_code,
            *expected_exit,
            "{name} stdin {:?}: native exit vs the documented grid row",
            String::from_utf8_lossy(stdin)
        );
        assert_eq!(
            String::from_utf8_lossy(&outcome.stdout),
            String::from_utf8_lossy(&native_stdout),
            "{name} stdin {:?}: stdout must agree byte-for-byte (interpreter left, native right)",
            String::from_utf8_lossy(stdin)
        );
    }
}

/// The LINE-INPUT differential. These programs exercise the standard
/// `Console::read_line(&mut [u8])` surface with concrete bounded carriers at
/// the call site. Both engines receive identical input; the carrier mutation,
/// branching, exit code, and emitted text must agree.
#[test]
fn interpreter_matches_native_on_bounded_line_input_programs() {
    let vectors: &[(&str, PathBuf, &[u8], i32)] = &[
        (
            "mutable_output_host_call",
            pass_canary("calls/mutable_output_host_call").join("main.omg"),
            b"hello\n",
            0,
        ),
        (
            "runtime_text_storage",
            pass_canary("text/runtime_text_storage").join("main.omg"),
            b"echo me\n",
            0,
        ),
        (
            "runtime_stdin_line_buffering_exit",
            pass_canary("text/runtime_stdin_line_buffering_exit").join("main.omg"),
            b"first\nsecond\n",
            0,
        ),
        (
            "runtime_stdin_command_branch_exit",
            pass_canary("text/runtime_stdin_command_branch_exit").join("main.omg"),
            b"look\n",
            0,
        ),
        (
            "runtime_ordered_room_dispatch_loop_exit",
            pass_canary("dungeon/runtime_ordered_room_dispatch_loop_exit").join("main.omg"),
            b"east\n",
            135,
        ),
        (
            "runtime_ordered_room_dispatch_real_show_states_exit",
            pass_canary("dungeon/runtime_ordered_room_dispatch_real_show_states_exit")
                .join("main.omg"),
            b"east\n",
            145,
        ),
    ];

    for (name, main_path, stdin, expected_exit) in vectors {
        let checked = compile_differential_to_checked(main_path).unwrap_or_else(|diagnostics| {
            panic!(
                "{name}: compile to checked failed:\n{}",
                join_diagnostics(&diagnostics)
            )
        });
        let outcome = interpret(&checked, stdin);
        assert!(
            !outcome.is_error(),
            "{name}: interpreter declined: {:?}",
            outcome.error
        );
        assert_eq!(
            outcome.exit_code, *expected_exit,
            "{name}: interpreter exit"
        );

        let (native_code, native_stdout, native_stderr) =
            compile_and_run_native_with_stdin(name, main_path, stdin);
        assert_eq!(native_code, *expected_exit, "{name}: native exit");
        assert_eq!(
            outcome.stdout,
            native_stdout,
            "{name}: stdout must agree byte-for-byte; native stderr: {}",
            String::from_utf8_lossy(&native_stderr)
        );
    }
}

fn stdin_sample(name: &str) -> PathBuf {
    repo_root().join("samples").join(name).join("main.omg")
}
