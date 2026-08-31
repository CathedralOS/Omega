//! Retired flat paths and prohibited proxy schedules.

use std::fs;

use crate::Audit;

use super::bounds::is_test_source;

pub(crate) fn check(audit: &mut Audit) {
    let repository = &audit.repository;
    let source_lines = &audit.source_lines;
    let violations = &mut audit.violations;

    let psi_pass_root =
        "source/omega-rust/omega/pipeline/optimization/omega-psi-optimizer/src/rules/passes/";
    for path in source_lines.keys().filter(|path| {
        path.starts_with(psi_pass_root)
            && !is_test_source(path)
            && (path.ends_with("/rule.rs") || path.ends_with("/rules.rs"))
    }) {
        violations.insert(format!(
            "Psi pass retains a generic rule leaf instead of an exact optimization name: {path}"
        ));
    }

    for obsolete in [
        "source/omega-rust/omega/representations/omega-optimization-core/src/manifest.rs",
        "source/omega-rust/omega/representations/omega-legalized-operations/src/validation/call_source.rs",
        "source/omega-rust/omega/representations/omega-optimization-unit/src/identity/operation_encoding.rs",
        "source/omega-rust/omega/pipeline/optimization/omega-optimization-validation/src/candidates/sparse_conditional_constant_propagation/candidate_validation.rs",
        "source/omega-rust/omega/pipeline/optimization/omega-optimization-validation/src/candidates/global_value_numbering/expression_keys.rs",
        "source/omega-rust/omega/pipeline/optimization/omega-optimization-validation/src/tests/structural_catalog.rs",
        "source/omega-rust/omega/pipeline/optimization/omega-regalloc/src/analyses/live_ranges/validate/replay.rs",
        "source/omega-rust/omega/pipeline/optimization/omega-regalloc/src/analyses/live_ranges/validate/tests.rs",
        "source/omega-rust/omega/representations/omega-optimization-unit/src/ledger.rs",
        "source/omega-rust/omega/pipeline/optimization/omega-optimization-pipeline/src/tests/stages/allocation/register_allocation.rs",
        "source/omega-rust/omega/pipeline/optimization/omega-optimization-pipeline/src/tests/stages/machine/selected_lowering.rs",
        "source/omega-rust/omega/pipeline/omega-abstract-operations-to-target-operations/src/lowering/scalar/conditional_control.rs",
        "source/omega-rust/omega/pipeline/omega-terminal-psi-to-native-artifact/src/realization/providers/settlements.rs",
        "source/omega-rust/omega/pipeline/optimization/omega-machine-optimizer/src/analyses/pre_allocation_effects/codec.rs",
        "source/omega-rust/omega/pipeline/optimization/omega-machine-optimizer/src/rules/aarch64/materialize_i64_movn/compute.rs",
        "source/omega-rust/omega/pipeline/omega-target-operations-to-selected-instructions/src/selection/validation/blocks.rs",
        "source/omega-rust/omega/pipeline/omega-target-operations-to-selected-instructions/src/legalization/source/leaves.rs",
        "source/omega-rust/omega/representations/omega-optimization-unit/src/rewrite/model.rs",
        "source/omega-rust/omega/representations/omega-optimization-unit/src/construction.rs",
        "source/omega-rust/omega/pipeline/optimization/omega-optimization-pipeline/src/stages/realization/function_relative_realization/codec.rs",
        "source/omega-rust/omega/pipeline/optimization/omega-psi-optimizer/src/rules/passes/control_flow_cleanup/block_merging.rs",
        "source/omega-rust/omega/pipeline/optimization/omega-psi-optimizer/src/rules/passes/control_flow_cleanup/empty_block_threading.rs",
        "source/omega-rust/omega/pipeline/optimization/omega-psi-optimizer/src/rules/passes/global_value_numbering/accounting.rs",
        "source/omega-rust/omega/pipeline/optimization/omega-psi-optimizer/src/rules/passes/proof_check_elision/identity_rewrite.rs",
        "source/omega-rust/omega/pipeline/optimization/omega-regalloc/src/rules/allocation_recovery/fixed_view_copy/validate.rs",
        "source/omega-rust/omega/pipeline/omega-psi-to-abstract-operations/src/lowering/machine/operation.rs",
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
    ] {
        let family_root = format!("{psi_pass_root}{relative_root}");
        for path in source_lines
            .keys()
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

    for obsolete in [
        "source/omega-rust/omega/pipeline/optimization/omega-psi-optimizer/src/rules/passes/global_value_numbering/identities/rule.rs",
        "source/omega-rust/omega/pipeline/optimization/omega-psi-optimizer/src/rules/passes/global_value_numbering/identities/shapes.rs",
    ] {
        if repository.join(obsolete).exists() {
            violations.insert(format!(
                "global-value-numbering identities retain a mixed catch-all: {obsolete}"
            ));
        }
    }

    let obsolete_post_allocation_manifest = "source/omega-rust/omega/pipeline/optimization/omega-regalloc/src/allocation/post_allocation_manifest.rs";
    if repository.join(obsolete_post_allocation_manifest).exists() {
        violations.insert(format!(
            "register allocation retains the mixed post-allocation manifest file: {obsolete_post_allocation_manifest}"
        ));
    }

    for obsolete in [
        "source/omega-rust/omega/pipeline/optimization/omega-regalloc/src/allocation/home_assignment/compute.rs",
        "source/omega-rust/omega/pipeline/optimization/omega-regalloc/src/allocation/home_assignment/validate.rs",
        "source/omega-rust/omega/pipeline/optimization/omega-regalloc/src/allocation/home_assignment/compute_tests.rs",
    ] {
        if repository.join(obsolete).exists() {
            violations.insert(format!(
                "register-home assignment retains a retired flat compute/replay/fixture leaf: {obsolete}"
            ));
        }
    }

    for obsolete in [
        "source/omega-rust/omega/pipeline/optimization/omega-regalloc/src/rules/allocation_recovery/fixed_view_copy/codec.rs",
        "source/omega-rust/omega/pipeline/optimization/omega-regalloc/src/rules/allocation_recovery/fixed_view_copy/codec_tests.rs",
    ] {
        if repository.join(obsolete).exists() {
            violations.insert(format!(
                "fixed-view-copy retains a retired flat codec surface: {obsolete}"
            ));
        }
    }

    for obsolete in [
        "source/omega-rust/omega/pipeline/omega-abstract-operations-to-target-operations/src/tests/translation_validation.rs",
        "source/omega-rust/omega/pipeline/omega-abstract-operations-to-target-operations/src/tests/translation_validation_boolean.rs",
        "source/omega-rust/omega/pipeline/omega-abstract-operations-to-target-operations/src/tests/translation_validation_crash.rs",
        "source/omega-rust/omega/pipeline/omega-abstract-operations-to-target-operations/src/tests/translation_validation_integer_bitwise_not_parameter.rs",
        "source/omega-rust/omega/pipeline/omega-abstract-operations-to-target-operations/src/tests/translation_validation_integer_less_or_equal_parameters.rs",
        "source/omega-rust/omega/pipeline/optimization/omega-optimization-pipeline/src/tests/stages/selection/optimized_target_operations/comparison.rs",
        "source/omega-rust/omega/pipeline/optimization/omega-optimization-pipeline/src/tests/stages/selection/optimized_target_operations/unary.rs",
        "source/omega-rust/omega/pipeline/omega-abstract-operations-to-target-operations/src/tests/parameter_translation_fixture/bitwise.rs",
        "source/omega-rust/omega/pipeline/optimization/omega-optimization-pipeline/src/tests/fixtures/target_translation/bitwise.rs",
        "source/omega-rust/omega/pipeline/optimization/omega-optimization-pipeline/src/tests/stages/selection/optimized_target_operations/bitwise.rs",
    ] {
        if repository.join(obsolete).exists() {
            violations.insert(format!(
                "translation validation retains a retired mixed test leaf: {obsolete}"
            ));
        }
    }

    let obsolete_selected_lowering_schedule = "source/omega-rust/omega/pipeline/optimization/omega-optimization-pipeline/src/stages/machine/literal_folds/schedule.rs";
    if repository
        .join(obsolete_selected_lowering_schedule)
        .exists()
    {
        violations.insert(format!(
            "selected lowering retains a proxy schedule beside its owning rule catalog: {obsolete_selected_lowering_schedule}"
        ));
    }

    let obsolete_external_policy_schema = "source/omega-rust/omega/pipeline/optimization/omega-optimization-policy/src/external_schema.rs";
    if repository.join(obsolete_external_policy_schema).exists() {
        violations.insert(format!(
            "external policy retains the mixed flat schema beside its governed entrance: {obsolete_external_policy_schema}"
        ));
    }
    for obsolete in [
        "source/omega-rust/omega/pipeline/omega-target-operations-to-selected-instructions/src/selection/construction/plan.rs",
        "source/omega-rust/omega/pipeline/omega-target-operations-to-selected-instructions/src/selection/construction/scalar.rs",
    ] {
        if repository.join(obsolete).exists() {
            violations.insert(format!(
                "selected construction retains an opaque flat coordinator: {obsolete}"
            ));
        }
    }
    for path in source_lines.keys().filter(|path| {
        !is_test_source(path)
            && (path.starts_with(
                "source/omega-rust/omega/pipeline/optimization/omega-regalloc/src/rules/",
            ) || path.starts_with(
                "source/omega-rust/omega/pipeline/optimization/omega-optimization-pipeline/src/stages/machine/literal_folds/",
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
