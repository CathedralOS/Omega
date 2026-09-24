//! Retired flat paths and prohibited proxy schedules.

use std::fs;

use crate::Audit;

fn is_test_source(path: &str) -> bool {
    path.contains("/tests/") || path.ends_with("/tests.rs") || path.ends_with("_tests.rs")
}

pub(crate) fn check(audit: &mut Audit) {
    let repository = &audit.repository;
    let source_files = &audit.source_files;
    let violations = &mut audit.violations;

    let psi_pass_root =
        "omega-rust/omega/pipeline/04_abstract-operations-to-abstract-operations/src/rules/";
    for path in source_files.iter().filter(|path| {
        path.starts_with(psi_pass_root)
            && !is_test_source(path)
            && (path.ends_with("/rule.rs") || path.ends_with("/rules.rs"))
    }) {
        violations.insert(format!(
            "Psi pass retains a generic rule leaf instead of an exact optimization name: {path}"
        ));
    }

    for obsolete in [
        "omega-rust/omega/compiler/native-realization/src/realization/physical_stage/fragment_shape.rs",
        "omega-rust/omega/pipeline/omega-register-homes-to-callee-saved-requirements/Cargo.toml",
        "omega-rust/omega/pipeline/omega-callee-saved-requirements-to-save-storage/Cargo.toml",
        "omega-rust/omega/pipeline/omega-spill-access-constraints-to-frame-requirements/Cargo.toml",
        "omega-rust/omega/pipeline/omega-frame-layout-to-frame-protocol/Cargo.toml",
        "omega-rust/omega/pipeline/omega-optimization-policy/Cargo.toml",
        "omega-rust/omega/pipeline/omega-optimization-policy/src/lib.rs",
        "omega-rust/omega/pipeline/omega-optimization-pipeline/Cargo.toml",
        "omega-rust/omega/pipeline/omega-optimization-pipeline/src/lib.rs",
        "omega-rust/omega/representations/optimization-core/src/manifest.rs",
        "omega-rust/omega/representations/legalized-operations/src/validation/call_source.rs",
        "omega-rust/omega/representations/optimization-unit/src/identity/operation_encoding.rs",
        "omega-rust/omega/semantics/optimization-unit-semantics/src/candidates/sparse_conditional_constant_propagation/candidate_validation.rs",
        "omega-rust/omega/semantics/optimization-unit-semantics/src/candidates/sparse_conditional_constant_propagation/boolean_evaluation/integer_comparisons.rs",
        "omega-rust/omega/semantics/optimization-unit-semantics/src/candidates/sparse_conditional_constant_propagation/range_comparisons.rs",
        "omega-rust/omega/semantics/optimization-unit-semantics/src/candidates/global_value_numbering/expression_keys.rs",
        "omega-rust/omega/semantics/optimization-unit-semantics/src/tests/structural_catalog.rs",
        "omega-rust/omega/pipeline/selected-instructions-to-selected-instructions/src/analyses/live_ranges/validate/replay.rs",
        "omega-rust/omega/pipeline/selected-instructions-to-selected-instructions/src/analyses/live_ranges/validate/tests.rs",
        "omega-rust/omega/representations/optimization-unit/src/ledger.rs",
        "tests/native-differential/tests/pipeline_ownership/stages/allocation/register_allocation.rs",
        "tests/native-differential/tests/pipeline_ownership/stages/machine/selected_lowering.rs",
        "omega-rust/omega/pipeline/abstract-operations-to-target-operations/src/lowering/scalar/conditional_control.rs",
        "omega-rust/omega/pipeline/abstract-operations-to-target-operations/src/lowering/scalar/conditional_control/mod.rs",
        "omega-rust/omega/pipeline/abstract-operations-to-target-operations/src/lowering/scalar/conditional_scalar/mod.rs",
        "omega-rust/omega/pipeline/abstract-operations-to-target-operations/src/lowering/scalar/straight_line/mod.rs",
        "omega-rust/omega/pipeline/abstract-operations-to-target-operations/src/validation/straight_line_parameter/mod.rs",
        "omega-rust/omega/pipeline/abstract-operations-to-target-operations/src/validation/catalog/dispatch/immediate.rs",
        "omega-rust/omega/pipeline/abstract-operations-to-target-operations/src/validation/straight_line_scalar_crash.rs",
        "omega-rust/omega/compiler/native-realization/src/realization/providers/settlements.rs",
        "omega-rust/omega/pipeline/selected-instructions-to-selected-instructions/src/analyses/machine_effects/facts/codec.rs",
        "omega-rust/omega/pipeline/post-allocation-machine-to-post-allocation-machine/src/rules/aarch64/materialize_i64_movn/compute.rs",
        "omega-rust/omega/pipeline/target-operations-to-selected-instructions/src/selection/validation/blocks.rs",
        "omega-rust/omega/pipeline/target-operations-to-selected-instructions/src/legalization/source/leaves.rs",
        "omega-rust/omega/representations/optimization-unit/src/rewrite/model.rs",
        "omega-rust/omega/representations/optimization-unit/src/construction.rs",
        "omega-rust/omega/backend/machine-emission/src/function_realization/codec.rs",
        "omega-rust/omega/semantics/optimization-unit-semantics/src/candidates/control_flow_cleanup/block_merging.rs",
        "omega-rust/omega/semantics/optimization-unit-semantics/src/candidates/control_flow_cleanup/empty_block_threading.rs",
        "omega-rust/omega/semantics/optimization-unit-semantics/src/candidates/control_flow_cleanup/constant_conditionals.rs",
        "omega-rust/omega/semantics/optimization-unit-semantics/src/candidates/control_flow_cleanup/shared_jump_fusion.rs",
        "omega-rust/omega/semantics/optimization-unit-semantics/src/candidates/control_flow_cleanup/unreachable_private_machines.rs",
        "omega-rust/omega/semantics/optimization-unit-semantics/src/candidates/control_flow_cleanup/empty_block_threading/linear.rs",
        "omega-rust/omega/semantics/optimization-unit-semantics/src/candidates/control_flow_cleanup/empty_block_threading/path_qualified.rs",
        "omega-rust/omega/semantics/optimization-unit-semantics/src/candidates/control_flow_cleanup/block_merging/adjacent.rs",
        "omega-rust/omega/semantics/optimization-unit-semantics/src/candidates/control_flow_cleanup/block_merging/non_adjacent.rs",
        "omega-rust/omega/pipeline/selected-instructions-to-selected-instructions/src/rewrites/allocation_recovery/fixed_view_copy/validate.rs",
        "omega-rust/omega/pipeline/selected-instructions-to-selected-instructions/src/rewrites/literal_arithmetic.rs",
        "omega-rust/omega/pipeline/selected-instructions-to-selected-instructions/src/rewrites/literal_compare.rs",
        "omega-rust/omega/pipeline/selected-instructions-to-selected-instructions/src/rewrites/literal_minuend.rs",
        "omega-rust/omega/pipeline/selected-instructions-to-selected-instructions/src/rewrites/literal_minuend/mod.rs",
        "omega-rust/omega/pipeline/selected-instructions-to-selected-instructions/src/rewrites/literal_minuend/admission.rs",
        "omega-rust/omega/pipeline/selected-instructions-to-selected-instructions/src/rewrites/literal_minuend/rewrite.rs",
        "omega-rust/omega/pipeline/selected-instructions-to-selected-instructions/src/rewrites/literal_minuend/validation.rs",
        "omega-rust/omega/pipeline/selected-instructions-to-selected-instructions/src/rewrites/literal_minuend/tests.rs",
        "omega-rust/omega/pipeline/03_terminal-psi-to-abstract-operations/src/lowering/machine/operation.rs",
    ] {
        if repository.join(obsolete).exists() {
            violations.insert(format!(
                "optimizer representation restored a retired flat or forwarding-wall path: {obsolete}"
            ));
        }
    }

    for (family, relative_root) in [
        ("proof-check elision", "proof_check_elision/"),
        ("control-flow cleanup", "control_flow_cleanup/"),
        ("global value numbering", "global_value_numbering/"),
        (
            "sparse conditional constant propagation",
            "sparse_conditional_constant_propagation/",
        ),
    ] {
        let family_root = format!("{psi_pass_root}{relative_root}");
        for path in source_files
            .iter()
            .filter(|path| path.starts_with(&family_root) && !is_test_source(path))
        {
            match fs::read_to_string(repository.join(path)) {
                Ok(contents)
                    if contents.contains("use super::*;")
                        || contents.contains("use super::super::*;") =>
                {
                    violations.insert(format!(
                        "{family} restored an inherited parent glob dependency: {path}"
                    ));
                }
                Ok(_) => {}
                Err(error) => {
                    violations.insert(format!("cannot read {path}: {error}"));
                }
            }
        }
    }

    let obsolete_post_allocation_manifest = "omega-rust/omega/pipeline/selected-instructions-to-register-homes/src/assignment/post_allocation_manifest.rs";
    if repository.join(obsolete_post_allocation_manifest).exists() {
        violations.insert(format!(
            "register allocation retains the mixed post-allocation manifest file: {obsolete_post_allocation_manifest}"
        ));
    }

    let retired_allocation_entry = "omega-rust/omega/pipeline/selected-instructions-to-register-homes/src/assignment/current.rs";
    if repository.join(retired_allocation_entry).exists() {
        violations.insert(format!(
            "register allocation retains its entry beneath the assignment group instead of the crate root: {retired_allocation_entry}"
        ));
    }

    for family in [
        "abstract_spill_access_constraints",
        "abstract_spill_insertion",
        "abstract_spill_memory_effects",
        "generalized_reload_value_homes",
        "generalized_spill_insertion",
        "generalized_spill_recovery_actions",
        "generalized_spill_recovery_choice",
        "generalized_spill_recovery_worklist",
        "recursive_reload_value_homes",
        "recursive_spill_insertion",
        "reload_value_homes",
        "spill_pseudo_instructions",
        "spill_recovery_actions",
        "spill_recovery_choice",
        "spill_recovery_worklist",
        "synthetic_reload_values",
    ] {
        let obsolete = format!(
            "omega-rust/omega/pipeline/selected-instructions-to-register-homes/src/assignment/{family}"
        );
        if repository.join(&obsolete).exists() {
            violations.insert(format!(
                "an unsequenced spill boundary sits beside the stages register allocation sequences: {obsolete}"
            ));
        }
    }

    for obsolete in [
        "omega-rust/omega/pipeline/selected-instructions-to-register-homes/src/assignment/home_assignment/compute.rs",
        "omega-rust/omega/pipeline/selected-instructions-to-register-homes/src/assignment/home_assignment/validate.rs",
        "omega-rust/omega/pipeline/selected-instructions-to-register-homes/src/assignment/home_assignment/compute_tests.rs",
    ] {
        if repository.join(obsolete).exists() {
            violations.insert(format!(
                "register-home assignment retains a retired flat compute/replay/fixture leaf: {obsolete}"
            ));
        }
    }

    for obsolete in [
        "omega-rust/omega/pipeline/selected-instructions-to-register-homes/src/unsequenced_spill_stages/logical_spill_operations.rs",
        "omega-rust/omega/pipeline/selected-instructions-to-register-homes/src/unsequenced_spill_stages/logical_spill_operations/compute.rs",
        "omega-rust/omega/pipeline/selected-instructions-to-register-homes/src/unsequenced_spill_stages/logical_spill_operations/validate.rs",
        "omega-rust/omega/pipeline/selected-instructions-to-register-homes/src/unsequenced_spill_stages/logical_spill_operations/codec.rs",
        "omega-rust/omega/pipeline/selected-instructions-to-register-homes/src/unsequenced_spill_stages/logical_spill_operations/tests.rs",
        "omega-rust/omega/pipeline/selected-instructions-to-register-homes/src/assignment/logical_spill_operations.rs",
        "omega-rust/omega/pipeline/selected-instructions-to-register-homes/src/assignment/logical_spill_operations/compute.rs",
        "omega-rust/omega/pipeline/selected-instructions-to-register-homes/src/assignment/logical_spill_operations/validate.rs",
        "omega-rust/omega/pipeline/selected-instructions-to-register-homes/src/assignment/logical_spill_operations/codec.rs",
        "omega-rust/omega/pipeline/selected-instructions-to-register-homes/src/assignment/logical_spill_operations/tests.rs",
    ] {
        if repository.join(obsolete).exists() {
            violations.insert(format!(
                "logical spill planning retains a retired flat entrance or mixed leaf: {obsolete}"
            ));
        }
    }

    let retired_coloring_home = "omega-rust/omega/pipeline/selected-instructions-to-register-homes/src/unsequenced_spill_stages/stack_slot_coloring";
    if repository.join(retired_coloring_home).exists() {
        violations.insert(format!(
            "stack-slot coloring retains an unsequenced home beside its sequenced assignment module: {retired_coloring_home}"
        ));
    }

    for obsolete in [
        "omega-rust/omega/pipeline/selected-instructions-to-selected-instructions/src/rewrites/allocation_recovery/fixed_view_copy/codec.rs",
        "omega-rust/omega/pipeline/selected-instructions-to-selected-instructions/src/rewrites/allocation_recovery/fixed_view_copy/codec_tests.rs",
    ] {
        if repository.join(obsolete).exists() {
            violations.insert(format!(
                "fixed-view-copy retains a retired flat codec surface: {obsolete}"
            ));
        }
    }

    for obsolete in [
        "omega-rust/omega/pipeline/abstract-operations-to-target-operations/src/tests/translation_validation.rs",
        "omega-rust/omega/pipeline/abstract-operations-to-target-operations/src/tests/translation_validation_boolean.rs",
        "omega-rust/omega/pipeline/abstract-operations-to-target-operations/src/tests/translation_validation_crash.rs",
        "omega-rust/omega/pipeline/abstract-operations-to-target-operations/src/tests/translation_validation_integer_bitwise_not_parameter.rs",
        "omega-rust/omega/pipeline/abstract-operations-to-target-operations/src/tests/translation_validation_integer_less_or_equal_parameters.rs",
        "tests/native-differential/tests/pipeline_ownership/stages/selection/optimized_target_operations/comparison.rs",
        "tests/native-differential/tests/pipeline_ownership/stages/selection/optimized_target_operations/unary.rs",
        "omega-rust/omega/pipeline/abstract-operations-to-target-operations/src/tests/parameter_translation_fixture/bitwise.rs",
        "tests/native-differential/tests/pipeline_ownership/fixtures/target_translation/bitwise.rs",
        "tests/native-differential/tests/pipeline_ownership/stages/selection/optimized_target_operations/bitwise.rs",
    ] {
        if repository.join(obsolete).exists() {
            violations.insert(format!(
                "translation validation retains a retired mixed test leaf: {obsolete}"
            ));
        }
    }

    let obsolete_selected_lowering_schedule =
        "omega-rust/omega/compiler/native-realization/src/stages/machine/literal_folds/schedule.rs";
    if repository
        .join(obsolete_selected_lowering_schedule)
        .exists()
    {
        violations.insert(format!(
            "selected lowering retains a proxy schedule beside its owning rule catalog: {obsolete_selected_lowering_schedule}"
        ));
    }

    let obsolete_external_policy_schema =
        "omega-rust/omega/representations/optimization-core/src/decisions/external_schema.rs";
    if repository.join(obsolete_external_policy_schema).exists() {
        violations.insert(format!(
            "external policy retains the mixed flat schema beside its governed entrance: {obsolete_external_policy_schema}"
        ));
    }
    for obsolete in [
        "omega-rust/omega/pipeline/target-operations-to-selected-instructions/src/selection/construction/plan.rs",
        "omega-rust/omega/pipeline/target-operations-to-selected-instructions/src/selection/construction/scalar.rs",
    ] {
        if repository.join(obsolete).exists() {
            violations.insert(format!(
                "selected construction retains an opaque flat coordinator: {obsolete}"
            ));
        }
    }
    for path in source_files.iter().filter(|path| {
        !is_test_source(path)
            && (path.starts_with(
                "omega-rust/omega/pipeline/selected-instructions-to-register-homes/src/rewrites/",
            ) || path.starts_with(
                "omega-rust/omega/pipeline/selected-instructions-to-selected-instructions/src/rewrites/",
            ) || path.starts_with(
                "omega-rust/omega/compiler/native-realization/src/stages/machine/literal_folds/",
            ))
    }) {
        match fs::read_to_string(repository.join(path)) {
            Ok(contents)
                if contents.contains("SelectedIncomingU12ExactAddAndSubtractImmediateV1")
                    || contents.contains("SelectedLoweringOptimizationSchedule") =>
            {
                violations.insert(format!(
                    "selected lowering retains a hidden combined policy or proxy schedule in {path}"
                ));
            }
            Ok(_) => {}
            Err(error) => {
                violations.insert(format!("cannot read {path}: {error}"));
            }
        }
    }
}
