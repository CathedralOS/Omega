use build_declarations::{BuildDeclaration, extract_build_declaration};
use compiler::{
    ArtifactEmissionPolicy, CheckedCompilation, CompileOptions as CompilerOptions, CompileReport,
    CompileRequest, RequestedCompileProduct, compile_to_checked as compile_standalone_to_checked,
    compile_to_checked_with_packages,
};
use package_compilation::{
    AcceptedSemanticBinding, AcceptedSemanticBindingRole, PackageCompilationInputs,
    PackageDependencyBinding, PackageSourceBinding,
};
use semantic_vocabulary::PackageKeyIdentity;
use std::path::PathBuf;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum CanaryCompileProduct {
    Check,
    NativeArtifactAndPublish,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct CanaryCompileSpec {
    root_path: PathBuf,
    build_dir: Option<PathBuf>,
    target_name: Option<String>,
    product: CanaryCompileProduct,
}

impl CanaryCompileSpec {
    fn into_request_parts(self) -> (CompilerOptions, CanaryCompileProduct) {
        (
            CompilerOptions {
                root_path: self.root_path,
                build_dir: self.build_dir,
                target_name: self.target_name,
            },
            self.product,
        )
    }
}

fn production_compile(
    spec: CanaryCompileSpec,
) -> Result<CompileReport, Vec<diagnostics::Diagnostic>> {
    let (options, product) = spec.into_request_parts();
    let build_dir = options.build_dir();
    let requested_product = match product {
        CanaryCompileProduct::Check => RequestedCompileProduct::Check,
        CanaryCompileProduct::NativeArtifactAndPublish => RequestedCompileProduct::NativeArtifact,
    };
    let package_inputs = reviewed_repository_fixture_package_inputs(
        &options.root_path,
        options.target_name.as_deref(),
    )?;
    let mut request = CompileRequest::new(options).with_requested_product(requested_product);
    if let Some(package_inputs) = package_inputs {
        let permission_policy = native_realization::terminal_authority_permission_policy_with_rows(
            package_inputs
                .accepted_semantic_bindings()
                .flat_map(|binding| binding.terminal_authority_permissions())
                .cloned()
                .collect(),
        )
        .map_err(|error| {
            vec![Diagnostic::error(format!(
                "cannot construct repository fixture terminal-authority policy: {error:?}"
            ))]
        })?;
        request = request.with_terminal_authority_permission_policy(permission_policy);
        request = request.with_package_inputs(package_inputs);
    }
    let report = compiler::compile(request)?;
    match product {
        CanaryCompileProduct::Check => Ok(report),
        CanaryCompileProduct::NativeArtifactAndPublish => report
            .publish_retained_native_artifact(&build_dir)
            .map_err(|error| vec![diagnostics::Diagnostic::error(error)]),
    }
}

fn compile_with_artifact_policy(
    spec: CanaryCompileSpec,
    artifact_policy: ArtifactEmissionPolicy,
) -> Result<CompileReport, Vec<Diagnostic>> {
    let (options, product) = spec.into_request_parts();
    let build_dir = options.build_dir();
    let requested_product = match product {
        CanaryCompileProduct::Check => RequestedCompileProduct::Check,
        CanaryCompileProduct::NativeArtifactAndPublish => RequestedCompileProduct::NativeArtifact,
    };
    let package_inputs = reviewed_repository_fixture_package_inputs(
        &options.root_path,
        options.target_name.as_deref(),
    )?;
    let mut request = CompileRequest::new(options)
        .with_requested_product(requested_product)
        .with_artifact_policy(artifact_policy);
    if let Some(package_inputs) = package_inputs {
        let permission_policy = native_realization::terminal_authority_permission_policy_with_rows(
            package_inputs
                .accepted_semantic_bindings()
                .flat_map(|binding| binding.terminal_authority_permissions())
                .cloned()
                .collect(),
        )
        .map_err(|error| {
            vec![Diagnostic::error(format!(
                "cannot construct repository fixture terminal-authority policy: {error:?}"
            ))]
        })?;
        request = request.with_terminal_authority_permission_policy(permission_policy);
        request = request.with_package_inputs(package_inputs);
    }
    let report = compiler::compile(request)?;
    match product {
        CanaryCompileProduct::Check => Ok(report),
        CanaryCompileProduct::NativeArtifactAndPublish => report
            .publish_retained_native_artifact(&build_dir)
            .map_err(|error| vec![Diagnostic::error(error)]),
    }
}

fn compile(spec: CanaryCompileSpec) -> Result<CompileReport, Vec<Diagnostic>> {
    compile_with_artifact_policy(spec, ArtifactEmissionPolicy::OutputOnly)
}

/// Compile a canary that explicitly asserts an auxiliary compiler artifact.
/// Disposable native/runtime canaries must use [`compile`] so their temporary
/// build directories contain only the certified executable they consume.
fn compile_with_auxiliary_artifacts(
    spec: CanaryCompileSpec,
) -> Result<CompileReport, Vec<Diagnostic>> {
    compile_with_artifact_policy(spec, ArtifactEmissionPolicy::Full)
}

use checked_interpreter::{
    FilesystemServiceBinding, InterpretOptions, InterpretOutcome, interpret_entry,
    interpret_entry_with_options,
};
use diagnostics::Diagnostic;
use language_semantics::content::{
    ContentAlgebraIdentity, ContentArithmeticOperator, ContentConservationOwnerKind,
    ContentConservationTerm, ContentPlaceRoot, ContentPlaceVersion, ContentProjectionExpression,
    ContentScalarExpression,
};
use std::fs;
#[cfg(not(windows))]
use std::io::Write;
#[cfg(windows)]
use std::io::Write;
use std::path::Path;
#[cfg(not(windows))]
use std::process::Command;
#[cfg(windows)]
use std::process::Command;
#[cfg(not(windows))]
use std::process::Stdio;
#[cfg(windows)]
use std::process::Stdio;
use std::sync::Mutex;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::mpsc;
use std::thread;

fn interpret(checked: &CheckedCompilation, stdin: &[u8]) -> InterpretOutcome {
    // These checked-only semantic fixtures predate target-owned build roots.
    // Their temporary execution choice is explicit in the harness rather than
    // inferred by CheckedCompilation.
    let Some(filesystem) =
        checked.resolved_semantic_binding(AcceptedSemanticBindingRole::FilesystemHostService)
    else {
        return interpret_entry(checked, "Main::main", stdin);
    };
    let binding = FilesystemServiceBinding::from_compiler_resolved_declaration(
        checked,
        filesystem.declaration_symbol(),
    )
    .expect("accepted filesystem fixture binding resolves one exact declaration");
    interpret_entry_with_options(
        checked,
        "Main::main",
        stdin,
        InterpretOptions::default().with_filesystem_service_binding(binding),
    )
}

#[path = "fixture_rosters/canary_suite.rs"]
mod fixture_roster;

#[path = "canary_suite/inline_asm.rs"]
mod inline_asm;
#[path = "canary_suite/relational_invariants.rs"]
mod relational_invariants;
#[path = "canary_suite/task_runtime.rs"]
mod task_runtime;

#[path = "canary_suite/abi_runtime_values_and_strings.rs"]
mod abi_runtime_values_and_strings;
#[path = "canary_suite/artifact_footprints.rs"]
mod artifact_footprints;
#[path = "canary_suite/content_text_and_carriers.rs"]
mod content_text_and_carriers;
#[path = "canary_suite/domains_control_and_structures.rs"]
mod domains_control_and_structures;
#[path = "canary_suite/exact_native_coverage.rs"]
mod exact_native_coverage;
#[path = "canary_suite/float_plans_and_policies.rs"]
mod float_plans_and_policies;
#[path = "canary_suite/roster.rs"]
mod roster;
use float_plans_and_policies::retained_float_differential_result_identity;
#[path = "canary_suite/arithmetic_and_data.rs"]
mod arithmetic_and_data;
#[path = "canary_suite/generics_and_dependent_facts.rs"]
mod generics_and_dependent_facts;
#[path = "canary_suite/host_text_filesystem_and_abi.rs"]
mod host_text_filesystem_and_abi;
#[path = "canary_suite/portable_terminal_reload.rs"]
mod portable_terminal_reload;
#[path = "canary_suite/providers_float_and_console.rs"]
mod providers_float_and_console;
#[path = "canary_suite/ranges_storage_and_entries.rs"]
mod ranges_storage_and_entries;
#[path = "canary_suite/recursion_slices_and_conversions.rs"]
mod recursion_slices_and_conversions;
#[path = "canary_suite/reports_and_capabilities.rs"]
mod reports_and_capabilities;
#[path = "canary_suite/structural_selected_operator.rs"]
mod structural_selected_operator;
#[path = "canary_suite/surface_and_targets.rs"]
mod surface_and_targets;
#[path = "canary_suite/time_hosts_and_indexed_storage.rs"]
mod time_hosts_and_indexed_storage;
#[path = "canary_suite/value_and_type_checks.rs"]
mod value_and_type_checks;
#[path = "canary_suite/value_calls_and_dispatch.rs"]
mod value_calls_and_dispatch;
#[path = "canary_suite/wire_and_algorithms.rs"]
mod wire_and_algorithms;

/// Pass canaries intentionally confined to a WINDOWS host because their
/// authored implementation or platform behavior has no other-target lowering.
/// Compiled by `pass_canaries_compile` on windows hosts only; their
/// `_canary_runs` twins are `#[cfg(windows)]`-gated the same way.
#[cfg_attr(not(windows), allow(dead_code))]
const WINDOWS_HOST_PASS_CANARIES: &[&str] = &[
    // Session slice 2's positioned-io contract canary: the windows_x64 impl
    // COMPOSES save-cursor/seek/op/restore over msvcrt rows, but the canary's
    // darwin lowering hits the pwrite simple-arg host-call fence (the
    // computed-offset arg only the msvcrt composition accepts) -- a permanent
    // darwin compile red until windows-gated (2026-07-20; found by the macOS
    // battery the same day the slice landed).
    "filesystem/windows_positioned_io_exit",
];

/// Canaries compiled with an EXPLICIT cross target on EVERY host. Most are
/// `uefi_x64` because the efi family's image facts are target-shaped
/// (PE32+/subsystem 10 from build.omg); target-specific instruction canaries
/// use the smallest registered architecture target that proves their gate.
const CROSS_TARGET_PASS_CANARIES: &[(&str, &str)] = &[
    ("build/explicit_program_entry_binding", "windows_x86_64"),
    ("build/receiver_bound_program_entry", "windows_x86_64"),
    (
        "build/static_machine_parameter_config_compile",
        "windows_x86_64",
    ),
    ("build/uefi_program_entry_storage_roots", "uefi_x86_64"),
    ("inline_asm/asm_fences_compile", "linux_x86_64"),
    ("inline_asm/asm_interrupt_control_compile", "linux_x86_64"),
    ("inline_asm/asm_flags_compile", "linux_x86_64"),
    ("inline_asm/asm_msr_compile", "linux_x86_64"),
    ("inline_asm/asm_control_registers_compile", "linux_x86_64"),
    (
        "inline_asm/asm_multi_instruction_block_compile",
        "uefi_x86_64",
    ),
    ("inline_asm/asm_where_exact_clobbers_compile", "uefi_x86_64"),
    ("targets/efi_vtable_call", "uefi_x86_64"),
    ("targets/efi_vtable_field_call", "uefi_x86_64"),
    ("targets/efi_out_param_call", "uefi_x86_64"),
    ("targets/efi_ref_param_call_arg", "uefi_x86_64"),
    ("targets/efi_small_aggregate_entry", "uefi_x86_64"),
    ("targets/efi_large_result_entry", "uefi_x86_64"),
    ("targets/efi_large_aggregate_entry", "uefi_x86_64"),
    ("targets/efi_large_aggregate_stack_entry", "uefi_x86_64"),
    ("targets/aarch64_hfa_entry_argument", "linux_arm64"),
    ("targets/aarch64_small_aggregate_entry", "linux_arm64"),
    ("targets/aarch64_small_aggregate_stack_entry", "linux_arm64"),
    ("targets/aarch64_large_aggregate_entry", "linux_arm64"),
    ("targets/aarch64_large_aggregate_stack_entry", "linux_arm64"),
    ("targets/aarch64_wide_aggregate_entry", "linux_arm64"),
    ("targets/aarch64_small_result_entry", "linux_arm64"),
    ("targets/aarch64_hfa_result_entry", "linux_arm64"),
    ("targets/aarch64_large_result_entry", "linux_arm64"),
    ("targets/sysv_small_aggregate_entry", "linux_x86_64"),
    ("targets/sysv_erased_small_aggregate_entry", "linux_x86_64"),
    ("targets/sysv_hfa_entry_argument", "linux_x86_64"),
    ("targets/sysv_mixed_aggregate_entry", "linux_x86_64"),
    ("targets/sysv_mixed_aggregate_stack_entry", "linux_x86_64"),
    ("targets/sysv_small_aggregate_stack_entry", "linux_x86_64"),
    ("targets/sysv_large_aggregate_entry", "linux_x86_64"),
    ("targets/sysv_wide_aggregate_entry", "linux_x86_64"),
    ("targets/sysv_large_hfa_result_entry", "linux_x86_64"),
    ("targets/sysv_small_result_entry", "linux_x86_64"),
    ("targets/sysv_hfa_result_entry", "linux_x86_64"),
    ("targets/sysv_mixed_result_entry", "linux_x86_64"),
    ("targets/sysv_wrapped_float_entry", "linux_x86_64"),
];

const CROSS_TARGET_FAIL_CANARIES: &[(&str, &str)] = &[
    ("build/duplicate_program_entry_binding", "windows_x86_64"),
    (
        "build/hosted_program_entry_visible_parameter",
        "windows_x86_64",
    ),
    ("build/program_entry_receiver_not_zii", "windows_x86_64"),
    ("build/program_entry_returns_value", "windows_x86_64"),
    ("build/unknown_program_entry_binding", "windows_x86_64"),
    (
        "build/uefi_program_entry_missing_storage_roots",
        "uefi_x86_64",
    ),
    ("build/uefi_program_entry_unqualified_image", "uefi_x86_64"),
    (
        "build/uefi_program_entry_local_physical_contract",
        "uefi_x86_64",
    ),
    (
        "build/uefi_program_entry_wrong_calling_policy",
        "uefi_x86_64",
    ),
    (
        "collections/deep_nested_runtime_indexed_write_rejected",
        "linux_x86_64",
    ),
    ("host/terminal_host_call_value", "linux_x86_64"),
    ("calls/machine_self_call_recursion_rejected", "linux_x86_64"),
    ("calls/guard_call_vs_call_rejected", "linux_x86_64"),
    ("calls/guarded_value_call_terminal_rejected", "linux_x86_64"),
    ("traits/runtime_dyn_varying_field_rejected", "linux_x86_64"),
];

/// Pure checked-semantics canaries. These deliberately do not enter native
/// lowering and therefore do not require a deployable `ProgramEntry` binding.
const CHECKED_ONLY_PASS_CANARIES: &[&str] = &[
    "constraints/mutable_scalar_value_reads",
    "constraints/state_argument_value_snapshots",
    "constraints/guarded_integer_return_landing",
    "constraints/guarded_callee_result_bounds",
    "constraints/guarded_parameter_result_contract",
    "arithmetic/bounded_arithmetic_return",
    "arithmetic/bounded_assignment",
    "arithmetic/bounded_literal_named_constraints",
    "arithmetic/bounded_member_guard_transition",
    "arithmetic/bounded_transition",
    "arithmetic/bounded_wrapping_literal",
    "domains/call_requires_dynamic_indexed_boolean_expression_preserved_across_disjoint_mutating_call",
    "domains/exit_ensures_dynamic_indexed_boolean_expression_preserved_across_disjoint_mutating_call",
    "dependent/cross_machine_field_equality_compile",
    "ownership/call_arg_move_in_struct_literal",
    "ownership/transition_value_owned_move",
    "traits/boundary_policy_and_service_parents",
    "traits/generic_trait_parent_binds_requirement",
    "traits/trait_header_parent_composition",
    "constraints/computed_storage_value_transport",
    "constraints/computed_local_value_transport",
    "constraints/anonymous_integer_local_landing",
    "constraints/anonymous_integer_return_landing",
    "constraints/selected_arithmetic_return_ensures",
    "control_flow/explicit_state_value_frontier",
    "control_flow/transition_operand_schedule",
    "control_flow/nested_parameter_receiver_call",
    "arithmetic/bounded_max_call",
    "arithmetic/float_unit_ratio_compile",
    "arithmetic/bounded_return_literal",
    "arithmetic/exact_integer_cast_proven",
    "arithmetic/narrowing_flow_and_widen_permitted",
    "core/array_core_surface",
    "core/atomic_outcomes_core_surface",
    "core/collections_core_surface",
    "core/placement_vocabulary_core_surface",
    "core/slice_core_surface",
    "core/vec_core_surface",
    "operators/slice_index_via_spelling_compile",
    "parser/deep_nesting_within_limit",
    "parser/invariant_is_an_identifier",
    "terminal_psi/content_custody_exit",
    "slices/guarded_slice_parameter_empty_false_index_compile",
    "slices/guarded_slice_parameter_empty_false_tail_compile",
    "slices/guarded_slice_parameter_bounded_subslice_compile",
    "slices/guarded_slice_parameter_end_subslice_compile",
    "slices/guarded_slice_parameter_end_equals_len_subslice_compile",
    "slices/guarded_slice_parameter_index_compile",
    "slices/guarded_slice_parameter_min_length_index_compile",
    "slices/guarded_slice_parameter_min_length_tail_compile",
    "slices/guarded_slice_parameter_nonempty_index_compile",
    "slices/guarded_slice_parameter_nonempty_tail_compile",
    "slices/guarded_slice_parameter_nonzero_index_compile",
    "slices/guarded_slice_parameter_nonzero_tail_compile",
    "slices/guarded_slice_parameter_start_equals_len_subslice_compile",
    "slices/guarded_slice_parameter_subslice_compile",
    "slices/guarded_slice_parameter_symmetric_false_guard_compile",
    "slices/guarded_slice_parameter_symmetric_true_guard_compile",
    "slices/guarded_slice_parameter_successor_index_compile",
    "slices/guarded_slice_parameter_successor_tail_compile",
    "slices/requires_field_count_alias_index_compile",
    "slices/requires_slice_parameter_bounded_subslice_compile",
    "slices/requires_slice_parameter_index_compile",
    "slices/requires_slice_parameter_successor_index_compile",
    "slices/slice_local_index_fact_compile",
    "slices/subslice_folded_bound_facts_compile",
    "slices/subslice_literal_bounds_compile",
    "slices/inclusive_subslice_literal_bounds_compile",
    "slices/inclusive_subslice_end_equals_len_minus_one_compile",
    "slices/full_range_subslice_compile",
    "slices/subslice_local_bound_facts_compile",
    "slices/subslice_range_surface_compile",
    "slices/window_shrink_exact_length_index_compile",
    "slices/window_shrink_unknown_base_index_compile",
    "slices/window_shrink_min_length_tail_index_compile",
    "slices/window_literal_bounds_min_length_parent_index_compile",
    "slices/window_subslice_within_exact_length_compile",
    "slices/disjoint_mut_subslice_windows_compile",
    "slices/termination_slice_length_compile",
    "slices/termination_slice_len_distance_compile",
    "dependent/data_where_membership_literal_compile",
    "dependent/data_where_membership_window_restored_compile",
    "dependent/data_where_membership_zero_valid_compile",
    "dependent/data_where_length_window_compile",
    "dependent/data_where_length_zero_valid_compile",
    "dependent/data_where_symbolic_equal_window_compile",
    "dependent/data_where_symbolic_affine_window_compile",
    "dependent/data_where_commutative_correlation_compile",
    "dependent/call_frame_preserves_disjoint_fact_compile",
    "dependent/transitive_call_frame_preserves_disjoint_fact_compile",
    "dependent/transition_call_frame_preserves_disjoint_fact_compile",
    "dependent/named_self_transition_call_frame_preserves_disjoint_fact_compile",
    "dependent/multi_state_cycle_call_frame_preserves_disjoint_fact_compile",
    "dependent/transition_parameter_frame_preserves_disjoint_fact_compile",
    "dependent/call_bearing_transition_frame_preserves_disjoint_fact_compile",
    "dependent/data_where_call_frame_preserves_disjoint_valuation_compile",
    "dependent/proof_call_frame_preserves_dependent_forward_compile",
    "dependent/proof_value_call_frame_preserves_dependent_forward_compile",
    "dependent/state_arrival_contract_guarded_compile",
    "dependent/loop_invariant_survives_disjoint_sibling_call_compile",
    "dependent/relational_loop_invariant_dynamic_length_compile",
    "dependent/relational_loop_invariant_stable_limit_compile",
    "dependent/relational_loop_invariant_mixed_strictness_compile",
    "dependent/relational_loop_invariant_stable_bound_chain_compile",
    "dependent/range_gated_machine_establishment_compile",
    "dependent/data_where_witness_write_after_borrow_compile",
    "dependent/data_where_capacity_measure_compile",
    "dependent/data_where_field_read_during_window_compile",
    "collections/std_option_storage_write",
    "collections/std_option_surface",
    "core/float_format_core_surface",
    "core/fixed_vec_core_surface",
    "core/arena_core_surface",
    "core/extent_core_surface",
    "core/interrupt_obligations_surface",
    "core/int_core_surface",
    "core/nat_core_surface",
    "core/ptr_core_surface",
    "core/local_value_intro_compile",
    "core/self_read_only_receiver_compile",
    "calls/effectless_mut_out_param_discard_compile",
    "calls/pure_discard_warns_compile",
    "calls/typed_return_from_local_call_compile",
    "expressions/float_literal_suffix",
    "expressions/integer_literal_suffix",
    "domains/call_requires_boundary_trait_satisfied_by_caller_requires",
    "domains/call_requires_boundary_satisfied_by_caller_requires",
    "domains/slice_carrier_domain",
    "domains/slice_domain_validator",
    "domains/utf8_slice_ops",
    "domains/utf8_literal_arg",
    "proofs/real_boundary_package_compile",
    "proofs/integer_measured_nat_induction_compile",
    "proofs/citation_requires_discharged",
    "proofs/polynomial_expand_core_nat",
    "proofs/proof_inductive_gauss_sum",
    "proofs/proof_inductive_climbing_sum",
    "proofs/proof_nat_structural_lemmas",
    "proofs/recursive_machine_with_requires_compiles",
    "drops/cleanup_machine_drop_shape",
    "drops/drop_ensures_unlocked_predicate",
    "drops/machine_effects_annotation",
    "drops/transfer_cleanup_into_state",
    "errors/fallible_result_data_shape",
    "errors/host_failure_boundary_machine",
    "errors/trap_unrecoverable_statement",
    "dependent/data_where_surface_compile",
    "dependent/data_where_gated_construction_compile",
    "dependent/data_where_length_construction_compile",
    "dependent/data_where_symbolic_equal_construction_compile",
    "dependent/data_where_invariant_window_restored_exit",
    "dependent/range_sugar_gated_construction_compile",
    "dependent/nested_gated_construction_compile",
    "dependent/zero_case_absorbs_nested_gate_compile",
    "dependent/nested_data_where_window_restored_exit",
    "dependent/indexed_data_where_window_restored_exit",
    "dependent/data_where_flow_proven_construction_compile",
    "dependent/data_where_zero_satisfying",
    "dependent/data_where_cross_state_establish",
    "dependent/data_where_cross_state_valuation",
    "dependent/data_where_param_write_proves",
    "dependent/data_where_hypothesis_discharges",
    "dependent/data_where_window_closes",
    "dependent/data_where_ranged_param_constructs",
    "dependent/data_where_product_hypothesis",
    "dependent/data_where_callee_establishes",
    "dependent/data_where_multistate_callee",
    "dependent/data_where_chained_hypothesis",
    "dependent/data_where_window_transport",
    "dependent/data_where_gated_literal_proves",
    "domains/if_domain_membership_check",
    "domains/match_domain_patterns",
    "domains/match_interleaved_domain_data_guard",
    "domains/domain_operator_spelling_selected",
    "domains/domain_operator_proven_fact_selects_meaning",
    "domains/domain_operator_unproven_keeps_builtin_meaning",
    "domains/domain_operator_inactive_same_carrier_coexists",
    "domains/domain_operator_requires_discharged",
    "float/named_float_to_integer_no_context_compile",
    "capabilities/boundary_trait_multiple_effects",
    "capabilities/declared_synchronous_invocation",
    "capabilities/invariant_parameterized_slice",
    "capabilities/string_domain_boundary_requirement",
    "capabilities/transitive_effect_inference",
    "operators/parenthesized_precedence_value",
    "operators/unary_logical_not",
    "capabilities/uses_caller_folder",
    "capabilities/uses_caller_capability_requires",
    "core/float_meaning_core_surface",
    "data/case_payload_declaration",
    "data/match_default_satisfies_exhaustiveness",
    "data/payload_less_case_equality",
    "data/property_carry_declared",
    "domains/transparent_alias_expansion",
    "domains/domain_import_valid",
    "domains/explicit_domain_erasure",
    "domains/call_requires_preserved_across_imported_disjoint_mutation",
    "domains/call_requires_preserved_across_disjoint_mutation",
    "domains/call_requires_satisfied_by_caller_requires",
    "domains/call_requires_free_machine_satisfied_by_caller_requires",
    "domains/call_requires_boolean_expression_from_domain_fact",
    "domains/domain_param_membership_satisfied",
    "domains/domain_param_forwarded",
    "domains/call_requires_boolean_expression_preserved_across_disjoint_mutating_call",
    "domains/call_requires_dynamic_indexed_scalar_member_expression_from_domain_fact",
    "domains/call_requires_fixed_indexed_boolean_expression_preserved_across_disjoint_mutating_call",
    "domains/call_requires_fixed_indexed_scalar_member_expression_from_domain_fact",
    "domains/call_requires_scalar_member_expression_from_domain_fact",
    "domains/call_requires_boolean_union_expression_from_domain_fact",
    "domains/call_requires_domain_intersection_preserved",
    "domains/call_requires_domain_union_left_branch_preserved",
    "domains/call_requires_domain_union_right_branch_preserved",
    "domains/call_requires_domain_membership_preserved_across_disjoint_dynamic_field_mutation",
    "domains/call_requires_domain_membership_preserved_across_disjoint_literal_element_mutation",
    "domains/exit_ensures_domain_union_left_branch_preserved",
    "domains/exit_ensures_domain_union_right_branch_preserved",
    "domains/exit_ensures_boolean_expression_from_domain_fact",
    "domains/exit_ensures_boolean_expression_preserved_across_disjoint_mutating_call",
    "domains/exit_ensures_boolean_union_expression_from_domain_fact",
    "domains/exit_ensures_fixed_indexed_boolean_expression_preserved_across_disjoint_mutating_call",
    "domains/exit_ensures_dynamic_indexed_scalar_member_expression_from_domain_fact",
    "domains/exit_ensures_fixed_indexed_scalar_member_expression_from_domain_fact",
    "domains/indexed_domain_requires_preserved_across_disjoint_field_mutation",
    "domains/exit_ensures_preserved_from_entry",
    "domains/local_alias_domain_transfer",
    "domains/user_authored_predicate_machine",
    "domains/vacuous_domain_qualification",
    "domains/semantic_cast_literal_mint",
    "domains/semantic_cast_range_mint",
    "domains/semantic_cast_guard_chain_mint",
    "domains/contracts_domain_membership_surface",
    "domains/domain_intersection_contract_surface",
    "domains/signature_free_requirement_route_compile",
    "domains/string_non_empty_fact",
    "domains/bodyless_internal_state_forwarding",
    "dependent/value_rebinding_cycle_call_frame_preserves_disjoint_fact_compile",
    "generics/const_data_param",
    "generics/const_machine_value_params",
    "generics/generic_data_instantiation",
    "generics/generic_data_type_param",
    "generics/generic_machine_call_monomorphization",
    "generics/generic_seq_consuming_map_filter",
    "generics/generic_machine_multiple_type_params",
    "generics/generic_machine_type_param_signature",
    "generics/generic_machine_where_trait_bound",
    "generics/generic_trait_type_param",
    "generics/generic_type_param_in_state",
    "generics/machine_bound_satisfied_at_call",
    "generics/nominal_machine_parameter_satisfaction_compile",
    "generics/property_bound_type_parameter",
    "borrow/borrow_carrying_field_reassignment",
    "borrow/aggregate_cast_unrelated_source_compile",
    "borrow/disjoint_field_owner_call_compile",
    "borrow/disjoint_mutable_slice_element_reborrow_compile",
    "borrow/disjoint_subslice_owner_write_compile",
    "borrow/explicit_aggregate_lifetime_selects_source",
    "borrow/multi_lifetime_result_field_sources",
    "borrow/nested_aggregate_result_unrelated_source_compile",
    "borrow/provider_owned_view_after_last_use",
    "borrow/provider_view_claim_invalidated_after_last_use",
    "borrow/whole_place_recast_disjoint_member_compile",
    "borrows/borrow_disjoint_fixed_index_call_mut",
    "borrows/borrow_disjoint_fixed_index_mut",
    "borrows/borrow_unique",
    "borrows/local_alias_boolean_transfer",
    "constraints/multi_fact_contract_without_separators",
    "constraints/proof_machine_order_fact",
    "constraints/nat_proof_literal_suffix",
    "constraints/contract_range_membership_unimplemented",
    "constraints/scalar_ensures_field_contract_surface",
    "constraints/scalar_requires_satisfied_by_literal",
    "modules/module_declaration",
    "modules/package_declaration",
    "modules/pub_visibility_modifier",
    "modules/use_imports_sibling_data",
    "modules/use_imports_sibling_trait",
    "operators/core_operator_declaration_surface",
    "operators/core_boundary_operator_surface",
    "operators/domain_operator_declaration_surface",
    "operators/domain_operator_overload_signature_compile",
    "operators/root_operator_overload_signature_compile",
    "ownership/linear_property_surface",
    "ownership/linear_branch_reconciliation",
    "ownership/linear_assignment_establishes",
    "ownership/owned_assignment_before_exit",
    "ownership/conditional_linear_sum",
    "ownership/conditional_linear_payload_extraction",
    "ownership/linear_returned_obligation",
    "ownership/linear_zero_storage_unestablished",
    "ownership/move_keyword_field_assignment",
    "ownership/compound_assign_add_field",
    "ownership/copy_value_field_read_compile",
    "core/task_core_linear_claim",
    "core/task_outcome_linear_payloads",
    "parameters/shared_and_mut_borrow_params_compile",
    "proofs/ring_law_conformance",
    "proofs/ring_rearrange_core_nat",
    "proofs/ring_full_polynomial_compile",
    "proofs/ring_identity_slot_bridge_compile",
    "proofs/rat_metric_compile",
    "proofs/signed_rat_metric_compile",
    "proofs/cauchy_predicates_compile",
    "proofs/nat_metric_triangle_compile",
    "proofs/proposition_relation_hierarchy_compile",
    "proofs/quotient_equivalence_compile",
    "proofs/quotient_generic_relation_compile",
    "proofs/quotient_machine_family_compile",
    "proofs/higher_order_machine_schema_compile",
    "proofs/machine_parameterized_data_compile",
    "proofs/named_witness_concrete_lane_compile",
    "proofs/named_witness_static_trait_call_compile",
    "proofs/named_witness_static_trait_i32_compile",
    "proofs/named_witness_static_trait_bool_compile",
    "proofs/named_witness_static_trait_plural_compile",
    "proofs/proof_constant_arithmetic_identity",
    "proofs/proof_bignum_constant_fold",
    "proofs/proof_order_transitivity",
    "proofs/proof_linear_range_sum",
    "proofs/proof_congruence_add_constant",
    "proofs/proof_addition_commutativity",
    "proofs/proof_nonlinear_square_range",
    "proofs/proof_order_antisymmetry",
    "proofs/proof_multiplication_distributivity",
    "proofs/proof_integer_embedding",
    "proofs/proof_remainder_range",
    "proofs/proof_bag_view_reflexivity",
    "traits/default_machine_in_trait",
    "traits/dyn_trait_object_dispatch",
    "traits/generic_trait_parameter",
    "traits/trait_composition_satisfies",
    "traits/trait_declaration_bundle",
    "traits/trait_inferred_satisfaction",
    "traits/trait_method_ensures_clause",
    "traits/trait_satisfies_machine_signature",
    "providers/service_fused_erasure_compile",
    "control_flow/termination_countdown_compile",
    "control_flow/entry_local_member_access_in_nested_state",
    "control_flow/entry_local_nested_member_access_in_nested_state",
    "termination/custom_ranking_order_compile",
    "termination/cyclic_bound_countdown_compile",
    "termination/increasing_to_rank_range_compile",
    "termination/inherited_acyclic_requirement_guarantee_compile",
    "termination/inherited_requirement_guarantee_compile",
    "termination/joint_lexicographic_machine_call_cycle_compile",
    "termination/mutual_recursion_countdown_compile",
    "termination/default_order_nat_countdown_compile",
    "termination/default_order_slice_length_compile",
    "termination/default_order_bounded_distance_compile",
    "termination/bounded_distance_named_view",
    "termination/increasing_cursor_bounded_view",
    "termination/increasing_cursor_rank_range",
    "termination/default_order_unsigned_width_countdown_compile",
    "termination/proof_non_tail_joint_machine_cycle_compile",
    "versioning/migration_generic_trait",
    "versioning/version_scoped_machine",
    "versioning/data_version_block",
    "versioning/migration_machine_from_v1",
    "versioning/versioned_match_all_eras_exhaustive",
    "versioning/versioned_match_default_arm",
];

const CHECKED_ONLY_FAIL_CANARIES: &[&str] = &[
    "types/addr_plus_addr_rejected",
    "types/isize_rejected",
    "types/usize_rejected",
    "data/proof_only_local_rejected",
    "data/proof_only_reference_view_rejected",
    "data/proof_only_runtime_property_rejected",
    "data/proof_only_state_param_rejected",
    "wire/proof_only_wire_field_rejected",
    "float/suffix_call_argument_disagrees_rejected",
    "float/named_argument_format_rejected",
    "traits/conformance_item_missing_member",
    "traits/conformance_item_unknown_trait",
    "constraints/mutable_scalar_value_invalidated",
    "constraints/state_argument_value_reread",
    "constraints/guarded_integer_return_range",
    "constraints/callee_result_bound_exceeds_cast",
    "constraints/parameter_result_argument_exceeds_requirement",
    "constraints/negative_divisor_argument_bound",
    "arithmetic/bounded_call_unproven",
    "constants/free_const_field_collision",
    "constants/free_const_local_collision",
    "traits/generic_trait_parent_binding_mismatch",
    "traits/trait_parent_generic_arity",
    "traits/trait_parent_unknown_argument",
    "traits/ordinary_trait_inherits_boundary",
    "constraints/computed_storage_value_range",
    "constraints/computed_local_value_invalidated",
    "constraints/anonymous_integer_local_range",
    "constraints/anonymous_integer_return_wrong_ensures",
    "constraints/transition_mutated_argument",
    "control_flow/implicit_entry_parameter_capture",
    "control_flow/implicit_entry_local_capture",
    "control_flow/implicit_entry_write_capture",
    "control_flow/implicit_entry_receiver_capture",
    "control_flow/implicit_entry_contract_capture",
    "constraints/selected_arithmetic_return_wrong_ensures",
    "core/atomic_outcome_key_parameter_rejected",
    "core/placed_construction_rejected",
    "core/placed_wrong_arity_rejected",
    "core/placement_custody_wrong_arity_rejected",
    "core/placement_outcome_wrong_arity_rejected",
    "core/placement_return_wrong_arity_rejected",
    "core/resident_index_identity_mismatch_rejected",
    "core/resident_wrong_arity_rejected",
    "capabilities/boundary_qualification_subject_rejected",
    "capabilities/direct_accepted_qualification_rejected",
    "providers/via_with_body_rejected",
    "providers/via_on_axiom_rejected",
    "providers/via_requires_satisfies",
    "providers/via_binding_must_be_qualified",
    "providers/via_repeated_effects_rejected",
    "providers/via_signature_mismatch_rejected",
    "providers/via_runtime_binding_rejected",
    "providers/via_unknown_binding_rejected",
    "providers/vtable_slot_retired",
    "providers/via_bare_field_binding_rejected",
    "providers/duplicate_external_leaf_rejected",
    "providers/free_adapter_rejected",
    "providers/service_missing_bound_rejected",
    "providers/service_nonboundary_requirement_rejected",
    "providers/service_bound_nonservice_rejected",
    "providers/service_authored_lookalike_not_privileged",
    "providers/service_borrowed_parameter_rejected",
    "providers/service_nested_carrier_rejected",
    "inline_asm/asm_pushfq_requires_u64_destination",
    "inline_asm/asm_popfq_requires_saved_place",
    "inline_asm/asm_rdmsr_requires_u64_destination",
    "inline_asm/asm_wrmsr_requires_u64_value",
    "inline_asm/asm_read_cr3_requires_u64_destination",
    "inline_asm/asm_write_cr3_requires_u64_value",
    "inline_asm/asm_port_out_wrong_port_type",
    "inline_asm/asm_port_out_wrong_value_type",
    "inline_asm/asm_port_in_wrong_destination_type",
    "inline_asm/asm_port_in_wrong_port_type",
    "inline_asm/asm_port_literal_out_of_range",
    "inline_asm/asm_where_missing_clobber",
    "inline_asm/asm_where_extra_clobber",
    "inline_asm/asm_where_contract",
    "inline_asm/asm_label_loop",
    "inline_asm/asm_structured_ldr_str",
    "inline_asm/asm_deriver_only_exit",
    "inline_asm/asm_lidt_deriver_only",
    "inline_asm/asm_hidden_return",
    "inline_asm/asm_service_import_required",
    "inline_asm/asm_machine_control_service_required",
    "inline_asm/asm_port_io_service_required",
    "inline_asm/asm_machine_control_transitive_service_required",
    "operators/duplicate_spelling_binding",
    "providers/checked_boundary_operator_missing_contract",
    "providers/checked_boundary_operator_parameter_swap",
    "providers/checked_boundary_operator_stronger_requires",
    "providers/provider_selection_outside_build",
    "providers/adapter_forwarding_bad_lead",
    "recast/recast_size_mismatch_rejected",
    "recast/recast_position_fenced",
    "recast/interior_recast_footprint_rejected",
    "recast/runtime_offset_footprint_rejected",
    "recast/record_view_footprint_rejected",
    "recast/symbolic_stride_footprint_rejected",
    "layouts/plan_laid_dynamic_plan",
    "layouts/plan_laid_policy_without_plan_machine",
    "providers/adapter_hidden_effect",
    "build/accept_boundary_outside_build",
    "build/program_entry_binding_outside_build",
    "capabilities/native_slice_external_leaf_rejected",
    "capabilities/native_bounded_text_external_leaf_rejected",
    "capabilities/native_vector_external_leaf_rejected",
    "targets/target_machine_missing_rejected",
    "targets/target_machine_duplicate_rejected",
    "collections/triple_runtime_indexed_read_rejected",
    "collections/nested_three_level_index_rejected",
    "tasks/task_runtime_machine_selection_effect_mismatch",
    "build/static_machine_parameter_contract_mismatch",
    "build/build_machine_wrong_arity",
    "build/build_effects_undeclared",
    "build/build_boundary_rowless",
    "build/build_service_name_spoof",
    "core/content_projection_foreign_owner",
    "core/content_projection_duplicate",
    "core/content_projection_legacy_interval",
    "core/content_projection_arbitrary_call",
    "core/content_projection_signed_embedding",
    "proofs/proof_integer_embedding_boolean",
    "proofs/proof_bignum_constant_false",
    "proofs/proof_integer_embedding_runtime",
    "core/content_conservation_unqualified_place",
    "core/content_conservation_entry_former_retired",
    "calls/library_block_retired",
    "capabilities/capability_entry_retired",
    "calls/explicit_machine_entry_retired",
    "calls/public_machine_entry_retired",
    "calls/trailing_boundary_host_retired",
    "calls/trailing_boundary_named_retired",
    "core/content_retained_custody_from_borrow",
    "core/extent_reconstruction_does_not_grant",
    "core/extent_no_wrap_lookalike",
    "core/extent_root_adapter_direct_call_does_not_grant",
    "core/program_storage_entry_ordinary_call_does_not_mint",
    "core/carry_permission_adapter_direct_call_does_not_grant",
    "core/task_parked_continuation_projection_rejected",
    "core/task_parked_continuation_recast_rejected",
    "core/task_parked_continuation_address_rejected",
    "core/task_parked_continuation_mutation_rejected",
    "core/extent_unqualified_construction_scope_loss",
    "core/extent_scope_loss",
    "core/interrupt_mask_guard_scope_loss",
    "core/interrupt_acknowledgement_scope_loss",
    "core/interrupt_obligation_construction_rejected",
    "core/interrupt_mask_guard_explicit_qualification_rejected",
    "core/interrupt_acknowledgement_double_complete",
    "constants/const_non_literal_initializer",
    "constants/const_free_floating_rejected",
    "constants/const_shadows_case",
    "comptime/effectful_const_array_length",
    "comptime/negative_const_array_length",
    "comptime/parameterized_const_array_length",
    "comptime/unknown_const_array_length",
    "comptime/const_array_length_index_out_of_bounds",
    "comptime/fuel_exhausted_const_array_length",
    "parse/machine_clause_garbage_rejected",
    "parse/relax_retired",
    "parser/nesting_exceeds_max_depth",
    "data/bare_payload_case_equality_guard",
    "data/bare_payload_case_equality_suggests_in",
    "data/case_payload_equality_interim",
    "data/case_payload_malformed",
    "data/field_default_retired",
    "data/property_zero_init_retired",
    "data/enum_keyword_retired",
    "data/match_nonexhaustive_cases",
    "data/match_predicate_domain_needs_default",
    "data/mixed_common_field_nonscalar",
    "data/mixed_payload_field_shadows_common",
    "data/mixed_record_literal",
    "data/property_copy_violation",
    "data/property_sized_declared",
    "data/property_unknown",
    "data/property_send_case_payload",
    "domains/domain_import_cycle",
    "domains/domain_import_unknown",
    "domains/domain_import_wrong_target",
    "domains/domain_alias_cycle",
    "domains/domain_alias_publishes_private_constituent",
    "domains/domain_alias_unknown",
    "domains/domain_alias_wrong_target",
    "domains/domain_alias_reports_atomic_requirement",
    "domains/domain_param_requires_membership",
    "domains/domain_field_write_raw_value",
    "domains/state_parameter_field_domain_write_unestablished",
    "domains/literal_violates_domain_fact",
    "domains/domain_field_read_no_write_unproven",
    "domains/call_requires_invalidated_by_mutation",
    "domains/call_requires_domain_intersection_invalidated_by_mutation",
    "domains/call_requires_boolean_expression_invalidated_by_mutating_call",
    "domains/call_requires_dynamic_indexed_boolean_expression_invalidated_by_mutating_call",
    "domains/call_requires_domain_union_unproven",
    "domains/call_requires_fixed_indexed_boolean_expression_invalidated_by_mutating_call",
    "domains/call_requires_scalar_member_expression_invalidated_by_same_index_mutation",
    "domains/exit_ensures_boolean_expression_invalidated_by_mutating_call",
    "domains/exit_ensures_dynamic_indexed_boolean_expression_invalidated_by_mutating_call",
    "domains/exit_ensures_fixed_indexed_boolean_expression_invalidated_by_mutating_call",
    "domains/call_requires_unproven",
    "domains/call_requires_free_machine_value_unproven",
    "domains/call_requires_free_machine_statement_unproven",
    "domains/call_requires_domain_membership_invalidated_by_same_literal_element_call",
    "domains/exit_ensures_domain_union_unproven",
    "domains/exit_ensures_unproven",
    "domains/indexed_domain_requires_invalidated_by_same_index_mutation",
    "domains/indexed_domain_requires_invalidated_by_unknown_index_mutation",
    "domains/domain_when_clause_retired",
    "domains/domain_non_boolean_fact",
    "domains/domain_carrier_mismatch",
    "domains/type_constraint_unknown_domain",
    "domains/domain_pattern_payload_binding_rejected",
    "domains/signature_free_requirement_route_overloaded",
    "generics/closed_indexed_array_element_mismatch",
    "generics/closed_indexed_domain_mismatch",
    "generics/closed_indexed_domain_noncanonical_rat",
    "generics/closed_indexed_domain_unknown_const",
    "generics/closed_indexed_domain_wrong_arity",
    "generics/closed_indexed_domain_wrong_type",
    "generics/closed_indexed_struct_field_mismatch",
    "generics/colon_bound_rejected",
    "generics/const_data_argument_out_of_range",
    "generics/const_data_argument_requires_value",
    "generics/const_data_expression_division_by_zero",
    "generics/const_data_expression_type_parameter",
    "generics/const_data_forwarded_type_mismatch",
    "generics/const_data_machine_call_requires_zero_arguments",
    "generics/const_data_named_value_out_of_range",
    "generics/const_data_symbolic_expression_unknown",
    "generics/const_data_where_domain_membership_false",
    "generics/const_data_where_fact_false",
    "generics/const_data_where_machine_fact_composed_false",
    "generics/const_data_where_machine_fact_false",
    "generics/const_data_where_machine_fact_nested_false",
    "generics/const_data_where_membership_carrier_mismatch",
    "generics/const_data_where_membership_false",
    "generics/const_data_where_mixed_fact_violated",
    "generics/generic_machine_where_machine_requirement",
    "generics/machine_bound_value_call_unchecked",
    "generics/machine_bound_violated_at_call",
    "generics/negative_const_data_argument_unsigned",
    "generics/non_integer_const_array_length",
    "generics/open_index_unestablished_equality",
    "generics/property_bound_missing_on_field",
    "generics/property_bound_violated_at_instantiation",
    "generics/signature_free_nominal_binder_overloaded",
    "generics/signed_const_data_argument_out_of_range",
    "generics/signed_const_data_shift_overflow",
    "generics/type_parameter_array_length",
    "generics/unresolved_symbolic_array_length",
    "borrow/borrow_carrying_field_reassignment_invalidated",
    "borrow/aggregate_cast_loan_invalidated",
    "borrow/borrow_carrying_local_transfer_invalidated",
    "borrow/carrier_view_invalidated_by_owner_write",
    "borrow/lifetime_argument_arity",
    "borrow/multi_lifetime_result_field_invalidated",
    "borrow/nested_aggregate_result_invalidated",
    "borrow/nested_borrow_carrying_local_escape",
    "borrow/persistent_borrow_storage_requires_outlives",
    "borrow/provider_owned_view_invalidated_by_receiver_call",
    "borrow/provider_view_claim_invalidated_while_live",
    "borrow/slice_view_invalidated_by_owner_call",
    "borrow/slice_view_invalidated_by_owner_write",
    "borrow/subslice_view_invalidated_by_owner_write",
    "borrow/undeclared_lifetime_argument",
    "borrow/undeclared_lifetime_tag",
    "borrow/vec_view_invalidated_by_push",
    "borrow/whole_place_recast_loan_invalidated",
    "borrows/borrow_duplicate_mut",
    "borrows/borrow_local_alias_active",
    "borrows/borrow_local_alias_reborrow_active",
    "borrows/borrow_mut_and_read",
    "borrows/borrow_mut_literal",
    "borrows/borrow_same_fixed_index_call_mut",
    "borrows/borrow_same_fixed_index_mut",
    "borrows/borrow_same_fixed_index_slice_alias_mut",
    "borrows/borrow_unknown_index_pair_mut",
    "constraints/finite_core_domain_on_int",
    "constraints/multiple_policy_domain_chain",
    "constraints/scalar_requires_unproven_literal",
    "data/recursive_data_infinite_size",
    "data/unknown_nested_field_read_rejected",
    "data/unknown_nested_intermediate_field_read_rejected",
    "data/unknown_nested_field_write_rejected",
    "data/unknown_nested_intermediate_field_write_rejected",
    "data/struct_literal_duplicate_field_rejected",
    "data/struct_literal_primitive_type_rejected",
    "data/struct_literal_unknown_type_rejected",
    "data/struct_literal_wrong_data_type_rejected",
    "data/nested_array_literal_inner_length_rejected",
    "data/array_scalar_shape_mismatch_rejected",
    "data/array_return_shape_mismatch_rejected",
    "data/construction_field_shape_mismatch_rejected",
    "control_flow/bare_machine_arrow_transition",
    "control_flow/bare_state_arrow_transition",
    "control_flow/nonplace_record_pattern_missing_field",
    "control_flow/nonplace_record_pattern_requires_copy",
    "control_flow/nonplace_record_pattern_unknown_field",
    "control_flow/termination_countdown_stalled_decrease",
    "control_flow/termination_cycle_missing_decreases",
    "control_flow/tuple_destructure_duplicate_binding_rejected",
    "control_flow/tuple_destructure_second_missing_field",
    "modules/ambiguous_imported_data",
    "modules/use_unresolved_path",
    "modules/boundary_signature_selects_private_data",
    "operators/domain_operator_alpha_equivalent_generic_duplicate",
    "operators/domain_operator_duplicate",
    "operators/domain_operator_reordered_generic_duplicate",
    "operators/domain_operator_return_only_overload",
    "operators/root_operator_alpha_equivalent_generic_duplicate",
    "operators/root_operator_reordered_generic_duplicate",
    "operators/root_operator_return_only_overload",
    "operators/root_operator_duplicate",
    "operators/named_operator_result_overload_duplicate_dispatch",
    "ownership/copy_linear_conflict",
    "ownership/linear_ambiguous_state_result_mapping",
    "ownership/linear_mixed_branch_treatment",
    "ownership/linear_live_overwrite",
    "ownership/linear_transparent_record_sibling_scope_loss",
    "ownership/linear_transparent_record_duplicate_move",
    "ownership/conditional_linear_live_scope_loss",
    "ownership/conditional_linear_zero_storage_not_established",
    "ownership/linear_scope_loss",
    "ownership/linear_second_transfer",
    "ownership/linear_zero_storage_not_established",
    "ownership/assign_immutable_parameter",
    "core/task_core_scope_loss",
    "core/task_outcome_linear_payload_scope_loss",
    "core/start_outcome_linear_arguments_scope_loss",
    "types/range_under_non_exact_domain_rejected",
    "proofs/cauchy_zero_precision_rejected",
    "proofs/float_meaning_zero_finite_rejected",
    "proofs/nat_metric_false_gap_zero",
    "proofs/rat_metric_false_reflexivity",
    "proofs/rat_zero_denominator_rejected",
    "proofs/conditional_ih_requires_discharge",
    "proofs/citation_later_fact_unavailable",
    "proofs/nat_lemma_citation_false_rejected",
    "proofs/uncited_structural_fact_rejected",
    "proofs/citation_requires_bearing_rejected",
    "proofs/ring_law_unproven_rejected",
    "proofs/ring_law_weaker_instance_rejected",
    "proofs/ring_law_slot_unbound_rejected",
    "proofs/ring_rearrange_unlicensed_rejected",
    "proofs/ring_rearrange_false_shuffle_rejected",
    "proofs/ring_full_polynomial_false_rejected",
    "proofs/ring_identity_slots_distinct_rejected",
    "proofs/signed_rat_zero_denominator_rejected",
    "proofs/quotient_affine_carrier_content_rejected",
    "proofs/quotient_routed_carrier_content_rejected",
    "proofs/quotient_missing_symmetry",
    "proofs/quotient_noncarrier_construction",
    "proofs/quotient_cross_family_construction",
    "proofs/quotient_boundary_law_rejected",
    "proofs/quotient_runtime_equality_rejected",
    "proofs/quotient_pattern_rejected",
    "proofs/quotient_struct_literal_rejected",
    "proofs/quotient_generic_struct_literal_rejected",
    "proofs/quotient_respect_lift_compile",
    "proofs/quotient_attached_respect_lift_compile",
    "proofs/quotient_lift_missing_respect",
    "proofs/quotient_attached_lift_missing_respect",
    "proofs/higher_order_machine_forwarded_contract_mismatch",
    "proofs/higher_order_machine_schema_contract_mismatch",
    "proofs/machine_parameterized_data_contract_mismatch",
    "proofs/machine_parameterized_runtime_data_rejected",
    "proofs/constant_equation_refuted",
    "proofs/order_asymmetry_refuted",
    "proofs/order_transitivity_false_twin",
    "proofs/linear_range_sum_false_twin",
    "proofs/congruence_false_twin",
    "proofs/addition_commutativity_false_twin",
    "proofs/nonlinear_square_range_false_twin",
    "proofs/order_antisymmetry_false_twin",
    "proofs/remainder_range_false_twin",
    "proofs/bag_view_false_twin",
    "proofs/vacuity_satisfiable_premise_false_twin",
    "proofs/ih_citation_false_twin",
    "proofs/computed_subject_requires_undischarged",
    "proofs/computed_edge_positivity_missing",
    "proofs/inductive_gauss_sum_false_twin",
    "proofs/inductive_gauss_sum_step_false_twin",
    "proofs/inductive_climbing_sum_step_false_twin",
    "proofs/nat_unmeasured_recursion_rejected",
    "proofs/nat_nondescending_recursion_rejected",
    "proofs/integer_measured_nat_recursion_stalled",
    "proofs/nat_substate_nondescending_rejected",
    "proofs/integer_measured_nat_claim_refuted",
    "proofs/nat_structural_disproof_refuted",
    "proofs/nat_payload_disjointness_refuted",
    "proofs/nat_ground_compute_refuted",
    "proofs/nat_inductive_claim_refuted",
    "traits/default_keyword_retired",
    "traits/equatable_field_not_equatable",
    "traits/equatable_missing_conformance_suggested",
    "traits/equatable_recursive_type",
    "traits/generic_trait_default_arity",
    "traits/generic_trait_default_binding_mismatch",
    "traits/inherited_default_ambiguous",
    "traits/inherited_default_reabstracted",
    "traits/trait_oneoff_machine_requirement",
    "traits/trait_composition_missing_requirement",
    "traits/trait_requirement_cycle",
    "traits/trait_requires_unknown",
    "traits/trait_satisfies_arity_mismatch",
    "traits/trait_satisfies_missing_machine",
    "traits/trait_satisfies_parameter_mismatch",
    "traits/trait_satisfies_unknown",
    "traits/trait_unknown_signature_type",
    "termination/bounded_distance_inverted",
    "termination/custom_ranking_field_stalled_decrease",
    "termination/custom_ranking_order_non_numeric",
    "termination/custom_ranking_order_parameter_mismatch",
    "termination/custom_ranking_order_unknown",
    "termination/custom_ranking_order_wrong_arity",
    "termination/default_order_ambiguous",
    "termination/default_order_declared_measure_not_inferred",
    "termination/increasing_unbounded_rejected",
    "termination/joint_machine_call_cycle_forwarding_rejected",
    "termination/joint_machine_call_cycle_stalled",
    "termination/mutual_recursion_no_decrease",
    "termination/proof_joint_machine_cycle_nondecreasing",
    "termination/proof_joint_machine_cycle_unmeasured",
    "termination/rank_range_excludes_floor",
    "termination/retired_block_form",
    "termination/retired_standalone_decreases",
    "termination/retired_standalone_increases",
    "termination/runtime_non_tail_joint_machine_cycle",
    "termination/subtraction_spelling_retired",
    "slices/termination_slice_length_order_unimplemented",
    "expressions/real_literal_suffix_retired",
    "expressions/match_duplicate_pattern_rejected",
    "expressions/primitive_member_access_rejected",
    "expressions/float_bitwise_rejected",
    "expressions/bitwise_not_non_integer_rejected",
    "expressions/logical_not_non_bool_rejected",
    "expressions/number_to_bool_cast_rejected",
    "expressions/cast_struct_to_number_rejected",
    "expressions/cast_array_to_number_rejected",
    "expressions/cross_type_equality_rejected",
    "expressions/array_equality_rejected",
    "expressions/scalar_into_data_field_rejected",
    "expressions/cross_enum_case_comparison_rejected",
    "expressions/cross_enum_case_membership_rejected",
    "expressions/mismatched_width_comparison_rejected",
    "expressions/mismatched_width_bitwise_rejected",
    "expressions/bool_numeric_operand_mixing_rejected",
    "expressions/non_bool_logical_operand_rejected",
    "expressions/array_operator_rejected",
    "expressions/struct_operator_undeclared_rejected",
    "expressions/cast_to_non_scalar_type_rejected",
    "calls/value_call_wrong_argument_count_rejected",
    "calls/plain_let_reassign_rejected",
    "calls/unresolved_value_call_rejected",
    "calls/unresolved_receiver_method_rejected",
    "calls/void_value_callee_rejected",
    "calls/empty_body_return_machine_rejected",
    "calls/discarded_call_result",
    "calls/discarded_trait_call_result",
    "calls/terminal_return_type_mismatch_rejected",
    "calls/abs_call_argument_rejected",
    "calls/nested_value_call_arg_rejected",
    "expressions/out_of_range_comparison_literal_rejected",
    "arithmetic/float_to_int_exact_unproven",
    "arithmetic/float_to_int_wrapping_rejected",
    "float/named_float_to_integer_exact_unproven",
    "float/named_float_to_integer_wrapping_rejected",
    "float/named_float_to_integer_no_context_unproven",
    "float/named_float_to_integer_implicit_discard_rejected",
    "arithmetic/exact_integer_cast_unproven",
    "expressions/arithmetic_domain_mixed",
    "arithmetic/wrapping_target_plain_operands_rejected",
    "arithmetic/u64_literal_into_i64_rejected",
    "arithmetic/u64_literal_ordering_guard_rejected",
    "arithmetic/narrowing_literal_wider_than_target",
    "arithmetic/narrowing_wide_local_unproven",
    "arithmetic/narrowing_signedness_rejected",
    "arithmetic/suffix_type_disagrees_rejected",
    "arithmetic/suffix_magnitude_overflow_rejected",
    "arithmetic/float_cast_wrapping_rejected",
    "arithmetic/u64_magnitude_arg_non_u64_rejected",
    "arithmetic/float_cast_unproven_rejected",
    "arithmetic/suffix_negative_unsigned_rejected",
    "float/suffix_format_disagrees_rejected",
    "expressions/arithmetic_domain_literal_target_overflow",
    "arithmetic/bool_arithmetic_into_bool_rejected",
    "arithmetic/bitwise_numeric_into_bool_rejected",
    "arithmetic/undeclared_bare_name_rejected",
    "arithmetic/indexed_element_class_rejected",
    "arithmetic/indexed_element_narrowing_rejected",
    "arithmetic/array_literal_too_few_rejected",
    "arithmetic/array_literal_too_many_rejected",
    "arithmetic/removed_range_constraint_syntax",
    "arithmetic/construction_payload_out_of_range",
    "arithmetic/unconstrained_payload_arithmetic",
    "arithmetic/bounded_assignment_unproven",
    "arithmetic/zii_range_excludes_zero_rejected",
    "arithmetic/exact_shift_left_value_unproven",
    "arithmetic/exact_shift_count_out_of_range",
    "arithmetic/exact_shift_count_unproven",
    "arithmetic/saturating_shift_count_unproven",
    "arithmetic/divide_by_zero_rejected",
    "arithmetic/modulo_by_zero_rejected",
    "arithmetic/expression_range_bound_store_rejected",
    "arithmetic/non_constant_range_bound_rejected",
    "arithmetic/ranged_divide_target_too_narrow",
    "arithmetic/ranged_divide_possibly_zero_divisor",
    "expressions/nested_i32_mul_overflow",
    "arithmetic/nested_field_exact_overflow_rejected",
    "arithmetic/guard_invalidated_by_prior_write_rejected",
    "arithmetic/struct_field_arithmetic_unproven",
    "arithmetic/transition_arg_unguarded_overflow",
    "arithmetic/shift_count_literal_oor_rejected",
    "arithmetic/shift_count_unproven_rejected",
    "arithmetic/shift_count_saturating_oor_rejected",
    "constraints/inverted_range_rejected",
    "constraints/finite_window_open_at_consumption",
    "time/duration_subsecond_range_rejected",
    "dependent/data_where_literal_violates",
    "dependent/data_where_write_violates",
    "dependent/data_where_read_before_establish",
    "dependent/data_where_cross_state_unknown_refuses",
    "dependent/data_where_param_write_unproven",
    "dependent/data_where_multistate_partial_refuses",
    "dependent/data_where_cyclic_hypothesis_refuses",
    "dependent/data_where_window_unclosed_terminal",
    "slices/dynamic_subslice_bounded_unproven",
    "slices/dynamic_subslice_end_unproven",
    "slices/dynamic_subslice_start_unproven",
    "slices/invalid_fixed_array_literal_index_unchecked",
    "slices/known_length_dynamic_index_unproven",
    "slices/machine_field_index_reassigned_unproven",
    "slices/invalid_slice_folded_index_unchecked",
    "slices/invalid_slice_local_index_unchecked",
    "slices/invalid_subslice_folded_bounds_unchecked",
    "slices/invalid_subslice_bounded_end_unchecked",
    "slices/invalid_subslice_bounded_order_unchecked",
    "slices/invalid_subslice_bounds_unchecked",
    "slices/invalid_subslice_end_bounds_unchecked",
    "slices/invalid_inclusive_subslice_end_at_len_unchecked",
    "slices/invalid_slice_literal_index_unchecked",
    "slices/invalid_slice_reassigned_local_index_unchecked",
    "slices/slice_parameter_index_unproven",
    "slices/slice_parameter_literal_index_unproven",
    "slices/slice_parameter_literal_subslice_unproven",
    "slices/slice_parameter_subslice_unproven",
    "slices/guarded_slice_parameter_bounded_subslice_order_unproven",
    "slices/window_shrink_index_out_of_length",
    "slices/window_subslice_end_over_exact_length",
    "slices/window_shrink_min_length_tail_index_unproven",
    "slices/window_literal_bounds_min_length_parent_index_unproven",
    "slices/window_reassigned_shrunk_floor_unproven",
    "slices/subslice_range_operator_contract_unproven",
    "slices/index_operator_contract_unproven",
    "slices/overlapping_mut_subslice_windows_rejected",
    "slices/unknown_bounds_mut_subslice_windows_rejected",
    "range/guarded_copy_stale_after_write",
    "range/guarded_copy_bound_too_wide",
    "range/guarded_binary_operand_too_wide",
    "range/guarded_binary_operand_stale",
    "range/funnel_guard_one_edge_unguarded",
    "range/funnel_guard_edges_disagree",
    "range/element_range_write_rejected",
    "range/element_range_runtime_write_rejected",
    "range/indexed_field_range_runtime_write_rejected",
    "range/element_range_zero_excluded",
    "range/guarded_element_increment_too_wide",
    "range/guarded_element_increment_stale",
    "range/guarded_runtime_index_reassigned",
    "range/guarded_runtime_index_collection_write",
    "ranges/loop_increment_index_unbounded",
    "ranges/loop_body_resets_index",
    "ranges/loop_init_exceeds_capacity",
    "ranges/index_join_unbounded_arm",
    "ranges/index_read_after_increment_oob",
    "ranges/index_read_after_decrement_negative",
    "ranges/index_signed_guard_below_zero",
    "ranges/sum_payload_direct_access_unproven",
    "ranges/sum_payload_non_case_guard_unproven",
    "ranges/sum_payload_arith_too_wide_unproven",
    "collections/computed_index_unproven_rejected",
    "collections/declared_range_index_too_wide",
    "collections/u64_high_bit_index_rejected",
    "collections/wrapping_range_index_unproven",
    "collections/fixed_vec_push_without_room",
    "collections/fixed_vec_get_past_length",
    "collections/write_first_loop_bound_exceeds_capacity",
    "dependent/data_where_capacity_mismatch_rejected",
    "dependent/data_where_whole_read_during_window_rejected",
    "dependent/dependent_arg_unrelated_rejected",
    "dependent/dependent_field_unranged_rejected",
    "dependent/dependent_range_on_field_rejected",
    "dependent/dependent_call_arg_unproven_rejected",
    "dependent/dependent_forward_after_write_rejected",
    "dependent/dependent_subtract_after_write_rejected",
    "dependent/requires_call_unproven_rejected",
    "dependent/sibling_len_arg_unrelated_rejected",
    "dependent/sibling_len_unknown_sibling_rejected",
    "dependent/bounded_product_weak_coupling_rejected",
    "dependent/data_where_gated_field_omitted_rejected",
    "dependent/data_where_ambiguous_domain_short_name_rejected",
    "dependent/data_where_gated_false_literal_rejected",
    "dependent/data_where_membership_literal_rejected",
    "dependent/data_where_length_mismatch_rejected",
    "dependent/data_where_membership_carrier_mismatch_rejected",
    "dependent/data_where_symbolic_correlation_stale_rejected",
    "dependent/data_where_noncommutative_correlation_rejected",
    "dependent/data_where_standing_bound_absent_rejected",
    "dependent/call_frame_invalidates_written_fact_rejected",
    "dependent/transitive_call_frame_invalidates_written_fact_rejected",
    "dependent/transition_call_frame_invalidates_written_fact_rejected",
    "dependent/transition_parameter_frame_invalidates_written_fact_rejected",
    "dependent/call_bearing_transition_frame_invalidates_written_fact_rejected",
    "dependent/data_where_call_frame_invalidates_written_valuation_rejected",
    "dependent/proof_call_frame_invalidates_dependent_forward_rejected",
    "dependent/proof_value_call_frame_invalidates_dependent_forward_rejected",
    "dependent/state_arrival_contract_unproven_rejected",
    "dependent/loop_invariant_invalidated_by_sibling_call_rejected",
    "dependent/relational_loop_invariant_reassigned_index_rejected",
    "dependent/relational_loop_invariant_collection_call_rejected",
    "dependent/relational_loop_invariant_limit_bridge_absent_rejected",
    "dependent/relational_loop_invariant_limit_call_rejected",
    "dependent/relational_loop_invariant_limit_preheader_write_rejected",
    "dependent/relational_loop_invariant_fully_nonstrict_rejected",
    "dependent/relational_loop_invariant_bound_chain_call_rejected",
    "dependent/relational_loop_invariant_bound_chain_fully_nonstrict_rejected",
    "dependent/data_where_invariant_window_unclosed_rejected",
    "dependent/data_where_gated_machine_unestablished_rejected",
    "dependent/range_sugar_gated_field_omitted_rejected",
    "dependent/nested_gated_field_omitted_rejected",
    "dependent/nested_data_where_window_unclosed_rejected",
    "dependent/data_where_live_borrow_pins_witness_rejected",
    "dependent/indexed_data_where_window_unclosed_rejected",
    "dependent/data_where_borrow_during_window_rejected",
    "domains/routed_domain_as_rejected",
    "domains/implicit_semantic_domain_local_weakening",
    "domains/implicit_semantic_domain_assignment_weakening",
    "domains/implicit_semantic_domain_field_weakening",
    "domains/implicit_semantic_domain_array_weakening",
    "domains/implicit_semantic_domain_call_weakening",
    "domains/implicit_semantic_domain_return_weakening",
    "domains/implicit_arithmetic_policy_weakening",
    "domains/implicit_routed_domain_weakening",
    "domains/implicit_alias_semantic_weakening",
    "domains/domain_route_result_mismatch",
    "domains/bodyless_internal_state_reconstruction_rejected",
    "domains/boundary_operator_mutation_invalidates_domain",
    "domains/semantic_cast_mint_staged",
    "domains/semantic_cast_fact_false",
    "domains/semantic_cast_range_insufficient",
    "domains/semantic_cast_requires_missing",
    "domains/semantic_cast_unknown_domain",
    "domains/call_requires_boundary_trait_unproven",
    "domains/call_requires_boundary_unproven",
    "domains/result_domain_overload_duplicate_predicate",
    "domains/result_domain_overload_semantic_weakening",
    "domains/domain_operator_competing_spelling_meanings",
    "domains/domain_operator_meaning_unproven",
    "domains/domain_operator_meaning_invalidated_by_mutation",
    "domains/domain_operator_requires_unproven",
    "proofs/float_meaning_runtime_consumption_rejected",
    "proofs/cauchy_triangle_wrong_middle_den_rejected",
    "proofs/core_nat_runtime_consumption_rejected",
    "proofs/generic_contract_pre_specialization_false_law_rejected",
    "proofs/nested_structural_case_false_rejected",
    "proofs/rat_close_triangle_false_rejected",
    "proofs/rat_scaled_triangle_false_rejected",
    "proofs/static_machine_selection_false_equality_rejected",
    "proofs/value_call_refuted_inequality_rejected",
    "proofs/structural_ensures_unjudged_rejected",
    "proofs/record_false_comm_rejected",
    "proofs/ring_exchange_unhypothesized_rejected",
    "proofs/runtime_attached_machine_proof_parameter_rejected",
    "proofs/zero_value_wrong_home_case_rejected",
    "proofs/zero_value_gated_home_rejected",
    "proofs/zero_value_executable_rejected",
    "proofs/polynomial_false_expand",
    "traits/ring_requirement_unknown_rejected",
    "traits/ring_requirement_signature_rejected",
    "traits/free_machine_bare_satisfies_rejected",
    "data/builtin_type_name_shadow",
    "data/record_pattern_missing_field",
    "data/record_pattern_unknown_field",
    "control_flow/if_statement_retired",
    "control_flow/tuple_transition_uncovered_rejected",
    "control_flow/sum_tuple_matrix_uncovered_rejected",
    "control_flow/transition_fall_through_bool",
    "control_flow/transition_fall_through_value_match",
    "control_flow/arm_pattern_waived_field_use",
    "control_flow/arm_pattern_missing_field",
    "control_flow/arm_pattern_rest_unknown_field",
    "termination/terminates_block_form_retired",
    "termination/rank_floor_unconsumed",
    "termination/requirement_witness_rejected",
    "termination/standalone_decreases_retired",
    "termination/rank_range_unconsumed",
    "generics/const_data_where_machine_fact_effectful",
    "generics/const_data_machine_call_requires_pure",
    "borrow/free_machine_view_invalidated_by_linked_input_write",
    "borrow/view_return_ambiguous_ref_inputs",
    "borrow/method_view_receiver_unrelated_field_write",
    "borrows/borrow_helper_alias_active",
    "calls/runtime_two_state_tail_cycle_forwarding_rejected",
    "calls/machine_call_cycle_rejected",
    "calls/statement_tail_self_call_rejected",
    "calls/terminal_self_call_recursion_rejected",
    "calls/runtime_helper_ordering_return",
    "calls/mutual_cycle_decrease_unproven",
    "calls/mutual_cycle_disqualified_shape",
    "drops/authored_cleanup_static_argument_rejected",
    "drops/authored_cleanup_forwarded_reference_rejected",
    "drops/nonempty_drop_body_rejected",
    "drops/drop_ensures_nonempty_body_rejected",
    "drops/drop_nonblocking_effect_unknown",
    "relax/retired_relax_statement",
    "relax/retired_relaxed_reference",
    "slices/guarded_slice_parameter_index_equals_len_compile",
    "slices/unguarded_slice_parameter_end_subslice_compile",
    "slices/unguarded_slice_parameter_index_compile",
    "slices/unguarded_slice_parameter_subslice_compile",
    "text/bounded_carrier_construction_over_capacity_rejected",
    "text/bounded_carrier_return_over_capacity_rejected",
    "expressions/undeclared_two_segment_path_rejected",
    "platform/platform_block_retired",
    "ffi/raw_ptr_read_unavailable",
    "core/numeric_exact_narrowing_unproven",
    "core/numeric_cross_signed_exact_negative_unproven",
    "core/numeric_cross_signed_exact_unsigned_unproven",
    "data/boundary_data_construction_rejected",
    "data/generic_boundary_data_construction_rejected",
    "data/boundary_data_relaxed_carry_unadmitted",
    "capabilities/provider_widens_requirement_ceiling",
    "capabilities/provider_hidden_extra_effect",
    "capabilities/unapproved_host_call",
    "capabilities/blocking_beneath_no_block_root",
    "capabilities/cyclic_synchronous_invocation",
    "capabilities/effect_ceiling_exceeded",
    "capabilities/effect_outside_trait_requirement",
    "capabilities/unknown_effect_name",
    "capabilities/undeclared_synchronous_invocation",
    "concurrency/suspend_live_value_rejected",
    "concurrency/suspend_call_argument_rejected",
    "concurrency/suspend_later_operand_rejected",
    "concurrency/suspend_self_field_reachable_state_rejected",
    "concurrency/barrier_wait_contract",
    "concurrency/mutex_lock_guard",
    "concurrency/spawn_retired",
    "concurrency/join_type_retired",
    "atomics/atomic_load_publish_ordering_rejected",
    "atomics/atomic_store_receive_ordering_rejected",
    "atomics/atomic_compare_exchange_failure_publish_rejected",
    "atomics/atomic_compare_exchange_failure_stronger_rejected",
    "atomics/atomic_legacy_ordering_rejected",
    "atomics/atomic_unknown_ordering_rejected",
    "wire/encode_wire_spelling_renamed",
    "wire/decode_verdict_must_be_enum",
    "wire/wire_data_form_retired",
    "wire/reserved_spelling_retired",
    "wire/legacy_numbered_field_spelling",
    "wire/unnumbered_field_in_numbered_data",
    "wire/duplicate_field_number",
    "wire/field_reuses_reserved_number",
    "wire/duplicate_version_declaration",
    "wire/unknown_field_type",
    "wire/erased_unknown_field_type",
    "wire/version_field_retired_without_reserved",
    "wire/version_chain_retired_without_reserved",
    "wire/nested_schema_cycle",
    "recast/recast_mut_fact_fenced",
    "recast/reference_let_pun_requires_recast",
    "recast/recast_mut_cross_carrier_domain_not_equivalent",
    "recast/recast_mut_range_bit_sets_differ",
    "recast/recast_mut_bool_bit_sets_differ",
    "recast/recast_shared_bool_fact_fenced",
    "recast/recast_shared_interior_fact_fenced",
    "recast/recast_shared_domain_strengthening_rejected",
    "recast/recast_shared_float_range_strengthening_rejected",
    "recast/recast_shared_record_float_leaf_strengthening_rejected",
    "recast/recast_mut_record_float_leaf_sets_differ",
    "recast/recast_mut_record_leaf_sets_differ",
    "recast/recast_mut_interior_fact_fenced",
    "recast/recast_mut_record_fact_fenced",
    "recast/recast_mut_record_array_fact_fenced",
    "versioning/data_version_block_retired",
];

#[path = "canary_suite/entry_and_abi.rs"]
mod entry_and_abi;
#[path = "canary_suite/layouts_and_pending.rs"]
mod layouts_and_pending;
#[path = "canary_suite/proof_and_float_suites.rs"]
mod proof_and_float_suites;
fn compile_canary_without_output_for_target(
    canary_dir: &Path,
    target: &str,
) -> Result<CompileReport, Vec<Diagnostic>> {
    let build_dir = unique_no_output_build_dir();
    let result = compile_with_artifact_policy(
        CanaryCompileSpec {
            root_path: canary_dir.join("main.omg"),
            build_dir: Some(build_dir.clone()),
            target_name: Some(target.into()),
            product: CanaryCompileProduct::Check,
        },
        ArtifactEmissionPolicy::OutputOnly,
    );
    let _ = fs::remove_dir_all(&build_dir);
    result
}

fn compile_canary_without_output(canary_dir: &Path) -> Result<CompileReport, Vec<Diagnostic>> {
    // A checking request writes pipeline phase artifacts into `build_dir()`,
    // and a `None` build dir defaults to `<canary>/build`
    // -- a path SHARED by every test that compiles the same canary. Under parallel
    // test threads two such compiles race on the artifact files (delete-while-write
    // / file-in-use on Windows), which is exactly the intermittent
    // `pass_canaries_compile` vs `capability_pass_canaries_compile_in_isolation`
    // full-suite flake. Give every no-output compile its own temp dir instead.
    let build_dir = unique_no_output_build_dir();
    let result = compile_with_artifact_policy(
        CanaryCompileSpec {
            root_path: canary_dir.join("main.omg"),
            build_dir: Some(build_dir.clone()),
            target_name: None,
            product: CanaryCompileProduct::Check,
        },
        ArtifactEmissionPolicy::OutputOnly,
    );
    let _ = fs::remove_dir_all(&build_dir);
    result
}

fn compile_native_canary_without_output(
    canary_dir: &Path,
) -> Result<CompileReport, Vec<Diagnostic>> {
    let build_dir = unique_no_output_build_dir();
    let result = compiler::compile(
        CompileRequest::new(CompilerOptions {
            root_path: canary_dir.join("main.omg"),
            build_dir: Some(build_dir.clone()),
            target_name: None,
        })
        .with_requested_product(RequestedCompileProduct::NativeArtifact)
        .with_artifact_policy(ArtifactEmissionPolicy::OutputOnly),
    );
    let _ = fs::remove_dir_all(&build_dir);
    result
}

fn compile_rooted_backend_canary_without_output(
    canary_dir: &Path,
) -> Result<CompileReport, Vec<Diagnostic>> {
    compile_rooted_backend_canary_without_output_for_target(canary_dir, native_hosted_target())
}

fn compile_rooted_backend_canary_without_output_for_target(
    canary_dir: &Path,
    target: &str,
) -> Result<CompileReport, Vec<Diagnostic>> {
    compile_rooted_backend_canary_without_output_for_target_and_permission_policy(
        canary_dir,
        target,
        native_realization::current_terminal_authority_permission_policy(),
    )
}

fn compile_rooted_backend_canary_without_output_for_target_with_fixture_permissions(
    canary_dir: &Path,
    target: &str,
) -> Result<CompileReport, Vec<Diagnostic>> {
    let build_dir = unique_no_output_build_dir();
    let root_path = canary_dir.join("main.omg");
    let package_inputs = reviewed_repository_fixture_package_inputs(&root_path, Some(target))?
        .ok_or_else(|| {
            vec![Diagnostic::error(
                "fixture has no repository package inputs",
            )]
        })?;
    let permission_policy = native_realization::terminal_authority_permission_policy_with_rows(
        package_inputs
            .accepted_semantic_bindings()
            .flat_map(|binding| binding.terminal_authority_permissions())
            .cloned()
            .collect(),
    )
    .map_err(|error| {
        vec![Diagnostic::error(format!(
            "cannot construct repository fixture terminal-authority policy: {error:?}"
        ))]
    })?;
    let result = compiler::compile(
        CompileRequest::new(CompilerOptions {
            root_path,
            build_dir: Some(build_dir.clone()),
            target_name: Some(target.into()),
        })
        .with_requested_product(RequestedCompileProduct::NativeArtifact)
        .with_artifact_policy(ArtifactEmissionPolicy::OutputOnly)
        .with_terminal_authority_permission_policy(permission_policy)
        .with_package_inputs(package_inputs),
    );
    let _ = fs::remove_dir_all(&build_dir);
    result
}

fn compile_rooted_backend_canary_without_output_for_target_and_permission_policy(
    canary_dir: &Path,
    target: &str,
    permission_policy: native_realization::TerminalAuthorityPermissionPolicy,
) -> Result<CompileReport, Vec<Diagnostic>> {
    let build_dir = unique_no_output_build_dir();
    let root_path = canary_dir.join("main.omg");
    let package_inputs = reviewed_repository_fixture_package_inputs(&root_path, Some(target))?;
    let mut request = CompileRequest::new(CompilerOptions {
        root_path,
        build_dir: Some(build_dir.clone()),
        target_name: Some(target.into()),
    })
    .with_requested_product(RequestedCompileProduct::NativeArtifact)
    .with_artifact_policy(ArtifactEmissionPolicy::OutputOnly)
    .with_terminal_authority_permission_policy(permission_policy);
    if let Some(package_inputs) = package_inputs {
        request = request.with_package_inputs(package_inputs);
    }
    let result = compiler::compile(request);
    let _ = fs::remove_dir_all(&build_dir);
    result
}

fn compile_rooted_canary_for_native_host(
    canary_dir: &Path,
    build_dir: PathBuf,
) -> Result<CompileReport, Vec<Diagnostic>> {
    compile_rooted_canary_for_target(canary_dir, build_dir, native_hosted_target())
}

fn compile_rooted_canary_for_native_host_with_auxiliary_artifacts(
    canary_dir: &Path,
    build_dir: PathBuf,
) -> Result<CompileReport, Vec<Diagnostic>> {
    compile_rooted_canary_for_target_with_auxiliary_artifacts(
        canary_dir,
        build_dir,
        native_hosted_target(),
    )
}

fn compile_rooted_canary_for_target(
    canary_dir: &Path,
    build_dir: PathBuf,
    target: &str,
) -> Result<CompileReport, Vec<Diagnostic>> {
    compile_rooted_canary_for_target_with_artifact_policy(
        canary_dir,
        build_dir,
        target,
        ArtifactEmissionPolicy::OutputOnly,
    )
}

fn compile_rooted_canary_for_target_with_auxiliary_artifacts(
    canary_dir: &Path,
    build_dir: PathBuf,
    target: &str,
) -> Result<CompileReport, Vec<Diagnostic>> {
    compile_rooted_canary_for_target_with_artifact_policy(
        canary_dir,
        build_dir,
        target,
        ArtifactEmissionPolicy::Full,
    )
}

fn compile_rooted_canary_for_target_with_artifact_policy(
    canary_dir: &Path,
    build_dir: PathBuf,
    target: &str,
    artifact_policy: ArtifactEmissionPolicy,
) -> Result<CompileReport, Vec<Diagnostic>> {
    compile_with_artifact_policy(
        CanaryCompileSpec {
            root_path: canary_dir.join("main.omg"),
            build_dir: Some(build_dir),
            target_name: Some(target.into()),
            product: CanaryCompileProduct::NativeArtifactAndPublish,
        },
        artifact_policy,
    )
}

// These runtime/layout/recast fixtures are deployable on every hosted target.
// Their authored build roots are part of the canary: the pass umbrella must
// exercise production entry selection and may not substitute the legacy entry
// seam.
const ROOTED_BACKEND_PASS_CANARIES: &[&str] = &[
    "core/content_projection_owner",
    "core/content_conservation_contract",
    "core/content_retained_custody_round_trip",
    "calls/runtime_referenced_local_outlives_sibling_guard_call_exit",
    "control_flow/runtime_tuple_transition_exit",
    "errors/runtime_result_match_exit",
    "expressions/runtime_enum_match_breadth_exit",
    "expressions/runtime_f64_state_arg_exit",
    "slices/runtime_field_array_element_value_operand_exit",
    "slices/runtime_indexed_rmw_temp_exit",
    "slices/runtime_indexed_struct_field_write_exit",
    "slices/runtime_indexed_write_adjacent_field_exit",
    "slices/runtime_indexed_write_const_read_exit",
    "slices/runtime_join_meet_bound_exit",
    "slices/runtime_local_aggregate_into_let_exit",
    "slices/runtime_local_slice_len_comparison_value_exit",
    "slices/runtime_slice_fixed_index_guard_exit",
    "slices/runtime_slice_index_transition_exit",
    "slices/runtime_slice_iteration_exit",
    "slices/runtime_slice_len_transition_exit",
    "storage/runtime_dispatch_local_index_binary_write_exit",
    "storage/runtime_machine_owned_indexed_nested_room_copy_exit",
    "storage/runtime_slice_alias_indexed_field_write_exit",
    "text/runtime_case_payload_domain_forward_exit",
    "text/runtime_param_domain_forward_exit",
    "traits/runtime_dyn_single_impl_dispatch_exit",
    "traits/runtime_local_named_dyn_devirtualized_exit",
    "traits/runtime_dyn_two_impl_dispatch_exit",
    "traits/runtime_dyn_two_impl_dispatch_swapped_exit",
    "traits/runtime_ref_param_method_dispatch_exit",
    "borrow/runtime_view_linked_input_unrelated_ref_write_exit",
    "build/runtime_main_source_builder_is_ordinary_exit",
    "calls/runtime_alias_indexed_read_through_transition_exit",
    "calls/runtime_alias_write_through_guarded_transition_exit",
    "calls/runtime_call_result_through_reference_field_exit",
    "calls/runtime_nested_field_terminal_second_instance_exit",
    "calls/runtime_nested_local_terminal_second_instance_exit",
    "calls/runtime_reference_param_forwarded_through_loop_exit",
    "calls/runtime_value_call_through_alias_in_dispatch_exit",
    "control_flow/runtime_state_loop_indexed_search_exit",
    "control_flow/runtime_statement_call_single_execution_exit",
    "dungeon/runtime_nested_value_call_caller_local_guard_exit",
    "expressions/borrow_carrying_data_field_exit",
    "host/runtime_write_no_newline_exit",
    "time/runtime_value_machine_receiver_field_postentry_exit",
    "calls/runtime_dispatch_float_terminal_exit",
    "calls/runtime_dispatch_slice_element_terminal_exit",
    "calls/runtime_let_local_nested_state_arg_exit",
    "calls/runtime_multiarm_texteq_local_exit",
    "calls/runtime_nested_inline_chain_result_exit",
    "calls/runtime_nonentry_inline_second_receiver_exit",
    "calls/runtime_param_forward_chain_second_receiver_exit",
    "calls/runtime_param_receiver_second_instance_exit",
    "calls/runtime_param_receiver_single_instance_exit",
    "calls/runtime_pre_guard_texteq_local_arg_forward_exit",
    "calls/runtime_pre_guard_texteq_local_guard_exit",
    "calls/runtime_same_type_second_receiver_mutation_exit",
    "calls/runtime_value_call_slice_len_guard_exit",
    "collections/runtime_dutch_flag_partition_exit",
    "references/runtime_nested_receiver_same_type_exit",
    "calls/runtime_called_machine_loop_search_exit",
    "calls/runtime_computed_transition_args_exit",
    "calls/runtime_cross_machine_substate_name_exit",
    "calls/runtime_dispatch_machine_array_slice_arg_exit",
    "calls/runtime_dispatch_result_alias_read_exit",
    "calls/runtime_dispatch_result_enum_case_exit",
    "calls/runtime_dispatch_result_field_binding_exit",
    "calls/runtime_dispatch_second_receiver_exit",
    "calls/runtime_nonentry_second_receiver_exit",
    "calls/runtime_option_value_call_exit",
    "calls/runtime_selfcall_chain_second_receiver_exit",
    "calls/runtime_struct_by_value_param_exit",
    "calls/runtime_struct_value_call_exit",
    "calls/runtime_value_call_composition_exit",
    "calls/runtime_value_call_to_array_element_exit",
    "filesystem/discarded_self_call_literal_errno_exit",
    "filesystem/field_receiver_method_exit",
    "filesystem/self_value_call_literal_path_exit",
    "filesystem/wrapper_open_with_exit",
    "filesystem/wrapper_param_shadow_exit",
    "host/runtime_console_byte_echo_exit",
    "time/runtime_saturating_time_arith_exit",
    "traits/runtime_typed_two_method_receivers_exit",
    "types/runtime_i16_signed_arith_exit",
    "types/runtime_i64_signed_arith_exit",
    "types/runtime_i8_signed_arith_exit",
    "types/runtime_u16_field_arith_exit",
    "types/runtime_u8_field_arith_exit",
    "wire/runtime_wire_policy_authored_nested_exit",
    "wire/runtime_wire_policy_authored_plan_exit",
    "calls/runtime_dispatch_sibling_value_calls_exit",
    "calls/runtime_inline_repeated_receiver_value_calls_exit",
    "calls/runtime_value_call_struct_literal_arms_exit",
    "calls/runtime_value_call_struct_result_to_target_exit",
    "collections/runtime_case_array_element_write_exit",
    "collections/runtime_indexed_field_local_operand_exit",
    "collections/runtime_indexed_guard_true_false_pair_exit",
    "collections/runtime_indexed_local_bitwise_exit",
    "collections/runtime_indexed_local_compare_exit",
    "domains/runtime_result_domain_machine_overload_exit",
    "filesystem/windows_hard_link_exit",
    "filesystem/windows_positioned_io_exit",
    "filesystem/windows_read_dir_nth_exit",
    "filesystem/windows_wrapper_breadth_exit",
    "filesystem/windows_wrapper_copy_exit",
    "filesystem/windows_wrapper_create_new_exit",
    "filesystem/windows_wrapper_dark_methods_exit",
    "filesystem/windows_wrapper_metadata_exit",
    "filesystem/windows_wrapper_results_exit",
    "filesystem/windows_wrapper_set_len_exit",
    "core/numeric_cross_signed_negative_traps",
    "core/numeric_cross_signed_unsigned_overflow_traps",
    "core/numeric_trapping_conversion_overflow",
    "filesystem/repeated_dir_walk_scan_exit",
    "filesystem/windows_wrapper_exists_exit",
    "float/float_trapping_divide_zero_traps",
    "float/float_trapping_invalid_traps",
    "float/float_trapping_overflow_traps",
    "float/float_trapping_propagated_infinity_traps",
    "float/float_trapping_propagated_nan_traps",
    "calls/runtime_arm_target_host_result_exit",
    "collections/runtime_indexed_rmw_loop_exit",
    "dungeon/runtime_ordered_room_dispatch_real_show_states_exit",
    "filesystem/runtime_local_host_result_dispatch_exit",
    "proofs/accepted_axiom_cited_exit",
    "storage/runtime_machine_owned_indexed_integer_write_exit",
    "targets/single_target_internal_machine_skipped",
    "targets/target_machine_gating_exit",
    "text/runtime_stdin_command_branch_exit",
    "text/runtime_stdin_line_buffering_exit",
    "data/case_membership_union_guard_exit",
    "data/runtime_proof_only_data_declared_exit",
    "dependent/runtime_requires_guarded_call_exit",
    "expressions/arithmetic_domain_saturating_const_fold_exit",
    "proofs/runtime_core_nat_declared_exit",
    "proofs/runtime_core_rat_declared_exit",
    "proofs/runtime_core_roster_ops_exit",
    "proofs/runtime_nat_structural_recursion_exit",
    "storage/runtime_dispatch_helper_local_alias_add_exit",
    "ownership/linear_state_call_handoff",
    "ownership/linear_transition_nested_call_handoff",
    "ownership/linear_repeated_transition_call_handoff",
    "ownership/linear_live_across_call_continuation",
    "ownership/linear_fresh_state_call_result_handoff",
    "ownership/linear_transfer_and_consume",
    "ownership/linear_transparent_record_frontier",
    "ownership/linear_transparent_record_state_result",
    "ownership/linear_aggregate_state_result",
    "arithmetic/runtime_unsigned_modulo_call_argument_exit",
    "arithmetic/runtime_unsigned_modulo_cast_operand_exit",
    "calls/free_standing_machine_helper_compile",
    "calls/runtime_call_result_after_splice_mutation_exit",
    "control_flow/runtime_entry_cast_result_exit",
    "control_flow/runtime_entry_nested_binary_result_exit",
    "control_flow/runtime_entry_return_field_exit",
    "control_flow/runtime_entry_unary_result_exit",
    "calls/runtime_explicit_discard_executes_exit",
    "collections/record_array_field_access",
    "control_flow/guarded_transition_dispatch",
    "control_flow/runtime_straight_line_terminal_local_exit",
    "control_flow/runtime_straight_line_terminal_field_readback_exit",
    "slices/runtime_mutable_slice_element_write_straight_line_exit",
    "slices/runtime_array_indexed_read_exit",
    "slices/runtime_array_indexed_loop_exit",
    "slices/runtime_decreasing_index_exit",
    "slices/runtime_slice_indexed_read_exit",
    "slices/runtime_array_adjacent_index_exit",
    "slices/runtime_nested_decreasing_index_exit",
    "slices/runtime_narrow_widen_cast_exit",
    "slices/runtime_signed_index_guarded_exit",
    "slices/recursive_subslice_element_accumulator_exit",
    "slices/runtime_branched_index_bound_exit",
    "slices/runtime_dispatch_mutable_slice_element_write_exit",
    "slices/runtime_indexed_array_write_exit",
    "slices/runtime_indexed_read_operand_exit",
    "slices/runtime_machine_field_subslice_arg_index_exit",
    "slices/runtime_mutable_slice_element_write_exit",
    "slices/runtime_two_pointer_reverse_exit",
    "slices/runtime_two_pointer_sum_exit",
    "slices/runtime_slice_index_read_exit",
    "slices/runtime_slice_index_read_dispatch_exit",
    "slices/runtime_slice_index_copy_exit",
    "slices/runtime_slice_index_copy_dispatch_exit",
    "slices/runtime_frame_array_slice_parameter_alias_exit",
    "slices/runtime_subslice_range_len_exit",
    "slices/runtime_subslice_bounded_range_len_exit",
    "slices/runtime_subslice_bounded_dynamic_index_exit",
    "slices/runtime_subslice_dynamic_index_exit",
    "slices/runtime_subslice_end_dynamic_index_exit",
    "slices/runtime_nested_subslice_dynamic_index_exit",
    "slices/runtime_nested_subslice_fixed_index_exit",
    "slices/runtime_subslice_range_pointer_exit",
    "slices/runtime_subslice_nested_of_param_exit",
    "slices/runtime_subslice_of_slice_param_exit",
    "slices/runtime_subslice_param_bounded_range_exit",
    "slices/runtime_subslice_param_end_only_exit",
    "slices/runtime_subslice_param_inclusive_end_exit",
    "slices/runtime_subslice_param_local_exit",
    "slices/runtime_subslice_runtime_end_exit",
    "calls/runtime_reference_returned_slice_element_through_param_exit",
    "calls/runtime_nested_guarded_reference_returned_slice_element_exit",
    "calls/runtime_mutable_local_indexed_parameter_write_exit",
    "calls/runtime_mutable_machine_owned_local_indexed_parameter_write_exit",
    "calls/runtime_mutable_dynamic_indexed_machine_owned_parameter_write_exit",
    "calls/runtime_mutable_machine_owned_parameter_write_exit",
    "calls/runtime_terminal_tail_recursion_exit",
    "calls/runtime_measured_tail_recursion_exit",
    "calls/float_value_call_runtime_arg_exit",
    "calls/runtime_value_call_terminal_exit",
    "calls/runtime_std_math_sin_cos_exit",
    "calls/runtime_value_call_struct_payload_cast_field_exit",
    "calls/runtime_branch_leaf_multiple_named_conversion_exit",
    "calls/runtime_nested_named_conversion_alias_exit",
    "calls/runtime_mut_ref_forward_exit",
    "calls/runtime_trailing_state_mut_param_phase_exit",
    "calls/runtime_value_call_transition_args_exit",
    "calls/runtime_value_call_transition_args_straight_line_exit",
    "calls/runtime_guarded_effectful_transition_argument_exit",
    "calls/runtime_value_call_literal_len_arm_guard_exit",
    "calls/runtime_value_call_nested_entry_call_exit",
    "calls/runtime_value_call_same_callee_sites_exit",
    "calls/runtime_two_site_struct_result_exit",
    "calls/runtime_nested_value_call_guard_exit",
    "calls/runtime_cross_callee_let_names_exit",
    "calls/runtime_cross_callee_division_exit",
    "calls/runtime_value_call_shared_payload_name_exit",
    "calls/runtime_value_call_shared_slot_straight_line_exit",
    "calls/runtime_enum_self_method_exit",
    "calls/runtime_same_type_contained_direct_fields_exit",
    "calls/runtime_shared_ref_param_member_exit",
    "calls/runtime_shared_ref_param_large_deref_exit",
    "calls/runtime_large_shared_ref_direct_assignment_exit",
    "calls/runtime_value_call_dispatch_results_exit",
    "calls/runtime_value_call_entry_field_write_exit",
    "calls/runtime_value_call_guard_subject_exit",
    "calls/runtime_effectful_guard_local_and_self_terminal_exit",
    "collections/runtime_dual_indexed_guard_compare_exit",
    "collections/runtime_cross_array_indexed_guard_compare_exit",
    "collections/runtime_dual_indexed_guard_equality_exit",
    "collections/runtime_dual_indexed_copy_exit",
    "collections/runtime_dual_indexed_copy_in_loop_exit",
    "collections/runtime_indexed_write_frame_local_source_exit",
    "collections/runtime_indexed_local_copy_chain_exit",
    "collections/runtime_indexed_reduction_loop_exit",
    "collections/runtime_write_first_loop_index_exit",
    "collections/runtime_inplace_reverse_local_temp_exit",
    "collections/runtime_argmax_index_exit",
    "collections/runtime_array_max_and_sum_exit",
    "collections/runtime_array_min_max_builtin_exit",
    "collections/runtime_bracket_matcher_stack_exit",
    "collections/runtime_computed_array_fill_via_temp_exit",
    "collections/runtime_computed_index_match_subject_exit",
    "collections/runtime_computed_indexed_write_exit",
    "collections/runtime_dual_indexed_comparison_guard_exit",
    "collections/runtime_hoisted_index_write_exit",
    "collections/runtime_indexed_guard_subject_exit",
    "collections/runtime_loop_counter_init_hoisted_exit",
    "collections/runtime_nested_const_product_index_exit",
    "collections/runtime_nested_loop_fill_exit",
    "collections/runtime_palindrome_two_pointer_exit",
    "dependent/runtime_bounded_product_index_exit",
    "dependent/runtime_dependent_ordering_chain_exit",
    "dependent/runtime_dependent_param_range_exit",
    "dependent/runtime_dependent_product_index_exit",
    "dependent/runtime_dependent_subtract_exit",
    "dependent/runtime_requires_subtract_exit",
    "dependent/runtime_sibling_len_index_exit",
    "dungeon/runtime_clear_carve_render_string_fields_exit",
    "dungeon/runtime_direct_boolean_conjunction_exit",
    "dungeon/runtime_enemy_clear_reentry_exit",
    "dungeon/runtime_full_level_wrapper_lookup_string_field_exit",
    "dungeon/runtime_guarded_inline_leaf_arm_skip_exit",
    "dungeon/runtime_multi_room_reentry_exit",
    "dungeon/runtime_ordered_room_dispatch_exit",
    "dungeon/runtime_ordered_room_dispatch_after_call_exit",
    "dungeon/runtime_ordered_room_dispatch_game_shape_exit",
    "dungeon/runtime_ordered_room_dispatch_large_machine_exit",
    "atomics/runtime_atomic_load_store_exit",
    "atomics/runtime_atomic_fetch_add_exit",
    "atomics/runtime_atomic_fetch_sub_exit",
    "atomics/runtime_atomic_fetch_xor_exit",
    "atomics/runtime_atomic_fetch_or_exit",
    "atomics/runtime_atomic_fetch_and_exit",
    "atomics/runtime_atomic_swap_exit",
    "atomics/runtime_atomic_compare_exchange_exit",
    "structs/runtime_enum_classify_dispatch_exit",
    "structs/runtime_nested_field_accumulate_loop_exit",
    "data/runtime_deep_nested_field_exit",
    "data/runtime_struct_value_copy_exit",
    "structs/runtime_particle_system_exit",
    "structs/runtime_nested_struct_construction_exit",
    "structs/runtime_entity_component_exit",
    "structs/runtime_nested_struct_state_machine_exit",
    "structs/runtime_array_element_struct_copy_exit",
    "structs/runtime_nested_struct_value_semantics_exit",
    "structs/runtime_struct_array_literal_exit",
    "structs/runtime_enum_struct_payload_exit",
    "calls/runtime_trailing_local_return_exit",
    "calls/value_call_sequential_result_slots_exit",
    "calls/value_call_sequential_self_capture_exit",
    "storage/runtime_machine_owned_fixed_indexed_struct_copy_exit",
    "storage/runtime_machine_owned_indexed_struct_copy_exit",
    "storage/runtime_machine_owned_indexed_nested_exit_write_exit",
    "control_flow/runtime_sum_field_store_payload_exit",
    "slices/runtime_subslice_len_exit",
    "slices/runtime_subslice_runtime_start_exit",
    "slices/runtime_subslice_runtime_start_over_local_exit",
    "text/runtime_text_storage",
    "text/runtime_large_room_lookup_struct_field_concat_exit",
    "text/runtime_call_argument_struct_string_field_slice_alias_exit",
    "text/runtime_mutable_struct_string_field_copy_concat_write_line",
    "calls/runtime_string_call_result_through_reference_field_exit",
    "calls/runtime_two_string_call_results_through_reference_fields_exit",
    "calls/runtime_offset_string_call_results_through_reference_fields_exit",
    "dungeon/runtime_ordered_room_dispatch_loop_exit",
    "dungeon/runtime_room_use_reentry_exit",
    "control_flow/runtime_tuple_matrix_exhaustive_exit",
    "domains/executable_domain_membership_expression_exit",
    "domains/executable_domain_membership_intersection_guard_exit",
    "domains/executable_imported_domain_membership_exit",
    "domains/executable_imported_domain_membership_guard_exit",
    "domains/executable_domain_membership_union_guard_exit",
    "domains/executable_domain_membership_union_value_exit",
    "types/runtime_addr_value_flow_exit",
    "types/runtime_addr_algebra_exit",
    "control_flow/runtime_sum_tuple_matrix_exhaustive_exit",
    "traits/trait_generic_bound_static_dispatch",
    "calls/recursive_result_bind_first_arg",
    "calls/guarded_value_call_arm_exit",
    "calls/nested_machine_continuation",
    "calls/runtime_branching_callee_chain_exit",
    "calls/runtime_call_guard",
    "calls/runtime_call_value",
    "calls/runtime_exit_code_exit",
    "calls/runtime_inline_recursive_walk_exit",
    "calls/runtime_let_mut_reassign_exit",
    "calls/runtime_local_string_field_copy_through_mut_exit",
    "calls/runtime_min_call_result_arithmetic_exit",
    "calls/runtime_value_call_direct_recursive_walk_exit",
    "calls/runtime_value_call_statement_recursive_walk_exit",
    "control_flow/arm_pattern_rest_optout_exit",
    "control_flow/case_pattern_rename_waive_exit",
    "control_flow/runtime_boolean_or_guard_exit",
    "control_flow/runtime_branching_helper_string",
    "control_flow/runtime_branching_helper_struct",
    "control_flow/runtime_branching_helper_value",
    "control_flow/runtime_case_member_dispatch_exit",
    "control_flow/runtime_compare_pair_dispatch_exit",
    "control_flow/runtime_integer_literal_dispatch_exit",
    "control_flow/runtime_string_literal_dispatch_exit",
    "control_flow/runtime_local_boolean_or_value_exit",
    "control_flow/runtime_multi_assignment_value_calls",
    "control_flow/runtime_nested_branch_value",
    "control_flow/runtime_negated_boolean_place_guard_exit",
    "control_flow/runtime_negated_comparison_guard_exit",
    "control_flow/runtime_nonplace_record_pattern_single_evaluation_exit",
    "control_flow/runtime_tuple_case_destructure_exit",
    "control_flow/record_pattern_arm_rename_guard_exit",
    "control_flow/state_transition_chain",
    "data/case_payload_native_construction",
    "data/match_exhaustive_by_case_union_domain",
    "data/match_exhaustive_by_cases",
    "data/record_pattern_bind_all_exit",
    "data/record_pattern_double_underscore_field",
    "data/record_pattern_let_exit",
    "data/runtime_array_literal_string_field_exit",
    "data/runtime_case_payload_guard_read_exit",
    "data/runtime_case_reassignment_exit",
    "data/runtime_mixed_shape_exit",
    "data/runtime_record_field_value_pattern_exit",
    "data/runtime_struct_literal_string_field_exit",
    "memory/repr_native_stable_layout",
    "host/runtime_console_byte_literal_exit",
    "host/runtime_tick_count_monotonic_exit",
    "wire/wire_data_field_numbers",
    "wire/wire_data_reserved_field",
    "traits/equatable_sum_stale_payload_exit",
    "traits/ring_requirement_satisfies_exit",
    "traits/runtime_trait_default_dispatch_exit",
    "traits/runtime_inherited_trait_default_exit",
    "traits/runtime_generic_trait_default_exit",
    "providers/runtime_adapter_dispatch_exit",
    "providers/provider_type_slot_selected",
    "providers/component_owner_provider_override_compile",
    "providers/test_owner_provider_override_compile",
    "providers/provider_type_target_default",
    "providers/provider_type_target_default_override",
    "providers/adapter_satisfies_compile",
    "providers/external_leaf_via_compile",
    "providers/runtime_adapter_forwarding_exit",
    "providers/runtime_boundary_capability_state_forwarding_exit",
    "providers/checked_boundary_operator_dispatch_exit",
    "providers/runtime_result_domain_requirement_overload_exit",
    "float/named_provider_min_max_sqrt_exit",
    "float/named_provider_negate_is_nan_exit",
    "float/named_provider_classification_predicates_exit",
    "float/named_provider_classify_exit",
    "float/named_provider_multiply_then_add_exit",
    "float/named_provider_fused_multiply_add_exit",
    "float/named_provider_directed_fused_multiply_add_exit",
    "float/runtime_named_format_conversion_exit",
    "float/runtime_named_integer_to_float_conversion_exit",
    "float/runtime_named_float_to_integer_conversion_exit",
    "arithmetic/runtime_float_self_compare_nan_exit",
    "arithmetic/runtime_abs_desugar_exit",
    "arithmetic/runtime_sqrt_builtin_exit",
    "arithmetic/runtime_clamp_desugar_exit",
    "arithmetic/runtime_clamp_narrowing_exit",
    "arithmetic/runtime_negative_float_to_int_exit",
    "arithmetic/runtime_float_min_max_abs_clamp_exit",
    "arithmetic/runtime_float_running_min_max_fold_exit",
    "arithmetic/runtime_shift_count_domain_exit",
    "arithmetic/runtime_shift_atwidth_signed_modular_exit",
    "arithmetic/runtime_shift_subword_masked_count_exit",
    "arithmetic/runtime_shl_saturating_exit",
    "arithmetic/runtime_shift_right_atwidth_exit",
    "arithmetic/runtime_sat_min_idiom_exit",
    "text/runtime_string_concat_membership_exit",
    "text/runtime_string_field_concat_exit",
    "text/runtime_machine_owned_indexed_string_field_concat_exit",
    "text/runtime_machine_owned_indexed_bounded_carrier_literal_exit",
    "text/runtime_machine_owned_double_indexed_bounded_carrier_literal_exit",
    "text/runtime_machine_owned_double_indexed_string_field_concat_exit",
    "text/runtime_slice_alias_indexed_string_field_concat_exit",
    "text/runtime_slice_indexed_string_guard_exit",
    "text/runtime_slice_machine_indexed_string_guard_exit",
    "text/runtime_local_array_indexed_string_guard_exit",
    "text/runtime_local_array_indexed_string_field_concat_exit",
    "text/runtime_slice_fixed_indexed_string_guard_exit",
    "text/runtime_pointee_string_guard_exit",
    "text/runtime_string_field_literal_guard_exit",
    "text/runtime_mutable_string_parameter_concat_exit",
    "text/runtime_mutable_string_parameter_concat_write_line",
    "text/runtime_mutable_string_parameter_wrapped_concat_write_line",
    "text/runtime_mutable_struct_string_field_copy_concat_exit",
    "text/runtime_lookup_struct_field_concat_exit",
    "text/runtime_large_lookup_struct_field_concat_exit",
    "text/runtime_alias_string_write",
    "text/runtime_alias_text_builder_write",
    "text/runtime_chained_string_append_exit",
    "text/runtime_machine_string_append_in_place_exit",
    "text/runtime_string_concat_two_fields_exit",
    "text/runtime_string_append_in_place_exit",
    "text/runtime_local_struct_string_field_concat_exit",
    "text/runtime_string_stored_suffix_exit",
    "calls/bool_value_call_return_exit",
    "calls/struct_literal_transition_arg_exit",
    "slices/runtime_indexed_element_copy_write_exit",
    "arithmetic/suffix_landed_operand_position_exit",
    "float/suffix_f32_single_rounding_exit",
    "float/unsuffixed_f32_destination_single_rounding_exit",
    "float/unsuffixed_f32_argument_single_rounding_exit",
    "arithmetic/f32_transition_arg_rounding",
    "arithmetic/f32_field_store_rounding",
    "arithmetic/const_fold_cast_signedness",
    "calls/mutual_cycle_tail_admitted_exit",
    "arithmetic/const_fold_unsigned_landed_ops_exit",
    "arithmetic/const_fold_unsigned_shift_right_arg_exit",
    "arithmetic/const_fold_unsigned_divide_arg_exit",
    "arithmetic/unsigned_min_max_wrapping_local_exit",
    "arithmetic/unsigned_min_max_operand_position_exit",
    "arithmetic/suffix_boundary_magnitudes_exit",
    "calls/float_value_call_return_exit",
    "float/expansion_float_local_guard_exit",
    "float/f32_chain_per_op_rounding_exit",
    "float/f32_per_operation_rounding_exit",
    "float/anonymous_exact_rat_const_exit",
    "float/finite_core_domain_range_discharge",
    "float/float_to_int_trapping_nan_traps",
    "float/float_to_int_trapping_overflow_traps",
    "float/runtime_named_float_to_integer_trapping_nan_traps",
    "float/runtime_named_float_to_integer_trapping_overflow_traps",
    "float/runtime_std_is_finite_exit",
    "float/float_saturating_arithmetic_exit",
    "float/float_to_int_exact_proofs_exit",
    "float/float_to_int_policy_exit",
    "float/f32_guard_const_arith_landed_exit",
    "float/f32_arg_const_arith_landed_exit",
    "backend/value_machine_self_array_local_index_exit",
    "backend/value_machine_const_index_self_array_exit",
    "storage/runtime_slice_indexed_binary_rmw_exit",
    "storage/runtime_local_slice_forward_exit",
    "arithmetic/struct_literal_field_coercion",
    "arithmetic/array_element_write_width_domain",
    "arithmetic/int_transition_arg_width_wrap",
    "termination/custom_ranking_field_countdown_compile",
    "termination/custom_ranking_struct_view",
    "termination/runtime_recursive_result_roles_exit",
    "arithmetic/runtime_float_nested_operand_exit",
    "arithmetic/runtime_exact_guarded_shift_count_exit",
    "arithmetic/runtime_shift_atwidth_indexed_targets_exit",
    "arithmetic/runtime_sat_nested_operand_domain_exit",
    "arithmetic/runtime_sat_unsigned_onedirection_exit",
    "arithmetic/runtime_shl_saturating_value_overflow_exit",
    "arithmetic/float_literal_cast_proves_exit",
    "arithmetic/u64_magnitude_transition_arg_exit",
    "arithmetic/runtime_shift_count_proven_range_exit",
    "proofs/runtime_decreases_u64_measure_exit",
    "arithmetic/runtime_wrapping_operand_truncation_exit",
    "arithmetic/runtime_float_compare_bool_exit",
    "arithmetic/runtime_trapping_overflow_traps",
    "arithmetic/runtime_guard_proven_counter_exit",
    "arithmetic/runtime_guard_narrowed_transition_arg_exit",
    "structs/aggregate_transition_args_exit",
    "structs/deep_nested_write_paths_exit",
    "core/numeric_conversion_surface",
    "core/numeric_cross_signed_conversion_surface",
    "core/numeric_signed_conversion_surface",
    "core/zii_default_composite_exit",
    "text/zii_string_host_write_exit",
    "text/zii_default_string_equality_exit",
    "text/runtime_owned_string_byte_view_exit",
    "text/runtime_text_not_equals_exit",
    "text/runtime_text_equals_boolean_operand_exit",
    "text/case_literal_texteq_terminal_exit",
    "text/case_literal_texteq_field_store_exit",
    "text/runtime_text_equals_value_positions_exit",
    "control_flow/sum_payload_cast_operand_field_exit",
    "arithmetic/runtime_comparison_value_signedness_exit",
    "arithmetic/runtime_min_max_signedness_exit",
    "arithmetic/runtime_unsigned_division_exit",
    "arithmetic/saturating_multiply_overflow_both_signs",
    "arithmetic/saturating_signed_divide_min_by_neg_one",
    "arithmetic/wrapping_signed_divide_min_by_neg_one",
    "arithmetic/runtime_signed_division_exit",
    "arithmetic/runtime_shift_right_signedness",
    "arithmetic/const_fold_saturating_narrow_exit",
    "arithmetic/const_fold_wrapping_narrow_exit",
    "traits/equatable_record_equality_exit",
    "traits/equatable_sum_payload_equality_exit",
    "traits/equatable_mixed_shape_equality_exit",
    "traits/equatable_string_field_equality_exit",
    "traits/equatable_string_not_equals_exit",
    "traits/equatable_string_equality_guard_exit",
    "data/runtime_whole_struct_mutation_copy_exit",
    "operators/compound_assignment_exit",
    "operators/unary_negation_exit",
    "arithmetic/runtime_chained_field_mutation_exit",
    "arithmetic/runtime_copy_then_read_exit",
    "arithmetic/runtime_i64_full_width_exit",
    "arithmetic/runtime_f32_field_guard_exit",
    "arithmetic/runtime_u64_guarded_cap_store_exit",
    "arithmetic/runtime_nested_payload_range_narrowing_exit",
    "arithmetic/runtime_arithmetic_guard",
    "generics/runtime_generic_value_call_exit",
    "generics/runtime_generic_value_call_agreeing_exit",
    "expressions/runtime_qualified_case_value_exit",
    "generics/runtime_generic_record_instance_exit",
    "generics/runtime_generic_two_instantiations_exit",
    "generics/runtime_generic_enum_payload_exit",
    "generics/runtime_generic_param_position_inference_exit",
    "generics/runtime_generic_multiple_specializations_exit",
    "generics/runtime_const_data_array_length_exit",
    "generics/runtime_const_data_named_value_exit",
    "generics/runtime_const_data_expression_exit",
    "generics/runtime_const_data_machine_call_exit",
    "generics/runtime_const_data_machine_fact_exit",
    "generics/runtime_const_data_where_fact_exit",
    "generics/runtime_const_data_forwarded_length_exit",
    "generics/runtime_const_data_multiple_instances_exit",
    "generics/runtime_const_data_symbolic_expression_exit",
    "generics/runtime_signed_const_data_exit",
    "generics/runtime_const_container_methods_exit",
    "constants/runtime_free_const_exit",
    "comptime/runtime_const_measured_recursion_exit",
    "build/runtime_depend_mapping_exit",
    "arithmetic/runtime_comparison_guard_signedness_exit",
    "expressions/arithmetic_domain_return_range_proven_exact_exit",
    "expressions/arithmetic_domain_trapping_let_overflow",
    "arithmetic/constant_trapping_shift_value_overflow_traps",
    "expressions/f32_field_binary_to_local_cast",
    "expressions/f32_deep_chain_binary",
    "expressions/f32_to_f64_local_cast",
    "control_flow/no_payload_case_variant_after_payload_dispatch_exit",
    "control_flow/entry_surface_receiver_paths",
    "dependent/data_where_standing_bound_exit",
    "dependent/nested_data_where_standing_bound_exit",
    "dependent/indexed_data_where_standing_bound_exit",
    "calls/transition_arg_local_from_embedded_call_exit",
    "calls/value_call_embedded_in_binary_exit",
    "calls/sequential_self_field_rmw_exit",
    "expressions/runtime_float_constant_store_exit",
    "expressions/runtime_match_value_exit",
    "arithmetic/runtime_fnv1a_hash_exit",
    "arithmetic/runtime_min_max_clamp_narrowing_exit",
    "arithmetic/runtime_modulo_div_narrowing_exit",
    "expressions/arithmetic_domain_trapping_mul_overflow",
    "expressions/arithmetic_domain_saturating_signed_exit",
    "expressions/arithmetic_domain_requires_proven_exact_exit",
    "expressions/arithmetic_domain_range_proven_exact_exit",
    "expressions/arithmetic_domain_cast_exit",
    "expressions/arithmetic_domain_trapping_exit",
    "expressions/arithmetic_domain_trapping_overflow",
    "arithmetic/runtime_transition_arg_saturating_exit",
    "arithmetic/runtime_cast_element_accumulator_exit",
    "arithmetic/runtime_inferred_multipath_return_exit",
    "arithmetic/runtime_inferred_return_range_exit",
    "arithmetic/runtime_provable_field_construction_exit",
    "arithmetic/runtime_struct_field_range_narrowing_exit",
    "arithmetic/runtime_payload_range_narrowing_exit",
    "ranges/sum_payload_range_narrowed_exit",
    "ranges/sum_payload_range_arith_narrowed_exit",
    "arithmetic/runtime_exclusive_range_constraint_exit",
    "control_flow/sum_field_storage_roundtrip",
    "control_flow/sum_mixed_width_payload_layout",
    "expressions/arithmetic_domain_saturating_mul_exit",
    "expressions/arithmetic_domain_saturating_mul_signed_exit",
    "expressions/arithmetic_domain_trapping_div_exit",
    "expressions/arithmetic_domain_trapping_mul_exit",
    "arithmetic/runtime_transition_arg_guard_narrowing_exit",
    "arithmetic/runtime_requires_one_sided_bound_exit",
    "arithmetic/runtime_transition_value_guard_narrowing_exit",
    "arithmetic/runtime_transition_arg_false_arm_narrowing_exit",
    "collections/runtime_fixed_vec_round_trip_exit",
    "expressions/float_array_binary_op_zero",
    "expressions/f32_array_binary_op_zero",
    "expressions/arithmetic_domain_wrapping_exit",
    "expressions/arithmetic_domain_saturating_exit",
    "control_flow/case_payload_shared_field_name_exit",
    "arithmetic/bare_name_scopes",
    "arithmetic/const_fold_overflow_compiles",
    "arithmetic/runtime_bitwise_high_ops_exit",
    "arithmetic/runtime_cast_sign_zero_extension_exit",
    "arithmetic/runtime_float32_array_conversion_exit",
    "arithmetic/runtime_float_nan_comparison_exit",
    "arithmetic/runtime_float_negative_ops_exit",
    "arithmetic/runtime_i64_signed_arithmetic_exit",
    "arithmetic/runtime_gcd_euclid_exit",
    "arithmetic/runtime_monte_carlo_pi_exit",
    "arithmetic/runtime_newton_sqrt_exit",
    "arithmetic/runtime_i64_min_literal_exit",
    "arithmetic/runtime_i64_to_u64_exact_guard_exit",
    "arithmetic/runtime_expression_range_bound_exit",
    "arithmetic/runtime_ranged_bitwise_and_mask_exit",
    "arithmetic/runtime_ranged_divide_modulo_chain_exit",
    "arithmetic/runtime_saturating_domain_exit",
    "arithmetic/runtime_signed_modulo_shift_edges_exit",
    "arithmetic/runtime_u64_max_literal_exit",
    "arithmetic/runtime_unsigned_high_comparison_exit",
    "constants/runtime_scoped_const_exit",
    "collections/runtime_activity_selection_greedy_exit",
    "collections/runtime_2d_transpose_exit",
    "collections/runtime_bfs_traversal_exit",
    "collections/runtime_binary_search_exit",
    "collections/runtime_bubble_sort_exit",
    "collections/runtime_coin_change_dp_exit",
    "collections/runtime_enum_grid_scan_exit",
    "collections/runtime_hash_table_exit",
    "collections/runtime_indexed_through_guard_chain_exit",
    "collections/runtime_matrix_multiply_exit",
    "collections/runtime_maze_pathfind_exit",
    "collections/runtime_nested_struct_array_field_exit",
    "collections/runtime_nqueens_backtracking_exit",
    "collections/runtime_ring_buffer_queue_exit",
    "collections/runtime_rpn_evaluator_exit",
    "collections/runtime_two_indexed_reads_binary_exit",
    "collections/runtime_two_pointer_palindrome_exit",
    "collections/runtime_indexed_read_then_guard_exit",
    "collections/runtime_indexed_struct_write_loop_exit",
    "collections/runtime_nested_array_const_index_exit",
    "collections/runtime_row_const_column_write_exit",
    "collections/runtime_rule90_automaton_exit",
    "collections/runtime_struct_field_temp_arith_exit",
    "collections/runtime_whole_array_value_copy_exit",
    "collections/runtime_whole_struct_value_copy_exit",
    "collections/std_option_runtime_match_exit",
    "collections/runtime_declared_range_index_read_exit",
    "collections/runtime_declared_range_index_write_exit",
    "collections/runtime_indexed_struct_field_operand_exit",
    "collections/runtime_indexed_struct_field_rmw_exit",
    "range/runtime_element_range_dataflow_exit",
    "range/runtime_funnel_guard_agreement_exit",
    "range/runtime_guarded_copy_narrowing_exit",
    "range/runtime_guarded_element_increment_exit",
    "range/runtime_guarded_runtime_index_increment_exit",
    "text/runtime_bounded_carrier_byte_write_exit",
    "text/runtime_carrier_byte_write_width_coercion",
    "text/runtime_utf16_literal_exit",
    "calls/runtime_slice_length_field_exit",
    "range/runtime_guarded_binary_operand_exit",
    "calls/runtime_machine_indexed_arg_exit",
    "calls/runtime_machine_indexed_struct_field_arg_exit",
    "collections/runtime_frame_indexed_param_read_exit",
    "collections/runtime_frame_indexed_param_operand_arg_exit",
    "collections/runtime_frame_indexed_param_field_exit",
    "collections/runtime_frame_indexed_local_read_exit",
    "collections/runtime_frame_indexed_byte_param_read_exit",
    "collections/runtime_machine_frame_index_read_exit",
    "collections/runtime_machine_frame_index_write_exit",
    "collections/runtime_machine_frame_index_dual_frame_write_exit",
    "collections/runtime_machine_frame_index_rmw_exit",
    "calls/runtime_machine_frame_index_arg_operand_exit",
    "collections/runtime_nested_const_row_indexed_read_exit",
    "collections/runtime_nested_const_row_struct_field_write_exit",
    "collections/runtime_nested_middle_index_3d_exit",
    "collections/runtime_let_bound_computed_index_exit",
    "collections/runtime_struct_field_operand_matrix_exit",
    "collections/runtime_struct_field_operand_param_exit",
    "collections/runtime_double_indexed_read_exit",
    "collections/runtime_nested_deep_const_prefix_exit",
    "collections/runtime_dual_frame_index_copy_exit",
    "collections/runtime_frame_mixed_index_pair_copy_exit",
    "collections/runtime_cross_region_indexed_pair_copy_exit",
    "collections/runtime_cross_region_double_indexed_pair_copy_exit",
    "collections/constant_nested_index_guard_exit",
    "collections/runtime_dual_mixed_index_copy_exit",
    "slices/runtime_slice_element_machine_roundtrip_exit",
    "slices/runtime_slice_element_runtime_index_read_exit",
    "slices/runtime_machine_bounded_subslice_local_exit",
    "slices/runtime_subslice_start_pointer_exit",
    "slices/runtime_end_fixed_array_subslice_local_exit",
    "slices/runtime_end_fixed_array_subslice_element_exit",
    "slices/guard_fixed_array_len_operand_exit",
    "slices/runtime_bounded_fixed_array_subslice_arg_exit",
    "text/runtime_bounded_carrier_concat_exit",
    "text/runtime_bounded_carrier_alias_concat_exit",
    "text/runtime_bounded_carrier_local_source_concat_exit",
    "text/runtime_value_call_slice_view_carrier_guard_exit",
    "calls/runtime_value_call_slice_view_element_arg_exit",
    "expressions/runtime_fixed_array_field_value_exit",
    "control_flow/fixed_array_element_guard",
    "control_flow/runtime_multi_field_payload_arith_exit",
    "control_flow/runtime_captured_local_remutated_field_exit",
    "control_flow/runtime_composite_initializer_local_arg_exit",
    "control_flow/runtime_captured_local_swap_exit",
    "control_flow/runtime_linear_search_early_exit",
    "control_flow/runtime_loop_patterns_exit",
    "control_flow/runtime_nested_loop_grid_sum_exit",
    "expressions/arithmetic_domain_saturating_div_mod_exit",
    "expressions/runtime_float_local_arithmetic_exit",
    "expressions/runtime_guard_divide_modulo_exit",
    "expressions/runtime_guard_divide_modulo_signedness_exit",
    "expressions/runtime_guard_negative_arithmetic_exit",
    "arithmetic/runtime_and_of_or_guard_exit",
    "arithmetic/runtime_cast_in_guard_exit",
    "arithmetic/runtime_comparison_signedness_exit",
    "arithmetic/runtime_domain_boundaries_exit",
    "arithmetic/runtime_guard_feature_composition_exit",
    "arithmetic/runtime_negated_boolean_nesting_guard_exit",
    "arithmetic/runtime_parenthesized_guard_subjects_exit",
    "arithmetic/runtime_saturating_narrow_add_sub_exit",
    "arithmetic/runtime_shift_in_guard_exit",
    "arithmetic/runtime_shift_signedness_exit",
    "arithmetic/runtime_float_compare_cast_exit",
    "arithmetic/runtime_float_operations_exit",
    "arithmetic/runtime_i64_divide_modulo_exit",
    "arithmetic/runtime_integer_casts_exit",
    "arithmetic/runtime_mixed_width_sign_exit",
    "arithmetic/runtime_narrow_signed_divide_guard_exit",
    "arithmetic/runtime_narrow_signed_guard_ops_exit",
    "arithmetic/runtime_narrow_signed_wrap_boundaries_exit",
    "arithmetic/runtime_saturating_narrow_divide_exit",
    "arithmetic/runtime_unsigned_high_bit_u32_ops_exit",
    "arithmetic/runtime_unsigned_min_max_exit",
    "data/runtime_data_properties_exit",
    "domains/bodyless_domain_declarations_exit",
    "domains/domain_field_write_then_read_exit",
    "domains/executable_domain_membership_intersection_value_exit",
    "domains/executable_imported_domain_membership_intersection_guard_exit",
    "domains/executable_imported_domain_membership_intersection_value_exit",
    "domains/executable_imported_domain_membership_union_guard_exit",
    "domains/executable_imported_domain_membership_union_value_exit",
    "domains/utf8_field_read_carries_domain_exit",
    "expressions/runtime_flat_boolean_logic_exit",
    "expressions/runtime_literal_source_cast_exit",
    "traits/runtime_conformance_item_exit",
    "arithmetic/runtime_saturating_expression_domain_exit",
    "arithmetic/runtime_saturating_param_carry_exit",
    "arithmetic/runtime_saturating_wide_boundaries_exit",
    "arithmetic/runtime_wrapping_expression_guard_exit",
    "control_flow/runtime_boolean_transition_argument_after_string_guard_exit",
    "control_flow/runtime_direct_boolean_transition_argument_exit",
    "control_flow/runtime_local_boolean_conjunction_value_exit",
    "control_flow/runtime_local_boolean_transition_argument_exit",
    "control_flow/runtime_local_scalar_comparison_value_exit",
    "control_flow/runtime_local_string_comparison_value_exit",
    "arithmetic/float_to_int_saturating_exit",
    "arithmetic/float_to_int_unsigned_narrow_saturating_exit",
    "arithmetic/runtime_divide_min_edge_guard_exit",
    "arithmetic/runtime_nested_unsigned_witness_exit",
    "calls/runtime_assignment_call_post_mutation_value_exit",
    "calls/runtime_min_guard_true_false_pair_exit",
    "calls/runtime_min_max_guard_subject_hoist_exit",
    "calls/runtime_transition_subject_call_single_evaluation_exit",
    "calls/runtime_value_call_single_execution_exit",
    "control_flow/runtime_effectful_subject_single_evaluation_exit",
    "arithmetic/runtime_u64_literal_let_guard_exit",
    "calls/runtime_call_in_inlined_substate_exit",
    "calls/runtime_contained_machine_exit",
    "calls/runtime_deep_state_name_collision_exit",
    "calls/runtime_dispatch_binary_call_argument_exit",
    "calls/runtime_looping_cast_return_exit",
    "calls/runtime_looping_value_return_exit",
    "calls/runtime_multiarm_same_named_locals_exit",
    "calls/runtime_nested_value_call_in_substate_exit",
    "calls/runtime_value_call_self_field_enum_match_exit",
    "calls/runtime_dispatch_result_binary_terminal_exit",
    "calls/runtime_dispatch_result_multi_arm_exit",
    "calls/runtime_dispatch_result_guard_subject_exit",
    "calls/runtime_dispatch_result_transition_arg_exit",
    "calls/runtime_dispatched_effectful_reentrant_exit",
    "calls/runtime_dispatch_result_field_terminal_exit",
    "calls/runtime_nested_called_machine_loop_exit",
    "calls/runtime_value_callee_post_entry_lets_exit",
    "calls/runtime_post_entry_deep_chain_exit",
    "calls/runtime_post_entry_chained_let_exit",
    "calls/by_value_case_param_self_write_exit",
    "calls/runtime_attached_machine_struct_arg_exit",
    "calls/runtime_free_machine_looping_value_call_exit",
    "calls/runtime_free_machine_struct_arg_exit",
    "calls/runtime_free_machine_struct_return_exit",
    "calls/runtime_free_machine_value_call_mut_arg_exit",
    "calls/runtime_free_machine_value_call_exit",
    "calls/runtime_record_forwarding_statement_call_exit",
    "calls/runtime_value_call_let_combine_exit",
    "calls/runtime_multi_arm_value_transition_exit",
    "calls/runtime_value_transition_unsigned_guard_exit",
    "calls/runtime_value_position_branching_call_exit",
    "comptime/runtime_const_array_length_bare_call_arm_exit",
    "comptime/runtime_const_array_length_exit",
    "comptime/runtime_const_array_length_transitive_exit",
    "domains/utf8_return_view_equals_exit",
    "borrow/runtime_method_view_write_after_last_use_exit",
    "borrow/runtime_view_of_view_chain_exit",
    "expressions/runtime_16bit_cast_exit",
    "expressions/runtime_fixed_array_field_guard_exit",
    "expressions/runtime_call_result_binary_operand_exit",
    "expressions/runtime_cast_operand_exit",
    "expressions/runtime_f32_arithmetic_exit",
    "expressions/runtime_float_arithmetic_exit",
    "expressions/runtime_float_comparison_exit",
    "expressions/runtime_float_place_comparison_exit",
    "expressions/runtime_numeric_cast_exit",
    "expressions/runtime_widened_bitwise_exit",
    "expressions/runtime_widened_comparison_exit",
    "operators/integer_literal_suffix_exit",
    "operators/runtime_bitwise_guard_exit",
    "operators/runtime_bitwise_operators_exit",
    "operators/runtime_popcount_loop_exit",
    "operators/runtime_shift_operators_exit",
    "operators/runtime_xorshift_prng_exit",
    "text/runtime_base64_encode_exit",
    "text/runtime_binary_format_exit",
    "text/runtime_bounded_carrier_pointee_guard_exit",
    "text/runtime_bounded_carrier_slice_field_write_exit",
    "text/runtime_bounded_carrier_write_line_exit",
    "text/runtime_carrier_cipher_exit",
    "text/runtime_carrier_fnv_loop_exit",
    "text/runtime_carrier_indexed_const_write_exit",
    "text/runtime_carrier_indexed_read_operand_exit",
    "text/runtime_carrier_indexed_write_exit",
    "text/runtime_carrier_itoa_exit",
    "text/runtime_carrier_len_guard_exit",
    "text/runtime_crc32_exit",
    "text/runtime_decimal_to_number_exit",
    "text/runtime_mandelbrot_render_exit",
    "text/runtime_number_to_decimal_exit",
    "text/runtime_run_length_encode_exit",
    "text/runtime_string_palindrome_exit",
    "text/runtime_substring_search_exit",
    "text/runtime_text_builder",
    "text/runtime_stderr_write_exit",
    "time/runtime_duration_core_exit",
    "time/runtime_duration_totals_exit",
    "time/runtime_instant_elapsed_exit",
    "time/runtime_system_time_after_2026_exit",
    "time/runtime_time_elapsed_since_exit",
    "time/runtime_checked_time_arith_exit",
    "time/runtime_sleep_for_exit",
    "termination/runtime_shrinking_slice_recursion_exit",
    "traits/runtime_equatable_scalar_not_equals_guard_exit",
    "data/runtime_case_membership_mixed_shape_exit",
    "versioning/runtime_version_migration_exit",
    "versioning/runtime_versioned_match_zii_exit",
    "versioning/runtime_versioned_three_era_match_zii_exit",
    "wire/runtime_wire_roundtrip_repeated_max_one_exit",
    "wire/runtime_wire_roundtrip_utf8_exit",
    "wire/runtime_wire_utf8_edge_verdicts_exit",
    "wire/runtime_wire_utf8_invalid_refused_exit",
    "wire/runtime_wire_schema_as_value_type_exit",
    "wire/runtime_wire_decode_let_compare_exit",
    "wire/runtime_wire_encode_repeated_then_string_exit",
    "wire/runtime_wire_roundtrip_nested_and_repeated_exit",
    "wire/runtime_wire_encode_primitive_exit",
    "wire/runtime_wire_encode_era_discriminator_exit",
    "wire/runtime_wire_roundtrip_primitive_exit",
    "wire/runtime_wire_decode_ranged_field_exit",
    "wire/runtime_wire_decode_ranged_repeated_exit",
    "wire/runtime_wire_decode_rejects_noncanonical_bool_exit",
    "wire/runtime_wire_decode_rejects_noncanonical_varint_exit",
    "wire/runtime_wire_decode_rejects_scalar_width_overflow_exit",
    "wire/runtime_wire_roundtrip_nested_exit",
    "wire/runtime_wire_decode_rejects_bad_nested_length_exit",
    "wire/runtime_wire_roundtrip_repeated_exit",
    "wire/runtime_wire_decode_rejects_repeated_overflow_exit",
    "wire/runtime_wire_decode_rejects_wrong_era_exit",
    "wire/runtime_wire_encode_string_exit",
    "wire/runtime_wire_encode_byte_slice_exit",
    "wire/runtime_wire_encode_borrowed_scalar_slice_exit",
    "wire/runtime_wire_decode_byte_slice_exit",
    "wire/runtime_wire_decoded_byte_slice_index_exit",
    "wire/runtime_wire_decoded_byte_slice_len_exit",
    "layouts/runtime_plan_laid_value_field_exit",
    "layouts/runtime_plan_laid_compact_bits_exit",
    "layouts/runtime_plan_laid_erased_field_exit",
    "layouts/runtime_plan_laid_integer_at_projection_exit",
    "layouts/runtime_plan_laid_integer_at_proved_write_exit",
    "layouts/runtime_plan_laid_integer_at_total_write_exit",
    "layouts/runtime_plan_laid_value_by_value_param_exit",
    "layouts/runtime_plan_laid_record_view_exit",
    "layouts/runtime_plan_laid_fixed_array_view_exit",
    "layouts/runtime_plan_laid_fixed_array_mutable_write_exit",
    "layouts/runtime_plan_laid_nested_fixed_array_mutable_write_exit",
    "layouts/runtime_plan_laid_nested_record_mutable_write_exit",
    "layouts/runtime_plan_laid_record_array_mutable_write_exit",
    "layouts/runtime_plan_laid_record_mutable_write_exit",
    "recast/constant_offset_record_view_after_write_exit",
    "recast/runtime_aggregate_slice_representation_recast_exit",
    "recast/runtime_bool_representation_recast_exit",
    "recast/runtime_fixed_array_view_mutable_write_exit",
    "recast/runtime_float_range_representation_recast_exit",
    "recast/runtime_guarded_offset_recast_exit",
    "recast/runtime_interior_byte_recast_exit",
    "recast/runtime_interior_slice_view_mutable_write_exit",
    "recast/runtime_multi_edge_offset_meet_exit",
    "recast/runtime_mutable_equivalent_domain_recast_exit",
    "recast/runtime_mutable_equivalent_range_recast_exit",
    "recast/runtime_mutable_equivalent_record_recast_exit",
    "recast/runtime_offset_byte_recast_exit",
    "recast/runtime_offset_byte_recast_mutable_write_exit",
    "recast/runtime_record_array_view_mutable_write_exit",
    "recast/runtime_record_view_assignment_binary_exit",
    "recast/runtime_record_view_exit",
    "recast/runtime_scalar_pun_mutable_write_exit",
    "recast/runtime_scalar_pun_shared_let_exit",
    "recast/runtime_shared_domain_weakening_recast_exit",
    "recast/runtime_shared_record_float_range_weakening_exit",
    "recast/runtime_slice_view_mutable_write_exit",
    "recast/runtime_symbolic_stride_footprint_exit",
];

// These deployable native probes assert target-specific provider semantics.
// Cross-compile their exact authored root on every development host instead
// of selecting the development host or substituting the legacy entry seam.
const ROOTED_TARGET_BACKEND_PASS_CANARIES: &[(&str, &str)] = &[
    ("filesystem/windows_raw_breadth_exit", "windows_x86_64"),
    ("filesystem/windows_raw_roundtrip_exit", "windows_x86_64"),
    ("host/runtime_user32_key_state_exit", "windows_x86_64"),
    ("time/runtime_time_host_native_exit", "windows_x86_64"),
    ("time/runtime_time_host_native_darwin_exit", "macos_arm64"),
    ("providers/external_leaf_syscall_compile", "linux_x86_64"),
    ("providers/external_leaf_syscall_compile", "linux_arm64"),
    ("providers/external_leaf_dllimport_compile", "macos_arm64"),
    ("providers/runtime_import_call_argument_exit", "macos_arm64"),
    (
        "capabilities/windows_provides_import_exit",
        "windows_x86_64",
    ),
    ("host/runtime_gui_memory_dc_blit_exit", "windows_x86_64"),
    ("filesystem/windows_canonicalize_exit", "windows_x86_64"),
];

fn check_canary(canary_dir: &Path) -> Result<(), Vec<Diagnostic>> {
    compile_to_checked(&canary_dir.join("main.omg"), None).map(|_| ())
}

static CANARY_UMBRELLA_LOCK: Mutex<()> = Mutex::new(());

const DEFAULT_CANARY_OUTER_JOB_CAP: usize = 12;

fn configured_canary_worker_count(
    variable: &str,
    configured: Option<String>,
    default: usize,
) -> usize {
    configured
        .map(|value| {
            value
                .parse::<usize>()
                .ok()
                .filter(|count| *count > 0)
                .unwrap_or_else(|| panic!("{variable} must be a positive integer, got {value:?}"))
        })
        .unwrap_or(default)
}

fn default_canary_outer_job_count(available_parallelism: usize) -> usize {
    available_parallelism.clamp(1, DEFAULT_CANARY_OUTER_JOB_CAP)
}

/// Run independent corpus members with bounded outer parallelism. Backend
/// compiles share the same single production route. Results return in source
/// order so diagnostics remain deterministic.
fn run_bounded_canary_jobs<T, R>(items: &[T], worker: impl Fn(&T) -> R + Sync) -> Vec<R>
where
    T: Sync,
    R: Send,
{
    if items.is_empty() {
        return Vec::new();
    }
    let default_jobs = default_canary_outer_job_count(
        thread::available_parallelism()
            .map(usize::from)
            .unwrap_or(1),
    );
    let requested_jobs = configured_canary_worker_count(
        "OMEGA_CANARY_JOBS",
        std::env::var("OMEGA_CANARY_JOBS").ok(),
        default_jobs,
    );
    let job_count = requested_jobs.min(items.len());
    if job_count == 1 {
        return items.iter().map(worker).collect();
    }

    let next = AtomicUsize::new(0);
    thread::scope(|scope| {
        let (sender, receiver) = mpsc::channel();
        for _ in 0..job_count {
            let sender = sender.clone();
            let worker = &worker;
            let next = &next;
            scope.spawn(move || {
                loop {
                    let index = next.fetch_add(1, Ordering::Relaxed);
                    let Some(item) = items.get(index) else {
                        break;
                    };
                    sender
                        .send((index, worker(item)))
                        .expect("canary result receiver dropped");
                }
            });
        }
        drop(sender);
        let mut results = receiver.into_iter().collect::<Vec<_>>();
        results.sort_by_key(|(index, _)| *index);
        results.into_iter().map(|(_, result)| result).collect()
    })
}

#[test]
fn canary_parallelism_defaults_and_overrides_are_pinned() {
    assert_eq!(default_canary_outer_job_count(14), 12);
    assert_eq!(default_canary_outer_job_count(4), 4);
    assert_eq!(default_canary_outer_job_count(0), 1);
    assert_eq!(configured_canary_worker_count("TEST_WORKERS", None, 8), 8);
    assert_eq!(
        configured_canary_worker_count("TEST_WORKERS", Some("3".to_owned()), 8),
        3
    );
    assert!(
        std::panic::catch_unwind(|| configured_canary_worker_count(
            "TEST_WORKERS",
            Some("0".to_owned()),
            8,
        ))
        .is_err()
    );
    assert!(
        std::panic::catch_unwind(|| configured_canary_worker_count(
            "TEST_WORKERS",
            Some("many".to_owned()),
            8,
        ))
        .is_err()
    );
}

/// A build dir no other concurrent compile can collide with: process id plus a
/// process-wide counter (parallel test threads share the process, so the id alone
/// is not unique).
fn unique_no_output_build_dir() -> PathBuf {
    use std::sync::atomic::{AtomicU64, Ordering};
    static NEXT_BUILD_DIR: AtomicU64 = AtomicU64::new(0);
    let unique = NEXT_BUILD_DIR.fetch_add(1, Ordering::Relaxed);
    std::env::temp_dir().join(format!(
        "omega-canary-no-output-{}-{unique}",
        std::process::id()
    ))
}

fn repo_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .ancestors()
        .nth(4)
        .expect("compiler crate should live under omega-rust/omega/compiler/compiler")
        .to_path_buf()
}

fn fixture_declares_ordinary_std(project_root: &Path) -> bool {
    fs::read_to_string(project_root.join("build.omg")).is_ok_and(|build| {
        build.contains("builder.depend(Source::Path") && build.contains("source/library/std")
    })
}

fn fixture_package_identity(marker: u8) -> PackageKeyIdentity {
    PackageKeyIdentity::from_digest([marker; 32])
        .expect("repository fixture package identity is nonzero")
}

fn repository_fixture_package_inputs(root_path: &Path) -> Option<PackageCompilationInputs> {
    let project_root = root_path
        .parent()
        .expect("fixture source has a project root");
    if !fixture_declares_ordinary_std(project_root) {
        return None;
    }

    let declaration = extract_build_declaration(project_root)
        .unwrap_or_else(|error| panic!("fixture {}: {error}", project_root.display()));
    let root_role = declaration.kind();
    let root_name = match declaration {
        BuildDeclaration::Application(application) => application.name,
        BuildDeclaration::Package(package) => package.name,
        BuildDeclaration::Workspace(_) => {
            panic!(
                "fixture {} cannot be a workspace root",
                project_root.display()
            )
        }
    };
    let root_identity = fixture_package_identity(1);
    let standard_library_identity = fixture_package_identity(2);
    let packages = vec![
        PackageSourceBinding::new(
            root_identity,
            root_name.into_string(),
            project_root.to_path_buf(),
        ),
        PackageSourceBinding::new(
            standard_library_identity,
            "omega-language-std",
            repo_root().join("source/library/std"),
        ),
    ];
    let dependencies = vec![PackageDependencyBinding::new(
        root_identity,
        "omega_language_std",
        standard_library_identity,
    )];

    Some(
        PackageCompilationInputs::new(root_identity, root_role, packages, dependencies)
            .unwrap_or_else(|errors| panic!("fixture {}: {errors:#?}", project_root.display())),
    )
}

fn fixture_accepts_filesystem_service(root_path: &Path) -> bool {
    fs::read_to_string(root_path).is_ok_and(|source| {
        source.contains("omega_language_std::filesystem")
            || source.contains("omega_language_std::filesystem_host")
    })
}

fn fixture_accepts_console_exit(root_path: &Path) -> bool {
    fs::read_to_string(root_path).is_ok_and(|source| {
        source.contains("omega_language_std::console") && source.contains(".exit_process(")
    })
}

fn fixture_accepts_console_output(root_path: &Path) -> bool {
    fs::read_to_string(root_path).is_ok_and(|source| {
        source.contains("omega_language_std::console") && source.contains(".write_byte(")
    })
}

fn fixture_accepts_console_input(root_path: &Path) -> bool {
    fs::read_to_string(root_path).is_ok_and(|source| {
        source.contains("omega_language_std::console") && source.contains(".read_byte(")
    })
}

fn candidate_console_exit_binding(
    checked: &CheckedCompilation,
    accepts_console_output: bool,
    accepts_console_input: bool,
) -> Result<AcceptedSemanticBinding, Vec<Diagnostic>> {
    let standard_library = fixture_package_identity(2);
    let candidates = checked
        .selected_provider_plans()
        .plans()
        .iter()
        .zip(checked.selected_provider_provenance())
        .filter(|(plan, provenance)| {
            plan.schema.trait_name == "Console"
                && plan.rows.iter().any(|row| row.method == "exit_process")
                && checked
                    .typed
                    .symbols
                    .symbol_package_identity(provenance.provider.schema.symbol())
                    == Some(standard_library)
        })
        .collect::<Vec<_>>();
    let [(plan, provenance)] = candidates.as_slice() else {
        return Err(vec![Diagnostic::error(format!(
            "repository fixture Console exit acceptance resolved {} exact std provider plans instead of one",
            candidates.len()
        ))]);
    };
    let declaration_path = checked
        .typed
        .symbols
        .display_path(provenance.provider.schema.symbol(), "::");
    let binding = AcceptedSemanticBinding::new(
        AcceptedSemanticBindingRole::ConsoleExitProcessI32,
        standard_library,
        declaration_path,
        plan.schema.identity_digest(),
        plan.identity_digest(),
    )
    .map_err(|error| {
        vec![Diagnostic::error(format!(
            "cannot construct repository fixture Console exit binding: {error}"
        ))]
    })?;
    let mut permissions = plan
        .schema
        .methods
        .iter()
        .filter(|method| {
            method.name == "exit_process"
                || (accepts_console_output && method.name == "write_byte")
                || (accepts_console_input && method.name == "read_byte")
        })
        .map(|method| {
            effects::ServiceTerminalAuthorityPermission::new(
                plan.schema.identity_digest(),
                method.requirement_identity.clone(),
                effects::TerminalAuthorityDisposition::from_classes(match method.name.as_str() {
                    "exit_process" => {
                        vec![effects::TerminalAuthorityClass::ProcessTermination]
                    }
                    "write_byte" => {
                        vec![effects::TerminalAuthorityClass::ProcessOutput]
                    }
                    "read_byte" => {
                        vec![effects::TerminalAuthorityClass::ProcessInput]
                    }
                    _ => unreachable!("filtered above"),
                }),
            )
        })
        .collect::<Vec<_>>();
    permissions.sort_by(|left, right| {
        left.requirement_identity()
            .cmp(right.requirement_identity())
    });
    binding
        .with_terminal_authority_permissions(permissions)
        .map_err(|error| {
            vec![Diagnostic::error(format!(
                "cannot attach repository fixture Console terminal permissions: {error}"
            ))]
        })
}

fn reviewed_repository_fixture_package_inputs(
    root_path: &Path,
    target_name: Option<&str>,
) -> Result<Option<PackageCompilationInputs>, Vec<Diagnostic>> {
    let Some(package_inputs) = repository_fixture_package_inputs(root_path) else {
        return Ok(None);
    };
    let accepts_filesystem = fixture_accepts_filesystem_service(root_path);
    let accepts_console_exit = fixture_accepts_console_exit(root_path);
    let accepts_console_output = fixture_accepts_console_output(root_path);
    let accepts_console_input = fixture_accepts_console_input(root_path);
    if !accepts_filesystem
        && !accepts_console_exit
        && !accepts_console_output
        && !accepts_console_input
    {
        return Ok(Some(package_inputs));
    }

    // These canaries explicitly exercise and accept dangerous standard-library
    // services. Source spelling selects only this repository's test policy;
    // every admitted row is then derived from and replayed against the exact
    // preliminary checked graph. This is not evidence that an audit occurred
    // and is not production accepted-lock recovery.
    let preliminary =
        compile_to_checked_with_packages(root_path, target_name, package_inputs.clone())?;
    let mut bindings = Vec::new();
    if accepts_filesystem {
        bindings.push(
            preliminary
                .candidate_service_binding(
                    AcceptedSemanticBindingRole::FilesystemHostService,
                    fixture_package_identity(2),
                    "FilesystemHost",
                )
                .map_err(|diagnostic| vec![diagnostic])?,
        );
    }
    if accepts_console_exit || accepts_console_output || accepts_console_input {
        bindings.push(candidate_console_exit_binding(
            &preliminary,
            accepts_console_output,
            accepts_console_input,
        )?);
    }
    package_inputs
        .with_accepted_semantic_bindings(bindings)
        .map(Some)
        .map_err(|errors| {
            vec![Diagnostic::error(format!(
                "cannot admit repository fixture semantic binding: {errors:?}"
            ))]
        })
}

fn compile_to_checked(
    root_path: &Path,
    target_name: Option<&str>,
) -> Result<CheckedCompilation, Vec<Diagnostic>> {
    match reviewed_repository_fixture_package_inputs(root_path, target_name)? {
        Some(package_inputs) => {
            compile_to_checked_with_packages(root_path, target_name, package_inputs)
        }
        None => compile_standalone_to_checked(root_path, target_name),
    }
}

fn sample_project(path: &str) -> PathBuf {
    repo_root().join("samples").join(path)
}

fn pass_canary(path: &str) -> PathBuf {
    repo_root().join("tests/omega/pass").join(path)
}

fn fail_canary(path: &str) -> PathBuf {
    repo_root().join("tests/omega/fail").join(path)
}

fn hosted_main_program_entry_build(target: &str) -> String {
    let root_owner = hosted_program_entry_owner(target);
    format!(
        "machine build(builder: &mut Build) {{\n    builder.application(\"hosted-main-program-entry\");\n    builder.roots.bind({root_owner}::ProgramEntry, Main::main);\n}}\n"
    )
}

fn hosted_main_program_entry_build_with_std(target: &str) -> String {
    let root_owner = hosted_program_entry_owner(target);
    let standard_library = repo_root()
        .join("source/library/std")
        .to_string_lossy()
        .replace('\\', "/");
    format!(
        "machine build(builder: &mut Build) {{\n    builder.application(\"hosted-main-program-entry\");\n    builder.depend(Source::Path {{\n        location: \"{standard_library}\"\n    }});\n    builder.roots.bind({root_owner}::ProgramEntry, Main::main);\n}}\n"
    )
}

fn hosted_program_entry_owner(target: &str) -> &'static str {
    match target {
        "windows_x86_64" => "windows_x86_64",
        "linux_x86_64" => "linux_x86_64",
        "macos_arm64" => "macos_arm64",
        "linux_arm64" => "linux_arm64",
        _ => panic!("no hosted ProgramEntry root owner for target `{target}`"),
    }
}

fn compile_single_file_hosted_main(
    canary: &Path,
    scratch: &Path,
    target: &str,
) -> Result<CompileReport, Vec<Diagnostic>> {
    let _ = fs::remove_dir_all(scratch);
    let source = scratch.join("source");
    fs::create_dir_all(&source).expect("create exact-entry hosted source directory");
    fs::copy(canary.join("main.omg"), source.join("main.omg"))
        .expect("copy single-file hosted canary");
    let build = if fixture_declares_ordinary_std(canary) {
        hosted_main_program_entry_build_with_std(target)
    } else {
        hosted_main_program_entry_build(target)
    };
    fs::write(source.join("build.omg"), build).expect("write exact hosted ProgramEntry binding");
    production_compile(CanaryCompileSpec {
        root_path: source.join("main.omg"),
        build_dir: Some(scratch.join("out")),
        target_name: Some(target.into()),
        product: CanaryCompileProduct::NativeArtifactAndPublish,
    })
}

#[cfg(all(target_os = "windows", target_arch = "x86_64"))]
fn native_hosted_target() -> &'static str {
    "windows_x86_64"
}

#[cfg(all(target_os = "linux", target_arch = "x86_64"))]
fn native_hosted_target() -> &'static str {
    "linux_x86_64"
}

#[cfg(all(target_os = "linux", target_arch = "aarch64"))]
fn native_hosted_target() -> &'static str {
    "linux_arm64"
}

#[cfg(all(target_os = "macos", target_arch = "aarch64"))]
fn native_hosted_target() -> &'static str {
    "macos_arm64"
}

fn pending_canary(path: &str) -> PathBuf {
    repo_root().join("tests/omega/pending").join(path)
}

fn run_canary(path: &str) -> PathBuf {
    repo_root().join("tests/omega/run").join(path)
}

#[cfg(not(windows))]
fn copy_dir_recursive(from: &Path, to: &Path) -> std::io::Result<()> {
    fs::create_dir_all(to)?;
    for entry in fs::read_dir(from)? {
        let entry = entry?;
        let file_type = entry.file_type()?;
        let destination = to.join(entry.file_name());
        if file_type.is_dir() {
            copy_dir_recursive(&entry.path(), &destination)?;
        } else if file_type.is_file() {
            fs::copy(entry.path(), destination)?;
        }
    }
    Ok(())
}

#[cfg(windows)]
fn executable_name() -> &'static str {
    "omega-program.exe"
}

#[cfg(not(windows))]
fn executable_name() -> &'static str {
    "omega-program"
}

#[test]
fn boundary_equality_recast_witness_compiles_to_checked_trees() {
    let canary = pass_canary(fixture_roster::BOUNDARY_EQUALITY_RECAST_WITNESS_COMPILE);
    compile_to_checked(&canary.join("main.omg"), None)
        .expect("boundary equality/recast witness should reach checked trees");
}

#[test]
fn task_runtime_machine_selection_builds_omega_activation_sidecar() {
    let canary = pass_canary(fixture_roster::TASK_RUNTIME_MACHINE_SELECTION_COMPILE);
    let checked = compile_to_checked(&canary.join("main.omg"), None)
        .expect("core task-runtime machine selection should reach checked trees");
    let activations = checked.task_activations().as_slice();

    assert_eq!(
        activations.len(),
        2,
        "the authored start and try_start calls must each elaborate one activation"
    );
    assert!(
        activations.iter().any(|activation| matches!(
            activation.operation,
            task_plans::TaskStartOperation::Start
        ))
    );
    assert!(activations.iter().any(|activation| matches!(
        activation.operation,
        task_plans::TaskStartOperation::TryStart
    )));
    for activation in activations {
        let target = checked
            .machines()
            .iter()
            .find(|machine| machine.symbol == activation.target_machine)
            .expect("activation target machine should survive specialization");
        assert_eq!(target.name.as_str(), "Worker::run");
        assert!(activation.plan.candidate().may_suspend);
        assert!(!activation.plan.candidate().may_block);
        assert_ne!(
            activation.plan.normalized_identity().normalized_identity(),
            0,
            "the complete selected activation demand must have a public identity"
        );
        assert_eq!(
            activation.selected_runtime.provider_plan_name,
            "CanaryTaskRuntime::satisfies::TaskRuntime"
        );
        assert_eq!(
            activation.selected_runtime.runtime.normalized_identity(),
            checked
                .selected_provider_plans()
                .plans()
                .first()
                .expect("selected TaskRuntime provider plan")
                .report_fingerprint()
        );
        assert!(
            activation
                .selected_runtime
                .requirement_identity
                .contains("TaskRuntime")
        );
    }

    // Runtime-instance dispatch/lowering is a later rung. Exercise the exact
    // Omega sidecar directly instead of pretending the canary provider's
    // placeholder intrinsic is an executable backend implementation.
    let manifest =
        visualizations::task_activation_manifest_json(&checked, checked.task_activations());
    let carry_manifest = visualizations::carry_manifest_json(&checked);
    assert!(manifest.contains("\"operation\": \"start\""));
    assert!(manifest.contains("\"operation\": \"try_start\""));
    assert!(manifest.contains("\"start_requirement\": \"TaskRuntime::start\""));
    assert!(manifest.contains("\"start_requirement\": \"TaskRuntime::try_start\""));
    assert_eq!(
        manifest
            .matches("\"target_machine\": \"Worker::run\"")
            .count(),
        2
    );
    assert_eq!(manifest.matches("\"may_suspend\": true").count(), 2);
    assert_eq!(manifest.matches("\"may_block\": false").count(), 2);
    assert_eq!(manifest.matches("\"activation_plan_id\": \"0x").count(), 2);
    assert_eq!(manifest.matches("\"selected_runtime\": {").count(), 2);
    assert_eq!(
        manifest
            .matches("\"provider_plan\": \"CanaryTaskRuntime::satisfies::TaskRuntime\"")
            .count(),
        2
    );
    assert_eq!(
        manifest
            .matches("\"canonical_suspension_crossings\": [")
            .count(),
        2
    );
    assert_eq!(manifest.matches("\"stack_plan\": {\"bytes\":").count(), 2);
    assert_eq!(
        manifest
            .matches("\"cpu_thread_preservation\": {\"preserve_cpu\":")
            .count(),
        2
    );
    assert!(!manifest.contains("\"runtime_admission\""));
    assert!(!manifest.contains("\"asynchronous_migration\""));
    assert!(carry_manifest.contains("\"safe_point_crossings\": [\n    {"));
    assert!(carry_manifest.contains("\"machine\": \"Worker::run\""));
    assert!(carry_manifest.contains("\"target\": \"Sleeper::park\""));
    assert!(carry_manifest.contains("\"storage\": \"call_argument\""));
    assert!(carry_manifest.contains("\"storage\": \"local\""));
}

const ACTIVE_PASS_CANARIES: &[&str] = &[
    "calls/runtime_referenced_local_outlives_sibling_guard_call_exit",
    "control_flow/runtime_tuple_transition_exit",
    "errors/runtime_result_match_exit",
    "expressions/runtime_enum_match_breadth_exit",
    "expressions/runtime_f64_state_arg_exit",
    "slices/runtime_field_array_element_value_operand_exit",
    "slices/runtime_indexed_rmw_temp_exit",
    "slices/runtime_indexed_struct_field_write_exit",
    "slices/runtime_indexed_write_adjacent_field_exit",
    "slices/runtime_indexed_write_const_read_exit",
    "slices/runtime_join_meet_bound_exit",
    "slices/runtime_local_aggregate_into_let_exit",
    "slices/runtime_local_slice_len_comparison_value_exit",
    "slices/runtime_slice_fixed_index_guard_exit",
    "slices/runtime_slice_index_transition_exit",
    "slices/runtime_slice_iteration_exit",
    "slices/runtime_slice_len_transition_exit",
    "storage/runtime_dispatch_local_index_binary_write_exit",
    "storage/runtime_machine_owned_indexed_nested_room_copy_exit",
    "storage/runtime_slice_alias_indexed_field_write_exit",
    "text/runtime_case_payload_domain_forward_exit",
    "text/runtime_param_domain_forward_exit",
    "traits/runtime_dyn_single_impl_dispatch_exit",
    "traits/runtime_local_named_dyn_devirtualized_exit",
    "traits/runtime_dyn_two_impl_dispatch_exit",
    "traits/runtime_dyn_two_impl_dispatch_swapped_exit",
    "traits/runtime_ref_param_method_dispatch_exit",
    "build/runtime_main_source_builder_is_ordinary_exit",
    "calls/runtime_alias_indexed_read_through_transition_exit",
    "calls/runtime_alias_write_through_guarded_transition_exit",
    "calls/runtime_call_result_through_reference_field_exit",
    "calls/runtime_nested_field_terminal_second_instance_exit",
    "calls/runtime_nested_local_terminal_second_instance_exit",
    "calls/runtime_reference_param_forwarded_through_loop_exit",
    "calls/runtime_value_call_through_alias_in_dispatch_exit",
    "control_flow/runtime_state_loop_indexed_search_exit",
    "control_flow/runtime_statement_call_single_execution_exit",
    "expressions/borrow_carrying_data_field_exit",
    "host/runtime_write_no_newline_exit",
    "time/runtime_value_machine_receiver_field_postentry_exit",
    "calls/runtime_dispatch_float_terminal_exit",
    "calls/runtime_dispatch_slice_element_terminal_exit",
    "calls/runtime_let_local_nested_state_arg_exit",
    "calls/runtime_multiarm_texteq_local_exit",
    "calls/runtime_nested_inline_chain_result_exit",
    "calls/runtime_nonentry_inline_second_receiver_exit",
    "calls/runtime_param_forward_chain_second_receiver_exit",
    "calls/runtime_param_receiver_second_instance_exit",
    "calls/runtime_param_receiver_single_instance_exit",
    "calls/runtime_pre_guard_texteq_local_arg_forward_exit",
    "calls/runtime_pre_guard_texteq_local_guard_exit",
    "calls/runtime_same_type_second_receiver_mutation_exit",
    "calls/runtime_value_call_slice_len_guard_exit",
    "collections/runtime_dutch_flag_partition_exit",
    "references/runtime_nested_receiver_same_type_exit",
    "calls/runtime_called_machine_loop_search_exit",
    "calls/runtime_computed_transition_args_exit",
    "calls/runtime_cross_machine_substate_name_exit",
    "calls/runtime_dispatch_machine_array_slice_arg_exit",
    "calls/runtime_dispatch_result_alias_read_exit",
    "calls/runtime_dispatch_result_enum_case_exit",
    "calls/runtime_dispatch_result_field_binding_exit",
    "calls/runtime_dispatch_second_receiver_exit",
    "calls/runtime_nonentry_second_receiver_exit",
    "calls/runtime_option_value_call_exit",
    "calls/runtime_selfcall_chain_second_receiver_exit",
    "calls/runtime_struct_by_value_param_exit",
    "calls/runtime_struct_value_call_exit",
    "calls/runtime_value_call_composition_exit",
    "calls/runtime_value_call_to_array_element_exit",
    "filesystem/discarded_self_call_literal_errno_exit",
    "filesystem/field_receiver_method_exit",
    "filesystem/self_value_call_literal_path_exit",
    "filesystem/wrapper_open_with_exit",
    "filesystem/wrapper_param_shadow_exit",
    "host/runtime_console_byte_echo_exit",
    "time/runtime_saturating_time_arith_exit",
    "traits/runtime_typed_two_method_receivers_exit",
    "types/runtime_i16_signed_arith_exit",
    "types/runtime_i64_signed_arith_exit",
    "types/runtime_i8_signed_arith_exit",
    "types/runtime_u16_field_arith_exit",
    "types/runtime_u8_field_arith_exit",
    "wire/runtime_wire_policy_authored_nested_exit",
    "wire/runtime_wire_policy_authored_plan_exit",
    "calls/runtime_dispatch_sibling_value_calls_exit",
    "calls/runtime_inline_repeated_receiver_value_calls_exit",
    "calls/runtime_value_call_struct_literal_arms_exit",
    "calls/runtime_value_call_struct_result_to_target_exit",
    "collections/runtime_case_array_element_write_exit",
    "collections/runtime_indexed_field_local_operand_exit",
    "collections/runtime_indexed_guard_true_false_pair_exit",
    "collections/runtime_indexed_local_bitwise_exit",
    "collections/runtime_indexed_local_compare_exit",
    "domains/runtime_result_domain_machine_overload_exit",
    "ownership/linear_state_call_handoff",
    "ownership/linear_transition_nested_call_handoff",
    "ownership/linear_repeated_transition_call_handoff",
    "ownership/linear_boundary_entry_handoff",
    "ownership/linear_live_across_call_continuation",
    "ownership/linear_fresh_state_call_result_handoff",
    "ownership/linear_transfer_and_consume",
    "ownership/linear_transparent_record_frontier",
    "ownership/linear_transparent_record_state_result",
    "ownership/linear_aggregate_state_result",
    "arithmetic/bare_name_scopes",
    "arithmetic/const_fold_overflow_compiles",
    "arithmetic/runtime_i64_min_literal_exit",
    "arithmetic/runtime_i64_to_u64_exact_guard_exit",
    "arithmetic/runtime_expression_range_bound_exit",
    "arithmetic/runtime_gcd_euclid_exit",
    "arithmetic/runtime_ranged_bitwise_and_mask_exit",
    "arithmetic/runtime_ranged_divide_modulo_chain_exit",
    "arithmetic/runtime_monte_carlo_pi_exit",
    "arithmetic/runtime_newton_sqrt_exit",
    "arithmetic/runtime_u64_max_literal_exit",
    "collections/runtime_activity_selection_greedy_exit",
    "collections/runtime_2d_transpose_exit",
    "collections/runtime_bfs_traversal_exit",
    "collections/runtime_binary_search_exit",
    "collections/runtime_bubble_sort_exit",
    "collections/runtime_coin_change_dp_exit",
    "collections/runtime_enum_grid_scan_exit",
    "collections/runtime_hash_table_exit",
    "collections/runtime_indexed_through_guard_chain_exit",
    "collections/runtime_matrix_multiply_exit",
    "collections/runtime_maze_pathfind_exit",
    "collections/runtime_nested_struct_array_field_exit",
    "collections/runtime_nqueens_backtracking_exit",
    "collections/runtime_ring_buffer_queue_exit",
    "collections/runtime_rpn_evaluator_exit",
    "collections/runtime_two_indexed_reads_binary_exit",
    "collections/runtime_two_pointer_palindrome_exit",
    "collections/runtime_indexed_read_then_guard_exit",
    "collections/runtime_indexed_struct_write_loop_exit",
    "collections/runtime_nested_array_const_index_exit",
    "collections/runtime_row_const_column_write_exit",
    "collections/runtime_rule90_automaton_exit",
    "collections/runtime_struct_field_temp_arith_exit",
    "collections/runtime_whole_array_value_copy_exit",
    "collections/runtime_whole_struct_value_copy_exit",
    "collections/std_option_runtime_match_exit",
    "collections/runtime_declared_range_index_read_exit",
    "collections/runtime_declared_range_index_write_exit",
    "collections/runtime_indexed_struct_field_operand_exit",
    "collections/runtime_indexed_struct_field_rmw_exit",
    "range/runtime_element_range_dataflow_exit",
    "range/runtime_funnel_guard_agreement_exit",
    "range/runtime_guarded_copy_narrowing_exit",
    "range/runtime_guarded_element_increment_exit",
    "range/runtime_guarded_runtime_index_increment_exit",
    "text/runtime_bounded_carrier_byte_write_exit",
    "text/runtime_carrier_byte_write_width_coercion",
    "text/runtime_utf16_literal_exit",
    "calls/runtime_slice_length_field_exit",
    "range/runtime_guarded_binary_operand_exit",
    "calls/runtime_machine_indexed_arg_exit",
    "calls/runtime_machine_indexed_struct_field_arg_exit",
    "collections/runtime_frame_indexed_param_read_exit",
    "collections/runtime_frame_indexed_param_operand_arg_exit",
    "collections/runtime_frame_indexed_param_field_exit",
    "collections/runtime_frame_indexed_local_read_exit",
    "collections/runtime_frame_indexed_byte_param_read_exit",
    "collections/runtime_machine_frame_index_read_exit",
    "collections/runtime_machine_frame_index_write_exit",
    "collections/runtime_machine_frame_index_dual_frame_write_exit",
    "collections/runtime_machine_frame_index_rmw_exit",
    "calls/runtime_machine_frame_index_arg_operand_exit",
    "collections/runtime_nested_const_row_indexed_read_exit",
    "collections/runtime_nested_const_row_struct_field_write_exit",
    "collections/runtime_nested_middle_index_3d_exit",
    "collections/runtime_let_bound_computed_index_exit",
    "collections/runtime_struct_field_operand_matrix_exit",
    "collections/runtime_struct_field_operand_param_exit",
    "collections/runtime_double_indexed_read_exit",
    "collections/runtime_nested_deep_const_prefix_exit",
    "collections/runtime_dual_frame_index_copy_exit",
    "collections/runtime_frame_mixed_index_pair_copy_exit",
    "collections/runtime_cross_region_indexed_pair_copy_exit",
    "collections/runtime_cross_region_double_indexed_pair_copy_exit",
    "collections/constant_nested_index_guard_exit",
    "collections/runtime_dual_mixed_index_copy_exit",
    "slices/runtime_slice_element_machine_roundtrip_exit",
    "slices/runtime_slice_element_runtime_index_read_exit",
    "slices/runtime_machine_bounded_subslice_local_exit",
    "slices/runtime_subslice_start_pointer_exit",
    "slices/runtime_end_fixed_array_subslice_local_exit",
    "slices/runtime_end_fixed_array_subslice_element_exit",
    "slices/guard_fixed_array_len_operand_exit",
    "slices/runtime_bounded_fixed_array_subslice_arg_exit",
    "text/runtime_bounded_carrier_concat_exit",
    "text/runtime_bounded_carrier_alias_concat_exit",
    "text/runtime_bounded_carrier_local_source_concat_exit",
    "text/runtime_value_call_slice_view_carrier_guard_exit",
    "calls/runtime_value_call_slice_view_element_arg_exit",
    "expressions/runtime_fixed_array_field_value_exit",
    "control_flow/fixed_array_element_guard",
    "control_flow/runtime_multi_field_payload_arith_exit",
    "control_flow/runtime_captured_local_remutated_field_exit",
    "control_flow/runtime_composite_initializer_local_arg_exit",
    "control_flow/runtime_linear_search_early_exit",
    "control_flow/runtime_loop_patterns_exit",
    "control_flow/runtime_nested_loop_grid_sum_exit",
    "expressions/arithmetic_domain_saturating_div_mod_exit",
    "expressions/runtime_float_local_arithmetic_exit",
    "expressions/runtime_guard_divide_modulo_exit",
    "expressions/runtime_guard_divide_modulo_signedness_exit",
    "expressions/runtime_guard_negative_arithmetic_exit",
    "arithmetic/runtime_and_of_or_guard_exit",
    "arithmetic/runtime_cast_in_guard_exit",
    "arithmetic/runtime_comparison_signedness_exit",
    "arithmetic/runtime_domain_boundaries_exit",
    "arithmetic/runtime_guard_feature_composition_exit",
    "arithmetic/runtime_negated_boolean_nesting_guard_exit",
    "arithmetic/runtime_parenthesized_guard_subjects_exit",
    "arithmetic/runtime_saturating_narrow_add_sub_exit",
    "arithmetic/runtime_shift_in_guard_exit",
    "arithmetic/runtime_shift_signedness_exit",
    "arithmetic/runtime_float_compare_cast_exit",
    "arithmetic/runtime_float_operations_exit",
    "arithmetic/runtime_i64_divide_modulo_exit",
    "arithmetic/runtime_integer_casts_exit",
    "arithmetic/runtime_mixed_width_sign_exit",
    "arithmetic/runtime_narrow_signed_divide_guard_exit",
    "arithmetic/runtime_narrow_signed_guard_ops_exit",
    "arithmetic/runtime_narrow_signed_wrap_boundaries_exit",
    "arithmetic/runtime_saturating_narrow_divide_exit",
    "arithmetic/runtime_unsigned_high_bit_u32_ops_exit",
    "arithmetic/runtime_unsigned_min_max_exit",
    "data/runtime_data_properties_exit",
    "domains/executable_domain_membership_intersection_value_exit",
    "domains/executable_imported_domain_membership_intersection_guard_exit",
    "domains/executable_imported_domain_membership_intersection_value_exit",
    "domains/executable_imported_domain_membership_union_guard_exit",
    "domains/executable_imported_domain_membership_union_value_exit",
    "expressions/runtime_flat_boolean_logic_exit",
    "expressions/runtime_literal_source_cast_exit",
    "traits/runtime_conformance_item_exit",
    "arithmetic/runtime_saturating_expression_domain_exit",
    "arithmetic/runtime_saturating_param_carry_exit",
    "arithmetic/runtime_saturating_wide_boundaries_exit",
    "arithmetic/runtime_wrapping_expression_guard_exit",
    "control_flow/runtime_boolean_transition_argument_after_string_guard_exit",
    "control_flow/runtime_direct_boolean_transition_argument_exit",
    "control_flow/runtime_local_boolean_conjunction_value_exit",
    "control_flow/runtime_local_boolean_transition_argument_exit",
    "control_flow/runtime_local_scalar_comparison_value_exit",
    "control_flow/runtime_local_string_comparison_value_exit",
    "arithmetic/float_to_int_saturating_exit",
    "arithmetic/float_to_int_unsigned_narrow_saturating_exit",
    "arithmetic/runtime_divide_min_edge_guard_exit",
    "arithmetic/runtime_nested_unsigned_witness_exit",
    "calls/runtime_assignment_call_post_mutation_value_exit",
    "calls/runtime_min_guard_true_false_pair_exit",
    "calls/runtime_min_max_guard_subject_hoist_exit",
    "calls/runtime_transition_subject_call_single_evaluation_exit",
    "calls/runtime_value_call_single_execution_exit",
    "control_flow/runtime_effectful_subject_single_evaluation_exit",
    "arithmetic/runtime_u64_literal_let_guard_exit",
    "calls/runtime_call_in_inlined_substate_exit",
    "calls/runtime_contained_machine_exit",
    "calls/runtime_deep_state_name_collision_exit",
    "calls/runtime_dispatch_binary_call_argument_exit",
    "calls/runtime_looping_cast_return_exit",
    "calls/runtime_looping_value_return_exit",
    "calls/runtime_multiarm_same_named_locals_exit",
    "calls/runtime_nested_value_call_in_substate_exit",
    "calls/runtime_value_call_self_field_enum_match_exit",
    "calls/runtime_dispatch_result_binary_terminal_exit",
    "calls/runtime_dispatch_result_multi_arm_exit",
    "calls/runtime_dispatch_result_guard_subject_exit",
    "calls/runtime_dispatch_result_transition_arg_exit",
    "calls/runtime_dispatched_effectful_reentrant_exit",
    "calls/runtime_dispatch_result_field_terminal_exit",
    "calls/runtime_nested_called_machine_loop_exit",
    "calls/runtime_value_callee_post_entry_lets_exit",
    "calls/runtime_post_entry_deep_chain_exit",
    "calls/runtime_post_entry_chained_let_exit",
    "constants/runtime_scoped_const_exit",
    "calls/runtime_free_machine_looping_value_call_exit",
    "calls/runtime_free_machine_value_call_mut_arg_exit",
    "calls/runtime_free_machine_value_call_exit",
    "domains/utf8_return_view_equals_exit",
    "expressions/runtime_16bit_cast_exit",
    "expressions/runtime_fixed_array_field_guard_exit",
    "expressions/runtime_widened_bitwise_exit",
    "expressions/runtime_widened_comparison_exit",
    "text/runtime_base64_encode_exit",
    "text/runtime_binary_format_exit",
    "text/runtime_bounded_carrier_pointee_guard_exit",
    "text/runtime_bounded_carrier_slice_field_write_exit",
    "text/runtime_bounded_carrier_write_line_exit",
    "text/runtime_carrier_cipher_exit",
    "text/runtime_carrier_fnv_loop_exit",
    "text/runtime_carrier_indexed_const_write_exit",
    "text/runtime_carrier_indexed_read_operand_exit",
    "text/runtime_carrier_indexed_write_exit",
    "text/runtime_carrier_itoa_exit",
    "text/runtime_carrier_len_guard_exit",
    "text/runtime_crc32_exit",
    "text/runtime_decimal_to_number_exit",
    "text/runtime_mandelbrot_render_exit",
    "text/runtime_number_to_decimal_exit",
    "text/runtime_run_length_encode_exit",
    "text/runtime_string_palindrome_exit",
    "text/runtime_substring_search_exit",
    "text/runtime_text_builder",
    "time/runtime_duration_core_exit",
    "time/runtime_duration_totals_exit",
    "time/runtime_fs_mtime_interop_windows_exit",
    "time/runtime_instant_elapsed_exit",
    "time/runtime_system_time_after_2026_exit",
    "time/runtime_time_elapsed_since_exit",
    "time/runtime_checked_time_arith_exit",
    "time/runtime_sleep_for_exit",
    "traits/boundary_trait_effects_host_call",
    "traits/equatable_sum_stale_payload_exit",
    "capabilities/acquires_filesystem_authority",
    "capabilities/stores_capability",
    "capabilities/external_leaf_binding_forms",
    "capabilities/native_fixed_array_import_compile",
    "providers/checked_boundary_operator_dispatch_exit",
    "capabilities/win64_pointer_length_vs_descriptor_compile",
    "targets/target_machine_gating_exit",
    "targets/single_target_internal_machine_skipped",
    "traits/ring_requirement_satisfies_exit",
    "traits/runtime_trait_default_dispatch_exit",
    "traits/runtime_inherited_trait_default_exit",
    "traits/runtime_generic_trait_default_exit",
    "float/runtime_total_order_satisfiers_exit",
    "expressions/runtime_qualified_case_value_exit",
    "calls/recursive_result_bind_first_arg",
    "calls/runtime_branching_callee_chain_exit",
    "calls/runtime_inline_recursive_walk_exit",
    "calls/runtime_value_call_direct_recursive_walk_exit",
    "calls/runtime_value_call_statement_recursive_walk_exit",
    "filesystem/windows_wrapper_create_new_exit",
    "filesystem/windows_wrapper_metadata_exit",
    "filesystem/windows_read_dir_nth_exit",
    "filesystem/windows_hard_link_exit",
    "filesystem/windows_wrapper_exists_exit",
    "filesystem/windows_wrapper_set_len_exit",
    "filesystem/windows_wrapper_copy_exit",
    "targets/efi_freestanding_skeleton",
    "targets/efi_entry_arguments",
    "targets/efi_float_entry_argument",
    "targets/efi_stack_entry_argument",
    "targets/entry_run_args_bytes",
    "targets/efi_struct_handoff",
    "targets/efi_conout_projection",
    "targets/efi_ref_param_direct_faces",
    "comptime/runtime_const_array_length_exit",
    "layouts/runtime_plan_laid_value_field_exit",
    "layouts/runtime_plan_laid_compact_bits_exit",
    "layouts/runtime_plan_laid_integer_at_projection_exit",
    "layouts/runtime_plan_laid_integer_at_proved_write_exit",
    "layouts/runtime_plan_laid_integer_at_total_write_exit",
    "layouts/runtime_plan_laid_value_by_value_param_exit",
    "layouts/runtime_plan_laid_record_view_exit",
    "layouts/runtime_plan_laid_fixed_array_view_exit",
    "layouts/runtime_plan_laid_fixed_array_mutable_write_exit",
    "layouts/runtime_plan_laid_nested_fixed_array_mutable_write_exit",
    "layouts/runtime_plan_laid_nested_record_mutable_write_exit",
    "layouts/runtime_plan_laid_record_array_mutable_write_exit",
    "layouts/runtime_plan_laid_record_mutable_write_exit",
    "control_flow/runtime_compare_pair_dispatch_exit",
    "arithmetic/runtime_float_self_compare_nan_exit",
    "arithmetic/runtime_abs_desugar_exit",
    "arithmetic/runtime_sqrt_builtin_exit",
    "arithmetic/runtime_clamp_desugar_exit",
    "arithmetic/runtime_clamp_narrowing_exit",
    "arithmetic/runtime_negative_float_to_int_exit",
    "arithmetic/runtime_float_min_max_abs_clamp_exit",
    "arithmetic/runtime_float_running_min_max_fold_exit",
    "collections/runtime_dual_indexed_guard_compare_exit",
    "collections/runtime_cross_array_indexed_guard_compare_exit",
    "collections/runtime_dual_indexed_guard_equality_exit",
    "collections/runtime_dual_indexed_copy_exit",
    "collections/runtime_dual_indexed_copy_in_loop_exit",
    "collections/runtime_indexed_write_frame_local_source_exit",
    "collections/runtime_indexed_local_copy_chain_exit",
    "collections/runtime_inplace_reverse_local_temp_exit",
    "control_flow/runtime_captured_local_swap_exit",
    "calls/runtime_arm_target_host_result_exit",
    "calls/runtime_enum_self_method_exit",
    "calls/runtime_same_type_contained_direct_fields_exit",
    "calls/runtime_shared_ref_param_member_exit",
    "calls/runtime_shared_ref_param_large_deref_exit",
    "calls/runtime_large_shared_ref_direct_assignment_exit",
    "calls/runtime_value_call_dispatch_results_exit",
    "calls/runtime_value_call_entry_field_write_exit",
    "calls/runtime_value_call_guard_subject_exit",
    "calls/runtime_effectful_guard_local_and_self_terminal_exit",
    "calls/runtime_guarded_effectful_transition_argument_exit",
    "calls/runtime_value_call_literal_len_arm_guard_exit",
    "calls/runtime_value_call_nested_entry_call_exit",
    "calls/runtime_value_call_same_callee_sites_exit",
    "calls/runtime_two_site_struct_result_exit",
    "calls/runtime_nested_value_call_guard_exit",
    "calls/runtime_cross_callee_let_names_exit",
    "calls/runtime_cross_callee_division_exit",
    "calls/runtime_value_call_shared_payload_name_exit",
    "calls/runtime_value_call_shared_slot_straight_line_exit",
    "calls/runtime_value_call_struct_payload_cast_field_exit",
    "calls/runtime_branch_leaf_multiple_named_conversion_exit",
    "calls/runtime_value_call_transition_args_exit",
    "calls/runtime_value_call_transition_args_straight_line_exit",
    "filesystem/windows_wrapper_breadth_exit",
    "filesystem/runtime_local_host_result_dispatch_exit",
    "filesystem/windows_wrapper_results_exit",
    "filesystem/windows_wrapper_dark_methods_exit",
    "filesystem/repeated_dir_walk_scan_exit",
    "slices/runtime_indexed_element_copy_write_exit",
    "calls/struct_literal_transition_arg_exit",
    "calls/bool_value_call_return_exit",
    "float/expansion_float_local_guard_exit",
    "calls/float_value_call_return_exit",
    "calls/float_value_call_runtime_arg_exit",
    "float/runtime_std_is_finite_exit",
    "float/f32_chain_per_op_rounding_exit",
    "float/named_provider_min_max_sqrt_exit",
    "float/named_provider_negate_is_nan_exit",
    "float/named_provider_classification_predicates_exit",
    "float/named_provider_classify_exit",
    "float/named_provider_multiply_then_add_exit",
    "float/named_provider_fused_multiply_add_exit",
    "float/named_provider_directed_fused_multiply_add_exit",
    "float/runtime_named_format_conversion_exit",
    "float/runtime_named_integer_to_float_conversion_exit",
    "float/runtime_named_float_to_integer_conversion_exit",
    "float/runtime_named_float_to_integer_trapping_nan_traps",
    "float/runtime_named_float_to_integer_trapping_overflow_traps",
    "collections/runtime_palindrome_two_pointer_exit",
    "collections/runtime_bracket_matcher_stack_exit",
    "collections/runtime_argmax_index_exit",
    "control_flow/runtime_sum_field_store_payload_exit",
    "data/case_payload_native_construction",
    "data/match_exhaustive_by_case_union_domain",
    "data/match_exhaustive_by_cases",
    "data/runtime_array_literal_string_field_exit",
    "data/runtime_case_payload_guard_read_exit",
    "data/runtime_case_reassignment_exit",
    "data/runtime_mixed_shape_exit",
    "data/runtime_struct_literal_string_field_exit",
    "data/runtime_whole_struct_mutation_copy_exit",
    "domains/utf8_value_call_field_write",
    "domains/utf8_field_write_from_param",
    "domains/utf8_field_read_carries_domain_exit",
    "domains/domain_field_write_then_read_exit",
    "control_flow/composite_field_guard_dispatch",
    "control_flow/composite_range_guard_dispatch",
    "control_flow/guarded_leaf_branch_expansion",
    "control_flow/guarded_transition_dispatch",
    "control_flow/state_transition_chain",
    "control_flow/no_payload_case_variant_after_payload_dispatch_exit",
    "control_flow/case_payload_shared_field_name_exit",
    "control_flow/sum_mixed_width_payload_layout",
    "control_flow/sum_field_storage_roundtrip",
    "control_flow/sum_payload_cast_operand_field_exit",
    "arithmetic/runtime_float_compare_bool_exit",
    "arithmetic/runtime_float_nested_operand_exit",
    "arithmetic/runtime_shift_count_domain_exit",
    "arithmetic/runtime_exact_guarded_shift_count_exit",
    "arithmetic/runtime_shift_atwidth_signed_modular_exit",
    "arithmetic/runtime_shift_right_atwidth_exit",
    "arithmetic/runtime_shift_atwidth_indexed_targets_exit",
    "arithmetic/constant_trapping_shift_value_overflow_traps",
    "arithmetic/runtime_sat_nested_operand_domain_exit",
    "arithmetic/runtime_sat_unsigned_onedirection_exit",
    "arithmetic/runtime_sat_min_idiom_exit",
    "types/runtime_addr_value_flow_exit",
    "types/runtime_addr_algebra_exit",
    "arithmetic/runtime_shl_saturating_exit",
    "arithmetic/runtime_shl_saturating_value_overflow_exit",
    "arithmetic/runtime_shift_count_proven_range_exit",
    "arithmetic/runtime_shift_subword_masked_count_exit",
    "arithmetic/u64_magnitude_transition_arg_exit",
    "arithmetic/float_literal_cast_proves_exit",
    "proofs/runtime_decreases_u64_measure_exit",
    "arithmetic/runtime_wrapping_operand_truncation_exit",
    "text/case_literal_texteq_field_store_exit",
    "text/case_literal_texteq_terminal_exit",
    "text/runtime_text_equals_boolean_operand_exit",
    "text/runtime_text_not_equals_exit",
    "text/runtime_owned_string_byte_view_exit",
    "text/zii_default_string_equality_exit",
    "text/zii_string_host_write_exit",
    "text/runtime_text_equals_value_positions_exit",
    "wire/runtime_wire_roundtrip_utf8_exit",
    "wire/runtime_wire_utf8_edge_verdicts_exit",
    "wire/runtime_wire_utf8_invalid_refused_exit",
    "wire/runtime_wire_schema_as_value_type_exit",
    "wire/runtime_wire_decode_let_compare_exit",
    "control_flow/runtime_case_member_dispatch_exit",
    "control_flow/runtime_local_boolean_or_value_exit",
    "control_flow/runtime_straight_line_terminal_local_exit",
    "control_flow/runtime_straight_line_terminal_field_readback_exit",
    "control_flow/termination_index_distance_compile",
    "termination/custom_ranking_field_countdown_compile",
    "termination/custom_ranking_struct_view",
    "termination/runtime_recursive_result_roles_exit",
    "domains/bodyless_domain_declarations_exit",
    "domains/executable_domain_membership_expression_exit",
    "domains/executable_domain_membership_intersection_guard_exit",
    "domains/executable_imported_domain_membership_exit",
    "domains/executable_imported_domain_membership_guard_exit",
    "domains/executable_domain_membership_union_guard_exit",
    "domains/executable_domain_membership_union_value_exit",
    "control_flow/entry_surface_receiver_paths",
    "borrow/runtime_view_linked_input_unrelated_ref_write_exit",
    "calls/mutable_output_host_call",
    "calls/nested_machine_continuation",
    "collections/record_array_field_access",
    "calls/runtime_attached_machine_struct_arg_exit",
    "calls/runtime_record_forwarding_statement_call_exit",
    "calls/by_value_case_param_self_write_exit",
    "calls/runtime_explicit_discard_executes_exit",
    "calls/runtime_free_machine_struct_arg_exit",
    "calls/runtime_free_machine_struct_return_exit",
    "calls/sequential_self_field_rmw_exit",
    "calls/transition_arg_local_from_embedded_call_exit",
    "calls/value_call_embedded_in_binary_exit",
    "storage/runtime_alias_integer_write",
    "storage/runtime_alias_field_integer",
    "storage/runtime_alias_field_binary",
    "storage/runtime_machine_owned_fixed_indexed_struct_copy_exit",
    "storage/runtime_machine_owned_indexed_integer_write_exit",
    "storage/runtime_machine_owned_indexed_nested_exit_write_exit",
    "storage/runtime_machine_owned_indexed_struct_copy_exit",
    "storage/runtime_dispatch_helper_local_alias_add_exit",
    "storage/runtime_dispatch_helper_local_alias_add_compile",
    "storage/requires_slice_indexed_alias_field_binary_compile",
    "storage/runtime_slice_indexed_binary_rmw_exit",
    "calls/runtime_mut_ref_forward_exit",
    "storage/runtime_local_slice_forward_exit",
    "float/f32_guard_const_arith_landed_exit",
    "float/f32_arg_const_arith_landed_exit",
    "text/runtime_alias_string_write",
    "text/runtime_alias_text_builder_write",
    "text/runtime_string_concat_membership_exit",
    "text/runtime_string_append_in_place_exit",
    "text/runtime_string_field_concat_exit",
    "text/runtime_machine_owned_indexed_string_field_concat_exit",
    "text/runtime_machine_owned_indexed_bounded_carrier_literal_exit",
    "text/runtime_machine_owned_double_indexed_bounded_carrier_literal_exit",
    "text/runtime_machine_owned_double_indexed_string_field_concat_exit",
    "text/runtime_slice_alias_indexed_string_field_concat_exit",
    "text/runtime_slice_indexed_string_guard_exit",
    "text/runtime_slice_machine_indexed_string_guard_exit",
    "text/runtime_local_array_indexed_string_guard_exit",
    "text/runtime_local_array_indexed_string_field_concat_exit",
    "text/runtime_slice_fixed_indexed_string_guard_exit",
    "text/runtime_pointee_string_guard_exit",
    "text/runtime_string_field_literal_guard_exit",
    "text/runtime_mutable_string_parameter_concat_exit",
    "text/runtime_mutable_string_parameter_concat_write_line",
    "text/runtime_mutable_string_parameter_wrapped_concat_write_line",
    "text/runtime_mutable_struct_string_field_copy_concat_exit",
    "text/runtime_local_struct_string_field_concat_exit",
    "text/runtime_string_stored_suffix_exit",
    "text/runtime_lookup_struct_field_concat_exit",
    "text/runtime_large_lookup_struct_field_concat_exit",
    "text/runtime_large_room_lookup_struct_field_concat_exit",
    "text/runtime_call_argument_struct_string_field_slice_alias_exit",
    "text/runtime_mutable_struct_string_field_copy_concat_write_line",
    "arithmetic/runtime_arithmetic_guard",
    "arithmetic/runtime_arithmetic_value",
    "calls/runtime_call_guard",
    "control_flow/runtime_branching_helper_guard",
    "control_flow/runtime_branching_helper_local_guard_value",
    "control_flow/runtime_branching_helper_string",
    "control_flow/runtime_branching_helper_struct",
    "control_flow/runtime_branching_helper_value",
    "rewards/runtime_branch_enemy_reward_shape",
    "calls/runtime_call_value",
    "calls/runtime_call_enum_field_value",
    "calls/runtime_call_enum_field_with_args",
    "calls/runtime_call_enum_field_with_mut_arg",
    "calls/runtime_call_enum_sequence",
    "calls/runtime_call_enum_value",
    "calls/runtime_nested_named_conversion_alias_exit",
    "calls/runtime_call_result_after_splice_mutation_exit",
    "calls/runtime_string_call_result_through_reference_field_exit",
    "calls/runtime_two_string_call_results_through_reference_fields_exit",
    "calls/runtime_offset_string_call_results_through_reference_fields_exit",
    "calls/runtime_reference_returned_slice_element_through_param_exit",
    "calls/runtime_nested_guarded_reference_returned_slice_element_exit",
    "calls/runtime_mutable_machine_owned_parameter_write_exit",
    "calls/runtime_mutable_local_indexed_parameter_write_exit",
    "calls/runtime_mutable_machine_owned_local_indexed_parameter_write_exit",
    "calls/runtime_mutable_dynamic_indexed_machine_owned_parameter_write_exit",
    "dungeon/runtime_boolean_helper_guard_dispatch",
    "dungeon/runtime_direct_boolean_conjunction_dispatch",
    "dungeon/runtime_direct_boolean_conjunction_exit",
    "dungeon/runtime_enemy_clear_reentry_exit",
    "dungeon/runtime_clear_carve_render_string_fields_exit",
    "dungeon/runtime_full_level_wrapper_lookup_string_field_exit",
    "dungeon/runtime_enemy_clear_reentry_guard",
    "control_flow/runtime_guarded_leaf_ordering_call",
    "dungeon/runtime_ordered_room_dispatch_after_call_exit",
    "dungeon/runtime_ordered_room_dispatch_exit",
    "dungeon/runtime_ordered_room_dispatch_game_shape_exit",
    "dungeon/runtime_ordered_room_dispatch_large_machine_exit",
    "dungeon/runtime_ordered_room_dispatch_loop_exit",
    "dungeon/runtime_ordered_room_dispatch_real_show_states_exit",
    "dungeon/runtime_guarded_inline_leaf_arm_skip_exit",
    "dungeon/runtime_nested_value_call_caller_local_guard_exit",
    "dungeon/runtime_threaded_mut_arg_interrupt_soak_exit",
    "calls/runtime_contained_call_value",
    "rewards/runtime_contained_reward_table_roll_item",
    "control_flow/runtime_nested_branch_assignment_prelude_value",
    "control_flow/runtime_nested_branch_prelude_value",
    "control_flow/runtime_nested_branch_value",
    "slices/runtime_dispatch_mutable_slice_element_write_compile",
    "slices/runtime_dispatch_mutable_slice_element_write_exit",
    "slices/runtime_array_indexed_read_exit",
    "slices/runtime_array_indexed_loop_exit",
    "slices/runtime_decreasing_index_exit",
    "slices/runtime_slice_indexed_read_exit",
    "slices/runtime_array_adjacent_index_exit",
    "slices/runtime_nested_decreasing_index_exit",
    "slices/runtime_narrow_widen_cast_exit",
    "slices/runtime_signed_index_guarded_exit",
    "slices/runtime_two_pointer_sum_exit",
    "slices/runtime_two_pointer_reverse_exit",
    "slices/runtime_branched_index_bound_exit",
    "slices/runtime_indexed_array_write_exit",
    "arithmetic/runtime_modulo_value",
    "arithmetic/runtime_modulo_div_narrowing_exit",
    "arithmetic/runtime_nested_payload_range_narrowing_exit",
    "arithmetic/runtime_guard_proven_counter_exit",
    "arithmetic/runtime_guard_narrowed_transition_arg_exit",
    "arithmetic/runtime_trapping_overflow_traps",
    "arithmetic/runtime_saturating_domain_exit",
    "arithmetic/runtime_fnv1a_hash_exit",
    "arithmetic/runtime_min_max_clamp_narrowing_exit",
    "arithmetic/runtime_transition_arg_guard_narrowing_exit",
    "arithmetic/runtime_transition_value_guard_narrowing_exit",
    "arithmetic/runtime_requires_one_sided_bound_exit",
    "arithmetic/runtime_transition_arg_false_arm_narrowing_exit",
    "arithmetic/runtime_transition_arg_saturating_exit",
    "arithmetic/runtime_cast_element_accumulator_exit",
    "arithmetic/runtime_exclusive_range_constraint_exit",
    "arithmetic/runtime_payload_range_narrowing_exit",
    "ranges/sum_payload_range_narrowed_exit",
    "ranges/sum_payload_range_arith_narrowed_exit",
    "arithmetic/runtime_struct_field_range_narrowing_exit",
    "arithmetic/runtime_provable_field_construction_exit",
    "arithmetic/runtime_inferred_return_range_exit",
    "arithmetic/runtime_inferred_multipath_return_exit",
    "control_flow/runtime_multi_assignment_value_calls",
    "control_flow/runtime_boolean_or_guard_exit",
    "control_flow/runtime_negated_boolean_place_guard_exit",
    "control_flow/runtime_negated_comparison_guard_exit",
    "dungeon/runtime_multi_room_reentry_exit",
    "slices/runtime_mutable_slice_element_write_straight_line_exit",
    "slices/runtime_mutable_slice_element_write_exit",
    "slices/runtime_subslice_range_len_exit",
    "slices/runtime_subslice_bounded_range_len_exit",
    "slices/runtime_subslice_bounded_dynamic_index_exit",
    "slices/runtime_subslice_dynamic_index_exit",
    "slices/runtime_subslice_end_dynamic_index_exit",
    "slices/runtime_nested_subslice_dynamic_index_exit",
    "slices/runtime_nested_subslice_fixed_index_exit",
    "slices/runtime_subslice_range_pointer_exit",
    "slices/runtime_frame_array_slice_parameter_alias_exit",
    "slices/runtime_slice_index_copy_dispatch_exit",
    "slices/runtime_slice_index_copy_exit",
    "slices/runtime_slice_index_read_dispatch_exit",
    "slices/runtime_slice_index_read_exit",
    "slices/runtime_indexed_read_operand_exit",
    "slices/runtime_subslice_len_exit",
    "slices/runtime_machine_field_subslice_arg_index_exit",
    "slices/recursive_subslice_element_accumulator_exit",
    "slices/runtime_subslice_of_slice_param_exit",
    "slices/runtime_subslice_param_bounded_range_exit",
    "slices/runtime_subslice_param_end_only_exit",
    "slices/runtime_subslice_param_local_exit",
    "slices/runtime_subslice_runtime_start_exit",
    "slices/runtime_subslice_runtime_end_exit",
    "slices/runtime_subslice_nested_of_param_exit",
    "slices/runtime_subslice_runtime_start_over_local_exit",
    "slices/runtime_subslice_param_inclusive_end_exit",
    "rewards/runtime_reward_table_roll_item_shape",
    "dungeon/runtime_room_use_reentry_guard",
    "dungeon/runtime_room_use_reentry_exit",
    "text/runtime_text_storage",
    "text/runtime_stderr_write_exit",
    "text/runtime_stdin_command_branch_exit",
    "text/runtime_stdin_line_buffering_exit",
    "calls/runtime_trailing_local_return_exit",
    "calls/runtime_transition_subject_call_guard",
    "calls/runtime_transition_argument_call_value",
    "collections/runtime_fixed_vec_round_trip_exit",
    "collections/runtime_write_first_loop_index_exit",
    "collections/runtime_loop_counter_init_hoisted_exit",
    "collections/runtime_nested_loop_fill_exit",
    "collections/runtime_computed_array_fill_via_temp_exit",
    "collections/runtime_computed_indexed_write_exit",
    "collections/runtime_nested_const_product_index_exit",
    "collections/runtime_hoisted_index_write_exit",
    "calls/runtime_let_mut_reassign_exit",
    "control_flow/runtime_tuple_matrix_exhaustive_exit",
    "control_flow/runtime_sum_tuple_matrix_exhaustive_exit",
    "control_flow/runtime_tuple_case_destructure_exit",
    "dependent/runtime_dependent_param_range_exit",
    "dependent/runtime_dependent_product_index_exit",
    "dependent/runtime_dependent_subtract_exit",
    "dependent/runtime_dependent_ordering_chain_exit",
    "dependent/runtime_requires_subtract_exit",
    "dependent/runtime_requires_guarded_call_exit",
    "dependent/runtime_sibling_len_index_exit",
    "dependent/runtime_bounded_product_index_exit",
    "dependent/data_where_standing_bound_exit",
    "dependent/data_where_gated_machine_established_exit",
    "dependent/nested_data_where_standing_bound_exit",
    "dependent/indexed_data_where_standing_bound_exit",
    "recast/runtime_scalar_pun_shared_let_exit",
    "recast/runtime_scalar_pun_mutable_write_exit",
    "recast/runtime_mutable_equivalent_domain_recast_exit",
    "recast/runtime_mutable_equivalent_range_recast_exit",
    "recast/runtime_bool_representation_recast_exit",
    "recast/runtime_shared_domain_weakening_recast_exit",
    "recast/runtime_float_range_representation_recast_exit",
    "recast/runtime_shared_record_float_range_weakening_exit",
    "recast/runtime_mutable_equivalent_record_recast_exit",
    "recast/runtime_aggregate_slice_representation_recast_exit",
    "recast/runtime_fixed_array_view_mutable_write_exit",
    "recast/runtime_interior_slice_view_mutable_write_exit",
    "recast/runtime_record_view_assignment_binary_exit",
    "recast/runtime_slice_view_mutable_write_exit",
    "recast/runtime_offset_byte_recast_mutable_write_exit",
    "recast/runtime_interior_byte_recast_exit",
    "recast/runtime_offset_byte_recast_exit",
    "recast/runtime_guarded_offset_recast_exit",
    "recast/runtime_symbolic_stride_footprint_exit",
    "data/case_membership_union_guard_exit",
    "data/runtime_proof_only_data_declared_exit",
    "arithmetic/runtime_u64_guarded_cap_store_exit",
    "calls/runtime_measured_tail_recursion_exit",
    "calls/runtime_terminal_tail_recursion_exit",
    "comptime/runtime_const_measured_recursion_exit",
    "collections/runtime_computed_index_match_subject_exit",
    "recast/runtime_multi_edge_offset_meet_exit",
    "calls/runtime_std_math_sin_cos_exit",
    "calls/runtime_value_call_terminal_exit",
    "calls/guarded_value_call_arm_exit",
    "constants/runtime_free_const_exit",
    "proofs/runtime_core_nat_declared_exit",
    "proofs/runtime_core_rat_declared_exit",
    "proofs/accepted_axiom_cited_exit",
    "proofs/runtime_nat_structural_recursion_exit",
    "proofs/runtime_core_roster_ops_exit",
    "build/runtime_depend_mapping_exit",
    "recast/runtime_record_view_exit",
    "recast/runtime_record_array_view_mutable_write_exit",
    "recast/constant_offset_record_view_after_write_exit",
    "arithmetic/runtime_f32_field_guard_exit",
    "collections/runtime_indexed_rmw_loop_exit",
    "collections/runtime_indexed_reduction_loop_exit",
    "collections/runtime_array_max_and_sum_exit",
    "collections/runtime_indexed_guard_subject_exit",
    "collections/runtime_array_min_max_builtin_exit",
    "collections/runtime_dual_indexed_comparison_guard_exit",
    "core/zii_default_composite_exit",
    "core/content_projection_owner",
    "core/content_conservation_contract",
    "core/content_retained_custody_round_trip",
    "core/extent_root_provider_adapter",
    "core/carry_permission_provider_adapter",
    "core/task_lifecycle_operations",
    "data/record_pattern_let_exit",
    "data/record_pattern_double_underscore_field",
    "data/record_pattern_bind_all_exit",
    "data/runtime_record_field_value_pattern_exit",
    "control_flow/case_pattern_rename_waive_exit",
    "control_flow/record_pattern_arm_rename_guard_exit",
    "control_flow/runtime_nonplace_record_pattern_single_evaluation_exit",
    "control_flow/arm_pattern_rest_optout_exit",
    "operators/core_operator_spelling_surface",
    "operators/float_operator_identities",
    "traits/equatable_record_equality_exit",
    "traits/equatable_mixed_shape_equality_exit",
    "traits/equatable_string_field_equality_exit",
    "traits/equatable_string_not_equals_exit",
    "traits/equatable_string_equality_guard_exit",
    "traits/equatable_sum_payload_equality_exit",
    "termination/runtime_shrinking_slice_recursion_exit",
    // --- Language-guide chapter coverage (Ch1-22) ---
    "calls/runtime_local_string_field_copy_through_mut_exit",
    "calls/runtime_min_call_result_arithmetic_exit",
    "text/runtime_machine_string_append_in_place_exit",
    "text/runtime_string_concat_two_fields_exit",
    "text/runtime_chained_string_append_exit",
    "arithmetic/runtime_i64_full_width_exit",
    "arithmetic/runtime_chained_field_mutation_exit",
    "arithmetic/runtime_copy_then_read_exit",
    "arithmetic/const_fold_cast_signedness",
    "arithmetic/wrapping_signed_divide_min_by_neg_one",
    "arithmetic/saturating_signed_divide_min_by_neg_one",
    "arithmetic/saturating_multiply_overflow_both_signs",
    "arithmetic/f32_field_store_rounding",
    "arithmetic/f32_transition_arg_rounding",
    "arithmetic/int_transition_arg_width_wrap",
    "arithmetic/array_element_write_width_domain",
    "arithmetic/struct_literal_field_coercion",
    "arithmetic/runtime_signed_division_exit",
    "arithmetic/runtime_shift_right_signedness",
    "arithmetic/const_fold_unsigned_landed_ops_exit",
    "arithmetic/const_fold_saturating_narrow_exit",
    "arithmetic/const_fold_wrapping_narrow_exit",
    "calls/mutual_cycle_tail_admitted_exit",
    "providers/external_leaf_via_compile",
    "providers/adapter_satisfies_compile",
    "providers/provider_type_slot_selected",
    "providers/component_owner_provider_override_compile",
    "providers/test_owner_provider_override_compile",
    "providers/provider_type_target_default",
    "providers/provider_type_target_default_override",
    "providers/runtime_adapter_dispatch_exit",
    "providers/runtime_result_domain_requirement_overload_exit",
    "providers/runtime_adapter_forwarding_exit",
    "providers/runtime_boundary_capability_state_forwarding_exit",
    "host/runtime_console_byte_literal_exit",
    "arithmetic/const_fold_unsigned_shift_right_arg_exit",
    "arithmetic/const_fold_unsigned_divide_arg_exit",
    "arithmetic/unsigned_min_max_wrapping_local_exit",
    "arithmetic/unsigned_min_max_operand_position_exit",
    "arithmetic/suffix_landed_operand_position_exit",
    "arithmetic/suffix_boundary_magnitudes_exit",
    "float/anonymous_exact_rat_const_exit",
    "float/f32_per_operation_rounding_exit",
    "float/finite_core_domain_range_discharge",
    "float/float_saturating_arithmetic_exit",
    "float/float_to_int_exact_proofs_exit",
    "float/float_to_int_policy_exit",
    "float/float_to_int_trapping_nan_traps",
    "float/float_to_int_trapping_overflow_traps",
    "float/float_trapping_divide_zero_traps",
    "float/float_trapping_invalid_traps",
    "float/float_trapping_overflow_traps",
    "float/float_trapping_propagated_nan_traps",
    "float/float_trapping_propagated_infinity_traps",
    "float/suffix_f32_single_rounding_exit",
    "float/unsuffixed_f32_destination_single_rounding_exit",
    "float/unsuffixed_f32_argument_single_rounding_exit",
    "arithmetic/runtime_unsigned_division_exit",
    "arithmetic/runtime_min_max_signedness_exit",
    "arithmetic/runtime_comparison_value_signedness_exit",
    "arithmetic/runtime_comparison_guard_signedness_exit",
    "operators/unary_negation_exit",
    "operators/compound_assignment_exit",
    "expressions/runtime_match_value_exit",
    "expressions/runtime_float_constant_store_exit",
    "expressions/runtime_float_arithmetic_exit",
    "expressions/runtime_float_comparison_exit",
    "expressions/runtime_float_place_comparison_exit",
    "expressions/runtime_numeric_cast_exit",
    "calls/runtime_value_position_branching_call_exit",
    "calls/runtime_value_call_let_combine_exit",
    "data/runtime_deep_nested_field_exit",
    "data/runtime_struct_value_copy_exit",
    "structs/runtime_particle_system_exit",
    "structs/runtime_nested_struct_construction_exit",
    "structs/runtime_entity_component_exit",
    "structs/runtime_nested_struct_state_machine_exit",
    "structs/runtime_array_element_struct_copy_exit",
    "structs/runtime_nested_struct_value_semantics_exit",
    "structs/runtime_struct_array_literal_exit",
    "structs/runtime_enum_struct_payload_exit",
    "structs/runtime_nested_field_accumulate_loop_exit",
    "structs/aggregate_transition_args_exit",
    "structs/deep_nested_write_paths_exit",
    "structs/runtime_enum_classify_dispatch_exit",
    "calls/runtime_value_transition_unsigned_guard_exit",
    "calls/runtime_exit_code_exit",
    "calls/value_call_sequential_result_slots_exit",
    "calls/value_call_sequential_self_capture_exit",
    "operators/integer_literal_suffix_exit",
    "operators/runtime_shift_operators_exit",
    "operators/runtime_bitwise_operators_exit",
    "operators/runtime_bitwise_guard_exit",
    "operators/runtime_xorshift_prng_exit",
    "operators/runtime_popcount_loop_exit",
    "calls/free_standing_machine_helper_compile",
    "calls/statement_call_recursive_argument_compile",
    "capabilities/provider_within_ceiling",
    "capabilities/derives_authority_via_boundary",
    "capabilities/acquires_through_helper_return",
    "capabilities/derives_through_helper",
    "control_flow/runtime_integer_literal_dispatch_exit",
    "control_flow/runtime_string_literal_dispatch_exit",
    "core/numeric_conversion_surface",
    "core/numeric_cross_signed_conversion_surface",
    "core/numeric_cross_signed_negative_traps",
    "core/numeric_cross_signed_unsigned_overflow_traps",
    "core/numeric_signed_conversion_surface",
    "core/numeric_trapping_conversion_overflow",
    "expressions/float_array_binary_op_zero",
    "expressions/f32_array_binary_op_zero",
    "expressions/arithmetic_domain_wrapping_exit",
    "expressions/arithmetic_domain_saturating_exit",
    "expressions/arithmetic_domain_saturating_mul_exit",
    "expressions/arithmetic_domain_saturating_const_fold_exit",
    "expressions/arithmetic_domain_return_range_proven_exact_exit",
    "expressions/arithmetic_domain_saturating_mul_signed_exit",
    "expressions/arithmetic_domain_trapping_mul_exit",
    "expressions/arithmetic_domain_trapping_div_exit",
    "expressions/arithmetic_domain_trapping_mul_overflow",
    "expressions/arithmetic_domain_saturating_signed_exit",
    "expressions/arithmetic_domain_trapping_exit",
    "expressions/arithmetic_domain_trapping_overflow",
    "expressions/arithmetic_domain_trapping_let_overflow",
    "expressions/arithmetic_domain_cast_exit",
    "expressions/arithmetic_domain_range_proven_exact_exit",
    "expressions/arithmetic_domain_requires_proven_exact_exit",
    "expressions/f32_field_binary_to_local_cast",
    "expressions/f32_deep_chain_binary",
    "expressions/f32_to_f64_local_cast",
    "generics/runtime_const_data_array_length_exit",
    "generics/runtime_const_data_expression_exit",
    "generics/runtime_const_data_machine_call_exit",
    "generics/runtime_const_data_machine_fact_exit",
    "generics/runtime_const_data_where_fact_exit",
    "generics/runtime_const_data_symbolic_expression_exit",
    "generics/runtime_const_data_forwarded_length_exit",
    "generics/runtime_const_data_multiple_instances_exit",
    "generics/runtime_const_data_named_value_exit",
    "generics/runtime_signed_const_data_exit",
    "generics/runtime_const_container_methods_exit",
    "generics/runtime_generic_record_instance_exit",
    "generics/runtime_generic_two_instantiations_exit",
    "generics/runtime_generic_enum_payload_exit",
    "generics/runtime_generic_value_call_exit",
    "generics/runtime_generic_value_call_agreeing_exit",
    "generics/runtime_generic_param_position_inference_exit",
    "generics/runtime_generic_multiple_specializations_exit",
    "host/runtime_tick_count_monotonic_exit",
    "host/runtime_tick_paced_marquee_exit",
    "host/runtime_gui_window_blit_exit",
    "host/runtime_gui_window_lifecycle_exit",
    "host/runtime_gui_foreground_window_exit",
    "inline_asm/asm_block_jmp_state",
    "memory/repr_native_stable_layout",
    "operators/runtime_integer_division_value",
    "traits/trait_generic_bound_static_dispatch",
    "versioning/runtime_version_migration_exit",
    "versioning/runtime_versioned_match_zii_exit",
    "versioning/runtime_versioned_three_era_match_zii_exit",
    "wire/wire_generic_trait",
    "wire/wire_compatibility_demand_report",
    "wire/runtime_transform_machine_from_wire",
    "wire/runtime_transform_machine_to_wire",
    "wire/wire_data_field_numbers",
    "wire/wire_data_reserved_field",
    "wire/wire_data_version_block",
    "wire/wire_data_encoding_family",
    "wire/wire_multi_version_evolution",
    "wire/wire_field_references_program_types",
    "wire/wire_cross_era_type_change_migration",
    "wire/wire_cross_era_number_recycling",
    "wire/runtime_wire_encode_primitive_exit",
    "wire/runtime_wire_encode_era_discriminator_exit",
    "wire/runtime_wire_roundtrip_primitive_exit",
    "wire/runtime_wire_decode_ranged_field_exit",
    "wire/runtime_wire_decode_ranged_repeated_exit",
    "wire/runtime_wire_decode_rejects_noncanonical_bool_exit",
    "wire/runtime_wire_decode_rejects_noncanonical_varint_exit",
    "wire/runtime_wire_decode_rejects_scalar_width_overflow_exit",
    "wire/runtime_wire_decode_rejects_wrong_era_exit",
    "wire/runtime_wire_encode_string_exit",
    "wire/runtime_wire_encode_byte_slice_exit",
    "wire/runtime_wire_encode_borrowed_scalar_slice_exit",
    "wire/runtime_wire_decode_byte_slice_exit",
    "wire/runtime_wire_decoded_byte_slice_len_exit",
    "wire/runtime_wire_decoded_byte_slice_index_exit",
    "wire/runtime_wire_roundtrip_repeated_exit",
    "wire/runtime_wire_exact_array_without_count_exit",
    "wire/runtime_wire_decode_rejects_repeated_overflow_exit",
    // --- 2026-06-12 canary coverage sweep (feature-edge additions) ---
    "wire/runtime_wire_roundtrip_repeated_max_one_exit",
    "wire/runtime_wire_encode_repeated_then_string_exit",
    "wire/runtime_wire_roundtrip_nested_and_repeated_exit",
    "comptime/runtime_const_array_length_transitive_exit",
    "comptime/runtime_const_array_length_bare_call_arm_exit",
    "data/runtime_case_membership_mixed_shape_exit",
    "traits/runtime_equatable_scalar_not_equals_guard_exit",
    "borrow/runtime_view_of_view_chain_exit",
    "borrow/runtime_method_view_write_after_last_use_exit",
    // Frontend-only scheduler/carry fixtures live in `tests/concurrency_carry.rs`.
    // Their assertion ends at checked trees; requiring native host lowering
    // here would test an unrelated, presently absent Scheduler provider.
    // --- ch17 atomics (concurrency stage 1) ---
    "atomics/atomic_field_declared",
    "atomics/runtime_atomic_load_store_exit",
    "atomics/runtime_atomic_fetch_add_exit",
    "atomics/runtime_atomic_fetch_sub_exit",
    "atomics/runtime_atomic_fetch_xor_exit",
    "atomics/runtime_atomic_fetch_or_exit",
    "atomics/runtime_atomic_fetch_and_exit",
    "atomics/runtime_atomic_swap_exit",
    "atomics/runtime_atomic_compare_exchange_exit",
];

const ACTIVE_FAIL_CANARIES: &[&str] = &[
    "modules/export_item_retired",
    "traits/trait_invariant_clause_retired",
    "traits/trait_contract_undeclared_self_member",
    "tasks/task_runtime_provider_contract_narrowing",
    "tasks/task_runtime_selected_provider_missing",
    "data/fixed_array_too_large",
    "collections/deep_nested_runtime_indexed_write_rejected",
    "build/duplicate_program_entry_binding",
    "build/hosted_program_entry_visible_parameter",
    "build/program_entry_returns_value",
    "build/program_entry_receiver_not_zii",
    "build/unknown_program_entry_binding",
    "build/uefi_program_entry_missing_storage_roots",
    "build/uefi_program_entry_unqualified_image",
    "build/uefi_program_entry_local_physical_contract",
    "build/uefi_program_entry_wrong_calling_policy",
    "providers/slot_plan_ambiguous",
    "providers/provider_type_slot_ambiguous",
    "providers/provider_type_slot_unknown",
    "providers/provider_type_target_default_conflict",
    "providers/scoped_provider_selection_outside_build",
    "host/terminal_host_call_value",
    "calls/guarded_value_call_terminal_rejected",
    "boundary/entry_typed_params_unmarked",
    "wire/layout_domain_on_stored_bytes",
    "wire/layout_domain_grammar_not_implemented",
    "wire/layout_domain_unnumbered_schema",
    "wire/layout_domain_on_non_bytes",
    "calls/machine_self_call_recursion_rejected",
    "calls/ambiguous_spliced_second_receiver_rejected",
    "wire/wire_policy_plan_disagrees",
    "wire/wire_compatibility_preservation_unmet",
    "wire/encode_unsupported_field_type",
    "wire/encode_text_field_not_last",
    "wire/encode_case_bearing_value",
    "wire/encode_nested_in_nested",
    "wire/repeated_text_element",
    "wire/repeated_nested_element",
    "wire/borrowed_scalar_slice_decode_requires_storage",
    "inline_asm/asm_cli_requires_machine_authority",
    "inline_asm/asm_popfq_requires_machine_authority",
    "inline_asm/asm_wrmsr_requires_machine_authority",
    "inline_asm/asm_write_cr3_requires_machine_authority",
    "traits/runtime_dyn_varying_field_rejected",
    // --- Language-guide chapter coverage (Ch1-22) ---
    "calls/param_receiver_method_rejected",
    "calls/guard_call_vs_call_rejected",
    "calls/value_call_effectful_arm_rejected",
    "calls/value_call_param_effect_arm_rejected",
    // The accepted-axiom veto remains here pending its separate trust audit.
    "proofs/accepted_axiom_engine_veto",
];

#[derive(Clone, Copy)]
#[allow(dead_code)]
enum PendingCanaryExpectation {
    CurrentlyAccepts,
    CurrentlyRejects { fragment: &'static str },
}

#[allow(dead_code)]
struct PendingCanary {
    path: &'static str,
    expectation: PendingCanaryExpectation,
}

// fixed_array_element_guard was promoted to pass/ once the guard-operand layout
// applied the constant array index (see fixed_array_element_guard_canary_runs and
// runtime_fixed_array_field_guard_exit_canary_runs).
//
// The proofs false twins were promoted to fail/proofs/ when the contract
// entailment engine (validation/src/contract_entailment.rs) landed:
// empty-body proof machines whose contracts lie inside the engine's language
// are now PROVED or REJECTED, never silently accepted. The pass/proofs/
// ladder pins the proving side; the rungs map to engine increments in
// wiki/proof_engine_roadmap.md.
//
// case_payload_native_construction was promoted to pass/data/ when native case
// payload codegen landed (tag-prefix write + payload field writes + tag-only
// guard compares + payload member reads); the compiler-side lowering gate was
// removed with it.
// machine_bound_value_call_unchecked was promoted to fail/generics/ when
// `validate_value_position_calls` landed in validation/src/calls.rs:
// the machine-call type-parameter bound check now runs for VALUE-position
// calls (`let r = self.pick(&self.h)`) via an expression-tree walker that
// mirrors the statement-position `validate_call_node` path.  A companion
// pass canary (generics/machine_bound_satisfied_at_value_call) pins the
// accepted side: `[copy]`-satisfying data types and scalars compile fine.
// const_array_length_bare_call_arm was promoted to
// pass/comptime/runtime_const_array_length_bare_call_arm_exit when the
// parenthesized-lone-call arm body became a VALUE expression (the parser
// defers; symbol assignment re-classifies back into a state transition only
// for sibling-state and self-recursion callees).
// 2026-06-12 canary coverage sweep: all five bugs it pinned as pending were
// fixed and promoted the same day --
// - traits/equatable_string_not_equals_value -> pass/traits/
//   equatable_string_not_equals_exit (String `!=` lowers as the negated
//   TextEquals leaf instead of dropping the String term).
// - traits/equatable_string_equality_guard_unlowered -> pass/traits/
//   equatable_string_equality_guard_exit (guard-position String place
//   compares route through TextEquals).
// - concurrency/spawn_struct_result_miscompiled: by-value struct RETURNS
//   landed (leaf terminal-value StructLiteral substitution + call-result-backed
//   locals keep their name). Task-runtime TR1 later retired the fake spawn
//   wrapper; calls/runtime_free_machine_struct_return_exit keeps the real pin.
// - comptime/const_array_length_bare_call_arm -> pass/comptime/
//   runtime_const_array_length_bare_call_arm_exit (parenthesized lone-call
//   arm bodies are value expressions; sibling-state callees re-classify).
#[allow(dead_code)]
// The f32 scalar-width family + the sequential self-field RMW stale-fold all
// closed 2026-06-14 and are now pass RUN canaries. The float-to-int divergence
// was retired 2026-07-18 when F4's proof-or-policy conversion landed; its exact,
// saturating, trapping, NaN, and Wrapping legs now live in active pass/fail
// coverage rather than this drift ledger.
// Empty: nested_i32_mul_overflow_divergence was promoted to a FAIL canary
// (expressions/nested_i32_mul_overflow) once decision 17 S3 made the unprovable
// i32 multiply a compile error -- the divergence was a symptom of accepting an
// unprovable overflow, now rejected.
// Repopulated 2026-07-10 (the list had drifted EMPTY while 13 pending
// canaries sat on disk unwatched -- every drift recheck was a manual
// `omega run` sweep). Expectations mirror each canary's header; a flip here
// means a parked repro graduated (promote it) or regressed differently
// (rediagnose it). The compile-only check cannot adjudicate the RUNTIME
// divergences (those stay documented in the headers and the periodic
// `omega run --both` sweep), but it pins accepts-vs-rejects drift for free.
const ACTIVE_PENDING_CANARIES: &[PendingCanary] = &[
    // float_to_int_overflow_divergence RETIRED 2026-07-16 by the F4 Exact
    // cast obligation: a BARE out-of-range float->int cast is now a compile
    // error (proof-or-policy), so the pinned three-way native divergence is
    // unreachable; the defined policies are pinned by
    // pass/arithmetic/float_to_int_saturating_exit +
    // trapping_float_to_int_cast_traps (arch-gated; the x86 policy fixups
    // are its host session's rung).
    // immutable_arg_for_mut_param_not_checked PROMOTED to
    // fail/calls/immutable_arg_for_mut_param_rejected (borrow-mutability
    // enforcement landed 2026-07-18).
    // unsigned_min_max_operand_position_divergence PROMOTED to
    // pass/arithmetic/unsigned_min_max_operand_position_exit (carrier CR3
    // landed 2026-07-18: binding-capture stamping + operand-derived
    // anonymous-destination folds carry the landing to the signedness probe).
    // local_slice_forward_segfault PROMOTED to
    // pass/storage/runtime_local_slice_forward_exit (the struct-literal
    // slice-view slot carve-out landed; see the canary header).
    // shift_amount_at_or_above_width_divergence PROMOTED to
    // pass/arithmetic/runtime_shift_atwidth_signed_modular_exit; F8 now pins
    // the final Wrapping rule, count masking by the language operand width.
    // multiarm_texteq_local_divergence PROMOTED to
    // pass/calls/runtime_multiarm_texteq_local_exit: Terminal-value arm
    // expansions now carry their sub-state's call-free LocalData
    // initializers and the leaf writer serves texteq via the frame-slot
    // text-comparison write.
    // dead_trapping_let_not_elided PROMOTED to
    // pass/expressions/dead_trapping_let_traps (abort-as-effect first
    // sentence ruled + landed 2026-07-18: a trap is an effect, never dead).
];

#[path = "canary_suite/atomics_and_target_canaries.rs"]
mod atomics_and_target_canaries;
